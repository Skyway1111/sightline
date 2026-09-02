//! One pin per `Scope` query.

use ruff_python_ast::{Expr, Stmt};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::RepoFacts;
use sightline_py_facts::module::Module;
use sightline_py_facts::unparse;
use sightline_py_provers::scope::{Scope, bound_from, class_fields};
use sightline_testkit::PyStack;
use sightline_testkit::build;

const BODY: &str = concat!(
    "CACHE = {}\n",
    "\n",
    "def f(a, b, *, c=None):\n",
    "    x: int = 1\n",
    "    y = a\n",
    "    z = [1, 2]\n",
    "    if isinstance(a, int):\n",
    "        b.field = z\n",
    "    for item in z:\n",
    "        x = item\n",
    "    assert c is not None\n",
    "    return x, y, item\n",
);

fn scope_of<'a>(stack: &'a PyStack, qname: &str) -> &'a Scope {
    stack
        .provers
        .scope_of(stack.facts(), qname)
        .expect("a function scope")
}

fn sorted(names: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = names.iter().map(|n| (*n).to_string()).collect();
    out.sort();
    out
}

fn roots(scope: &Scope, facts: &RepoFacts<'_>, own_only: bool) -> Vec<String> {
    let mut out: Vec<String> = scope
        .writes(facts)
        .iter()
        .filter(|w| !own_only || w.own)
        .filter_map(|w| w.root.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn declared_is_params_plus_annotated_locals() {
    let (_dir, stack) = build(&[("m.py", BODY)]);
    let facts = stack.facts();
    let scope = scope_of(&stack, "m.f");

    assert_eq!(scope.params(facts), ["a", "b", "c"]);
    // `y`/`z` launder an inferred type
    assert_eq!(
        scope.declared(facts).iter().cloned().collect::<Vec<_>>(),
        sorted(&["a", "b", "c", "x"])
    );
}

#[test]
fn rebound_before_counts_earlier_bindings_and_spanning_loops() {
    let (_dir, stack) = build(&[("m.py", BODY)]);
    let facts = stack.facts();
    let scope = scope_of(&stack, "m.f");

    assert_eq!(scope.loops(facts), [(9, 10)]);
    // `x` is a declaration, not a rebinding
    assert_eq!(
        scope
            .rebound_before(facts, 6, false)
            .into_iter()
            .collect::<Vec<_>>(),
        ["y"]
    );
    // inside the loop `x = item` is on a path to line 10 though it follows it
    let at_ten = scope.rebound_before(facts, 10, false);
    assert!(at_ten.contains("x") && at_ten.contains("item"));
    assert!(!scope.rebound_before(facts, 8, false).contains("x"));
}

#[test]
fn stored_excludes_reference_writes_and_deletes() {
    let (_dir, stack) = build(&[("m.py", BODY)]);
    let facts = stack.facts();
    // not `b`, written through
    assert_eq!(
        scope_of(&stack, "m.f")
            .stored(facts)
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        sorted(&["x", "y", "z", "item"])
    );
}

#[test]
fn alias_taint_follows_bindings_rooted_outside_the_function() {
    let (_dir, stack) = build(&[("m.py", BODY)]);
    let facts = stack.facts();
    // `y = a` aliases a param; `z` is a fresh display, so its elements stay local
    assert_eq!(
        scope_of(&stack, "m.f")
            .alias_tainted(facts)
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        ["y"]
    );
}

#[test]
fn a_global_root_is_shared_and_taints_what_binds_to_it() {
    let (_dir, stack) = build(&[(
        "m.py",
        "CACHE = {}\n\ndef g():\n    global CACHE\n    CACHE = {}\n    v = CACHE\n    v['k'] = 1\n",
    )]);
    let facts = stack.facts();
    let scope = scope_of(&stack, "m.g");

    assert_eq!(
        scope.outer_names(facts).iter().cloned().collect::<Vec<_>>(),
        ["CACHE"]
    );
    // storing CACHE does not make it local
    assert_eq!(
        scope
            .alias_tainted(facts)
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        ["v"]
    );
}

#[test]
fn guards_are_param_checks_in_document_order() {
    let (_dir, stack) = build(&[("m.py", BODY)]);
    let facts = stack.facts();
    let rows: Vec<(&str, &str, Vec<String>)> = scope_of(&stack, "m.f")
        .guards(facts)
        .iter()
        .map(|g| (g.param.as_str(), g.kind, g.classes.clone()))
        .collect();

    assert_eq!(
        rows,
        [
            ("a", "isinstance", vec!["int".to_string()]),
            ("c", "is-not-none", Vec::new()),
        ]
    );
}

/// `isinstance(n, models.Node)` is a guard on Node when `models` is bound by
/// an import; a chain rooted at a local object (`kinds.T`) is no guard.
#[test]
fn guards_resolve_an_attribute_chain_class_through_bindings() {
    let (_dir, stack) = build(&[
        ("models.py", "class Node:\n    pass\n"),
        (
            "m.py",
            "import models\n\
             def f(n, o, kinds):\n\
                 \x20   if isinstance(n, models.Node):\n        return 1\n\
                 \x20   if isinstance(o, kinds.T):\n        return 2\n\
                 \x20   assert isinstance(kinds, (str, models.Node))\n",
        ),
    ]);
    let facts = stack.facts();
    let rows: Vec<(&str, Vec<String>)> = scope_of(&stack, "m.f")
        .guards(facts)
        .iter()
        .map(|g| (g.param.as_str(), g.classes.clone()))
        .collect();

    assert_eq!(
        rows,
        [
            ("n", vec!["Node".to_string()]),
            ("kinds", vec!["str".to_string(), "Node".to_string()]),
        ]
    );
}

/// A guard after the param is rebound judges a value no caller sent.
#[test]
fn guards_read_the_entry_value_only() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f(x, y, z):\n\
         \x20   if x is None:\n\
         \x20       x = 1\n\
         \x20   y = int(y)\n\
         \x20   if y is None:\n\
         \x20       return x\n\
         \x20   for x in [x]:\n\
         \x20       assert isinstance(x, int)\n\
         \x20   z: dict = dict(z)\n\
         \x20   if isinstance(z, dict):\n\
         \x20       return z\n",
    )]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let rows: Vec<(&str, u32)> = scope_of(&stack, "m.f")
        .guards(facts)
        .iter()
        .map(|g| (g.param.as_str(), module.line_of(g.node)))
        .collect();

    assert_eq!(rows, [("x", 2)]);
}

