//! `provers/comments.py`'s AST half: comment runs as a rule reads them.

use sightline_py_provers::comments::*;
use sightline_testkit::build;

#[test]
fn a_run_of_whole_line_comments_groups_by_consecutive_lines() {
    let (_dir, stack) = build(&[(
        "m.py",
        "# one\n# two\nx = 1  # trailing\n\n# three\ny = 2\n",
    )]);
    let facts = stack.facts();
    let module = &facts.modules["m"];
    assert_eq!(
        comment_blocks(module),
        [(1, vec!["# one", "# two"]), (5, vec!["# three"])]
    );
}

#[test]
fn a_first_screen_of_prose_documents_the_module() {
    let (_dir, prose) = build(&[("m.py", "# What this module is\nx = 1\n")]);
    assert!(documents_module(&prose.facts().modules["m"]));
    let (_dir, bar) = build(&[("m.py", "# -------\nx = 1\n")]);
    assert!(!documents_module(&bar.facts().modules["m"]));
    let (_dir, late) = build(&[("m.py", "x = 1\n# What this module is\n")]);
    assert!(!documents_module(&late.facts().modules["m"]));
}

#[test]
fn a_commented_out_run_parses_as_code_only_when_it_does_something() {
    assert!(parses_as_code(&["# x = compute(1)"]));
    assert!(parses_as_code(&["# run(x)"]));
    assert!(parses_as_code(&["#     elif x:", "#         return 1"]));
    assert!(parses_as_code(&["# else:", "#     return 1"]));
    assert!(!parses_as_code(&["# what this does"]));
    assert!(!parses_as_code(&["# x"]));
    assert!(!parses_as_code(&["# def f(:"]));
}

#[test]
fn a_def_declares_no_raise_in_its_docstring_or_a_comment_in_its_span() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def a():\n    \"\"\"Must not raise.\"\"\"\n    pass\n\
         def b():\n    walk()  # no sink; must never raise\n\
         def c():\n    pass\n",
    )]);
    let facts = stack.facts();
    let module = &facts.modules["m"];
    let at = |q: &str| facts.symbols[q].node;
    assert!(declares_no_raise(module, at("m.a")));
    assert!(declares_no_raise(module, at("m.b")));
    assert!(!declares_no_raise(module, at("m.c")));
}
