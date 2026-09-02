//! Effect summaries, bottom-up over the SCC condensation of the call graph's
//! callee edges (`callgraph::callee_of`: a class call runs `__init__`).
//! Unknown taints: any site the graph cannot follow to a body makes a summary
//! `unknown` rather than clean. An external call has the effect classes
//! `catalog.rs` gives it - a heuristic priced by the WP tier.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use indexmap::IndexMap;
use rayon::prelude::*;
use ruff_python_ast::{Expr, Stmt, StmtRaise};
use serde_json::{Value, json};

use sightline_core::catalog::IO;
use sightline_core::findings::Qname;
use sightline_core::graph::tarjan_scc;
use sightline_py_facts::astutil::{RECEIVERS, chain_root, is_mutable_init, line_span, walk};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{CallSite, NodeIndex, RefKind, RepoFacts, Resolution, Symbol};
use sightline_py_facts::module::Module;
use sightline_py_facts::qnames::resolve_qname;

use crate::Provers;
use crate::callgraph::callee_of;
use crate::catalog::classes_of;
use crate::closed_world::ClosedWorld;
use crate::handlers::is_exception;
use crate::scope::{Scope, functions, is_mutation_context};

/// Atoms about the callee's receiver: what they mean to a caller depends on
/// whose object the call site hands over (`owned`).
const MUTATES_SELF: &str = "mutates-self";
const MUTATES_FIELD: &str = "mutates-field";
/// `_CHAIN`: a call result on the way is no one's.
const CHAIN: [Kind; 2] = [Kind::Attribute, Kind::Subscript];
/// Closed-world escapes under which calling `q` by name runs something other
/// than the body the graph read; the rest (re-export, a passed reference,
/// kwargs, nesting) only say callers may exist the graph cannot see.
const CALLED_ESCAPES: [&str; 3] = ["unknown-decorator", "method-override", "dynamic-access"];
/// A library base opens the caller set, never the body.
const BODY_KNOWN: [&str; 1] = ["framework-base"];
/// A raise whose type the module's bindings cannot name (`raise e`).
pub const UNNAMED: &str = "?";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects {
    /// `gw:<qname>` writes, `gm:<qname>` memo fills, `gr:<qname>` mutable
    /// reads, `io`, `mutates-arg` (a param or an alias of one), `mutates-self`
    /// (the receiver's own slots), `mutates-field` (through what a receiver
    /// field holds, possibly a caller's object), and the pair the fold keeps
    /// apart from a store, `gs:<qname>` / `slots-arg` (a callee wrote the own
    /// slots of a global / of a passed object).
    pub atoms: BTreeSet<String>,
    /// an edge escaped resolution: never claim clean
    pub unknown: bool,
}

impl Effects {
    pub fn clean(&self) -> bool {
        self.atoms.is_empty() && !self.unknown
    }
}

/// Per call edge: a receiver atom of the callee translated into the caller's
/// atoms for it, `None` when the object's owner is unknown.
#[derive(Debug, Clone)]
struct Xlate {
    mutates_self: Option<BTreeSet<String>>,
    mutates_field: Option<BTreeSet<String>>,
}

impl Default for Xlate {
    /// `dict.fromkeys(_RECEIVER_ATOMS, frozenset())`.
    fn default() -> Xlate {
        Xlate {
            mutates_self: Some(BTreeSet::new()),
            mutates_field: Some(BTreeSet::new()),
        }
    }
}

impl Xlate {
    fn get(&self, atom: &str) -> &Option<BTreeSet<String>> {
        if atom == MUTATES_SELF {
            &self.mutates_self
        } else {
            &self.mutates_field
        }
    }

    /// `edge[a] = None if None in (owned, edge[a]) else edge[a] | owned`.
    fn merge(&mut self, site: Xlate) {
        for (slot, owned) in [
            (&mut self.mutates_self, site.mutates_self),
            (&mut self.mutates_field, site.mutates_field),
        ] {
            *slot = match (slot.take(), owned) {
                (Some(mut have), Some(owned)) => {
                    have.extend(owned);
                    Some(have)
                }
                _ => None,
            };
        }
    }
}

/// The type a raise statement names through the module's bindings
/// (`raise errors.ParseError(...)` is `ParseError`), a builtin by its name;
/// `None` for a bare re-raise, `UNNAMED` for what neither binds.
mod atoms;

pub use atoms::raised_name;
use atoms::{
    Direct, branch_tested, is_class_symbol, is_io, mutable_global, through_field, translate,
};

