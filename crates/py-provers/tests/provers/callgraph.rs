//! Port of REF `tests/provers/test_callgraph.py`, the oracle-free half: facts'
//! CHA re-judged by callee edges wherever CHA had no typed evidence. A by-name
//! guess the edges do not confirm goes unknown; overrides keep taint;
//! `callee_of` is the one answer to "what body runs" (class -> `__init__`,
//! instance -> `__call__`), and the three tests that ask a real `Oracle` for
//! the edges.

use camino::Utf8Path;
use indexmap::IndexMap;
use sightline_core::config::Config;
use sightline_core::findings::Qname;
use sightline_py_facts::model::{CallSite, RepoFacts, Resolution};
use sightline_py_provers::Provers;
use sightline_py_provers::callgraph::{
    CallEdge, CallGraph, build_call_graph, callee_of, judged_call_graph,
};
use sightline_py_provers::effects::{Effects, summaries};
use sightline_py_provers::oracle::Oracle;
use sightline_testkit::PyStack;
use sightline_testkit::{build, build_with};
use tempfile::TempDir;

/// `a.run()` is a plain-variable receiver: CHA by name finds A.run and B.run
/// (AMBIGUOUS, taints `use`); the receiver's declared type picks A.run.
const SRC: &str = concat!(
    "class A:\n",
    "    def run(self):\n", // line 2
    "        return 1\n",
    "class B:\n",
    "    def run(self):\n", // line 5
    "        print('x')\n",
    "def use(a: A):\n",
    "    return a.run()\n", // line 8, the call spans byte cols 11..18
);
/// lines 9 to 11
const OVERRIDE: &str = "class A2(A):\n    def run(self):\n        print('y')\n";

/// Row H: `f.write(s)` on a file object, in a repo whose only `write` is a
/// method of an unrelated class. CHA by name has nothing but the name.
const FALSE_EDGE: &str = concat!(
    "SEEN = []\n",
    "class Journal:\n",
    "    def write(self, s):\n", // line 3
    "        SEEN.append(s)\n",
    "def save(p, s):\n",
    "    f = open(p, 'w')\n",
    "    f.write(s)\n", // line 7
);

fn edge(target_line: u32) -> CallEdge {
    CallEdge {
        rel: "m.py".into(),
        line: 8,
        col: 11,
        end_line: 8,
        end_col: 18,
        targets: vec![("m.py".into(), target_line)],
        external: Vec::new(),
    }
}

fn site(graph: &CallGraph, line: u32) -> &CallSite {
    graph
        .sites
        .iter()
        .find(|s| s.lineno == line)
        .unwrap_or_else(|| panic!("a call site on line {line}"))
}

/// The fold over a hand-built graph: the `calls` cell is seeded with it.
fn effects_of(facts: &RepoFacts<'_>, graph: &CallGraph) -> IndexMap<Qname, Effects> {
    let provers = Provers::bare(facts);
    let _ = provers.calls.set(graph.clone());
    summaries(facts, &provers)
}

#[test]
fn oracle_edge_resolves_past_cha_and_untaints_effects() {
    let (_dir, stack) = build(&[("m.py", SRC)]);
    let facts = stack.facts();
    let bare = build_call_graph(facts);
    assert_eq!(site(&bare, 8).resolution, Resolution::Ambiguous);
    assert!(effects_of(facts, &bare)["m.use"].unknown);

    let graph = judged_call_graph(facts, Some(&[edge(2)]));
    let judged = site(&graph, 8);
    assert_eq!(judged.resolution, Resolution::Resolved);
    assert_eq!(judged.target.as_deref(), Some("m.A.run"));
    assert_eq!(graph.upgraded, 1);
    assert_eq!(
        graph
            .callers("m.A.run")
            .map(|c| c.enclosing.to_string())
            .collect::<Vec<_>>(),
        ["m.use"]
    );
    assert!(!effects_of(facts, &graph)["m.use"].unknown);
    // facts untouched
    let module = facts.modules.get("m").expect("the fixture module");
    let at = facts.call_index[&(module.id, judged.node)];
    assert_eq!(
        facts.call_sites[at as usize].resolution,
        Resolution::Ambiguous
    );
}

#[test]
fn subclass_override_keeps_the_site_ambiguous() {
    let src = format!("{SRC}{OVERRIDE}");
    let (_dir, stack) = build(&[("m.py", &src)]);
    let facts = stack.facts();
    let graph = judged_call_graph(facts, Some(&[edge(2)]));
    let judged = site(&graph, 8);

    assert_eq!(judged.resolution, Resolution::Ambiguous);
    assert_eq!(
        judged
            .candidates
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>(),
        ["m.A.run", "m.A2.run"]
    );
    assert!(effects_of(facts, &graph)["m.use"].unknown);
}

