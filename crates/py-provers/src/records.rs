//! Port of `provers/records.py` (codemap 3.3): the string-key sets a
//! function returns as dict literals (a closed producer) and the keys each
//! receiver of its result reads by constant (a sink), joined over RESOLVED
//! call sites. Closure is literal: a producer closes only when every return
//! path is a >= 3-key literal and the body cannot fall off its end; a sink
//! closes only when the param or local holding the record is read by constant
//! key and nothing else. Anything else is open and contributes nothing.
//! Serves #57. Bindings resolve through the call graph only, never the
//! oracle's return types.

use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;
use ruff_python_ast::{CmpOp, Expr, ExprContext, Stmt, UnaryOp};
use serde_json::{Map, Value, json};

use sightline_core::findings::Qname;
use sightline_py_facts::astutil::{RECEIVERS, fn_pos_args};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{CallSite, FUNCTION_KINDS, NodeIndex, RepoFacts, Resolution};
use sightline_py_facts::module::Module;

use crate::Provers;
use crate::callgraph::CallGraph;
use crate::scope::Scope;

pub const RECORD_MIN_KEYS: usize = 3;
const READ_METHODS: [&str; 3] = ["get", "pop", "setdefault"];
/// A read that changes the key set.
const RESHAPES: [&str; 2] = ["pop", "setdefault"];
const REBINDS: [&str; 6] = ["name", "del", "except", "import", "global", "nonlocal"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// closed producer qname
    pub producer: Qname,
    /// the function whose param or local receives the record
    pub sink: Qname,
    /// that param or local; "" when the result escapes unnamed
    pub name: String,
    /// keys the sink reads by constant; `None` is open
    pub reads: Option<BTreeSet<String>>,
}

#[derive(Default)]
pub struct Records {
    /// closed producer -> key set per return path, each shape sorted
    pub produced: IndexMap<Qname, Vec<BTreeSet<String>>>,
    /// every RESOLVED site of a closed producer yields >= 1
    pub edges: Vec<Edge>,
}

fn str_const(node: Option<&Expr>) -> Option<&str> {
    match node {
        Some(Expr::StringLiteral(s)) => Some(s.value.to_str()),
        _ => None,
    }
}

/// Keys of a record literal: >= 3 string-constant keys, no `**` spread, no
/// duplicate; `None` for anything else.
pub fn record_keys(node: Option<&Expr>) -> Option<BTreeSet<String>> {
    let Some(Expr::Dict(d)) = node else {
        return None;
    };
    if d.items.len() < RECORD_MIN_KEYS {
        return None;
    }
    let keys: Vec<&str> = d
        .items
        .iter()
        .filter_map(|i| str_const(i.key.as_ref()))
        .collect();
    let set: BTreeSet<String> = keys.iter().map(|k| (*k).to_string()).collect();
    if keys.len() != d.items.len() || set.len() != keys.len() {
        return None;
    }
    Some(set)
}

/// The CPython `orelse` of an `If`, which ruff spells as elif/else clauses:
/// an `elif` is one nested `If` statement, a plain `else` is its body.
fn clauses_return(rest: &[ruff_python_ast::ElifElseClause]) -> bool {
    match rest.first() {
        None => false,
        Some(c) if c.test.is_some() => always_returns(&c.body) && clauses_return(&rest[1..]),
        Some(c) => always_returns(&c.body),
    }
}

/// Every path through the block ends in a return or raise: a proof, the
/// direction #33's provable-fall-through reading does not need.
pub fn always_returns(stmts: &[Stmt]) -> bool {
    match stmts.last() {
        Some(Stmt::Return(_) | Stmt::Raise(_)) => true,
        Some(Stmt::If(n)) => always_returns(&n.body) && clauses_return(&n.elif_else_clauses),
        Some(Stmt::With(n)) => always_returns(&n.body),
        Some(Stmt::Try(n)) => {
            always_returns(&n.finalbody)
                || ((always_returns(&n.body) || always_returns(&n.orelse))
                    && n.handlers.iter().all(|h| {
                        let ruff_python_ast::ExceptHandler::ExceptHandler(h) = h;
                        always_returns(&h.body)
                    }))
        }
        Some(Stmt::Match(n)) => match n.cases.last() {
            Some(last) => {
                matches!(&last.pattern, ruff_python_ast::Pattern::MatchAs(p) if p.pattern.is_none())
                    && last.guard.is_none()
                    && n.cases.iter().all(|c| always_returns(&c.body))
            }
            None => false,
        },
        _ => false,
    }
}

