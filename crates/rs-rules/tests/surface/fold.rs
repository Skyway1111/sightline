//! #48 over Rust: the one-line private `fn` with one prod call edge, and the
//! four shapes the judged round exempted.

use sightline_core::findings::{Engine, Evidence, Tier};
use sightline_testkit::run_rs_rule;

use crate::{causes, krate, run_with_edges};

const CALL: (&str, &str, &str, u32, bool) = (
    "demo_crate::caller",
    "demo_crate::helper",
    "src/lib.rs",
    2,
    true,
);
const CALLER: &str = "pub fn caller(x: u32) -> u32 { helper(x) }\n";

/// `_folds`: the helper the test writes, plus a caller for it.
fn folds(body: &str, caller: &str) -> String {
    format!("{body}{caller}")
}

/// The edge the graph would hold for a call at `line`.
fn call_at(line: u32) -> (&'static str, &'static str, &'static str, u32, bool) {
    (
        "demo_crate::caller",
        "demo_crate::helper",
        "src/lib.rs",
        line,
        true,
    )
}

#[test]
fn a_one_line_private_fn_with_one_call_edge_folds() {
    let src = folds("fn helper(x: u32) -> u32 { x + 1 }\n", CALLER);
    let found = run_with_edges("48", &krate(&src), &[CALL]);

    assert_eq!(causes(&found), ["fold:demo_crate::helper"]);
    assert_eq!(
        found[0].message,
        "demo_crate::helper (one line) is called once, from demo_crate::caller: fold it into \
         the caller"
    );
    assert_eq!(
        found[0].evidence,
        Evidence::Wp {
            premises: vec![
                "prod-callers:1".to_string(),
                "caller:demo_crate::caller".to_string()
            ]
        }
    );
    assert_eq!(
        (found[0].engine(), found[0].tier()),
        (Engine::Wp, Tier::Indexed)
    );
}

#[test]
fn a_body_a_block_holds_on_one_line_still_folds() {
    let src = folds("fn helper(x: u32) -> u32 {\n    x + 1\n}\n", CALLER);
    let found = run_with_edges("48", &krate(&src), &[call_at(4)]);

    assert_eq!(causes(&found), ["fold:demo_crate::helper"]);
}

#[test]
fn a_second_caller_is_reuse() {
    let src = folds("fn helper(x: u32) -> u32 { x + 1 }\n", CALLER);
    let other = (
        "demo_crate::other",
        "demo_crate::helper",
        "src/lib.rs",
        3,
        true,
    );
    let found = run_with_edges("48", &krate(&src), &[CALL, other]);

    assert!(found.is_empty());
}

#[test]
fn a_reference_that_is_not_a_call_is_still_a_reference() {
    // a fn pointer given to something else: the name has a second reader
    let src = folds("fn helper(x: u32) -> u32 { x + 1 }\n", CALLER);
    let table = (
        "demo_crate::table",
        "demo_crate::helper",
        "src/lib.rs",
        3,
        false,
    );
    let found = run_with_edges("48", &krate(&src), &[CALL, table]);

    assert!(found.is_empty());
}

#[test]
fn a_two_line_body_is_not_a_substitution() {
    let src = folds(
        "fn helper(x: u32) -> u32 {\n    let y = x + 1;\n    y * 2\n}\n",
        CALLER,
    );
    let found = run_with_edges("48", &krate(&src), &[call_at(5)]);

    assert!(found.is_empty());
}

#[test]
fn a_pub_crate_fn_is_not_private() {
    let src = folds("pub(crate) fn helper(x: u32) -> u32 { x + 1 }\n", CALLER);

    assert!(run_with_edges("48", &krate(&src), &[CALL]).is_empty());
}

#[test]
fn a_body_naming_a_literal_table_is_that_table() {
    let src = folds(
        "fn helper() -> [u32; 3] { [1, 2, 3] }\n",
        "pub fn caller() -> [u32; 3] { helper() }\n",
    );

    assert!(run_with_edges("48", &krate(&src), &[CALL]).is_empty());
}

#[test]
fn a_trait_impl_method_is_reached_through_its_trait() {
    let src = "pub trait Step { fn go(&self) -> u32; }\npub struct A;\n\
               impl Step for A { fn go(&self) -> u32 { 1 } }\n\
               pub fn caller(a: &A) -> u32 { a.go() }\n";
    let edge = (
        "demo_crate::caller",
        "demo_crate::A::go",
        "src/lib.rs",
        4,
        true,
    );

    assert!(run_with_edges("48", &krate(src), &[edge]).is_empty());
}

#[test]
fn a_name_a_macro_body_spells_has_readers_no_index_counts() {
    let src = "macro_rules! wire { () => { helper(1) }; }\n\
               fn helper(x: u32) -> u32 { x + 1 }\n\
               pub fn caller(x: u32) -> u32 { helper(x) }\n";

    assert!(run_with_edges("48", &krate(src), &[call_at(3)]).is_empty());
}

#[test]
fn a_test_caller_is_no_prod_caller() {
    let src = "fn helper(x: u32) -> u32 { x + 1 }\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
               fn t() { super::helper(1); }\n}\n";
    let edge = (
        "demo_crate::tests::t",
        "demo_crate::helper",
        "src/lib.rs",
        5,
        true,
    );

    assert!(run_with_edges("48", &krate(src), &[edge]).is_empty());
}

#[test]
fn no_edge_at_all_is_no_claim() {
    // the degraded run: without the oracle the graph is empty and #48 silent
    let src = folds("fn helper(x: u32) -> u32 { x + 1 }\n", CALLER);

    assert!(run_rs_rule("48", &krate(&src)).is_empty());
}

