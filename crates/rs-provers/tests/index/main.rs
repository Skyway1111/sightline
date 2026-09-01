//! The index half of `tests/rs/test_rs_oracle.py`: the three dispatch
//! shapes, a call edge against a plain reference, a call written inside a
//! macro, a definition outside the project root, a reference across two
//! crates, two audits of one tree, and a root whose load stops.
//!
//! Every test here spawns cargo and loads the workspace, so every one is
//! `#[ignore]` (decision 17) and `xtask check` runs them. Each fixture loads
//! once for the binary.
//!
//! `test_an_index_that_reads_ranges_in_another_encoding_is_a_fault` has no
//! twin: there is no dump and no position encoding to read wrong.

use std::sync::LazyLock;

use camino::Utf8Path;
use sightline_core::config::Config;
use sightline_core::walk;
use sightline_rs_facts::build::build_facts;
use sightline_rs_provers::closed_world::ClosedWorld;
use sightline_rs_provers::oracle::index::{RsEdge, RsGraph};
use sightline_rs_provers::oracle::{RsAnswers, RsOracle, answers_of};
use sightline_testkit::RsStack;
use sightline_testkit::rs_fixtures::{self, borrowed};
use sightline_testkit::{build_rs, build_rs_oracle, make_repo};
use tempfile::TempDir;

/// `LIB` as one package: the `crate` fixture of the Python file.
static ONE: LazyLock<(TempDir, RsStack)> = LazyLock::new(|| build_rs_oracle(rs_fixtures::CRATE));

/// Two packages under a root with no manifest, one a path dependency of the
/// other, so the oracle runs a project per package.
static TWO: LazyLock<(TempDir, RsStack)> = LazyLock::new(|| {
    let files = rs_fixtures::siblings();
    build_rs_oracle(&borrowed(&files))
});

