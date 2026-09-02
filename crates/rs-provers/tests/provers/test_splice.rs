//! `deletion` and `verify_splice` over the `LIB` fixture below. Every
//! expected value comes from running the two on that fixture.

use sightline_rs_provers::oracle::RsAnswers;
use sightline_rs_provers::splice::{deletion, verify_splice};
use sightline_testkit::build_rs;

const LIB: &str = "/// what f does
/// a second doc row
#[inline]
pub fn f() {}

// a detached note

pub fn g() {}
";

fn edits(splice: &sightline_rs_provers::splice::RsSplice) -> Vec<(u32, u32, u32, &str)> {
    splice
        .edits
        .iter()
        .map(|e| (e.line, e.col_start, e.col_end, e.text.as_str()))
        .collect()
}

#[test]
fn a_deletion_takes_the_doc_and_attribute_run_that_abuts_the_item() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", LIB)]);
    let facts = stack.facts();

    let f = deletion(facts, &facts.symbols["demo_crate::f"], "one").expect("f is in range");
    assert_eq!(f.rel, "src/lib.rs");
    assert_eq!(f.id, "one");
    assert_eq!(
        edits(&f),
        [
            (1, 0, 15, ""),
            (2, 0, 20, ""),
            (3, 0, 9, ""),
            (4, 0, 13, "")
        ]
    );

    // a blank line between the comment and `g` breaks the run, so the
    // splice is the item's own line
    let g = deletion(facts, &facts.symbols["demo_crate::g"], "two").expect("g is in range");
    assert_eq!(edits(&g), [(8, 0, 13, "")]);
}

/// Silence verifies nothing: without an oracle no world answers, so the
/// merged world goes missing and every splice is dropped rather than cleared.
#[test]
fn without_an_oracle_no_splice_is_verified() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", LIB)]);
    let facts = stack.facts();
    let both: Vec<_> = ["demo_crate::f", "demo_crate::g"]
        .iter()
        .map(|q| deletion(facts, &facts.symbols[*q], q).expect("in range"))
        .collect();

    assert!(verify_splice(facts, &RsAnswers::default(), both).is_empty());
    assert!(verify_splice(facts, &RsAnswers::default(), Vec::new()).is_empty());
}
