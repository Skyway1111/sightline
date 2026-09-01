//! `provers/grounding.py`: is an oracle verdict grounded in a claim the repo
//! wrote? Every predicate here reads a hand-built `OracleDiag`, so none of
//! them needs a checker.

use std::collections::HashSet;

use sightline_core::findings::Rel;
use sightline_py_provers::grounding::{
    broken_declaration, container_shape_check, grounding, none_default_lie,
};
use sightline_py_provers::oracle::OracleDiag;
use sightline_testkit::build;

fn diag(line: u32, col: u32, message: &str) -> OracleDiag {
    at_rule("reportUnnecessaryIsInstance", line, col, message)
}

fn at_rule(rule: &str, line: u32, col: u32, message: &str) -> OracleDiag {
    OracleDiag {
        rel: "m.py".into(),
        line,
        col,
        rule: rule.to_string(),
        message: message.to_string(),
        severity: "warning".to_string(),
    }
}

/// `tests/provers/test_oracle.py:test_grounding_annotated_vs_not`.
#[test]
fn an_annotated_param_grounds_and_an_inferred_local_does_not() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def g(x: str) -> bool:\n    return isinstance(x, str)\n\
         def u(v):\n    s = 'lit'\n    return isinstance(s, str)\n",
    )]);
    let facts = stack.facts();
    let empty = stack.provers.arg_types(facts);
    let rejected = HashSet::new();

    assert!(grounding(
        &diag(2, 11, ""),
        facts,
        &stack.provers,
        empty,
        &rejected
    ));
    assert!(!grounding(
        &diag(5, 11, ""),
        facts,
        &stack.provers,
        empty,
        &rejected
    ));
}

/// `tests/provers/test_oracle.py:test_never_operand_never_grounds`: the fork
/// reports `"Never" is always an instance of "X"` where its own narrowing
/// emptied the type, a checker artifact rather than a claim.
#[test]
fn a_never_operand_never_grounds() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f(x: int) -> bool:\n    return isinstance(x, int)\n",
    )]);
    let facts = stack.facts();
    let empty = stack.provers.arg_types(facts);
    let rejected = HashSet::new();

    let declared = diag(
        2,
        11,
        "Unnecessary isinstance call; \"int\" is always an instance of \"int\"",
    );
    let never = diag(
        2,
        11,
        "Unnecessary isinstance call; \"Never\" is always an instance of \"int\"",
    );

    assert!(grounding(
        &declared,
        facts,
        &stack.provers,
        empty,
        &rejected
    ));
    assert!(!grounding(&never, facts, &stack.provers, empty, &rejected));
}

/// The ABC carve-out beside it: `isinstance(True, Integral)` is True, so
/// pyright's nominal no-overlap claim over an `ABCMeta.register()` family is
/// unsound (`_VIRTUAL_ABCS`).
#[test]
fn an_abc_registered_class_never_grounds() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f(x: int) -> bool:\n    return isinstance(x, int)\n",
    )]);
    let facts = stack.facts();
    let empty = stack.provers.arg_types(facts);
    let rejected = HashSet::new();

    let abc = diag(
        2,
        11,
        "Unnecessary isinstance call; \"int\" is always an instance of \"Integral\"",
    );

    assert!(!grounding(&abc, facts, &stack.provers, empty, &rejected));
}

/// Module level is ungrounded: there is no signature to read.
#[test]
fn module_level_never_grounds() {
    let (_dir, stack) = build(&[("m.py", "x: int = 1\nprint(isinstance(x, int))\n")]);
    let facts = stack.facts();
    let empty = stack.provers.arg_types(facts);

    assert!(!grounding(
        &diag(2, 6, ""),
        facts,
        &stack.provers,
        empty,
        &HashSet::new()
    ));
}

/// `none_default_lie`: the def contradicts its own declaration, by a literal
/// `None` default or by the fallback its body supplies. Rule #2 skips both;
/// #1 owns the default's half. Nothing else pins this predicate until phase
/// 5's `tests/rules/test_oracle_rules.py`.
#[test]
fn a_none_default_and_a_supplied_fallback_are_both_the_lie() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f(x: str = None) -> bool:\n    return x is None\n\
         def g(x: str) -> str:\n    if x is None:\n        x = 'd'\n    return x\n\
         def h(x: str) -> str:\n    if x is None:\n        raise ValueError(x)\n    return x\n",
    )]);
    let facts = stack.facts();
    let cmp = "reportUnnecessaryComparison";

    assert!(none_default_lie(
        &at_rule(cmp, 2, 11, ""),
        facts,
        &stack.provers
    ));
    assert!(none_default_lie(
        &at_rule(cmp, 4, 7, ""),
        facts,
        &stack.provers
    ));
    // a branch that raises instead is the redundancy #2 is after
    assert!(!none_default_lie(
        &at_rule(cmp, 8, 7, ""),
        facts,
        &stack.provers
    ));
}

/// `container_shape_check`: an `isinstance` against bare container classes is
/// what makes the annotation true, not a duplicate of it.
#[test]
fn an_isinstance_against_bare_containers_is_a_shape_check() {
    let (_dir, stack) = build(&[(
        "m.py",
        "class C:\n    pass\n\
         def f(d: dict) -> bool:\n    return isinstance(d, dict)\n\
         def g(d: dict) -> bool:\n    return isinstance(d, (list, dict))\n\
         def h(d: dict) -> bool:\n    return isinstance(d, C)\n",
    )]);
    let facts = stack.facts();

    assert!(container_shape_check(&diag(4, 11, ""), facts));
    assert!(container_shape_check(&diag(6, 11, ""), facts));
    assert!(!container_shape_check(&diag(8, 11, ""), facts));
}

/// `broken_declaration`: the tested name is rebound on a path to the check
/// and one of those rebindings is an assignment the checker rejected, so the
/// verdict belongs to the declaration the body outgrew.
#[test]
fn a_rejected_rebinding_breaks_the_declaration() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f(x: int) -> bool:\n    x = 'a'\n    return isinstance(x, int)\n",
    )]);
    let facts = stack.facts();
    let check = diag(3, 11, "");

    let rejected: HashSet<(Rel, u32)> = [(Rel::from("m.py"), 2)].into_iter().collect();
    assert!(broken_declaration(&check, facts, &stack.provers, &rejected));
    // a valid rebinding keeps its verdict
    assert!(!broken_declaration(
        &check,
        facts,
        &stack.provers,
        &HashSet::new()
    ));
}