#[test]
fn footprints_cover_the_receiver_and_hide_shadowed_params() {
    let (_dir, stack) = build(&[(
        "m.py",
        "class C:\n\
         \x20   def m(self, items, key):\n\
         \x20       self.seen = 1\n\
         \x20       items.append(key)\n\
         \x20       def inner(items):\n\
         \x20           items.append(0)\n\
         \x20       return len(key)\n",
    )]);
    let facts = stack.facts();
    let scope = scope_of(&stack, "m.C.m");
    let fps = scope.footprints(facts);

    assert_eq!(
        fps["items"].called.iter().cloned().collect::<Vec<_>>(),
        ["append"]
    );
    assert!(fps["key"].sized);
    // `self.seen = 1` is a mutation
    assert_eq!(
        scope
            .mutated_params(facts)
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        sorted(&["self", "items"])
    );
    // inner's `items` is not this one
    assert!(fps["items"].forwarded.is_empty());
}

#[test]
fn a_keyword_argument_forwards_by_the_callee_signature() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def g(a, b):\n    return a.x + b.y\n\
         class C:\n\
         \x20   def m(self, a, b):\n        return a.x + b.y\n\
         \x20   def f(self, p, q, r, s):\n\
         \x20       g(b=p)\n\
         \x20       self.m(b=q)\n\
         \x20       g(r)\n\
         \x20       g(**s)\n",
    )]);
    let facts = stack.facts();
    let fps = scope_of(&stack, "m.C.f").footprints(facts);
    let forwarded = |p: &str| -> Vec<(String, usize)> {
        fps[p]
            .forwarded
            .iter()
            .map(|(q, at)| (q.to_string(), *at))
            .collect()
    };

    assert_eq!(forwarded("p"), [("m.g".to_string(), 1)]);
    assert!(!fps["p"].other);
    // the receiver is not a position
    assert_eq!(forwarded("q"), [("m.C.m".to_string(), 1)]);
    assert_eq!(forwarded("r"), [("m.g".to_string(), 0)]);
    // a splat names nothing
    assert!(forwarded("s").is_empty() && fps["s"].other);
}

