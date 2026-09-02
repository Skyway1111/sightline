//! The oracle-established argument types at call sites, batched once for all
//! #5 candidates and #2's grounding. The type-string algebra is
//! `typestrings.rs`'s.

use indexmap::IndexMap;
use ruff_python_ast::{Expr, ExprCall, Stmt, StmtFunctionDef};
use serde_json::{Value, json};

use sightline_core::findings::Qname;
use sightline_py_facts::astutil::{fn_defaults, fn_pos_args};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::model::{CallSiteId, NodeIndex, RepoFacts, Symbol, is_test_path};
use sightline_py_facts::module::Module;

use crate::Provers;
use crate::callgraph::CallGraph;
use crate::oracle::TypeQuery;

/// What a call site binds to one parameter (R19: Python's `OMITTED` sentinel
/// and its `None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    Expr(NodeIndex),
    /// the call demonstrably relies on the default
    Omitted,
    /// a splat makes the binding unknowable
    Unknown,
}

/// The expression each prod call site binds to `param`: #14's in-hand test
/// reads one enumeration.
pub fn prod_args(facts: &RepoFacts<'_>, calls: &CallGraph, sym: &Symbol, param: &str) -> Vec<Arg> {
    prod_arg_sites(facts, calls, sym, param)
        .into_iter()
        .map(|(_, arg)| arg)
        .collect()
}

/// `prod_args` with the module each argument node lives in: #14 reads the
/// expression, `prod_args` only its shape.
pub fn prod_arg_sites<'a, 't>(
    facts: &'a RepoFacts<'t>,
    calls: &CallGraph,
    sym: &Symbol,
    param: &str,
) -> Vec<(&'a Module<'t>, Arg)> {
    let Some(fn_def) = func_def(facts, sym) else {
        return Vec::new();
    };
    let idx = position_of(fn_def, param);
    calls
        .callers(&sym.qname)
        .filter(|c| !facts.rel_of(&c.module).is_some_and(|rel| is_test_path(rel)))
        .filter_map(|c| {
            let module = facts.modules.get(&c.module)?;
            Some((module, arg_expr(module.call_at(c.node)?, idx, param)))
        })
        .collect()
}

/// The expression bound to the param, `Unknown` where a splat makes it
/// unknowable, `Omitted` where the call demonstrably relies on the default.
pub fn arg_expr(call: &ExprCall, idx: usize, param: &str) -> Arg {
    let args = &call.arguments.args;
    let keywords = &call.arguments.keywords;
    if idx < args.len() {
        if args[..=idx].iter().any(|a| matches!(a, Expr::Starred(_))) {
            return Arg::Unknown; // a preceding splat shifts positions unknowably
        }
        return stamped(&args[idx]);
    }
    if let Some(kw) = keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == param))
    {
        return stamped(&kw.value);
    }
    if args.iter().any(|a| matches!(a, Expr::Starred(_)))
        || keywords.iter().any(|kw| kw.arg.is_none())
    {
        return Arg::Unknown; // *args / **kwargs may fill it
    }
    Arg::Omitted
}

/// One call site's contribution to a parameter's observed type, or the
/// parameter's own default (`call: None`, the invariant source).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgObservation {
    pub call: Option<CallSiteId>,
    /// `None`: the oracle gave no answer (multi-line expression, lossy
    /// module, miss)
    pub ty: Option<String>,
}

/// Every closed-world `(callee, param)` worth querying, unannotated params
/// (#5 candidates) and guarded params (#2's grounding), with every argument
/// type resolved in one `Oracle::span_types` batch. Without an oracle it is
/// the empty answer: no queries, no observations.
#[derive(Debug, Default)]
pub struct ArgTypes {
    table: IndexMap<(Qname, String), Vec<ArgObservation>>,
}

impl ArgTypes {
    /// Reads `provers.oracle()`, `provers.closed_world(facts)`,
    /// `provers.calls(facts)` (the oracle-upgraded caller table) and
    /// `provers.scope_of` (the guarded params).
    pub fn new(facts: &RepoFacts<'_>, provers: &Provers) -> ArgTypes {
        let Some(oracle) = provers.oracle() else {
            return ArgTypes::default();
        };
        let (queries, slots) = enumerate(facts, provers);
        let answers = oracle.span_types(&queries);
        ArgTypes {
            table: slots
                .into_iter()
                .map(|(key, rows)| {
                    let observed = rows
                        .into_iter()
                        .map(|(call, at)| ArgObservation {
                            call,
                            ty: at.and_then(|i| answers.get(i).cloned().flatten()),
                        })
                        .collect();
                    (key, observed)
                })
                .collect(),
        }
    }

