//! The internal import graph and its readings.

use indexmap::IndexSet;

use sightline_core::findings::Qname;
use sightline_py_facts::kinds::Kind;
use sightline_py_provers::imports::*;
use sightline_testkit::build;

#[test]
fn the_import_graph_is_built_once_per_provers() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/util.py", "def helper():\n    return 1\n"),
        (
            "app.py",
            "def f():\n    from pkg import util\n    return util.helper()\n",
        ),
    ]);
    let facts = stack.facts();
    let first = stack.provers.import_graph(facts);
    let second = stack.provers.import_graph(facts);
    assert!(std::ptr::eq(first, second));
    // a function-scope import is a deferred edge, never a top-level one
    assert_eq!(first.full["app"], IndexSet::from([Qname::from("pkg.util")]));
    assert!(first.top["app"].is_empty());
}

/// PEP 420: `bin/tool.py` imports `ns.pkg.mod` across a namespace
/// package.
#[test]
fn a_namespace_package_import_is_an_edge() {
    let (_dir, stack) = build(&[
        ("src/ns/pkg/__init__.py", ""),
        ("src/ns/pkg/mod.py", "def f():\n    pass\n"),
        (
            "bin/tool.py",
            "from ns.pkg import mod\n\n\ndef run():\n    mod.f()\n",
        ),
    ]);
    let facts = stack.facts();
    let graph = import_graph(facts);
    assert_eq!(
        importers(&graph)["ns.pkg.mod"],
        IndexSet::from([Qname::from("tool")])
    );
}

#[test]
fn a_type_checking_edge_is_typed_and_never_top() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/util.py", "def helper():\n    return 1\n"),
        (
            "app.py",
            "from typing import TYPE_CHECKING\n\nif TYPE_CHECKING:\n    from pkg import util\n",
        ),
    ]);
    let graph = import_graph(stack.facts());
    assert_eq!(graph.full["app"], IndexSet::from([Qname::from("pkg.util")]));
    assert_eq!(
        graph.typed["app"],
        IndexSet::from([Qname::from("pkg.util")])
    );
    assert!(graph.top["app"].is_empty());
}

#[test]
fn a_probe_import_is_the_feature_test_itself() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/opt.py", "X = 1\n"),
        (
            "app.py",
            "try:\n    from pkg import opt\nexcept ImportError:\n    opt = None\n",
        ),
    ]);
    let facts = stack.facts();
    let module = &facts.modules["app"];
    let imports = module.nodes(&[Kind::ImportFrom], None, false);
    assert!(probes_availability(module, imports[0]));
}
