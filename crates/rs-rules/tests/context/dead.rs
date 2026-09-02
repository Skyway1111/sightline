//! #32 and #56 against a real workspace under a temp root - a bin crate, an
//! unpublished lib with an integration test, and a member whose base check
//! fails. Every test that builds the workspace spawns cargo and is
//! `#[ignore]`; the fixture loads once per binary. The crates have no
//! dependencies, so no run touches the network.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use tempfile::TempDir;

use sightline_core::findings::{Evidence, Finding};
use sightline_rs_provers::splice::{deletion, verify_splice};
use sightline_testkit::rs_fixtures::{borrowed, member};
use sightline_testkit::{RsStack, build_rs, build_rs_oracle, run_rs_rule_on};

const WORKSPACE: &str =
    "[workspace]\nmembers = [\"binny\", \"libby\", \"broken\"]\nresolver = \"2\"\n";

const BIN: &str = r#"pub fn orphan_bin() {}

pub fn used_by_test() -> u32 {
    5
}

pub fn also_used() -> u32 {
    7
}

pub fn escapes_by_reference() -> u32 {
    9
}

#[cfg(feature = "extra")]
pub fn feature_gated() -> u32 {
    10
}

macro_rules! run {
    () => {
        crate::macro_called()
    };
}

pub fn macro_called() -> u32 {
    13
}

fn main() {
    let f: fn() -> u32 = escapes_by_reference;
    let total = also_used() + f() + run!();
    println!("{}", total);
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {
        let got = super::used_by_test();
        assert_eq!(got, 5);
    }
}
"#;

const LIB: &str = r#"pub mod inner;

/// An item nothing reaches.
pub fn orphan_lib() {}

pub fn needed_by_integration() -> u32 {
    3
}

pub(crate) fn rustc_owns() {}
"#;

fn files() -> Vec<(&'static str, String)> {
    vec![
        ("Cargo.toml", WORKSPACE.to_string()),
        (
            "binny/Cargo.toml",
            member("binny", "\n[features]\nextra = []\n"),
        ),
        ("binny/src/main.rs", BIN.to_string()),
        ("libby/Cargo.toml", member("libby", "publish = false\n")),
        ("libby/src/lib.rs", LIB.to_string()),
        (
            "libby/src/inner.rs",
            "pub fn deep_orphan() {}\n".to_string(),
        ),
        (
            "libby/tests/it.rs",
            "#[test]\nfn uses_it() {\n    assert_eq!(libby::needed_by_integration(), 3);\n}\n"
                .to_string(),
        ),
        ("broken/Cargo.toml", member("broken", "publish = false\n")),
        (
            "broken/src/lib.rs",
            "use std::nope::Missing;\npub fn bad() -> u32 { 1 }\n".to_string(),
        ),
    ]
}

static TREE: LazyLock<(TempDir, RsStack)> = LazyLock::new(|| {
    let sources = files();
    build_rs_oracle(&borrowed(&sources))
});

fn fired(id: &str) -> BTreeMap<String, Finding> {
    run_rs_rule_on(id, &TREE.1)
        .into_iter()
        .map(|f| (f.cause.clone(), f))
        .collect()
}

static DEAD_32: LazyLock<BTreeMap<String, Finding>> = LazyLock::new(|| fired("32"));
static DEAD_56: LazyLock<BTreeMap<String, Finding>> = LazyLock::new(|| fired("56"));

fn caused(found: &BTreeMap<String, Finding>) -> Vec<&str> {
    found.keys().map(String::as_str).collect()
}

// --- #32 ---------------------------------------------------------------------

#[test]
#[ignore = "spawns cargo"]
fn rule_32_fires_on_a_bin_and_an_unpublished_lib_item_no_edge_reaches() {
    assert_eq!(
        caused(&DEAD_32),
        [
            "dead-symbol:binny::orphan_bin",
            "dead-symbol:libby::inner::deep_orphan",
            "dead-symbol:libby::orphan_lib",
        ]
    );
}