    pub fn for_param(&self, callee: &str, param: &str) -> Option<&[ArgObservation]> {
        self.table
            .get(&(Qname::from(callee), param.to_string()))
            .map(Vec::as_slice)
    }

    /// `_oracle_answers`' `arg_types`: sorted `[callee, param, observations]`
    /// rows, an observation as `{"call": [rel, line, col] | null, "type"}`.
    pub fn dump_rows(&self, facts: &RepoFacts<'_>) -> Value {
        let mut rows: Vec<(&Qname, &String, &Vec<ArgObservation>)> = self
            .table
            .iter()
            .map(|((callee, param), observed)| (callee, param, observed))
            .collect();
        rows.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        json!(
            rows.into_iter()
                .map(|(callee, param, observed)| json!([
                    &**callee,
                    param,
                    observed
                        .iter()
                        .map(|o| json!({
                            "call": o.call.map(|at| call_position(facts, at)),
                            "type": o.ty,
                        }))
                        .collect::<Vec<_>>()
                ]))
                .collect::<Vec<_>>()
        )
    }
}

/// One slot row: the call site (`None` is the parameter's own default) and
/// the query answering it (`None` is unqueryable).
type Row = (Option<CallSiteId>, Option<usize>);

/// The one batch, and the slots its answers fill.
type Enumeration = (Vec<TypeQuery>, IndexMap<(Qname, String), Vec<Row>>);

/// `_enumerate`: the query batch and the slots it fills, keyed
/// `(callee, param)` in `facts.symbols` order. Reads no oracle, as Python's
/// does not.
fn enumerate(facts: &RepoFacts<'_>, provers: &Provers) -> Enumeration {
    let calls = provers.calls(facts);
    let cw = provers.closed_world(facts);
    let mut queries: Vec<TypeQuery> = Vec::new();
    let mut slots: IndexMap<(Qname, String), Vec<Row>> = IndexMap::new();
    for (qname, sym) in &facts.symbols {
        let sites = calls.calls_to.get(qname).map_or(&[][..], |v| v);
        // a symbol no def backs has no params to align, so it is dropped
        // before anything else is read
        let Some(fn_def) = func_def(facts, sym) else {
            continue;
        };
        if sites.is_empty() || !cw.verdict(qname).passed {
            continue;
        }
        let callee_module = &facts.modules[&sym.module];
        for (param, default) in candidate_params(facts, provers, sym, fn_def) {
            let idx = position_of(fn_def, &param);
            let mut rows: Vec<Row> = Vec::new();
            let default_at = default.and_then(|d| add_query(&mut queries, callee_module, d));
            if default.is_some() {
                rows.push((None, default_at));
            }
            for at in sites {
                let site = &calls.sites[*at as usize];
                let Some(module) = facts.modules.get(&site.module) else {
                    continue;
                };
                let Some(call) = module.call_at(site.node) else {
                    continue;
                };
                rows.push((
                    Some(*at),
                    match arg_expr(call, idx, &param) {
                        Arg::Omitted => default_at, // the callee sees the default here
                        Arg::Unknown => None,
                        Arg::Expr(expr) => add_query(&mut queries, module, expr),
                    },
                ));
            }
            slots.insert((qname.clone(), param), rows);
        }
    }
    (queries, slots)
}

/// `_candidate_params`: `(name, default expr)`, defaulted params included -
/// the default is an invariant source the union must model.
fn candidate_params<'t>(
    facts: &RepoFacts<'t>,
    provers: &Provers,
    sym: &Symbol,
    fn_def: &'t StmtFunctionDef,
) -> Vec<(String, Option<NodeIndex>)> {
    let module = &facts.modules[&sym.module];
    let defaults: IndexMap<&str, &Expr> = fn_defaults(fn_def)
        .into_iter()
        .map(|(a, d)| (a.name.as_str(), d))
        .collect();
    let guarded: Vec<&str> = provers
        .scope_of(facts, &sym.qname)
        .map(|s| s.guards(facts).iter().map(|g| g.param.as_str()).collect())
        .unwrap_or_default();
    fn_pos_args(fn_def)
        .into_iter()
        .filter(|a| {
            let annotated = Cn::Param(a)
                .stamped()
                .is_some_and(|at| module.annotation(at).is_some());
            !annotated || guarded.contains(&a.name.as_str())
        })
        .map(|a| {
            let default = defaults
                .get(a.name.as_str())
                .and_then(|d| Cn::Expr(d).stamped());
            (a.name.to_string(), default)
        })
        .collect()
}

