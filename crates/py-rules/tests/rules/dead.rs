//! `tests/rules/test_dead.py`: #32, #34, #56, #60 and `dead_symbol_splice`.
//!
//! file-length-ok: one test file per rule family, the shape `src/dead.rs`
//! states for the sources these tests pin.

use std::collections::BTreeSet;

use sightline_core::findings::{Evidence, Finding};
use sightline_py_rules::dead::dead_symbol_splice;
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

fn cause_set(findings: &[Finding]) -> BTreeSet<String> {
    findings.iter().map(|f| f.cause.clone()).collect()
}

fn set_of(causes: &[&str]) -> BTreeSet<String> {
    causes.iter().map(|c| (*c).to_string()).collect()
}

fn owned(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(rel, src)| ((*rel).to_string(), (*src).to_string()))
        .collect()
}

/// The same tree with one more file.
fn plus(files: &[(&str, &str)], extra: (&str, &str)) -> Vec<(String, String)> {
    let mut out = owned(files);
    out.push((extra.0.to_string(), extra.1.to_string()));
    out
}

// --- #32 dead symbols ---------------------------------------------------------

#[test]
fn rule_32_fires_on_dead_symbol_param_and_import() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "import os\n",
                    "import sys\n",
                    "def _helper(x):\n    return x\n",
                    "def work(data, extra):\n    return sys.path + [data]\n",
                    "def used():\n    return work(1, 2)\n",
                ),
            ),
            ("main.py", "from m import used\nused()\n"),
        ],
    );
    let found = cause_set(&findings);
    assert!(found.contains("dead-symbol:m._helper"));
    assert!(found.contains("dead-param:m.work:extra"));
    assert!(found.contains("dead-import:m:os"));
    assert!(
        !found
            .iter()
            .any(|c| c.contains("sys") || c.contains(":data") || c.contains("used"))
    );
}

/// A field spelled in a module-level container literal is read by string, so
/// #32 stays silent; a table in a test module names nothing. #56 keeps firing.
#[test]
fn rule_32_a_prod_string_table_names_the_symbol() {
    let table = concat!(
        "class Row:\n    energized_stacks: int = 0\n    chain_min: int = 0\n",
        "    other: int = 0\n",
        "def _proto(x):\n    return x\n",
        "def _dead(x):\n    return x\n",
        "KEYS = ('energized_stacks',)\n",
        "COVERAGE = {'r1': 'pkg.mod._proto'}\n",
    );
    let findings = run_rule(
        "32",
        &[
            ("m.py", table),
            (
                "tests/test_m.py",
                "TESTED = ('chain_min',)\ndef test_it():\n    assert 'other'\n",
            ),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.contains("dead-symbol:m.Row.energized_stacks"));
    assert!(!found.contains("dead-symbol:m._proto"));
    for want in [
        "dead-symbol:m.Row.chain_min",
        "dead-symbol:m.Row.other",
        "dead-symbol:m._dead",
    ] {
        assert!(found.contains(want), "{want}");
    }
    let only_tests = run_rule(
        "56",
        &[
            ("m.py", table),
            (
                "tests/test_m.py",
                "from m import _proto, _dead\ndef test_it():\n    assert _proto(1) and _dead(1)\n",
            ),
        ],
    );
    assert_eq!(
        cause_set(&only_tests),
        set_of(&["test-only:m._dead", "test-only:m._proto"])
    );
}

/// `from pkg.util import helper` elsewhere reaches util's own import alias:
/// the hop is a use of it, the alias no one hops through stays dead.
#[test]
fn rule_32_re_exported_import_is_a_use() {
    let findings = run_rule(
        "32",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/base.py",
                "def helper():\n    return 1\ndef other():\n    return 2\ndef third():\n    return 3\n",
            ),
            ("pkg/util.py", "from pkg.base import helper, other, third\n"),
            (
                "pkg/app.py",
                "from pkg.util import helper\ndef run():\n    return helper()\n",
            ),
            (
                "pkg/app2.py",
                "import pkg.util\ndef run2():\n    return pkg.util.third()\n",
            ),
            (
                "main.py",
                "from pkg.app import run\nfrom pkg.app2 import run2\nrun() + run2()\n",
            ),
        ],
    );
    assert_eq!(causes(&findings), ["dead-import:pkg.util:other"]);
}

/// `try: import x / except ImportError:` is the feature test itself; an
/// import the handler makes stays judged, and so does one outside any try.
#[test]
fn rule_32_an_availability_probe_import_is_the_test() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "try:\n    import triton\n    HAVE = True\n",
                    "except ImportError:\n    import shutil\n    HAVE = False\n",
                    "import json\n",
                    "def flag():\n    return HAVE\n",
                ),
            ),
            ("main.py", "from m import flag\nflag()\n"),
        ],
    );
    assert_eq!(
        causes(&findings),
        ["dead-import:m:shutil", "dead-import:m:json"]
    );
}

/// `from helpers import *` republishes helpers' own imports: the alias a star
/// importer loads is that re-export.
#[test]
fn rule_32_a_star_importer_reads_the_alias() {
    let findings = run_rule(
        "32",
        &[
            ("pkg/__init__.py", ""),
            ("pkg/helpers.py", "import glob\nimport json\n"),
            (
                "pkg/tool.py",
                "from pkg.helpers import *\ndef run():\n    return glob.glob('*')\n",
            ),
            ("main.py", "from pkg.tool import run\nrun()\n"),
        ],
    );
    assert_eq!(causes(&findings), ["dead-import:pkg.helpers:json"]);
}

/// A packaged module's public name is called downstream, so no in-tree
/// reference set judges it; the same tree unpackaged has every caller here.
#[test]
fn rule_32_a_distributions_public_symbol_is_its_surface() {
    let tree: [(&str, &str); 2] = [
        ("pkg/__init__.py", ""),
        (
            "pkg/api.py",
            "class Client:\n    def attach(self):\n        return 1\n    def _own(self):\n        return 2\n",
        ),
    ];
    assert!(cause_set(&run_rule("32", &tree)).contains("dead-symbol:pkg.api.Client.attach"));
    let packaged = plus(
        &tree,
        (
            "pyproject.toml",
            "[project]\nname = \"pkg\"\n\n[build-system]\nrequires = [\"setuptools\"]\n",
        ),
    );
    assert_eq!(
        causes(&run("32", &packaged)),
        ["dead-symbol:pkg.api.Client._own"]
    );
}

#[test]
fn rule_32_the_throwaway_underscore_has_nothing_to_delete() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                "def split(pair):\n    return pair\nhead, _ = split((1, 2))\ndef use():\n    return head\n",
            ),
            ("main.py", "from m import use\nuse()\n"),
        ],
    );
    assert!(findings.is_empty());
}

