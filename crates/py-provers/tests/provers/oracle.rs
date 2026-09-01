//! Port of REF `tests/provers/test_oracle.py`, `tests/provers/test_fused_pass.py`
//! (the two parts that are the oracle's own) and `tests/provers/test_unresolved.py`:
//! the checker in process. Each test builds one `ProjectDatabase` on a mini
//! repo, so each pays a cold check of the vendored typeshed.

use camino::Utf8Path;
use indexmap::IndexMap;
use sightline_core::config::Config;
use sightline_core::findings::Rel;
use sightline_py_provers::oracle::{Oracle, TypeQuery, UnresolvedImports};
use sightline_py_provers::{NO_ORACLE_NOTE, Provers};
use sightline_testkit::build;

const PROBE: &str = concat!(
    "from typing import cast\n",
    "def a(x: str) -> bool:\n",
    "    return isinstance(x, str)\n",
    "def b(x: int) -> bool:\n",
    "    return x is None\n",
    "def c(x: int) -> bool:\n",
    "    return x in ['s']\n",
    "def d(x: str) -> str:\n",
    "    return cast(str, x)\n",
);

fn oracle_at(root: &Utf8Path) -> Oracle {
    Oracle::new(root, &[], &[], None).expect("the checker builds on a mini repo")
}

fn query(rel: &str, line: u32, col_start: u32, col_end: u32) -> TypeQuery {
    TypeQuery {
        id: "q1".to_string(),
        rel: Rel::from(rel),
        line,
        col_start,
        col_end,
    }
}

#[test]
fn unnecessary_diagnostics_roundtrip() {
    let (dir, stack) = build(&[("m.py", PROBE)]);
    let oracle = oracle_at(&stack.facts().root);
    let mut rules: Vec<&str> = oracle
        .unnecessary()
        .iter()
        .map(|d| d.rule.as_str())
        .collect();
    rules.sort();
    assert_eq!(
        rules,
        [
            "reportUnnecessaryCast",
            "reportUnnecessaryComparison",
            "reportUnnecessaryContains",
            "reportUnnecessaryIsInstance",
        ]
    );
    assert!(oracle.unnecessary().iter().all(|d| &*d.rel == "m.py"));
    drop(dir);
}

#[test]
fn established_types_are_narrowed() {
    const SRC: &str = concat!(
        "def use(v: object) -> None:\n",
        "    pass\n",
        "def f(a: int | str) -> None:\n",
        "    if isinstance(a, str):\n",
        "        use(a)\n", // line 5, `a` at byte column 12
    );
    let (dir, stack) = build(&[("m.py", SRC)]);
    let oracle = oracle_at(&stack.facts().root);
    assert_eq!(
        oracle.span_types(&[query("m.py", 5, 12, 13)]),
        vec![Some("str".to_string())]
    );
    drop(dir);
}

#[test]
fn a_bad_span_answers_none() {
    let (dir, stack) = build(&[("m.py", "x = 1\n")]);
    let oracle = oracle_at(&stack.facts().root);
    // a line past the file, and a file the project cannot resolve: an honest
    // miss either way, never a panic
    assert_eq!(oracle.span_types(&[query("m.py", 99, 0, 1)]), vec![None]);
    assert_eq!(
        oracle.span_types(&[query("absent.py", 1, 0, 1)]),
        vec![None]
    );
    assert!(oracle.failure().is_none());
    drop(dir);
}

#[test]
fn an_expr_query_answers_at_module_scope() {
    let (dir, stack) = build(&[("m.py", "def predicate(q: str):\n    return q.count('x')\n")]);
    let oracle = oracle_at(&stack.facts().root);
    assert_eq!(
        oracle.module_member_type(&Rel::from("m.py"), "predicate"),
        Some("(q: str) -> int".to_string())
    );
    // the appended reveal leaves no diagnostic and no override behind
    assert!(oracle.diagnostics().is_empty());
    drop(dir);
}

#[test]
fn a_module_member_is_a_class_or_is_not() {
    let (dir, stack) = build(&[(
        "m.py",
        "import collections\nimport os\ndef f() -> int:\n    return 1\n",
    )]);
    let oracle = oracle_at(&stack.facts().root);
    let rel = Rel::from("m.py");
    assert!(oracle.member_is_class(&rel, "collections", "OrderedDict"));
    assert!(!oracle.member_is_class(&rel, "os", "getcwd"));
    assert!(!oracle.member_is_class(&rel, "os", "absent_member"));
    drop(dir);
}

