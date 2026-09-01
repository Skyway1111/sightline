//! `provers/liveness.py`: name-level liveness and what defeats it.

use std::collections::BTreeSet;

use sightline_core::findings::Qname;
use sightline_py_provers::liveness::*;
use sightline_testkit::build;

#[test]
fn referenced_only_from_tests_names_the_test_modules() {
    let (_dir, stack) = build(&[
        (
            "m.py",
            "def only_tested():\n    return 4\ndef dead():\n    return 5\n\
             def used():\n    return 6\ndef run():\n    return used()\n",
        ),
        ("tests/__init__.py", ""),
        (
            "tests/test_m.py",
            "from m import only_tested, used\ndef test_it():\n    assert only_tested() and used()\n",
        ),
        (
            "tests/test_more.py",
            "import m\ndef test_again():\n    assert m.only_tested()\n",
        ),
    ]);
    let facts = stack.facts();
    let live = live_names(facts);
    let query = |q: &str, n: &str| referenced_only_from_tests(facts, q, n, &live);
    assert_eq!(
        query("m.only_tested", "only_tested"),
        BTreeSet::from([Qname::from("tests.test_m"), Qname::from("tests.test_more")])
    );
    // one prod reference is a prod reach
    assert!(query("m.used", "used").is_empty());
    // nothing reaches it: #32's, not #56's
    assert!(query("m.dead", "dead").is_empty());
    assert!(!referenced_outside(facts, "m.dead", "dead", &live));
    assert!(referenced_outside(
        facts,
        "m.only_tested",
        "only_tested",
        &live
    ));
}

#[test]
fn a_self_reference_and_an_entry_point_root_are_not_test_reach() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/cli.py",
            "def recurse(n):\n    return recurse(n - 1)\ndef main():\n    return 1\n",
        ),
        (
            "pyproject.toml",
            "[project]\nname = \"pkg\"\n\n[project.scripts]\nx = \"pkg.cli:main\"\n",
        ),
        ("tests/__init__.py", ""),
        (
            "tests/test_cli.py",
            "from pkg.cli import main, recurse\ndef test_it():\n    assert main() and recurse(1)\n",
        ),
    ]);
    let facts = stack.facts();
    let live = live_names(facts);
    assert_eq!(
        referenced_only_from_tests(facts, "pkg.cli.recurse", "recurse", &live),
        BTreeSet::from([Qname::from("tests.test_cli")])
    );
    assert!(referenced_only_from_tests(facts, "pkg.cli.main", "main", &live).is_empty());
}

#[test]
fn a_prod_string_table_names_what_no_reference_reaches() {
    let (_dir, stack) = build(&[
        ("m.py", "KEYS = (\"damageshare\", \"kills\")\n"),
        ("tests/__init__.py", ""),
        ("tests/test_m.py", "OTHER = (\"only_in_a_test\",)\n"),
    ]);
    let unseen = unseen_names(stack.facts());
    assert!(unseen.named("damageshare"));
    assert!(!unseen.named("only_in_a_test"));
}
