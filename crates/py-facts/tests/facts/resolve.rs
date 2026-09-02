//! The pass-B half of the build: what a name refers to, how a call
//! resolves, and which nodes a signature's expressions belong to.

use std::collections::BTreeMap;

use ruff_python_ast::Stmt;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{Ref, RefKind, RepoFacts, Resolution};
use sightline_py_facts::unparse;
use sightline_testkit::build;

/// Enclosing symbols of the call sites facts alone resolves to `qname` (the
/// caller table itself lives on the oracle-upgraded call graph).
fn resolved_callers<'a>(facts: &'a RepoFacts<'_>, qname: &str) -> Vec<&'a str> {
    facts
        .call_sites
        .iter()
        .filter(|c| c.resolution == Resolution::Resolved && c.target.as_deref() == Some(qname))
        .map(|c| &*c.enclosing)
        .collect()
}

fn refs_to<'a>(facts: &'a RepoFacts<'_>, target: &str) -> Vec<&'a Ref> {
    facts
        .refs_to
        .get(target)
        .map(|ids| ids.iter().map(|i| &facts.refs[*i as usize]).collect())
        .unwrap_or_default()
}

fn scope_of(facts: &RepoFacts<'_>, r: &Ref) -> String {
    facts
        .enclosing(&facts.modules[&r.module], r.node)
        .to_string()
}

fn resolutions<'a>(facts: &'a RepoFacts<'_>, hit: Resolution) -> Vec<(&'a str, Option<&'a str>)> {
    facts
        .call_sites
        .iter()
        .filter(|c| c.resolution == hit)
        .map(|c| (&*c.enclosing, c.target.as_deref()))
        .collect()
}

#[test]
fn type_comments_are_the_annotations_they_spell() {
    // cityscapesScripts annotates three modules by PEP 484 comments: every
    // reader of an annotation or a return sees the declared type. This test
    // asserts the annotations alone; #32 over them is a rules test.
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "from typing import List\n",
            "class C:\n",
            "    def f(self,\n",
            "          a,  # type: List[str]\n",
            "          b=1,  # type: int\n",
            "          ):\n",
            "        # type: (...) -> bool\n",
            "        return bool(a)\n",
            "def g(x, y):\n",
            "    # type: (int, str) -> None\n",
            "    return None\n",
            "def h(z):\n",
            "    return z  # type: not a type at all\n",
        ),
    )]);
    let facts = stack.facts();
    assert_eq!(
        annotations(facts, "m.C.f"),
        [None, Some("List[str]".into()), Some("int".into())]
    );
    assert_eq!(returns(facts, "m.C.f"), Some("bool".into()));
    assert_eq!(
        annotations(facts, "m.g"),
        [Some("int".into()), Some("str".into())]
    );
    assert_eq!(returns(facts, "m.g"), Some("None".into()));
    assert_eq!(returns(facts, "m.h"), None);
    assert!(facts.errors.is_empty());
}

fn annotations(facts: &RepoFacts<'_>, qname: &str) -> Vec<Option<String>> {
    let sym = &facts.symbols[qname];
    let m = &facts.modules[&sym.module];
    let Cn::Stmt(Stmt::FunctionDef(def)) = m.nodes[sym.node as usize] else {
        panic!("{qname} is a def");
    };
    def.parameters
        .args
        .iter()
        .map(|a| {
            Cn::Param(&a.parameter)
                .stamped()
                .and_then(|i| m.annotation(i))
                .map(unparse::expr)
        })
        .collect()
}

fn returns(facts: &RepoFacts<'_>, qname: &str) -> Option<String> {
    let sym = &facts.symbols[qname];
    facts.modules[&sym.module]
        .returns(sym.node)
        .map(unparse::expr)
}

#[test]
fn direct_call_resolution_and_imports() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/a.py", "def f():\n    pass\n"),
        (
            "pkg/b.py",
            concat!(
                "from pkg.a import f\n",
                "import pkg.a\n",
                "def g():\n",
                "    f()\n",
                "    pkg.a.f()\n",
                "    unknown()\n",
            ),
        ),
    ]);
    let facts = stack.facts();
    assert_eq!(resolved_callers(facts, "pkg.a.f"), ["pkg.b.g", "pkg.b.g"]);
    assert_eq!(resolutions(facts, Resolution::Unresolved).len(), 1);
}

