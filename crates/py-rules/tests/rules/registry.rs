//! Registry-level checks (`tests/rules/test_registry.py`): rule presence,
//! the posture table, tier derivation over every emitted finding.
//!
//! Dropped from the Python file: `test_rule_23_is_a_ranking_input` reads
//! `inspect.getsource`, which no Rust build has;
//! `test_explain_prints_every_record_under_one_id` is phase 9's `explain`
//! verb; `test_module_docstring_names_its_family_and_rules` reads
//! `__doc__`, which a compiled crate does not carry.
//!
use sightline_core::rule::{Posture, Scope};
use sightline_py_rules::{RULES, run_rules};
use sightline_testkit::build;

/// Ids no rule table may hold: retired, and answered by `explain` out of the
/// burial table instead (codemap 7).
const RETIRED: [&str; 18] = [
    "4", "8", "13", "15", "16", "17", "19", "22", "25", "28", "30", "31", "43", "45", "46", "51",
    "52", "61",
];

/// `test_every_rule_id_present`: the table holds `1..=61` less `RETIRED`,
/// each id once. The burials are in `corpus-ext/decisions.tsv`: 52 at
/// residue/cut, 8, 30 and 46 at g4/cut, 28 at todo3/py, 31 at
/// release/31-buried, the rest at g3/cut.
#[test]
fn every_rule_id_present() {
    let mut ids: Vec<&str> = RULES.iter().map(|r| r.record.id).collect();
    let held = ids.len();
    ids.sort_by_key(|id| id.parse::<u32>().expect("a rule id is a number"));
    ids.dedup();
    assert_eq!(ids.len(), held, "a rule id is spelled twice");
    let expected: Vec<String> = (1..=61u32)
        .map(|n| n.to_string())
        .filter(|id| !RETIRED.contains(&id.as_str()))
        .collect();
    assert_eq!(ids, expected);
}

/// What `gate` blocks on. No rule has held GATE since #31's burial, so the
/// whole blocking axis is RATCHET against a baseline, and the fast gate -
/// the PostToolUse hook and every `gate --files` run - reaches only the
/// file-scoped rules. With no RATCHET reading at file scope the hook blocks
/// on nothing and every gate test passes for the wrong reason; the pole then
/// proves nothing either, which `corpus::polarity` prints rather than counts
/// as proof.
#[test]
fn the_fast_gate_holds_a_reading_it_can_block_on() {
    assert!(
        RULES
            .iter()
            .any(|r| r.record.posture == Posture::Ratchet && r.record.scope == Scope::File)
    );
}

/// RATCHET is the default; REPORT costs the rule's own goal text saying why
/// the reading does not gate.
#[test]
fn report_posture_is_argued_in_the_rule_goal() {
    for rule in RULES {
        if rule.record.posture == Posture::Report {
            assert!(
                rule.record.goal.contains("gat"),
                "#{} claims REPORT without arguing it",
                rule.record.id
            );
        }
    }
}

/// A slug names the concept, so a language's reading shares its sibling's:
/// what is unique is the reading (slug, language), and the id a slug means.
#[test]
fn slugs_unique_and_metadata_present() {
    let keys: Vec<(&str, &str)> = RULES
        .iter()
        .map(|r| (r.record.slug, r.record.lang))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), keys.len());

    let mut by_slug: Vec<(&str, &str)> =
        RULES.iter().map(|r| (r.record.slug, r.record.id)).collect();
    by_slug.sort_unstable();
    by_slug.dedup();
    let mut slugs: Vec<&str> = RULES.iter().map(|r| r.record.slug).collect();
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(by_slug.len(), slugs.len());

    for rule in RULES {
        assert!(["A", "B", "C", "P", "T", "Z"].contains(&rule.record.family));
        assert!(!rule.record.meaning.is_empty() && !rule.record.goal.is_empty());
        if rule.record.family == "P" {
            // no static perf finding may ever gate
            assert_eq!(rule.record.posture, Posture::Report);
        }
    }
}

/// Engine is stamped from evidence and tier derives from engine; a rule
/// sets neither.
#[test]
fn tier_derives_from_engine_for_all_emitted_findings() {
    let (_dir, stack) = build(&[
        ("state.py", "cache = {}\n"),
        (
            "m.py",
            concat!(
                "from typing import Any\n",
                "from state import cache\n",
                "def load(cfg: dict[str, Any]) -> Any:\n",
                "    # Step 1: read\n",
                "    if cfg:\n",
                "        cfg.update({})\n",
                "    # Step 2: write\n",
                "    cache['k'] = 1\n",
                "    return cfg\n",
            ),
        ),
        (
            "n.py",
            "from state import cache\ndef wb():\n    cache.clear()\n",
        ),
    ]);
    let mut sink = sightline_core::findings::Sink::new();
    run_rules(
        stack.facts(),
        &stack.provers,
        &Default::default(),
        &mut sink,
        None,
    );
    assert!(!sink.0.is_empty(), "the fixture produces findings");
    for f in &sink.0 {
        assert_eq!(f.tier(), f.engine().tier());
        assert_eq!(f.lang, "py");
    }
}
