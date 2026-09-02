//! The oracle-established argument types at call sites.

use crate::oracle_fixture;

use ruff_python_ast::{Expr, ExprCall};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_provers::argtypes::{Arg, arg_expr, prod_args};
use sightline_testkit::build;

/// `arg_expr` reads the call, not the oracle: a splat before the slot is
/// unknowable, a filled slot is its expression, an empty one the default.
#[test]
fn arg_expr_reads_positions_keywords_and_splats() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f(a, b=1): pass\nf(1, 2)\nf(1)\nf(*xs, 2)\nf(1, b=3)\nf(1, **kw)\n",
    )]);
    let facts = stack.facts();
    let module = &facts.modules["m"];
    let calls: Vec<&ExprCall> = module
        .nodes(&[Kind::Call], None, false)
        .into_iter()
        .filter_map(|at| match module.nodes[at as usize] {
            Cn::Expr(Expr::Call(c)) => Some(c),
            _ => None,
        })
        .collect();

    assert!(matches!(arg_expr(calls[0], 1, "b"), Arg::Expr(_)));
    assert_eq!(arg_expr(calls[1], 1, "b"), Arg::Omitted);
    assert_eq!(arg_expr(calls[2], 1, "b"), Arg::Unknown);
    assert!(matches!(arg_expr(calls[3], 1, "b"), Arg::Expr(_)));
    assert_eq!(arg_expr(calls[4], 1, "b"), Arg::Unknown);
    // no positional slot for the param: the sentinel index (R19)
    assert_eq!(arg_expr(calls[1], usize::MAX, "b"), Arg::Omitted);
}

/// `prod_args` reads one enumeration over the bare graph, test callers
/// dropped (#14 reads it).
#[test]
fn prod_args_drops_test_callers() {
    let (_dir, stack) = build(&[
        ("m.py", "def f(a):\n    return a\n"),
        ("app.py", "from m import f\n\nf(1)\n"),
        ("tests/test_m.py", "from m import f\n\nf(2)\n"),
    ]);
    let facts = stack.facts();
    let calls = stack.provers.calls(facts);

    let args = prod_args(facts, calls, &facts.symbols["m.f"], "a");

    assert_eq!(args.len(), 1);
    assert!(matches!(args[0], Arg::Expr(_)));
}

/// The `arg_types` half of a run with no oracle: the accessor answers
/// empty, never absent. `unresolved`, `diagnostics` and `errors` are the
/// oracle tests'.
#[test]
fn arg_types_answer_empty_without_an_oracle() {
    let (_dir, stack) = build(&[(
        "m.py",
        "import absent_xyz\n\ndef f(a):\n    return absent_xyz.g(a)\n",
    )]);
    let facts = stack.facts();

    assert!(
        stack
            .provers
            .arg_types(facts)
            .for_param("m.f", "a")
            .is_none()
    );
    assert_eq!(
        stack.provers.arg_types(facts).dump_rows(facts),
        serde_json::json!([])
    );
}

/// One batch answers every closed-world call site of an unannotated param:
/// the default row first, then one row per call.
#[test]
fn every_call_site_of_an_unannotated_param_gets_a_type() {
    let (dir, mut stack) = build(&[(
        "m.py",
        "def _scale(n, factor=2):\n    return n * factor\n\
         def use(x: int) -> int:\n    return _scale(x) + _scale(3)\n",
    )]);
    oracle_fixture::attach(&dir, &mut stack);
    let facts = stack.facts();

    let observed = stack
        .provers
        .arg_types(facts)
        .for_param("m._scale", "n")
        .expect("the closed-world callee's param");

    // values pinned from a probe of `arg_types` over this fixture
    let spelled: Vec<(bool, Option<&str>)> = observed
        .iter()
        .map(|o| (o.call.is_some(), o.ty.as_deref()))
        .collect();
    assert_eq!(spelled, [(true, Some("int")), (true, Some("Literal[3]"))]);

    // the default is the first row of a defaulted param, with no call, and
    // every omitting call site reads it
    let defaulted = stack
        .provers
        .arg_types(facts)
        .for_param("m._scale", "factor")
        .expect("the defaulted param");
    let spelled: Vec<(bool, Option<&str>)> = defaulted
        .iter()
        .map(|o| (o.call.is_some(), o.ty.as_deref()))
        .collect();
    assert_eq!(
        spelled,
        [
            (false, Some("Literal[2]")),
            (true, Some("Literal[2]")),
            (true, Some("Literal[2]")),
        ]
    );
}