/// A callback a signature holds by value has its params fixed by the
/// consumer; the same def only called keeps its own.
#[test]
fn rule_32_a_def_held_as_a_default_is_registered() {
    let held = run_rule(
        "32",
        &[
            (
                "m.py",
                "def _cb(evt, extra):\n    return evt\ndef run(x, cb=_cb):\n    return cb(x)\n",
            ),
            ("main.py", "from m import run\nrun(1)\n"),
        ],
    );
    assert!(held.is_empty());
    let called = run_rule(
        "32",
        &[
            (
                "m.py",
                "def _cb(evt, extra):\n    return evt\ndef run(x):\n    return _cb(x, 1)\n",
            ),
            ("main.py", "from m import run\nrun(1)\n"),
        ],
    );
    assert_eq!(causes(&called), ["dead-param:m._cb:extra"]);
}

/// A forward reference names the symbol in a string the AST never Loads; a
/// TypeVar nothing spells anywhere is dead.
#[test]
fn rule_32_a_quoted_annotation_is_a_use() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "from typing import TypeVar\n",
                    "_T = TypeVar('_T')\n",
                    "_U = TypeVar('_U')\n",
                    "def first(xs: 'list[_T]') -> '_T | None':\n",
                    "    return xs[0] if xs else None\n",
                ),
            ),
            ("main.py", "from m import first\nfirst([])\n"),
        ],
    );
    assert_eq!(causes(&findings), ["dead-symbol:m._U"]);
}

/// A closure the body never names is dead weight like a module-level def; one
/// the body calls, returns or hands over by value is not.
#[test]
fn rule_32_nested_def_nothing_loads_is_dead() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "def run(rows):\n",
                    "    def _dead(r):\n        return r\n",
                    "    def _called(r):\n        return r\n",
                    "    def _held(r):\n        return r\n",
                    "    return _called(rows), _held\n",
                ),
            ),
            ("main.py", "from m import run\nrun([])\n"),
        ],
    );
    assert_eq!(causes(&findings), ["dead-symbol:m.run._dead"]);
}

/// A body's `global X; X = ...` rebinds X without reading it. A read anywhere
/// keeps it live - `X += 1` and `del X` read the name.
#[test]
fn rule_32_write_only_global_is_dead() {
    let findings = run_rule(
        "32",
        &[(
            "m.py",
            concat!(
                "_CACHE = None\n_SEEN = None\n_N = 0\n_T = 0\n_T += 1\n",
                "def reset():\n    global _CACHE, _SEEN\n    _CACHE = None\n    _SEEN = None\n",
                "def peek():\n    return _SEEN\n",
                "def bump():\n    global _N\n    _N += 1\n",
            ),
        )],
    );
    let dead: BTreeSet<&str> = findings
        .iter()
        .map(|f| f.cause.as_str())
        .filter(|c| c.starts_with("dead-symbol:m._"))
        .collect();
    assert_eq!(dead, ["dead-symbol:m._CACHE"].into_iter().collect());
}

/// A def a table holds by value has its signature fixed by the table's
/// consumer.
#[test]
fn rule_32_table_registered_params_are_contract() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "def check_a(facts, provers):\n    return facts\n",
                    "def check_b(facts, provers):\n    return facts\n",
                    "RULES = [check_a]\n",
                    "def run(facts):\n    return check_b(facts, None)\n",
                ),
            ),
            ("main.py", "from m import RULES, run\nrun(RULES)\n"),
        ],
    );
    assert_eq!(
        cause_set(&findings),
        set_of(&["dead-param:m.check_b:provers"])
    );
}

/// Two loaders spelling one signature answer to whatever loads them by path:
/// an unread slot is the ABI's. A signature only one module spells is its own.
#[test]
fn rule_32_a_repeated_module_signature_is_a_plugin_contract() {
    let findings = run_rule(
        "32",
        &[
            (
                "one.py",
                "def judge(answer, truth, question):\n    return answer == truth\n",
            ),
            (
                "two.py",
                "def judge(answer, truth, question):\n    return bool(answer)\n",
            ),
            (
                "solo.py",
                "def score(answer, truth, question):\n    return answer == truth\n",
            ),
            ("main.py", "import one\nimport two\nimport solo\n"),
        ],
    );
    let params: BTreeSet<&str> = findings
        .iter()
        .map(|f| f.cause.as_str())
        .filter(|c| c.starts_with("dead-param"))
        .collect();
    assert_eq!(
        params,
        ["dead-param:solo.score:question"].into_iter().collect()
    );
}

/// Sphinx reads conf.py's globals by name: every top-level binding is the
/// file's whole interface. The same names elsewhere stay judged.
#[test]
fn rule_32_a_sphinx_conf_is_its_tools_namespace() {
    let conf = "project = 'x'\nextensions = ['sphinx.ext.autodoc']\nhtml_theme = 'alabaster'\n";
    let findings = run_rule("32", &[("doc/conf.py", conf), ("settings.py", conf)]);
    let found = cause_set(&findings);
    assert!(!found.iter().any(|c| c.contains("conf")));
    assert!(found.contains("dead-symbol:settings.html_theme"));
}

/// The built name sits one statement from the getattr.
#[test]
fn rule_32_a_dispatch_name_built_in_a_local_reaches_the_method() {
    let findings = run_rule(
        "32",
        &[
            (
                "a.py",
                concat!(
                    "class V:\n",
                    "    def run(self, domain, rule):\n",
                    "        name = '_{0}_{1}'.format(domain, rule)\n",
                    "        return getattr(self, name)()\n",
                    "    def _validate_max(self):\n        return 1\n",
                ),
            ),
            ("main.py", "from a import V\nV().run('validate', 'max')\n"),
        ],
    );
    assert!(findings.is_empty());
}

#[test]
fn rule_32_a_local_bound_twice_builds_no_dispatch_name() {
    let findings = run_rule(
        "32",
        &[
            (
                "b.py",
                concat!(
                    "class V:\n",
                    "    def run(self, domain, rule):\n",
                    "        name = rule\n",
                    "        name = '_{0}_{1}'.format(domain, rule)\n",
                    "        return getattr(self, name)()\n",
                    "    def _validate_max(self):\n        return 1\n",
                ),
            ),
            ("main.py", "from b import V\nV().run('validate', 'max')\n"),
        ],
    );
    assert_eq!(causes(&findings), ["dead-symbol:b.V._validate_max"]);
}

