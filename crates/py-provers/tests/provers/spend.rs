//! Port of `provers/spend.py`'s contract (REF has no test file of its own for
//! it: #59's assertions live in the rules tests phase 5 ports). Expected values
//! from a probe through REF's own `spend_of` / `own_params` /
//! `handed_through`: `sightline-phase3/scratch/py-provers-a/probe_shipping_spend.py`.

use std::collections::BTreeSet;

use ruff_python_ast::{Expr, Stmt};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::NodeIndex;
use sightline_py_provers::spend::{handed_through, own_params, spend_of};
use sightline_testkit::build;

const SRC: &str = concat!(
    "import subprocess\n",
    "import shutil\n",
    "def runs(cmd):\n",
    "    return subprocess.run(cmd)\n",
    "def given(source):\n",
    "    return source.read_text()\n",
    "def loops():\n",
    "    while True:\n",
    "        pass\n",
    "def pure(a, b):\n",
    "    return a + b\n",
    "def registers():\n",
    "    @app.route('/')\n",
    "    def handler():\n",
    "        shutil.rmtree('x')\n",
    "    return handler\n",
    "def owns():\n",
    "    def inner():\n",
    "        shutil.rmtree('y')\n",
    "    return inner\n",
    "def callee(target, other):\n",
    "    return target\n",
    "def caller(p, q):\n",
    "    return callee(p, other=q)\n",
);

#[test]
fn a_spend_is_the_first_catalog_call_on_what_the_body_was_not_given() {
    let (_dir, stack) = build(&[("m.py", SRC)]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let def = |q: &str| -> NodeIndex { facts.symbols.get(q).expect("the fixture symbol").node };
    let spend = |q: &str| spend_of(module, module.nodes[def(q) as usize], None);

    assert_eq!(spend("m.runs").as_deref(), Some("subprocess.run"));
    // the cost is in the signature, not hidden by it
    assert_eq!(spend("m.given"), None);
    assert_eq!(spend("m.loops").as_deref(), Some("while True"));
    assert_eq!(spend("m.pure"), None);
    // a handler a factory registers spends when the handler runs
    assert_eq!(spend("m.registers"), None);
    // an undecorated nested def is the body's own
    assert_eq!(spend("m.owns").as_deref(), Some("shutil.rmtree"));

    assert_eq!(
        own_params(module.nodes[def("m.pure") as usize])
            .into_iter()
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert!(own_params(module.nodes[def("m.loops") as usize]).is_empty());
}

#[test]
fn handed_through_names_the_callee_params_the_caller_filled_from_its_own() {
    let (_dir, stack) = build(&[("m.py", SRC)]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let caller = facts.symbols.get("m.caller").expect("the fixture symbol");
    let callee = facts.symbols.get("m.callee").expect("the fixture symbol");
    let Cn::Stmt(Stmt::FunctionDef(callee_def)) = module.nodes[callee.node as usize] else {
        panic!("not a def")
    };
    let call = module
        .nodes(&[Kind::Call], Some("m.caller"), false)
        .into_iter()
        .find_map(|n| match module.nodes[n as usize] {
            Cn::Expr(Expr::Call(c)) => Some(c),
            _ => None,
        })
        .expect("the call site");
    let params: BTreeSet<String> = own_params(module.nodes[caller.node as usize]);

    assert_eq!(
        handed_through(call, callee_def, &params)
            .into_iter()
            .collect::<Vec<_>>(),
        ["other", "target"]
    );
}
