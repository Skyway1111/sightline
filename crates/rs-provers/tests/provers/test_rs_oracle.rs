//! The toolchain-free part of `tests/rs/test_rs_oracle.py`: the two arms
//! where `build_answers` never asks the toolchain and says so in the header.
//! Every other test in that file spawns cargo and lands with the index in
//! phase 7.

use sightline_core::config::Config;
use sightline_core::rule::RuleSet;
use sightline_rs_provers::oracle::{ORACLE_RULES, build_answers};
use sightline_testkit::build_rs;

#[test]
fn every_oracle_rule_off_skips_the_toolchain() {
    let (dir, stack) = build_rs(&[("src/lib.rs", "pub fn f() {}\n")]);
    let root = camino::Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let off: RuleSet = ORACLE_RULES.iter().map(|id| (*id).to_string()).collect();

    let answers = build_answers(root, &Config::new(), &off, stack.facts());

    assert_eq!(answers.block, serde_json::json!({"enabled": false}));
    assert!(answers.notes.iter().any(|n| n.contains("not run")));
}

#[test]
fn the_config_can_turn_the_oracle_off() {
    let (dir, stack) = build_rs(&[("src/lib.rs", "pub fn a() {}\n")]);
    let root = camino::Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let config = Config {
        oracle: false,
        ..Config::new()
    };

    let answers = build_answers(root, &config, &RuleSet::new(), stack.facts());

    assert_eq!(answers.block, serde_json::json!({"enabled": false}));
    assert!(
        answers
            .notes
            .iter()
            .any(|n| n.contains("disabled by config"))
    );
    assert!(answers.graph.edges.is_empty());
}