#[test]
fn writes_inside_a_nested_def_are_not_own_scope() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def h(p):\n    def inner():\n        q = p\n    r = 1\n",
    )]);
    let facts = stack.facts();
    let scope = scope_of(&stack, "m.h");

    // the def itself binds `inner` in h; what inner binds is inner's
    assert_eq!(roots(scope, facts, true), sorted(&["inner", "r"]));
    assert_eq!(roots(scope, facts, false), sorted(&["inner", "q", "r"]));
    assert_eq!(
        scope
            .rebound_before(facts, 99, false)
            .into_iter()
            .collect::<Vec<_>>(),
        sorted(&["inner", "r"])
    );
}

/// `self.x: T = v` is a declaration written through `self`; a plain attribute
/// store is not; `declared` stays the params plus AnnAssign'd names.
#[test]
fn decl_marks_an_annotated_attribute_target_too() {
    let (_dir, stack) = build(&[(
        "m.py",
        "class C:\n\
         \x20   def __init__(self, v):\n\
         \x20       self.x: int = v\n\
         \x20       self.y = v\n\
         \x20       self.z: int\n",
    )]);
    let facts = stack.facts();
    let scope = scope_of(&stack, "m.C.__init__");
    let module = facts.modules.get("m").expect("the fixture module");
    let mut rows: Vec<(String, &str, bool)> = scope
        .writes(facts)
        .iter()
        .map(|w| {
            let attr = match module.nodes[w.node as usize] {
                Cn::Expr(Expr::Attribute(a)) => a.attr.to_string(),
                other => panic!("{:?} is no attribute write", other.kind()),
            };
            (attr, w.kind, w.decl)
        })
        .collect();
    rows.sort();

    assert_eq!(
        rows,
        [
            ("x".to_string(), "attr", true),
            ("y".to_string(), "attr", false),
            ("z".to_string(), "attr", true),
        ]
    );
    assert_eq!(
        scope.declared(facts).iter().cloned().collect::<Vec<_>>(),
        sorted(&["self", "v"])
    );
}

/// R20: the memo lives in `Provers`, not on the module.
#[test]
fn scope_is_built_once_and_memoized_per_symbol() {
    let (_dir, stack) = build(&[("m.py", BODY)]);
    let facts = stack.facts();
    let first = scope_of(&stack, "m.f");
    let second = scope_of(&stack, "m.f");

    assert!(std::ptr::eq(first, second));
    assert!(stack.provers.scope_of(facts, "m.CACHE").is_none());
}

#[test]
fn a_lambda_is_its_own_scope() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def k(xs):\n    f = lambda ys: [(w := y) for y in ys]\n    return f(xs)\n",
    )]);
    let facts = stack.facts();

    // `w`/`y` bind in the lambda
    assert_eq!(roots(scope_of(&stack, "m.k"), facts, true), ["f"]);
    let sym = facts.symbols.get("m.k").expect("the fixture symbol");
    let module = facts.modules.get("m").expect("the fixture module");
    assert!(matches!(
        module.nodes[sym.node as usize],
        Cn::Stmt(Stmt::FunctionDef(_))
    ));
}

