//! `util.rs` and `framework.rs` over one mini repo.
//!
//! Every helper here is also judged through a rule that reads it (#6, #10,
//! #32, #38, #40, #41, #48, #55). These pin the helpers on their own, and
//! the expected values are what they answer on the repo built below.

use sightline_core::findings::SpanEdit;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::RepoFacts;
use sightline_py_facts::module::Module;
use sightline_py_rules::framework;
use sightline_py_rules::util;
use sightline_testkit::{PyStack, build};

const M: &str = concat!(
    "import os.path\n",            // 1
    "\n\n",                        // 2, 3
    "def keep(fn):\n",             // 4
    "    return fn\n",             // 5
    "\n\n",                        // 6, 7
    "@keep\n",                     // 8
    "def held(x):\n",              // 9
    "    return x\n",              // 10
    "\n\n",                        // 11, 12
    "def solo():\n",               // 13
    "    x = 1\n",                 // 14
    "    return x\n",              // 15
    "\n\n",                        // 16, 17
    "def two():\n",                // 18
    "    y = 'abc'\n",             // 19
    "    z = os.path.join('a')\n", // 20
    "    return y, z\n",           // 21
    "\n\n",                        // 22, 23
    "class Base:\n",               // 24
    "    def run(self):\n",        // 25
    "        return 1\n",          // 26
    "\n\n",                        // 27, 28
    "class Child(Base):\n",        // 29
    "    def run(self):\n",        // 30
    "        return 2\n",          // 31
);

const TESTS: &str = concat!(
    "class TestThing:\n",
    "    def test_ok(self):\n",
    "        assert True\n",
    "\n\n",
    "def test_free():\n",
    "    assert True\n",
    "\n\n",
    "def helper():\n",
    "    return 1\n",
);

fn repo() -> (tempfile::TempDir, PyStack) {
    build(&[
        ("pkg/__init__.py", ""),
        ("pkg/m.py", M),
        ("pkg/n.py", "def held(x):\n    return x\n"),
        ("tests/test_it.py", TESTS),
    ])
}

fn module<'a, 't>(facts: &'a RepoFacts<'t>) -> &'a Module<'t> {
    &facts.modules["pkg.m"]
}

fn rows(edits: &[SpanEdit]) -> Vec<(u32, u32, u32, &str)> {
    edits
        .iter()
        .map(|e| (e.line, e.col_start, e.col_end, e.text.as_str()))
        .collect()
}

#[test]
fn deletion_blanks_the_statement_and_its_decorators() {
    let (_dir, stack) = repo();
    let facts = stack.facts();
    let m = module(facts);

    let x = m.nodes(&[Kind::Assign], Some("pkg.m.solo"), false)[0];
    assert_eq!(rows(&util::deletion(m, x)), [(14, 0, 9, "")]);

    let y = m.nodes(&[Kind::Assign], Some("pkg.m.two"), false)[0];
    assert_eq!(rows(&util::deletion(m, y)), [(19, 0, 13, "")]);

    // the decorator line joins the span
    let held = facts.symbols["pkg.m.held"].node;
    assert_eq!(
        rows(&util::deletion(m, held)),
        [(8, 0, 5, ""), (9, 0, 12, ""), (10, 0, 12, "")]
    );

    // taking the only statement of a block would empty it
    let only = m.nodes(&[Kind::Return], Some("pkg.m.keep"), false)[0];
    assert_eq!(util::deletion(m, only), []);
}

#[test]
fn enclosing_at_line_is_the_innermost_span() {
    let (_dir, stack) = repo();
    let facts = stack.facts();
    let m = module(facts);
    let at = |line| util::enclosing_at_line(facts, m, line);
    assert_eq!(at(1), "pkg.m");
    assert_eq!(at(9), "pkg.m.held");
    assert_eq!(at(14), "pkg.m.solo");
    assert_eq!(at(26), "pkg.m.Base.run");
    assert_eq!(at(27), "pkg.m");
}

