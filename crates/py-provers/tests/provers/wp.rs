//! Port of REF `tests/provers/test_wp.py`: closed-world escapes, effects over
//! the SCC condensation, callers, usage footprints. Every named escape reason
//! is pinned here.

use ruff_python_ast::Stmt;
use sightline_core::findings::Qname;
use sightline_core::verdict::CwVerdict;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::RefKind;
use sightline_py_provers::callgraph::callers_of;
use sightline_py_provers::effects::{Effects, UNNAMED, raised_name};
use sightline_testkit::PyStack;
use sightline_testkit::build;

fn verdict<'a>(stack: &'a PyStack, qname: &str) -> &'a CwVerdict {
    stack.provers.closed_world(stack.facts()).verdict(qname)
}

fn reason<'a>(stack: &'a PyStack, qname: &str) -> Option<&'a str> {
    verdict(stack, qname).reason.as_deref()
}

fn effects<'a>(stack: &'a PyStack, qname: &str) -> &'a Effects {
    stack
        .provers
        .effects(stack.facts())
        .get(qname)
        .unwrap_or_else(|| panic!("{qname} has a summary"))
}

fn atoms(stack: &PyStack, qname: &str) -> Vec<String> {
    effects(stack, qname).atoms.iter().cloned().collect()
}

fn reasons(stack: &PyStack, qname: &str) -> Vec<String> {
    let mut out: Vec<String> = verdict(stack, qname).reasons.iter().cloned().collect();
    out.sort();
    out
}

// --- closed world -------------------------------------------------------------

#[test]
fn passes_for_plain_internal_function() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def _helper(x):\n    return x\ndef use():\n    return _helper(1)\n",
    )]);
    let v = verdict(&stack, "m._helper");
    assert!(v.passed && v.reason.is_none());
}

#[test]
fn reexport_escape() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", "from pkg.impl import helper\n"),
        ("pkg/impl.py", "def helper(x):\n    return x\n"),
    ]);
    assert_eq!(reason(&stack, "pkg.impl.helper"), Some("re-export"));
}

#[test]
fn all_listing_is_reexport() {
    let (_dir, stack) = build(&[(
        "m.py",
        "__all__ = ['helper']\ndef helper(x):\n    return x\n",
    )]);
    assert_eq!(reason(&stack, "m.helper"), Some("re-export"));
}

#[test]
fn reference_escape() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def _cb(x):\n    return x\n",
            "def use(reg):\n    reg.register(_cb)\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m._cb"), Some("reference-escape"));
}

#[test]
fn unknown_decorator_escape() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "from flask import route\n",
            "@route('/x')\n",
            "def _handler(req):\n    return req\n",
            "@staticmethod\n",
            "def _ok(x):\n    return x\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m._handler"), Some("unknown-decorator"));
}

#[test]
fn kwargs_forward_escape() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def _sink(a, **kw):\n    return a\n",
            "def open_fn(**kw):\n    return _sink(1, **kw)\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m._sink"), Some("kwargs-forward"));
}

#[test]
fn dynamic_access_escape() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def _target(x):\n    return x\n",
            "def dispatch(mod):\n    return getattr(mod, '_target')\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m._target"), Some("dynamic-access"));
}

/// `@mylib.cache` is not functools' cache: a bare-attr match let a
/// signature-changing wrapper pass. Builtins, functools/typing/abc homes and
/// property accessors keep the signature.
#[test]
fn decorators_resolve_through_bindings() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/mylib.py",
            "def cache(fn):\n    return lambda x: fn(str(x))\n",
        ),
        (
            "pkg/core.py",
            concat!(
                "import functools\n",
                "from pkg import mylib\n",
                "@mylib.cache\n",
                "def _wrapped(x):\n    return x\n",
                "@functools.lru_cache(maxsize=2)\n",
                "def _cached(x):\n    return x\n",
                "class C:\n",
                "    @property\n",
                "    def p(self):\n        return 1\n",
                "    @p.setter\n",
                "    def p(self, v):\n        pass\n",
                "def use() -> None:\n    _wrapped(1)\n    _cached(2)\n",
            ),
        ),
    ]);
    assert_eq!(
        reason(&stack, "pkg.core._wrapped"),
        Some("unknown-decorator")
    );
    assert!(verdict(&stack, "pkg.core._cached").passed);
    assert!(verdict(&stack, "pkg.core.C.p").passed);
}

/// `json.JSONEncoder` calls `default` from library code no repo file shows; an
/// internal-only chain keeps the world closed.
#[test]
fn framework_base_escape() {
    let (_dir, stack) = build(&[(
        "enc.py",
        concat!(
            "import json\n",
            "class E(json.JSONEncoder):\n",
            "    def default(self, o):\n        return o\n",
            "class B:\n    pass\n",
            "class F(B):\n",
            "    def default(self, o):\n        return o\n",
            "def use() -> None:\n    E().default(1)\n    F().default(1)\n",
        ),
    )]);
    assert_eq!(reason(&stack, "enc.E.default"), Some("framework-base"));
    assert!(verdict(&stack, "enc.F.default").passed);
}

