//! The `facts` verb's printout (`describe.py`). The Python tree has no test
//! file of its own for it, so these pin the shape phase 9's verb prints: one
//! line per accessor, a module qname answering for its whole file, and an
//! unknown qname answering with the nearest names.

use sightline_core::lang::Stack;
use sightline_testkit::{build, run_rule};

#[test]
fn a_symbol_prints_one_line_per_accessor_and_its_own_findings() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def helper(x):\n    return x\n\n\ndef use(n: int) -> int:\n    return helper(n)\n",
    )]);
    let findings = run_rule(
        "1",
        &[(
            "m.py",
            "def helper(x):\n    return x\n\n\ndef use(n: int) -> int:\n    return helper(n)\n",
        )],
    );
    let out = stack
        .describe(&findings, "m.helper")
        .expect("the symbol is indexed");
    let heads: Vec<&str> = out
        .lines()
        .map(|l| l.split(':').next().unwrap_or(""))
        .collect();
    assert_eq!(
        &heads[..7],
        [
            "m.helper  function  m.py L1-2",
            "callers prod",
            "callers test",
            "effects",
            "closed world",
            "hot",
            "liveness",
        ]
    );
    assert!(out.contains("callers prod: 1 sites, from m.use"));
    assert!(out.contains("fixes:        0 verified"));
    assert!(out.ends_with('\n'));
}

#[test]
fn a_module_qname_answers_for_its_whole_file() {
    let (_dir, stack) = build(&[("m.py", "X = 1\nY = 2\n")]);
    let out = stack.describe(&[], "m").expect("the module is indexed");
    assert!(out.starts_with("m  module  m.py L1-2\n"));
    assert!(out.contains("findings:     0"));
}

#[test]
fn an_unknown_qname_answers_with_the_nearest_names() {
    let (_dir, stack) = build(&[("m.py", "def helper(x):\n    return x\n")]);
    let near = stack.describe(&[], "m.helpr").expect_err("no such symbol");
    assert!(near.contains(&"m.helper".to_string()), "{near:?}");
}
