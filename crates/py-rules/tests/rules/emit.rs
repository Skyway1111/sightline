//! `tests/test_emit.py`: span edits and import insertion into one
//! git-apply-able unified diff. The corpus half (every patch applies, the
//! target suites pass, patched findings vanish) is phase 9's `xtask
//! fix-check`.

use std::fs;

use sightline_core::edits::blank;
use sightline_core::findings::{Evidence, Finding, Fix, Site, SpanEdit};
use sightline_py_rules::emit;
use sightline_testkit::build;
use tempfile::TempDir;

fn edit(line: u32, col_start: u32, col_end: u32, text: &str) -> SpanEdit {
    SpanEdit {
        line,
        col_start,
        col_end,
        text: text.to_string(),
    }
}

fn fix(rel: &str, edits: Vec<SpanEdit>, imports: &[&str]) -> Fix {
    Fix {
        rel: rel.into(),
        edits,
        imports: imports.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn finding(fix: Fix) -> Finding {
    Finding {
        rule: "5",
        site: Site {
            rel: fix.rel.clone(),
            line: 1,
            col: 0,
            symbol: "m.f".into(),
        },
        message: "m".to_string(),
        cause: "c".to_string(),
        evidence: Evidence::ast(),
        salience: 0.0,
        fix: Some(fix),
        lang: "py",
    }
}

/// `git apply -` on the mini repo. The bytes go through a file, never a
/// shell: a pipe would CRLF-translate the patch itself on Windows.
#[allow(clippy::disallowed_types, clippy::disallowed_methods)]
fn apply(dir: &TempDir, diff: &str) {
    let patch = dir.path().join("fixes.patch");
    fs::write(&patch, diff.as_bytes()).expect("the patch file");
    let done = std::process::Command::new("git")
        .args(["apply", &patch.to_string_lossy()])
        .current_dir(dir.path())
        .output()
        .expect("running git apply");
    fs::remove_file(&patch).expect("the patch file goes");
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
}

/// The patched file as `Path.read_text` hands it over: universal newlines,
/// so `core.autocrlf` deciding what `git apply` writes is not the assertion.
#[allow(clippy::disallowed_methods)]
fn read(dir: &TempDir, rel: &str) -> String {
    fs::read_to_string(dir.path().join(rel))
        .expect("the patched file")
        .replace("\r\n", "\n")
}

/// The diff of one fix over an inline repo.
fn diff_of(files: &[(&str, &str)], f: Finding) -> (TempDir, String) {
    let (dir, stack) = build(files);
    let text = emit::unified_diff(&[f], stack.facts());
    (dir, text)
}

#[test]
fn an_insert_edit_round_trips_through_git_apply() {
    let (dir, diff) = diff_of(
        &[("m.py", "def f(xs):\n    return list(xs)\n")],
        finding(fix(
            "m.py",
            vec![edit(1, 8, 8, ": Sequence[int]")],
            &["from collections.abc import Sequence"],
        )),
    );
    assert!(diff.contains("--- a/m.py") && diff.contains("+++ b/m.py"));
    apply(&dir, &diff);
    assert_eq!(
        read(&dir, "m.py"),
        "from collections.abc import Sequence\ndef f(xs: Sequence[int]):\n    return list(xs)\n"
    );
}

#[test]
fn an_import_lands_after_the_last_top_of_file_import() {
    let (dir, diff) = diff_of(
        &[(
            "m.py",
            "\"\"\"doc.\"\"\"\nimport os\nimport sys\n\n\ndef f(xs: list[int]):\n    return [x for x in xs]\n",
        )],
        finding(fix(
            "m.py",
            vec![edit(6, 10, 19, "Iterable[int]")],
            &["from collections.abc import Iterable"],
        )),
    );
    apply(&dir, &diff);
    let text = read(&dir, "m.py");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[3], "from collections.abc import Iterable");
    assert_eq!(lines[6], "def f(xs: Iterable[int]):");
}

#[test]
fn imports_land_one_line_per_home() {
    let (dir, diff) = diff_of(
        &[("m.py", "def f(xs):\n    return list(xs)\n")],
        finding(fix(
            "m.py",
            vec![edit(1, 8, 8, ": Optional[Sequence[int]]")],
            &[
                "from typing import Optional",
                "from collections.abc import Sequence",
            ],
        )),
    );
    apply(&dir, &diff);
    let text = read(&dir, "m.py");
    assert_eq!(
        text.lines().take(2).collect::<Vec<_>>(),
        [
            "from collections.abc import Sequence",
            "from typing import Optional"
        ]
    );
}

#[test]
fn a_home_the_file_already_imports_from_grows_that_line() {
    let (dir, diff) = diff_of(
        &[(
            "m.py",
            "from collections.abc import Mapping\n\ndef f(xs):\n    return list(xs)\n",
        )],
        finding(fix(
            "m.py",
            vec![edit(3, 8, 8, ": Sequence[int]")],
            &["from collections.abc import Sequence"],
        )),
    );
    apply(&dir, &diff);
    let text = read(&dir, "m.py");
    assert_eq!(
        text.lines().take(2).collect::<Vec<_>>(),
        ["from collections.abc import Mapping, Sequence", ""]
    );
}

#[test]
#[allow(clippy::disallowed_methods)]
fn crlf_files_patch_cleanly() {
    // `make_repo` writes the bytes as given, so the fixture keeps its CRLF
    let (dir, diff) = diff_of(
        &[("m.py", "def f(xs):\r\n    return list(xs)\r\n")],
        finding(fix("m.py", vec![edit(1, 8, 8, ": list[int]")], &[])),
    );
    apply(&dir, &diff);
    assert_eq!(
        fs::read(dir.path().join("m.py")).expect("the patched bytes"),
        b"def f(xs: list[int]):\r\n    return list(xs)\r\n".to_vec()
    );
}

const DELETABLE: &str = "import os\nimport sys\n\n\ndef f():\n    return sys\n";

#[test]
fn a_deletion_drops_its_lines_from_the_patch() {
    // the world blanks the line (its diagnostic diff is line-keyed); the
    // patch, whose text does move, removes it
    let (dir, stack) = build(&[("m.py", DELETABLE)]);
    let lines = &stack.facts().modules["m"].lines;
    let diff = emit::unified_diff(
        &[finding(fix("m.py", blank(lines, 1, 1), &[]))],
        stack.facts(),
    );
    apply(&dir, &diff);
    assert_eq!(
        read(&dir, "m.py"),
        "import sys\n\n\ndef f():\n    return sys\n"
    );
}

#[test]
fn no_fixable_findings_yields_an_empty_diff() {
    let (_dir, stack) = build(&[("m.py", "def f():\n    return 1\n")]);
    let plain = Finding {
        rule: "29",
        fix: None,
        ..finding(fix("m.py", vec![], &[]))
    };
    assert_eq!(emit::unified_diff(&[plain], stack.facts()), "");
}

#[test]
fn a_deletion_takes_the_blank_run_it_would_leave() {
    // a patch that removes a statement between two-blank separators would
    // leave four: the edit extends over the extras
    let src = concat!(
        "import os\n\n\n",
        "def keep():\n    return os.sep\n\n\n",
        "def dead():\n    return 1\n\n\n",
        "def tail():\n    return 2\n",
    );
    let (dir, stack) = build(&[("m.py", src)]);
    let lines = &stack.facts().modules["m"].lines;
    let diff = emit::unified_diff(
        &[finding(fix("m.py", blank(lines, 8, 9), &[]))],
        stack.facts(),
    );
    apply(&dir, &diff);
    assert_eq!(
        read(&dir, "m.py"),
        concat!(
            "import os\n\n\n",
            "def keep():\n    return os.sep\n\n\n",
            "def tail():\n    return 2\n",
        )
    );
}