#[test]
fn local_shadowing_blocks_resolution() {
    let (_dir, stack) = build(&[("m.py", "def f():\n    pass\ndef g(f):\n    f()\n")]);
    assert!(resolved_callers(stack.facts(), "m.f").is_empty());
}

#[test]
fn plain_receiver_name_match_is_by_name_not_resolved() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "class A:\n",
            "    def only_here(self):\n        pass\n",
            "    def shared(self):\n        pass\n",
            "class B:\n",
            "    def shared(self):\n        pass\n",
            "def use(a):\n",
            "    a.only_here()\n",
            "    a.shared()\n",
        ),
    )]);
    let facts = stack.facts();
    assert_eq!(
        resolutions(facts, Resolution::ByName),
        [("m.use", Some("m.A.only_here"))]
    );
    // a plain-receiver name match is a guess: never a resolved caller edge
    assert!(resolved_callers(facts, "m.A.only_here").is_empty());
    let shared: Vec<&Vec<_>> = facts
        .call_sites
        .iter()
        .filter(|c| c.resolution == Resolution::Ambiguous)
        .map(|c| &c.candidates)
        .collect();
    assert_eq!(shared.len(), 1);
    let mut names: Vec<&str> = shared[0].iter().map(|q| &**q).collect();
    names.sort_unstable();
    assert_eq!(names, ["m.A.shared", "m.B.shared"]);
}

#[test]
fn self_calls_resolve_within_hierarchy() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "class Base:\n",
            "    def helper(self):\n        pass\n",
            "class Child(Base):\n",
            "    def run(self):\n",
            "        self.helper()\n",
        ),
    )]);
    assert_eq!(
        resolved_callers(stack.facts(), "m.Base.helper"),
        ["m.Child.run"]
    );
}

#[test]
fn refs_distinguish_callee_from_escape() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f():\n    pass\ndef g():\n    f()\n    h = f\n    return h\n",
    )]);
    let mut kinds: Vec<&str> = refs_to(stack.facts(), "m.f")
        .iter()
        .map(|r| r.kind.value())
        .collect();
    kinds.sort_unstable();
    assert_eq!(kinds, ["callee", "load"]);
}

#[test]
fn a_chain_prefix_is_a_ref_on_the_attribute_that_closes_it() {
    let (_dir, stack) = build(&[
        ("state.py", "cache = {}\n"),
        (
            "user.py",
            "import state\ndef f():\n    state.cache.update({1: 2})\n",
        ),
    ]);
    let facts = stack.facts();
    let hits = refs_to(facts, "state.cache");
    assert_eq!(hits.len(), 1);
    let m = &facts.modules["user"];
    let parent = m.parent_of(hits[0].node).expect("the `update` attribute");
    assert_eq!(m.nodes[parent as usize].kind(), Kind::Attribute);
    let Cn::Expr(ruff_python_ast::Expr::Attribute(a)) = m.nodes[parent as usize] else {
        panic!("an attribute");
    };
    assert_eq!(a.attr.as_str(), "update");
}

#[test]
fn function_level_imports_resolve_and_body_defs_are_local() {
    // a def or import at the top of a function body is a local whose loads
    // resolve to what it binds, so `refs_to` has a key for a closure
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/m.py",
            "def f():\n    return 1\ndef g():\n    return 2\n",
        ),
        (
            "use.py",
            concat!(
                "def run():\n",
                "    from pkg.m import f\n",
                "    import pkg.m as pm\n",
                "    def helper():\n",
                "        inner = [x for x in range(3)]\n",
                "        return inner\n",
                "    return f() + pm.g() + helper()\n",
            ),
        ),
    ]);
    let facts = stack.facts();
    let mut targets: Vec<&str> = facts
        .call_sites
        .iter()
        .filter(|c| &*c.module == "use" && c.resolution == Resolution::Resolved)
        .filter_map(|c| c.target.as_deref())
        .collect();
    targets.sort_unstable();
    targets.dedup();
    assert_eq!(targets, ["pkg.m.f", "pkg.m.g", "use.run.helper"]);
    let kinds: Vec<RefKind> = refs_to(facts, "use.run.helper")
        .iter()
        .map(|r| r.kind)
        .collect();
    assert_eq!(kinds, [RefKind::Callee]);
    assert!(resolutions(facts, Resolution::Unresolved).is_empty());
}

