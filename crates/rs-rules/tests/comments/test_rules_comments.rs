//! #18, #34 and #39 over Rust facts: labeled phases inside one `fn`, a
//! non-doc comment run Rust parses as code, and one doc run on several items.

use sightline_core::findings::Finding;
use sightline_testkit::run_rs_rule;

/// (cause, line, salience), the triple the Python tests read.
fn rows(found: &[Finding]) -> Vec<(&str, u32, f64)> {
    found
        .iter()
        .map(|f| (f.cause.as_str(), f.site.line, f.salience))
        .collect()
}

// --- #18 section comments -----------------------------------------------------

#[test]
fn two_labeled_runs_in_one_fn_fire() {
    let found = run_rs_rule(
        "18",
        &[(
            "src/lib.rs",
            "pub fn run(x: u32) -> u32 {\n\
             \x20   // 1. widen the input\n\
             \x20   let y = x + 1;\n\
             \x20   // 2. narrow it back\n\
             \x20   y - 1\n\
             }\n",
        )],
    );

    assert_eq!(rows(&found), [("sections:demo_crate::run", 2, 2.0)]);
}

#[test]
fn one_label_per_fn_is_silent() {
    let found = run_rs_rule(
        "18",
        &[(
            "src/lib.rs",
            "pub fn run(x: u32) -> u32 {\n\
             \x20   // 1. widen the input\n\
             \x20   let y = x + 1;\n\
             \x20   // the caller narrows what it needs\n\
             \x20   y - 1\n\
             }\n\
             \n\
             pub fn other(x: u32) -> u32 {\n\
             \x20   // 2. narrow\n\
             \x20   x - 1\n\
             }\n",
        )],
    );

    assert!(found.is_empty());
}

/// A numbered rationale written as one comment block heads no code of its
/// own: it is one label, not two phases.
#[test]
fn labels_in_one_run_count_once() {
    let found = run_rs_rule(
        "18",
        &[(
            "src/lib.rs",
            "pub fn run(x: u32) -> u32 {\n\
             \x20   // 1. widen the input\n\
             \x20   // 2. narrow it back\n\
             \x20   x\n\
             }\n",
        )],
    );

    assert!(found.is_empty());
}

#[test]
fn module_level_and_doc_labels_are_silent() {
    let found = run_rs_rule(
        "18",
        &[(
            "src/lib.rs",
            "//! 1. the crate doc\n\
             \n\
             // 1. the first table\n\
             pub const A: u32 = 1;\n\
             \n\
             // 2. the second table\n\
             pub const B: u32 = 2;\n\
             \n\
             pub fn run() -> u32 {\n\
             \x20   /// 1. what inner does\n\
             \x20   fn inner() -> u32 { 1 }\n\
             \x20   /// 2. what other does\n\
             \x20   fn other() -> u32 { 2 }\n\
             \x20   inner() + other()\n\
             }\n",
        )],
    );

    assert!(found.is_empty());
}

// --- #34 commented-out code ---------------------------------------------------

#[test]
fn a_commented_out_rust_run_fires() {
    let found = run_rs_rule(
        "34",
        &[(
            "src/lib.rs",
            "pub fn run() -> u32 {\n\
             \x20   // let a = compute(1);\n\
             \x20   // let b = a + 2;\n\
             \x20   // println!(\"{}\", b);\n\
             \x20   3\n\
             }\n",
        )],
    );

    assert_eq!(rows(&found), [("commented-code:demo_crate:2", 2, 3.0)]);
}

#[test]
fn prose_short_and_doc_runs_are_silent() {
    let found = run_rs_rule(
        "34",
        &[(
            "src/lib.rs",
            "/// let a = 1;\n\
             /// let b = 2;\n\
             /// let c = 3;\n\
             pub const X: u32 = 1;\n\
             \n\
             pub fn run() -> u32 {\n\
             \x20   // widen the input by one, then narrow it back so the\n\
             \x20   // caller sees the identity, and the optimizer folds it\n\
             \x20   // away without changing what the function returns\n\
             \n\
             \x20   // let a = 1;\n\
             \x20   // let b = 2;\n\
             \x20   3\n\
             }\n",
        )],
    );

    assert!(found.is_empty());
}

// --- #34's identity `match` ---------------------------------------------------
// clippy's `needless_match` reads the same shape with types, and reports none
// of the sites the three Rust corpus repos hold.

#[test]
fn a_match_that_re_returns_every_pattern_fires() {
    let found = run_rs_rule(
        "34",
        &[(
            "src/lib.rs",
            "pub fn run(x: Result<u8, u8>) -> Result<u8, u8> {\n\
             \x20   match x {\n\
             \x20       Ok(v) => Ok(v),\n\
             \x20       Err(e) => Err(e),\n\
             \x20   }\n\
             }\n",
        )],
    );

    assert_eq!(rows(&found), [("noop-match:demo_crate::run", 2, 4.0)]);
    assert!(
        found[0]
            .message
            .contains("returns what it matched in every arm")
    );
}

/// `Poll::Ready(Err(e)) => Poll::Ready(Err(e))` beside an arm that rewrites is
/// a re-wrap the types force, not a no-op.
#[test]
fn an_arm_that_rewrites_makes_it_a_conversion() {
    let found = run_rs_rule(
        "34",
        &[(
            "src/lib.rs",
            "pub fn run(x: Result<u8, u8>) -> Result<u16, u8> {\n\
             \x20   match x {\n\
             \x20       Ok(v) => Ok(v as u16),\n\
             \x20       Err(e) => Err(e),\n\
             \x20   }\n\
             }\n",
        )],
    );

    assert!(found.is_empty());
}

