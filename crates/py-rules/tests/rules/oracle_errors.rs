//! #58 checker-error (`tests/rules/test_oracle_rules.py`,
//! `TestRule58CheckerError`): the checker's own verdicts, forwarded. Every
//! case builds a real shim at a mini repo.

use std::collections::BTreeSet;

use camino::Utf8Path;
use sightline_core::findings::{Finding, Tier};
use sightline_py_provers::oracle::Oracle;
use sightline_testkit::{build, run_rule_on};

/// A mini repo whose provers carry a real checker; the shim closes with the
/// stack.
fn run_58(files: &[(&str, &str)]) -> Vec<Finding> {
    let (dir, mut stack) = build(files);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let roots = stack.facts().import_roots.clone();
    stack.provers.oracle =
        Some(Oracle::new(root, &[], &roots, None).expect("the checker builds on a mini repo"));
    let found = run_rule_on("58", &stack);
    stack.provers.close();
    found
}

#[test]
fn the_checkers_verdicts_are_reported_ungrounded() {
    let found = run_58(&[(
        "m.py",
        concat!(
            "import os\n",
            "class Base:\n",
            "    def run(self, a: int) -> int:\n",
            "        return a\n",
            "class Child(Base):\n",
            "    def run(self, a: str) -> str:\n",
            "        return a\n",
            "def attr() -> None:\n",
            "    print(os.no_such_member)\n",
            "def bad() -> int:\n",
            "    return 'text'\n",
        ),
    )]);
    // `os.no_such_member` is an unresolved-attribute: an arm cut at the judge
    // wave (7 real / 7 fp), so `m.attr` proves nothing
    let causes: BTreeSet<&str> = found.iter().map(|f| f.cause.as_str()).collect();
    assert_eq!(
        causes,
        BTreeSet::from([
            "invalid-method-override:m:6:8",
            "invalid-return-type:m:11:11"
        ])
    );
    let symbols: BTreeSet<&str> = found.iter().map(|f| &*f.site.symbol).collect();
    assert_eq!(symbols, BTreeSet::from(["m.Child.run", "m.bad"]));
    // the claim is the checker's, never the repo's: never proved
    assert!(found.iter().all(|f| f.tier() == Tier::Heuristic));
    assert!(found.iter().all(|f| {
        let prefix = f.cause.split(':').next().expect("a cause has a prefix");
        f.message.starts_with(&format!("{prefix}: "))
    }));
}

/// The same defect in both files; the header already reports the missing
/// module, and it explains everything the checker says about that file.
#[test]
fn a_module_an_unresolved_import_blinded_stays_silent() {
    let found = run_58(&[
        ("seen.py", "def bad() -> int:\n    return 'text'\n"),
        (
            "blind.py",
            concat!(
                "import totally_absent_package\n",
                "def bad() -> int:\n",
                "    return 'text'\n",
            ),
        ),
    ]);
    let rels: Vec<&str> = found.iter().map(|f| &*f.site.rel).collect();
    assert_eq!(rels, ["seen.py"]);
}

/// The argument-type arm was cut at the close (pooled 8/32: duck-typed
/// stand-ins, `**` splats, TypedDict narrowing); the return-type arm beside it
/// still fires.
#[test]
fn an_argument_type_error_is_the_cut_arm() {
    let found = run_58(&[(
        "m.py",
        concat!(
            "def g(a: int) -> None:\n",
            "    pass\n",
            "g('s')\n",
            "def h() -> int:\n",
            "    return 't'\n",
        ),
    )]);
    let rows: Vec<(u32, &str)> = found
        .iter()
        .map(|f| {
            (
                f.site.line,
                f.cause.split(':').next().expect("a cause has a prefix"),
            )
        })
        .collect();
    assert_eq!(rows, [(5, "invalid-return-type")]);
}

/// The judged UnboundLocalError shape, cut at 0 / 20 (judge wave 20260848):
/// the shim still raises the read at warning severity, #58 forwards none.
#[test]
fn a_name_bound_in_only_some_arms_stays_silent() {
    let found = run_58(&[(
        "m.py",
        concat!(
            "def pick(spec: str) -> str:\n",
            "    if spec == 'a':\n",
            "        field = 'a'\n",
            "    elif spec == 'b':\n",
            "        field = 'b'\n",
            "    return field\n",
            "def covered(spec: str) -> str:\n",
            "    if spec == 'a':\n",
            "        name = 'a'\n",
            "    else:\n",
            "        name = 'b'\n",
            "    return name\n",
        ),
    )]);
    assert!(found.is_empty(), "{:?}", found);
}

/// The repos scope their own checkers out of their test trees, and a line the
/// repo already silenced is not news.
#[test]
fn a_test_path_and_a_silenced_line_stay_out() {
    let found = run_58(&[
        (
            "m.py",
            concat!(
                "def g() -> int:\n    return 's'  # type: ignore[return-value]\n",
                "def h() -> int:\n",
                "    return 't'\n",
            ),
        ),
        ("tests/test_m.py", "def k() -> int:\n    return 'u'\n"),
    ]);
    let rows: Vec<(&str, u32)> = found.iter().map(|f| (&*f.site.rel, f.site.line)).collect();
    assert_eq!(rows, [("m.py", 4)]);
}

/// ROFL's `session.py:184`: the checker anchors the verdict at the statement's
/// first line, the repo silenced it on the closing one. The twin's pragma is
/// on the line after the return.
#[test]
fn a_pragma_on_a_multi_line_returns_closing_line_silences_it() {
    let found = run_58(&[(
        "m.py",
        concat!(
            "def g() -> int:\n    return (\n        's'\n    )  # type: ignore[return-value]\n",
            "def h() -> int:\n    return (\n        't'\n    )\n    # type: ignore[return-value]\n",
        ),
    )]);
    // the checker anchors at the returned expression
    let rows: Vec<(&str, u32)> = found.iter().map(|f| (&*f.site.rel, f.site.line)).collect();
    assert_eq!(rows, [("m.py", 7)]);
}

#[test]
fn unresolved_reference_is_the_headers_population_not_this_rules() {
    let found = run_58(&[("m.py", "def f() -> int:\n    return never_defined_name\n")]);
    assert!(found.is_empty(), "{:?}", found);
}
