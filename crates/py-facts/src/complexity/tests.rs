//! `complexity`'s own tests, in their own file: a reader of the reading
//! does not pay them (#27).

use std::collections::HashMap;

use ruff_python_ast::{ModModule, Stmt};
use ruff_python_parser::{Parsed, parse_module};

use super::*;
use crate::astutil::subnodes;

fn parse(source: &str) -> Parsed<ModModule> {
    parse_module(source).expect("the fixture parses")
}

fn def(stmt: &Stmt) -> &StmtFunctionDef {
    match stmt {
        Stmt::FunctionDef(f) => f,
        other => panic!("{other:?} is not a def"),
    }
}

fn cc(source: &str) -> u32 {
    let parsed = parse(source);
    cognitive_complexity(def(&parsed.suite()[0]), 0)
}

/// The first method of the class the fixture opens with.
fn method(source: &str) -> u32 {
    let parsed = parse(source);
    let Stmt::ClassDef(c) = &parsed.suite()[0] else {
        panic!("the fixture opens with a class");
    };
    cognitive_complexity(def(&c.body[0]), 0)
}

#[test]
fn cognitive_complexity_orders_nesting() {
    let flat = cc("def f(x):\n    if x: return 1\n    if x: return 2\n");
    let nested = cc("def f(x):\n    if x:\n        if x:\n            return 1\n");
    assert!(nested > 0);
    assert!(flat > 0);
    let deep = cc(
        "def f(x):\n    for i in x:\n        if i:\n            if i > 1:\n                return i\n",
    );
    assert!(deep > flat);
}

#[test]
fn cognitive_complexity_is_sonars() {
    // one +1 per run of like boolean operators; an operator change is a new run
    assert_eq!(
        cc("def f(a, b, c):\n    if a and b and c:\n        return 1\n"),
        2
    );
    assert_eq!(
        cc("def f(a, b, c):\n    if a and b or c:\n        return 1\n"),
        3
    );
    // elif and else are +1 flat: no nesting penalty
    assert_eq!(
        cc(
            "def f(x):\n    if x:\n        return 1\n    elif x > 1:\n        return 2\n    else:\n        return 3\n"
        ),
        3
    );
    // nesting: an if inside an if is 1 + (1 + 1)
    assert_eq!(
        cc("def f(x):\n    if x:\n        if x:\n            return 1\n"),
        3
    );
    // comprehensions are free; a handler inside a loop nests
    assert_eq!(cc("def f(xs):\n    return [x for x in xs if x]\n"), 0);
    assert_eq!(
        cc(
            "def f(xs):\n    for x in xs:\n        try:\n            g()\n        except E:\n            pass\n"
        ),
        3
    );
    // `x or default` coalesces (Python's `??`); a run that decides counts
    assert_eq!(
        cc("def f(v):\n    return str(v.get('t') or '').strip()\n"),
        0
    );
    assert_eq!(cc("def f(a, b):\n    return a or b\n"), 1);
    // a direct recursive call is +1 flat
    assert_eq!(
        cc("def fact(n):\n    if n == 0:\n        return 1\n    return n * fact(n - 1)\n"),
        2
    );
    assert_eq!(
        cc("def fact(n):\n    if n == 0:\n        return 1\n    return n * other(n - 1)\n"),
        1
    );
    // a nested def is scored as its own finding, never in its parent too
    assert_eq!(
        cc("def f(n):\n    def g(k):\n        return g(k)\n    return f(n)\n"),
        1
    );
    assert_eq!(cc("def g(k):\n    return g(k)\n"), 1);
    // a method calling itself on its receiver is recursion too; another
    // method on the receiver is not
    let walk = |call: &str| {
        method(&format!(
            "class C:\n    def walk(self, n):\n        if n:\n            return {call}\n        return 0\n"
        ))
    };
    assert_eq!(walk("self.walk(n - 1)"), 2);
    assert_eq!(walk("cls.walk(n - 1)"), 2);
    assert_eq!(walk("self.other(n - 1)"), 1);
    assert_eq!(walk("node.walk(n - 1)"), 1);
}

/// Node identity for the test's parent index; wave 2 replaces it with the
/// `NodeIndex` facts stamp.
fn key(node: Cn<'_>) -> (Kind, usize) {
    let addr = match node {
        Cn::Module(m) => std::ptr::from_ref(m) as usize,
        Cn::Stmt(s) => std::ptr::from_ref(s) as usize,
        Cn::Elif(r) => r.as_ptr() as usize,
        Cn::Expr(e) => std::ptr::from_ref(e) as usize,
        Cn::Params(p) => std::ptr::from_ref(p) as usize,
        Cn::Param(p) => std::ptr::from_ref(p) as usize,
        Cn::Handler(h) => std::ptr::from_ref(h) as usize,
        Cn::Comp(c) => std::ptr::from_ref(c) as usize,
        Cn::Item(w) => std::ptr::from_ref(w) as usize,
        Cn::Case(c) => std::ptr::from_ref(c) as usize,
        Cn::Pattern(p) => std::ptr::from_ref(p) as usize,
        Cn::TypeParam(t) => std::ptr::from_ref(t) as usize,
        Cn::Alias(a) => std::ptr::from_ref(a) as usize,
        Cn::Keyword(k) => std::ptr::from_ref(k) as usize,
        Cn::Interp(i, _) => std::ptr::from_ref(i) as usize,
        Cn::Spec(s) => std::ptr::from_ref(s) as usize,
        Cn::CallGen(g, _) => std::ptr::from_ref(g) as usize,
        Cn::FConst { range, .. } => u32::from(range.start()) as usize,
        Cn::TypeIgnore(line) => line as usize,
    };
    (node.kind(), addr)
}

