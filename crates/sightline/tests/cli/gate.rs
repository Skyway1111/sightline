//! The gate round trip (port of `tests/test_gate.py`'s gate half): block,
//! suppress, baseline, untouched files, parse errors, `--since` and the two
//! postures. The single-file / full-repo identity half is the rules crates'.

use std::process::Command;

use sightline_testkit::make_repo;

use crate::{NO_ORACLE, root, run};

/// A name built at runtime: rule #24, `scope="file"`, RATCHET.
const VIOLATION: &str = "def f(obj: object, kind: str) -> object:\n\
                         \x20   return getattr(obj, 'handle_' + kind)\n";
const CLEAN: &str = "\"\"\"ok.\"\"\"\n";
/// The one thing the hook must never green-light.
const BROKEN: &str = "def f(:\n    pass\n";

#[test]
fn a_violation_in_a_changed_file_blocks() {
    let dir = make_repo(&[("v.py", VIOLATION), ("c.py", CLEAN)]);
    let out = run(&["gate", &root(&dir), "--files", "v.py"]);

    assert_eq!(out.code, 1);
    assert!(
        out.out.contains("v.py:2") && out.out.contains("#24"),
        "{}",
        out.out
    );
}

#[test]
fn a_suppressed_finding_passes() {
    let marked = VIOLATION.replace("kind)\n", "kind)  # sightline-ok: 24\n");
    let dir = make_repo(&[("v.py", &marked)]);
    let out = run(&["gate", &root(&dir), "--files", "v.py"]);

    assert_eq!(out.code, 0);
    assert!(out.out.contains("suppressed 1"), "{}", out.out);
}

#[test]
fn a_baselined_finding_passes() {
    let dir = make_repo(&[("pyproject.toml", NO_ORACLE), ("v.py", VIOLATION)]);
    assert_eq!(run(&["baseline", &root(&dir)]).code, 0);
    let out = run(&["gate", &root(&dir), "--files", "v.py"]);

    assert_eq!(out.code, 0);
    assert!(out.out.contains("baselined 1"), "{}", out.out);
}

#[test]
fn suppression_survives_lines_the_tokenizer_does_not_count() {
    // `str.splitlines` breaks at \x0c and \x85; the tokenizer and the AST
    // count \n alone, so every marker after one would sit a line off
    let dir = make_repo(&[(
        "v.py",
        "def a() -> int:\n    return 1\n\x0c\n\
         def f(obj: object, kind: str) -> object:\n\
         \x20   # sightline-ok: 24\n\
         \x20   return getattr(obj, 'h_' + kind)  # \u{85} inside a comment\n",
    )]);
    let out = run(&["gate", &root(&dir), "--files", "v.py"]);

    assert_eq!(out.code, 0);
    assert!(out.out.contains("suppressed 1"), "{}", out.out);
}

#[test]
fn untouched_and_deleted_and_other_suffix_files_are_skipped() {
    let dir = make_repo(&[
        ("v.py", VIOLATION),
        ("c.py", CLEAN),
        ("notes.md", "# notes\n"),
    ]);
    let out = run(&[
        "gate",
        &root(&dir),
        "--files",
        "gone.py",
        "notes.md",
        "c.py",
    ]);

    assert_eq!(out.code, 0);
    assert!(out.out.contains("files checked 1"), "{}", out.out);
}

#[test]
fn an_unparsable_changed_file_blocks_and_is_named() {
    // a file that does not parse has no findings at all: the fast gate must
    // never green-light it (the hook's one unconditional job)
    let dir = make_repo(&[("pyproject.toml", NO_ORACLE), ("broken.py", BROKEN)]);
    let out = run(&["gate", &root(&dir), "--files", "broken.py"]);

    assert_eq!(out.code, 1);
    assert!(out.out.contains("blocking 0"), "{}", out.out);
    assert!(
        out.out.contains("note: unparsable: broken.py"),
        "{}",
        out.out
    );
}

#[test]
fn gate_full_names_a_parse_error_without_blocking() {
    // --full has no changed-file context, and the ratchet has no key to
    // absorb a parse error with: a vendored unparsable file is named, never
    // a permanent CI failure
    let dir = make_repo(&[("pyproject.toml", NO_ORACLE), ("broken.py", BROKEN)]);
    let out = run(&["gate", &root(&dir), "--full"]);

    assert_eq!(out.code, 0, "{}", out.out);
    assert!(
        out.out.contains("note: unparsable: broken.py"),
        "{}",
        out.out
    );
}