#[test]
fn a_guarded_arm_never_spells_its_pattern_back() {
    let found = run_rs_rule(
        "34",
        &[(
            "src/lib.rs",
            "pub fn run(x: Result<u8, u8>) -> Result<u8, u8> {\n\
             \x20   match x {\n\
             \x20       Ok(v) if v > 1 => Ok(v),\n\
             \x20       Ok(v) => Ok(v),\n\
             \x20       Err(e) => Err(e),\n\
             \x20   }\n\
             }\n",
        )],
    );

    assert!(found.is_empty());
}

#[test]
fn a_block_arm_holding_one_expression_still_counts() {
    let found = run_rs_rule(
        "34",
        &[(
            "src/lib.rs",
            "pub fn run(x: Option<u8>) -> Option<u8> {\n\
             \x20   match x {\n\
             \x20       Some(v) => {\n\
             \x20           Some(v)\n\
             \x20       }\n\
             \x20       None => None,\n\
             \x20   }\n\
             }\n",
        )],
    );

    assert_eq!(
        found.iter().map(|f| f.cause.as_str()).collect::<Vec<_>>(),
        ["noop-match:demo_crate::run"]
    );
}

#[test]
fn an_arm_that_returns_its_pattern_is_the_same_shape() {
    let found = run_rs_rule(
        "34",
        &[(
            "src/lib.rs",
            "pub fn run(x: Result<u8, u8>) -> Result<u8, u8> {\n\
             \x20   match x {\n\
             \x20       Ok(v) => return Ok(v),\n\
             \x20       Err(e) => return Err(e),\n\
             \x20   }\n\
             }\n",
        )],
    );

    assert_eq!(
        found.iter().map(|f| f.cause.as_str()).collect::<Vec<_>>(),
        ["noop-match:demo_crate::run"]
    );
}

// --- #39 copied doc -----------------------------------------------------------
// One `///` run pasted onto items that do different things. The floors (two
// filled lines, 60 characters) and the one-name exemption are what tell a
// paste from the prose Rust makes an author write twice.

const LONG: &str = "/// The total-order key for tied extrema; attachment depth belongs\n\
                    /// only to arbitration after the active pair has been selected.\n";

#[test]
fn one_run_on_two_differently_named_fns_fires() {
    let source = format!(
        "{LONG}pub fn cell_source_key(x: u8) -> u8 {{ x }}\n\
         \n\
         {LONG}pub fn pin_source_key(x: u16) -> u16 {{ x }}\n"
    );
    let found = run_rs_rule("39", &[("src/lib.rs", &source)]);

    assert_eq!(
        found
            .iter()
            .map(|f| (f.site.line, &*f.site.symbol, f.salience))
            .collect::<Vec<_>>(),
        [(3, "demo_crate::cell_source_key", 2.0)]
    );
    assert_eq!(
        found[0].message,
        "the doc on demo_crate::cell_source_key is word for word the doc on \
         demo_crate::pin_source_key"
    );
    assert!(found[0].cause.starts_with("comment-discipline:doc-copied:"));
}

#[test]
fn a_one_line_run_is_shared_prose() {
    let found = run_rs_rule(
        "39",
        &[(
            "src/lib.rs",
            "/// The parsed document this stage hands the next one along.\n\
             pub fn first(x: u8) -> u8 { x }\n\
             \n\
             /// The parsed document this stage hands the next one along.\n\
             pub fn second(x: u16) -> u16 { x }\n",
        )],
    );

    assert!(found.is_empty());
}

#[test]
fn a_short_run_is_shared_prose() {
    let found = run_rs_rule(
        "39",
        &[(
            "src/lib.rs",
            "/// The name.\n/// Never empty.\npub fn first(x: u8) -> u8 { x }\n\
             \n\
             /// The name.\n/// Never empty.\npub fn second(x: u16) -> u16 { x }\n",
        )],
    );

    assert!(found.is_empty());
}

/// A builder and the value it builds document the same option twice.
#[test]
fn items_of_one_name_are_one_operation() {
    let source = format!(
        "pub struct A;\npub struct B;\n\
         impl A {{\n\
         {LONG}    pub fn source_key(&self) -> u8 {{ 0 }}\n\
         }}\n\
         impl B {{\n\
         {LONG}    pub fn source_key(&self) -> u8 {{ 0 }}\n\
         }}\n"
    );
    let found = run_rs_rule("39", &[("src/lib.rs", &source)]);

    assert!(found.is_empty());
}

#[test]
fn test_items_are_silent() {
    let source = format!(
        "#[cfg(test)]\nmod tests {{\n\
         {LONG}    fn cell_source_key(x: u8) -> u8 {{ x }}\n\
         {LONG}    fn pin_source_key(x: u16) -> u16 {{ x }}\n\
         }}\n"
    );
    let found = run_rs_rule("39", &[("src/lib.rs", &source)]);

    assert!(found.is_empty());
}

/// The `///` node spans into the row after it, so the item on that row owns
/// no doc: only `run` holds this one.
#[test]
fn a_const_in_a_body_never_claims_the_fns_doc() {
    let source = format!(
        "{LONG}pub fn run() -> u8 {{\n\
         \x20   const K: u8 = 1;\n\
         \x20   K\n\
         }}\n\
         \n\
         {LONG}pub fn other() -> u8 {{ 2 }}\n"
    );
    let found = run_rs_rule("39", &[("src/lib.rs", &source)]);

    assert_eq!(
        found.iter().map(|f| &*f.site.symbol).collect::<Vec<_>>(),
        ["demo_crate::other"]
    );
    assert!(found[0].message.ends_with("the doc on demo_crate::run"));
}
