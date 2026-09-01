//! Port of `provers/hotness.py` (codemap 3.3): `[tool.sightline] hot-roots`
//! plus #59 cost-declaring docstrings, propagated over the SCC condensation
//! of the call graph's callee edges (`callgraph::callee_of`). The
//! amplification factor is the loop depth crossed along the path, a recursive
//! cycle's own loops counted once, and a memo guard is a barrier
//! (`memo_guards`); family P (#41) emits only inside the hot set.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use rayon::prelude::*;
use regex::Regex;
use ruff_python_ast::{Expr, ExprContext, Stmt};
use serde_json::{Map, Value, json};

use sightline_core::findings::Qname;
use sightline_core::graph::tarjan_scc;
use sightline_core::pytext::fnmatchcase;
use sightline_py_facts::astutil::walk;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{FUNCTION_KINDS, NodeIndex, RepoFacts, Resolution, is_test_path};
use sightline_py_facts::module::Module;

use crate::Provers;
use crate::callgraph::{CallGraph, callee_of};
use crate::comments::{body_of, docstring};

/// The #59 docstring is the one home for "this is heavy": a declared cost
/// seeds hotness (secondary to explicit hot-roots config).
pub static COST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(hot (path|loop)|per[- ](frame|tick|request|row|token|batch)|O\([^)\s]{1,20}\))",
    )
    .expect("COST_RE compiles")
});

const LOOP_NODES: [Kind; 7] = [
    Kind::For,
    Kind::AsyncFor,
    Kind::While,
    Kind::ListComp,
    Kind::SetComp,
    Kind::DictComp,
    Kind::GeneratorExp,
];
const SCOPE_NODES: [Kind; 3] = [Kind::FunctionDef, Kind::AsyncFunctionDef, Kind::Lambda];

#[derive(Default)]
pub struct HotSet {
    /// qname -> loop depth crossed from a root
    pub amplification: IndexMap<Qname, u32>,
    pub roots: Vec<Qname>,
    /// configured patterns matching no symbol
    pub missing_roots: Vec<String>,
}

/// The position under `loop_node` that runs once per entry, not per
/// iteration: a `for`'s iterable, and a comprehension's first `iter` (Python
/// evaluates it in the enclosing scope, before the loop).
fn runs_once(
    module: &Module<'_>,
    loop_node: NodeIndex,
    child: NodeIndex,
    grandchild: Option<NodeIndex>,
) -> bool {
    let generators = match module.nodes[loop_node as usize] {
        Cn::Stmt(Stmt::For(f)) => return Cn::Expr(&f.iter).stamped() == Some(child),
        Cn::Stmt(Stmt::While(_)) => return false,
        Cn::Expr(Expr::ListComp(c)) => &c.generators,
        Cn::Expr(Expr::SetComp(c)) => &c.generators,
        Cn::Expr(Expr::DictComp(c)) => &c.generators,
        Cn::Expr(Expr::Generator(g)) => &g.generators,
        Cn::CallGen(g, _) => &g.generators,
        _ => return false,
    };
    let Some(first) = generators.first() else {
        return false;
    };
    Cn::Comp(first).stamped() == Some(child) && Cn::Expr(&first.iter).stamped() == grandchild
}

/// Enclosing loop levels of a node within its own function scope.
pub fn loop_depth(module: &Module<'_>, node: NodeIndex) -> u32 {
    let mut depth = 0;
    let (mut grandchild, mut child) = (None, node);
    let mut cur = module.parent_of(node);
    while let Some(at) = cur {
        let kind = module.nodes[at as usize].kind();
        if SCOPE_NODES.contains(&kind) {
            break;
        }
        if LOOP_NODES.contains(&kind) && !runs_once(module, at, child, grandchild) {
            depth += 1;
        }
        grandchild = Some(child);
        child = at;
        cur = module.parent_of(at);
    }
    depth
}

/// Writes through a reference that fill a store the guard read.
const STORE_FILLERS: [&str; 5] = ["update", "append", "add", "extend", "setdefault"];

/// The store an expression reads, directly or through a local the body bound
/// to a read of it (`hit = CACHE.get(key)`).
fn reads_a_store<'a>(
    expr: &Expr,
    stores: &'a BTreeSet<String>,
    aliases: &'a HashMap<String, String>,
) -> Option<&'a str> {
    for node in walk(Cn::Expr(expr)) {
        if let Cn::Expr(Expr::Name(n)) = node {
            let id = n.id.as_str();
            if let Some(hit) = stores.get(id) {
                return Some(hit);
            }
            if let Some(hit) = aliases.get(id) {
                return Some(hit);
            }
        }
    }
    None
}

