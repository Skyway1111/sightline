//! Family C, import topology: #35, the cycles the repo did not declare, over
//! the import graph prover.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use ruff_python_ast::Stmt;

use sightline_core::clones::digest_n;
use sightline_core::findings::{Evidence, Finding, Qname, Sink};
use sightline_core::graph::{edges, tarjan_scc};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{FUNCTION_KINDS, NodeIndex, RepoFacts, Resolution, is_test_path};
use sightline_py_facts::module::Module;
use sightline_py_facts::unparse;
use sightline_py_provers::Provers;
use sightline_py_provers::counterfactual::Splice;
use sightline_py_provers::import_effects::binds_only;
use sightline_py_provers::imports::{
    ImportGraph, in_type_checking, internal_module, loads, module_imports,
};
use sightline_py_provers::shipping::surface_growth;

use crate::model::Rule;
use crate::util::{deletion, node_site};

// --- #35 import topology ------------------------------------------------------
// Cycles in the facts import graph, plus cycles that hide behind deferred
// (function-scoped, guarded) imports; a TYPE_CHECKING-only edge is the
// checker's and closes none.

/// One import cycle: `(sorted component, reported names, the top-level cycle
/// members it already covers, hidden?)`.
struct Cycle {
    comp: Vec<String>,
    names: Vec<String>,
    known: Vec<String>,
    hidden: bool,
}

/// Top-level SCCs, then runtime any-scope SCCs no top-level SCC equals,
/// type-only edges excluded.
fn cycles(graph: &ImportGraph) -> Vec<Cycle> {
    let (top, _) = tarjan_scc(&edges(graph.top.iter()));
    let runtime: BTreeMap<String, BTreeSet<String>> = graph
        .full
        .iter()
        .map(|(q, dsts)| {
            let typed = &graph.typed[q];
            (
                q.to_string(),
                dsts.iter()
                    .filter(|d| !typed.contains(*d))
                    .map(|d| d.to_string())
                    .collect(),
            )
        })
        .collect();
    let (lazy, _) = tarjan_scc(&runtime);
    let members = |c: &Vec<String>| -> BTreeSet<String> { c.iter().cloned().collect() };
    let top_sets: HashSet<BTreeSet<String>> =
        top.iter().filter(|c| c.len() > 1).map(&members).collect();
    let mut out = Vec::new();
    let walk = top
        .iter()
        .chain(lazy.iter().filter(|c| !top_sets.contains(&members(c))));
    for comp in walk {
        if comp.len() < 2 {
            continue;
        }
        let set = members(comp);
        let hidden = !top_sets.contains(&set);
        let mut known: Vec<String> = Vec::new();
        if hidden {
            for s in &top_sets {
                if s.is_subset(&set) {
                    known.extend(s.iter().cloned());
                }
            }
        }
        known.sort();
        let mut names: Vec<String> = set.iter().filter(|q| !known.contains(q)).cloned().collect();
        if names.is_empty() {
            names = set.iter().cloned().collect();
        }
        out.push(Cycle {
            comp: set.into_iter().collect(),
            names,
            known,
            hidden,
        });
    }
    out
}

/// A prod function-scope import outside TYPE_CHECKING and not under an
/// `if`/`try` (a guarded import is an intentional deferral).
fn deferred_import(facts: &RepoFacts<'_>, module: &Module<'_>, node: NodeIndex) -> bool {
    if is_test_path(&module.rel) || in_type_checking(module, node) {
        return false;
    }
    let Some(owner) = facts.enclosing_symbol(module, node) else {
        return false;
    };
    if !FUNCTION_KINDS.contains(&owner.kind) {
        return false;
    }
    let mut cur = module.parent_of(node);
    while let Some(up) = cur {
        if up == owner.node {
            break;
        }
        if matches!(module.nodes[up as usize].kind(), Kind::If | Kind::Try) {
            return false;
        }
        cur = module.parent_of(up);
    }
    true
}

/// Prod function-scope imports none of whose internal targets reach the
/// importer over any-scope edges, and whose targets' top-level closure calls
/// no repo code at module scope: the deferral hides nothing. One row per node,
/// with its first internal target.
fn hoistable_imports<'a, 't>(
    facts: &'a RepoFacts<'t>,
    graph: &ImportGraph,
) -> Vec<(&'a Module<'t>, NodeIndex, Qname)> {
    let working: HashSet<&Qname> = facts
        .call_sites
        .iter()
        .filter(|c| c.enclosing == c.module && c.resolution == Resolution::Resolved)
        .map(|c| &c.module)
        .collect();
    let rows = module_imports(facts);
    let mut out = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let mut j = i;
        while j < rows.len() && rows[j].0 == rows[i].0 && rows[j].1 == rows[i].1 {
            j += 1;
        }
        let (qname, node) = (&rows[i].0, rows[i].1);
        let module = &facts.modules[qname];
        // a dynamic import is not a statement to hoist
        if matches!(
            module.nodes[node as usize].kind(),
            Kind::Import | Kind::ImportFrom
        ) {
            let dsts: Vec<Qname> = rows[i..j]
                .iter()
                .filter_map(|(_, _, t)| internal_module(facts, t))
                .filter(|d| **d != module.qname)
                .cloned()
                .collect();
            let clean = |d: &Qname| {
                !graph.reach(d, false).contains(&module.qname)
                    && loads(graph, d).iter().all(|m| !working.contains(m))
            };
            if !dsts.is_empty() && deferred_import(facts, module, node) && dsts.iter().all(clean) {
                out.push((module, node, dsts[0].clone()));
            }
        }
        i = j;
    }
    out
}