/// abc/typing markers *would* keep the world closed, and today every external
/// base is a framework base. This pins today's behaviour; a change that closes
/// the world on such a base is a wanted move, not a regression.
#[test]
fn typing_markers_read_as_framework_bases_today() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import abc\nimport json\nimport enum\n",
            "from typing import Generic, Protocol, TypeVar\n",
            "T = TypeVar('T')\n",
            "class A(abc.ABC):\n    def ma(self, k):\n        return k\n",
            "class G(Generic[T]):\n    def mg(self, k):\n        return k\n",
            "class P(Protocol):\n    def mp(self, k):\n        return k\n",
            "class E(json.JSONEncoder):\n    def me(self, k):\n        return k\n",
            "class N(enum.Enum):\n    X = 1\n    def mn(self, k):\n        return k\n",
            "def use(a: A, g: G, p: P, e: E, n: N) -> None:\n",
            "    a.ma(1)\n    g.mg(1)\n    p.mp(1)\n    e.me(1)\n    n.mn(1)\n",
        ),
    )]);
    for q in ["m.A.ma", "m.G.mg", "m.P.mp", "m.E.me", "m.N.mn"] {
        assert_eq!(reason(&stack, q), Some("framework-base"), "{q}");
    }
}

/// `functools.singledispatch` and `typing.no_type_check` live in the allowed
/// homes yet change what a call does; the named pure wrappers (cache, final,
/// contextmanager) pass.
#[test]
fn signature_keepers_are_a_name_set_not_a_home() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import functools\nimport typing\nfrom contextlib import contextmanager\n",
            "@functools.singledispatch\ndef _disp(x):\n    return x\n",
            "@typing.no_type_check\ndef _unchecked(x):\n    return x\n",
            "@functools.cache\ndef _cached(x):\n    return x\n",
            "@typing.final\ndef _fin(x):\n    return x\n",
            "@contextmanager\ndef _ctx(x):\n    yield x\n",
            "def use() -> None:\n",
            "    _disp(1)\n    _unchecked(1)\n    _cached(1)\n    _fin(1)\n",
            "    with _ctx(1):\n        pass\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m._disp"), Some("unknown-decorator"));
    assert_eq!(reason(&stack, "m._unchecked"), Some("unknown-decorator"));
    assert!(verdict(&stack, "m._cached").passed);
    assert!(verdict(&stack, "m._fin").passed);
    assert!(verdict(&stack, "m._ctx").passed);
}

/// `'on_{}'.format(e)` and a suffix-only build (`f'{e}_hook'`) reach the
/// methods #32 keeps live - one `literal_affixes` for both - and never open the
/// receiver's whole class.
#[test]
fn built_names_reach_by_the_affixes_liveness_reads() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "class Bus:\n",
            "    def on_ping(self, k):\n        return k\n",
            "    def ping_hook(self, k):\n        return k\n",
            "    def other(self, k):\n        return k\n",
            "    def d1(self, e):\n        return getattr(self, 'on_{}'.format(e))()\n",
            "    def d2(self, e):\n        return getattr(self, f'{e}_hook')()\n",
            "def use(b: Bus) -> None:\n    b.on_ping(1)\n    b.ping_hook(1)\n    b.other(1)\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m.Bus.on_ping"), Some("dynamic-access"));
    assert_eq!(reason(&stack, "m.Bus.ping_hook"), Some("dynamic-access"));
    assert!(verdict(&stack, "m.Bus.other").passed);
}

#[test]
fn dynamic_access_reaches_by_prefix_class_and_module() {
    let (_dir, stack) = build(&[
        (
            "bus.py",
            concat!(
                "from operator import methodcaller\n",
                "class Bus:\n",
                "    def on_ping(self, k):\n        return k\n",
                "    def off_x(self, k):\n        return k\n",
                "    def dispatch(self, name):\n        return getattr(self, f'on_{name}')()\n",
                "    def dispatch2(self, name):\n        return methodcaller('on_' + name)(self)\n",
                "class Opaque:\n",
                "    def m(self, k):\n        return k\n",
                "    def find(self, name):\n        return getattr(self, name)\n",
                "def handle_a(k):\n    return k\n",
                "def run(n):\n    return globals()[n]()\n",
                "def use(b: Bus, o: Opaque) -> None:\n",
                "    b.on_ping(1)\n    b.off_x(1)\n    o.m(1)\n    handle_a(1)\n",
            ),
        ),
        (
            "other.py",
            "def free(k):\n    return k\ndef use() -> None:\n    free(1)\n",
        ),
    ]);
    // the `on_` prefix, and not the prefix
    assert_eq!(reason(&stack, "bus.Bus.on_ping"), Some("dynamic-access"));
    assert!(verdict(&stack, "bus.Bus.off_x").passed);
    // getattr(self, n): the chain; another class stays closed
    assert_eq!(reason(&stack, "bus.Opaque.m"), Some("dynamic-access"));
    assert!(verdict(&stack, "bus.Bus.dispatch").passed);
    // globals()[n]: the module; no reach into another module
    assert_eq!(reason(&stack, "bus.handle_a"), Some("dynamic-access"));
    assert!(verdict(&stack, "other.free").passed);
}

