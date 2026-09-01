//! `tests/test_fixes.py`, the #33 half and the fold that owns a lift's def:
//! a verified splice rides its finding as a `Fix`, a vetoed one keeps the
//! finding and drops only the patch. The #32, #35, #39 and #48 splices are
//! judged in their own rule files; what is here needs `emit::attach_fixes`.

use std::collections::HashMap;

use camino::Utf8Path;
use sightline_core::findings::{Engine, Finding, Fix, SpanEdit};
use sightline_py_provers::oracle::Oracle;
use sightline_py_rules::emit;
use sightline_testkit::{PyStack, build, run_rule_on};
use tempfile::TempDir;

const FUTURE: &str = "from __future__ import annotations\n";
const LIES: &str = "def lies(x: int) -> int:\n    if x:\n        return 1\n    return None\n";
/// A caller that accepts anything: the widening never breaks it.
const TOLERANT: &str = "def use(x: int) -> object:\n    return lies(x)\n";

fn edit(line: u32, col_start: u32, col_end: u32, text: &str) -> SpanEdit {
    SpanEdit {
        line,
        col_start,
        col_end,
        text: text.to_string(),
    }
}

fn with_oracle(files: &[(&str, &str)]) -> (TempDir, PyStack) {
    let (dir, mut stack) = build(files);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let import_roots = stack.facts().import_roots.clone();
    stack.provers.oracle =
        Some(Oracle::new(root, &[], &import_roots, None).expect("an in-process checker"));
    (dir, stack)
}

/// `_by_cause`: one rule's findings over an inline repo the checker also
/// sees, run through the fix table and keyed by cause.
fn by_cause(files: &[(&str, &str)], rule: &str) -> HashMap<String, Finding> {
    let (_dir, mut stack) = with_oracle(files);
    let findings = run_rule_on(rule, &stack);
    let patched = emit::attach_fixes(findings, stack.facts(), &stack.provers);
    stack.provers.close();
    patched.into_iter().map(|f| (f.cause.clone(), f)).collect()
}

fn fix_of<'a>(by: &'a HashMap<String, Finding>, cause: &str) -> &'a Fix {
    by.get(cause)
        .unwrap_or_else(|| panic!("no finding caused by {cause}"))
        .fix
        .as_ref()
        .unwrap_or_else(|| panic!("{cause} holds no fix"))
}

#[test]
fn a_none_path_lie_widens_when_callers_tolerate_none() {
    let src = format!("{FUTURE}{LIES}def use(x: int) -> int | None:\n    return lies(x)\n");
    let by = by_cause(&[("m.py", &src)], "33");
    let f = &by["lying-return:m.lies"];
    assert_eq!(
        f.fix.as_ref().expect("a fix").edits,
        [edit(2, 23, 23, " | None")]
    );
    // the claim's tier is untouched by the patch
    assert_eq!(f.engine(), Engine::Ast);
}

#[test]
fn a_none_path_fix_is_vetoed_by_a_caller_relying_on_the_lie() {
    let src = format!("{FUTURE}{LIES}def use(x: int) -> int:\n    return lies(x).bit_length()\n");
    let by = by_cause(&[("m.py", &src)], "33");
    // the finding is kept, the patch dropped
    assert!(by["lying-return:m.lies"].fix.is_none());
}

#[test]
fn mixed_returns_get_the_revealed_union_with_none_last() {
    let src = format!(
        "{FUTURE}def mixed(\n    x: int,\n):\n    if x:\n        return False\n    return\n\
         def use(x: int) -> bool | None:\n    return mixed(x)\n"
    );
    let by = by_cause(&[("m.py", &src)], "33");
    // Literal[False] | None deliteralizes to bool | None, spliced at the ':'
    assert_eq!(
        fix_of(&by, "mixed-returns:m.mixed").edits,
        [edit(4, 1, 1, " -> bool | None")]
    );
}

#[test]
fn a_mixed_fix_is_vetoed_when_a_caller_needs_the_value() {
    let src = format!(
        "{FUTURE}def mixed(x: int):\n    if x:\n        return 1\n    return\n\
         def use(x: int) -> int:\n    return mixed(x)\n"
    );
    let by = by_cause(&[("m.py", &src)], "33");
    assert!(by["mixed-returns:m.mixed"].fix.is_none());
}

#[test]
fn a_repo_class_the_module_binds_is_spelled_without_an_import() {
    let src = format!(
        "{FUTURE}class Foo:\n    pass\ndef mixed(x: int):\n    if x:\n        return Foo()\n    return\n"
    );
    let by = by_cause(&[("m.py", &src)], "33");
    assert_eq!(
        fix_of(&by, "mixed-returns:m.mixed"),
        &Fix {
            rel: "m.py".into(),
            edits: vec![edit(4, 17, 17, " -> Foo | None")],
            imports: Vec::new(),
        }
    );
}

#[test]
fn no_fix_where_the_splice_cannot_be_honest() {
    let unbound = format!(
        "{FUTURE}from n import make\ndef mixed(x: int):\n    if x:\n        return make()\n    return\n"
    );
    let lying_none = format!("{FUTURE}def none_lies(x: int) -> None:\n    return x + 1\n");
    let cases: Vec<Vec<(&str, &str)>> = vec![
        // a repo class the module never binds in the revealed union
        vec![
            (
                "n.py",
                "class Foo:\n    pass\ndef make() -> Foo:\n    return Foo()\n",
            ),
            ("m.py", &unbound),
        ],
        // `-> None` returning a value: intent, not mechanics
        vec![("m.py", &lying_none)],
        // `list[int] | None` revealed, no PEP 585 evidence the module runs
        vec![(
            "m.py",
            "def items(x: int):\n    if x:\n        return [x]\n    return\n\
             def use(x: int) -> object:\n    return items(x)\n",
        )],
    ];
    for files in cases {
        let by = by_cause(&files, "33");
        assert!(!by.is_empty(), "the fixture must fire");
        assert!(by.values().all(|f| f.fix.is_none()));
    }
}