#[test]
fn a_name_an_attribute_string_spells_is_called_by_a_derive() {
    // doxx `#[serde(default = "default_preset")]`: serde's derive calls the fn
    // the string names, and no index holds that edge
    let src = "#[derive(Deserialize)]\npub struct Keymap {\n    \
               #[serde(default = \"default_preset\")]\n    pub preset: String,\n}\n\
               fn default_preset() -> String { \"default\".to_string() }\n\
               pub fn caller() -> String { default_preset() }\n";
    let edge = (
        "demo_crate::caller",
        "demo_crate::default_preset",
        "src/lib.rs",
        7,
        true,
    );

    assert!(run_with_edges("48", &krate(src), &[edge]).is_empty());
}

#[test]
fn a_doc_run_is_prose_the_call_site_cannot_hold() {
    // turmoil `abort_connection`, salvo `encode_uri`
    let src = folds(
        "/// Abort via retransmit exhaustion, not a reset.\nfn helper(x: u32) -> u32 { x + 1 }\n",
        CALLER,
    );

    assert!(run_with_edges("48", &krate(&src), &[call_at(3)]).is_empty());
}

#[test]
fn a_comment_in_the_body_is_prose_of_its_own() {
    // log4rs `is_env_var_start`: the comment records the regex it replaced
    let src = "fn helper(c: char) -> bool {\n    \
               // close replacement for the old character class\n    \
               c.is_alphanumeric()\n}\npub fn caller(c: char) -> bool { helper(c) }\n";

    assert!(run_with_edges("48", &krate(src), &[call_at(5)]).is_empty());
}

#[test]
fn a_field_of_the_receiver_is_the_types_own_reach() {
    // turmoil `FlowControl::has_credits`: folding leaks the ordering out
    let src = "pub struct S { credits: u32 }\nimpl S {\n    \
               fn helper(&self) -> bool { self.credits > 0 }\n    \
               pub fn caller(&self) -> bool { self.helper() }\n}\n";
    let edge = (
        "demo_crate::S::caller",
        "demo_crate::S::helper",
        "src/lib.rs",
        4,
        true,
    );

    assert!(run_with_edges("48", &krate(src), &[edge]).is_empty());
}

#[test]
fn a_macro_spelling_the_receiver_reaches_it_too() {
    // salvo `config_url`: a macro body is tokens, so the walk reads them
    let src = "pub struct S { issuer: String }\nimpl S {\n    \
               fn helper(&self) -> String { format!(\"{}/config\", &self.issuer) }\n    \
               pub fn caller(&self) -> String { self.helper() }\n}\n";
    let edge = (
        "demo_crate::S::caller",
        "demo_crate::S::helper",
        "src/lib.rs",
        4,
        true,
    );

    assert!(run_with_edges("48", &krate(src), &[edge]).is_empty());
}

#[test]
fn a_body_building_its_own_type_is_where_the_type_is_made() {
    // turmoil `FdGuard::new`: the caller would state the invariant itself
    let src = "pub struct S { fd: u32, armed: bool }\nimpl S {\n    \
               fn helper(fd: u32) -> Self { Self { fd, armed: true } }\n    \
               pub fn caller(fd: u32) -> Self { Self::helper(fd) }\n}\n";
    let edge = (
        "demo_crate::S::caller",
        "demo_crate::S::helper",
        "src/lib.rs",
        4,
        true,
    );

    assert!(run_with_edges("48", &krate(src), &[edge]).is_empty());
}

#[test]
fn a_method_call_on_the_receiver_reaches_no_state() {
    // salvo `jwks_uri`: `self.f()` is a hop like any other, so the fold stands
    let src = "pub struct S { issuer: String }\nimpl S {\n    \
               pub fn config(&self) -> String { self.issuer.clone() }\n    \
               fn helper(&self) -> String { self.config() }\n    \
               pub fn caller(&self) -> String { self.helper() }\n}\n";
    let edge = (
        "demo_crate::S::caller",
        "demo_crate::S::helper",
        "src/lib.rs",
        5,
        true,
    );
    let found = run_with_edges("48", &krate(src), &[edge]);

    assert_eq!(causes(&found), ["fold:demo_crate::S::helper"]);
}

#[test]
fn a_call_site_inside_a_boolean_condition_needs_parens() {
    // turmoil `is_ipv4_broadcast`, salvo `request_has_private_cache_headers`
    let src = "fn helper(x: u32) -> bool { x > 1 }\n\
               pub fn caller(x: u32, flag: bool) -> bool { helper(x) && !flag }\n";

    assert!(run_with_edges("48", &krate(src), &[CALL]).is_empty());
}

#[test]
fn a_call_site_in_a_match_guard_needs_parens() {
    // log4rs `is_env_var_part`: `Some(ch) if is_env_var_part(ch) =>`
    let src = "fn helper(x: u32) -> bool { x > 1 }\npub fn caller(v: Option<u32>) -> bool {\n    \
               match v {\n        Some(x) if helper(x) => true,\n        _ => false,\n    }\n}\n";

    assert!(run_with_edges("48", &krate(src), &[call_at(4)]).is_empty());
}

#[test]
fn a_plain_if_is_no_compound_condition() {
    // termimad `is_seq_start`: one predicate, one `if`, nothing to parenthesize
    let src = "fn helper(x: u32) -> bool { x > 1 }\n\
               pub fn caller(x: u32) -> bool { if helper(x) { true } else { false } }\n";
    let found = run_with_edges("48", &krate(src), &[CALL]);

    assert_eq!(causes(&found), ["fold:demo_crate::helper"]);
}
