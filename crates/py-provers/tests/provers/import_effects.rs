//! `provers/import_effects.py`: what an import runs.

use crate::oracle_fixture;

use sightline_testkit::build;

#[test]
fn a_module_whose_import_calls_something_uncatalogued_runs_work() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/data.py", "import json\nHEADER = json.dumps({})\n"),
        ("pkg/pure.py", "import re\nPAT = re.compile('a')\n"),
        (
            "app.py",
            "def f():\n    from pkg import data, pure\n    return data.HEADER, pure.PAT\n",
        ),
    ]);
    let facts = stack.facts();
    let effects = stack.provers.import_effects(facts);
    assert!(effects.contains("pkg.data"));
    assert!(!effects.contains("pkg.pure"));
    // the importer pays what its top-level closure loads, and app's
    // import is deferred
    assert!(!effects.contains("app"));
}

/// A degraded run keeps `callee`'s reading: a method on a module global
/// stays import-time work (`tests/provers/test_import_graph.py`'s
/// `test_degraded_run_keeps_the_unspelled_reading`).
#[test]
fn a_degraded_run_keeps_the_unspelled_reading() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/data.py",
            "_SEP = ', '\nHEADER = _SEP.join(['a', 'b'])\n",
        ),
        (
            "app.py",
            "def f():\n    from pkg import data\n    return data.HEADER\n",
        ),
    ]);
    let facts = stack.facts();
    assert!(stack.provers.import_effects(facts).contains("pkg.data"));
    assert!(
        stack
            .provers
            .notes()
            .iter()
            .any(|n| n.starts_with("no oracle: an import-time"))
    );
}

#[test]
fn a_module_scope_store_through_an_attribute_is_work() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/reg.py", "import sys\nsys.modules['x'] = 1\n"),
        (
            "app.py",
            "def f():\n    from pkg import reg\n    return reg\n",
        ),
    ]);
    assert!(
        stack
            .provers
            .import_effects(stack.facts())
            .contains("pkg.reg")
    );
}

#[test]
fn a_function_body_runs_when_it_is_called_not_when_it_is_imported() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/lazy.py",
            "import json\n\n\ndef load():\n    return json.dumps({})\n",
        ),
        (
            "app.py",
            "def f():\n    from pkg import lazy\n    return lazy.load()\n",
        ),
    ]);
    assert!(
        !stack
            .provers
            .import_effects(stack.facts())
            .contains("pkg.lazy")
    );
}

// --- the typed receiver (`test_import_graph.py:TestTypedReceiver`) ---------

/// The three files that class's `_facts` helper writes, with the import-time
/// call under test, and an in-process checker at the root.
fn typed_receiver(call: &str) -> (tempfile::TempDir, sightline_testkit::PyStack) {
    let data = format!("_SEP = ', '\nHEADER = {call}\n");
    let (dir, mut stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/data.py", &data),
        (
            "app.py",
            "def f():\n    from pkg import data\n    return data.HEADER\n",
        ),
    ]);
    oracle_fixture::attach(&dir, &mut stack);
    (dir, stack)
}

/// `test_catalogued_receiver_class_lifts_the_guard`: the span query names
/// `str.join`, a kind the catalog holds, so the import runs nothing. That
/// test's #35 hoist half waits for phase 5.
#[test]
fn a_catalogued_receiver_class_lifts_the_guard() {
    let (_dir, stack) = typed_receiver("_SEP.join(['a', 'b'])");
    assert!(
        !stack
            .provers
            .import_effects(stack.facts())
            .contains("pkg.data")
    );
}

/// `test_a_paths_resolve_is_inert_through_its_receiver`: `Path(__file__)`
/// spells itself, and only the span query spells the `.resolve()` off it.
#[test]
fn a_paths_resolve_is_inert_through_its_receiver() {
    let (dir, mut stack) = build(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/data.py",
            "from pathlib import Path\nHERE = Path(__file__).resolve()\n",
        ),
        (
            "app.py",
            "def f():\n    from pkg import data\n    return data.HERE\n",
        ),
    ]);
    oracle_fixture::attach(&dir, &mut stack);
    assert!(
        !stack
            .provers
            .import_effects(stack.facts())
            .contains("pkg.data")
    );
}

/// `test_receiver_class_outside_the_catalog_stays_work`: the near miss, one
/// method apart, and `str.encode` is not a kind the catalog holds.
#[test]
fn a_receiver_class_outside_the_catalog_stays_work() {
    let (_dir, stack) = typed_receiver("_SEP.encode()");
    assert!(
        stack
            .provers
            .import_effects(stack.facts())
            .contains("pkg.data")
    );
}
