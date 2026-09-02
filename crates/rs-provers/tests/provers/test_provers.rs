//! What `RsProvers` answers. Body
//! queries, the trait-impl index, module literals and `#[allow]`s, the Rust
//! complexity classification over the neutral scorer, and the digest
//! sequences the neutral repeat mining reads.
//!
//! `describe` is covered next door: the symbols record it prints and the
//! nearest symbol it names for a typo are `rs-rules/tests/comments`.

use std::collections::BTreeSet;

use sightline_core::clones::{Seq, repeats};
use sightline_rs_provers::{RsAllow, parses_as_code};
use sightline_testkit::build_rs;

const BODY: &str = "pub fn run(n: i32) -> i32 {
    helper(n);
    println!(\"{}\", n);
    let double = |x: i32| x * 2 + 1;
    unsafe { raw(n); }
    #[allow(clippy::needless_return)]
    let out = double(n);
    out
}
pub fn helper(n: i32) -> i32 { n }
";

#[test]
fn a_body_answers_calls_macros_unsafe_closures_and_allows() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", BODY)]);
    let provers = stack.provers();
    let body = provers.body("demo_crate::run");

    let names: Vec<&str> = body.calls.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["helper", "raw", "double"]);
    let macros: Vec<&str> = body.macros.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(macros, ["println"]);
    assert_eq!(body.unsafe_blocks.len(), 1);
    assert_eq!(body.closures.len(), 1);
    assert_eq!(body.closures[0].line, 4);
    assert_eq!(body.allows, ["clippy::needless_return"]);
}

/// `check::<Scheme>(b)` calls `check`; the type argument is the call's.
#[test]
fn a_turbofish_call_is_named_by_its_function() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "pub fn run(b: &[u8]) { check::<Scheme>(b); }\n",
    )]);
    let provers = stack.provers();

    let call = &provers.body("demo_crate::run").calls[0];
    assert_eq!((call.name.as_str(), call.path.as_str()), ("check", "check"));
}

#[test]
fn a_body_stops_at_a_nested_fn() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "pub fn outer() {\n    fn inner() { deep(); }\n    near();\n}\n",
    )]);
    let provers = stack.provers();

    let outer: Vec<&str> = provers
        .body("demo_crate::outer")
        .calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(outer, ["near"]);
    let inner: Vec<&str> = provers
        .body("demo_crate::outer::inner")
        .calls
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(inner, ["deep"]);
}

#[test]
fn module_allows_read_the_whole_file() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "#![allow(dead_code)]\npub const A: &str = \"top\";\n\
         #[allow(unused, clippy::all)]\npub fn f() { let s = \"inside\"; }\n\
         #[cfg(feature = \"serde\")]\npub const B: &str = \"flagged\";\n\
         cfg_feature! {\n#![feature = \"quinn\"]\npub use quinn;\n}\n",
    )]);
    let provers = stack.provers();

    assert_eq!(
        provers.allows()["demo_crate"],
        [
            RsAllow {
                names: vec!["dead_code".to_string()],
                line: 1
            },
            RsAllow {
                names: vec!["unused".to_string(), "clippy::all".to_string()],
                line: 3
            },
        ]
    );
}

#[test]
fn the_trait_impl_index_lists_every_implementing_type() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "pub trait T { fn go(&self); }\npub struct A;\npub struct B;\n\
         impl T for A { fn go(&self) {} }\nimpl T for B { fn go(&self) {} }\n\
         impl A { fn own(&self) {} }\n",
    )]);
    let provers = stack.provers();

    assert_eq!(provers.trait_impls().len(), 1);
    assert_eq!(
        provers.trait_impls()["T"],
        ["demo_crate::A", "demo_crate::B"]
    );
}

// --- complexity -------------------------------------------------------------

const FLAT: &str = "pub fn f(a: i32) -> i32 { a + 1 }\n";
// if(1) + `&&` run(1) + else(1) + match(1) + for(1) + while one in(2) = 7
const NESTED: &str = "pub fn f(a: i32) -> i32 {
    if a > 1 && a < 9 { return 1 } else { return 2 }
    match a { 0 => {}, _ => {} }
    for i in 0..a { while i > 0 { return 3 } }
    0
}
";

#[test]
fn the_rust_classification_scores_branches_loops_and_boolean_runs() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", FLAT)]);
    assert_eq!(stack.provers().complexity("demo_crate::f"), 0);

    let (_dir, stack) = build_rs(&[("src/lib.rs", NESTED)]);
    assert_eq!(stack.provers().complexity("demo_crate::f"), 7);
}

#[test]
fn an_else_if_is_flat_and_a_recursive_call_counts() {
    // if(1) + else-if(1) + the recursive call(1)
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "pub fn f(a: i32) -> i32 {\n    if a > 1 { 1 } else if a < 0 { f(a + 1) } else { 0 }\n}\n",
    )]);

    // the trailing `else` is a fourth
    assert_eq!(stack.provers().complexity("demo_crate::f"), 4);
}

// --- clones -----------------------------------------------------------------

const CLONE_BODY: &str = "    let a = load(1);
    let b = load(2);
    let c = a + b;
    let d = c * 2;
    let e = d - 1;
    report(e);
";

/// `two` opens with a statement of its own, so the shared run is a block
/// repeat and not the whole-body duplicate the function arm owns.
fn clones() -> String {
    format!("pub fn one() {{\n{CLONE_BODY}}}\npub fn two() {{\n    setup();\n{CLONE_BODY}}}\n")
}