// --- per-name uses inside one function ---------------------------------------

#[derive(Default)]
struct Uses {
    reads: BTreeSet<String>,
    /// (call node, the name passed whole)
    forwards: Vec<(NodeIndex, NodeIndex)>,
    /// rebound, mutated, iterated, spread, returned, ...
    escaped: bool,
}

/// The test of an `If` (an `elif` included), a `While`, an `Assert` or an
/// `IfExp`: `records.py`'s `_TESTS`.
fn test_of<'t>(module: &Module<'t>, node: NodeIndex) -> Option<&'t Expr> {
    match module.nodes[node as usize] {
        Cn::Stmt(Stmt::If(n)) => Some(&n.test),
        Cn::Elif(rest) => rest[0].test.as_ref(),
        Cn::Stmt(Stmt::While(n)) => Some(&n.test),
        Cn::Stmt(Stmt::Assert(n)) => Some(&n.test),
        Cn::Expr(Expr::If(n)) => Some(&n.test),
        _ => None,
    }
}

/// `if p`, `if not p and ...`: the value is only truth-tested.
fn truth_tested(module: &Module<'_>, node: NodeIndex) -> bool {
    let mut cur = node;
    let mut parent = module.parent_of(node);
    while let Some(at) = parent {
        let wrapper = match module.nodes[at as usize] {
            Cn::Expr(Expr::BoolOp(_)) => true,
            Cn::Expr(Expr::UnaryOp(u)) => u.op == UnaryOp::Not,
            _ => false,
        };
        if !wrapper {
            break;
        }
        cur = at;
        parent = module.parent_of(at);
    }
    parent.is_some_and(|at| test_of(module, at).is_some_and(|t| Cn::Expr(t).stamped() == Some(cur)))
}

/// `rec is None`: identity-tested, not read.
fn is_none_test(module: &Module<'_>, n: NodeIndex, p: NodeIndex) -> bool {
    let Cn::Expr(Expr::Compare(c)) = module.nodes[p as usize] else {
        return false;
    };
    Cn::Expr(&c.left).stamped() == Some(n)
        && c.ops.iter().zip(c.comparators.iter()).all(|(op, cmp)| {
            matches!(op, CmpOp::Is | CmpOp::IsNot) && matches!(cmp, Expr::NoneLiteral(_))
        })
}

/// (constant key `p` reads from `n`, reshapes): `n["k"]`, `n.get("k", ...)`,
/// `n.pop("k", ...)`, `n.setdefault("k", ...)`, `"k" in n`; pop and setdefault
/// also change the key set of the record they read.
fn read_key(module: &Module<'_>, n: NodeIndex, p: NodeIndex) -> (Option<String>, bool) {
    match module.nodes[p as usize] {
        Cn::Expr(Expr::Subscript(s))
            if s.ctx == ExprContext::Load && Cn::Expr(&s.value).stamped() == Some(n) =>
        {
            return (str_const(Some(&s.slice)).map(str::to_string), false);
        }
        Cn::Expr(Expr::Attribute(a)) if READ_METHODS.contains(&a.attr.as_str()) => {
            let call = module.parent_of(p).map(|at| module.nodes[at as usize]);
            if let Some(Cn::Expr(Expr::Call(c))) = call
                && Cn::Expr(&c.func).stamped() == Some(p)
                && !c.arguments.args.is_empty()
            {
                return (
                    str_const(c.arguments.args.first()).map(str::to_string),
                    RESHAPES.contains(&a.attr.as_str()),
                );
            }
        }
        Cn::Expr(Expr::Compare(c))
            if c.ops.len() == 1
                && c.comparators
                    .iter()
                    .any(|e| Cn::Expr(e).stamped() == Some(n)) =>
        {
            if matches!(c.ops[0], CmpOp::In | CmpOp::NotIn) {
                return (str_const(Some(&c.left)).map(str::to_string), false);
            }
        }
        _ => {}
    }
    (None, false)
}

