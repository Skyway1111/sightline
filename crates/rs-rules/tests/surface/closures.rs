//! #20 over Rust: the closure body written three times in one module.

use sightline_core::findings::Tier;
use sightline_testkit::run_rs_rule;

use crate::krate;

const SORT: &str = "|p: &Row, q: &Row| p.rank().cmp(&q.rank())";

/// `_closures`: one `fn` binding each body in turn.
fn closures(bodies: &[&str]) -> String {
    let lines: String = bodies
        .iter()
        .enumerate()
        .map(|(i, b)| format!("    let c{i} = {b};\n"))
        .collect();
    format!("pub fn f() {{\n{lines}    go();\n}}\n")
}

#[test]
fn a_closure_written_three_times_is_a_finding() {
    let src = closures(&[SORT, SORT, SORT]);
    let found = run_rs_rule("20", &krate(&src));

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].site.line, 2);
    assert!(found[0].message.contains("appears 3x in demo_crate"));
    assert_eq!(found[0].tier(), Tier::Heuristic);
}

#[test]
fn two_copies_of_a_closure_stay_silent() {
    let src = closures(&[SORT, SORT]);

    assert!(run_rs_rule("20", &krate(&src)).is_empty());
}

#[test]
fn three_closures_that_read_different_fields_are_not_one() {
    let other = "|p: &Row, q: &Row| p.seed().cmp(&q.seed())";
    let src = closures(&[SORT, SORT, other]);

    assert!(run_rs_rule("20", &krate(&src)).is_empty());
}

#[test]
fn a_closure_under_the_node_floor_is_never_named() {
    let src = closures(&["|x: i32| x", "|y: i32| y", "|z: i32| z"]);

    assert!(run_rs_rule("20", &krate(&src)).is_empty());
}

#[test]
fn the_node_floor_is_the_node_it_is() {
    // `d.n > 4` is five nodes, `!d.ok` four: three copies of each, one named
    let (over, under) = ("|d| d.n > 4", "|d| !d.ok");
    let src = closures(&[over, over, over, under, under, under]);
    let found = run_rs_rule("20", &krate(&src));

    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains(&format!("closure `{over}`")));
}

#[test]
fn a_closure_that_only_forwards_names_is_never_named() {
    let forward = "|d| d.rows.len()";
    let src = closures(&[forward, forward, forward]);

    assert!(run_rs_rule("20", &krate(&src)).is_empty());
}

#[test]
fn a_forwarding_closure_that_compares_something_is_still_named() {
    let decides = "|d| d.rows.len() > 4";
    let src = closures(&[decides, decides, decides]);
    let found = run_rs_rule("20", &krate(&src));

    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("appears 3x"));
}

#[test]
fn copies_inside_test_code_are_never_named() {
    let lines: String = (0..3)
        .map(|i| format!("        let c{i} = {SORT};\n"))
        .collect();
    let src = format!(
        "#[cfg(test)]\nmod tests {{\n    #[test]\n    fn f() {{\n{lines}        go();\n    }}\n}}\n"
    );

    assert!(run_rs_rule("20", &krate(&src)).is_empty());
}

#[test]
fn the_message_names_the_module_and_the_count() {
    let src = closures(&[SORT, SORT, SORT]);
    let found = run_rs_rule("20", &krate(&src));

    assert_eq!(
        found[0].message,
        format!("closure `{SORT}` appears 3x in demo_crate - name it once")
    );
    assert_eq!(found[0].salience, 3.0);
    assert!(found[0].cause.starts_with("closure:demo_crate:"));
    assert_eq!(found[0].cause.len(), "closure:demo_crate:".len() + 8);
}
