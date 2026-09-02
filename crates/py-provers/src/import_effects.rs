//! What an import runs: the import-time work no world can see, and so what
//! an emitter may move (#35) or drop (#32) around.
//! `import_time` walks what importing evaluates, `runs` reads one node
//! against the catalog by its own spelling or the `<class>.<method>` its
//! receiver's type spells, `import_time_effects` folds that over the graph's
//! top-level closure and `binds_only` answers for one import statement. The
//! graph itself is `imports.rs`'s.

use std::collections::{BTreeSet, HashMap};

use ruff_python_ast::{Expr, ExprContext};
use serde_json::{Value, json};

use sightline_core::findings::Qname;
use sightline_py_facts::astutil::is_const_str;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::model::{ModuleId, NodeIndex, RepoFacts};
use sightline_py_facts::module::Module;
use sightline_py_facts::order;

use crate::catalog::inert;
use crate::imports::{ImportGraph, import_targets, internal_module, loads};
use crate::oracle::Oracle;
use crate::typestrings::{generic_base, union_members};

/// Every node importing a module evaluates: class bodies, decorators and
/// defaults included, function bodies excluded (they run when called).
pub fn import_time<'a>(node: Cn<'a>, out: &mut Vec<Cn<'a>>) {
    let mut kids: Vec<Cn<'a>> = Vec::new();
    match node {
        Cn::Stmt(ruff_python_ast::Stmt::FunctionDef(f)) => {
            kids.extend(f.decorator_list.iter().map(|d| Cn::Expr(&d.expression)));
            kids.push(Cn::Params(&f.parameters));
        }
        Cn::Expr(Expr::Lambda(l)) => {
            kids.extend(l.parameters.iter().map(|p| Cn::Params(p)));
        }
        other => order::children(other, &mut kids),
    }
    for child in kids {
        out.push(child);
        import_time(child, out);
    }
}

/// A call's spelling: the callee through the module's bindings, a bare
/// unbound name being a builtin, a method on a string literal
/// `str.<method>`. `None` for a method on a global or on a call's result -
/// only the checker names those (`ReceiverTypes`).
pub fn callee(module: &Module<'_>, f: &Expr) -> Option<String> {
    if let Some(dotted) = module.dotted_name(f) {
        return Some(dotted);
    }
    match f {
        Expr::Name(n) if !module.bindings.contains_key(n.id.as_str()) => Some(n.id.to_string()),
        Expr::Attribute(a) if is_const_str(Some(&a.value)) => Some(format!("str.{}", a.attr)),
        _ => None,
    }
}

pub use crate::oracle::TypeQuery;

/// The class an import-time method call's receiver holds, for the calls
/// `callee` spells to nothing a catalog can hold - a method on a module
/// global (`_KNOT_GRID.setflags(write=False)`) or on a call's result
/// (`Path(__file__).resolve()`). One span query per such receiver
/// (`Oracle::span_types`, one batch); the callee then spells
/// `<class>.<method>`. Without an oracle there are no answers: a degraded run
/// keeps `callee`'s reading and rejects a superset of what the full run
/// rejects.
#[derive(Default)]
pub struct ReceiverTypes {
    queries: Vec<(TypeQuery, Box<str>)>,
    by_call: HashMap<(ModuleId, NodeIndex), usize>,
    /// by query index, only where the answer named one class
    spellings: HashMap<usize, String>,
}

impl ReceiverTypes {
    pub fn new(facts: &RepoFacts<'_>, oracle: Option<&Oracle>) -> ReceiverTypes {
        let Some(oracle) = oracle else {
            return ReceiverTypes::default();
        };
        let mut receivers = ReceiverTypes::enumerate(facts);
        let answers = oracle.span_types(&receivers.pending_queries());
        receivers.spellings = receivers
            .queries
            .iter()
            .enumerate()
            .filter_map(|(i, (_, attr))| {
                let answer = answers.get(i).cloned().flatten();
                let members = union_members(answer.as_deref().unwrap_or("Any"))?;
                let [only] = members.as_slice() else {
                    return None;
                };
                Some((i, format!("{}.{attr}", generic_base(only))))
            })
            .collect();
        receivers
    }

    /// `_queries`: one span query per receiver only the checker can name, in
    /// sorted-module order.
    pub(crate) fn enumerate(facts: &RepoFacts<'_>) -> ReceiverTypes {
        let mut queries: Vec<(TypeQuery, Box<str>)> = Vec::new();
        let mut by_call: HashMap<(ModuleId, NodeIndex), usize> = HashMap::new();
        let mut sorted: Vec<&Qname> = facts.modules.keys().collect();
        sorted.sort();
        for qname in sorted {
            let module = &facts.modules[qname];
            // lossy: no span query reaches it
            let Some(pending) = module_queries(module) else {
                continue;
            };
            for (call, recv, attr) in pending {
                let Some(span) = module.span(recv) else {
                    continue;
                };
                by_call.insert((module.id, call), queries.len());
                queries.push((
                    TypeQuery {
                        id: format!("v{}", queries.len()),
                        rel: module.rel.clone(),
                        line: span[0].unwrap_or_default(),
                        col_start: span[1].unwrap_or_default(),
                        col_end: match span[3] {
                            Some(0) | None => span[1].unwrap_or_default(),
                            Some(end) => end,
                        },
                    },
                    attr,
                ));
            }
        }
        ReceiverTypes {
            queries,
            by_call,
            spellings: HashMap::new(),
        }
    }