/// Per function: own atoms, the unknown-tainted set, resolved call edges and
/// per edge the callee's receiver atoms translated (`translate`), unknown when
/// any site behind the edge cannot name the owner.
fn direct_effects(facts: &RepoFacts<'_>, sites: &[CallSite], provers: &Provers) -> Direct {
    let mut atoms: IndexMap<Qname, BTreeSet<String>> = functions(facts)
        .into_iter()
        .map(|q| (q.clone(), BTreeSet::new()))
        .collect();
    let mut unknown: HashSet<Qname> = HashSet::new();
    let mut edges: BTreeMap<String, BTreeSet<String>> = atoms
        .keys()
        .map(|q| (q.to_string(), BTreeSet::new()))
        .collect();
    let mut xlate: HashMap<(String, String), Xlate> = HashMap::new();
    // `_branch_tested` per writing owner
    let mut tested: HashMap<Qname, HashMap<String, u32>> = HashMap::new();

    let mutable_globals: HashSet<&Qname> = facts
        .symbols
        .iter()
        .filter(|(_, s)| s.kind == "variable" && s.parent.is_none() && mutable_global(facts, s))
        .map(|(q, _)| q)
        .collect();

    for r in &facts.refs {
        let Some(target_sym) = facts.symbols.get(&r.target) else {
            continue;
        };
        if target_sym.kind != "variable" {
            continue;
        }
        let Some(module) = facts.modules.get(&r.module) else {
            continue;
        };
        let owner = facts.enclosing(module, r.node);
        if !atoms.contains_key(&owner) {
            continue;
        }
        if r.kind == RefKind::Store || is_mutation_context(module, r.node) {
            // tested before written: the write can only fill what the test
            // found missing - a memo, invisible to every caller
            let at = tested
                .entry(owner.clone())
                .or_insert_with(|| branch_tested(module, &owner))
                .get(&*target_sym.name)
                .copied();
            let kind = match at {
                Some(at) if at <= module.line_of(r.node) => "gm",
                _ => "gw",
            };
            atoms[&owner].insert(format!("{kind}:{}", r.target));
        } else if mutable_globals.contains(&r.target) {
            atoms[&owner].insert(format!("gr:{}", r.target));
        }
    }

    for call in sites {
        let owner = &call.enclosing;
        if !atoms.contains_key(owner) {
            continue;
        }
        if call.resolution == Resolution::Resolved {
            let callee = callee_of(facts, call);
            let target = call.target.as_ref().and_then(|t| facts.symbols.get(t));
            if let Some(callee) = callee {
                edges
                    .get_mut(&**owner)
                    .expect("every function is a graph node")
                    .insert(callee.to_string());
                let edge = xlate
                    .entry((owner.to_string(), callee.to_string()))
                    .or_default();
                if let Some(module) = facts.modules.get(&call.module)
                    && let Cn::Expr(Expr::Call(node)) = module.nodes[call.node as usize]
                    && let Some(scope) = provers.scope_of(facts, owner)
                {
                    let is_class = target.is_some_and(is_class_symbol);
                    edge.merge(translate(facts, module, scope, node, is_class));
                }
            } else if target.is_none_or(|t| t.kind != "class") {
                // a name bound to a value, not to a body
                unknown.insert(owner.clone());
            }
        } else if call.resolution != Resolution::External {
            unknown.insert(owner.clone());
        } else if facts
            .modules
            .get(&call.module)
            .is_some_and(|m| is_io(m, call))
        {
            atoms[owner].insert("io".to_string());
        }
    }

    let receivers: BTreeSet<&str> = RECEIVERS.into_iter().collect();
    for (q, own) in &mut atoms {
        let Some(scope) = provers.scope_of(facts, q) else {
            continue;
        };
        let Some(module) = facts
            .symbols
            .get(q)
            .and_then(|s| facts.modules.get(&s.module))
        else {
            continue;
        };
        let params: BTreeSet<&str> = scope.params(facts).iter().map(String::as_str).collect();
        let through: BTreeSet<&str> = scope
            .writes(facts)
            .iter()
            .filter(|w| w.own && through_field(module, w.node))
            .filter_map(|w| w.root.as_deref())
            .collect();
        let mutated = scope.mutated_params(facts);
        let mutates_arg = mutated.iter().any(|p| !receivers.contains(p.as_str()))
            || scope.mutates_alias(facts)
            || through
                .iter()
                .any(|t| params.contains(t) && !receivers.contains(t));
        if mutates_arg {
            own.insert("mutates-arg".to_string());
        }
        if mutated.iter().any(|p| receivers.contains(p.as_str())) {
            own.insert(MUTATES_SELF.to_string());
        }
        if through
            .iter()
            .any(|t| params.contains(t) && receivers.contains(t))
        {
            own.insert(MUTATES_FIELD.to_string());
        }
    }
    (atoms, unknown, edges, xlate)
}

