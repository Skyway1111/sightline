//! Roots from config plus #59 cost docstrings, propagated over the resolved
//! callee graph; amplification is the loop depth crossed. The last two are
//! degraded-run tests, which live here because they read the same accessor
//! and the same notes.

use camino::Utf8Path;
use tempfile::TempDir;

use sightline_core::config::Config;
use sightline_core::walk;
use sightline_py_facts::build::build_facts;
use sightline_py_provers::hotness::{HotSet, roots_of};
use sightline_py_provers::{NO_ORACLE_NOTE, Provers};
use sightline_testkit::PyStack;
use sightline_testkit::{build, build_with, make_repo};

/// `testkit::build_with` with an audit's `Provers` rather than a bare one, so
/// the notes a run would print are on the stack (`Provers::bare` skips the
/// config and git notes).
fn build_noting(files: &[(&str, &str)], config: Config) -> (TempDir, PyStack) {
    let dir = make_repo(files);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let listing = walk::discover(root, &config);
    let built = build_facts(root, &config, &listing, None);
    let provers = Provers::new(root, &config, built.borrow_dependent(), false);
    let stack = PyStack::new(built, provers, Default::default());
    (dir, stack)
}

fn hot_config(roots: &[&str]) -> Config {
    Config {
        hot_roots: roots.iter().map(|r| (*r).to_string()).collect(),
        ..Config::new()
    }
}

fn amplification(hot: &HotSet) -> Vec<(&str, u32)> {
    let mut rows: Vec<(&str, u32)> = hot.amplification.iter().map(|(q, d)| (&**q, *d)).collect();
    rows.sort();
    rows
}

fn roots(hot: &HotSet) -> Vec<&str> {
    hot.roots.iter().map(|q| &**q).collect()
}

const FILES: [(&str, &str); 1] = [(
    "m.py",
    concat!(
        "def root(rows):\n",
        "    for row in rows:\n",
        "        for cell in row:\n",
        "            inner(cell)\n",
        "    outer(rows)\n",
        "def inner(cell):\n    return leaf(cell)\n",
        "def outer(rows):\n    return len(rows)\n",
        "def leaf(cell):\n    return cell\n",
        "def cold(x):\n    return x\n",
    ),
)];

#[test]
fn config_roots_propagate_with_loop_amplification() {
    let (_dir, stack) = build_with(&FILES, hot_config(&["m.root"]));
    let facts = stack.facts();
    let hot = stack.provers.hot(facts);
    // built once, like every other accessor
    assert!(std::ptr::eq(stack.provers.hot(facts), hot));
    assert_eq!(
        amplification(hot),
        vec![
            ("m.inner", 2), // called across two loop levels
            ("m.leaf", 2),  // inherits inner's amplification, no extra loop
            ("m.outer", 0), // called outside any loop
            ("m.root", 0),  // the root itself is hot
        ]
    );
    assert!(!hot.amplification.contains_key("m.cold"));
    assert_eq!(roots(hot), vec!["m.root"]);
}

/// A once-flag loader and a cache-dict derivation each compute below an early
/// return on a module store they fill: what they call there runs once per key,
/// so it inherits no amplification. The same body reached without a guard, and
/// the call on the guard's own hit path, still do.
#[test]
fn a_memo_guard_is_a_barrier() {
    let (_dir, stack) = build_with(
        &[(
            "m.py",
            concat!(
                "_loaded = False\n_CACHE = {}\n",
                "def root(rows):\n",
                "    for row in rows:\n",
                "        ensure_loaded()\n        derive(row)\n        eager(row)\n",
                "def ensure_loaded():\n",
                "    global _loaded\n",
                "    if _loaded:\n        return\n",
                "    parse_table()\n    _loaded = True\n",
                "def derive(row):\n",
                "    hit = _CACHE.get(row)\n",
                "    if hit is not None:\n        return fit(hit)\n",
                "    built = build(row)\n    _CACHE[row] = built\n    return fit(built)\n",
                "def eager(row):\n    return build(row)\n",
                "def parse_table():\n    return 1\n",
                "def build(row):\n    return row\n",
                "def fit(x):\n    return x\n",
            ),
        )],
        hot_config(&["m.root"]),
    );
    let hot = stack.provers.hot(stack.facts());
    // parse_table sits below ensure_loaded's flag guard: not hot at all.
    // build keeps eager's amplification, never derive's below-the-guard call;
    // fit is called on the hit path, which runs per call.
    assert!(!hot.amplification.contains_key("m.parse_table"));
    assert_eq!(hot.amplification["m.build"], 1);
    assert_eq!(hot.amplification["m.fit"], 1);
}