// --- typing spellings where the module does not run PEP 604 ------------------

#[test]
fn a_none_path_lie_spells_optional_over_the_annotation() {
    let src = format!("{LIES}{TOLERANT}");
    let by = by_cause(&[("m.py", &src)], "33");
    assert_eq!(
        fix_of(&by, "lying-return:m.lies"),
        &Fix {
            rel: "m.py".into(),
            edits: vec![edit(1, 20, 23, "Optional[int]")],
            imports: vec!["from typing import Optional".to_string()],
        }
    );
}

#[test]
fn an_import_rides_a_line_where_annotations_resolve_in_order() {
    // below 3.14 ty evaluates annotations eagerly, so an end-of-file import
    // splice reads as unresolved and vetoes every import-bringing proposal;
    // the world rides the first top-of-file import, else a blank line
    let with_import = format!("import os\n{LIES}{TOLERANT}");
    let without = format!("# no imports at all\n\n{LIES}{TOLERANT}");
    let by = by_cause(
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"fx\"\nrequires-python = \">=3.8\"\n",
            ),
            ("m.py", &with_import),
            ("n.py", &without),
        ],
        "33",
    );
    assert_eq!(
        fix_of(&by, "lying-return:m.lies").imports,
        ["from typing import Optional"]
    );
    assert_eq!(
        fix_of(&by, "lying-return:n.lies").imports,
        ["from typing import Optional"]
    );
}

#[test]
fn function_local_annotations_are_not_runtime_evidence() {
    // `y: int | None` inside a body never executes; the def header would
    let src = format!(
        "{}{TOLERANT}",
        "def lies(x: int) -> int:\n    y: int | None = None\n    if x:\n        return 1\n    return None\n"
    );
    let by = by_cause(&[("m.py", &src)], "33");
    assert_eq!(
        fix_of(&by, "lying-return:m.lies").edits[0].text,
        "Optional[int]"
    );
}

#[test]
fn annotations_under_a_module_level_block_are_runtime_evidence() {
    // a `try:` (or an `if`) at module scope runs its body at import: the
    // `int | None` there is the module's own PEP 604
    let src = format!(
        "try:\n    LIMIT: int | None = None\nexcept TypeError:\n    LIMIT = None\n{LIES}{TOLERANT}"
    );
    let by = by_cause(&[("m.py", &src)], "33");
    assert_eq!(fix_of(&by, "lying-return:m.lies").edits[0].text, " | None");
}

#[test]
fn mixed_returns_spell_typing_unions() {
    let by = by_cause(
        &[(
            "m.py",
            // `Optional` already bound: no import to add
            "from typing import Optional\n\
             def one(x: int):\n    if x:\n        return False\n    return\n\
             def two(x: int):\n    if x:\n        return 1\n\
             \x20   if x > 1:\n        return 's'\n    return\n\
             def use(x: int) -> object:\n    return one(x), two(x)\n",
        )],
        "33",
    );
    assert_eq!(
        fix_of(&by, "mixed-returns:m.one"),
        &Fix {
            rel: "m.py".into(),
            edits: vec![edit(2, 15, 15, " -> Optional[bool]")],
            imports: Vec::new(),
        }
    );
    assert_eq!(
        fix_of(&by, "mixed-returns:m.two"),
        &Fix {
            rel: "m.py".into(),
            edits: vec![edit(6, 15, 15, " -> Union[int, str, None]")],
            imports: vec!["from typing import Union".to_string()],
        }
    );
}

#[test]
fn a_subscripted_member_needs_pep585_evidence() {
    let by = by_cause(
        &[(
            "m.py",
            "def items(x: list[int]):\n    if len(x) > 1:\n        return x\n    return\n\
             def use(x: list[int]) -> object:\n    return items(x)\n",
        )],
        "33",
    );
    assert_eq!(
        fix_of(&by, "mixed-returns:m.items").edits[0].text,
        " -> Optional[list[int]]"
    );
}

// --- #48 folds ---------------------------------------------------------------

#[test]
fn a_fold_owns_the_def_a_lift_would_annotate() {
    // #5 lifts `x: int` onto the def #48 deletes: the fold's deletion span
    // owns the def, so the lift keeps its finding and loses its patch, and
    // the fold keeps both, the caller-side edit included
    let (_dir, mut stack) = with_oracle(&[(
        "m.py",
        "def _double(x):\n    return x * 2\n\n\n\
         def use(n: int) -> int:\n    return _double(n) + 1\n",
    )]);
    let mut findings: Vec<Finding> = run_rule_on("5", &stack);
    findings.extend(run_rule_on("48", &stack));
    assert_eq!(
        findings
            .iter()
            .map(|f| f.cause.as_str())
            .collect::<Vec<_>>(),
        ["lift:m._double:x", "fold:m._double"]
    );
    // the lift verified on its own
    assert!(findings[0].fix.is_some());
    let patched = emit::attach_fixes(findings, stack.facts(), &stack.provers);
    stack.provers.close();
    let (lift, fold) = (&patched[0], &patched[1]);
    assert!(lift.fix.is_none() && lift.cause == "lift:m._double:x");
    assert_eq!(
        fold.fix.as_ref().expect("the fold's patch").edits[0].line,
        6
    );
}