#[test]
fn rule_32_silent_on_live_names_and_exemptions() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "import fakefw\n",
                    "__all__ = ['exported']\n",
                    "def exported():\n    return Config().field\n",
                    "class Config:\n    def __init__(self):\n        self.field = 1\n",
                    "@fakefw.route('/x')\n",
                    "def handler(req):\n    return req\n",
                    "class Sub(fakefw.Base):\n",
                    "    def hook(self, event):\n        return None\n",
                    "def dyn_used():\n    return getattr(exported, 'dyn_used')\n",
                ),
            ),
            ("main.py", ""),
            (
                "tests/test_m.py",
                "def test_ok(tmp_path):\n    assert tmp_path\n",
            ),
        ],
    );
    // __all__ export, decorated registration, framework override method,
    // getattr-string liveness, test fixture params: all exempt
    let found = cause_set(&findings);
    for name in ["exported", "handler", "hook", "Sub", "tmp_path", "test_ok"] {
        assert!(!found.iter().any(|c| c.contains(name)), "{name}");
    }
    assert!(
        !found
            .iter()
            .any(|c| c.starts_with("dead-symbol") && c.contains("dyn_used"))
    );
}

/// Class-body variables of a class answering to an external base are consumed
/// by the framework's contract, one internal hop included. The same
/// unreferenced var in a plain internal class stays dead.
#[test]
fn rule_32_declarative_vars_on_framework_classes_exempt() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "import fakefw\n",
                    "class Mid(fakefw.Model):\n    pass\n",
                    "class Leaf(Mid):\n    verbose_label = 'x'\n",
                    "class Plain:\n    stale_const = 'y'\n    def go(self):\n        return 1\n",
                    "def use():\n    return Plain().go() and Leaf()\n",
                ),
            ),
            ("main.py", "from m import use\nuse()\n"),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.iter().any(|c| c.contains("verbose_label")));
    assert!(found.iter().any(|c| c.contains("stale_const")));
}

/// A mixin's methods are the framework's to call once any subclass chain
/// reaches an external base; a `metaclass=` consumes its class body by code.
#[test]
fn rule_32_mixin_methods_and_metaclass_vars_exempt() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "import fakefw\n",
                    "class ReadOnlyMixin:\n",
                    "    def has_add_permission(self):\n        return False\n",
                    "class Admin(ReadOnlyMixin, fakefw.ModelAdmin):\n    pass\n",
                    "class Meta(type):\n    pass\n",
                    "class Choices(metaclass=Meta):\n    account_exists = 'x'\n",
                    "class Plain:\n    stale = 'y'\n    def unused_method(self):\n        return 1\n",
                    "def use():\n    return Admin() and Choices() and Plain()\n",
                ),
            ),
            ("main.py", "from m import use\nuse()\n"),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.iter().any(|c| c.contains("has_add_permission")));
    assert!(!found.iter().any(|c| c.contains("account_exists")));
    assert!(found.iter().any(|c| c.contains("stale")));
    assert!(found.iter().any(|c| c.contains("unused_method")));
}

/// A class nested in a coupled body and its variables are declarative; a
/// nested class in a plain internal class keeps deadness.
#[test]
fn rule_32_nested_meta_class_is_declarative() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "import fakefw\n",
                    "class Model(fakefw.Base):\n    class Meta:\n        ordering = ['x']\n",
                    "class Plain:\n    class Inner:\n        stale = 'y'\n",
                    "def use():\n    return Model() and Plain()\n",
                ),
            ),
            ("main.py", "from m import use\nuse()\n"),
        ],
    );
    let found = cause_set(&findings);
    assert!(
        !found
            .iter()
            .any(|c| c.contains("Meta") || c.contains("ordering"))
    );
    assert!(
        found
            .iter()
            .any(|c| c.contains("Inner") || c.contains("stale"))
    );
}

/// `getattr(x, f"validate_{k}")` reaches every `validate_*` method; an
/// unmatched sibling name stays dead.
#[test]
fn rule_32_format_string_dispatch_names_are_live() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "class V:\n",
                    "    def validate_scope(self):\n        return 1\n",
                    "    def handle_scope(self):\n        return 2\n",
                    "def run(v: V, key: str):\n    return getattr(v, f'validate_{key}')()\n",
                ),
            ),
            ("main.py", "from m import run\nrun(None, 'scope')\n"),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.iter().any(|c| c.contains("validate_scope")));
    assert!(found.iter().any(|c| c.contains("handle_scope")));
}

// --- #32 precision round: what counts as a use --------------------------------

/// `reg.cache` stores the def for `reg.lookup(...)`: alive by construction;
/// `functools.cache` under the same bare name is a pure wrapper.
#[test]
fn rule_32_a_repo_decorator_may_register_and_a_stdlib_wrapper_never_does() {
    let findings = run_rule(
        "32",
        &[
            (
                "reg.py",
                concat!(
                    "TABLE = {}\n",
                    "def cache(fn):\n    TABLE[fn.__name__] = fn\n    return fn\n",
                    "def lookup(name):\n    return TABLE[name]\n",
                ),
            ),
            (
                "m.py",
                concat!(
                    "import functools\nimport reg\n",
                    "@reg.cache\ndef handler():\n    return 1\n",
                    "@functools.cache\ndef cached_but_dead():\n    return 2\n",
                ),
            ),
            ("main.py", "import reg\nreg.lookup('handler')()\n"),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.contains("dead-symbol:m.handler"));
    assert!(found.contains("dead-symbol:m.cached_but_dead"));
}

/// A decorator that registers nothing leaves the def as dead as an
/// undecorated one; a framework registration still exempts.
#[test]
fn rule_32_pure_wrappers_do_not_exempt() {
    let findings = run_rule(
        "32",
        &[(
            "m.py",
            concat!(
                "import functools\nimport contextlib\nimport fakefw\n",
                "@functools.lru_cache(maxsize=None)\n",
                "def cached_but_dead():\n    return 2\n",
                "@contextlib.contextmanager\n",
                "def ctx_but_dead():\n    yield\n",
                "@fakefw.route('/x')\n",
                "def handler(req):\n    return req\n",
            ),
        )],
    );
    let found = cause_set(&findings);
    assert!(found.contains("dead-symbol:m.cached_but_dead"));
    assert!(found.contains("dead-symbol:m.ctx_but_dead"));
    assert!(!found.contains("dead-symbol:m.handler"));
}

#[test]
fn rule_32_built_dispatch_names_are_live() {
    for build_expr in [
        "'on_' + evt",
        "'on_%s' % evt",
        "'on_{}'.format(evt)",
        "'on_' + evt + ''",
    ] {
        let src = format!(
            concat!(
                "class H:\n",
                "    def on_click(self):\n        return 1\n",
                "    def off_click(self):\n        return 2\n",
                "    def dispatch(self, evt):\n        return getattr(self, {})()\n",
            ),
            build_expr
        );
        let main = ("main.py", "from m import H\nH().dispatch('click')\n");
        let files = vec![
            ("m.py".to_string(), src.clone()),
            (main.0.to_string(), main.1.to_string()),
        ];
        let found = cause_set(&run("32", &files));
        assert!(!found.contains("dead-symbol:m.H.on_click"), "{build_expr}");
        assert!(found.contains("dead-symbol:m.H.off_click"), "{build_expr}");
        // no constant text: no evidence, the name stays reported
        let bare = vec![
            ("m.py".to_string(), src.replace(build_expr, "evt")),
            (main.0.to_string(), main.1.to_string()),
        ];
        assert!(
            cause_set(&run("32", &bare)).contains("dead-symbol:m.H.on_click"),
            "{build_expr}"
        );
    }
}

#[test]
fn rule_32_self_reference_is_not_a_use() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                concat!(
                    "def fact(n):\n    return 1 if n <= 1 else n * fact(n - 1)\n",
                    "def lonely(n):\n    return 0 if n == 0 else lonely(n - 1)\n",
                    "class W:\n    def walk(self, n):\n        return self.walk(n - 1)\n",
                    "def run():\n    return fact(3)\n",
                ),
            ),
            ("main.py", "from m import run\nrun()\n"),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.contains("dead-symbol:m.fact"));
    assert!(found.contains("dead-symbol:m.lonely"));
    assert!(found.contains("dead-symbol:m.W.walk"));
}

