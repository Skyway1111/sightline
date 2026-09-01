//! `tests/rules/test_context.py`: #24, #26, #27, #29, #36, #38, #59.

use sightline_core::findings::Finding;
use sightline_testkit::run_rule;

/// `run_rule` over a tree a test built out of owned strings.
fn run(id: &str, files: &[(String, String)]) -> Vec<Finding> {
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(rel, src)| (rel.as_str(), src.as_str()))
        .collect();
    run_rule(id, &refs)
}

fn repeat(n: usize, line: impl Fn(usize) -> String) -> String {
    (0..n).map(line).collect()
}

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

fn symbols(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| &*f.site.symbol).collect()
}

// --- #24 dynamic identifiers -------------------------------------------------

#[test]
fn rule_24_fires_on_dynamic_name_construction() {
    let findings = run_rule(
        "24",
        &[(
            "m.py",
            concat!(
                "import importlib\n",
                "def dispatch(obj, kind):\n",
                "    fn = getattr(obj, f'handle_{kind}')\n",
                "    mod = importlib.import_module('plugins.' + kind)\n",
                "    return fn, mod, globals()[f'_{kind}_table']\n",
            ),
        )],
    );
    let mut forms: Vec<&str> = findings
        .iter()
        .map(|f| f.cause.split(':').nth(1).expect("a form segment"))
        .collect();
    forms.sort_unstable();
    forms.dedup();
    assert_eq!(forms, ["getattr", "globals", "import_module"]);
}

/// Nothing is assembled here: the name is a loop variable over a literal
/// table, a pair the runtime hands back, a parameter, or an attribute the data
/// holds. A receiver's own members are enumerable in one class. A name built
/// here still fires.
#[test]
fn rule_24_a_name_the_data_or_the_caller_supplies_is_silent() {
    let findings = run_rule(
        "24",
        &[(
            "m.py",
            concat!(
                "import importlib\n",
                "TEMP = ['_a_cache', '_b_cache']\n",
                "def clear(obj):\n",
                "    for attr in TEMP:\n",
                "        delattr(obj, attr)\n",
                "def materialize(model):\n",
                "    for name, buf in model.named_buffers():\n",
                "        setattr(model, name, buf)\n",
                "def store(model, path, value):\n",
                "    holder, leaf = navigate(model, path)\n",
                "    setattr(holder, leaf, value)\n",
                "def read(row, key, alt):\n",
                "    col = key or alt\n",
                "    return getattr(row, col)\n",
                "def field(obj, spec):\n",
                "    return getattr(obj, spec.encode_name)\n",
                "def check():\n",
                "    for mod in ('pkg.a', 'pkg.b'):\n",
                "        importlib.import_module(mod)\n",
                "class V:\n",
                "    def handler(self, domain, rule):\n",
                "        return getattr(self, '_{0}_{1}'.format(domain, rule))\n",
                "def built(err, kind):\n",
                "    return getattr(err, kind + '_path')\n",
                "def built_above(cfg, kind):\n",
                "    attr = f'_config_{kind}'\n",
                "    setattr(cfg, attr, 1)\n",
            ),
        )],
    );
    assert_eq!(symbols(&findings), ["m.built", "m.built_above"]);
}

/// `getattr` on a repo module and a `globals()[name]` dispatch resolve a name
/// the repo spells somewhere grep cannot follow. Silent: a literal, an
/// external module's attribute, and a dynamic import of an arriving path.
#[test]
fn rule_24_a_module_namespace_resolves_an_arriving_name_too() {
    let findings = run_rule(
        "24",
        &[
            ("pkg/__init__.py", ""),
            ("pkg/metrics.py", "def mae():\n    return 1\n"),
            (
                "m.py",
                concat!(
                    "import importlib\n",
                    "import logging\n",
                    "import os\n",
                    "import sys\n",
                    "from pkg import metrics\n",
                    "def level():\n",
                    "    return getattr(metrics, os.environ['METRIC'])\n",
                    "def external():\n",
                    "    return getattr(logging, os.environ['LEVEL'])\n",
                    "def metric(name):\n",
                    "    return globals()[name]\n",
                    "def load(path):\n",
                    "    return importlib.import_module(path)\n",
                    "def plugin(modname):\n",
                    "    return __import__(modname)\n",
                    "def cls(path, name):\n",
                    "    return getattr(sys.modules[path], name)\n",
                    "def fixed():\n",
                    "    return getattr(logging, 'INFO'), importlib.import_module('pkg.a')\n",
                    "def table():\n",
                    "    return [getattr(logging, n) for n in ('INFO', 'DEBUG')]\n",
                ),
            ),
        ],
    );
    assert_eq!(symbols(&findings), ["m.level", "m.metric"]);
}

/// A whole program text is not a name a reader greps for.
#[test]
fn rule_24_exec_and_eval_are_not_identifiers() {
    let findings = run_rule(
        "24",
        &[(
            "m.py",
            "def run(script, expr):\n    exec(script, {})\n    return eval(expr)\n",
        )],
    );
    assert!(findings.is_empty());
}