#[test]
fn no_git_and_no_files_is_an_error() {
    let dir = make_repo(&[("m.py", CLEAN)]);
    let out = run(&["gate", &root(&dir)]);

    assert_eq!(out.code, 2);
    assert!(out.err.contains("--files"), "{}", out.err);
}

#[test]
fn gate_full_rejects_a_file_list() {
    let dir = make_repo(&[("m.py", CLEAN)]);
    let out = run(&["gate", &root(&dir), "--full", "--files", "x.py"]);

    assert_eq!(out.code, 2);
    assert!(
        out.err.contains("--full gates the whole tree"),
        "{}",
        out.err
    );
}

#[test]
fn a_baseline_key_from_the_full_build_absorbs_a_single_file_gate() {
    // stem collision and PEP 420: baseline keys come from the full build, and
    // the single-file build must name the symbol the same way or a baselined
    // finding blocks
    for files in [
        vec![
            ("pyproject.toml", NO_ORACLE),
            ("mod.py", CLEAN),
            ("sub/mod.py", VIOLATION),
        ],
        vec![
            ("pyproject.toml", NO_ORACLE),
            ("src/ns/pkg/__init__.py", CLEAN),
            ("src/ns/pkg/mod.py", VIOLATION),
        ],
    ] {
        let target = files.last().unwrap().0;
        let dir = make_repo(&files);
        assert_eq!(run(&["baseline", &root(&dir)]).code, 0);
        let out = run(&["gate", &root(&dir), "--files", target]);
        assert_eq!(out.code, 0, "{target}: {}", out.out);
        assert!(out.out.contains("baselined 1"), "{target}: {}", out.out);
    }
}

fn git(root: &str, args: &[&str]) {
    let done = Command::new("git")
        .args(["-C", root, "-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(done.status.success(), "git {args:?}: {:?}", done.stderr);
}

#[test]
fn since_unions_the_branch_commits_with_the_working_tree() {
    let dir = make_repo(&[("c.py", CLEAN), ("v.py", CLEAN), ("w.py", CLEAN)]);
    let at = root(&dir);
    git(&at, &["init", "-q", "-b", "main"]);
    git(&at, &["add", "."]);
    git(&at, &["commit", "-q", "-m", "base"]);
    git(&at, &["checkout", "-q", "-b", "feat"]);
    std::fs::write(dir.path().join("v.py"), VIOLATION).unwrap();
    git(&at, &["commit", "-q", "-am", "committed violation"]);
    std::fs::write(dir.path().join("w.py"), VIOLATION).unwrap();

    let gated = |extra: &[&str]| {
        let mut args = vec!["gate", at.as_str()];
        args.extend_from_slice(extra);
        let out = run(&args);
        let mut hit: Vec<String> = out
            .out
            .lines()
            .filter(|l| l.contains("#24"))
            .map(|l| l.split(':').next().unwrap_or_default().to_string())
            .collect();
        hit.sort();
        (out.code, hit)
    };

    assert_eq!(
        gated(&["--since", "main"]),
        (1, vec!["v.py".to_string(), "w.py".to_string()])
    );
    assert_eq!(gated(&[]), (1, vec!["w.py".to_string()]));
    assert_eq!(gated(&["--since", "main", "--files", "c.py"]), (0, vec![]));
    assert_eq!(run(&["gate", &at, "--full", "--since", "main"]).code, 2);
}

/// #23's own goal forbids gating it: it stays in the audit, out of the fast
/// gate and out of the baseline.
#[test]
fn a_report_rule_reports_but_never_blocks_or_baselines() {
    let deep = "from other import helper\n\n\ndef f(xs):\n\
                \x20   for a in xs:\n\
                \x20       if a:\n\
                \x20           for b in a:\n\
                \x20               if b:\n\
                \x20                   if b > 1:\n\
                \x20                       return b\n\
                \x20   return 0\n";
    let dir = make_repo(&[
        ("pyproject.toml", NO_ORACLE),
        ("m.py", deep),
        ("other.py", "def helper(v):\n    return v\n"),
    ]);

    let audited = run(&["audit", &root(&dir), "--json"]);
    assert_eq!(audited.code, 0);
    let rules: Vec<String> = crate::findings(&audited.out)
        .iter()
        .map(|f| f.0.clone())
        .collect();
    assert!(rules.contains(&"23".to_string()), "{rules:?}");

    let fast = run(&["gate", &root(&dir), "--files", "m.py"]);
    assert!(!fast.out.contains("#23"), "{}", fast.out);
    assert_eq!(run(&["baseline", &root(&dir)]).code, 0);
    let counts = std::fs::read_to_string(dir.path().join(".sightline-baseline.json")).unwrap();
    assert!(!counts.contains("\"23|"), "{counts}");
}
