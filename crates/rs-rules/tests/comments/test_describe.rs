//! `sightline facts <qname>` over a Rust symbol (the two `test_describe_*` of
//! `tests/rs/test_provers.py`).

use sightline_rs_rules::describe::describe;
use sightline_testkit::build_rs;

const BODY: &str = "pub fn run(n: i32) -> i32 {\n\
                    \x20   helper(n);\n\
                    \x20   println!(\"{}\", n);\n\
                    \x20   let double = |x: i32| x * 2 + 1;\n\
                    \x20   unsafe { raw(n); }\n\
                    \x20   #[allow(clippy::needless_return)]\n\
                    \x20   let out = double(n);\n\
                    \x20   out\n\
                    }\n\
                    pub fn helper(n: i32) -> i32 { n }\n";

#[test]
fn describe_prints_one_symbols_record() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", BODY)]);
    let provers = stack.provers();

    let out =
        describe(provers.facts(), &provers, &[], "demo_crate::run").expect("the symbol is indexed");

    assert!(out.starts_with("demo_crate::run  function  src/lib.rs L1-9\n"));
    assert!(out.contains("visibility:   pub"));
    assert!(out.contains("1 closure(s), 1 unsafe block(s)"));
    assert!(out.contains("findings:     0"));
}

#[test]
fn describe_names_the_nearest_symbol_for_a_typo() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", BODY)]);
    let provers = stack.provers();

    let near =
        describe(provers.facts(), &provers, &[], "demo_crate::runn").expect_err("no such symbol");

    assert_eq!(near.first().map(String::as_str), Some("demo_crate::run"));
}