/// `reason` is the first in the computed order; `reasons` is the set effects
/// read, so a decorated def that is also re-exported by value still counts as
/// a wrapper.
#[test]
fn a_verdict_carries_every_escape_that_holds() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def retry(fn):\n    return fn\n",
            "@retry\n",
            "def g(x):\n    return x\n",
            "H = g\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m.g"), Some("reference-escape"));
    assert_eq!(
        reasons(&stack, "m.g"),
        ["reference-escape", "unknown-decorator"]
    );
}

/// A re-export shim (`globals()[name] = value`) mints no caller.
#[test]
fn a_globals_store_is_not_a_read() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def free(k):\n    return k\n",
            "def shim(name, value) -> None:\n    globals()[name] = value\n",
            "def use() -> None:\n    free(1)\n",
        ),
    )]);
    assert!(verdict(&stack, "m.free").passed);
}

/// `import_module(f"plugins.{k}")` reaches every module under `plugins.` as
/// `globals()[k]` reaches its own; an opaque name (`__import__(name)`), a
/// constant one (an ordinary import edge) and a test's dynamic import (its
/// subject, not the program's dispatch) reach nothing.
#[test]
fn a_built_import_name_reaches_the_modules_its_affixes_match() {
    let (_dir, stack) = build(&[
        ("plugins/__init__.py", ""),
        ("plugins/a.py", "def run(k):\n    return k\n"),
        ("plugins/b.py", "def run(k):\n    return k\n"),
        ("ext/__init__.py", ""),
        ("ext/x.py", "def run(k):\n    return k\n"),
        ("core.py", "def free(k):\n    return k\n"),
        (
            "app.py",
            concat!(
                "import importlib\n",
                "def load(k):\n    return importlib.import_module(f'plugins.{k}')\n",
                "def load_any(name):\n    return __import__(name)\n",
                "def load_core():\n    return importlib.import_module('core')\n",
            ),
        ),
        (
            "tests/test_ext.py",
            concat!(
                "from importlib import import_module\n",
                "def test_ext(k):\n    assert import_module(f'ext.{k}')\n",
            ),
        ),
    ]);
    assert_eq!(reason(&stack, "plugins.a.run"), Some("dynamic-access"));
    assert_eq!(reason(&stack, "plugins.b.run"), Some("dynamic-access"));
    assert!(verdict(&stack, "core.free").passed);
    assert!(verdict(&stack, "ext.x.run").passed);
}

/// `getattr(self, f.name) for f in fields(self)` names dataclass fields, so
/// the class's methods stay closed; `f.name` drawn from anything else (a
/// param) is the opaque read that opens the chain.
#[test]
fn a_fields_iteration_reads_fields_never_methods() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import dataclasses\n",
            "from dataclasses import dataclass, fields\n",
            "@dataclass\nclass R:\n    x: int = 0\n",
            "    def m(self, k):\n        return k\n",
            "    def to(self):\n",
            "        return {f.name: getattr(self, f.name) for f in fields(self)}\n",
            "    def reset(self):\n",
            "        for f in dataclasses.fields(type(self)):\n",
            "            setattr(self, f.name, 0)\n",
            "@dataclass\nclass O:\n    x: int = 0\n",
            "    def m(self, k):\n        return k\n",
            "    def find(self, f):\n        return getattr(self, f.name)\n",
            "def use(r: R, o: O) -> None:\n    r.m(1)\n    o.m(1)\n",
        ),
    )]);
    assert!(verdict(&stack, "m.R.m").passed);
    assert_eq!(reason(&stack, "m.O.m"), Some("dynamic-access"));
}

/// `getattr(obj, k)` opens the chain of the class `obj` is declared as - a
/// param's annotation or a `self` field's, through aliases and string forms -
/// never every function; an undeclared receiver (a bare param, a field only
/// ever assigned) still reaches nothing.
#[test]
fn an_opaque_reflector_reaches_the_receivers_declared_class() {
    let (_dir, stack) = build(&[
        (
            "models.py",
            concat!(
                "class Node:\n    def m(self, k):\n        return k\n",
                "class Leaf(Node):\n    def n(self, k):\n        return k\n",
                "class Other:\n    def o(self, k):\n        return k\n",
                "class Third:\n    def t(self, k):\n        return k\n",
                "NodeLike = Node | None\n",
            ),
        ),
        (
            "m.py",
            concat!(
                "from models import NodeLike, Other, Third\n",
                "def d(node: NodeLike, k):\n    return getattr(node, k)\n",
                "class Holder:\n",
                "    child: 'Other'\n",
                "    def __init__(self, third: Third):\n        self.plain = third\n",
                "    def find(self, k):\n        return getattr(self.child, k)\n",
                "    def find_plain(self, k):\n        return getattr(self.plain, k)\n",
                "def u(o, k):\n    return getattr(o, k)\n",
            ),
        ),
        (
            "n.py",
            "def free(k):\n    return k\ndef use() -> None:\n    free(1)\n",
        ),
    ]);
    // the alias, a subclass, the field
    assert_eq!(reason(&stack, "models.Node.m"), Some("dynamic-access"));
    assert_eq!(reason(&stack, "models.Leaf.n"), Some("dynamic-access"));
    assert_eq!(reason(&stack, "models.Other.o"), Some("dynamic-access"));
    // an unannotated field, and a bare param
    assert!(verdict(&stack, "models.Third.t").passed);
    assert!(verdict(&stack, "n.free").passed);
}

