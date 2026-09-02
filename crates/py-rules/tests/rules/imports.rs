//! #35's import topology and `hoist_splice`.

use sightline_core::findings::Finding;
use sightline_py_provers::counterfactual::Splice;
use sightline_py_rules::imports::hoist_splice;
use sightline_testkit::{build, run_rule};

fn run(id: &str, files: &[(String, String)]) -> Vec<Finding> {
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(rel, src)| (rel.as_str(), src.as_str()))
        .collect();
    run_rule(id, &refs)
}

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

fn splice_of(cause: &str, files: &[(String, String)]) -> Option<Splice> {
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(rel, src)| (rel.as_str(), src.as_str()))
        .collect();
    let (_dir, stack) = build(&refs);
    hoist_splice(cause, stack.facts(), &stack.provers)
}

// --- #35 import topology ------------------------------------------------------

/// The known cycle in a fixture package, one finding.
#[test]
fn rule_35_cycle_scc_reported_once() {
    let findings = run_rule(
        "35",
        &[
            ("pkg/__init__.py", ""),
            ("pkg/a.py", "from pkg import b\ndef fa():\n    return b\n"),
            ("pkg/b.py", "import pkg.a\ndef fb():\n    return pkg.a\n"),
        ],
    );
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert!(f.cause.starts_with("import-cycle:"));
    // the sorted-first member anchors
    assert_eq!(&*f.site.rel, "pkg/a.py");
    assert!(f.message.contains("pkg.a") && f.message.contains("pkg.b"));
}

#[test]
fn rule_35_three_cycle_is_still_one_finding() {
    let findings = run_rule(
        "35",
        &[
            ("a.py", "import b\n"),
            ("b.py", "import c\n"),
            ("c.py", "import a\n"),
        ],
    );
    assert_eq!(findings.len(), 1);
    assert!(findings[0].cause.starts_with("import-cycle:"));
    assert!(findings[0].message.contains("3 modules"));
}

#[test]
fn rule_35_lazy_mutual_import_is_entanglement_not_cycle() {
    let findings = run_rule(
        "35",
        &[
            ("a.py", "def fa():\n    import b\n    return b\n"),
            ("b.py", "import a\ndef fb():\n    return a\n"),
        ],
    );
    assert_eq!(causes(&findings), ["entangled:a<->b"]);
    assert!(findings[0].message.contains("deferred import"));
}

#[test]
fn rule_35_acyclic_imports_silent() {
    let findings = run_rule(
        "35",
        &[
            ("a.py", "import b\nimport c\n"),
            ("b.py", "import c\n"),
            ("c.py", "def f():\n    return 1\n"),
        ],
    );
    assert!(findings.is_empty());
}

const PKG: [(&str, &str); 2] = [
    ("pkg/__init__.py", ""),
    ("pkg/util.py", "def helper():\n    return 1\n"),
];

const APP_SRC: &str = "def f():\n    from pkg import util\n    return util.helper()\n";

#[test]
fn rule_35_local_import_hiding_no_cycle_is_hoistable() {
    let findings = run_rule("35", &[PKG[0], PKG[1], ("app.py", APP_SRC)]);
    assert_eq!(
        causes(&findings),
        ["hoistable-import:app.f:from pkg import util"]
    );
    let f = &findings[0];
    assert_eq!((&*f.site.rel, &*f.site.symbol), ("app.py", "app.f"));
    assert!(f.message.contains("pkg.util") && f.message.contains("sightline-ok: 35"));
}

/// The twin: `pkg.util` imports app at top level, so the deferral is needed.
#[test]
fn rule_35_local_import_hiding_a_cycle_is_not_hoistable() {
    let findings = run_rule(
        "35",
        &[
            PKG[0],
            ("pkg/util.py", "import app\ndef helper():\n    return app\n"),
            ("app.py", APP_SRC),
        ],
    );
    assert!(
        !findings
            .iter()
            .any(|f| f.cause.starts_with("hoistable-import:"))
    );
}