#[test]
fn writes_in_a_loop_span_are_the_loops_stores() {
    let (_dir, stack) = build(&[("m.py", BODY)]);
    let facts = stack.facts();
    let scope = scope_of(&stack, "m.f");
    let module = facts.modules.get("m").expect("the fixture module");
    let sym = facts.symbols.get("m.f").expect("the fixture symbol");
    let Cn::Stmt(Stmt::FunctionDef(f)) = module.nodes[sym.node as usize] else {
        panic!("not a def")
    };
    let loop_stmt = Cn::Stmt(&f.body[4]).stamped().expect("a stamped node");
    let Stmt::For(for_stmt) = &f.body[4] else {
        panic!("body[4] is the for")
    };
    let inner = Cn::Stmt(&for_stmt.body[0])
        .stamped()
        .expect("a stamped node");
    let first = Cn::Stmt(&f.body[0]).stamped().expect("a stamped node");

    assert_eq!(scope.enclosing_loop(facts, inner), Some(loop_stmt));
    let mut rows: Vec<(String, &str)> = scope
        .writes_in(facts, loop_stmt)
        .into_iter()
        .map(|w| (w.root.clone().unwrap_or_default(), w.kind))
        .collect();
    rows.sort();
    rows.dedup();
    assert_eq!(
        rows,
        [("item".to_string(), "name"), ("x".to_string(), "name")]
    );
    assert_eq!(scope.enclosing_loop(facts, first), None);
}

/// The default `0` is signature, the lambda's and inner's `cfg` are theirs.
#[test]
fn uses_of_skips_a_nested_def_that_rebinds_the_name() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f(cfg: int = 0):\n\
         \x20   g = lambda cfg: cfg\n\
         \x20   def inner(cfg):\n\
         \x20       return cfg\n\
         \x20   return cfg, g, inner\n",
    )]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let lines: Vec<u32> = scope_of(&stack, "m.f")
        .uses_of(facts, "cfg")
        .into_iter()
        .map(|n| module.line_of(n))
        .collect();

    assert_eq!(lines, [5]);
}