/// `getattr(o, n)` on a receiver declared as nothing opens no world: the "else
/// repo-wide" arm emptied #4/#5/#37 on every corpus repo at 8 TP / 0 FP per
/// seed-20260841 sample - reverted, and the bar this pins.
#[test]
fn an_opaque_reflector_on_an_undeclared_receiver_reaches_nothing() {
    let (_dir, stack) = build(&[
        ("m.py", "def d(o, n):\n    return getattr(o, n)\n"),
        (
            "n.py",
            "def free(k):\n    return k\ndef use() -> None:\n    free(1)\n",
        ),
    ]);
    assert!(verdict(&stack, "n.free").passed);
}

// --- effects -------------------------------------------------------------------

#[test]
fn direct_and_propagated_writes() {
    let (_dir, stack) = build(&[
        ("state.py", "cache = {}\n"),
        (
            "m.py",
            concat!(
                "from state import cache\n",
                "def write(k, v):\n    cache[k] = v\n",
                "def outer(k):\n    write(k, 1)\n",
                "def pure(x):\n    return x + 1\n",
            ),
        ),
    ]);
    assert!(atoms(&stack, "m.write").contains(&"gw:state.cache".to_string()));
    // propagated up
    assert!(atoms(&stack, "m.outer").contains(&"gw:state.cache".to_string()));
    assert!(effects(&stack, "m.pure").clean());
}

#[test]
fn unresolved_call_taints_unknown_not_clean() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def _cb():\n    pass\n",
            "def use(reg):\n    reg.hooks.invoke(_cb)\n",
            "def caller():\n    return use(None)\n",
        ),
    )]);
    // reg.hooks.invoke unresolved -> not clean
    assert!(effects(&stack, "m.use").unknown);
    assert!(effects(&stack, "m.caller").unknown);
}

#[test]
fn io_and_cycles() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def a(n):\n",
            "    print(n)\n",
            "    return b(n - 1) if n else 0\n",
            "def b(n):\n    return a(n)\n",
        ),
    )]);
    assert!(atoms(&stack, "m.a").contains(&"io".to_string()));
    assert!(atoms(&stack, "m.b").contains(&"io".to_string()));
}

/// `self.v = v` is `mutates-self`, never `mutates-arg`; a class call hands the
/// caller a fresh object it owns, so the atom is dropped across that edge - the
/// constructor's io still crosses it, and a method call keeps the receiver atom.
#[test]
fn receiver_writes_stay_with_a_fresh_object() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import socket\n",
            "class P:\n",
            "    def __init__(self, v):\n",
            "        self.v = v\n",
            "    def get_x(self):\n",
            "        self._c = 1\n",
            "        return self._c\n",
            "    def get_y(self):\n",
            "        return self.get_x()\n",
            "class Conn:\n",
            "    def __init__(self):\n",
            "        self.s = socket.socket()\n",
            "def get_p():\n",
            "    return P(1)\n",
            "def get_conn():\n",
            "    return Conn()\n",
            "def get_v(p: P):\n",
            "    p.v = 2\n",
            "    return p\n",
        ),
    )]);
    assert_eq!(atoms(&stack, "m.P.__init__"), ["mutates-self"]);
    assert!(effects(&stack, "m.get_p").clean());
    assert_eq!(atoms(&stack, "m.get_conn"), ["io"]);
    assert_eq!(atoms(&stack, "m.P.get_x"), ["mutates-self"]);
    // propagated over a method call
    assert_eq!(atoms(&stack, "m.P.get_y"), ["mutates-self"]);
    assert_eq!(atoms(&stack, "m.get_v"), ["mutates-arg"]);
}