/// `pkg/__init__.py`'s `def session` and `pkg/session.py` share the qname
/// `pkg.session`: the module's top-level imports are no one's function scope.
#[test]
fn rule_35_a_module_named_like_a_sibling_def_keeps_its_top_level_imports() {
    let findings = run_rule(
        "35",
        &[
            ("pkg/__init__.py", "def session():\n    return 1\n"),
            (
                "pkg/session.py",
                "from pkg import util\n\ndef go():\n    return util.helper()\n",
            ),
            ("pkg/util.py", "def helper():\n    return 1\n"),
        ],
    );
    assert!(
        !findings
            .iter()
            .any(|f| f.cause.starts_with("hoistable-import:"))
    );
}

#[test]
fn rule_35_deferrals_with_a_reason_are_silent() {
    let apps = [
        // TYPE_CHECKING-guarded and third-party local imports
        concat!(
            "from typing import TYPE_CHECKING\ndef f(x):\n    if TYPE_CHECKING:\n",
            "        from pkg import util\n    import numpy\n    return numpy.asarray(x)\n",
        ),
        // an import under if/try is an intentional deferral
        concat!(
            "def f(flag):\n    if flag:\n        from pkg import util\n    try:\n",
            "        from pkg import util as u\n    except ImportError:\n        u = None\n",
            "    return util, u\n",
        ),
        // a class-body import runs at import time
        "class C:\n    from pkg import util\n",
    ];
    for app in apps {
        assert!(
            run_rule("35", &[PKG[0], PKG[1], ("app.py", app)]).is_empty(),
            "{app}"
        );
    }
}

/// A test's per-function import is isolation, not a deferral.
#[test]
fn rule_35_test_path_local_import_silent() {
    let findings = run_rule(
        "35",
        &[
            PKG[0],
            PKG[1],
            (
                "tests/test_app.py",
                "def test_f():\n    from pkg import util\n    assert util\n",
            ),
        ],
    );
    assert!(findings.is_empty());
}

// --- hoist_splice: the emitter is stricter than the finding -------------------

const HOIST_CAUSE: &str = "hoistable-import:app.f:from pkg import util";

/// The hoist-guard receipt passed every class; only a kind the catalog can
/// spell joins.
#[test]
fn hoist_splice_inert_kinds_the_catalog_can_spell() {
    let cases: [(&str, bool); 3] = [
        // a method on a string literal: `str.join`
        ("SEP = ', '.join(['a', 'b'])\n", true),
        // a global's method
        (
            "import numpy as np\nGRID = np.zeros(3)\nGRID.setflags(write=False)\n",
            false,
        ),
        // a call's result
        (
            "from pathlib import Path\nHERE = Path(__file__).resolve()\n",
            false,
        ),
    ];
    for (work, fixed) in cases {
        let files = vec![
            ("pkg/__init__.py".to_string(), String::new()),
            (
                "pkg/util.py".to_string(),
                format!("{work}def helper():\n    return 1\n"),
            ),
            ("app.py".to_string(), APP_SRC.to_string()),
        ];
        assert_eq!(splice_of(HOIST_CAUSE, &files).is_some(), fixed, "{work}");
    }
}

#[test]
fn hoist_splice_a_target_that_runs_at_import_gets_the_finding_and_no_fix() {
    let heavy = vec![
        ("pkg/__init__.py".to_string(), String::new()),
        (
            "pkg/util.py".to_string(),
            "import os\n\nTOKEN = os.environ.get('T')\n\n\ndef helper():\n    return TOKEN\n"
                .to_string(),
        ),
        ("app.py".to_string(), APP_SRC.to_string()),
    ];
    assert_eq!(causes(&run("35", &heavy)), [HOIST_CAUSE]);
    assert!(splice_of(HOIST_CAUSE, &heavy).is_none());
}

#[test]
fn hoist_splice_a_target_that_only_binds_names_gets_the_fix() {
    let files = vec![
        ("pkg/__init__.py".to_string(), String::new()),
        (
            "pkg/util.py".to_string(),
            "def helper():\n    return 1\n".to_string(),
        ),
        ("app.py".to_string(), APP_SRC.to_string()),
    ];
    assert!(splice_of(HOIST_CAUSE, &files).is_some());
}

// --- hoist_splice: a shipped subset pins the import surface -------------------

fn shipped_files(target: &str) -> Vec<(String, String)> {
    vec![
        (
            "stage.py".to_string(),
            "STAGED = ('core.py', 'inside.py')\n".to_string(),
        ),
        ("pkg/__init__.py".to_string(), String::new()),
        ("pkg/inside.py".to_string(), "VALUE = 1\n".to_string()),
        ("pkg/outside.py".to_string(), "VALUE = 2\n".to_string()),
        (
            "pkg/core.py".to_string(),
            format!("def f():\n    from pkg import {target}\n    return {target}.VALUE\n"),
        ),
    ]
}