/// #32 claims "occurs in no other place"; "reached only by tests" is #56's.
#[test]
fn rule_32_a_test_only_reference_keeps_the_symbol() {
    let findings = run_rule(
        "32",
        &[
            (
                "m.py",
                "def only_tested():\n    return 4\ndef dead():\n    return 5\n",
            ),
            (
                "tests/test_m.py",
                "from m import only_tested\ndef test_it():\n    assert only_tested()\n",
            ),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.contains("dead-symbol:m.only_tested"));
    assert!(found.contains("dead-symbol:m.dead"));
}

#[test]
fn rule_32_dataclass_fields_a_serializer_reads_are_live() {
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\nclass Row:\n    a: int\n    b: int\n",
        "def dump():\n    return dataclasses.asdict(Row(1, 2))\n",
    );
    let findings = run_rule("32", &[("m.py", src)]);
    assert!(!findings.iter().any(|f| f.cause.contains("Row.")));
    let other = src
        .replace("dataclasses.asdict(Row(1, 2))", "Row(1).a")
        .replace("b: int", "b: int = 0");
    let findings = run("32", &[("m.py".to_string(), other)]);
    assert!(cause_set(&findings).contains("dead-symbol:m.Row.b"));
}

/// `[project.scripts]` and every `[project.entry-points.*]` group name
/// objects an installed distribution reaches: #32 never reports them.
#[test]
fn rule_32_pyproject_entry_points_are_live_roots() {
    let tree: [(&str, &str); 2] = [
        ("pkg/__init__.py", ""),
        (
            "pkg/cli.py",
            "class Alpha:\n    pass\n\n\nclass Beta:\n    pass\n",
        ),
    ];
    let dead = cause_set(&run_rule("32", &tree));
    assert!(dead.contains("dead-symbol:pkg.cli.Alpha"));
    assert!(dead.contains("dead-symbol:pkg.cli.Beta"));
    let declared = plus(
        &tree,
        (
            "pyproject.toml",
            concat!(
                "[project]\nname = \"pkg\"\n\n",
                "[project.scripts]\nalpha = \"pkg.cli:Alpha\"\n\n",
                "[project.entry-points.\"my.group\"]\nbeta = \"pkg.cli:Beta\"\n",
            ),
        ),
    );
    let findings = run("32", &declared);
    assert!(cause_set(&findings).intersection(&dead).next().is_none());
}

/// The plugin host calls the entry-point class's public methods.
#[test]
fn rule_32_entry_point_class_members_are_roots() {
    let findings = run_rule(
        "32",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/plugins.py",
                concat!(
                    "class Alpha:\n    def run(self):\n        return 1\n",
                    "    def _own(self):\n        return 2\n",
                    "class Dead:\n    def run(self):\n        return 3\n",
                ),
            ),
            (
                "pyproject.toml",
                concat!(
                    "[project]\nname = \"pkg\"\n\n",
                    "[project.entry-points.\"my.group\"]\nalpha = \"pkg.plugins:Alpha\"\n",
                ),
            ),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.contains("dead-symbol:pkg.plugins.Alpha"));
    assert!(!found.contains("dead-symbol:pkg.plugins.Alpha.run"));
    assert!(found.contains("dead-symbol:pkg.plugins.Alpha._own"));
    assert!(found.contains("dead-symbol:pkg.plugins.Dead"));
}

/// The plugin host calls the hook where the base defines it.
#[test]
fn rule_32_entry_point_class_inherits_its_hooks() {
    let findings = run_rule(
        "32",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/base.py",
                "class Base:\n    def run(self):\n        return 1\n    def _own(self):\n        return 2\n",
            ),
            (
                "pkg/plugins.py",
                "from pkg.base import Base\nclass Alpha(Base):\n    pass\n",
            ),
            (
                "pyproject.toml",
                concat!(
                    "[project]\nname = \"pkg\"\n\n",
                    "[project.entry-points.\"my.group\"]\nalpha = \"pkg.plugins:Alpha\"\n",
                ),
            ),
        ],
    );
    let found = cause_set(&findings);
    assert!(!found.contains("dead-symbol:pkg.base.Base.run"));
    assert!(found.contains("dead-symbol:pkg.base.Base._own"));
}

#[test]
fn rule_32_positional_construction_uses_the_field() {
    let head =
        "import dataclasses\n@dataclasses.dataclass\nclass Row:\n    a: int\n    b: int = 0\n";
    let both = format!("{head}def use():\n    return Row(1, 2)\n");
    assert!(
        !run("32", &[("m.py".to_string(), both)])
            .iter()
            .any(|f| f.cause.contains("Row."))
    );
    let one = format!("{head}def use():\n    return Row(1)\n");
    assert!(cause_set(&run("32", &[("m.py".to_string(), one)])).contains("dead-symbol:m.Row.b"));
    // an own __init__ decides what a positional argument binds
    let own = concat!(
        "class P:\n    a: int\n    def __init__(self, x):\n        self.x = x\n",
        "def use():\n    return P(1)\n",
    );
    assert!(cause_set(&run_rule("32", &[("m.py", own)])).contains("dead-symbol:m.P.a"));
}

