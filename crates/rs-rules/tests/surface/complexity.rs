//! #23 over Rust: the SonarSource threshold on the ranking prior's score.

use sightline_testkit::run_rs_rule;

use crate::krate;

/// `_branches`: n `if` statements at one nesting level.
fn branches(n: u32) -> String {
    let arms: String = (0..n)
        .map(|i| format!("    if a == {i} {{ return {i} }}\n"))
        .collect();
    format!("pub fn f(a: i32) -> i32 {{\n{arms}    0\n}}\n")
}

#[test]
fn a_body_past_the_threshold_is_reported() {
    let src = branches(15);
    let found = run_rs_rule("23", &krate(&src));

    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].message,
        "demo_crate::f has cognitive complexity 15 (threshold 15)"
    );
    assert_eq!(&*found[0].site.symbol, "demo_crate::f");
    assert_eq!(found[0].cause, "cognitive-complexity:demo_crate::f");
    assert_eq!(found[0].salience, 15.0);
}

#[test]
fn a_body_one_decision_short_stays_silent() {
    let src = branches(14);

    assert!(run_rs_rule("23", &krate(&src)).is_empty());
}
