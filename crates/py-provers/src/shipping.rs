//! What the repo ships as a unit, the module sets a prod list of source-file
//! names copies into a runtime of its own. The import surface of a module
//! inside such a set is pinned by the copy, and no kind of import-time work
//! in the target tells that hoist from a safe one (#35's emitter rejects it;
//! `import_effects.rs` owns the kind question).

use std::collections::BTreeSet;

use ruff_python_ast::Expr;
use rustc_hash::FxHashMap;

use sightline_core::findings::Qname;
use sightline_core::pytext;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{NodeIndex, RepoFacts, is_test_path};
use sightline_py_facts::module::Module;

use crate::imports::{import_targets, internal_module};

/// Module sets the repo copies as a unit: a module-scope collection of string
/// literals in prod code, two or more of which name a module file by its path
/// tail. Such a list stages a runtime elsewhere, so its modules run with
/// nothing outside it on the path (ROFL's `_ROFL_CONTAINER_FILES` stages
/// `rofl/metadata.py` without `rofl/data.py`, and its bootstrap suite pins
/// that). A test's list of files asserts over the tree and ships nothing.
pub fn shipped_subsets(facts: &RepoFacts<'_>) -> Vec<BTreeSet<Qname>> {
    let mut by_tail: FxHashMap<&str, Vec<&Qname>> = FxHashMap::default();
    for module in facts.modules.values() {
        by_tail
            .entry(pytext::rpartition(&module.rel, "/").2)
            .or_default()
            .push(&module.qname);
    }
    let mut out: Vec<BTreeSet<Qname>> = Vec::new();
    for module in facts.modules.values() {
        if is_test_path(&module.rel) {
            continue;
        }
        for node in module.nodes(
            &[Kind::List, Kind::Tuple, Kind::Set],
            Some(&module.qname),
            false,
        ) {
            let elts: &[Expr] = match module.nodes[node as usize] {
                Cn::Expr(Expr::List(l)) => &l.elts,
                Cn::Expr(Expr::Tuple(t)) => &t.elts,
                Cn::Expr(Expr::Set(s)) => &s.elts,
                _ => continue,
            };
            let literals: Option<Vec<&str>> = (elts.len() >= 2)
                .then(|| {
                    elts.iter()
                        .map(|e| match e {
                            Expr::StringLiteral(s) => Some(s.value.to_str()),
                            _ => None,
                        })
                        .collect()
                })
                .flatten();
            let Some(literals) = literals else { continue };
            let named: BTreeSet<Qname> = literals
                .into_iter()
                .filter(|text| text.ends_with(".py"))
                .flat_map(|text| {
                    by_tail
                        .get(pytext::rpartition(text, "/").2)
                        .map_or(&[][..], |v| v)
                        .iter()
                        .filter(move |q| facts.modules[&***q].rel.ends_with(text))
                        .map(|q| (*q).clone())
                })
                .collect();
            if named.len() >= 2 && named.len() < facts.modules.len() {
                out.push(named);
            }
        }
    }
    out
}

/// The modules hoisting `node` would pull into a shipped subset holding its
/// own module - the targets the subset does not carry. The set already stages
/// what its own members import, so only the edge the hoist adds is asked
/// about; non-empty is an import-surface growth, and the copy has no such file.
pub fn surface_growth(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    node: NodeIndex,
    subsets: &[BTreeSet<Qname>],
) -> BTreeSet<Qname> {
    let dsts: BTreeSet<Qname> = import_targets(facts, module, node)
        .iter()
        .filter_map(|t| internal_module(facts, t).cloned())
        .collect();
    subsets
        .iter()
        .filter(|s| s.contains(&module.qname))
        .flat_map(|s| dsts.difference(s).cloned())
        .collect()
}