/// What an expression writes: the name it stores, the container it stores
/// into (`s[k] = v`), or the receiver of a filling method.
fn written(node: &Expr) -> Option<&Expr> {
    match node {
        Expr::Name(n) if n.ctx == ExprContext::Store => Some(node),
        Expr::Subscript(s) if s.ctx == ExprContext::Store => Some(&s.value),
        Expr::Call(c) => match &*c.func {
            Expr::Attribute(a) if STORE_FILLERS.contains(&a.attr.as_str()) => Some(&a.value),
            _ => None,
        },
        _ => None,
    }
}

/// The body writes the store below `after`.
fn fills(module: &Module<'_>, func: NodeIndex, store: &str, after: u32) -> bool {
    walk(module.nodes[func as usize]).any(|node| {
        let Cn::Expr(expr) = node else { return false };
        let line = node.stamped().map_or(0, |i| module.line_of(i));
        line > after && matches!(written(expr), Some(Expr::Name(w)) if w.id.as_str() == store)
    })
}

/// The line a memo guard closes at: an early return on a module-level store
/// the body fills below it (`if _loaded: return ...` after a `global` flag,
/// `hit = CACHE.get(k)` / `if hit is not None: return hit`). What the body
/// computes below runs once per key however hot the callers are, so those
/// call sites carry no amplification.
fn memo_guard(module: &Module<'_>, func: NodeIndex, stores: &BTreeSet<String>) -> Option<u32> {
    let Cn::Stmt(Stmt::FunctionDef(f)) = module.nodes[func as usize] else {
        return None;
    };
    let mut aliases: HashMap<String, String> = HashMap::new();
    for st in &f.body {
        if let Stmt::Assign(a) = st
            && let Some(store) = reads_a_store(&a.value, stores, &aliases).map(str::to_string)
        {
            for target in &a.targets {
                if let Expr::Name(n) = target {
                    aliases.insert(n.id.to_string(), store.clone());
                }
            }
        }
        if let Stmt::If(iff) = st
            && walk(Cn::Stmt(st)).any(|n| n.kind() == Kind::Return)
        {
            let store = reads_a_store(&iff.test, stores, &aliases).map(str::to_string);
            let end = Cn::Stmt(st).stamped().map_or(0, |i| module.end_line_of(i));
            if let Some(store) = store
                && fills(module, func, &store, end)
            {
                return Some(end);
            }
        }
    }
    None
}

/// qname -> the line its memo guard closes at, for every function with one.
fn memo_guards(facts: &RepoFacts<'_>) -> HashMap<Qname, u32> {
    let modules: Vec<&Module<'_>> = facts.modules.values().collect();
    let per_module: Vec<Vec<(Qname, u32)>> = modules
        .par_iter()
        .map(|module| {
            // names bound at the module's top level
            let stores: BTreeSet<String> = module
                .nodes(&[Kind::Assign, Kind::AnnAssign], Some(&module.qname), false)
                .into_iter()
                .flat_map(|at| match module.nodes[at as usize] {
                    Cn::Stmt(Stmt::Assign(a)) => a.targets.iter().collect::<Vec<&Expr>>(),
                    Cn::Stmt(Stmt::AnnAssign(a)) => vec![&*a.target],
                    _ => Vec::new(),
                })
                .filter_map(|t| match t {
                    Expr::Name(n) => Some(n.id.to_string()),
                    _ => None,
                })
                .collect();
            if stores.is_empty() {
                return Vec::new();
            }
            facts
                .symbols_by_module
                .get(&module.qname)
                .map_or(&[][..], |v| v)
                .iter()
                .filter_map(|id| facts.symbols.get_index(*id as usize).map(|(_, s)| s))
                .filter(|sym| FUNCTION_KINDS.contains(&sym.kind))
                .filter_map(|sym| {
                    memo_guard(module, sym.node, &stores).map(|line| (sym.qname.clone(), line))
                })
                .collect()
        })
        .collect();
    per_module.into_iter().flatten().collect()
}

/// (seed qnames, unmatched patterns): each `hot-roots` entry is an fnmatch
/// pattern over qnames (config order, matches sorted), then #59
/// cost-declaring docstrings, prod only, since a test's cost docstring
/// narrates its subject and test glue stays cold.
pub fn roots_of(facts: &RepoFacts<'_>) -> (IndexSet<Qname>, Vec<String>) {
    let mut names: Vec<&Qname> = facts.symbols.keys().collect();
    names.sort();
    let mut roots: IndexSet<Qname> = IndexSet::new();
    let mut missing: Vec<String> = Vec::new();
    for pattern in &facts.config.hot_roots {
        let matched: Vec<&Qname> = names
            .iter()
            .copied()
            .filter(|q| fnmatchcase(q, pattern))
            .collect();
        if matched.is_empty() {
            missing.push(pattern.clone());
        }
        roots.extend(matched.into_iter().cloned());
    }
    for (qname, sym) in &facts.symbols {
        if !FUNCTION_KINDS.contains(&sym.kind)
            || facts
                .rel_of(&sym.module)
                .is_some_and(|rel| is_test_path(rel))
        {
            continue;
        }
        let module = facts.modules.get(&sym.module).expect("the symbol's module");
        if body_of(module, sym.node)
            .and_then(docstring)
            .is_some_and(|doc| COST_RE.is_match(&doc))
        {
            roots.insert(qname.clone());
        }
    }
    (roots, missing)
}