#[test]
fn re_export_hops_are_refs_on_the_alias() {
    // a `from M import name` or `M.name` that `resolve_qname` follows through
    // M's import binding records a LOAD on the alias `M.name` beside the ref
    // on the origin; a direct import hops nothing
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/base.py", "def helper():\n    return 1\n"),
        ("pkg/util.py", "from pkg.base import helper\n"),
        (
            "pkg/app.py",
            "from pkg.util import helper\ndef run():\n    return helper()\n",
        ),
        (
            "pkg/app2.py",
            "import pkg.util\ndef run2():\n    return pkg.util.helper.__name__\n",
        ),
        (
            "pkg/app3.py",
            "def run3():\n    from pkg.base import helper\n    return helper()\n",
        ),
    ]);
    let facts = stack.facts();
    let mut hops: Vec<(&str, &str)> = refs_to(facts, "pkg.util.helper")
        .iter()
        .map(|r| (&*r.module, r.kind.value()))
        .collect();
    hops.sort_unstable();
    assert_eq!(hops, [("pkg.app", "load"), ("pkg.app2", "load")]);
    let mut origin: Vec<(&str, &str)> = refs_to(facts, "pkg.base.helper")
        .iter()
        .map(|r| (&*r.module, r.kind.value()))
        .collect();
    origin.sort_unstable();
    assert_eq!(
        origin,
        [
            // app2's chain prefix lands on the origin too
            ("pkg.app", "callee"),
            ("pkg.app", "load"),
            ("pkg.app2", "load"),
            ("pkg.app3", "callee"),
            ("pkg.app3", "load"),
            ("pkg.util", "load"),
        ]
    );
    let load = refs_to(facts, "pkg.base.helper")
        .into_iter()
        .find(|r| &*r.module == "pkg.app3" && r.kind == RefKind::Load)
        .expect("app3's function-level import");
    assert_eq!(scope_of(facts, load), "pkg.app3.run3");
}

#[test]
fn a_lambda_default_does_not_reopen_the_defs_call_sites() {
    // the signature flag nests: a lambda's own signature ends, the def's
    // defaults after it are still no call site of the def
    let (_dir, stack) = build(&[(
        "m.py",
        "def g():\n    return 0\ndef f(a=lambda: 1, b=g()):\n    return a, b\n",
    )]);
    assert!(stack.facts().call_sites.is_empty());
}

#[test]
fn signature_expressions_are_refs() {
    // a default, an annotation or a return annotation is a reference like any
    // other load, resolved in the enclosing scope and indexed under the def
    // whose signature holds it
    let (_dir, stack) = build(&[
        (
            "t.py",
            "class T:\n    pass\nDEFAULT = 3\ndef cb():\n    return 0\n",
        ),
        (
            "m.py",
            concat!(
                "from t import T, DEFAULT, cb\n",
                "def f(x: T = None, y=DEFAULT, *, z: 'T' = 1, w=cb()) -> T:\n",
                "    def inner(a=cb):\n",
                "        return a\n",
                "    return inner, (lambda q=cb: q)\n",
            ),
        ),
    ]);
    let facts = stack.facts();
    // a default's call runs once at definition: a ref, never a call site
    assert!(facts.call_sites.iter().all(|c| &*c.module != "m"));
    let m = &facts.modules["m"];
    let mut by_target: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (target, ids) in &facts.refs_to {
        let mut scopes: Vec<String> = ids
            .iter()
            .map(|i| &facts.refs[*i as usize])
            .filter(|r| &*r.module == "m" && m.nodes[r.node as usize].kind() != Kind::Alias)
            .map(|r| scope_of(facts, r))
            .collect();
        scopes.sort();
        by_target.insert(target, scopes);
    }
    let want: BTreeMap<&str, Vec<String>> = [
        ("t.T", vec!["m.f".to_string(), "m.f".to_string()]),
        ("t.DEFAULT", vec!["m.f".to_string()]),
        (
            "t.cb",
            vec![
                "m.f".to_string(),
                "m.f".to_string(),
                "m.f.inner".to_string(),
            ],
        ),
        ("m.f.inner", vec!["m.f".to_string()]),
    ]
    .into_iter()
    .collect();
    assert_eq!(by_target, want);
}

