//! The two dead-weight readings the resolved
//! edges carry, #32 (an item no edge reaches, deleted in a world cargo
//! accepted) and #56 (an item only tests reach). Both judge the same set -
//! what a crate root reaches through bare `pub` - because that is the set
//! rustc's own `dead_code` stays silent on.

use std::collections::BTreeSet;

use sightline_core::findings::{Evidence, Finding, Qname, Sink};
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_rs_facts::model::{RsFacts, RsSymbol};
use sightline_rs_provers::RsProvers;
use sightline_rs_provers::closed_world::ClosedWorld;
use sightline_rs_provers::oracle::index::RsEdge;
use sightline_rs_provers::splice::{RsSplice, deletion, verify_splice};

use crate::Rule;
use crate::util::site;

const RUSTC_OWNS: &str = "rustc's dead_code covers private, `pub(crate)` and \
                          `pub`-in-private-module items, in a bin crate and a lib alike";

/// The items rustc leaves to us, in a stable order.
fn judged<'a, 't>(facts: &'a RsFacts<'t>, world: &ClosedWorld<'t>) -> Vec<&'a RsSymbol<'t>> {
    let mut names: Vec<&Qname> = world.reachable().iter().collect();
    names.sort();
    names.into_iter().map(|q| &facts.symbols[q]).collect()
}

/// The reference sits under a test: a `tests`/`benches`/`examples` path, or a
/// `#[cfg(test)]` module's item.
fn test_edge(facts: &RsFacts<'_>, edge: &RsEdge) -> bool {
    facts.is_test(&edge.rel)
        || facts
            .symbols
            .get(edge.caller.as_str())
            .is_some_and(|caller| caller.is_test)
}

pub const RULE_32: Rule = Rule {
    record: RuleRecord {
        id: "32",
        slug: "dead-symbols",
        family: "context",
        engine_class: "WP",
        posture: Posture::Ratchet,
        meaning: "a root-reachable `pub` item no resolved edge reaches, over a passed closed \
                  world and a deletion cargo checked without a new error",
        goal: "Dead code taxes every reader: an item the program never enters is context an \
               agent ingests for nothing. An application is where this fires, its whole \
               surface being judged: 11/11 real on doxx and mcfly read as applications, every \
               deletion cargo-checked clean.",
        lang: "rs",
        scope: Scope::Repo,
        complement: RUSTC_OWNS,
    },
    run: rule_32,
};

/// Items nothing in the tree references, each priced by deleting it: the
/// index alone would report every reference a macro writes or a feature arm
/// hides, so the finding ships only where a world without the item still
/// compiles. A bin crate, an application and an unpublished lib are where
/// this can fire at all - a published item escapes the closed world, its
/// callers being downstream.
fn rule_32<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    let world = provers.closed_world();
    let dead: Vec<&RsSymbol<'t>> = judged(facts, world)
        .into_iter()
        .filter(|sym| {
            provers.rust.graph.edges_to(&sym.qname).is_empty() && world.verdict(&sym.qname).passed
        })
        .collect();
    let splices: Vec<RsSplice> = dead
        .iter()
        .filter_map(|sym| deletion(facts, sym, &format!("dead-symbol:{}", sym.qname)))
        .collect();
    let verified = verify_splice(facts, provers.rust, splices);
    for sym in dead {
        let Some((evidence, fix)) = verified.get(format!("dead-symbol:{}", sym.qname).as_str())
        else {
            continue;
        };
        out.push(Finding {
            rule: "32",
            site: site(facts, sym, sym.node),
            message: format!(
                "{} {} is `pub` and reachable from the crate root, and no resolved reference \
                 in the repo names it - deleting it leaves cargo check clean",
                sym.kind, sym.qname
            ),
            cause: format!("dead-symbol:{}", sym.qname),
            evidence: evidence.clone(),
            salience: f64::from(sym.end_lineno - sym.lineno + 1),
            fix: Some(fix.clone()),
            lang: "rs",
        });
    }
}

pub const RULE_56: Rule = Rule {
    record: RuleRecord {
        id: "56",
        slug: "test-only-symbol",
        family: "context",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "a root-reachable `pub` item, over a passed closed world, every resolved edge \
                  to which sits under a test path or a `#[cfg(test)]` module",
        goal: "An item only its tests reach is not dead code - a reference to it does exist - \
               but it is a feature nothing ships, kept alive by tests proving nothing the \
               product does: delete both, the item and its tests. A reference implementation \
               shipped in the product module and entered only by tests and benches is the same \
               finding; its fix is `#[cfg(test)]`.",
        lang: "rs",
        scope: Scope::Repo,
        complement: RUSTC_OWNS,
    },
    run: rule_56,
};

/// Items the product never enters, over #32's judged set and the same
/// escapes; no patch, because the tests reaching it come out with it and no
/// world can price that.
fn rule_56<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    let world = provers.closed_world();
    for sym in judged(facts, world) {
        let edges = provers.rust.graph.edges_to(&sym.qname);
        if edges.is_empty() || !world.verdict(&sym.qname).passed {
            continue;
        }
        if !edges.iter().all(|e| test_edge(facts, e)) {
            continue;
        }
        let homes: Vec<&str> = edges
            .iter()
            .map(|e| e.rel.as_str())
            .collect::<BTreeSet<&str>>()
            .into_iter()
            .collect();
        out.push(Finding {
            rule: "56",
            site: site(facts, sym, sym.node),
            message: format!(
                "{} {} is referenced only by tests ({}) - delete both",
                sym.kind,
                sym.qname,
                homes.join(", ")
            ),
            cause: format!("test-only:{}", sym.qname),
            evidence: Evidence::idx(),
            salience: f64::from(sym.end_lineno - sym.lineno + 1),
            fix: None,
            lang: "rs",
        });
    }
}