#[test]
#[ignore = "spawns cargo"]
fn rule_32_carries_the_world_receipt_and_the_patch() {
    let found = &DEAD_32["dead-symbol:libby::orphan_lib"];

    assert_eq!(
        found.evidence,
        Evidence::Wp {
            premises: vec!["counterfactual:clean".to_string()]
        }
    );
    let fix = found.fix.as_ref().expect("the deletion is verified");
    assert_eq!(&*fix.rel, "libby/src/lib.rs");
    // the item's `///` line comes out with it
    assert_eq!(fix.edits.iter().map(|e| e.line).collect::<Vec<_>>(), [3, 4]);
}

#[test]
#[ignore = "spawns cargo"]
fn rule_32_is_silent_where_the_world_or_the_reading_says_so() {
    for qname in [
        "libby::needed_by_integration", // an edge reaches it
        "binny::also_used",             // a prod call edge
        "binny::escapes_by_reference",  // referenced, never called
        "binny::feature_gated",         // a `#[cfg(feature)]` arm
        "broken::bad",                  // a member the base check failed
        "libby::rustc_owns",            // `pub(crate)`: rustc's own dead_code
    ] {
        assert!(
            !DEAD_32.contains_key(&format!("dead-symbol:{qname}")),
            "{qname}"
        );
    }
}

/// A macro writes the only call, so no edge reaches `macro_called` and the
/// closed world passes; the world that deletes it does not compile.
#[test]
#[ignore = "spawns cargo"]
fn rule_32_is_silent_where_the_world_vetoes_the_deletion() {
    let provers = TREE.1.provers();

    assert!(provers.closed_world().verdict("binny::macro_called").passed);
    assert!(!DEAD_32.contains_key("dead-symbol:binny::macro_called"));
}

/// `--all-targets`: the only caller of this one is an integration test, which
/// a plain `cargo check` never compiles.
#[test]
#[ignore = "spawns cargo"]
fn verify_splice_returns_no_entry_for_a_vetoed_splice() {
    let provers = TREE.1.provers();
    let facts = provers.facts();
    let splices = ["libby::needed_by_integration", "libby::orphan_lib"]
        .into_iter()
        .zip(["used", "orphan"])
        .filter_map(|(qname, sid)| deletion(facts, &facts.symbols[qname], sid))
        .collect();

    let verified = verify_splice(facts, provers.rust, splices);

    assert_eq!(
        verified.keys().map(String::as_str).collect::<Vec<_>>(),
        ["orphan"]
    );
}

/// A container with no toolchain answers empty worlds: silence is not a
/// receipt, so `fix` ships nothing.
#[test]
fn no_oracle_verifies_nothing() {
    let (_dir, stack) = build_rs(&[
        (
            "Cargo.toml",
            "[package]\nname = \"solo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        ("src/main.rs", "pub fn f() {}\n"),
    ]);
    let provers = stack.provers();
    let facts = provers.facts();
    let splices = vec![deletion(facts, &facts.symbols["solo::f"], "f").expect("a deletion")];

    assert!(verify_splice(facts, provers.rust, splices).is_empty());
}

// --- #56 ---------------------------------------------------------------------

/// Both shapes: a `#[cfg(test)]` module's call and an integration test's,
/// each written through `assert_eq!`, whose tokens carry no call site.
#[test]
#[ignore = "spawns cargo"]
fn rule_56_fires_on_an_item_only_its_tests_reach() {
    assert_eq!(
        caused(&DEAD_56),
        [
            "test-only:binny::used_by_test",
            "test-only:libby::needed_by_integration",
        ]
    );
    assert!(
        DEAD_56["test-only:binny::used_by_test"]
            .message
            .contains("binny/src/main.rs")
    );
}

#[test]
#[ignore = "spawns cargo"]
fn rule_56_is_silent_where_a_prod_edge_exists() {
    assert!(!DEAD_56.contains_key("test-only:binny::also_used"));
}

#[test]
#[ignore = "spawns cargo"]
fn rule_56_proposes_no_patch() {
    assert!(DEAD_56["test-only:binny::used_by_test"].fix.is_none());
}