/// A field every construction passes by keyword occurs in no other place, so
/// "its name occurs in no other place" is a false claim.
#[test]
fn rule_32_keyword_construction_uses_the_field() {
    let head =
        "import dataclasses\n@dataclasses.dataclass\nclass Row:\n    a: int = 0\n    b: int = 0\n";
    let src = format!("{head}def use():\n    return Row(b=2)\n");
    let found = cause_set(&run("32", &[("m.py".to_string(), src)]));
    assert!(!found.contains("dead-symbol:m.Row.b"));
    assert!(found.contains("dead-symbol:m.Row.a"));
}

/// A ClassVar and an `init=False` field take no positional; KW_ONLY ends the
/// positional slots.
#[test]
fn rule_32_positional_slots_skip_class_vars_and_init_false() {
    let head = concat!(
        "from dataclasses import KW_ONLY, dataclass, field\nfrom typing import ClassVar\n",
        "@dataclass\nclass Row:\n    kind: ClassVar[str] = 'r'\n    a: int = 0\n",
        "    n: int = field(init=False, default=0)\n    b: int = 0\n    _: KW_ONLY\n    c: int = 0\n",
    );
    let two = cause_set(&run(
        "32",
        &[(
            "m.py".to_string(),
            format!("{head}def use():\n    return Row(1, 2)\n"),
        )],
    ));
    assert!(!two.contains("dead-symbol:m.Row.a"));
    assert!(!two.contains("dead-symbol:m.Row.b"));
    assert!(two.contains("dead-symbol:m.Row.n"));
    assert!(two.contains("dead-symbol:m.Row.c"));
    let three = cause_set(&run(
        "32",
        &[(
            "m.py".to_string(),
            format!("{head}def use():\n    return Row(1, 2, 3)\n"),
        )],
    ));
    assert!(three.contains("dead-symbol:m.Row.c"));
}

// --- dead_symbol_splice -------------------------------------------------------

fn splice_lines(cause: &str, files: &[(&str, &str)]) -> Option<BTreeSet<u32>> {
    let (_dir, stack) = build(files);
    dead_symbol_splice(cause, stack.facts(), &stack.provers)
        .map(|s| s.edits.iter().map(|e| e.line).collect())
}

#[test]
fn dead_symbol_splice_a_twice_bound_name_gets_no_splice() {
    // `try: X = a` / `except ImportError: X = b`: one symbol, two bindings.
    // A deletion of the recorded node would leave the other and the re-audit
    // would still report the symbol, so no splice is the verdict.
    let lines = splice_lines(
        "dead-symbol:app.FLAG",
        &[(
            "app.py",
            "try:\n    from os import sep\n\n    FLAG = sep == \"/\"\nexcept ImportError:\n    FLAG = False\n",
        )],
    );
    assert_eq!(lines, None);
}

#[test]
fn dead_symbol_splice_a_local_of_the_same_name_is_no_rebinding() {
    let lines = splice_lines(
        "dead-symbol:app.FLAG",
        &[(
            "app.py",
            "FLAG = 1\n\n\ndef live():\n    FLAG = 2\n    return FLAG\n",
        )],
    );
    assert_eq!(lines, Some([1].into_iter().collect()));
}

#[test]
fn dead_symbol_splice_a_registering_import_stays() {
    let lines = splice_lines(
        "dead-symbol:app._dead",
        &[
            ("core.py", "REGISTRY = {}\n"),
            (
                "plugin.py",
                "import core\n\ncore.REGISTRY[\"plugin\"] = 1\n",
            ),
            ("app.py", "import plugin\n\n\ndef _dead():\n    return 1\n"),
        ],
    );
    // the def alone
    assert_eq!(lines, Some([4, 5].into_iter().collect()));
}

#[test]
fn dead_symbol_splice_a_pure_helper_import_goes() {
    let lines = splice_lines(
        "dead-symbol:app._dead",
        &[
            ("pure.py", "def helper():\n    return 1\n"),
            (
                "app.py",
                "import pure\n\n\ndef _dead():\n    return pure.helper()\n",
            ),
        ],
    );
    // the import line too
    assert_eq!(lines, Some([1, 4, 5].into_iter().collect()));
}

// --- #34 no-op code -----------------------------------------------------------

/// A commented-out block starting with `elif ...:` does not parse as a module
/// on its own but is still code. Prose blocks stay silent.
#[test]
fn rule_34_orphan_elif_fragment_is_commented_code() {
    let findings = run_rule(
        "34",
        &[(
            "m.py",
            concat!(
                "x = 1\n",
                "# elif to_process.compare is not None:\n",
                "#     values = to_process.compare.dropna()\n",
                "#     graph.draw(values)\n",
                "y = 2\n",
                "# these three lines describe intent in plain prose only\n",
                "# and never parse as python statements at all\n",
                "# so this prose block stays silent\n",
            ),
        )],
    );
    let found: Vec<&str> = findings
        .iter()
        .map(|f| f.cause.as_str())
        .filter(|c| c.contains("commented-code"))
        .collect();
    assert_eq!(found, ["commented-code:m:2"]);
}

#[test]
fn rule_34_fires_on_commented_code_noop_try_and_swallow() {
    let findings = run_rule(
        "34",
        &[(
            "m.py",
            concat!(
                "def run(x):\n",
                "    # y = x * 2\n",
                "    # if y > 3:\n",
                "    #     return y\n",
                "    try:\n        return x\n",
                "    except ValueError as e:\n        raise e\n",
                "def swallow(x):\n",
                "    v = None\n",
                "    try:\n        v = int(x)\n",
                "    except Exception:\n        return None\n",
                "    return v\n",
            ),
        )],
    );
    let mut kinds: Vec<&str> = findings
        .iter()
        .map(|f| f.cause.split(':').next().expect("an arm"))
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(
        kinds,
        ["commented-code", "noop-try", "swallowed-default-return"]
    );
}