/// `self.run()` resolved over A's own hierarchy is typed evidence: an edge
/// naming the unrelated `B.run` does not move it.
#[test]
fn a_resolved_site_is_never_rejudged() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "class A:\n",
            "    def run(self):\n        return 1\n",
            "    def use(self):\n",
            "        return self.run()\n", // line 5, cols 15..25
            "class B:\n",
            "    def run(self):\n        print('x')\n", // line 7
        ),
    )]);
    let facts = stack.facts();
    let graph = judged_call_graph(
        facts,
        Some(&[CallEdge {
            rel: "m.py".into(),
            line: 5,
            col: 15,
            end_line: 5,
            end_col: 25,
            targets: vec![("m.py".into(), 7)],
            external: Vec::new(),
        }]),
    );
    let judged = site(&graph, 5);

    assert_eq!(judged.resolution, Resolution::Resolved);
    assert_eq!(judged.target.as_deref(), Some("m.A.run"));
    assert_eq!(graph.upgraded, 0);
    assert!(effects_of(facts, &graph)["m.A.use"].clean());
}

#[test]
fn target_outside_facts_symbols_leaves_cha_verdict() {
    let (_dir, stack) = build(&[("m.py", SRC)]);
    let facts = stack.facts();
    // line 3: no def
    let graph = judged_call_graph(facts, Some(&[edge(3)]));

    assert_eq!(site(&graph, 8).resolution, Resolution::Ambiguous);
    assert_eq!(graph.upgraded, 0);
}

#[test]
fn by_name_guess_stands_only_without_an_oracle() {
    let (_dir, stack) = build(&[("m.py", FALSE_EDGE)]);
    let facts = stack.facts();
    let module = facts.modules.get("m").expect("the fixture module");
    let off = build_call_graph(facts);
    let at = facts.call_index[&(module.id, site(&off, 7).node)];
    assert_eq!(facts.call_sites[at as usize].resolution, Resolution::ByName);

    // oracle = false: the guess stands
    let guessed = site(&off, 7);
    assert_eq!(guessed.resolution, Resolution::Resolved);
    assert_eq!(guessed.target.as_deref(), Some("m.Journal.write"));
    assert!(
        effects_of(facts, &off)["m.save"]
            .atoms
            .contains("gw:m.SEEN")
    );

    // no edge confirms it
    let on = judged_call_graph(facts, Some(&[]));
    let unconfirmed = site(&on, 7);
    assert_eq!(unconfirmed.resolution, Resolution::Unresolved);
    assert!(unconfirmed.target.is_none());
    let eff = &effects_of(facts, &on)["m.save"];
    assert!(!eff.atoms.contains("gw:m.SEEN") && eff.unknown);
}

#[test]
fn an_external_edge_is_a_verdict_no_repo_body_runs() {
    let (_dir, stack) = build(&[("m.py", FALSE_EDGE)]);
    let facts = stack.facts();
    let graph = judged_call_graph(
        facts,
        Some(&[CallEdge {
            rel: "m.py".into(),
            line: 7,
            col: 4,
            end_line: 7,
            end_col: 14,
            targets: Vec::new(),
            external: vec!["_io._TextIOBase.write".into()],
        }]),
    );
    let judged = site(&graph, 7);

    assert_eq!(judged.resolution, Resolution::External);
    assert!(judged.target.is_none());
    assert_eq!(
        judged
            .candidates
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>(),
        ["_io._TextIOBase.write"]
    );
    assert_eq!(graph.upgraded, 1);
    let eff = &effects_of(facts, &graph)["m.save"];
    assert!(!eff.unknown && !eff.atoms.contains("gw:m.SEEN") && eff.atoms.contains("io"));
}

/// `conn.execute` on a `sqlite3.Connection` param: CHA by name has the repo's
/// `Store.execute`; the edge's home names a DB write, io.
#[test]
fn an_external_home_reaches_the_io_catalog() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "class Store:\n",
            "    def execute(self, q):\n        return q\n",
            "def insert(conn, q):\n",
            "    return conn.execute(q)\n", // line 5, cols 11..26
            "def pure(xs, q):\n",
            "    return xs.count(q)\n", // line 7, cols 11..22
        ),
    )]);
    let facts = stack.facts();
    let edges = [
        CallEdge {
            rel: "m.py".into(),
            line: 5,
            col: 11,
            end_line: 5,
            end_col: 26,
            targets: Vec::new(),
            external: vec!["sqlite3.Connection.execute".into()],
        },
        CallEdge {
            rel: "m.py".into(),
            line: 7,
            col: 11,
            end_line: 7,
            end_col: 22,
            targets: Vec::new(),
            external: vec!["builtins.list.count".into()],
        },
    ];
    let eff = effects_of(facts, &judged_call_graph(facts, Some(&edges)));

    assert_eq!(
        eff["m.insert"].atoms.iter().cloned().collect::<Vec<_>>(),
        ["io"]
    );
    assert!(eff["m.pure"].clean());
}

