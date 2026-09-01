//! The cargo half of `tests/rs/test_rs_oracle.py`: the members, the check,
//! the unchecked set, the worlds and every degraded mode the toolchain can
//! reach. A test that builds a fixture spawns cargo and is `#[ignore]`
//! (decision 17); `xtask check` runs those as their own stage. Each fixture
//! loads once per binary.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use serde_json::json;
use sightline_core::config::Config;
use sightline_core::rule::RuleSet;
use sightline_rs_provers::RsProvers;
use sightline_rs_provers::oracle::cargo::{RA_AP, find_cargo, pinned};
use sightline_rs_provers::oracle::{RsAnswers, RsOracle, answers_of, answers_with};
use sightline_testkit::RsStack;
use sightline_testkit::rs_fixtures::{CRATE, blanked, borrowed, member, siblings, workspace};
use sightline_testkit::{build_rs, build_rs_oracle};
use tempfile::TempDir;

type Fixture = (TempDir, RsStack);

static ONE_CRATE: LazyLock<Fixture> = LazyLock::new(|| build_rs_oracle(CRATE));
static MEMBERS: LazyLock<Fixture> = LazyLock::new(|| {
    let files = workspace();
    build_rs_oracle(&borrowed(&files))
});
static SIBLINGS: LazyLock<Fixture> = LazyLock::new(|| {
    let files = siblings();
    build_rs_oracle(&borrowed(&files))
});

fn root(fixture: &Fixture) -> &Utf8Path {
    Utf8Path::from_path(fixture.0.path()).expect("a utf-8 temp path")
}

fn answers(fixture: &Fixture) -> &RsAnswers {
    fixture.1.provers().rust
}

fn source(fixture: &Fixture, rel: &str) -> String {
    std::fs::read_to_string(root(fixture).join(rel)).expect("the fixture's file")
}

/// One world, as `{rel: content}`, and the errors it added.
fn added(fixture: &Fixture, rel: &str, content: String) -> Vec<(String, String)> {
    let overlay = IndexMap::from([(rel.to_string(), content)]);
    let answered = answers(fixture).verify_worlds(&[("w".to_string(), overlay)]);
    let rows = answered.get("w").expect("the world answered").iter();
    rows.filter(|d| d.level == "error")
        .map(|d| (d.code.clone(), d.rel.clone()))
        .collect()
}

// --- the header ---------------------------------------------------------------

#[test]
#[ignore = "spawns cargo"]
fn the_header_names_the_tools_and_the_feature_set() {
    let block = &answers(&ONE_CRATE).block;

    assert_eq!(block["enabled"], json!(true));
    assert_eq!(block["features"], json!("default"));
    let tools = block["tools"].as_object().expect("a tools table");
    assert_eq!(
        tools.keys().cloned().collect::<Vec<_>>(),
        ["cargo", "ra_ap"]
    );
    assert!(
        tools
            .values()
            .all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
    );
}

#[test]
#[ignore = "spawns cargo"]
fn two_audits_of_one_tree_report_the_same_members_and_diagnostics() {
    let at = root(&ONE_CRATE);
    let mine = answers(&ONE_CRATE);
    let built = sightline_rs_facts::build::build_facts(
        at,
        &Config::new(),
        &sightline_core::walk::discover(at, &Config::new()),
        None,
    );
    let facts = built.borrow_dependent();
    let target = at.join("_target_twin");
    let twin = answers_of(
        RsOracle::new(at, &Config::new(), &facts.crates, Some(&target)),
        facts,
    );

    assert_eq!(twin.checked, mine.checked);
    assert_eq!(twin.unchecked, mine.unchecked);
    assert_eq!(twin.diagnostics, mine.diagnostics);
}

// --- worlds -------------------------------------------------------------------

#[test]
#[ignore = "spawns cargo"]
fn a_world_reports_the_error_its_overlay_adds_and_a_clean_one_none() {
    let lib = source(&ONE_CRATE, "src/lib.rs");

    assert_eq!(added(&ONE_CRATE, "src/lib.rs", lib.clone()), []);
    let gone = blanked(&lib, "pub fn concrete", 3);
    assert_eq!(
        added(&ONE_CRATE, "src/lib.rs", gone),
        [("E0425".to_string(), "src/lib.rs".to_string())]
    );
}

