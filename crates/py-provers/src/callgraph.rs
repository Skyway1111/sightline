//! Port of `provers/callgraph.py` and `callers.py` (codemap 3.3): the call
//! graph consumers read, and `callee_of` - the one answer to "what body does
//! this site run".
//!
//! Facts' CHA verdicts, re-judged by the oracle's callee edges wherever CHA
//! had no typed evidence (BY_NAME, UNRESOLVED, AMBIGUOUS); a by-name guess the
//! oracle does not confirm goes unknown rather than standing as a false edge,
//! and stands only when there is no oracle. A callee the oracle places outside
//! the repo (`f.write` on a file object, `Path(p)`) is EXTERNAL - no repo body
//! runs - with its dotted homes as the site's `candidates`
//! (`sqlite3.Connection.execute`: effects' io catalog reads them). Override
//! candidates still come from the class table (a method with subclass
//! overrides stays AMBIGUOUS). Facts never see the oracle: an overlay of
//! copies.

use std::collections::{BTreeSet, HashMap};

use indexmap::{IndexMap, IndexSet};
use ruff_python_ast::{Expr, ExprContext, Stmt};
use serde_json::{Value, json};

use sightline_core::findings::{Qname, Rel};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{
    CallSite, CallSiteId, FUNCTION_KINDS, ModuleId, NodeIndex, RepoFacts, Resolution, Step, Symbol,
    class_walk, is_test_path,
};

use crate::Provers;

mod judged;
use judged::{name_cha_stands, oracle_judged, unconfirmed};

/// CHA verdicts resting on a name match rather than the receiver's type.
const UNTYPED: [Resolution; 3] = [
    Resolution::ByName,
    Resolution::Unresolved,
    Resolution::Ambiguous,
];

/// A call's checker-resolved callee definitions (the oracle's `call_edges`):
/// the span in CPython `ast` terms, targets as `(rel, definition line)`.
/// `external` (no targets): every definition lies outside the root, and the
/// definitions' dotted homes are what effects' io catalog reads. Phase 4 fills
/// these; phase 3 always passes `None`.
#[derive(Debug, Clone)]
pub struct CallEdge {
    pub rel: Rel,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub targets: Vec<(Rel, u32)>,
    pub external: Vec<Qname>,
}

#[derive(Clone, Default)]
pub struct CallGraph {
    /// one entry per `facts.call_sites` entry, in that order
    pub sites: Vec<CallSite>,
    pub calls_to: IndexMap<Qname, Vec<CallSiteId>>,
    /// sites the oracle resolved past CHA (provenance receipt)
    pub upgraded: u32,
}

impl CallGraph {
    /// The Call node's verdict, facts' index re-judged. `sites` runs parallel
    /// to `facts.call_sites`, so facts' own index is the lookup.
    pub fn by_node(
        &self,
        facts: &RepoFacts<'_>,
        module: ModuleId,
        node: NodeIndex,
    ) -> Option<&CallSite> {
        let at = facts.call_index.get(&(module, node))?;
        self.sites.get(*at as usize)
    }

    pub fn callers(&self, qname: &str) -> impl Iterator<Item = &CallSite> {
        self.calls_to
            .get(qname)
            .map_or(&[][..], |v| v)
            .iter()
            .map(|i| &self.sites[*i as usize])
    }
}

/// facts' CHA resolution; the oracle re-judges it from phase 4.
pub fn build_call_graph(facts: &RepoFacts<'_>) -> CallGraph {
    judged_call_graph(facts, None)
}

/// `build_call_graph(facts, oracle)`: with the oracle's edges a by-name guess
/// stands only where an edge confirms it; without them the guess is the only
/// verdict there is.
pub fn judged_call_graph(facts: &RepoFacts<'_>, edges: Option<&[CallEdge]>) -> CallGraph {
    let (judged, sites): (usize, Vec<CallSite>) = match edges {
        None => (0, facts.call_sites.iter().map(name_cha_stands).collect()),
        Some(edges) => {
            let judged = oracle_judged(facts, edges);
            let sites = facts
                .call_sites
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    judged
                        .get(&(i as CallSiteId))
                        .cloned()
                        .unwrap_or_else(|| unconfirmed(c))
                })
                .collect();
            (judged.len(), sites)
        }
    };
    let mut calls_to: IndexMap<Qname, Vec<CallSiteId>> = IndexMap::new();
    for (i, c) in sites.iter().enumerate() {
        if c.resolution == Resolution::Resolved
            && let Some(target) = &c.target
        {
            calls_to
                .entry(target.clone())
                .or_default()
                .push(i as CallSiteId);
        }
    }
    CallGraph {
        sites,
        calls_to,
        upgraded: judged as u32,
    }
}