#[test]
fn worlds_report_what_they_add_and_inherit_nothing() {
    const SRC: &str = concat!(
        "def helper(q: str):\n",
        "    return q.count('x')\n",
        "def caller(n: int) -> int:\n",
        "    return helper('abc')\n",
    );
    let (dir, stack) = build(&[("m.py", SRC)]);
    let oracle = oracle_at(&stack.facts().root);
    let world = |content: String| -> IndexMap<String, String> {
        IndexMap::from([("m.py".to_string(), content)])
    };
    let added = oracle.verify_worlds(
        &[
            ("bad".to_string(), world(SRC.replace("q: str", "q: int"))),
            (
                "ok".to_string(),
                world(SRC.replace("def helper(q: str):", "def helper(q: str) -> int:")),
            ),
        ],
        None,
    );
    assert!(added["bad"].iter().any(|d| d.severity == "error"));
    // the clean world runs after the breaking one on the same db: override
    // isolation means it inherits none of the breakage
    assert!(added["ok"].is_empty());
    // every call is logged for the `verify` layer, an empty one included
    assert!(oracle.verify_worlds(&[], None).is_empty());
    let calls = oracle.world_calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].worlds[0].0, "bad");
    assert_eq!(&*calls[0].worlds[0].1[0], "m.py");
    assert!(calls[1].worlds.is_empty() && calls[1].added.is_empty());
    drop(dir);
}

#[test]
fn unresolved_imports_are_counted_per_module() {
    const SRC: &str = concat!(
        "import absentmod\n",
        "from json import loads\n",
        "def load(p: str):\n",
        "    return absentmod.load(p)\n",
        "def parse(p: str):\n",
        "    return loads(p)\n",
        "def local(p: str):\n",
        "    import other.missing as om\n",
        "    return om.read(p)\n",
    );
    let (dir, stack) = build(&[("m.py", SRC)]);
    let oracle = oracle_at(&stack.facts().root);
    let unresolved = UnresolvedImports::new(stack.facts(), Some(&oracle));
    assert_eq!(
        unresolved.modules,
        IndexMap::from([
            ("absentmod".to_string(), 1),
            ("other.missing".to_string(), 1)
        ])
    );
    assert_eq!(unresolved.count(), 2);
    drop(dir);
}

#[test]
fn a_checker_that_never_started_degrades_the_run_and_the_header_says_so() {
    // the in-process twin of the shim that crashed: a root the project cannot be
    // discovered at is a crashed oracle, never a `None` one, so `close` names
    // it and the rules it serves go silent
    let (dir, stack) = build(&[(
        "m.py",
        "def check(x: str) -> bool:\n    return isinstance(x, str)\n",
    )]);
    let mut provers = Provers::bare(stack.facts());
    provers.oracle = Some(
        Oracle::new(&stack.facts().root.join("absent"), &[], &[], None)
            .expect("a failed construction is still an oracle"),
    );
    assert!(provers.no_oracle());
    let failure = provers
        .oracle
        .as_ref()
        .and_then(Oracle::failure)
        .expect("the construction failure is recorded");
    assert!(failure.starts_with("construction: "), "{failure}");
    provers.close();
    assert!(
        provers
            .notes()
            .iter()
            .any(|n| n.starts_with("oracle crashed mid-run") && n.contains(NO_ORACLE_NOTE)),
        "{:?}",
        provers.notes()
    );
    assert!(provers.diagnostics(stack.facts()).is_empty());
    drop(dir);
}

#[test]
fn a_lossily_decoded_module_is_named() {
    // `# caf\xe9` is latin-1: facts decode it as U+FFFD, so the module's byte
    // columns are no one else's and the header says it is left alone. The
    // "never queried" half is the query units'.
    let dir = sightline_testkit::make_repo(&[]);
    std::fs::write(
        dir.path().join("m.py"),
        b"# caf\xe9\ndef f(x: int) -> int:\n    return x\n",
    )
    .unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let mut config = Config::new();
    config.oracle = false;
    let listing = sightline_core::walk::discover(root, &config);
    let built = sightline_py_facts::build::build_facts(root, &config, &listing, None);
    let provers = Provers::new(root, &config, built.borrow_dependent(), false);
    assert!(
        provers.notes().iter().any(|n| n
            == "non-UTF-8 bytes decoded lossily (no oracle span queries or fixes there): m.py"),
        "{:?}",
        provers.notes()
    );
}