#[test]
fn hoist_splice_a_hoist_out_of_the_shipped_set_is_refused() {
    let files = shipped_files("outside");
    let findings = run("35", &files);
    assert_eq!(
        causes(&findings),
        ["hoistable-import:pkg.core.f:from pkg import outside"]
    );
    assert!(findings[0].message.contains("pkg.outside") && findings[0].message.contains("ships"));
    assert!(
        splice_of(
            "hoistable-import:pkg.core.f:from pkg import outside",
            &files
        )
        .is_none()
    );
}

/// The twin: same shape one target apart - `inside.py` is staged too.
#[test]
fn hoist_splice_a_hoist_inside_the_shipped_set_still_applies() {
    let files = shipped_files("inside");
    let findings = run("35", &files);
    assert!(!findings[0].message.contains("ships"));
    assert!(splice_of("hoistable-import:pkg.core.f:from pkg import inside", &files).is_some());
}

// --- #35 precision round 20260841: which deferred edges entangle --------------

#[test]
fn rule_35_type_only_back_edge_never_entangles() {
    let typed = concat!(
        "from typing import TYPE_CHECKING\n",
        "if TYPE_CHECKING:\n    import a\n",
        "def fb(x: 'a.T'):\n    return x\n",
    );
    let base = ("a.py", "import b\nclass T:\n    pass\n");
    assert!(run_rule("35", &[base, ("b.py", typed)]).is_empty());
    // twin: the same back-edge at function scope is a hidden cycle
    let lazy = "def fb():\n    import a\n    return a\n";
    assert_eq!(
        causes(&run_rule("35", &[base, ("b.py", lazy)])),
        ["entangled:a<->b"]
    );
}

#[test]
fn rule_35_lazy_three_cycle_is_one_entanglement() {
    let findings = run_rule(
        "35",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/c1.py",
                "from pkg import c2\ndef f1():\n    return c2.f2()\n",
            ),
            (
                "pkg/c2.py",
                "from pkg import c3\ndef f2():\n    return c3.f3()\n",
            ),
            (
                "pkg/c3.py",
                "def f3():\n    from pkg import c1\n    return c1.f1\n",
            ),
        ],
    );
    assert_eq!(causes(&findings), ["entangled:pkg.c1<->pkg.c2<->pkg.c3"]);
    assert_eq!(&*findings[0].site.rel, "pkg/c1.py");
    assert!(findings[0].message.contains("deferred import"));
}

/// `a<->b` is a top-level cycle already reported; c's deferred import of a is
/// what hides c's membership, so the hidden part alone entangles.
#[test]
fn rule_35_a_lazy_superset_of_a_top_cycle_reports_the_hidden_part() {
    let findings = run_rule(
        "35",
        &[
            ("a.py", "import b\n"),
            ("b.py", "import a\nimport c\n"),
            ("c.py", "def f():\n    import a\n    return a\n"),
        ],
    );
    let mut found = causes(&findings);
    found.sort_unstable();
    assert_eq!(&found[..found.len() - 1], ["entangled:c"]);
    assert!(found[found.len() - 1].starts_with("import-cycle:"));
    let hidden = findings
        .iter()
        .find(|f| f.cause == "entangled:c")
        .expect("the hidden member is reported");
    assert_eq!(&*hidden.site.rel, "c.py");
    assert!(hidden.message.contains("a, b"));
}

#[test]
fn rule_35_constant_string_dynamic_import_is_an_edge() {
    for call in [
        "importlib.import_module('pkg.d2')",
        "import_module('pkg.d2')",
        "__import__('pkg.d2')",
    ] {
        let d1 = format!(
            "import importlib\nfrom importlib import import_module\ndef g1():\n    return {call}.g2()\n"
        );
        let files = vec![
            ("pkg/__init__.py".to_string(), String::new()),
            ("pkg/d1.py".to_string(), d1.clone()),
            (
                "pkg/d2.py".to_string(),
                "from pkg import d1\ndef g2():\n    return d1\n".to_string(),
            ),
        ];
        assert_eq!(
            causes(&run("35", &files)),
            ["entangled:pkg.d1<->pkg.d2"],
            "{call}"
        );
        // a name argument adds no edge
        let mut named = files;
        named[1].1 = d1.replace("'pkg.d2'", "name");
        assert!(run("35", &named).is_empty(), "{call}");
    }
}

