//! Rust rules: one file per rule family, one `pub const RULE_N: Rule` beside
//! each `fn rule_n`. A rule reads facts and provers and nothing else (the
//! fence): the crate lists no parser and no oracle crate, `Node` reaches it
//! through `rs_facts::Node`, and `clippy.toml` beside `Cargo.toml` bans
//! file, process and environment reads inside it.
//!
//! `deny(dead_code)` is what makes a `RULE_N` the `RULES` list forgot a
//! build error.

#![deny(dead_code)]

pub mod comments;
pub mod context;
pub mod dead;
pub mod describe;
pub mod emit;
pub mod stack;
pub mod surface;
pub mod tests_quality;
pub mod trust;
pub mod util;

use std::time::{Duration, Instant};

use sightline_core::findings::{Finding, Sink};
use sightline_core::lang::Timing;
use sightline_core::rule::{RuleRecord, RuleSet, run_split};
use sightline_rs_facts::model::RsFacts;
use sightline_rs_provers::RsProvers;

/// A rule fn: facts and provers in, findings out in yield order. The
/// provers borrow the facts they read, so the two share one `'t`.
pub type RuleFn = for<'t> fn(&'t RsFacts<'t>, &RsProvers<'t>, &mut Sink);

/// One rule: the record that describes it beside the fn that reads it.
/// `pub const RULE_N: Rule` sits next to `fn rule_n`, and `RULES` lists it
/// (`deny(dead_code)` fails the build for one the list forgot).
pub struct Rule {
    pub record: RuleRecord,
    pub run: RuleFn,
}

/// Every Rust rule, in id order (one language here, so `(id as int)` is the
/// whole key). `run_rules` yields findings in this order.
pub static RULES: &[&Rule] = &[
    &trust::RULE_9,
    &surface::RULE_11,
    &comments::RULE_18,
    &surface::RULE_20,
    &surface::RULE_21,
    &surface::RULE_23,
    &context::RULE_27,
    &context::RULE_29,
    &dead::RULE_32,
    &comments::RULE_34,
    &surface::RULE_37,
    &comments::RULE_39,
    &tests_quality::RULE_42,
    &tests_quality::RULE_47,
    &surface::RULE_48,
    &trust::RULE_53,
    &dead::RULE_56,
    &context::RULE_59,
];

/// The one rule `run_rules` keeps out of the parallel group: #32 cargo-checks
/// its deletion worlds inside its fn, after every reader.
const WORLD_OWNERS: [&str; 1] = ["32"];

/// Every rule over one pass's provers, findings pushed in `RULES` order
/// through `core::rule::run_split` (group A under rayon, #32 sequential
/// after). A rule in `off` is skipped and timed at zero, as the Python
/// runner times it. The runner stamps each finding's language, as prover
/// machinery stamps its engine.
pub fn run_rules(provers: &RsProvers<'_>, off: &RuleSet, sink: &mut Sink, timing: Timing) {
    let facts = provers.facts();
    let ids: Vec<&'static str> = RULES.iter().map(|r| r.record.id).collect();
    let run_one = |at: usize| -> (Vec<Finding>, Duration) {
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
    // every memo whose initializer runs rayon jobs first, on this thread: one
    // first touched inside a worker can steal another rule's closure and
    // re-enter itself (`Provers::warm` on the Python side)
    provers.warm();
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
                reg.reading(rule.record.id, "rs").map(|r| r.slug),
                Some(rule.record.slug)
            );
        }
    }
}