#[test]
#[ignore = "spawns cargo"]
fn a_member_whose_lib_errors_is_unchecked_and_the_rest_still_verify() {
    let good = source(&MEMBERS, "good/src/lib.rs");
    let rows = added(&MEMBERS, "good/src/lib.rs", blanked(&good, "pub fn ok", 1));
    let notes = answers(&MEMBERS)
        .oracle
        .as_ref()
        .expect("an oracle")
        .notes();

    let want: BTreeSet<String> = ["broken", "dependent"]
        .iter()
        .map(|n| n.to_string())
        .collect();
    assert_eq!(answers(&MEMBERS).unchecked, want);
    assert_eq!(
        answers(&MEMBERS)
            .checked
            .iter()
            .map(|m| &m.name)
            .collect::<Vec<_>>(),
        ["good"]
    );
    assert!(notes.iter().any(|n| n.contains("broken (lib broken)")));
    assert!(notes.iter().any(|n| n.contains("never reached dependent")));
    assert_eq!(
        rows.iter().map(|(_, rel)| rel.as_str()).collect::<Vec<_>>(),
        ["good/src/lib.rs"]
    );
}

// --- a tree with no manifest at its root ---------------------------------------

#[test]
#[ignore = "spawns cargo"]
fn a_root_with_no_manifest_answers_as_one_project_per_crate() {
    let found = answers(&SIBLINGS);
    let oracle = found.oracle.as_ref().expect("an oracle");

    assert_eq!(found.block["projects"], json!(["lower", "upper"]));
    let named: Vec<(&str, &str)> = oracle
        .members()
        .iter()
        .map(|m| (m.name.as_str(), m.dir.as_str()))
        .collect();
    assert_eq!(named, [("lower", "lower"), ("upper", "upper")]);
    assert!(found.unchecked.is_empty());
}

#[test]
#[ignore = "spawns cargo"]
fn a_world_checks_in_the_crates_that_read_the_overlay() {
    let lower = source(&SIBLINGS, "lower/src/lib.rs");
    let gone = blanked(&lower, "pub fn shared", 1);

    assert_eq!(
        added(&SIBLINGS, "lower/src/lib.rs", gone),
        [("E0425".to_string(), "upper/src/lib.rs".to_string())]
    );
}

// --- degraded modes -----------------------------------------------------------

#[test]
fn the_pin_names_cargo() {
    assert_eq!(pinned().keys().cloned().collect::<Vec<_>>(), ["cargo"]);
    assert!(!pinned()["cargo"].is_empty());
}

/// `RA_AP` is what the header reports; the manifest is what cargo compiled.
#[test]
fn the_manifest_pins_the_ra_ap_version_the_header_names() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let text = std::fs::read_to_string(manifest).expect("the crate's manifest");

    assert!(text.contains(&format!("version = \"={RA_AP}\"")));
}

#[test]
fn a_missing_tool_disables_the_oracle_and_the_header_says_so() {
    let (dir, stack) = build_rs(&[("src/lib.rs", "pub fn a() {}\n")]);
    let at = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let empty = std::env::join_paths([dir.path().join("no-tools")]).expect("a search path");

    let found = find_cargo(Some(&empty));
    let answers = answers_with(
        at,
        &Config::new(),
        &RuleSet::new(),
        stack.facts(),
        found.clone(),
    );
    let provers = RsProvers::new(stack.facts(), &answers);

    assert_eq!(found, None);
    assert!(answers.oracle.is_none() && answers.graph.edges.is_empty());
    let says = |n: &String| n.contains("cargo") && n.contains("not on PATH");
    assert!(answers.notes.iter().any(says));
    assert_eq!(
        provers.provenance(stack.facts())["rs"]["oracle"],
        json!({"enabled": false})
    );
}

#[test]
fn a_dead_check_stops_the_oracle_and_the_audit_runs() {
    let (dir, stack) = build_rs(&[
        ("Cargo.toml", &member("dead", "")),
        ("src/lib.rs", "pub fn a() {}\n"),
    ]);
    let at = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let target = at.join("_target");
    // a program that answers, but never in cargo's terms
    let dead = Utf8PathBuf::from_path_buf(std::env::current_exe().expect("this binary")).ok();

    let mut oracle = RsOracle::new(at, &Config::new(), &stack.facts().crates, Some(&target));
    oracle.cargo.exe = dead;
    let mut answers = answers_of(oracle, stack.facts());
    let notes = answers.close();
    let provers = RsProvers::new(stack.facts(), &answers);

    assert!(answers.graph.edges.is_empty() && answers.diagnostics.is_empty());
    assert!(
        answers
            .oracle
            .as_ref()
            .expect("an oracle")
            .failure()
            .is_some()
    );
    assert_eq!(
        provers.provenance(stack.facts())["rs"]["oracle"],
        json!({"enabled": false})
    );
    assert!(
        notes
            .iter()
            .any(|n| n.contains("rs oracle stopped") && n.contains("cargo check"))
    );
}