pub const RULE_35: Rule = Rule {
    record: RuleRecord {
        id: "35",
        slug: "import-topology",
        family: "context",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "import cycles (SCCs), cycles hidden behind deferred imports, and \
                  function-scope internal imports whose deferral hides no cycle",
        goal: "Modules should form a DAG: a cycle means no member can be \
               understood or loaded alone, and a lazy import only hides it.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_35,
};

/// SCCs of the top-level import graph (each anchored at the sorted-first
/// member) + SCCs of the runtime any-scope graph that no top-level SCC equals
/// + function-scope imports whose deferral hides no cycle.
fn rule_35(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let graph = provers.import_graph(facts);
    for cycle in cycles(graph) {
        let anchor: Qname = cycle.names[0].as_str().into();
        let node = cycle
            .comp
            .iter()
            .find_map(|d| graph.first.get(&(anchor.clone(), d.as_str().into())))
            .copied()
            .expect("an SCC member has an edge out of the anchor");
        let shown = joined(&cycle.names, 5);
        let message = if !cycle.known.is_empty() && cycle.names.len() < cycle.comp.len() {
            format!(
                "{shown} joins the import cycle of {} behind a deferred import",
                cycle
                    .known
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else if cycle.hidden {
            format!("{shown} import each other (cycle hidden behind a deferred import)")
        } else {
            format!("import cycle of {} modules: {shown}", cycle.names.len())
        };
        let cause = if cycle.hidden {
            format!("entangled:{}", cycle.names.join("<->"))
        } else {
            format!("import-cycle:{}", digest_n(&cycle.names.join("|"), 8))
        };
        out.push(Finding {
            rule: "35",
            site: node_site(facts, &facts.modules[&anchor], node),
            message,
            cause,
            evidence: Evidence::idx(),
            salience: if cycle.hidden {
                1.0
            } else {
                cycle.names.len() as f64
            },
            fix: None,
            lang: "py",
        });
    }
    let effects = provers.import_effects(facts);
    let subsets = provers.shipped_subsets(facts);
    for (module, node, dst) in hoistable_imports(facts, graph) {
        let grows = surface_growth(facts, module, node, subsets);
        let deferred = if !grows.is_empty() {
            format!(
                " - hoisting it pulls {} into a file set the repo ships {} without",
                grows
                    .iter()
                    .take(3)
                    .map(|q| &**q)
                    .collect::<Vec<_>>()
                    .join(", "),
                module.qname
            )
        } else if binds_only(facts, module, node, effects) {
            String::new()
        } else {
            format!(
                " - {dst} runs code at import time, which the deferral moves to \
                 first call"
            )
        };
        let Cn::Stmt(st) = module.nodes[node as usize] else {
            continue;
        };
        out.push(Finding {
            rule: "35",
            site: node_site(facts, module, node),
            message: format!(
                "function-scope import of {dst} hides no cycle{deferred}: hoist it \
                 (an intentional startup deferral takes # sightline-ok: 35)"
            ),
            // keyed by scope and statement, never the line: a RATCHET key that
            // moved with every edit above it re-reported the same site
            cause: format!(
                "hoistable-import:{}:{}",
                facts.enclosing(module, node),
                unparse::stmt(st)
            ),
            evidence: Evidence::idx(),
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

/// The first `n` names, an ellipsis where more follow.
fn joined(names: &[String], n: usize) -> String {
    names.iter().take(n).cloned().collect::<Vec<_>>().join(", ")
        + if names.len() > n { " ..." } else { "" }
}

/// #35's hoistable import as a patch: the statement leaves its line and joins
/// the top of the file. Silent where the line holds more than the statement as
/// written, where a name it binds already means something at module scope, and
/// where a string reaches that name. Stricter than the finding on the target:
/// the hoist moves import time, so every target must only bind names and may
/// not grow an import surface a file set the repo ships pins.
pub fn hoist_splice(cause: &str, facts: &RepoFacts<'_>, provers: &Provers) -> Option<Splice> {
    let mut parts = cause.splitn(3, ':');
    let (prefix, scope, stmt) = match (parts.next(), parts.next(), parts.next()) {
        (Some(p), Some(s), Some(t)) => (p, s, t),
        _ => ("", "", ""),
    };
    if prefix != "hoistable-import" {
        return None;
    }
    let owner = facts.symbols.get(scope)?;
    let module = facts.modules.get(&owner.module)?;
    let node = module
        .nodes(&[Kind::Import, Kind::ImportFrom], Some(scope), false)
        .into_iter()
        .find(|n| match module.nodes[*n as usize] {
            Cn::Stmt(st) => unparse::stmt(st) == stmt,
            _ => false,
        })?;
    let Cn::Stmt(st) = module.nodes[node as usize] else {
        return None;
    };
    let aliases = match st {
        Stmt::Import(n) => &n.names,
        Stmt::ImportFrom(n) => &n.names,
        _ => return None,
    };
    let strings = &provers.unseen(facts).strings;
    let taken = aliases.iter().any(|a| {
        let local = a.asname.as_ref().unwrap_or(&a.name);
        let head = local.split('.').next().unwrap_or("");
        let tail = local.split('.').next_back().unwrap_or("");
        module.bindings.contains_key(head) || strings.contains(tail)
    });
    if pytext::strip(module.lines[module.line_of(node) as usize - 1]) != stmt || taken {
        return None;
    }
    if !binds_only(facts, module, node, provers.import_effects(facts))
        || !surface_growth(facts, module, node, provers.shipped_subsets(facts)).is_empty()
    {
        return None;
    }
    let edits = deletion(module, node);
    (!edits.is_empty()).then(|| Splice {
        id: cause.to_string(),
        owner: module.qname.to_string(),
        edits,
        spelling: String::new(),
        imports: vec![stmt.to_string()],
        param: String::new(),
    })
}