fn answers(fixture: &'static LazyLock<(TempDir, RsStack)>) -> &'static RsAnswers {
    fixture.1.provers().rust
}

fn graph(fixture: &'static LazyLock<(TempDir, RsStack)>) -> &'static RsGraph {
    &answers(fixture).graph
}

/// `[(e.callee, e.open) for e in edges]`.
fn dispatch(edges: &[&RsEdge]) -> Vec<(String, bool)> {
    edges.iter().map(|e| (e.callee.clone(), e.open)).collect()
}

// --- dispatch ---------------------------------------------------------------

#[test]
#[ignore = "loads the workspace"]
fn a_concrete_receiver_resolves_to_the_impl_method() {
    let edges = graph(&ONE).calls_from("fixture::concrete");

    assert_eq!(
        dispatch(&edges),
        [("fixture::Loud::hello".to_string(), false)]
    );
}

/// A bound or a trait object dispatches at runtime, so the edge names the
/// trait and is open: the body that runs is not the one it points at.
#[test]
#[ignore = "loads the workspace"]
fn a_generic_or_dyn_receiver_resolves_to_the_trait() {
    for caller in ["fixture::generic", "fixture::dynamic"] {
        let edges = graph(&ONE).calls_from(caller);

        assert_eq!(
            dispatch(&edges),
            [("fixture::Greet".to_string(), true)],
            "{caller}"
        );
    }
}

/// `Loud` does not override `twice`, so the call lands in the trait's own
/// body: the trait is the callee, and the calls that body makes are its
/// edges.
#[test]
#[ignore = "loads the workspace"]
fn a_default_method_resolves_to_its_body_in_the_trait() {
    let calls = graph(&ONE).calls_from("fixture::defaulted");

    assert_eq!(dispatch(&calls), [("fixture::Greet".to_string(), true)]);
    let inner: Vec<&str> = graph(&ONE)
        .calls_from("fixture::Greet")
        .iter()
        .map(|e| e.callee.as_str())
        .collect();
    assert_eq!(inner, ["fixture::Greet", "fixture::Greet"]);
}

// --- the join ---------------------------------------------------------------

#[test]
#[ignore = "loads the workspace"]
fn a_call_edge_and_a_plain_reference_are_told_apart() {
    let mut callees: Vec<&str> = graph(&ONE)
        .calls_from("fixture::caller")
        .iter()
        .map(|e| e.callee.as_str())
        .collect();
    callees.sort_unstable();

    assert_eq!(callees, ["fixture::concrete", "fixture::limited"]);
    let calls: Vec<bool> = graph(&ONE)
        .edges_to("fixture::LIMIT")
        .iter()
        .map(|e| e.call)
        .collect();
    assert_eq!(calls, [false]);
    assert!(graph(&ONE).calls_to("fixture::LIMIT").is_empty());
}

/// `apply(helper) + helper()`: the range at the call's callee column is the
/// call, the argument a reference (blind audit A: both once read as calls,
/// and the closed world lost its `reference-escape`).
#[test]
#[ignore = "loads the workspace"]
fn two_ranges_of_one_name_on_one_line_are_a_reference_and_a_call() {
    let edges = graph(&ONE).edges_to("fixture::helper");

    let mut calls: Vec<bool> = edges.iter().map(|e| e.call).collect();
    calls.sort_unstable();
    assert_eq!(calls, [false, true]);
    let callers: Vec<&str> = edges.iter().map(|e| e.caller.as_str()).collect();
    assert_eq!(callers, ["fixture::both", "fixture::both"]);
}

/// tree-sitter leaves a macro's tokens unparsed, so no call site covers
/// `assert_eq!(only_asserted(), 3)` and the source has to say the name is
/// called; without that the closed world escapes the callee as a plain
/// reference.
#[test]
#[ignore = "loads the workspace"]
fn a_call_written_inside_a_macro_is_a_call() {
    let edges = graph(&ONE).edges_to("fixture::only_asserted");

    let found: Vec<(bool, &str)> = edges.iter().map(|e| (e.call, e.caller.as_str())).collect();
    assert_eq!(found, [(true, "fixture::tests::asserts")]);
    let world = ClosedWorld::new(ONE.1.facts(), answers(&ONE));
    assert!(world.verdict("fixture::only_asserted").passed);
    assert_eq!(answers(&ONE).block["macro_edges"], serde_json::json!(1));
}

/// `String::new` is defined in the standard library, whose files the vfs
/// holds and the join drops.
#[test]
#[ignore = "loads the workspace"]
fn a_definition_outside_the_project_root_is_no_edge() {
    assert!(graph(&ONE).calls_from("fixture::outside").is_empty());

    let block = &answers(&ONE).block;
    assert!(block["documents_out"].as_u64().expect("a count") > 0);
    assert_eq!(block["documents_in"], serde_json::json!(1));
}

/// The token order is rust-analyzer's; the edges are ours.
#[test]
#[ignore = "loads the workspace twice"]
fn two_audits_of_one_tree_report_the_same_edges() {
    let dir = make_repo(rs_fixtures::CRATE);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let config = Config::new();
    let listing = walk::discover(root, &config);
    let built = build_facts(root, &config, &listing, None);
    let facts = built.borrow_dependent();

    let first = answers_of(oracle_at(root, &config, facts, "_one"), facts);
    let second = answers_of(oracle_at(root, &config, facts, "_two"), facts);

    assert_eq!(first.graph.edges, second.graph.edges);
    assert_eq!(first.graph.counts, second.graph.counts);
}

fn oracle_at(
    root: &Utf8Path,
    config: &Config,
    facts: &sightline_rs_facts::model::RsFacts<'_>,
    target: &str,
) -> RsOracle {
    RsOracle::new(root, config, &facts.crates, Some(&root.join(target)))
}

/// `lower`'s files are `upper`'s load too, as a path dependency's. Read
/// against `lower`'s own project root they would be outside it and dropped;
/// read against the audited root they are the tree's, and the edge joins.
#[test]
#[ignore = "loads two workspaces"]
fn a_reference_across_two_crates_resolves() {
    let calls = graph(&TWO).calls_from("upper::uses_shared");

    let found: Vec<(&str, &str)> = calls
        .iter()
        .map(|e| (e.callee.as_str(), e.rel.as_str()))
        .collect();
    assert_eq!(found, [("lower::shared", "upper/src/lib.rs")]);
    assert_eq!(
        answers(&TWO).block["cross_crate_edges"],
        serde_json::json!(1)
    );
}

// --- degraded modes ---------------------------------------------------------

/// A manifest cargo cannot read ends the oracle, not the run: the graph goes
/// empty, the header says the oracle is off and names why.
#[test]
#[ignore = "runs cargo"]
fn a_dead_index_stops_the_oracle_and_the_audit_runs() {
    let (dir, stack) = build_rs(&[
        ("Cargo.toml", "this is not a manifest\n"),
        ("src/lib.rs", "pub fn a() {}\n"),
    ]);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let config = Config::new();
    let facts = stack.facts();

    let mut found = answers_of(oracle_at(root, &config, facts, "_target"), facts);
    let notes = found.close();

    assert!(found.graph.edges.is_empty() && found.diagnostics.is_empty());
    assert_eq!(found.block, serde_json::json!({"enabled": false}));
    assert!(
        notes
            .iter()
            .any(|n| n.contains("rs oracle stopped") && n.contains("workspace load")),
        "{notes:?}"
    );
}