/// What the graph holds no call edge for, by name: how many sites spell a name
/// it named no body for, every qname an AMBIGUOUS site still guesses at, and
/// every attribute name read as a value rather than called (a `@property`'s
/// `scope.params`, a method passed on as `cache(self._compute)`). #60 may
/// claim a def only when its name is in none of them - nor in `shadowed`, the
/// imported qnames a second binding of the same local name hides from every
/// call site.
#[derive(Debug, Default)]
pub struct Unspoken {
    pub unnamed: IndexMap<String, u32>,
    pub guessed: BTreeSet<Qname>,
    pub valued: BTreeSet<String>,
    pub shadowed: BTreeSet<Qname>,
}

/// Qnames an import binds under a local name the module binds again: the
/// if/else fallback pair (`from x import f as g` in one arm, `def g` in the
/// other) leaves the index one home per name, so every `g()` resolves to
/// whichever came last and no site can speak for the other.
fn shadowed(facts: &RepoFacts<'_>) -> BTreeSet<Qname> {
    let mut out: BTreeSet<Qname> = BTreeSet::new();
    for module in facts.modules.values() {
        let mut froms: Vec<(String, Qname)> = Vec::new();
        for node in module.nodes(&[Kind::ImportFrom], Some(&module.qname), false) {
            let Cn::Stmt(Stmt::ImportFrom(n)) = module.nodes[node as usize] else {
                continue;
            };
            let base = module.rel_import_base(n.level, n.module.as_ref().map(|m| m.as_str()));
            for alias in &n.names {
                if alias.name.as_str() == "*" {
                    continue;
                }
                let local = alias.asname.as_ref().unwrap_or(&alias.name);
                froms.push((
                    local.to_string(),
                    format!("{base}.{}", alias.name).as_str().into(),
                ));
            }
        }
        let mut bound: HashMap<&str, u32> = HashMap::new();
        for id in facts
            .symbols_by_module
            .get(&module.qname)
            .into_iter()
            .flatten()
        {
            let sym = &facts.symbols[*id as usize];
            if sym.parent.is_none() {
                *bound.entry(&sym.name).or_default() += 1;
            }
        }
        for (local, _) in &froms {
            *bound.entry(local.as_str()).or_default() += 1;
        }
        out.extend(
            froms
                .iter()
                .filter(|(local, q)| {
                    bound.get(local.as_str()).is_some_and(|n| *n > 1)
                        && facts.symbols.contains_key(q)
                })
                .map(|(_, q)| q.clone()),
        );
    }
    out
}

/// A guess counts against the claim, which is what keeps a degraded run a
/// subset: with no oracle a by-name guess stands as RESOLVED, so CHA's
/// ambiguity is all that is left of the sites the oracle would re-judge.
pub fn unspoken(facts: &RepoFacts<'_>, calls: &CallGraph) -> Unspoken {
    let mut unnamed: IndexMap<String, u32> = IndexMap::new();
    let mut guessed: BTreeSet<Qname> = BTreeSet::new();
    for call in &calls.sites {
        if call.resolution == Resolution::Resolved {
            continue;
        }
        if call.resolution == Resolution::Ambiguous {
            guessed.extend(call.candidates.iter().cloned());
        }
        let Some(module) = facts.modules.get(&call.module) else {
            continue;
        };
        if let Cn::Expr(Expr::Call(c)) = module.nodes[call.node as usize]
            && let Some(name) = spelled(&c.func)
        {
            *unnamed.entry(name.to_string()).or_default() += 1;
        }
    }
    let mut valued: BTreeSet<String> = BTreeSet::new();
    for module in facts.modules.values() {
        let called: IndexSet<NodeIndex> = module
            .nodes(&[Kind::Call], None, false)
            .into_iter()
            .filter_map(|n| match module.nodes[n as usize] {
                Cn::Expr(Expr::Call(c)) => Cn::Expr(&c.func).stamped(),
                _ => None,
            })
            .collect();
        for n in module.nodes(&[Kind::Attribute], None, false) {
            if called.contains(&n) {
                continue;
            }
            if let Cn::Expr(Expr::Attribute(a)) = module.nodes[n as usize]
                && a.ctx == ExprContext::Load
            {
                valued.insert(a.attr.to_string());
            }
        }
    }
    Unspoken {
        unnamed,
        guessed,
        valued,
        shadowed: shadowed(facts),
    }
}