fn parents<'a>(root: Cn<'a>) -> HashMap<(Kind, usize), Cn<'a>> {
    let mut out = HashMap::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let mut kids = Vec::new();
        order::children(node, &mut kids);
        for kid in kids {
            out.insert(key(kid), node);
            stack.push(kid);
        }
    }
    out
}

#[test]
fn nesting_prices_a_body_at_a_call_site_depth() {
    let source = concat!(
        "def a(xs):\n",
        "    for x in xs:\n",
        "        if x:\n",
        "            r = h(x)\n",
        "        elif x is None:\n",
        "            r = h(0)\n",
        "        else:\n",
        "            r = h(1)\n",
        "    while xs and h(xs):\n",
        "        pass\n",
    );
    // for > if body: 2; an elif's body and its else sit at the if's body
    // depth (the elif is +1 flat); a loop's test is not its block
    assert_eq!(depths(source), [2, 2, 2, 0]);

    let helper = parse("def h(x):\n    if x:\n        return 1\n    return 0\n");
    assert_eq!(cognitive_complexity(def(&helper.suite()[0]), 0), 1);
    assert_eq!(cognitive_complexity(def(&helper.suite()[0]), 2), 3);
}
/// Shapes the corpus does not settle, each priced by CPython through
/// `sightline.complexity` (scratch/facts-ast/probe_cc.py).
#[test]
fn the_shapes_the_corpus_leaves_open() {
    let cases = [
        // CPython cannot tell `else:` holding one `if` from `elif`
        (
            "def f(x):\n    if x:\n        pass\n    else:\n        if x:\n            g()\n",
            2,
        ),
        (
            "def f(x):\n    if x:\n        pass\n    elif x:\n        g()\n",
            2,
        ),
        // two statements in the else make it an else again
        (
            "def f(x):\n    if x:\n        pass\n    else:\n        if x:\n            g()\n        h()\n",
            4,
        ),
        // a loop's else is never an elif, however it is spelled
        (
            "def f(xs):\n    for x in xs:\n        pass\n    else:\n        if x:\n            g()\n",
            4,
        ),
        (
            "def f(xs):\n    while xs:\n        pass\n    else:\n        g()\n",
            2,
        ),
        ("def f(x):\n    return 1 if x else (2 if x else 3)\n", 3),
        (
            "def f(x):\n    match x:\n        case 1:\n            if x:\n                g()\n        case _:\n            pass\n",
            3,
        ),
        // a lambda sinks its body; its defaults sit at the lambda's depth
        ("def f(x):\n    return lambda y: (1 if y else 2)\n", 2),
        ("def f(x):\n    return lambda y=(1 if x else 2): y\n", 1),
        // a nested class is walked, a nested def is not
        (
            "def f(x):\n    class C:\n        if x:\n            y = 1\n    return C\n",
            1,
        ),
        (
            "def f():\n    try:\n        g()\n    except E:\n        if 1:\n            h()\n    finally:\n        if 1:\n            k()\n",
            4,
        ),
        (
            "def f():\n    with open('x') as fh:\n        if fh:\n            g()\n",
            1,
        ),
        // a run closed by a literal is a default
        ("def f(a):\n    return a or []\n", 0),
        ("def f(a):\n    return a or (1, 2)\n", 0),
        ("def f(a, b):\n    return a and b\n", 1),
        (
            "async def f(xs):\n    async for x in xs:\n        if x:\n            g()\n",
            3,
        ),
        // a decorator, a default and a return annotation are all scored
        ("@deco(1 if x else 2)\ndef f():\n    pass\n", 1),
        ("def f(a=(1 if x else 2)):\n    pass\n", 1),
        ("def f() -> (int if x else str):\n    pass\n", 1),
        ("def f(xs):\n    return sum(x for x in xs if x)\n", 0),
        (
            "def f(xs):\n    while (n := next(xs)):\n        if n:\n            g()\n",
            3,
        ),
    ];
    for (source, want) in cases {
        assert_eq!(cc(source), want, "{source}");
    }
}

fn depths(source: &str) -> Vec<u32> {
    let parsed = parse(source);
    let root = Cn::Stmt(&parsed.suite()[0]);
    let index = parents(root);
    let parent = |n: Cn<'_>| index.get(&key(n)).copied();
    let mut calls = subnodes(root, |k| k == Kind::Call);
    calls.sort_by_key(|c| c.range(source).map(|r| u32::from(r.start())));
    calls.iter().map(|c| nesting_at(*c, &parent)).collect()
}

/// An `else:` holding one `if` is an elif to `nesting_at` too; a loop's
/// else is a block of its own; a ternary has no block, so it adds nothing.
#[test]
fn nesting_reads_the_same_three_shapes() {
    assert_eq!(
        depths(
            "def a(xs):\n    if xs:\n        h(1)\n    else:\n        if xs:\n            h(2)\n"
        ),
        [1, 1]
    );
    assert_eq!(
        depths(
            "def a(xs):\n    for x in xs:\n        pass\n    else:\n        if x:\n            h(3)\n"
        ),
        [2]
    );
    assert_eq!(
        depths("def a(x):\n    return h(1) if x else h(2)\n"),
        [0, 0]
    );
}