/// How `name` is used in the body, nested defs included. `bound_by` is the
/// one Store allowed; `owned` means the name holds the function's own record:
/// `return name` is its exit, and a reshaping read escapes it.
fn uses(
    facts: &RepoFacts<'_>,
    scope: &Scope,
    name: &str,
    bound_by: Option<NodeIndex>,
    owned: bool,
) -> Uses {
    let module = scope.module(facts);
    let mut u = Uses::default();
    for n in scope.uses_of(facts, name) {
        let Cn::Expr(Expr::Name(nm)) = module.nodes[n as usize] else {
            continue;
        };
        if nm.ctx != ExprContext::Load {
            u.escaped |= Some(n) != bound_by;
            continue;
        }
        let Some(p) = module.parent_of(n) else {
            continue;
        };
        let (key, reshapes) = read_key(module, n, p);
        if let Some(key) = key {
            u.reads.insert(key);
            u.escaped |= owned && reshapes;
            continue;
        }
        match module.nodes[p as usize] {
            Cn::Expr(Expr::Call(c))
                if c.arguments
                    .args
                    .iter()
                    .any(|a| Cn::Expr(a).stamped() == Some(n)) =>
            {
                u.forwards.push((p, n));
            }
            Cn::Keyword(k) if k.arg.is_some() => {
                if let Some(call) = module.parent_of(p) {
                    u.forwards.push((call, n));
                }
            }
            node => {
                let exits = (owned && matches!(node, Cn::Stmt(Stmt::Return(_))))
                    || is_none_test(module, n, p)
                    || truth_tested(module, n);
                u.escaped |= !exits;
            }
        }
    }
    u
}

/// Keys of the record literal `name` is bound once to and then only read (a
/// callee could add keys, so a forward opens it too).
fn bound_record(facts: &RepoFacts<'_>, scope: &Scope, name: &str) -> Option<BTreeSet<String>> {
    let module = scope.module(facts);
    let binds: Vec<&crate::scope::Write> = scope
        .writes(facts)
        .iter()
        .filter(|w| w.root.as_deref() == Some(name) && REBINDS.contains(&w.kind))
        .collect();
    if scope.params(facts).iter().any(|p| p == name) || binds.len() != 1 || binds[0].kind != "name"
    {
        return None;
    }
    let target = binds[0].node;
    let Cn::Stmt(Stmt::Assign(a)) = module.nodes[module.parent_of(target)? as usize] else {
        return None;
    };
    if a.targets.len() != 1 || Cn::Expr(&a.targets[0]).stamped() != Some(target) {
        return None;
    }
    let keys = record_keys(Some(&a.value))?;
    let u = uses(facts, scope, name, Some(target), true);
    (!u.escaped && u.forwards.is_empty()).then_some(keys)
}

// --- summaries ---------------------------------------------------------------

/// The key sets every return path of a closed producer yields; `None` when
/// the body is open.
fn produced_of(facts: &RepoFacts<'_>, scope: &Scope) -> Option<BTreeSet<BTreeSet<String>>> {
    let module = scope.module(facts);
    let q = &scope.qname;
    // a generator yields; a body falling off its end returns None
    if !module
        .nodes(&[Kind::Yield, Kind::YieldFrom], Some(q), false)
        .is_empty()
        || !always_returns(scope.body(facts))
    {
        return None;
    }
    let mut shapes: BTreeSet<BTreeSet<String>> = BTreeSet::new();
    for at in module.nodes(&[Kind::Return], Some(q), false) {
        let Cn::Stmt(Stmt::Return(r)) = module.nodes[at as usize] else {
            continue;
        };
        let keys = match r.value.as_deref() {
            Some(Expr::Name(n)) => bound_record(facts, scope, n.id.as_str()),
            value => record_keys(value),
        }?;
        shapes.insert(keys);
    }
    (!shapes.is_empty()).then_some(shapes)
}