#[test]
fn callee_of_follows_class_and_instance_calls() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "SEEN = []\n",
            "class Base:\n",
            "    def __init__(self):\n",
            "        SEEN.append(1)\n",
            "class Kid(Base):\n",
            "    pass\n",
            "class Plain:\n",
            "    pass\n",
            "class H:\n",
            "    def __call__(self, x):\n",
            "        SEEN.append(x)\n",
            "handler = H()\n",
            "def make():\n",
            "    return Kid()\n", // line 14
            "def dispatch(x):\n",
            "    handler(x)\n", // line 16
            "def plain():\n",
            "    return Plain()\n", // line 18
        ),
    )]);
    let facts = stack.facts();
    let graph = build_call_graph(facts);

    assert_eq!(
        callee_of(facts, site(&graph, 14)).as_deref(),
        Some("m.Base.__init__")
    );
    assert_eq!(
        callee_of(facts, site(&graph, 16)).as_deref(),
        Some("m.H.__call__")
    );
    // no __init__ anywhere
    assert_eq!(callee_of(facts, site(&graph, 18)), None);
    let eff = effects_of(facts, &graph);
    assert!(eff["m.make"].atoms.contains("gw:m.SEEN"));
    assert!(eff["m.dispatch"].atoms.contains("gw:m.SEEN"));
    // a class that runs no repo body stays clean
    assert!(eff["m.plain"].clean());
}

#[test]
fn a_value_the_graph_cannot_follow_is_unknown() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import json\n",
            "loader = json.loads\n",
            "def read(s):\n",
            "    return loader(s)\n", // line 4
        ),
    )]);
    let facts = stack.facts();
    let graph = build_call_graph(facts);

    assert_eq!(callee_of(facts, site(&graph, 4)), None);
    assert!(effects_of(facts, &graph)["m.read"].unknown);
}

#[test]
fn degraded_name_cha_is_named_in_the_provenance() {
    let (_dir, stack) = build(&[("m.py", FALSE_EDGE)]);
    let facts = stack.facts();

    // built once
    assert!(std::ptr::eq(
        stack.provers.calls(facts),
        stack.provers.calls(facts)
    ));
    assert!(
        stack
            .provers
            .notes()
            .iter()
            .any(|n| n.contains("resolves by method name alone"))
    );
}

/// The oracle half: a mini repo whose provers carry a real checker.
fn with_oracle(files: &[(&str, &str)], config: Config) -> (TempDir, PyStack) {
    let (dir, mut stack) = build_with(files, config);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let roots = stack.facts().import_roots.clone();
    stack.provers.oracle =
        Some(Oracle::new(root, &[], &roots, None).expect("the checker builds on a mini repo"));
    (dir, stack)
}

#[test]
fn real_oracle_edges_feed_effects_and_hotness() {
    let (_dir, stack) = with_oracle(
        &[("m.py", SRC)],
        Config {
            hot_roots: vec!["m.use".to_string()],
            ..Config::new()
        },
    );
    let (facts, provers) = (stack.facts(), &stack.provers);
    let judged = site(provers.calls(facts), 8);
    assert_eq!(judged.resolution, Resolution::Resolved);
    assert_eq!(judged.target.as_deref(), Some("m.A.run"));
    assert!(!provers.effects(facts)["m.use"].unknown);
    // the hot set crosses the oracle-resolved edge and no other
    assert!(provers.hot(facts).amplification.contains_key("m.A.run"));
    assert!(!provers.hot(facts).amplification.contains_key("m.B.run"));
}

/// Row H: the typed callee of `f.write` is a file object's, not the repo's
/// `Journal.write`, so the checker places it outside the repo.
#[test]
fn real_oracle_refuses_the_by_name_write_edge() {
    let (_dir, stack) = with_oracle(&[("m.py", FALSE_EDGE)], Config::new());
    let (facts, provers) = (stack.facts(), &stack.provers);
    let judged = site(provers.calls(facts), 7);
    assert_eq!(judged.resolution, Resolution::External);
    assert_eq!(judged.target, None);
    assert!(
        !provers
            .calls(facts)
            .calls_to
            .contains_key("m.Journal.write")
    );
    assert!(!provers.effects(facts)["m.save"].unknown);
}

/// The receiver `make()` produced is `A` by the helper's inferred return,
/// whether bound to a name first or called on directly.
#[test]
fn real_oracle_types_a_helper_returned_receiver() {
    let (_dir, stack) = with_oracle(
        &[
            (
                "model.py",
                "class A:\n    def run(self):\n        return 1\n",
            ),
            (
                "other.py",
                "class B:\n    def run(self):\n        print('x')\n",
            ),
            (
                "app.py",
                concat!(
                    "from model import A\n",
                    "def make():\n",
                    "    return A()\n",
                    "def use():\n",
                    "    a = make()\n",
                    "    return a.run()\n", // line 6
                    "def chain():\n",
                    "    return make().run()\n", // line 8
                ),
            ),
        ],
        Config::new(),
    );
    let (facts, provers) = (stack.facts(), &stack.provers);
    let graph = provers.calls(facts);
    for line in [6u32, 8] {
        let judged = graph
            .sites
            .iter()
            .find(|s| &*s.module == "app" && s.lineno == line)
            .unwrap_or_else(|| panic!("a call site on app line {line}"));
        assert_eq!(judged.resolution, Resolution::Resolved);
        assert_eq!(judged.target.as_deref(), Some("model.A.run"));
    }
    assert!(provers.effects(facts)["app.use"].clean());
    // B.run's print never taints
    assert!(provers.effects(facts)["app.chain"].clean());
}