#[test]
fn rule_24_silent_on_constant_names() {
    let findings = run_rule(
        "24",
        &[(
            "m.py",
            concat!(
                "import importlib\n",
                "def load(obj):\n",
                "    h = getattr(obj, 'handle', None)\n",
                "    mod = importlib.import_module('plugins.core')\n",
                "    return h, mod\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

// --- #26 declaration literalness ---------------------------------------------

#[test]
fn rule_26_fires_on_dynamic_all_star_import_computed_constant() {
    let findings = run_rule(
        "26",
        &[
            ("pkg/__init__.py", ""),
            ("pkg/base.py", "A = 1\n"),
            (
                "m.py",
                concat!(
                    "from pkg.base import *\n",
                    "__all__ = ['a'] + ['b']\n",
                    "RAW = ['x', 'y']\n",
                    "FEATURES = sorted(p.upper() for p in RAW)\n",
                ),
            ),
        ],
    );
    let mut forms: Vec<&str> = findings
        .iter()
        .map(|f| f.cause.split(':').next().expect("an arm segment"))
        .collect();
    forms.sort_unstable();
    forms.dedup();
    assert_eq!(
        forms,
        ["computed-declaration", "dynamic-all", "star-import"]
    );
}

/// A transition package whose body is imports and an `__all__` delegated to
/// the sibling it re-exports: the star import is the declaration.
#[test]
fn rule_26_a_reexport_shim_is_silent() {
    let findings = run_rule(
        "26",
        &[
            ("pkg/__init__.py", "__all__ = ['A']\nA = 1\n"),
            (
                "old/__init__.py",
                concat!(
                    "\"\"\"Deprecated: import pkg instead.\"\"\"\n",
                    "import pkg\n",
                    "from pkg import *\n",
                    "__all__ = pkg.__all__\n",
                ),
            ),
            ("m.py", "from pkg import *\nWIDGETS = ['a']\n"),
        ],
    );
    assert_eq!(symbols(&findings), ["m"]);
}

/// Nested constructor calls whose leaves are literals or references read
/// top-down: declaration-literal, not assembled by code.
#[test]
fn rule_26_transparent_constructors_silent() {
    let findings = run_rule(
        "26",
        &[(
            "m.py",
            concat!(
                "import datetime\n",
                "import vol\n",
                "SCAN_INTERVAL = datetime.timedelta(minutes=5)\n",
                "SCHEMA = vol.Schema({vol.Required('host'): str})\n",
                "DEFAULTS = Config(name='x', size=3)\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

/// Real assemble-by-code: an empty init mutated at module level.
#[test]
fn rule_26_module_level_assembly_fires() {
    let findings = run_rule(
        "26",
        &[(
            "m.py",
            "CRC_TABLE = []\nfor i in range(256):\n    CRC_TABLE.append(i * 7)\n",
        )],
    );
    assert_eq!(causes(&findings), ["computed-declaration:m.CRC_TABLE"]);
}

/// `Path(__file__).parent / "documents"` says where the module is, not what
/// it holds; a comprehension-built member list still fires.
#[test]
fn rule_26_a_path_anchored_at_file_is_not_a_member_list() {
    let findings = run_rule(
        "26",
        &[(
            "m.py",
            concat!(
                "from pathlib import Path\n",
                "import os\n",
                "DOCUMENTS_PATH = Path(__file__).parent / 'documents'\n",
                "DATA_DIR = os.path.join(os.path.dirname(__file__), 'data')\n",
                "RAW = ['x']\n",
                "MODELS = [m.upper() for m in RAW]\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["computed-declaration:m.MODELS"]);
}

#[test]
fn rule_26_dynamic_all_in_try_detected() {
    let findings = run_rule(
        "26",
        &[(
            "m.py",
            concat!(
                "try:\n",
                "    import extras\n",
                "    __all__ = ['run'] + extras.NAMES\n",
                "except ImportError:\n",
                "    __all__ = ['run']\n",
            ),
        )],
    );
    assert!(findings.iter().any(|f| f.cause == "dynamic-all:m"));
}

#[test]
fn rule_26_derived_constants_and_test_fixtures_pair() {
    // scalar derivation keeps one source of truth: silent
    let findings = run_rule(
        "26",
        &[("m.py", "TIMEOUT_S = 30\nTIMEOUT_MS = TIMEOUT_S * 1000\n")],
    );
    assert!(findings.is_empty());
    // container assembly still fires
    let findings = run_rule(
        "26",
        &[("m.py", "RAW = ['a']\nPATTERNS = [p.upper() for p in RAW]\n")],
    );
    assert_eq!(causes(&findings), ["computed-declaration:m.PATTERNS"]);
    // computed constants in test files are test data: silent
    let findings = run_rule(
        "26",
        &[("tests/test_data.py", "EXPECTED = sorted(set(['x', 'y']))\n")],
    );
    assert!(findings.iter().all(|f| !f.site.rel.starts_with("tests/")));
}

#[test]
fn rule_26_silent_on_literal_declarations() {
    let findings = run_rule(
        "26",
        &[(
            "m.py",
            concat!(
                "from pkg import base\n",
                "__all__ = ['run']\n",
                "FEATURES = ['x', 'y']\n",
                // a literal-arg constructor, then a named data load
                "LIMITS = frozenset({'a', 'b'})\n",
                "SOURCES = base.load_sources('Diana')\n",
                "def run():\n    pass\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

// --- #27 purchase price -------------------------------------------------------

fn big() -> String {
    "def hot():\n    return 1\ndef warm():\n    return 2\n".to_string()
        + &repeat(250, |i| format!("def filler_{i}():\n    return {i}\n"))
}

fn mid() -> String {
    "def hot():\n    return 1\ndef warm():\n    return 2\n".to_string()
        + &repeat(220, |i| format!("def filler_{i}():\n    return {i}\n"))
}

fn one_class() -> String {
    "class Big:\n".to_string()
        + &repeat(260, |i| {
            format!("    def m{i}(self):\n        return {i}\n")
        })
}

fn hot_warm_users(n: usize, home: &str) -> Vec<(String, String)> {
    (0..n)
        .map(|j| {
            (
                format!("user_{j}.py"),
                format!("from {home} import hot, warm\ndef u{j}():\n    return hot(), warm()\n"),
            )
        })
        .collect()
}

#[test]
fn rule_27_one_finding_per_module_naming_its_hot_symbols() {
    let mut files = vec![("big.py".to_string(), big())];
    files.extend(hot_warm_users(3, "big"));
    let findings = run("27", &files);
    // once, not per symbol
    assert_eq!(causes(&findings), ["price:big"]);
    let f = &findings[0];
    assert_eq!((&*f.site.symbol, f.site.line, f.site.col), ("big", 1, 0));
    assert_eq!(
        f.message,
        "big is 504 lines holding 2 hot symbols, led by hot (6), warm (6) - \
         every reader pays the whole file"
    );
    // price x inbound cross-module refs
    assert_eq!(f.salience, 504.0 * 12.0);
}

/// Every judged module under 500 lines reads as one cohesive unit.
#[test]
fn rule_27_a_module_under_the_price_bar_is_one_read() {
    let mut files = vec![("mid.py".to_string(), mid())];
    files.extend(hot_warm_users(3, "mid"));
    assert!(run("27", &files).is_empty());
}

/// Every hot symbol is one top-level class or a member of it: the module
/// already is the smallest unit of its concept.
#[test]
fn rule_27_a_module_that_is_one_class_has_nothing_to_lift() {
    let mut files = vec![("big.py".to_string(), one_class())];
    files.extend((0..3).map(|j| {
        (
            format!("user_{j}.py"),
            format!("from big import Big\ndef u{j}():\n    return Big(), Big()\n"),
        )
    }));
    assert!(run("27", &files).is_empty());
}

#[test]
fn rule_27_a_second_hot_owner_beside_the_class_fires() {
    let mut files = vec![(
        "big.py".to_string(),
        one_class() + "def helper():\n    return 0\n",
    )];
    files.extend((0..3).map(|j| {
        (
            format!("user_{j}.py"),
            format!(
                "from big import Big, helper\ndef u{j}():\n    return Big(), helper(), Big(), helper()\n"
            ),
        )
    }));
    assert_eq!(causes(&run("27", &files)), ["price:big"]);
}

#[test]
fn rule_27_silent_below_the_price_and_fan_in_bars() {
    let findings = run_rule(
        "27",
        &[
            ("small.py", "def hot():\n    return 1\n"),
            (
                "user.py",
                "from small import hot\ndef u():\n    return hot()\n",
            ),
        ],
    );
    assert!(findings.is_empty());
    // priced, but no symbol is hot
    let files = vec![
        ("big.py".to_string(), big()),
        (
            "user.py".to_string(),
            "from big import hot\ndef u():\n    return hot\n".to_string(),
        ),
    ];
    assert!(run("27", &files).is_empty());
}

// --- #27 fan-out --------------------------------------------------------------

const READS: &str = "from hub import run\nRESULT = run()\n";
/// every reference sits under a main or a `__name__` guard
const LAUNCHES: &str = concat!(
    "import hub\n\ndef main():\n    return hub.run()\n\n",
    "if __name__ == '__main__':\n    from hub import run\n    run()\n"
);

/// `importer` imports n distinct internal modules: n-1 at the top, the last
/// inside a def, one of the top ones a second time there. `readers` modules
/// import it, `chain[0]` imports them and the rest of `chain` follows link by
/// link: a reader is an importer some imported prod module loads in turn.
fn fan_out_tree(
    n: usize,
    importer: &str,
    user: &str,
    chain: &[&str],
    readers: usize,
) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = (0..n)
        .map(|i| {
            (
                format!("pkg/m{i}.py"),
                format!("def f{i}():\n    return {i}\n"),
            )
        })
        .collect();
    files.push(("pkg/__init__.py".to_string(), String::new()));
    files.push((
        importer.to_string(),
        repeat(n - 1, |i| format!("import pkg.m{i}\n"))
            + &format!(
                "def run():\n    import pkg.m{}\n    import pkg.m0\n    return pkg.m0.f0()\n",
                n - 1
            ),
    ));
    let hub = importer
        .strip_suffix(".py")
        .unwrap_or(importer)
        .replace('/', ".");
    let hub = hub.strip_suffix(".__init__").unwrap_or(&hub).to_string();
    for k in 0..readers {
        files.push((format!("user{k}.py"), user.replace("hub", &hub)));
    }
    if let Some(first) = chain.first() {
        files.push((
            (*first).to_string(),
            repeat(readers, |k| format!("import user{k}\n")),
        ));
        for pair in chain.windows(2) {
            let below = pair[0]
                .strip_suffix(".py")
                .unwrap_or(pair[0])
                .replace('/', ".");
            files.push((pair[1].to_string(), format!("import {below}\n")));
        }
    }
    files
}

fn default_tree(n: usize) -> Vec<(String, String)> {
    fan_out_tree(n, "hub.py", READS, &["app.py", "wsgi.py"], 2)
}

#[test]
fn rule_27_ten_distinct_internal_modules_at_any_scope_fire() {
    let findings = run("27", &default_tree(10));
    let rows: Vec<(&str, u32, f64)> = findings
        .iter()
        .map(|f| (f.cause.as_str(), f.site.line, f.salience))
        .collect();
    assert_eq!(rows, [("fan-out:hub", 1, 10.0)]);
    assert_eq!(
        findings[0].message,
        "hub imports 10 internal modules - a reader loads 10 files to read one"
    );
}

#[test]
fn rule_27_nine_and_a_package_init_stay_silent() {
    assert!(run("27", &default_tree(9)).is_empty());
    let files = fan_out_tree(10, "pkg/__init__.py", READS, &["app.py", "wsgi.py"], 2);
    assert!(run("27", &files).is_empty());
}

// a reader is where the arm's cost is paid; each twin below lacks one

#[test]
fn rule_27_a_launcher_is_not_a_reader() {
    let files = fan_out_tree(10, "hub.py", LAUNCHES, &["app.py", "wsgi.py"], 2);
    assert!(run("27", &files).is_empty());
}

/// The module its single parent composes is where a reader starts.
#[test]
fn rule_27_one_reader_is_a_composition_parent() {
    let files = fan_out_tree(10, "hub.py", READS, &["app.py", "wsgi.py"], 1);
    assert!(run("27", &files).is_empty());
}

/// The checker's edge, never the interpreter's: nine files load.
#[test]
fn rule_27_a_type_checking_only_import_is_not_a_load() {
    let mut files = default_tree(10);
    let hub = "from typing import TYPE_CHECKING\n".to_string()
        + &repeat(9, |i| format!("import pkg.m{i}\n"))
        + "if TYPE_CHECKING:\n    import pkg.m9\n"
        + "def run():\n    import pkg.m0\n    return pkg.m0.f0()\n";
    for entry in &mut files {
        if entry.0 == "hub.py" {
            entry.1 = hub.clone();
        }
    }
    assert!(run("27", &files).is_empty());
}

/// A bench, a script, a test: where a reader starts, not what they load.
#[test]
fn rule_27_an_importer_nobody_loads_is_not_a_reader() {
    assert!(run("27", &fan_out_tree(10, "hub.py", READS, &[], 2)).is_empty());
    let files = fan_out_tree(10, "hub.py", READS, &["tests/test_user.py"], 2);
    assert!(run("27", &files).is_empty());
}

/// What the app loads is read; the app itself is not a reader.
#[test]
fn rule_27_an_importer_only_roots_load_is_driven_not_read() {
    let files = fan_out_tree(10, "hub.py", READS, &["bench.py"], 2);
    assert!(run("27", &files).is_empty());
}

#[test]
fn rule_27_an_import_nothing_loads_is_not_a_read() {
    let files = fan_out_tree(10, "hub.py", "import hub\n", &["app.py", "wsgi.py"], 2);
    assert!(run("27", &files).is_empty());
}

// --- #29 top-loading ----------------------------------------------------------

#[test]
fn rule_29_fires_on_undocumented_module() {
    let source = "def a():\n    pass\ndef b():\n    pass\ndef c():\n    pass\ndef heavy(path):\n"
        .to_string()
        + &repeat(150, |i| format!("    x{i} = {i}\n"))
        + "    return open(path).read()\n";
    assert_eq!(
        causes(&run("29", &[("m.py".to_string(), source)])),
        ["top-loading:m"]
    );
}

/// Small files are never punished: the module arm needs real size.
#[test]
fn rule_29_small_modules_never_fire() {
    let findings = run_rule(
        "29",
        &[(
            "m.py",
            "def a():\n    pass\ndef b():\n    pass\ndef c():\n    pass\ndef d():\n    pass\n",
        )],
    );
    assert!(findings.is_empty());
}

/// The first screen says what the module is: only the form differs from a
/// docstring. A licence header, a section banner under a gap and a tool
/// directive say nothing about the module.
#[test]
fn rule_29_a_leading_comment_block_is_the_map() {
    let body = repeat(160, |i| format!("x{i} = {i}\n"));
    let files = vec![
        (
            "mapped.py".to_string(),
            "#!/usr/bin/python\n#\n# Cityscapes labels\n#\n".to_string() + &body,
        ),
        (
            "licensed.py".to_string(),
            "# Copyright (c) 2025 Acme Ltd\n# Licensed under the Apache License, Version 2.0\n"
                .to_string()
                + &body,
        ),
        (
            "banner.py".to_string(),
            "#!/usr/bin/env python\n# -*- coding: utf-8 -*-\n\n\n#######\n# Import modules\n#######\n"
                .to_string()
                + &body,
        ),
        (
            "pragma.py".to_string(),
            "# ruff: noqa: PLR0133\n".to_string() + &body,
        ),
    ];
    assert_eq!(
        symbols(&run("29", &files)),
        ["banner", "licensed", "pragma"]
    );
}

/// The module top-loading arm skips test modules, as #59 does.
#[test]
fn rule_29_toploading_arm_skips_test_files() {
    let findings = run_rule(
        "29",
        &[(
            "tests/test_mod.py",
            "def test_a():\n    pass\ndef test_b():\n    pass\ndef test_c():\n    pass\n",
        )],
    );
    assert!(findings.is_empty());
}

// --- #59 cost docstring -------------------------------------------------------

fn heavy(n: usize) -> String {
    repeat(n, |i| format!("    x{i} = {i}\n"))
}

#[test]
fn rule_59_fires_on_a_heavy_entry_that_spends() {
    let source = "\"\"\"Documented module.\"\"\"\nimport shutil\ndef heavy(path):\n".to_string()
        + &heavy(150)
        + "    return shutil.rmtree(path)\n";
    assert_eq!(
        causes(&run("59", &[("m.py".to_string(), source)])),
        ["cost-docstring:m.heavy"]
    );
}

/// A switch, a registration list and a long prompt literal are long and spend
/// nothing: line count is not a cost.
#[test]
fn rule_59_a_long_body_that_only_computes_declares_nothing() {
    let source = "\"\"\"Documented module.\"\"\"\ndef render(kind):\n    parts = []\n".to_string()
        + &repeat(40, |i| format!("    parts.append('line {i}')\n"))
        + "    return kind, parts\n";
    assert!(run("59", &[("m.py".to_string(), source)]).is_empty());
}

/// The caller cannot see it either way; the file shows it.
#[test]
fn rule_59_a_spend_through_a_helper_in_the_same_file_counts() {
    let source = concat!(
        "\"\"\"Documented module.\"\"\"\n",
        "import subprocess\n",
        "def _retry():\n    subprocess.run(['x'])\n",
        "def collect(n):\n",
    )
    .to_string()
        + &heavy(31)
        + "    return _retry()\n";
    assert_eq!(
        causes(&run("59", &[("m.py".to_string(), source)])),
        ["cost-docstring:m.collect"]
    );
}

/// The near-miss twin calls a callable it was given, which resolves to no
/// body and makes no claim.
#[test]
fn rule_59_a_spend_one_repo_call_past_the_file_counts() {
    let body = heavy(31);
    let files = vec![
        (
            "llm.py".to_string(),
            concat!(
                "\"\"\"Documented module.\"\"\"\n",
                "import requests\n",
                "def post_query(prompt):\n    return requests.post(prompt)\n",
            )
            .to_string(),
        ),
        (
            "m.py".to_string(),
            "\"\"\"Documented module.\"\"\"\nfrom llm import post_query\ndef judge(answer):\n"
                .to_string()
                + &body
                + "    return post_query(answer)\n"
                + "def clear(answer, send):\n"
                + &body
                + "    return send(answer)\n",
        ),
    ];
    let findings = run("59", &files);
    assert_eq!(causes(&findings), ["cost-docstring:m.judge"]);
    assert!(
        findings[0]
            .message
            .contains("llm.post_query -> requests.post")
    );
}

/// Each judge calls a helper in its own file, whose spend is a call in
/// another module. The twin's helper calls a repo function that only computes.
#[test]
fn rule_59_a_same_file_helpers_repo_callee_is_the_second_hop() {
    let body = repeat(31, |i| format!("        x{i} = {i}\n"));
    let files = vec![
        (
            "llm.py".to_string(),
            concat!(
                "\"\"\"Documented module.\"\"\"\n",
                "import requests\n",
                "def post_query(prompt):\n    return requests.post(prompt)\n",
                "def shape(prompt):\n    return prompt.strip()\n",
            )
            .to_string(),
        ),
        (
            "m.py".to_string(),
            concat!(
                "\"\"\"Documented module.\"\"\"\n",
                "from llm import post_query, shape\n",
                "class Judge:\n",
                "    def judge(self, answer):\n",
            )
            .to_string()
                + &body
                + "        return self._fallback(answer)\n"
                + "    def _fallback(self, answer):\n        return post_query(answer)\n"
                + "    def clear(self, answer):\n"
                + &body
                + "        return self._tidy(answer)\n"
                + "    def _tidy(self, answer):\n        return shape(answer)\n",
        ),
    ];
    let findings = run("59", &files);
    assert_eq!(causes(&findings), ["cost-docstring:m.Judge.judge"]);
    assert!(
        findings[0]
            .message
            .contains("llm.post_query -> requests.post")
    );
}

/// The helper spends on something the body built, so the cost is the body's.
/// The twin hands its own parameter through, and that cost is in its
/// signature.
#[test]
fn rule_59_a_helper_spending_what_the_body_built_is_the_bodys_cost() {
    let body = heavy(31);
    let source = concat!(
        "\"\"\"Documented module.\"\"\"\n",
        "import workers\n",
        "def fan_out(pool, rows):\n    return pool.submit(rows)\n",
        "def run_all(rows):\n",
    )
    .to_string()
        + &body
        + "    pool = workers.pool()\n    return fan_out(pool, rows)\n"
        + "def run_with(pool, rows):\n"
        + &body
        + "    return fan_out(pool, rows)\n";
    let findings = run("59", &[("m.py".to_string(), source)]);
    assert_eq!(causes(&findings), ["cost-docstring:m.run_all"]);
    assert!(findings[0].message.contains("fan_out -> submit"));
}

/// A def whose only caller sits under the main guard is the script's entry -
/// `main` by another name. The twin is also called from prod code.
#[test]
fn rule_59_a_defs_only_caller_under_the_main_guard_is_the_scripts_entry() {
    let body = heavy(31);
    let files = vec![
        (
            "m.py".to_string(),
            "\"\"\"Walk-forward evaluation (~10 min); prints a table.\"\"\"\nimport sqlite3\ndef run():\n"
                .to_string()
                + &body
                + "    return sqlite3.connect('x')\n"
                + "def collect():\n"
                + &body
                + "    return sqlite3.connect('y')\n"
                + "if __name__ == '__main__':\n    run()\n    collect()\n",
        ),
        (
            "n.py".to_string(),
            "from m import collect\ndef go():\n    return collect()\n".to_string(),
        ),
    ];
    assert_eq!(causes(&run("59", &files)), ["cost-docstring:m.collect"]);
}

/// A callback on a class the body built: nothing outside the function can
/// name it, so no caller budgets for it.
#[test]
fn rule_59_a_method_of_a_class_a_body_defines_is_a_closure_too() {
    let source = concat!(
        "\"\"\"Documented module.\"\"\"\n",
        "import sqlite3\n",
        "def serve():\n",
        "    \"\"\"Documented entry.\"\"\"\n",
        "    class Handler:\n",
        "        def do_POST(self):\n",
    )
    .to_string()
        + &repeat(31, |i| format!("            x{i} = {i}\n"))
        + "            return sqlite3.connect('db')\n    return Handler\n";
    assert!(run("59", &[("m.py".to_string(), source)]).is_empty());
}

/// What is left is what a caller cannot walk back: another machine, another
/// process, deleted data.
#[test]
fn rule_59_local_file_work_is_not_a_spend() {
    let body = heavy(31);
    let source =
        "\"\"\"Documented module.\"\"\"\nimport glob, json, shutil\ndef report(rows, out):\n"
            .to_string()
            + &body
            + "    json.dump(rows, open(out, 'w'))\n"
            + "def scan(root):\n"
            + &body
            + "    return glob.glob(root)\n"
            + "def reset(root):\n"
            + &body
            + "    return shutil.rmtree(root)\n";
    assert_eq!(
        causes(&run("59", &[("m.py".to_string(), source)])),
        ["cost-docstring:m.reset"]
    );
}

/// The module docstring enumerates the phases and `main()` under the guard is
/// their only caller. The third hop is a helper of its own.
#[test]
fn rule_59_a_phase_main_alone_calls_is_the_scripts_entry_too() {
    let body = heavy(31);
    let source =
        "\"\"\"Two phases: launch the backend, then drive it.\"\"\"\nimport sqlite3\ndef prelude():\n"
            .to_string()
            + &body
            + "    return sqlite3.connect('c')\n"
            + "def phase_one():\n"
            + &body
            + "    return prelude(), sqlite3.connect('a')\n"
            + "def main():\n    return phase_one()\n"
            + "if __name__ == '__main__':\n    main()\n";
    assert_eq!(
        causes(&run("59", &[("m.py".to_string(), source)])),
        ["cost-docstring:m.prelude"]
    );
}

/// A header block under the shebang documents a script; the twin's header is
/// only the shebang and the coding cookie, and a checker directive is
/// machinery, not the file's first screen.
#[test]
fn rule_59_a_leading_comment_block_is_the_modules_first_screen() {
    let script = "import sqlite3\ndef main():\n".to_string()
        + &heavy(31)
        + "    return sqlite3.connect('db')\nif __name__ == '__main__':\n    main()\n";
    let files = vec![
        (
            "m.py".to_string(),
            "#!/usr/bin/python\n#\n# Converts the annotations to images, one per city.\n"
                .to_string()
                + &script,
        ),
        (
            "n.py".to_string(),
            "#!/usr/bin/python\n# -*- coding: utf-8 -*-\n".to_string() + &script,
        ),
        (
            "o.py".to_string(),
            "# ruff: noqa: PLR0133\n# Copyright (c) 2024 no one\n".to_string() + &script,
        ),
    ];
    let findings = run("59", &files);
    let mut found = causes(&findings);
    found.sort_unstable();
    assert_eq!(found, ["cost-docstring:n.main", "cost-docstring:o.main"]);
}

/// `-> Thread` says a thread is spawned, a `poll` parameter says the body
/// waits; the twin has neither.
#[test]
fn rule_59_a_signature_that_spells_the_spend_is_the_docstring() {
    let body = heavy(31);
    let source = concat!(
        "\"\"\"Documented module.\"\"\"\n",
        "import threading\n",
        "from threading import Thread\n",
        "def start_watcher(stop) -> Thread:\n",
    )
    .to_string()
        + &body
        + "    return threading.Thread(target=stop)\n"
        + "def resolve(ids, poll_timeout_secs=None):\n"
        + &body
        + "    return threading.Thread(target=ids)\n"
        + "def start_plain(stop):\n"
        + &body
        + "    return threading.Thread(target=stop)\n";
    assert_eq!(
        causes(&run("59", &[("m.py".to_string(), source)])),
        ["cost-docstring:m.start_plain"]
    );
}

/// `torch.cat` in a forward builds a tensor. The twin enumerates the devices,
/// which starts the driver.
#[test]
fn rule_59_tensor_math_is_the_files_subject_not_a_hidden_cost() {
    let body = heavy(31);
    let source = "\"\"\"Documented module.\"\"\"\nimport torch\ndef forward(a, b):\n".to_string()
        + &body
        + "    return torch.cat([a, b], dim=-1)\n"
        + "def define_schema():\n"
        + &body
        + "    return torch.cuda.device_count()\n";
    assert_eq!(
        causes(&run("59", &[("m.py".to_string(), source)])),
        ["cost-docstring:m.define_schema"]
    );
}

#[test]
fn rule_59_silent_on_documented_or_small() {
    let source = concat!(
        "\"\"\"Documented module.\"\"\"\n",
        "def heavy():\n",
        "    \"\"\"Costs one pass over everything.\"\"\"\n",
    )
    .to_string()
        + &heavy(31)
        + "    return x0\n";
    let files = vec![
        ("m.py".to_string(), source),
        ("tiny.py".to_string(), "def only():\n    pass\n".to_string()),
    ];
    assert!(run("59", &files).is_empty());
}

#[test]
fn rule_59_skips_test_files() {
    let source = "\"\"\"Documented test module.\"\"\"\ndef test_heavy():\n".to_string()
        + &heavy(31)
        + "    assert x0 == 0\n";
    assert!(run("59", &[("tests/test_big.py".to_string(), source)]).is_empty());
}

/// `opener.urlopen(url)` on an opener the caller passed is a cost the
/// signature already shows; the twin opens one the body picked.
#[test]
fn rule_59_a_spend_on_what_the_caller_handed_in_is_in_the_signature() {
    let body = heavy(31);
    let source = "\"\"\"Documented module.\"\"\"\nimport urllib.request\ndef build(opener, url):\n"
        .to_string()
        + &body
        + "    return opener.urlopen(url)\n"
        + "def build_here(url):\n"
        + &body
        + "    return urllib.request.urlopen(url)\n";
    assert_eq!(
        causes(&run("59", &[("m.py".to_string(), source)])),
        ["cost-docstring:m.build_here"]
    );
}

/// A heavy nested helper is not an entry point a reader budgets for.
#[test]
fn rule_59_skips_closures() {
    let source = concat!(
        "\"\"\"Documented module.\"\"\"\n",
        "def outer():\n",
        "    \"\"\"Documented entry.\"\"\"\n",
        "    def worker():\n",
    )
    .to_string()
        + &repeat(31, |i| format!("        y{i} = {i}\n"))
        + "        return y0\n    return worker\n";
    assert!(run("59", &[("m.py".to_string(), source)]).is_empty());
}

/// A table-registered fn is documented at the table, and a script's `main` by
/// the module docstring; a bare `main` is neither.
#[test]
fn rule_59_registered_fn_and_documented_scripts_main_are_not_entry_points() {
    let heavy_body = heavy(31) + "    os.remove('f')\n";
    let table = "import os\ndef rule_x(a):\n".to_string()
        + &heavy_body
        + "    return x1\nRULES = (rule_x,)\n";
    assert!(run("59", &[("m.py".to_string(), table)]).is_empty());
    let script = "\"\"\"Usage: m.py\"\"\"\nimport os\ndef main(argv):\n".to_string()
        + &heavy_body
        + "    return 0\n";
    assert!(run("59", &[("m.py".to_string(), script)]).is_empty());
    let bare = "import os\ndef main(argv):\n".to_string() + &heavy_body + "    return 0\n";
    assert_eq!(
        causes(&run("59", &[("m.py".to_string(), bare)])),
        ["cost-docstring:m.main"]
    );
}

/// The router factory only registers the nested handler; the handler spends
/// when it runs. A spend-verb name is no exemption.
#[test]
fn rule_59_a_registered_handlers_spend_is_not_the_factorys() {
    let files = vec![
        (
            "m.py".to_string(),
            "import requests\ndef create_router(app):\n".to_string()
                + &repeat(30, |i| format!("    x{i} = {i}\n"))
                + "    @app.get('/x')\n    def handler():\n        return requests.get('u')\n    return app\n"
                + "def fetch_all(app):\n"
                + &repeat(30, |i| format!("    y{i} = {i}\n"))
                + "    return requests.get('u')\n"
                + "def collect_all(app):\n"
                + &repeat(30, |i| format!("    z{i} = {i}\n"))
                + "    return requests.get('u')\n",
        ),
        (
            "main.py".to_string(),
            "from m import create_router, fetch_all, collect_all\ncreate_router(1)\nfetch_all(1)\ncollect_all(1)\n"
                .to_string(),
        ),
    ];
    assert_eq!(
        causes(&run("59", &files)),
        ["cost-docstring:m.fetch_all", "cost-docstring:m.collect_all"]
    );
}

// --- #36 type lies ------------------------------------------------------------

#[test]
fn rule_36_fires_on_dense_checker_silencing() {
    let findings = run_rule(
        "36",
        &[(
            "m.py",
            concat!(
                "def f(x):\n",
                "    a = x.go()  # type: ignore\n",
                "    b = x.run()  # pyright: ignore[attr-defined]\n",
                "    return a + b  # mypy: ignore\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["type-lies:m"]);
    assert!(findings[0].message.contains('3'));
    assert_eq!(findings[0].salience, 3.0);
}

/// The arm prices density, not a count.
#[test]
fn rule_36_three_pragmas_across_a_long_module_are_not_density() {
    let source = concat!(
        "def f(x):\n",
        "    a = x.go()  # type: ignore\n",
        "    b = x.run()  # pyright: ignore[attr-defined]\n",
        "    return a + b  # mypy: ignore\n",
    )
    .to_string()
        + &repeat(200, |i| format!("y{i} = {i}\n"));
    assert!(run("36", &[("m.py".to_string(), source)]).is_empty());
}

/// A pragma deletes the diagnostic; a cast is a typed claim the checker keeps
/// checking every other expression against.
#[test]
fn rule_36_casts_are_a_claim_not_a_silencing() {
    let findings = run_rule(
        "36",
        &[(
            "m.py",
            "from typing import cast\ndef f(x):\n    return cast(int, x), cast(str, x), cast(float, x)\n",
        )],
    );
    assert!(findings.is_empty());
}

/// A fixture's ignores blind no prover.
#[test]
fn rule_36_sparse_pragmas_and_a_test_fixtures_ignores_silent() {
    let findings = run_rule(
        "36",
        &[
            ("m.py", "def f(x):\n    return x.go()  # type: ignore\n"),
            (
                "tests/test_m.py",
                concat!(
                    "def test_f(x):\n",
                    "    a = x.go()  # type: ignore\n",
                    "    b = x.run()  # type: ignore\n",
                    "    assert a + b  # type: ignore\n",
                ),
            ),
        ],
    );
    assert!(findings.is_empty());
}

// --- #38 value duplication ----------------------------------------------------

#[test]
fn rule_38_same_string_in_three_modules_fires_everywhere() {
    let findings = run_rule(
        "38",
        &[
            ("a.py", "ENDPOINT = 'api/v2/rows'\n"),
            ("b.py", "ROWS_URL: str = 'api/v2/rows'\n"),
            ("c.py", "PATH = 'api/v2/rows'\n"),
        ],
    );
    assert_eq!(findings.len(), 3);
    let mut group = causes(&findings);
    group.sort_unstable();
    group.dedup();
    assert_eq!(group.len(), 1);
    assert!(findings[0].message.contains("3 modules"));
}

/// Numbers are never judged: coincidentally-equal domain facts are not one
/// fact.
#[test]
fn rule_38_numbers_short_strings_two_homes_and_tests_silent() {
    let findings = run_rule(
        "38",
        &[
            (
                "a.py",
                "N = 30\nEMPTY = ''\nOK = 'ab'\nURL = 'api/v2/rows'\n",
            ),
            (
                "b.py",
                "N = 30\nEMPTY = ''\nOK = 'ab'\nURL = 'api/v2/rows'\n",
            ),
            ("c.py", "N = 30\nEMPTY = ''\nOK = 'ab'\n"),
            ("tests/test_x.py", "URL = 'api/v2/rows'\n"),
        ],
    );
    assert!(findings.is_empty());
}

/// A directory with its own manifest is a home, not a duplicate: the copies
/// cannot share one. Three copies inside one bundle still fire.
#[test]
fn rule_38_bundles_that_ship_on_their_own_each_keep_a_copy() {
    let mut spread: Vec<(String, String)> = (0..3)
        .map(|i| {
            (
                format!("skills/s{i}/scripts/run.py"),
                "MODEL = 'glm-5v-turbo'\n".to_string(),
            )
        })
        .collect();
    spread.extend((0..3).map(|i| (format!("skills/s{i}/SKILL.md"), "# skill\n".to_string())));
    assert!(run("38", &spread).is_empty());

    let mut together: Vec<(String, String)> = (0..3)
        .map(|i| {
            (
                format!("skills/one/scripts/r{i}.py"),
                "MODEL = 'glm-5v-turbo'\n".to_string(),
            )
        })
        .collect();
    together.push(("skills/one/SKILL.md".to_string(), "# skill\n".to_string()));
    assert_eq!(run("38", &together).len(), 3);
}

/// Five `ENCODING = "utf-8"` homes is the goal text verbatim.
#[test]
fn rule_38_a_universal_literal_still_has_one_home() {
    let files: Vec<(String, String)> = (0..3)
        .map(|i| (format!("m{i}.py"), "ENCODING = 'utf-8'\n".to_string()))
        .collect();
    let findings = run("38", &files);
    assert_eq!(findings.len(), 3);
    assert!(findings[0].cause.starts_with("value-dup:"));
}