#[test]
fn rule_34_silent_on_prose_comments_and_real_handlers() {
    let findings = run_rule(
        "34",
        &[(
            "m.py",
            concat!(
                "def run(x):\n",
                "    # this loop walks the tree twice because the first\n",
                "    # pass only collects names and the second pass links\n",
                "    # them to their homes in the index\n",
                "    try:\n        return int(x)\n",
                "    except ValueError:\n        return 0\n",
                "def filtered(x):\n",
                "    try:\n        return int(x)\n",
                "    except KeyboardInterrupt:\n        raise\n",
                "    except Exception as e:\n        return str(e)\n",
                "def logged(x, log):\n",
                "    try:\n        return int(x)\n",
                "    except ValueError:\n",
                "        log.warning('bad %s', x)\n        return 0\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

// --- #34 swallow arms ---------------------------------------------------------

const LOGGING: &str = "import logging\nimport traceback\nlog = logging.getLogger(__name__)\n";

#[test]
fn rule_34_default_return_fires() {
    let source = LOGGING.to_string()
        + concat!(
            "def default(x):\n",
            "    try:\n        v = int(x)\n",
            "    except Exception:\n        return None\n",
            "    return v\n",
            "def empty(x):\n",
            "    try:\n        v = [int(x)]\n",
            "    except Exception as e:\n",
            "        log.warning('bad %s', str(e))\n        return []\n",
            "    return v\n",
        );
    let findings = run("34", &[("m.py".to_string(), source)]);
    let mut found = causes(&findings);
    found.sort_unstable();
    assert_eq!(
        found,
        [
            "swallowed-default-return:m:13",
            "swallowed-default-return:m:7"
        ]
    );
}

/// The author wrote the failure result beside the success one and no caller
/// reads past it: the sentinel is the alternative, not a lie.
#[test]
fn rule_34_a_try_that_returns_on_success_is_a_two_armed_choice() {
    let findings = run_rule(
        "34",
        &[(
            "m.py",
            concat!(
                "def cascade(x):\n",
                "    try:\n        return int(x)\n",
                "    except Exception:\n        return None\n",
                "def guarded(x):\n",
                "    try:\n",
                "        if x:\n            return int(x)\n",
                "    except Exception:\n        return {}\n",
                "    return {}\n",
                "def skipped(items):\n",
                "    for it in items:\n",
                "        try:\n            work(it)\n            continue\n",
                "        except Exception:\n            return []\n",
                "def short_circuits(x):\n",
                "    try:\n        v = int(x)\n",
                "    except Exception:\n        return None\n",
                "    return v\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["swallowed-default-return:m:23"]);
}

#[test]
fn rule_34_a_nested_defs_return_is_not_the_trys_exit() {
    let findings = run_rule(
        "34",
        &[(
            "m.py",
            concat!(
                "def outer(x):\n",
                "    try:\n",
                "        def inner():\n            return int(x)\n",
                "        v = inner()\n",
                "    except Exception:\n        return None\n",
                "    return v\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["swallowed-default-return:m:6"]);
}

#[test]
fn rule_34_handled_reraised_narrow_computed_and_tests_stay_silent() {
    let source = LOGGING.to_string()
        + concat!(
            "def collected(x, errors):\n",
            "    try:\n        v = int(x)\n",
            "    except Exception as e:\n",
            "        errors.append(e)\n        return None\n",
            "class S:\n",
            "    def stored(self, x):\n",
            "        try:\n            v = int(x)\n",
            "        except Exception as e:\n",
            "            self.last = e\n            return None\n",
            "def reraised(x):\n",
            "    try:\n        v = int(x)\n",
            "    except Exception:\n        log.error('bad')\n        raise\n",
            "def narrow(x):\n",
            "    try:\n        v = int(x)\n",
            "    except ValueError:\n        return None\n",
            "def computed(x, fallback):\n",
            "    try:\n        v = int(x)\n",
            "    except Exception:\n        return fallback(x)\n",
        );
    let files = vec![
        ("m.py".to_string(), source),
        (
            "tests/test_m.py".to_string(),
            "def test_default(x):\n    try:\n        v = int(x)\n    except Exception:\n        return None\n"
                .to_string(),
        ),
    ];
    assert!(run("34", &files).is_empty());
}

/// One finding per site: #33's contract oracle answers for the declared
/// non-Optional return, #34 fires on the untyped twin, and a declared
/// `| None` is the contract itself.
#[test]
fn rule_34_33_owns_the_typed_none_return() {
    let source = concat!(
        "def typed(x: str) -> int:\n",
        "    try:\n        v = int(x)\n",
        "    except Exception:\n        return None\n",
        "    return v\n",
        "def untyped(x):\n",
        "    try:\n        v = int(x)\n",
        "    except Exception:\n        return None\n",
        "    return v\n",
        "def optional(x: str) -> int | None:\n",
        "    try:\n        v = int(x)\n",
        "    except Exception:\n        return None\n",
        "    return v\n",
    );
    assert_eq!(
        causes(&run_rule("34", &[("m.py", source)])),
        ["swallowed-default-return:m:10"]
    );
    // the overlap oracle #34 asks, symbol by symbol
    let (_dir, stack) = build(&[("m.py", source)]);
    let facts = stack.facts();
    let module = &facts.modules["m"];
    let answered: Vec<&str> = ["m.typed", "m.untyped", "m.optional"]
        .into_iter()
        .filter(|q| {
            sightline_py_rules::returns::return_contract_finding(facts, module, &facts.symbols[*q])
                .is_some()
        })
        .collect();
    assert_eq!(answered, ["m.typed"]);
}

/// bool and message defaults are answers, not a swallowed failure.
#[test]
fn rule_34_answers_stay_silent() {
    let source = LOGGING.to_string()
        + concat!(
            "def is_open(s):\n",
            "    try:\n        s.connect()\n        ok = True\n",
            "    except Exception:\n        return False\n",
            "    return ok\n",
            "def problem():\n",
            "    try:\n        v = check()\n",
            "    except Exception:\n        return 'version could not be determined'\n",
            "    return v\n",
            "def zero(x):\n",
            "    try:\n        v = int(x)\n",
            "    except Exception:\n        return 0\n",
            "    return v\n",
        );
    assert_eq!(
        causes(&run("34", &[("m.py".to_string(), source)])),
        ["swallowed-default-return:m:20"]
    );
}

/// Recording the failure handles it without binding the name.
#[test]
fn rule_34_recording_the_failure_handles_it_without_the_name() {
    let source = "import logging\nlog = logging.getLogger(__name__)\nfailed = []\n".to_string()
        + concat!(
            "def recorded(path):\n    try:\n        v = work()\n",
            "    except Exception:\n        failed.append(path)\n        return None\n",
            "    return v\n",
            "class S:\n    def marked(self):\n        try:\n            v = work()\n",
            "        except Exception:\n            self.ok = False\n            return None\n",
            "        return v\n",
            "def plain():\n    try:\n        v = work()\n",
            "    except Exception:\n        return None\n",
            "    return v\n",
        );
    assert_eq!(
        causes(&run("34", &[("m.py".to_string(), source)])),
        ["swallowed-default-return:m:22"]
    );
}

// --- #60 dead by graph --------------------------------------------------------

/// Two modules spell `helper`: the import resolves the call to a's, nothing
/// runs b's, and the name occurs (so #32 stays silent on it).
const GRAPH: [(&str, &str); 5] = [
    ("pkg/__init__.py", ""),
    ("pkg/a.py", "def helper(x):\n    return x\n"),
    (
        "pkg/b.py",
        "def helper(x):\n    return x\ndef _dead(x):\n    return x\n",
    ),
    (
        "pkg/app.py",
        "from pkg.a import helper\ndef run():\n    return helper(1)\n",
    ),
    ("main.py", "from pkg.app import run\nrun()\n"),
];

#[test]
fn rule_60_fires_where_no_site_resolves_to_the_def() {
    let findings = run_rule("60", &GRAPH);
    assert_eq!(causes(&findings), ["dead-by-graph:pkg.b.helper"]);
    assert_eq!(
        findings[0].message,
        "no resolved caller in the whole program runs function pkg.b.helper \
         (the name occurs: 0 references, 0 unresolved/by-name sites)"
    );
    match &findings[0].evidence {
        Evidence::Wp { premises } => {
            assert_eq!(&premises[..2], ["closed-world:pass", "resolved-callers:0"]);
        }
        other => panic!("#60 reports WP evidence, not {other:?}"),
    }
    // #32 owns the name that occurs in no other place: neither rule reports
    // the other's site
    assert_eq!(causes(&run_rule("32", &GRAPH)), ["dead-symbol:pkg.b._dead"]);
}

#[test]
fn rule_60_a_published_def_and_a_console_script_stay_silent() {
    let pyproject = concat!(
        "[project]\nname = \"pkg\"\n\n",
        "[project.scripts]\nhelp-me = \"pkg.b:helper\"\n",
    );
    let script = plus(&GRAPH, ("pyproject.toml", pyproject));
    assert!(run("60", &script).is_empty());
    // published off: the console script alone is the root that keeps it
    let unpublished = plus(
        &GRAPH,
        (
            "pyproject.toml",
            &format!("{pyproject}\n[tool.sightline]\npublished = false\n"),
        ),
    );
    assert!(run("60", &unpublished).is_empty());
}

/// Without an oracle a by-name guess stands as resolved, so only CHA's
/// ambiguity is left: a site still guessing at the method is a caller.
#[test]
fn rule_60_an_ambiguous_candidate_is_a_caller() {
    let findings = run_rule(
        "60",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/a.py",
                "class A:\n    def flush(self):\n        return 1\n",
            ),
            (
                "pkg/b.py",
                "class B:\n    def flush(self):\n        return 2\n",
            ),
            ("pkg/run.py", "def go(x):\n    return x.flush()\n"),
            ("main.py", "from pkg.run import go\ngo(None)\n"),
        ],
    );
    assert!(findings.is_empty());
}

/// A by-name site of a name only one def holds is that def's caller, and a
/// method a Protocol declared in the tree names runs through the
/// protocol-typed variable.
#[test]
fn rule_60_the_judge_waves_two_fp_shapes_stay_silent() {
    let findings = run_rule(
        "60",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/m.py",
                concat!(
                    "from typing import Protocol\n",
                    "class Ledger(Protocol):\n    def skip(self, n): ...\n",
                    "class Real:\n    def skip(self, n):\n        return n\n",
                    "    def only_once(self, n):\n        return n\n",
                ),
            ),
            (
                "pkg/app.py",
                concat!(
                    "from pkg.m import Ledger\n",
                    "def run(ledger: Ledger, ctx):\n    return ledger.skip(1) + ctx.only_once(2)\n",
                ),
            ),
            (
                "main.py",
                "from pkg.app import run\nfrom pkg.m import Real\nrun(Real(), Real())\n",
            ),
        ],
    );
    assert!(findings.is_empty());
}

/// A config file may name a published package's public def by path; a
/// metaclass reads a class body's names by prefix. The private module def
/// nothing runs still fires.
#[test]
fn rule_60_a_distributions_public_def_and_a_metaclassed_method_stay_silent() {
    let findings = run_rule(
        "60",
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"pkg\"\n\n[build-system]\nrequires = [\"setuptools\"]\n",
            ),
            ("pkg/__init__.py", ""),
            (
                "pkg/m.py",
                concat!(
                    "class Meta(type):\n    pass\n",
                    "class Rules:\n",
                    "    def _check_with_items(self, v):\n        return v\n",
                    "class Validator(Rules, metaclass=Meta):\n    pass\n",
                    "def verify(x):\n    return x\n",
                    "def _stranded(x):\n    return x\n",
                    "def _twin(x):\n    return x\n",
                ),
            ),
            (
                "pkg/app.py",
                "from pkg.m import _twin\ndef run():\n    return _twin(1)\n",
            ),
            (
                "pkg/other.py",
                "def _stranded(x):\n    return x\ndef go():\n    return _stranded(2)\n",
            ),
            (
                "main.py",
                "from pkg.app import run\nfrom pkg.other import go\nrun()\ngo()\n",
            ),
        ],
    );
    assert_eq!(causes(&findings), ["dead-by-graph:pkg.m._stranded"]);
}

/// `from x import f as g` in one arm, `def g` in the other: the module binds
/// `g` twice, so no site can speak for the import's target.
#[test]
fn rule_60_an_aliased_import_a_fallback_def_shadows_is_unspoken() {
    let findings = run_rule(
        "60",
        &[
            ("pkg/__init__.py", ""),
            ("pkg/impl.py", "def is_quantized(t):\n    return True\n"),
            (
                "pkg/ops.py",
                concat!(
                    "HAVE = False\n",
                    "if HAVE:\n    from pkg.impl import is_quantized as check\n",
                    "else:\n",
                    "    def check(t):\n        return False\n",
                    "def run(t):\n    return check(t)\n",
                ),
            ),
            ("main.py", "from pkg.ops import run\nrun(1)\n"),
        ],
    );
    assert!(findings.is_empty());
}

// --- #56 test-only symbols ----------------------------------------------------

const ONLY_TESTED: [(&str, &str); 4] = [
    ("pkg/__init__.py", ""),
    (
        "pkg/m.py",
        concat!(
            "def only_tested():\n    return 4\n",
            "def _only_tested():\n    return 5\n",
            "class _Row:\n    def _only_tested_method(self):\n        return 6\n",
        ),
    ),
    ("tests/__init__.py", ""),
    (
        "tests/test_m.py",
        concat!(
            "from pkg.m import only_tested, _only_tested, _Row\n",
            "def test_it():\n",
            "    assert only_tested() and _only_tested() and _Row()._only_tested_method()\n",
        ),
    ),
];

/// Nothing here is packaged: every caller of `only_tested` would be in this
/// tree, and none is.
#[test]
fn rule_56_an_application_is_judged_on_public_names_too() {
    let findings = run_rule("56", &ONLY_TESTED);
    assert_eq!(
        causes(&findings),
        [
            "test-only:pkg.m.only_tested",
            "test-only:pkg.m._only_tested",
            "test-only:pkg.m._Row",
            "test-only:pkg.m._Row._only_tested_method",
        ]
    );
    assert_eq!(
        findings[0].message,
        "function pkg.m.only_tested is referenced only by tests (tests.test_m) - delete both"
    );
    // a private name, as #32 ranks it
    assert_eq!(findings[1].salience, 2.0);
}

/// The same tree packaged: the public names are the distribution's surface.
#[test]
fn rule_56_a_published_module_keeps_its_public_names() {
    let files = plus(
        &ONLY_TESTED,
        (
            "pyproject.toml",
            "[project]\nname = \"pkg\"\n\n[build-system]\nrequires = [\"setuptools\"]\n",
        ),
    );
    assert_eq!(
        causes(&run("56", &files)),
        [
            "test-only:pkg.m._only_tested",
            "test-only:pkg.m._Row",
            "test-only:pkg.m._Row._only_tested_method",
        ]
    );
}

/// A hand-run tool the repo's own docs name is shipped to the readers who run
/// it; the names no doc spells still fire.
#[test]
fn rule_56_the_repos_own_prose_publishes_the_tool() {
    let files = plus(
        &ONLY_TESTED,
        (
            "docs/TOOLS.md",
            "# Tools\n\n`only_tested` answers where a column is computed.\n",
        ),
    );
    assert_eq!(
        causes(&run("56", &files)),
        [
            "test-only:pkg.m._only_tested",
            "test-only:pkg.m._Row",
            "test-only:pkg.m._Row._only_tested_method",
        ]
    );
}

#[test]
fn rule_56_one_prod_reference_keeps_the_symbol() {
    let findings = run_rule(
        "56",
        &[
            (
                "m.py",
                "def shared():\n    return 4\ndef run():\n    return shared()\n",
            ),
            (
                "tests/test_m.py",
                "from m import shared\ndef test_it():\n    assert shared()\n",
            ),
        ],
    );
    assert!(findings.is_empty());
}

/// A public constant is a declared surface; a method name two classes carry
/// is unmeasurable by name; a method on a class prod uses is a live type's
/// helper; a keyword-only record field is `Unseen`'s.
#[test]
fn rule_56_the_judge_waves_four_fp_shapes_stay_silent() {
    let findings = run_rule(
        "56",
        &[
            (
                "m.py",
                concat!(
                    "HARDCODED = 3\n_HIDDEN = 4\n",
                    "class Live:\n    def helper(self):\n        return 1\n",
                    "class Other:\n    def reset(self):\n        return 2\n",
                    "class Also:\n    def reset(self):\n        return 3\n",
                    "class Idle:\n    def only_tested(self):\n        return 5\n",
                    "class Rec:\n    def __init__(self, unreached=0):\n        self.unreached = unreached\n",
                ),
            ),
            (
                "app.py",
                "from m import Live, Rec\nLive()\nRec(unreached=1)\n",
            ),
            (
                "tests/test_m.py",
                concat!(
                    "from m import HARDCODED, _HIDDEN, Live, Other, Also, Idle, Rec\n",
                    "def test_it():\n",
                    "    assert HARDCODED and _HIDDEN and Live().helper() and Other().reset()\n",
                    "    assert Also().reset() and Idle().only_tested() and Rec().unreached\n",
                ),
            ),
        ],
    );
    assert_eq!(
        causes(&findings),
        [
            "test-only:m.Other",
            "test-only:m.Also",
            "test-only:m.Idle",
            "test-only:m.Idle.only_tested",
        ]
    );
}

/// `__all__` and an entry point are the installed distribution's reach, a
/// dunder the interpreter's, a registering decorator the framework's.
#[test]
fn rule_56_the_roots_32_respects_stay_silent() {
    let findings = run_rule(
        "56",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/api.py",
                concat!(
                    "import fakefw\n",
                    "__all__ = ['exported']\n",
                    "def exported():\n    return 1\n",
                    "def main():\n    return 2\n",
                    "class Alpha:\n    def __len__(self):\n        return 3\n",
                    "@fakefw.route('/x')\n",
                    "def handler():\n    return 4\n",
                ),
            ),
            (
                "pyproject.toml",
                "[project]\nname = \"pkg\"\n\n[project.scripts]\nx = \"pkg.api:Alpha\"\n",
            ),
            (
                "tests/test_api.py",
                concat!(
                    "from pkg.api import exported, main, Alpha, handler\n",
                    "def test_it():\n    assert exported() and main() and len(Alpha()) and handler()\n",
                ),
            ),
        ],
    );
    assert!(findings.is_empty());
}

// --- `tests/test_fixes.py`'s #32 cases, at the splice the emitter rides -------
// The `attach_fixes` half (the verified `Fix` riding the finding) waits for
// `py-rules-close`; the splice's own edits are checked here.

const DEAD: &str = concat!(
    "import os\nfrom pkg import dep\n\n\n",
    "def _dead() -> str:\n    return dep.SEP + os.sep\n\n\n",
    "def live() -> int:\n    return 1\n",
);

/// Both names are orphaned once the def goes; only the import that binds
/// names may follow it - `os` may snapshot the environment as it loads, and
/// no world would say so.
#[test]
fn dead_symbol_splice_takes_its_lines_and_the_internal_import_only_it_read() {
    let lines = splice_lines(
        "dead-symbol:pkg.m._dead",
        &[
            ("pkg/__init__.py", ""),
            ("pkg/dep.py", "SEP = '/'\n"),
            ("pkg/m.py", DEAD),
        ],
    );
    // the def, and the dep import; the stdlib import on line 1 stays
    assert_eq!(lines, Some([2, 5, 6].into_iter().collect()));
}

/// A quoted annotation is a use liveness reads (no finding); a name a call is
/// passed as text is a reach no world sees: reported, unpatched.
#[test]
fn dead_symbol_splice_a_name_a_string_reaches_is_reported_without_a_patch() {
    let files: [(&str, &str); 2] = [
        (
            "m.py",
            concat!(
                "class _Rec:\n    pass\n\n\ndef _seam():\n    return 1\n\n\n",
                "def use(x: '_Rec') -> None:\n    return None\n",
            ),
        ),
        (
            "n.py",
            "import m\n\n\ndef go() -> None:\n    return m.use(1) and patch('m._seam')\n",
        ),
    ];
    let found = cause_set(&run_rule("32", &files));
    assert!(!found.contains("dead-symbol:m._Rec"));
    assert!(found.contains("dead-symbol:m._seam"));
    assert_eq!(splice_lines("dead-symbol:m._seam", &files), None);
}