#[test]
fn rule_35_hoist_message_names_the_targets_import_time_work() {
    let findings = run_rule(
        "35",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/db.py",
                "import sqlite3\nENGINE = sqlite3.connect(':memory:')\ndef query():\n    return ENGINE\n",
            ),
            ("pkg/leaf.py", "def value():\n    return 1\n"),
            (
                "app.py",
                concat!(
                    "def handler():\n    from pkg import db\n    return db.query()\n",
                    "def pure():\n    from pkg import leaf\n    return leaf.value()\n",
                ),
            ),
        ],
    );
    let mut found = causes(&findings);
    found.sort_unstable();
    assert_eq!(
        found,
        [
            "hoistable-import:app.handler:from pkg import db",
            "hoistable-import:app.pure:from pkg import leaf",
        ]
    );
    let message = |cause: &str| {
        findings
            .iter()
            .find(|f| f.cause == cause)
            .map(|f| f.message.clone())
            .expect("both arms fire")
    };
    assert!(message("hoistable-import:app.handler:from pkg import db").contains("import time"));
    assert!(!message("hoistable-import:app.pure:from pkg import leaf").contains("import time"));
    assert!(
        findings
            .iter()
            .all(|f| f.message.contains("sightline-ok: 35"))
    );
}

// --- #35's fix cases, at the splice the emitter rides -----------------------
// The `attach_fixes` half (the verified `Fix` riding the finding) waits for
// `py-rules-close`; the splice's own edits and imports are checked here.

const USE_DEP: &str = "def f() -> int:\n    from pkg import dep\n    return dep.VALUE\n";

fn dep_pkg(use_src: &str, dep: &str) -> Vec<(String, String)> {
    vec![
        ("pkg/__init__.py".to_string(), String::new()),
        ("pkg/dep.py".to_string(), dep.to_string()),
        ("pkg/use.py".to_string(), use_src.to_string()),
    ]
}

const DEP_CAUSE: &str = "hoistable-import:pkg.use.f:from pkg import dep";

#[test]
fn hoist_splice_leaves_its_line_and_rides_to_the_top() {
    let files = dep_pkg(
        concat!(
            "import os\n\n\ndef f() -> int:\n    from pkg import dep\n",
            "    return dep.VALUE + len(os.sep)\n",
        ),
        "VALUE = 1\n",
    );
    let splice = splice_of(DEP_CAUSE, &files).expect("an inert target hoists");
    assert_eq!(splice.imports, ["from pkg import dep"]);
    assert_eq!(splice.edits[0].line, 5);
    assert_eq!(splice.edits[0].text, "");
}

#[test]
fn hoist_splice_dropped_where_the_name_already_means_something() {
    let files = dep_pkg(
        "dep = 1\n\n\ndef f() -> int:\n    from pkg import dep\n    return dep.VALUE\n",
        "VALUE = 1\n",
    );
    assert!(splice_of(DEP_CAUSE, &files).is_none());
}

#[test]
fn hoist_splice_moves_only_an_inert_import_closure() {
    let cases: [(&str, bool); 3] = [
        // a logger, a compiled pattern, a record class: inert, the hoist ships
        (
            concat!(
                "import logging\nimport re\nfrom dataclasses import dataclass\n",
                "LOG = logging.getLogger(__name__)\nPAT = re.compile('x')\n",
                "@dataclass(frozen=True)\nclass Rec:\n    n: int = 0\nVALUE = 1\n",
            ),
            true,
        ),
        // a call outside the catalog reads the world at import: no patch
        ("import os\nHOME = os.getenv('HOME')\nVALUE = 1\n", false),
        // `Path(...)` at import builds a value
        (
            "from pathlib import Path\nHERE = Path('.')\nVALUE = 1\n",
            true,
        ),
    ];
    for (dep, patched) in cases {
        let files = dep_pkg(USE_DEP, dep);
        assert_eq!(splice_of(DEP_CAUSE, &files).is_some(), patched, "{dep}");
    }
}