    pub fn pending_queries(&self) -> Vec<TypeQuery> {
        self.queries.iter().map(|(q, _)| q.clone()).collect()
    }

    /// `<class>.<method>` for this call, `None` where the receiver holds no
    /// single class the checker names (unqueried, unanswered, `Any`, a union).
    /// Without an oracle that is every call.
    pub fn spelling(&self, module: &Module<'_>, call: NodeIndex) -> Option<String> {
        let at = self.by_call.get(&(module.id, call))?;
        self.spellings.get(at).cloned()
    }

    /// `_oracle_answers`' `recv_types`: sorted `[rel, line, col_start,
    /// spelling | null]` rows, one per query.
    pub fn dump_rows(&self) -> Value {
        let mut rows: Vec<(&str, u32, u32, Option<&String>)> = self
            .queries
            .iter()
            .enumerate()
            .map(|(i, (q, _))| (&*q.rel, q.line, q.col_start, self.spellings.get(&i)))
            .collect();
        rows.sort();
        json!(rows)
    }
}

/// The receiver spans one module asks about, `None` where its import time
/// already runs something no query could lift (its verdict is settled).
fn module_queries(module: &Module<'_>) -> Option<Vec<(NodeIndex, NodeIndex, Box<str>)>> {
    if module.lossy {
        return None;
    }
    let mut walked: Vec<Cn<'_>> = Vec::new();
    import_time(module.nodes[0], &mut walked);
    let mut pending = Vec::new();
    for node in walked {
        match node {
            Cn::Expr(Expr::Attribute(a)) => {
                if a.ctx == ExprContext::Store {
                    return None;
                }
                continue;
            }
            Cn::Expr(Expr::Subscript(s)) => {
                if s.ctx == ExprContext::Store {
                    return None;
                }
                continue;
            }
            Cn::Expr(Expr::Call(c)) => {
                if inert(callee(module, &c.func).as_deref()) {
                    continue;
                }
                let Expr::Attribute(f) = &*c.func else {
                    // work no receiver type could lift
                    return None;
                };
                let (Some(call), Some(recv)) = (node.stamped(), Cn::Expr(&f.value).stamped())
                else {
                    return None;
                };
                let span = module.span(recv)?;
                if span[0] != span[2] {
                    return None;
                }
                pending.push((call, recv, Box::from(f.attr.as_str())));
            }
            _ => {}
        }
    }
    Some(pending)
}

/// Import-time work no world can see: a store through an attribute or
/// subscript (`core.REGISTRY["p"] = 1`, a registration another module reads),
/// or a call the catalog calls inert neither by its own spelling (`callee`)
/// nor by the `<class>.<method>` its receiver's type spells.
pub fn runs(module: &Module<'_>, node: Cn<'_>, receivers: &ReceiverTypes) -> bool {
    match node {
        Cn::Expr(Expr::Attribute(a)) => a.ctx == ExprContext::Store,
        Cn::Expr(Expr::Subscript(s)) => s.ctx == ExprContext::Store,
        Cn::Expr(Expr::Call(c)) => {
            if inert(callee(module, &c.func).as_deref()) {
                return false;
            }
            let spelled = node.stamped().and_then(|at| receivers.spelling(module, at));
            !inert(spelled.as_deref())
        }
        _ => false,
    }
}

/// Internal modules an import may not be moved or dropped around: import time
/// runs something in their own body or anywhere their top-level closure
/// loads.
pub fn import_time_effects(
    facts: &RepoFacts<'_>,
    graph: &ImportGraph,
    receivers: &ReceiverTypes,
) -> BTreeSet<Qname> {
    let direct: BTreeSet<Qname> = facts
        .modules
        .values()
        .filter(|module| {
            let mut walked: Vec<Cn<'_>> = Vec::new();
            import_time(module.nodes[0], &mut walked);
            walked.into_iter().any(|n| runs(module, n, receivers))
        })
        .map(|module| module.qname.clone())
        .collect();
    facts
        .modules
        .keys()
        .filter(|q| loads(graph, q).iter().any(|m| direct.contains(m)))
        .cloned()
        .collect()
}

/// Does this import only bind names? Every target has to be an internal
/// module outside `import_time_effects` (a stdlib one may snapshot).
pub fn binds_only(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    node: NodeIndex,
    effects: &BTreeSet<Qname>,
) -> bool {
    let targets = import_targets(facts, module, node);
    !targets.is_empty()
        && targets
            .iter()
            .all(|t| internal_module(facts, t).is_some_and(|d| !effects.contains(d)))
}