#[test]
fn the_mined_sequences_feed_the_neutral_repeat_mining() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", &clones())]);
    let provers = stack.provers();
    let rows = provers.clone_sequences();
    let seqs: Vec<Seq> = rows.iter().map(|r| r.seq.clone()).collect();
    let found = repeats(&seqs);

    let owners: BTreeSet<&str> = rows.iter().map(|r| &*r.owner).collect();
    assert_eq!(
        owners,
        BTreeSet::from(["demo_crate::one", "demo_crate::two"])
    );
    assert!(rows.iter().all(|r| r.seq.top && r.seq.prod));
    let lengths: Vec<usize> = found.iter().map(|r| r.length).collect();
    assert_eq!(lengths, [6]);
    let members: BTreeSet<&str> = found[0]
        .runs
        .iter()
        .map(|(s, _)| &*rows[*s].owner)
        .collect();
    assert_eq!(
        members,
        BTreeSet::from(["demo_crate::one", "demo_crate::two"])
    );
}

#[test]
fn two_bodies_of_one_shape_share_a_function_digest() {
    let twins = format!("pub fn one() {{\n{CLONE_BODY}}}\npub fn two() {{\n{CLONE_BODY}}}\n");
    let (_dir, stack) = build_rs(&[("src/lib.rs", &twins)]);
    let provers = stack.provers();
    let digests = provers.function_digests();

    assert_eq!(digests["demo_crate::one"], digests["demo_crate::two"]);
}

#[test]
fn a_digest_is_blind_to_names_and_literals() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "pub fn a() { let x = 1; step(x); step(x); step(x); step(x); }\n\
         pub fn b() { let y = 9; walk(y); walk(y); walk(y); walk(y); }\n\
         pub fn c() { while true { drop(0); } }\n",
    )]);
    let provers = stack.provers();
    let digests = provers.function_digests();

    assert_eq!(digests["demo_crate::a"], digests["demo_crate::b"]);
    assert_ne!(
        digests.get("demo_crate::c"),
        Some(&digests["demo_crate::a"])
    );
}

#[test]
fn a_digest_still_reads_the_operators_and_never_the_comments() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "pub fn a(x: i32) -> i32 { let z = x + x; f(z); f(z); f(z); z }\n\
         pub fn b(x: i32) -> i32 { let z = x - x; f(z); f(z); f(z); z }\n\
         pub fn c(x: i32) -> i32 { let z = x + x; // why\n  f(z); f(z); f(z); z }\n",
    )]);
    let provers = stack.provers();
    let digests = provers.function_digests();

    assert_ne!(digests["demo_crate::a"], digests["demo_crate::b"]);
    assert_eq!(digests["demo_crate::a"], digests["demo_crate::c"]);
}

#[test]
fn a_closure_key_reads_the_names_the_blind_digest_drops() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "pub fn f() {\n\
         \x20   let a = |p: &Row, q: &Row| p.rank().cmp(&q.rank());\n\
         \x20   let b = |m: &Row, n: &Row| m.rank().cmp(&n.rank());\n\
         \x20   let c = |p: &Row, q: &Row| p.seed().cmp(&q.seed());\n\
         \x20   take(a, b, c);\n}\n",
    )]);
    let provers = stack.provers();
    let body = provers.body("demo_crate::f");
    let [a, b, c] = &body.closures[..] else {
        panic!("three closures");
    };

    // blind: one shape
    assert_eq!(a.digest, b.digest);
    assert_eq!(b.digest, c.digest);
    // the field sorted on is the fact
    assert_eq!(a.key, b.key);
    assert_ne!(a.key, c.key);
}

// --- comment blocks ---------------------------------------------------------

#[test]
fn a_comment_run_is_one_block_and_says_whether_rust_reads_it() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "// let x = compute(1);\n// drop(x);\npub fn f() {}\n\
         // just a note about f\npub fn g() {}\n",
    )]);
    let provers = stack.provers();
    let rows: Vec<(u32, bool)> = provers.comment_blocks()["demo_crate"]
        .iter()
        .map(|b| (b.start, b.code()))
        .collect();

    assert_eq!(rows, [(1, true), (4, false)]);
}

#[test]
fn a_doc_comment_is_never_a_commented_out_code_block() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", "/// let x = compute(1);\npub fn f() {}\n")]);
    let provers = stack.provers();

    assert!(provers.comment_blocks()["demo_crate"].is_empty());
}

#[test]
fn parses_as_code_needs_an_item_or_a_statement() {
    assert!(parses_as_code(&["fn f() {}"]));
    assert!(parses_as_code(&["let x = 1;"]));
    assert!(!parses_as_code(&["the walk stops here"]));
    assert!(!parses_as_code(&[""]));
}

// --- the header -------------------------------------------------------------

#[test]
fn provenance_names_the_parser_and_the_parse_errors() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", "pub fn f() {}\n")]);
    let provers = stack.provers();
    let prov = provers.provenance(stack.facts());
    let rs = &prov["rs"];

    assert!(
        rs["tree_sitter"]
            .as_str()
            .is_some_and(|v| v.starts_with("0.26"))
    );
    assert!(
        rs["tree_sitter_rust"]
            .as_str()
            .is_some_and(|v| v.starts_with("0.24"))
    );
    assert_eq!(rs["parse_errors"], 0);
}