/// A wrapper or an override runs something else than the body the graph read:
/// its callers are unknown. A passed reference only says a caller may be
/// hidden: the direct caller's summary stands.
#[test]
fn escapes_that_change_the_called_body_taint_callers() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def retry(fn):\n    return fn\n",
            "@retry\n",
            "def g(x):\n    return x + 1\n",
            "def get_g(x):\n    return g(x) * 2\n",
            "class Base:\n",
            "    def m(self, x):\n        return x + 1\n",
            "class Child(Base):\n",
            "    def m(self, x):\n        print(x)\n        return x\n",
            "def get_m(b: Base, x):\n    return Base.m(b, x) * 2\n",
            "def _cb(x):\n    return x\n",
            "def use(reg):\n    reg.hooks.register(_cb)\n",
            "def get_cb():\n    return _cb(3)\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m.g"), Some("unknown-decorator"));
    assert!(effects(&stack, "m.get_g").unknown);
    assert_eq!(reason(&stack, "m.Base.m"), Some("method-override"));
    assert!(effects(&stack, "m.get_m").unknown);
    assert_eq!(reason(&stack, "m._cb"), Some("reference-escape"));
    assert!(effects(&stack, "m._cb").unknown && effects(&stack, "m.get_cb").clean());
}

/// The world names the first escape only; a wrapper or an override behind a
/// passed reference still runs something else when called.
#[test]
fn a_called_escape_behind_another_still_taints_callers() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def retry(fn):\n    return fn\n",
            "@retry\n",
            "def g(x):\n    print(x)\n",
            "def get_g(x):\n    return g(x)\n",
            "H = g\n",
            "class Base:\n",
            "    def m(self, x):\n        return x\n",
            "class Child(Base):\n",
            "    def m(self, x):\n        print(x)\n",
            "def get_m(b: Base, x):\n    return Base.m(b, x)\n",
            "M = Base.m\n",
        ),
    )]);
    assert_eq!(reason(&stack, "m.g"), Some("reference-escape"));
    assert!(effects(&stack, "m.get_g").unknown);
    assert_eq!(reason(&stack, "m.Base.m"), Some("reference-escape"));
    assert!(effects(&stack, "m.get_m").unknown);
}

/// A callee's receiver write means to its caller whatever the call site passed
/// over: a param reached into is `mutates-arg`, one passed over whole
/// `slots-arg`, a global a `gw:`/`gs:`, a local the body bound or a display
/// nothing. A class call's object is the caller's fresh one, but a constructor
/// writing *through* a field reaches what it was given.
#[test]
fn receiver_atoms_translate_by_the_call_site_owner() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "class Bag:\n",
            "    def __init__(self, items):\n",
            "        self.items = items\n",
            "        self.items += [1]\n",
            "class Reg:\n",
            "    def __init__(self, table):\n",
            "        self.table = table\n",
            "        self.table['k'] = 1\n",
            "    def put_key(self, k):\n",
            "        self.table[k] = 1\n",
            "    def mark(self):\n",
            "        self.n = 1\n",
            "class Plain:\n",
            "    def __init__(self, v):\n",
            "        self.v = v\n",
            "ITEMS = []\n",
            "class Owner:\n",
            "    def __init__(self):\n",
            "        self.items = []\n",
            "    def grow(self):\n",
            "        Bag(self.items)\n",
            "def get_bag(xs):\n    Bag(xs)\n",
            "def get_reg(t):\n    Reg(t)\n",
            "def get_plain():\n    Plain(1)\n",
            "def get_fresh():\n    Bag([])\n",
            "def get_shared():\n    Bag(ITEMS)\n",
            "def get_marked(r: Reg):\n    r.mark()\n",
            "def get_own():\n    r = Reg({})\n    r.mark()\n    return 1\n",
            "def get_unbound(b: Reg):\n    Reg.mark(b)\n",
            "def set_item(p):\n    p.items[0] = 1\n",
        ),
    )]);
    assert_eq!(
        atoms(&stack, "m.Bag.__init__"),
        ["mutates-field", "mutates-self"]
    );
    assert_eq!(atoms(&stack, "m.Reg.put_key"), ["mutates-field"]);
    assert_eq!(atoms(&stack, "m.Reg.mark"), ["mutates-self"]);
    assert_eq!(atoms(&stack, "m.get_bag"), ["mutates-arg"]);
    assert!(!effects(&stack, "m.get_bag").unknown);
    assert_eq!(atoms(&stack, "m.get_reg"), ["mutates-arg"]);
    assert!(effects(&stack, "m.get_plain").clean() && effects(&stack, "m.get_fresh").clean());
    assert!(atoms(&stack, "m.get_shared").contains(&"gw:m.ITEMS".to_string()));
    assert_eq!(atoms(&stack, "m.Owner.grow"), ["mutates-field"]);
    // a receiver passed over whole is `slots-arg`, not a write by this caller:
    // `r.mark()` writes r's own slots, `p.items[0] = 1` reaches in
    assert_eq!(atoms(&stack, "m.get_marked"), ["slots-arg"]);
    assert!(effects(&stack, "m.get_own").clean());
    assert_eq!(atoms(&stack, "m.get_unbound"), ["slots-arg"]);
    assert_eq!(atoms(&stack, "m.set_item"), ["mutates-arg"]);
}