/// Does calling `q` by name run something other than its body: a wrapper, an
/// override, a reflective rebinding, or a store to its name. Read off every
/// escape that holds, not the first the world names.
fn escapes_when_called(facts: &RepoFacts<'_>, closed_world: &ClosedWorld, q: &str) -> bool {
    closed_world
        .verdict(q)
        .reasons
        .iter()
        .any(|r| CALLED_ESCAPES.contains(&r.as_str()))
        || facts.refs_to.get(q).is_some_and(|refs| {
            refs.iter()
                .any(|i| facts.refs[*i as usize].kind == RefKind::Store)
        })
}

/// A callee's summary as one edge's caller inherits it.
fn fold(callee: &Effects, edge: &Xlate) -> (BTreeSet<String>, bool) {
    let mut atoms: BTreeSet<String> = callee
        .atoms
        .iter()
        .filter(|a| *a != MUTATES_SELF && *a != MUTATES_FIELD)
        .cloned()
        .collect();
    let owned: Vec<&Option<BTreeSet<String>>> = callee
        .atoms
        .iter()
        .filter(|a| *a == MUTATES_SELF || *a == MUTATES_FIELD)
        .map(|a| edge.get(a))
        .collect();
    for o in owned.iter().filter_map(|o| o.as_ref()) {
        atoms.extend(o.iter().cloned());
    }
    (atoms, callee.unknown || owned.iter().any(|o| o.is_none()))
}

/// Symbols the `ClosedWorld` says escaped are forced unknown: an escape that
/// changes what a call by name runs (`escapes_when_called`) taints the callers
/// too, seeded before the fold; the others only the symbol. The verdicts
/// and the fold read one call graph, the `Provers` memo's.
pub fn summaries(facts: &RepoFacts<'_>, provers: &Provers) -> IndexMap<Qname, Effects> {
    let closed_world = provers.closed_world(facts);
    // the three alias products every function's fold reads, warmed in
    // parallel into `scope_of`'s memo
    functions(facts).into_par_iter().for_each(|q| {
        if let Some(scope) = provers.scope_of(facts, q) {
            scope.mutated_params(facts);
            scope.mutates_alias(facts);
            scope.alias_tainted(facts);
        }
    });

    let (atoms, mut unknown, edges, xlate) =
        direct_effects(facts, &provers.calls(facts).sites, provers);
    unknown.extend(
        atoms
            .keys()
            .filter(|q| escapes_when_called(facts, closed_world, q))
            .cloned(),
    );
    let (components, comp_of) = tarjan_scc(&edges);
    // components are emitted callees-first by Tarjan: fold successor effects up
    let mut summary: Vec<Effects> = Vec::with_capacity(components.len());
    for (ci, comp) in components.iter().enumerate() {
        let mut out = Effects::default();
        for q in comp {
            out.atoms
                .extend(atoms.get(q.as_str()).into_iter().flatten().cloned());
            if unknown.contains(q.as_str()) {
                out.unknown = true;
            }
            for c in &edges[q] {
                if comp_of[c] == ci {
                    continue;
                }
                let edge = xlate
                    .get(&(q.clone(), c.clone()))
                    .expect("every edge was translated");
                let (inherited, taints) = fold(&summary[comp_of[c]], edge);
                out.atoms.extend(inherited);
                out.unknown = out.unknown || taints;
            }
        }
        summary.push(out);
    }
    atoms
        .keys()
        .map(|q| {
            let own = &summary[comp_of[&**q]];
            let escaped = closed_world
                .verdict(q)
                .reasons
                .iter()
                .any(|r| !BODY_KNOWN.contains(&r.as_str()));
            (
                q.clone(),
                Effects {
                    atoms: own.atoms.clone(),
                    unknown: own.unknown || escaped,
                },
            )
        })
        .collect()
}

/// `layer_effects`.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let rows: serde_json::Map<String, Value> = provers
        .effects(facts)
        .iter()
        .map(|(q, e)| {
            (
                q.to_string(),
                json!({ "atoms": e.atoms, "unknown": e.unknown }),
            )
        })
        .collect();
    Some(json!({ "functions": rows }))
}