/// The query for one expression, `None` where it is multi-line or its module
/// is lossy: byte columns the file does not share are unqueryable.
fn add_query(queries: &mut Vec<TypeQuery>, module: &Module<'_>, expr: NodeIndex) -> Option<usize> {
    let span = module.span(expr)?;
    let line = span[0]?;
    if span[2] != Some(line) || module.lossy {
        return None;
    }
    let col_start = span[1].unwrap_or_default();
    queries.push(TypeQuery {
        id: format!("q{}", queries.len()),
        rel: module.rel.clone(),
        line,
        col_start,
        col_end: match span[3] {
            Some(0) | None => col_start,
            Some(end) => end,
        },
    });
    Some(queries.len() - 1)
}

/// The positional index of `param`, `usize::MAX` where the signature has no
/// such slot (R19: Python's `10**9`).
fn position_of(fn_def: &StmtFunctionDef, param: &str) -> usize {
    fn_pos_args(fn_def)
        .iter()
        .position(|a| a.name.as_str() == param)
        .unwrap_or(usize::MAX)
}

fn stamped(expr: &Expr) -> Arg {
    match Cn::Expr(expr).stamped() {
        Some(at) => Arg::Expr(at),
        None => Arg::Unknown,
    }
}

fn func_def<'t>(facts: &RepoFacts<'t>, sym: &Symbol) -> Option<&'t StmtFunctionDef> {
    match facts.modules.get(&sym.module)?.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => Some(f),
        _ => None,
    }
}

/// `[rel, line, col]` of a call site, as `_oracle_answers` prints it.
fn call_position(facts: &RepoFacts<'_>, at: CallSiteId) -> Value {
    let site = &facts.call_sites[at as usize];
    let module = &facts.modules[&site.module];
    let span = module.span(site.node).unwrap_or_default();
    json!([
        &*module.rel,
        span[0].unwrap_or_default(),
        span[1].unwrap_or_default()
    ])
}

/// A mini repo built in place. `sightline_testkit::build` cannot serve a unit
/// test here: the dev-dependency cycle gives the test binary a second copy of
/// this crate, whose `Provers` is a different type.
#[cfg(test)]
pub(crate) fn mini_repo(
    files: &[(&str, &[u8])],
) -> (tempfile::TempDir, sightline_py_facts::build::PyBuilt) {
    let dir = tempfile::tempdir().expect("a temp dir for the mini repo");
    for (rel, bytes) in files {
        std::fs::write(dir.path().join(rel), bytes).expect("the mini repo's files");
    }
    let root = camino::Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let config = sightline_core::config::Config::new();
    let listing = sightline_core::walk::discover(root, &config);
    let built = sightline_py_facts::build::build_facts(root, &config, &listing, None);
    (dir, built)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A defaulted receiver must not shift default alignment onto later
    /// params.
    #[test]
    fn candidate_params_align_defaults_before_dropping_the_receiver() {
        let (_dir, built) = mini_repo(&[(
            "m.py",
            b"class C:\n    def method(self=None, value=1): pass\n",
        )]);
        let facts = built.borrow_dependent();
        let provers = Provers::bare(facts);
        let sym = &facts.symbols["m.C.method"];
        let fn_def = func_def(facts, sym).expect("a def node");
        let module = &facts.modules[&sym.module];

        let candidates = candidate_params(facts, &provers, sym, fn_def);

        let spelled: Vec<(String, String)> = candidates
            .into_iter()
            .map(|(name, default)| {
                let text = match default.map(|at| module.nodes[at as usize]) {
                    Some(Cn::Expr(e)) => sightline_py_facts::unparse::expr(e),
                    _ => String::new(),
                };
                (name, text)
            })
            .collect();
        assert_eq!(spelled, [("value".to_string(), "1".to_string())]);
    }

    /// A lossy module's byte columns are no one else's: no `TypeQuery` names
    /// one (`test_a_lossily_decoded_module_is_named_and_never_queried`, the
    /// "never queried" half, for both query kinds).
    #[test]
    fn no_query_names_a_lossy_module() {
        // `# caf\xe9` is latin-1: facts decode it as U+FFFD, three bytes
        let (_dir, built) = mini_repo(&[(
            "m.py",
            b"# caf\xe9\ndef _scale(n):\n    return n * 2\ndef use(x: int) -> int:\n    return _scale(x) + _scale(x)\n",
        )]);
        let facts = built.borrow_dependent();
        let provers = Provers::bare(facts);
        assert!(facts.modules["m"].lossy);

        let (queries, slots) = enumerate(facts, &provers);
        let receivers = crate::import_effects::ReceiverTypes::enumerate(facts);

        assert!(queries.is_empty(), "{queries:?}");
        assert!(receivers.pending_queries().is_empty());
        // the slot is still enumerated: only its answer is unreachable
        assert!(slots.contains_key(&(Qname::from("m._scale"), "n".to_string())));
    }
}