#[test]
fn library_name_spells_a_call_through_the_bindings() {
    let (_dir, stack) = repo();
    let facts = stack.facts();
    let m = module(facts);
    let call = m.nodes(&[Kind::Call], Some("pkg.m.two"), false)[0];
    let sightline_py_facts::cn::Cn::Expr(ruff_python_ast::Expr::Call(c)) = m.nodes[call as usize]
    else {
        panic!("a Call bucket holds calls")
    };
    assert_eq!(
        util::library_name(m, &c.func).as_deref(),
        Some("os.path.join")
    );
}

#[test]
fn nontrivial_literal_keeps_strings_of_three_or_more() {
    let (_dir, stack) = repo();
    let facts = stack.facts();
    let m = module(facts);
    let assign = m.nodes(&[Kind::Assign], Some("pkg.m.two"), false)[0];
    let sightline_py_facts::cn::Cn::Stmt(ruff_python_ast::Stmt::Assign(a)) =
        m.nodes[assign as usize]
    else {
        panic!("an Assign bucket holds assignments")
    };
    assert_eq!(
        util::nontrivial_literal(Some(&a.value)),
        Some(("str", "'abc'".to_string()))
    );

    let short = m.nodes(&[Kind::Assign], Some("pkg.m.solo"), false)[0];
    let sightline_py_facts::cn::Cn::Stmt(ruff_python_ast::Stmt::Assign(a)) =
        m.nodes[short as usize]
    else {
        panic!("an Assign bucket holds assignments")
    };
    // `1` is a number, not a string of three code points
    assert_eq!(util::nontrivial_literal(Some(&a.value)), None);
}

#[test]
fn the_test_runners_collect_only_what_they_can_see() {
    let (_dir, stack) = repo();
    let facts = stack.facts();
    let found: Vec<&str> = util::iter_test_functions(facts)
        .map(|(_, sym)| &*sym.qname)
        .collect();
    assert_eq!(found, ["test_it.TestThing.test_ok", "test_it.test_free"]);
}

#[test]
fn util_reads_the_boundary_and_the_owner() {
    let (_dir, stack) = repo();
    let facts = stack.facts();
    let m = module(facts);
    assert!(util::is_exported(facts, m, &facts.symbols["pkg.m.solo"]));
    assert_eq!(
        util::owner_of(facts, &facts.symbols["pkg.m.Child.run"]).map(|s| &*s.qname),
        Some("pkg.m.Child")
    );
    let held = util::fn_of(m, &facts.symbols["pkg.m.held"]);
    let mut names: Vec<String> = util::decorator_names(held).into_iter().collect();
    names.sort_unstable();
    assert_eq!(names, ["keep"]);
}

#[test]
fn framework_reads_the_dispatch_contract() {
    let (_dir, stack) = repo();
    let facts = stack.facts();
    let m = module(facts);

    // `keep` returns its first parameter, so `@keep` fixes `held`'s signature
    assert!(framework::is_registered(
        facts,
        &facts.symbols["pkg.m.held"],
        None
    ));
    assert!(!framework::is_registered(
        facts,
        &facts.symbols["pkg.m.solo"],
        None
    ));

    assert!(framework::is_override_fixed(
        facts,
        &facts.symbols["pkg.m.Child.run"]
    ));
    assert!(!framework::is_override_fixed(
        facts,
        &facts.symbols["pkg.m.Base.run"]
    ));
    let mut inherited: Vec<String> = framework::inherited_method_names(facts, "pkg.m.Child")
        .into_iter()
        .collect();
    inherited.sort_unstable();
    assert_eq!(inherited, ["run"]);
    assert!(!framework::metaclassed(facts, "pkg.m.Child"));

    // `held` is spelled by two prod modules with the same parameters
    let mut plugins: Vec<(String, Vec<String>)> =
        framework::plugin_signatures(facts).into_iter().collect();
    plugins.sort();
    assert_eq!(plugins, [("held".to_string(), vec!["x".to_string()])]);

    let run = util::fn_of(m, &facts.symbols["pkg.m.Base.run"]);
    assert!(!framework::is_stub(&run.body));
}