/// Param -> keys the body reads from it by constant; `None` when the param is
/// used any other way (a whole forward included).
fn consumed_of(facts: &RepoFacts<'_>, scope: &Scope) -> IndexMap<String, Option<BTreeSet<String>>> {
    let mut out: IndexMap<String, Option<BTreeSet<String>>> = IndexMap::new();
    for p in scope.params(facts) {
        if RECEIVERS.contains(&p.as_str()) {
            continue;
        }
        let u = uses(facts, scope, p, None, false);
        let closed = !u.escaped && u.forwards.is_empty();
        out.insert(p.clone(), closed.then_some(u.reads));
    }
    out
}

// --- flow edges --------------------------------------------------------------

/// The callee parameter `expr` binds to at this call, `None` past a splat.
fn param_of(
    module: &Module<'_>,
    call: NodeIndex,
    positional: &[&str],
    expr: NodeIndex,
) -> Option<String> {
    let Cn::Expr(Expr::Call(c)) = module.nodes[call as usize] else {
        return None;
    };
    for (i, a) in c.arguments.args.iter().enumerate() {
        if matches!(a, Expr::Starred(_)) {
            return None;
        }
        if Cn::Expr(a).stamped() == Some(expr) {
            return positional.get(i).map(|p| (*p).to_string());
        }
    }
    c.arguments
        .keywords
        .iter()
        .find(|kw| Cn::Expr(&kw.value).stamped() == Some(expr))
        .and_then(|kw| kw.arg.as_ref())
        .map(|a| a.to_string())
}

/// Every param table this pass has read, built once per callee.
type Consumed = HashMap<Qname, IndexMap<String, Option<BTreeSet<String>>>>;

/// The result escapes where this pass can name it and nothing more.
fn open_edge(producer: &Qname, sink: &Qname) -> Edge {
    Edge {
        producer: producer.clone(),
        sink: sink.clone(),
        name: String::new(),
        reads: None,
    }
}

/// The edge one whole-value argument yields: into the callee's own param when
/// the site resolves to a function whose reads are known, else open here.
#[allow(clippy::too_many_arguments)]
fn arg_edge(
    facts: &RepoFacts<'_>,
    provers: &Provers,
    calls: &CallGraph,
    consumed: &mut Consumed,
    producer: &Qname,
    module: &Module<'_>,
    call: NodeIndex,
    expr: NodeIndex,
    here: &Qname,
) -> Edge {
    let target = calls
        .by_node(facts, module.id, call)
        .filter(|site| site.resolution == Resolution::Resolved)
        .and_then(|site| site.target.clone());
    if let Some(g) = target
        && let Some(callee) = facts.symbols.get(&g)
        && FUNCTION_KINDS.contains(&callee.kind)
        && let Some(callee_scope) = provers.scope_of(facts, &g)
    {
        if !consumed.contains_key(&g) {
            consumed.insert(g.clone(), consumed_of(facts, callee_scope));
        }
        let def = callee_scope.func_def(facts);
        let positional: Vec<&str> = fn_pos_args(def).iter().map(|p| p.name.as_str()).collect();
        if let Some(param) = param_of(module, call, &positional, expr)
            && let Some(reads) = consumed[&g].get(&param)
        {
            return Edge {
                producer: producer.clone(),
                sink: g,
                name: param,
                reads: reads.clone(),
            };
        }
    }
    open_edge(producer, here)
}