/// The name a call spells for its callee (`f()`, `x.f()`); `None` where the
/// callee is an expression (`handlers[k]()`).
fn spelled(func: &Expr) -> Option<&str> {
    match func {
        Expr::Attribute(a) => Some(a.attr.as_str()),
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// The repo function a site runs: a class call the first `__init__` up its
/// base chain, an instance call its class's `__call__`. `None` when no repo
/// body runs - an unresolved site, an external one, or a class with no
/// `__init__` anywhere.
pub fn callee_of(facts: &RepoFacts<'_>, site: &CallSite) -> Option<Qname> {
    if site.resolution != Resolution::Resolved {
        return None;
    }
    let target = site.target.as_ref()?;
    let sym = facts.symbols.get(target)?;
    if FUNCTION_KINDS.contains(&sym.kind) {
        return Some(target.clone());
    }
    let is_class = sym.kind == "class";
    let dunder = if is_class { "__init__" } else { "__call__" };
    let cls_q = if is_class {
        target.clone()
    } else {
        instance_class(facts, sym)?
    };
    class_walk(facts, &cls_q, Step::Bases)
        .into_iter()
        .find_map(|(_, info)| info.methods.get(dunder).cloned())
}

/// The class a module-level instance holds, read off its own initializer
/// (`handler = H()`): one hop over facts' call index, no propagation.
fn instance_class(facts: &RepoFacts<'_>, sym: &Symbol) -> Option<Qname> {
    let module = facts.modules.get(&sym.module)?;
    let init = match module.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::Assign(a)) => Some(&*a.value),
        Cn::Stmt(Stmt::AnnAssign(a)) => a.value.as_deref(),
        Cn::Stmt(Stmt::AugAssign(a)) => Some(&*a.value),
        _ => None,
    }?;
    let at = facts
        .call_index
        .get(&(module.id, Cn::Expr(init).stamped()?))?;
    let site = &facts.call_sites[*at as usize];
    if site.resolution != Resolution::Resolved {
        return None;
    }
    let target = site.target.as_ref()?;
    (facts.symbols.get(target)?.kind == "class").then(|| target.clone())
}

/// Caller enumeration with the prod/test split (test callers are veto, never
/// evidence). Meaningful only for `ClosedWorld`-passing symbols - #5 checks
/// the verdict first.
#[derive(Debug, Default)]
pub struct CallerSet<'a> {
    pub prod: Vec<&'a CallSite>,
    pub test: Vec<&'a CallSite>,
}

pub fn callers_of<'a>(qname: &str, facts: &RepoFacts<'_>, calls: &'a CallGraph) -> CallerSet<'a> {
    let mut out = CallerSet::default();
    for call in calls.callers(qname) {
        let test = facts
            .rel_of(&call.module)
            .is_some_and(|rel| is_test_path(rel));
        if test {
            out.test.push(call);
        } else {
            out.prod.push(call);
        }
    }
    out
}

// --- the `graph` dump layer ------------------------------------------------

/// `layer_graph`.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let calls = provers.calls(facts);
    let sites: Vec<Value> = calls
        .sites
        .iter()
        .map(|s| {
            let span = facts.modules.get(&s.module).and_then(|m| m.span(s.node));
            json!({
                "module": s.module.to_string(),
                "line": span.and_then(|p| p[0]),
                "col": span.and_then(|p| p[1]),
                "resolution": s.resolution.value(),
                "target": s.target.as_ref().map(|t| t.to_string()),
                "candidates": s.candidates.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
            })
        })
        .collect();
    let calls_to: serde_json::Map<String, Value> = calls
        .calls_to
        .iter()
        .map(|(q, v)| (q.to_string(), json!(v.len())))
        .collect();
    Some(json!({
        "sites": sites,
        "calls_to": calls_to,
        "upgraded": calls.upgraded,
    }))
}