/// A global written only where the function tested it can only fill what the
/// test found missing; the same store unguarded is a `gw:`.
#[test]
fn a_memo_fill_is_kept_apart_from_a_store() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "CACHE = {}\n",
            "SEEN = {}\n",
            "def c(k):\n    if k not in CACHE:\n        CACHE[k] = 1\n",
            "def d(k):\n    SEEN[k] = 1\n",
        ),
    )]);
    assert_eq!(atoms(&stack, "m.c"), ["gm:m.CACHE", "gr:m.CACHE"]);
    assert_eq!(atoms(&stack, "m.d"), ["gw:m.SEEN"]);
}

#[test]
fn rebound_callee_taints_callers() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def f(x):\n    return x\n",
            "def get_f(x):\n    return f(x)\n",
            "def patch():\n    global f\n    f = print\n",
        ),
    )]);
    assert!(effects(&stack, "m.get_f").unknown);
}

#[test]
fn alias_of_a_param_mutated_is_mutates_arg() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def get_alias(xs):\n    ys = xs\n    ys.append(1)\n    return ys\n",
            "def get_direct(xs):\n    xs.append(1)\n    return xs\n",
            "def get_fresh(xs):\n    ys = list(xs)\n    ys.append(1)\n    return ys\n",
        ),
    )]);
    assert_eq!(atoms(&stack, "m.get_alias"), ["mutates-arg"]);
    assert_eq!(atoms(&stack, "m.get_direct"), ["mutates-arg"]);
    assert!(effects(&stack, "m.get_fresh").clean());
}

/// `xs += [1]` on a param or an alias of one is the in-place write `.append`
/// is; a fresh copy is the body's own, and `pos += 1` on a number rebinds.
#[test]
fn augassign_of_a_display_is_mutates_arg() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def get_direct(xs):\n    xs += [1]\n    return xs\n",
            "def get_alias(xs):\n    ys = xs\n    ys += [1]\n    return ys\n",
            "def get_fresh(xs):\n    ys = list(xs)\n    ys += [1]\n    return ys\n",
            "def get_pos(xs, i):\n    pos = i\n    pos += 1\n    return xs[pos]\n",
            "def get_count(n):\n    n += 1\n    return n\n",
        ),
    )]);
    assert_eq!(atoms(&stack, "m.get_direct"), ["mutates-arg"]);
    assert_eq!(atoms(&stack, "m.get_alias"), ["mutates-arg"]);
    assert!(effects(&stack, "m.get_fresh").clean());
    assert!(effects(&stack, "m.get_pos").clean());
    assert!(effects(&stack, "m.get_count").clean());
}

/// A repo-class instance is a mutable global like a container is: reading it
/// is a `gr:`; an external class's or a function's result is no one's to read
/// as mutable.
#[test]
fn attribute_stores_and_chains_on_a_global() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "from pathlib import Path\n",
            "class Config:\n    x = 0\n    items = []\n",
            "def make():\n    return Config()\n",
            "CONFIG = Config()\n",
            "MADE = make()\n",
            "HOME = Path('x')\n",
            "BUCKET = []\n",
            "def get_mode():\n    CONFIG.x = 1\n    return 1\n",
            "def get_nested():\n    CONFIG.items.append(1)\n    return 1\n",
            "def get_read():\n    return CONFIG.x\n",
            "def get_made():\n    return MADE.x\n",
            "def get_home():\n    return HOME.name\n",
            "def get_fresh():\n    return CONFIG.copy().items.append(1)\n",
            "def get_bucket():\n    BUCKET.append(1)\n    return 1\n",
        ),
    )]);
    assert_eq!(atoms(&stack, "m.get_mode"), ["gw:m.CONFIG"]);
    assert_eq!(atoms(&stack, "m.get_nested"), ["gw:m.CONFIG"]);
    assert_eq!(atoms(&stack, "m.get_read"), ["gr:m.CONFIG"]);
    assert!(effects(&stack, "m.get_made").clean() && effects(&stack, "m.get_home").clean());
    // a call result is the caller's own: CONFIG is read, never written
    assert_eq!(atoms(&stack, "m.get_fresh"), ["gr:m.CONFIG"]);
    assert_eq!(atoms(&stack, "m.get_bucket"), ["gw:m.BUCKET"]);
}

#[test]
fn global_declared_stores_are_global_writes() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "COUNT = 0\n",
            "def get_global():\n    global COUNT\n    COUNT += 1\n    return COUNT\n",
            "def get_global_set():\n    global COUNT\n    COUNT = 1\n    return 1\n",
            "def get_local():\n    COUNT = 1\n    return COUNT\n",
            "COUNT = 2\n",
        ),
    )]);
    let facts = stack.facts();
    assert_eq!(atoms(&stack, "m.get_global"), ["gw:m.COUNT"]);
    assert_eq!(atoms(&stack, "m.get_global_set"), ["gw:m.COUNT"]);
    // a local shadow
    assert!(effects(&stack, "m.get_local").clean());
    let module = facts.modules.get("m").expect("the fixture module");
    let mut lines: Vec<u32> = facts.refs_to["m.COUNT"]
        .iter()
        .map(|i| &facts.refs[*i as usize])
        .filter(|r| r.kind == RefKind::Store)
        .map(|r| module.line_of(r.node))
        .collect();
    lines.sort();
    // the module rebinding still counts
    assert_eq!(lines, [4, 8, 13]);
}

