//! Python rules: one file per rule family, one `pub const RULE_N: Rule`
//! beside each `fn rule_n`. A rule reads facts and provers and nothing else
//! (the fence): the crate lists no parser and no oracle crate, and
//! `clippy.toml` beside `Cargo.toml` bans file, process and environment
//! reads inside it.
//!
//! `deny(dead_code)` is what makes a `RULE_N` the `RULES` list forgot a
//! build error.

#![deny(dead_code)]

pub mod comments;
pub mod context;
pub mod dead;
pub mod describe;
pub mod emit;
pub mod framework;
pub mod idioms;
pub mod imports;
pub mod model;
pub mod oracle_errors;
pub mod perf;
pub mod records;
pub mod returns;
pub mod stack;
pub mod surface;
pub mod tests_quality;
pub mod trust;
pub mod util;

use std::time::{Duration, Instant};

use sightline_core::findings::Sink;
use sightline_core::lang::Timing;
use sightline_core::rule::{RuleSet, run_split};
use sightline_py_facts::model::RepoFacts;
use sightline_py_provers::Provers;

use crate::model::Rule;

/// Every Python rule, in id order (one language here, so `(id as int)` is
/// the whole key). `run_rules` yields findings in this order.
pub static RULES: &[&Rule] = &[
    &trust::RULE_1,
    &trust::RULE_2,
    &trust::RULE_3,
    &trust::RULE_5,
    &trust::RULE_6,
    &trust::RULE_7,
    &trust::RULE_9,
    &trust::RULE_10,
    &surface::RULE_11,
    &idioms::RULE_12,
    &surface::RULE_14,
    &surface::RULE_18,
    &surface::RULE_20,
    &surface::RULE_21,
    &surface::RULE_23,
    &context::RULE_24,
    &context::RULE_26,
    &context::RULE_27,
    &context::RULE_29,
    &dead::RULE_32,
    &returns::RULE_33,
    &dead::RULE_34,
    &imports::RULE_35,
    &context::RULE_36,
    &surface::RULE_37,
    &context::RULE_38,
    &comments::RULE_39,
    &trust::RULE_40,
    &perf::RULE_41,
    &tests_quality::RULE_42,
    &tests_quality::RULE_44,
    &tests_quality::RULE_47,
    &surface::RULE_48,
    &trust::RULE_49,
    &trust::RULE_50,
    &returns::RULE_53,
    &surface::RULE_54,
    &surface::RULE_55,
    &dead::RULE_56,
    &records::RULE_57,
    &oracle_errors::RULE_58,
    &context::RULE_59,
    &dead::RULE_60,
];

/// The ids `run_rules` keeps out of the parallel group: each owns a world,
/// which takes the checker mutably.
const WORLD_OWNERS: [&str; 2] = ["5", "10"];

/// Every rule over one build, findings pushed in `RULES` order through
/// `core::rule::run_split` (group A under rayon, then #5 and #10 sequential:
/// each owns a world, which takes the checker mutably). The order is
/// deterministic. A rule in `off` is skipped and timed at zero. The runner
/// stamps each finding's language, as prover machinery stamps its engine.
pub fn run_rules(
    facts: &RepoFacts<'_>,
    provers: &Provers,
    off: &RuleSet,
    sink: &mut Sink,
    timing: Timing,
) {
    let ids: Vec<&'static str> = RULES.iter().map(|r| r.record.id).collect();
    let run_one = |at: usize| -> (Vec<sightline_core::findings::Finding>, Duration) {
        let rule = RULES[at];
        if off.contains(rule.record.id) {
            return (Vec::new(), Duration::ZERO);
        }
        let started = Instant::now();
        let mut own = Sink::new();
        (rule.run)(facts, provers, &mut own);
        for f in &mut own.0 {
            f.lang = rule.record.lang;
        }
        (own.0, started.elapsed())
    };
    // every memo on this thread first: a memo built inside a rayon worker
    // can steal another rule's closure and re-enter itself (`Provers::warm`)
    provers.warm(facts);
    run_split(&ids, &WORLD_OWNERS, run_one, sink, timing);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_declared_in_id_order() {
        let ids: Vec<u32> = RULES
            .iter()
            .map(|r| r.record.id.parse().expect("a rule id is a number"))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn the_registry_holds_every_rule() {
        let records = RULES.iter().map(|r| r.record.clone()).collect();
        let reg =
            sightline_core::registry::Registry::new(records).expect("the records build a registry");
        assert_eq!(reg.rules.len(), RULES.len());
        for rule in RULES {
            assert_eq!(
                reg.reading(rule.record.id, "py").map(|r| r.slug),
                Some(rule.record.slug)
            );
        }
    }
}