#[test]
fn docstring_cost_declaration_seeds_hotness() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def fit(xs):\n",
            "    \"\"\"Hot path: called per frame by the render loop.\"\"\"\n",
            "    return helper(xs)\n",
            "def helper(xs):\n    return xs\n",
            "def plain(xs):\n    \"\"\"Sorts the rows.\"\"\"\n    return xs\n",
        ),
    )]);
    let hot = stack.provers.hot(stack.facts());
    assert_eq!(
        amplification(hot)
            .iter()
            .map(|(q, _)| *q)
            .collect::<Vec<&str>>(),
        vec!["m.fit", "m.helper"]
    );
    assert_eq!(roots(hot), vec!["m.fit"]);
}

#[test]
fn no_roots_is_silent_and_noted() {
    let (_dir, stack) = build_noting(&[("m.py", "def f(x):\n    return x\n")], Config::new());
    let hot = stack.provers.hot(stack.facts());
    assert!(hot.amplification.is_empty() && hot.roots.is_empty());
    assert!(
        stack
            .provers
            .notes()
            .iter()
            .any(|n| n.contains("family P silent"))
    );
}

#[test]
fn missing_configured_root_is_noted() {
    let (_dir, stack) = build_noting(
        &[("m.py", "def f(x):\n    return x\n")],
        hot_config(&["m.gone", "m.f"]),
    );
    let hot = stack.provers.hot(stack.facts());
    assert_eq!(roots(hot), vec!["m.f"]);
    assert!(
        stack
            .provers
            .notes()
            .iter()
            .any(|n| n.contains("hot-root not found: m.gone"))
    );
}

/// A recursive walk in a loop is linear work: the cycle's loop is crossed
/// once, and nothing downstream of the cycle inherits a cap.
#[test]
fn recursion_in_loop_counts_its_loop_once() {
    let (_dir, stack) = build_with(
        &[(
            "m.py",
            concat!(
                "def spin(xs):\n",
                "    for x in xs:\n        spin(x)\n        leaf(x)\n",
                "    return tail(xs)\n",
                "def leaf(x):\n    return x\n",
                "def tail(xs):\n    return xs\n",
                "def root(xs):\n    for x in xs:\n        spin(x)\n",
            ),
        )],
        hot_config(&["m.root"]),
    );
    // spin: root's loop plus its own recursion; tail: per invocation; leaf:
    // one more loop level inside spin
    assert_eq!(
        amplification(stack.provers.hot(stack.facts())),
        vec![("m.leaf", 3), ("m.root", 0), ("m.spin", 2), ("m.tail", 2)]
    );
}

/// A test's cost docstring narrates its subject; test glue stays cold.
#[test]
fn test_docstrings_do_not_seed_hotness() {
    let (_dir, stack) = build(&[(
        "tests/test_m.py",
        "def helper(xs):\n    \"\"\"Hot path: called per request.\"\"\"\n    return xs\n",
    )]);
    assert!(stack.provers.hot(stack.facts()).roots.is_empty());
}

/// Calling a class runs its `__init__`, own or the first one up the base
/// chain. A class with no `__init__` anywhere is no edge, and its other
/// methods stay cold.
#[test]
fn class_call_is_an_edge_to_the_chain_init() {
    let (_dir, stack) = build_with(
        &[(
            "m.py",
            concat!(
                "class Thing:\n    def __init__(self, x):\n        self.x = x\n",
                "class Base:\n    def __init__(self, x):\n        self.x = x\n",
                "class Sub(Base):\n    def go(self):\n        return self.x\n",
                "class Bare:\n    def go(self):\n        return 1\n",
                "def root(xs):\n",
                "    for x in xs:\n        Thing(x)\n        Sub(x)\n        Bare()\n",
            ),
        )],
        hot_config(&["m.root"]),
    );
    assert_eq!(
        amplification(stack.provers.hot(stack.facts())),
        vec![
            ("m.Base.__init__", 1),
            ("m.Thing.__init__", 1),
            ("m.root", 0)
        ]
    );
}