/// A nested def is `nested` for the closed world, so a `nonlocal` write needs
/// no atom of its own to stay off the clean set.
#[test]
fn closure_writes_are_unknown_not_clean() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def outer():\n    n = 0\n",
            "    def get_n():\n        nonlocal n\n        n += 1\n        return n\n",
            "    return get_n\n",
        ),
    )]);
    assert!(effects(&stack, "m.outer.get_n").unknown);
    assert!(!effects(&stack, "m.outer.get_n").clean());
}

#[test]
fn io_members_and_roots_through_bindings() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import json\nimport os\nimport random\nimport threading\nimport time\n",
            "from time import sleep\n",
            "def get_slept():\n    time.sleep(0.01)\n    return 1\n",
            "def get_slept_bare():\n    sleep(0.01)\n    return 1\n",
            "def get_dumped(obj, fh):\n    json.dump(obj, fh)\n    return 1\n",
            "def get_seeded():\n    random.seed(1)\n    return 1\n",
            "def get_shuffled(xs):\n    random.shuffle(xs)\n    return xs\n",
            "def get_thread(fn):\n    threading.Thread(target=fn)\n    return 1\n",
            "def get_rand():\n    return random.random()\n",
            "def get_text(obj):\n    return json.dumps(obj) + os.path.join('a', 'b')\n",
            "def get_now():\n    return time.time()\n",
        ),
    )]);
    for q in [
        "get_slept",
        "get_slept_bare",
        "get_dumped",
        "get_seeded",
        "get_thread",
    ] {
        assert!(
            atoms(&stack, &format!("m.{q}")).contains(&"io".to_string()),
            "{q}"
        );
    }
    // a read of `xs`, the shuffle is the module's
    assert_eq!(atoms(&stack, "m.get_shuffled"), ["io"]);
    for q in ["get_rand", "get_text", "get_now"] {
        assert!(effects(&stack, &format!("m.{q}")).clean(), "{q}");
    }
}

/// No repo class defines `append`: no repo body can run, the site is external
/// and a local list's growth is the function's own business. A param's is
/// `mutates-arg`; a chain receiver stays unresolved.
#[test]
fn plain_receiver_without_a_repo_method_is_external() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def build():\n    xs = []\n    xs.append(1)\n    return xs\n",
            "def push(xs):\n    xs.append(1)\n    return xs\n",
            "def use(reg):\n    reg.hooks.invoke(1)\n    return 1\n",
        ),
    )]);
    let facts = stack.facts();
    assert!(effects(&stack, "m.build").clean());
    assert_eq!(atoms(&stack, "m.push"), ["mutates-arg"]);
    assert!(!effects(&stack, "m.push").unknown);
    assert!(effects(&stack, "m.use").unknown);
    let at = |line: u32| {
        facts
            .call_sites
            .iter()
            .find(|c| c.lineno == line)
            .map(|c| c.resolution)
    };
    assert_eq!(at(3), Some(sightline_py_facts::model::Resolution::External));
    assert_eq!(
        at(9),
        Some(sightline_py_facts::model::Resolution::Unresolved)
    );
}

/// "No repo class defines the method" is not "no repo body runs": a library
/// base's template method calls the repo's hook, and a prod `__getattr__`
/// answers any name. A test's fake proxy is not the program judged, and an
/// internal-only class keeps the arm.
#[test]
fn a_library_template_or_prod_proxy_may_run_a_repo_body() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import json\n",
            "class Enc(json.JSONEncoder):\n",
            "    def default(self, o):\n        print(o)\n        return o\n",
            "class F:\n    pass\n",
            "def get_encoded(e: Enc, o):\n    return e.encode(o)\n",
            "def get_plain(f: F):\n    return f.encode(1)\n",
            "def push(xs):\n    xs.append(1)\n    return xs\n",
        ),
    )]);
    assert!(effects(&stack, "m.get_encoded").unknown);
    assert!(effects(&stack, "m.get_plain").clean());
    assert_eq!(atoms(&stack, "m.push"), ["mutates-arg"]);
    assert!(!effects(&stack, "m.push").unknown);

    let (_dir2, proxied) = build(&[(
        "m.py",
        concat!(
            "class Proxy:\n",
            "    def __getattr__(self, name):\n        print(name)\n        return name\n",
            "def get_frob(p):\n    return p.frob()\n",
        ),
    )]);
    assert!(effects(&proxied, "m.get_frob").unknown);

    let (_dir3, faked) = build(&[
        ("m.py", "def get_frob(p):\n    return p.frob()\n"),
        (
            "tests/test_m.py",
            concat!(
                "class Fake:\n",
                "    def __getattr__(self, name):\n        return name\n",
            ),
        ),
    ]);
    assert!(effects(&faked, "m.get_frob").clean());
}

