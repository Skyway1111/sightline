//! Module coupling: the import graph (top-level and any-scope edges, reach,
//! importers, readers). One home for "who reaches whom" - #9, #27 and #35
//! read it. What an import *runs* is `import_effects.rs`'s question.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use indexmap::{IndexMap, IndexSet};
use ruff_python_ast::{Expr, Stmt};
use serde_json::{Map, Value, json};

use sightline_core::findings::Qname;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{NodeIndex, RepoFacts, is_test_path};
use sightline_py_facts::module::Module;

use crate::Provers;

/// The constant module name an `import_module("pkg.x")` / `__import__("pkg.x")`
/// call loads; `None` for a computed or relative one.
fn dynamic_target<'a>(module: &Module<'_>, call: &'a ruff_python_ast::ExprCall) -> Option<&'a str> {
    let first = call.arguments.args.first()?;
    let Expr::StringLiteral(s) = first else {
        return None;
    };
    let value = s.value.to_str();
    if value.starts_with('.') {
        return None;
    }
    let dynamic = matches!(&*call.func, Expr::Name(n) if n.id.as_str() == "__import__")
        || module.dotted_name(&call.func).as_deref() == Some("importlib.import_module");
    dynamic.then_some(value)
}

/// Dotted targets of one import statement; `from base import name` yields
/// `base.name` when that is a known module, else `base`; a constant-string
/// `import_module` / `__import__` call names its target.
pub fn import_targets(facts: &RepoFacts<'_>, module: &Module<'_>, node: NodeIndex) -> Vec<String> {
    match module.nodes[node as usize] {
        Cn::Stmt(Stmt::Import(n)) => n.names.iter().map(|a| a.name.to_string()).collect(),
        Cn::Stmt(Stmt::ImportFrom(n)) => {
            let base = module.rel_import_base(n.level, n.module.as_ref().map(|m| m.as_str()));
            if base.is_empty() {
                return Vec::new();
            }
            n.names
                .iter()
                .map(|a| {
                    let cand = format!("{base}.{}", a.name);
                    if a.name.as_str() != "*" && facts.modules.contains_key(cand.as_str()) {
                        cand
                    } else {
                        base.clone()
                    }
                })
                .collect()
        }
        Cn::Expr(Expr::Call(c)) => dynamic_target(module, c)
            .map(|t| vec![t.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// (module, node, target dotted name) for every import statement and
/// constant-string dynamic import, nested included.
pub fn module_imports(facts: &RepoFacts<'_>) -> Vec<(Qname, NodeIndex, String)> {
    let kinds = [Kind::Import, Kind::ImportFrom, Kind::Call];
    let mut out = Vec::new();
    for module in facts.modules.values() {
        for node in module.nodes(&kinds, None, false) {
            for target in import_targets(facts, module, node) {
                out.push((module.qname.clone(), node, target));
            }
        }
    }
    out
}

/// Longest internal-module prefix of a dotted import target.
pub fn internal_module<'f>(facts: &'f RepoFacts<'_>, target: &str) -> Option<&'f Qname> {
    let parts: Vec<&str> = target.split('.').collect();
    for i in (1..=parts.len()).rev() {
        let q = parts[..i].join(".");
        if let Some((key, _)) = facts.modules.get_key_value(q.as_str()) {
            return Some(key);
        }
    }
    None
}

/// The `if` test of a node that is one, an `elif` clause included.
fn if_test<'a>(module: &Module<'a>, node: NodeIndex) -> Option<&'a Expr> {
    match module.nodes[node as usize] {
        Cn::Stmt(Stmt::If(n)) => Some(&n.test),
        Cn::Elif(rest) => rest[0].test.as_ref(),
        _ => None,
    }
}

/// Is `node` under an `if` whose test `test` accepts, at any depth?
fn guarded(module: &Module<'_>, node: NodeIndex, test: impl Fn(&Expr) -> bool) -> bool {
    let mut cur = module.parent_of(node);
    while let Some(at) = cur {
        if if_test(module, at).is_some_and(&test) {
            return true;
        }
        cur = module.parent_of(at);
    }
    false
}

pub fn in_type_checking(module: &Module<'_>, node: NodeIndex) -> bool {
    guarded(module, node, |t| match t {
        Expr::Name(n) => n.id.as_str() == "TYPE_CHECKING",
        Expr::Attribute(a) => a.attr.as_str() == "TYPE_CHECKING",
        _ => false,
    })
}

pub fn under_main_guard(module: &Module<'_>, node: NodeIndex) -> bool {
    guarded(module, node, |t| match t {
        Expr::Compare(c) => std::iter::once(&*c.left)
            .chain(c.comparators.iter())
            .any(|x| matches!(x, Expr::Name(n) if n.id.as_str() == "__name__")),
        _ => false,
    })
}

const IMPORT_ERRORS: [&str; 2] = ["ImportError", "ModuleNotFoundError"];

/// Is `node` in the body of a `try` an import error handles? The import is
/// then the feature test itself, so nothing loads its name and taking it takes
/// the probe. An import the handler *makes* is a fallback binding, not a test.
pub fn probes_availability(module: &Module<'_>, node: NodeIndex) -> bool {
    let mut prev = node;
    let mut cur = module.parent_of(node);
    while let Some(at) = cur {
        if let Cn::Stmt(Stmt::Try(t)) = module.nodes[at as usize] {
            let in_body = t.body.iter().any(|st| Cn::Stmt(st).stamped() == Some(prev));
            let handled = t.handlers.iter().any(|h| {
                let ruff_python_ast::ExceptHandler::ExceptHandler(h) = h;
                let types: Vec<Option<&Expr>> = match h.type_.as_deref() {
                    Some(Expr::Tuple(tuple)) => tuple.elts.iter().map(Some).collect(),
                    other => vec![other],
                };
                types.into_iter().any(|t| match t {
                    Some(Expr::Name(n)) => IMPORT_ERRORS.contains(&n.id.as_str()),
                    Some(Expr::Attribute(a)) => IMPORT_ERRORS.contains(&a.attr.as_str()),
                    _ => false,
                })
            });
            if in_body && handled {
                return true;
            }
        }
        prev = at;
        cur = module.parent_of(at);
    }
    false
}

/// Internal import edges, self-edges dropped: `top` is what a reader pays at
/// import time (module scope, outside TYPE_CHECKING), `full` adds deferred
/// (function-scope, guarded, TYPE_CHECKING) edges; `typed` are the edges every
/// site of which sits under TYPE_CHECKING - the checker's, never the
/// interpreter's.
pub struct ImportGraph {
    pub top: IndexMap<Qname, IndexSet<Qname>>,
    pub full: IndexMap<Qname, IndexSet<Qname>>,
    /// first import node per edge, in the source module
    pub first: HashMap<(Qname, Qname), NodeIndex>,
    pub typed: IndexMap<Qname, IndexSet<Qname>>,
    reach: Mutex<HashMap<(Qname, bool), HashSet<Qname>>>,
}

impl ImportGraph {
    /// Every module `src` reaches over any-scope (or top-level) edges.
    pub fn reach(&self, src: &str, top: bool) -> HashSet<Qname> {
        let key = (Qname::from(src), top);
        if let Some(found) = self
            .reach
            .lock()
            .expect("no panic holds this lock")
            .get(&key)
        {
            return found.clone();
        }
        let graph = if top { &self.top } else { &self.full };
        let mut seen: HashSet<Qname> = HashSet::new();
        let mut stack: Vec<Qname> = vec![key.0.clone()];
        while let Some(at) = stack.pop() {
            for next in graph.get(&at).into_iter().flatten() {
                if seen.insert(next.clone()) {
                    stack.push(next.clone());
                }
            }
        }
        self.reach
            .lock()
            .expect("no panic holds this lock")
            .insert(key, seen.clone());
        seen
    }
}

/// `dst` and everything importing it loads at import time.
pub fn loads(graph: &ImportGraph, dst: &str) -> HashSet<Qname> {
    let mut out = graph.reach(dst, true);
    out.insert(Qname::from(dst));
    out
}

pub fn import_graph(facts: &RepoFacts<'_>) -> ImportGraph {
    let empty = || -> IndexMap<Qname, IndexSet<Qname>> {
        facts
            .modules
            .keys()
            .map(|q| (q.clone(), IndexSet::new()))
            .collect()
    };
    let mut top = empty();
    let mut full = empty();
    let mut runtime: HashSet<(Qname, Qname)> = HashSet::new();
    let mut first: HashMap<(Qname, Qname), NodeIndex> = HashMap::new();
    for (src, node, target) in module_imports(facts) {
        let Some(dst) = internal_module(facts, &target) else {
            continue;
        };
        if *dst == src {
            continue;
        }
        let dst = dst.clone();
        let module = &facts.modules[&src];
        full[&src].insert(dst.clone());
        first.entry((src.clone(), dst.clone())).or_insert(node);
        if in_type_checking(module, node) {
            continue;
        }
        runtime.insert((src.clone(), dst.clone()));
        if facts.enclosing(module, node) == src {
            top[&src].insert(dst);
        }
    }
    let typed = full
        .iter()
        .map(|(q, dsts)| {
            let kept: IndexSet<Qname> = dsts
                .iter()
                .filter(|d| !runtime.contains(&(q.clone(), (*d).clone())))
                .cloned()
                .collect();
            (q.clone(), kept)
        })
        .collect();
    ImportGraph {
        top,
        full,
        first,
        typed,
        reach: Mutex::new(HashMap::new()),
    }
}

/// module qname -> modules importing it (any scope).
pub fn importers(graph: &ImportGraph) -> IndexMap<Qname, IndexSet<Qname>> {
    let mut out: IndexMap<Qname, IndexSet<Qname>> = IndexMap::new();
    for (src, dsts) in &graph.full {
        for dst in dsts {
            out.entry(dst.clone()).or_default().insert(src.clone());
        }
    }
    out
}

/// A directory below the root that holds one of these ships on its own.
const MANIFESTS: [&str; 4] = ["pyproject.toml", "setup.py", "setup.cfg", "SKILL.md"];

/// module qname -> the home a second copy of its code could move into. A
/// directory that ships on its own is one home for all it holds; otherwise a
/// module a package holds (or one at the root) can import a new sibling
/// anywhere, so the repo is its home; a script outside every package can only
/// import out of a directory the repo already imports out of; a standalone
/// script nothing imports is its own home. Copies with different homes have no
/// shared place to live (#11 / #38).
pub fn shared_homes(facts: &RepoFacts<'_>) -> IndexMap<Qname, String> {
    let rels: HashSet<&str> = facts.modules.values().map(|m| &*m.rel).collect();
    let mut bundles: Vec<&str> = facts
        .all_files
        .iter()
        .filter(|rel| rel.contains('/') && MANIFESTS.contains(&rsplit_tail(rel)))
        .map(|rel| rsplit_head(rel))
        .collect::<HashSet<&str>>()
        .into_iter()
        .collect();
    // longest first: the innermost manifest wins
    bundles.sort_by_key(|b| (std::cmp::Reverse(b.len()), *b));
    let graph = import_graph(facts);
    let imported: HashSet<&Qname> = graph.full.values().flatten().collect();
    let reachable: HashSet<&str> = facts
        .modules
        .values()
        .filter(|m| imported.contains(&m.qname))
        .map(|m| rsplit_head(&m.rel))
        .collect();
    let mut homes: IndexMap<Qname, String> = IndexMap::new();
    for module in facts.modules.values() {
        let folder = rsplit_head(&module.rel);
        let bundle = bundles
            .iter()
            .find(|b| module.rel.starts_with(&format!("{b}/")));
        let home = match bundle {
            Some(b) => (*b).to_string(),
            None if folder.is_empty()
                || rels.contains(format!("{folder}/__init__.py").as_str()) =>
            {
                String::new()
            }
            None if reachable.contains(folder) => folder.to_string(),
            None => module.rel.to_string(),
        };
        homes.insert(module.qname.clone(), home);
    }
    homes
}

fn rsplit_head(rel: &str) -> &str {
    sightline_core::pytext::rpartition(rel, "/").0
}

fn rsplit_tail(rel: &str) -> &str {
    sightline_core::pytext::rpartition(rel, "/").2
}

/// module qname -> the prod modules that read it: an importer at depth >= 2
/// from every root (itself imported by an imported prod module - a launcher
/// drives what it loads, only what that loads is read) that references the
/// module (a load, not the import statement) outside a `__name__` guard or a
/// `main`.
pub fn readers(facts: &RepoFacts<'_>, graph: &ImportGraph) -> IndexMap<Qname, IndexSet<Qname>> {
    let prod: HashSet<&Qname> = facts
        .modules
        .iter()
        .filter(|(_, m)| !is_test_path(&m.rel))
        .map(|(q, _)| q)
        .collect();
    let imported: HashSet<&Qname> = graph
        .full
        .iter()
        .filter(|(s, _)| prod.contains(s))
        .flat_map(|(_, dsts)| dsts.iter())
        .collect();
    let read: HashSet<&Qname> = imported
        .iter()
        .filter(|d| prod.contains(**d))
        .filter_map(|d| graph.full.get(*d))
        .flatten()
        .collect();
    let mut out: IndexMap<Qname, IndexSet<Qname>> = IndexMap::new();
    for r in &facts.refs {
        let src = &r.module;
        if !prod.contains(src) || !read.contains(src) {
            continue;
        }
        let module = &facts.modules[src];
        if module.nodes[r.node as usize].kind() == Kind::Alias {
            continue;
        }
        let dst = match facts.symbols.get(&r.target) {
            Some(sym) => Some(&sym.module),
            None if facts.modules.contains_key(&r.target) => Some(&r.target),
            None => None,
        };
        let Some(dst) = dst else { continue };
        if dst == src || out.get(dst).is_some_and(|s| s.contains(src)) {
            continue;
        }
        let scope = facts.enclosing(module, r.node);
        let main = format!("{src}.main");
        if *scope == *main || scope.starts_with(&format!("{main}.")) {
            continue;
        }
        if under_main_guard(module, r.node) {
            continue;
        }
        out.entry(dst.clone()).or_default().insert(src.clone());
    }
    out
}

/// `layer_imports`.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let graph = provers.import_graph(facts);
    let mut subsets: Vec<Vec<&str>> = provers
        .shipped_subsets(facts)
        .iter()
        .map(|s| s.iter().map(|q| &**q).collect())
        .collect();
    subsets.sort();
    Some(json!({
        "top": edges(&graph.top),
        "full": edges(&graph.full),
        "typed": edges(&graph.typed),
        "import_effects": provers.import_effects(facts).iter().map(|q| &**q).collect::<Vec<&str>>(),
        "shipped_subsets": subsets,
        "shared_homes": Value::Object(
            shared_homes(facts)
                .into_iter()
                .map(|(q, home)| (q.to_string(), Value::from(home)))
                .collect::<Map<String, Value>>(),
        ),
        "readers": edges(&readers(facts, graph)),
    }))
}

fn edges(table: &IndexMap<Qname, IndexSet<Qname>>) -> Value {
    Value::Object(
        table
            .iter()
            .map(|(q, dsts)| {
                let mut sorted: Vec<&str> = dsts.iter().map(|d| &**d).collect();
                sorted.sort_unstable();
                (q.to_string(), Value::from(sorted))
            })
            .collect::<Map<String, Value>>(),
    )
}