/// Each caller's outgoing callee edges, with the loop depth each crosses.
type Edges = IndexMap<Qname, Vec<(Qname, u32)>>;
/// (caller, callee, depth)
type Edge = (Qname, Qname, u32);

/// SCCs callers-first (tarjan emits callees first), each with its edges split
/// into (outgoing, inside the component).
fn condensation(edges: &Edges) -> Vec<(Vec<String>, Vec<Edge>, Vec<Edge>)> {
    let mut graph: BTreeMap<String, BTreeSet<String>> = edges
        .values()
        .flatten()
        .map(|(c, _)| (c.to_string(), BTreeSet::new()))
        .collect();
    for (q, es) in edges {
        graph.insert(
            q.to_string(),
            es.iter().map(|(c, _)| c.to_string()).collect(),
        );
    }
    let (comps, comp_of) = tarjan_scc(&graph);
    let mut split: Vec<(Vec<Edge>, Vec<Edge>)> =
        (0..comps.len()).map(|_| (Vec::new(), Vec::new())).collect();
    for (q, es) in edges {
        for (c, d) in es {
            let (outgoing, inside) = &mut split[comp_of[&**q]];
            let row = (q.clone(), c.clone(), *d);
            if comp_of[&**c] == comp_of[&**q] {
                inside.push(row);
            } else {
                outgoing.push(row);
            }
        }
    }
    comps
        .into_iter()
        .zip(split)
        .map(|(comp, (outgoing, inside))| (comp, outgoing, inside))
        .rev()
        .collect()
}

/// Max amplification per node. Inside a cycle every member is as hot as the
/// hottest entry plus the cycle's own loop edges crossed once: a recursive
/// walk in a loop is linear work, not a nested loop, and a cap would pump the
/// whole downstream graph to the ceiling instead.
fn propagate(edges: &Edges, roots: &IndexSet<Qname>) -> IndexMap<Qname, u32> {
    let mut amp: IndexMap<Qname, u32> = roots.iter().map(|q| (q.clone(), 0)).collect();
    for (comp, outgoing, inside) in condensation(edges) {
        let Some(entry) = comp.iter().filter_map(|q| amp.get(&**q).copied()).max() else {
            continue; // unreachable from every root
        };
        for q in &comp {
            amp.insert(Qname::from(q.as_str()), entry);
        }
        for (_q, c, d) in inside {
            let raised = amp[&*c].max(entry + d);
            amp.insert(c, raised);
        }
        for (q, c, d) in outgoing {
            let reached = amp[&*q] + d;
            let raised = amp.get(&*c).map_or(reached, |had| (*had).max(reached));
            amp.insert(c, raised);
        }
    }
    amp
}

/// One pass over every call site of the graph, then `propagate` over the
/// condensed callee graph; a call below a memo guard (`memo_guards`) is no
/// edge, since what it computes runs once per key. Where the graph let
/// by-name guesses stand for lack of an oracle (`guesses_stand`), they are no
/// edge here either: a degraded run reports a subset of its oracle twin,
/// never a #41 the oracle would not.
pub fn hot_reachable(facts: &RepoFacts<'_>, calls: &CallGraph, guesses_stand: bool) -> HotSet {
    let (roots, missing_roots) = roots_of(facts);
    let guards = memo_guards(facts);
    let mut edges: Edges = IndexMap::new();
    for (at, call) in calls.sites.iter().enumerate() {
        if guesses_stand && facts.call_sites[at].resolution == Resolution::ByName {
            continue;
        }
        // below a memo guard: once per key, not once per call
        if guards
            .get(&call.enclosing)
            .is_some_and(|g| call.lineno > *g)
        {
            continue;
        }
        if let Some(callee) = callee_of(facts, call) {
            let module = facts.modules.get(&call.module).expect("the site's module");
            let depth = loop_depth(module, call.node);
            edges
                .entry(call.enclosing.clone())
                .or_default()
                .push((callee, depth));
        }
    }
    let amplification = propagate(&edges, &roots);
    let mut sorted: Vec<Qname> = roots.into_iter().collect();
    sorted.sort();
    HotSet {
        amplification,
        roots: sorted,
        missing_roots,
    }
}

/// `layer_hot`.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let hot = provers.hot(facts);
    Some(json!({
        "amplification": Value::Object(
            hot.amplification
                .iter()
                .map(|(q, d)| (q.to_string(), Value::from(*d)))
                .collect::<Map<String, Value>>(),
        ),
        "roots": hot.roots.iter().map(|q| &**q).collect::<Vec<&str>>(),
        "missing_roots": hot.missing_roots.clone(),
    }))
}