/// The edges one RESOLVED site of a closed producer yields: the result bound
/// to a local (binder), whose whole forwards yield their own edges into the
/// callee's param, passed straight to a param, or escaping unnamed (open).
fn result_edges(
    facts: &RepoFacts<'_>,
    provers: &Provers,
    calls: &CallGraph,
    consumed: &mut Consumed,
    site: &CallSite,
    scope: &Scope,
    out: &mut Vec<Edge>,
) {
    let p = site
        .target
        .clone()
        .expect("a resolved site names its target");
    let here = scope.qname.clone();
    let module = scope.module(facts);
    let Some(parent) = module.parent_of(site.node) else {
        out.push(open_edge(&p, &here));
        return;
    };
    match module.nodes[parent as usize] {
        Cn::Stmt(Stmt::Assign(a))
            if a.targets.len() == 1 && matches!(a.targets[0], Expr::Name(_)) =>
        {
            let Expr::Name(x) = &a.targets[0] else {
                unreachable!("the guard matched a Name target")
            };
            let name = x.id.to_string();
            let u = uses(
                facts,
                scope,
                &name,
                Cn::Expr(&a.targets[0]).stamped(),
                false,
            );
            if u.escaped || scope.params(facts).contains(&name) {
                out.push(Edge {
                    producer: p,
                    sink: here,
                    name,
                    reads: None,
                });
                return;
            }
            out.push(Edge {
                producer: p.clone(),
                sink: here.clone(),
                name,
                reads: Some(u.reads),
            });
            for (call, n) in u.forwards {
                let edge = arg_edge(facts, provers, calls, consumed, &p, module, call, n, &here);
                out.push(edge);
            }
        }
        Cn::Expr(Expr::Call(c))
            if c.arguments
                .args
                .iter()
                .any(|a| Cn::Expr(a).stamped() == Some(site.node)) =>
        {
            let edge = arg_edge(
                facts, provers, calls, consumed, &p, module, parent, site.node, &here,
            );
            out.push(edge);
        }
        Cn::Keyword(k) if k.arg.is_some() => {
            let call = module.parent_of(parent).expect("a keyword sits in a call");
            let edge = arg_edge(
                facts, provers, calls, consumed, &p, module, call, site.node, &here,
            );
            out.push(edge);
        }
        _ => out.push(open_edge(&p, &here)),
    }
}

/// Closed producers and where their results flow (#57).
pub fn build_records(facts: &RepoFacts<'_>, provers: &Provers) -> Records {
    let calls = provers.calls(facts);
    let mut produced: IndexMap<Qname, Vec<BTreeSet<String>>> = IndexMap::new();
    for (q, sym) in &facts.symbols {
        if FUNCTION_KINDS.contains(&sym.kind)
            && let Some(scope) = provers.scope_of(facts, q)
            && let Some(shapes) = produced_of(facts, scope)
        {
            produced.insert(q.clone(), shapes.into_iter().collect());
        }
    }
    let mut consumed: Consumed = HashMap::new();
    let mut edges: Vec<Edge> = Vec::new();
    for q in produced.keys() {
        for at in calls.calls_to.get(q).map_or(&[][..], |v| v) {
            let site = &calls.sites[*at as usize];
            let scope = facts
                .symbols
                .get(&site.enclosing)
                .filter(|sym| FUNCTION_KINDS.contains(&sym.kind))
                .and_then(|_| provers.scope_of(facts, &site.enclosing));
            match scope {
                // module or class body
                None => edges.push(Edge {
                    producer: q.clone(),
                    sink: site.enclosing.clone(),
                    name: String::new(),
                    reads: None,
                }),
                Some(scope) => result_edges(
                    facts,
                    provers,
                    calls,
                    &mut consumed,
                    site,
                    scope,
                    &mut edges,
                ),
            }
        }
    }
    Records { produced, edges }
}

/// `layer_records`.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let found = provers.records(facts);
    Some(json!({
        "produced": Value::Object(
            found
                .produced
                .iter()
                .map(|(q, shapes)| {
                    let mut rows: Vec<Vec<&str>> = shapes
                        .iter()
                        .map(|shape| shape.iter().map(|k| &**k).collect())
                        .collect();
                    rows.sort();
                    (q.to_string(), json!(rows))
                })
                .collect::<Map<String, Value>>(),
        ),
        "edges": found
            .edges
            .iter()
            .map(|e| {
                json!({
                    "producer": &*e.producer,
                    "sink": &*e.sink,
                    "name": e.name,
                    "reads": e.reads.as_ref().map(|r| r.iter().map(|k| &**k).collect::<Vec<&str>>()),
                })
            })
            .collect::<Vec<Value>>(),
    }))
}