/// `for x in blocks(mods)`: the iterable runs once per entry of the for, so
/// only an outer loop amplifies it; a comprehension's first `iter` likewise,
/// while its later generators, conditions and element run per item. The loop
/// body and a `while` test run per iteration.
#[test]
fn a_for_iterable_and_a_comprehensions_first_iter_run_once() {
    let (_dir, stack) = build_with(
        &[(
            "m.py",
            concat!(
                "def root(mods):\n",
                "    for line in blocks(mods):\n        use(line)\n",
                "    for mod in mods:\n        for line in inner(mod):\n            pass\n",
                "    [elt(x) for x in first(mods) for y in second(x) if keep(y)]\n",
                "    while more(mods):\n        pass\n",
                "def blocks(mods):\n    return mods\n",
                "def use(line):\n    return line\n",
                "def inner(mod):\n    return mod\n",
                "def elt(x):\n    return x\n",
                "def first(mods):\n    return mods\n",
                "def second(x):\n    return x\n",
                "def keep(y):\n    return y\n",
                "def more(mods):\n    return False\n",
            ),
        )],
        hot_config(&["m.root"]),
    );
    assert_eq!(
        amplification(stack.provers.hot(stack.facts())),
        vec![
            ("m.blocks", 0),
            ("m.elt", 1),
            ("m.first", 0),
            ("m.inner", 1),
            ("m.keep", 1),
            ("m.more", 1),
            ("m.root", 0),
            ("m.second", 1),
            ("m.use", 1),
        ]
    );
}

#[test]
fn glob_roots_expand_in_config_order_then_sorted() {
    let (_dir, stack) = build_noting(&FILES, hot_config(&["m.r*", "m.*e*", "m.z*"]));
    let facts = stack.facts();
    let (seeds, missing) = roots_of(facts);
    assert_eq!(
        seeds.iter().map(|q| &**q).collect::<Vec<&str>>(),
        vec!["m.root", "m.inner", "m.leaf", "m.outer"]
    );
    assert_eq!(missing, vec!["m.z*"]);
    let hot = stack.provers.hot(facts);
    assert_eq!(roots(hot), vec!["m.inner", "m.leaf", "m.outer", "m.root"]);
    assert!(
        stack
            .provers
            .notes()
            .iter()
            .any(|n| n.contains("hot-root not found: m.z*"))
    );
}

/// Without an oracle the graph lets a by-name guess stand
/// (effects and #37 inherit it); hotness does not, so the hot set is a subset
/// of the oracle twin's. Its twin: the same method reached through `self`.
#[test]
fn degraded_hotness_propagates_over_typed_edges_only() {
    let (_dir, stack) = build_with(
        &[(
            "m.py",
            concat!(
                "class Other:\n    def run(self, x):\n        return x\n",
                "class Runner:\n    def go(self, items):\n",
                "        for it in items:\n            it.run(1)\n",
            ),
        )],
        hot_config(&["m.Runner.go"]),
    );
    assert_eq!(
        amplification(stack.provers.hot(stack.facts())),
        vec![("m.Runner.go", 0)]
    );
    let (_dir, stack) = build_with(
        &[(
            "m.py",
            concat!(
                "class Runner:\n    def run(self, x):\n        return x\n",
                "    def go(self, items):\n",
                "        for it in items:\n            self.run(1)\n",
            ),
        )],
        hot_config(&["m.Runner.go"]),
    );
    assert_eq!(
        amplification(stack.provers.hot(stack.facts())),
        vec![("m.Runner.go", 0), ("m.Runner.run", 1)]
    );
}

/// What an absent oracle costs is the header's to report.
#[test]
fn a_run_without_an_oracle_names_what_goes_silent() {
    let config = Config {
        oracle: false,
        ..Config::new()
    };
    let (_dir, stack) = build_noting(
        &[(
            "m.py",
            "import absent_xyz\n\ndef f(a):\n    return absent_xyz.g(a)\n",
        )],
        config,
    );
    assert!(stack.provers.no_oracle());
    assert!(
        stack
            .provers
            .notes()
            .iter()
            .any(|n| n.contains(NO_ORACLE_NOTE))
    );
}
