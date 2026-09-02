//! #11 over Rust: whole-body clone groups and the block runs under them.

use sightline_core::findings::Tier;
use sightline_testkit::{MANIFEST, run_rs_rule};

use crate::{BODY, causes, krate};

#[test]
fn two_bodies_of_one_shape_are_a_clone_group() {
    let src = format!("pub fn one() {{\n{BODY}}}\npub fn two() {{\n{BODY}}}\n");
    let found = run_rs_rule("11", &krate(&src));

    assert_eq!(
        found.iter().map(|f| f.site.line).collect::<Vec<u32>>(),
        [1, 9]
    );
    assert!(
        found
            .iter()
            .all(|f| f.message.starts_with("structural clone x2: "))
    );
    assert!(found.iter().all(|f| f.cause == found[0].cause));
    assert_eq!(found[0].tier(), Tier::Indexed);
}

#[test]
fn a_body_under_the_node_floor_is_no_clone() {
    let found = run_rs_rule(
        "11",
        &krate("pub fn one() { go(); }\npub fn two() { go(); }\n"),
    );

    assert!(found.is_empty());
}

#[test]
fn a_repeated_run_inside_two_bodies_is_a_block_clone() {
    let src = format!("pub fn one() {{\n{BODY}}}\npub fn two() {{\n    setup();\n{BODY}}}\n");
    let found = run_rs_rule("11", &krate(&src));

    assert_eq!(
        found
            .iter()
            .map(|f| f.cause.split(':').next().unwrap_or_default())
            .collect::<Vec<&str>>(),
        ["clone-block", "clone-block"]
    );
    assert!(found.iter().all(|f| f.message.contains("(6 stmts)")));
}

#[test]
fn two_modules_sharing_a_run_are_one_block_clone() {
    let lib = format!("pub mod other;\npub fn one() {{\n{BODY}}}\n");
    let other = format!("pub fn two() {{\n    setup();\n{BODY}}}\n");
    let found = run_rs_rule(
        "11",
        &[
            ("Cargo.toml", MANIFEST),
            ("src/lib.rs", &lib),
            ("src/other.rs", &other),
        ],
    );

    assert_eq!(
        found
            .iter()
            .map(|f| (f.site.rel.to_string(), f.site.line))
            .collect::<Vec<(String, u32)>>(),
        [
            ("src/lib.rs".to_string(), 3),
            ("src/other.rs".to_string(), 3)
        ]
    );
    assert!(found.iter().all(|f| f.cause == found[0].cause));
}

#[test]
fn a_run_inside_a_match_arm_is_a_block_clone() {
    let tail = "        }\n        _ => {}\n    }\n}\n";
    let arm = |name: &str, head: &str| {
        format!(
            "pub fn {name}(v: Kind) {{\n{head}    match v {{\n        Kind::A => {{\n{BODY}{tail}"
        )
    };
    let src = arm("one", "") + &arm("two", "    setup();\n");
    let found = run_rs_rule("11", &krate(&src));

    assert_eq!(
        found.iter().map(|f| f.site.line).collect::<Vec<u32>>(),
        [4, 18]
    );
    assert!(found.iter().all(|f| f.message.contains("(6 stmts)")));
}

#[test]
fn a_module_path_on_a_call_is_a_name() {
    let qualified = BODY.replace("report(e);", "sink::report(e);");
    let src = format!("pub fn one() {{\n{BODY}}}\npub fn two() {{\n{qualified}}}\n");
    let found = run_rs_rule("11", &krate(&src));

    assert_eq!(
        found.iter().map(|f| f.site.line).collect::<Vec<u32>>(),
        [1, 9]
    );
    assert_eq!(
        found
            .iter()
            .map(|f| f.cause.split(':').next().unwrap_or_default())
            .collect::<Vec<&str>>(),
        ["clone", "clone"]
    );
}

#[test]
fn a_window_of_one_repeated_statement_shape_is_a_table() {
    let row = "    push(1);\n".repeat(6);
    let src = format!("pub fn one() {{\n{row}}}\npub fn two() {{\n    setup();\n{row}}}\n");

    assert!(run_rs_rule("11", &krate(&src)).is_empty());
}

#[test]
fn a_test_member_counts_toward_a_group_but_is_never_reported() {
    let src = format!(
        "pub fn one() {{\n{BODY}}}\n#[cfg(test)]\nmod tests {{\n    #[test]\n    \
         fn two() {{\n{BODY}    }}\n}}\n"
    );
    let found = run_rs_rule("11", &krate(&src));

    assert_eq!(
        found
            .iter()
            .map(|f| f.site.symbol.to_string())
            .collect::<Vec<String>>(),
        ["demo_crate::one"]
    );
    assert!(found[0].message.contains("x2"));
}

#[test]
fn a_nested_fn_is_no_clone_of_the_fn_that_holds_it() {
    let src = format!("pub fn outer() {{\n    fn inner() {{\n{BODY}    }}\n    inner();\n}}\n");

    assert!(run_rs_rule("11", &krate(&src)).is_empty());
}

#[test]
fn a_group_of_two_names_both_owners() {
    let src = format!("pub fn one() {{\n{BODY}}}\npub fn two() {{\n{BODY}}}\n");
    let found = run_rs_rule("11", &krate(&src));

    // each owner with its line, so a reader opens the other copy directly
    assert!(
        found[0]
            .message
            .starts_with("structural clone x2: demo_crate::one L1, demo_crate::two L"),
        "{}",
        found[0].message
    );
    assert_eq!(found[0].salience, 2.0);
    assert_eq!(causes(&found)[0], causes(&found)[1]);
}