/// The last dotted part through the module's bindings; a bare re-raise is
/// `None`, what the bindings cannot name is `UNNAMED`; and a raise is control
/// flow - `atoms` and `clean` never see one.
#[test]
fn raised_name_reads_the_bindings() {
    let (_dir, stack) = build(&[
        ("errors.py", "class ParseError(Exception):\n    pass\n"),
        (
            "m.py",
            concat!(
                "import errors\n",
                "from errors import ParseError as PE\n",
                "def parse(s):\n    raise errors.ParseError(s)\n",
                "def check(x):\n    raise PE\n",
                "def walk(x):\n    raise ValueError\n",
                "def again():\n    try:\n        pass\n    except Exception:\n        raise\n",
                "def rethrow(e):\n    raise e\n",
                "def field(self):\n    raise self.Error()\n",
            ),
        ),
    ]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let named = |q: &str| -> Vec<Option<String>> {
        module
            .nodes(&[Kind::Raise], Some(q), false)
            .into_iter()
            .map(|n| match module.nodes[n as usize] {
                Cn::Stmt(Stmt::Raise(r)) => raised_name(module, r),
                _ => None,
            })
            .collect()
    };
    assert_eq!(named("m.parse"), [Some("ParseError".to_string())]);
    assert_eq!(named("m.check"), [Some("ParseError".to_string())]);
    assert_eq!(named("m.walk"), [Some("ValueError".to_string())]);
    assert_eq!(named("m.again"), [None]);
    assert_eq!(named("m.rethrow"), [Some(UNNAMED.to_string())]);
    assert_eq!(named("m.field"), [Some(UNNAMED.to_string())]);
    assert!(effects(&stack, "m.walk").clean());
}

// --- callers and usage ---------------------------------------------------------

#[test]
fn prod_test_split() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/core.py", "def act(x):\n    return x\n"),
        (
            "pkg/use.py",
            "from pkg.core import act\ndef run():\n    return act(1)\n",
        ),
        (
            "tests/test_core.py",
            "from pkg.core import act\ndef test_act():\n    assert act(2)\n",
        ),
    ]);
    let facts = stack.facts();
    let cs = callers_of("pkg.core.act", facts, stack.provers.calls(facts));
    let names = |sites: &[&sightline_py_facts::model::CallSite]| -> Vec<Qname> {
        sites.iter().map(|c| c.enclosing.clone()).collect()
    };
    assert_eq!(names(&cs.prod), [Qname::from("pkg.use.run")]);
    assert_eq!(names(&cs.test), [Qname::from("test_core.test_act")]);
}

#[test]
fn footprint_attrs_calls_iteration_forwarding() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def f(rect, items, conn, blob):\n",
            "    area = rect.w * rect.h\n",
            "    for i in items:\n",
            "        conn.send(i)\n",
            "    return consume(blob)\n",
            "def consume(b):\n    return b\n",
        ),
    )]);
    let facts = stack.facts();
    let scope = stack
        .provers
        .scope_of(facts, "m.f")
        .expect("a function scope");
    let fp = scope.footprints(facts);

    assert_eq!(
        fp["rect"].attrs.iter().cloned().collect::<Vec<_>>(),
        ["h", "w"]
    );
    assert!(fp["items"].iterated && fp["items"].attrs.is_empty());
    assert_eq!(
        fp["conn"].called.iter().cloned().collect::<Vec<_>>(),
        ["send"]
    );
    assert_eq!(
        fp["blob"]
            .forwarded
            .iter()
            .map(|(q, at)| (q.to_string(), *at))
            .collect::<Vec<_>>(),
        [("m.consume".to_string(), 0)]
    );
    assert!(fp["rect"].forwarded.is_empty());
}

// --- the published escape ------------------------------------------------------

const LIB: [(&str, &str); 2] = [
    ("src/mypkg/__init__.py", ""),
    (
        "src/mypkg/api.py",
        concat!(
            "def helper(x):\n    return x\n",
            "def _helper(x):\n    return x\n",
            "def use():\n    return helper(1) + _helper(2)\n",
        ),
    ),
];
const DIST: &str = "[project]\nname = \"mypkg\"\n\n[build-system]\nrequires = [\"setuptools\"]\n";

/// A published module's public defs have callers this tree cannot show.
#[test]
fn published_escape() {
    let mut files: Vec<(&str, &str)> = LIB.to_vec();
    files.push(("pyproject.toml", DIST));
    let (_dir, stack) = build(&files);
    assert_eq!(reason(&stack, "mypkg.api.helper"), Some("published"));
    // the private twin ships nothing: its callers are all here
    assert!(verdict(&stack, "mypkg.api._helper").passed);
}

#[test]
fn the_same_tree_unpackaged_is_closed() {
    let mut files: Vec<(&str, &str)> = LIB.to_vec();
    files.push(("pyproject.toml", "[project]\nname = \"mypkg\"\n"));
    let (_dir, stack) = build(&files);
    assert!(verdict(&stack, "mypkg.api.helper").passed);
}