#[test]
fn a_store_rebinds_the_frames_own_name() {
    // a module-scope or `global` store to a name the module imported is a
    // STORE on the module's own binding (m.f), never on the origin (other.f);
    // a nested def's name rebound is a STORE on the nested symbol; a
    // function-level import alias rebound is a plain local
    let (_dir, stack) = build(&[
        ("other.py", "def f():\n    return 1\n"),
        (
            "m.py",
            concat!(
                "from other import f\n",
                "if f is None:\n    f = None\n",
                "def reset():\n    global f\n    f = None\n",
                "def load():\n",
                "    def _tidy(r):\n        return r\n",
                "    _tidy = staticmethod(_tidy)\n",
                "    return _tidy\n",
                "def g():\n    from other import f\n    f = None\n    return f\n",
            ),
        ),
    ]);
    let facts = stack.facts();
    let mut stores: Vec<(&str, String)> = facts
        .refs
        .iter()
        .filter(|r| r.kind == RefKind::Store)
        .map(|r| (&*r.target, scope_of(facts, r)))
        .collect();
    stores.sort();
    assert_eq!(
        stores,
        [
            ("m.f", "m.reset".to_string()),
            ("m.load._tidy", "m.load".to_string()),
        ]
    );
}

#[test]
fn nested_def_refs_are_indexed_at_any_depth() {
    // a nested def held by value, a nested class instantiated and a closure
    // calling itself all reach the nested symbol; a local shadowing the name
    // resolves to nothing
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def run(rows):\n",
            "    def _walk(n):\n",
            "        return [_walk(c) for c in n]\n",
            "    class Local:\n",
            "        def go(self):\n",
            "            return _walk(self)\n",
            "    table = {'walk': _walk}\n",
            "    def shadow(_walk):\n",
            "        return _walk(1)\n",
            "    return Local().go(), table, shadow\n",
        ),
    )]);
    let facts = stack.facts();
    let mut scopes: Vec<(&str, String)> = refs_to(facts, "m.run._walk")
        .iter()
        .map(|r| (r.kind.value(), scope_of(facts, r)))
        .collect();
    scopes.sort();
    scopes.dedup();
    assert_eq!(
        scopes,
        [
            ("callee", "m.run.Local.go".to_string()),
            ("callee", "m.run._walk".to_string()),
            ("load", "m.run".to_string()),
        ]
    );
    assert_eq!(resolved_callers(facts, "m.run.Local"), ["m.run"]);
    let m = &facts.modules["m"];
    let named: Vec<&str> = facts
        .call_sites
        .iter()
        .filter(|c| c.resolution == Resolution::Unresolved)
        .filter(|c| match m.nodes[c.node as usize] {
            Cn::Expr(ruff_python_ast::Expr::Call(call)) => {
                matches!(&*call.func, ruff_python_ast::Expr::Name(_))
            }
            _ => false,
        })
        .map(|c| &*c.enclosing)
        .collect();
    assert_eq!(named, ["m.run.shadow"]);
}

#[test]
fn subscript_and_attribute_stores_do_not_rebind_their_root() {
    // `os.environ[k] = v` at module level must not shadow `os` as a local
    // binding, or every later `os.x` in the module resolves to `m.os.x`
    let (_dir, stack) = build(&[
        (
            "m.py",
            concat!(
                "import os\nimport cfg\n",
                "os.environ['A'] = '1'\n",
                "cfg.value = 2\n",
                "x, (y, z) = 1, (2, 3)\n",
            ),
        ),
        ("cfg.py", "value = 1\n"),
    ]);
    let facts = stack.facts();
    let b = &facts.modules["m"].bindings;
    assert_eq!(b.get("os").map(|q| &**q), Some("os"));
    assert_eq!(b.get("cfg").map(|q| &**q), Some("cfg"));
    let mut roots: Vec<&str> = ["x", "y", "z"]
        .iter()
        .map(|n| &**b.get(*n).expect("a bound name"))
        .collect();
    roots.sort_unstable();
    assert_eq!(roots, ["m.x", "m.y", "m.z"]);
    // the cross-module reassignment is a STORE on the target, not a read
    let stores: Vec<(&str, &str)> = facts
        .refs
        .iter()
        .filter(|r| r.kind == RefKind::Store)
        .map(|r| (&*r.module, &*r.target))
        .collect();
    assert_eq!(stores, [("m", "cfg.value")]);
}