#[test]
fn ancestor_ids_climbs_to_the_def_and_stops() {
    let (_dir, stack) = build(&[("m.py", "def f(a):\n    return [a + 1 for _ in a]\n")]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let scope = scope_of(&stack, "m.f");
    let sym = facts.symbols.get("m.f").expect("the fixture symbol");
    let binop = module.nodes(&[Kind::BinOp], Some("m.f"), true)[0];
    let Cn::Stmt(Stmt::FunctionDef(f)) = module.nodes[sym.node as usize] else {
        panic!("not a def")
    };
    let ret = Cn::Stmt(&f.body[0]).stamped().expect("a stamped node");

    let ids = scope.ancestor_ids(facts, [binop], false);

    assert!(!ids.contains(&sym.node) && !ids.contains(&binop));
    // the Return, and the comprehension between
    assert!(ids.contains(&ret));
    assert!(scope.ancestor_ids(facts, [binop], true).contains(&binop));
}

#[test]
fn class_fields_reads_the_body_and_self_stores_up_the_chain() {
    let (_dir, stack) = build(&[(
        "m.py",
        "class Base:\n\
         \x20   kind: str = 'b'\n\
         \x20   def __init__(self):\n\
         \x20       self.seen = []\n\
         class C(Base):\n\
         \x20   limit = 3\n\
         \x20   def load(self):\n\
         \x20       self.rows: list[int] = []\n\
         \x20       self.count = 0\n",
    )]);
    let facts = stack.facts();
    let fields = class_fields(facts, "m.C");
    let mut names: Vec<&str> = fields.keys().map(String::as_str).collect();
    names.sort();

    assert_eq!(names, ["count", "kind", "limit", "rows", "seen"]);
    // `self.x: T` is a claim; `kind` is inherited and class-level
    assert_eq!(
        unparse::expr(fields["rows"].expect("an annotation")),
        "list[int]"
    );
    assert_eq!(unparse::expr(fields["kind"].expect("an annotation")), "str");
    assert!(fields["count"].is_none() && fields["seen"].is_none());
}

/// A capture, a def and a class bind in f; a method binds in K and a
/// comprehension target in its own scope.
#[test]
fn match_captures_and_defs_rebind_but_comprehension_targets_do_not() {
    let (_dir, stack) = build(&[(
        "m.py",
        "def f(x, y, z, w, q):\n\
         \x20   match y:\n\
         \x20       case [x, *rest]:\n\
         \x20           pass\n\
         \x20       case {**z}:\n\
         \x20           pass\n\
         \x20   def w():\n\
         \x20       pass\n\
         \x20   class K:\n\
         \x20       def q(self):\n\
         \x20           pass\n\
         \x20   return [v for v in y]\n",
    )]);
    let facts = stack.facts();

    assert_eq!(
        scope_of(&stack, "m.f")
            .rebound_before(facts, 99, false)
            .into_iter()
            .collect::<Vec<_>>(),
        sorted(&["x", "rest", "z", "w", "K"])
    );
}

/// A `for` or a comprehension around the node, innermost first; a nested def
/// or lambda rebinding the name in between cuts the climb; the module's own
/// twin answers at module scope and in a class body.
#[test]
fn bound_from_names_the_iterable_an_enclosing_loop_binds_from() {
    let (_dir, stack) = build(&[(
        "m.py",
        "KEYS = ('a', 'b')\n\
         for k in KEYS:\n\
         \x20   getattr(object, k)\n\
         class C:\n\
         \x20   for k in KEYS:\n\
         \x20       getattr(object, k)\n\
         def f(xs):\n\
         \x20   for k in xs:\n\
         \x20       [getattr(object, k) for k in KEYS]\n\
         \x20       getattr(object, k)\n\
         \x20       (lambda k: getattr(object, k))(1)\n\
         \x20   return getattr(object, k)\n",
    )]);
    let facts = stack.facts();
    let module: &Module<'_> = facts.modules.get("m").expect("the fixture module");
    let bound: Vec<Option<String>> = module
        .nodes(&[Kind::Call], None, false)
        .into_iter()
        .filter(|n| {
            matches!(module.nodes[*n as usize],
                Cn::Expr(Expr::Call(c)) if matches!(&*c.func, Expr::Name(x) if x.id.as_str() == "getattr"))
        })
        .map(|n| bound_from(module, n, "k").map(unparse::expr))
        .collect();

    assert_eq!(
        bound,
        [
            Some("KEYS".to_string()),
            Some("KEYS".to_string()),
            Some("KEYS".to_string()),
            Some("xs".to_string()),
            None,
            None,
        ]
    );
}

/// The name-up mutation predicate.
#[test]
fn mutation_context_uses_the_parent_index() {
    let (_dir, stack) = build(&[("m.py", "items.append(1)\nitems[0] = 2\nitems += [3]\n")]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let names: Vec<u32> = module
        .nodes(&[Kind::Name], None, false)
        .into_iter()
        .filter(|n| matches!(module.nodes[*n as usize], Cn::Expr(Expr::Name(x)) if x.id.as_str() == "items"))
        .collect();

    assert!(!names.is_empty());
    assert!(
        names
            .iter()
            .all(|n| sightline_py_provers::scope::is_mutation_context(module, *n))
    );
}

#[test]
fn mutation_context_climbs_attribute_chains_but_not_call_results() {
    let (_dir, stack) = build(&[(
        "m.py",
        "cfg.x = 1\ncfg.items.append(1)\ncfg.items[0] = 1\ndel cfg.x\n\
         y = cfg.x\ncfg.copy().items.append(1)\ncfg.get()\n",
    )]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let mut names: Vec<u32> = module
        .nodes(&[Kind::Name], None, false)
        .into_iter()
        .filter(|n| matches!(module.nodes[*n as usize], Cn::Expr(Expr::Name(x)) if x.id.as_str() == "cfg"))
        .collect();
    names.sort_by_key(|n| module.line_of(*n));
    let verdicts: Vec<bool> = names
        .iter()
        .map(|n| sightline_py_provers::scope::is_mutation_context(module, *n))
        .collect();

    assert_eq!(verdicts, [true, true, true, true, false, false, false]);
}
