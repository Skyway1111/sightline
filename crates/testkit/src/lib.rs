//! Dev-only fixtures every crate's tests build on: the mini repo a test
//! writes to disk, `core::testing`'s synthetic languages and registry,
//! and `run_rule`.

pub use sightline_core::testing::*;
pub use sightline_py_rules::stack::{PyLanguage, PyStack};
pub use sightline_rs_rules::stack::{RsLanguage, RsStack};

pub mod rs_fixtures;

use std::fs;

use camino::Utf8Path;
use sightline_core::config::Config;
use sightline_core::findings::{Finding, Sink};

use sightline_core::walk;
use sightline_py_provers::Provers;
use sightline_py_rules::RULES;
use sightline_rs_provers::oracle::index::{RsEdge, RsGraph};
use sightline_rs_provers::oracle::{RsAnswers, RsMember, RsOracle, answers_of};
use tempfile::TempDir;

/// The manifest a Rust fixture gets where it writes none, so a facts test
/// is its `.rs` sources alone.
pub const MANIFEST: &str = "[package]\nname = \"demo-crate\"\nversion = \"0.1.0\"\n";

/// One row of `ESCAPE_FIXTURES`: a closed-world escape reason, the repo
/// that produces it, the symbol it lands on, and whether the effects
/// summary is unknown (`framework-base` opens the caller set, not the
/// body). The escape tests in `py-provers` and the oracle-trust tests in
/// `py-rules` both read this table.
pub struct EscapeFixture {
    pub reason: &'static str,
    pub files: &'static [(&'static str, &'static str)],
    pub symbol: &'static str,
    pub unknown: bool,
}

pub const ESCAPE_FIXTURES: [EscapeFixture; 7] = [
    EscapeFixture {
        reason: "dynamic-access",
        files: &[(
            "m.py",
            concat!(
                "def _target(x: int) -> int:\n    return x\n",
                "def _use(n: str) -> int:\n    return globals()[n](1) + _target(2)\n",
            ),
        )],
        symbol: "m._target",
        unknown: true,
    },
    EscapeFixture {
        reason: "framework-base",
        files: &[(
            "m.py",
            concat!(
                "import json\n",
                "class Enc(json.JSONEncoder):\n",
                "    def default(self, o: object) -> object:\n        return o\n",
                "def _use(e: Enc) -> object:\n    return e.default(1)\n",
            ),
        )],
        symbol: "m.Enc.default",
        unknown: false,
    },
    EscapeFixture {
        reason: "kwargs-forward",
        files: &[(
            "m.py",
            concat!(
                "def _sink(a: int, **kw: int) -> int:\n    return a\n",
                "def open_fn(**kw: int) -> int:\n    return _sink(1, **kw)\n",
            ),
        )],
        symbol: "m._sink",
        unknown: true,
    },
    EscapeFixture {
        reason: "method-override",
        files: &[(
            "m.py",
            concat!(
                "class Base:\n    def _hook(self, x: int) -> int:\n        return x\n",
                "class Child(Base):\n    def _hook(self, x: int) -> int:\n",
                "        print(x)\n        return x\n",
                "def _use(b: Base) -> int:\n    return b._hook(1)\n",
            ),
        )],
        symbol: "m.Base._hook",
        unknown: true,
    },
    EscapeFixture {
        reason: "re-export",
        files: &[
            ("pkg/__init__.py", "from pkg.impl import helper\n"),
            (
                "pkg/impl.py",
                concat!(
                    "def helper(x: int) -> int:\n    return x\n",
                    "def _use() -> int:\n    return helper(1)\n",
                ),
            ),
        ],
        symbol: "pkg.impl.helper",
        unknown: true,
    },
    EscapeFixture {
        reason: "reference-escape",
        files: &[(
            "m.py",
            concat!(
                "def _cb(x: int) -> int:\n    return x\n",
                "def use(reg) -> None:\n    reg.register(_cb)\n",
                "def _call() -> int:\n    return _cb(3)\n",
            ),
        )],
        symbol: "m._cb",
        unknown: true,
    },
    EscapeFixture {
        reason: "unknown-decorator",
        files: &[(
            "m.py",
            concat!(
                "from webfx import route\n",
                "@route('/x')\n",
                "def _handler(req: int) -> int:\n    return req\n",
                "def _use() -> int:\n    return _handler(1)\n",
            ),
        )],
        symbol: "m._handler",
        unknown: true,
    },
];

/// A mini repo written to disk and built, with
/// bare provers (`Provers()`: no git read, no notes). The tree lives as long
/// as the returned handle, which every borrow of the stack outlives.
pub fn build(files: &[(&str, &str)]) -> (TempDir, PyStack) {
    build_with(files, Config::new())
}

pub fn build_with(files: &[(&str, &str)], config: Config) -> (TempDir, PyStack) {
    let dir = make_repo(files);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let listing = walk::discover(root, &config);
    let built = sightline_py_facts::build::build_facts(root, &config, &listing, None);
    let provers = Provers::bare(built.borrow_dependent());
    let stack = PyStack::new(built, provers, Default::default());
    (dir, stack)
}

/// One rule over an inline mini repo
/// with bare provers, findings in yield order with `lang` stamped.
pub fn run_rule(id: &str, files: &[(&str, &str)]) -> Vec<Finding> {
    let (_dir, stack) = build(files);
    run_rule_on(id, &stack)
}

/// `run_rule` over a stack a test built itself, so a test that needs the
/// oracle can set `stack.provers.oracle` first.
pub fn run_rule_on(id: &str, stack: &PyStack) -> Vec<Finding> {
    let rule = RULES
        .iter()
        .find(|r| r.record.id == id)
        .unwrap_or_else(|| panic!("no rule #{id} is registered"));
    let mut sink = Sink::new();
    (rule.run)(stack.facts(), &stack.provers, &mut sink);
    for f in &mut sink.0 {
        f.lang = rule.record.lang;
    }
    sink.0
}

/// `run_rule`'s Rust twin: one rs rule over an inline mini repo with no
/// toolchain, findings in yield order with `lang` stamped.
pub fn run_rs_rule(id: &str, files: &[(&str, &str)]) -> Vec<Finding> {
    let (_dir, stack) = build_rs(files);
    run_rs_rule_on(id, &stack)
}

/// `run_rs_rule` over a stack a test built itself, so a test that needs the
/// toolchain's answers can build with `rs_answers` or `build_rs_oracle`.
pub fn run_rs_rule_on(id: &str, stack: &RsStack) -> Vec<Finding> {
    let rule = sightline_rs_rules::RULES
        .iter()
        .find(|r| r.record.id == id)
        .unwrap_or_else(|| panic!("no rs rule #{id} is registered"));
    let mut sink = Sink::new();
    let provers = stack.provers();
    (rule.run)(provers.facts(), &provers, &mut sink);
    for f in &mut sink.0 {
        f.lang = rule.record.lang;
    }
    sink.0
}

/// A Rust mini repo written to disk and
/// built, with bare provers. The fixture's own `Cargo.toml` wins; without
/// one it gets `MANIFEST`, as `run_rs_rule` writes it.
pub fn build_rs(files: &[(&str, &str)]) -> (TempDir, RsStack) {
    build_rs_with(files, Config::new())
}

pub fn build_rs_with(files: &[(&str, &str)], config: Config) -> (TempDir, RsStack) {
    build_rs_stack(files, config, None, RsAnswers::default())
}

/// The full-control builder the other `build_rs*` fns wrap: the config, the
/// `only` subset single-file facts take, and the toolchain answers the stack
/// holds. `MANIFEST` fills in when the fixture writes no `Cargo.toml`.
pub fn build_rs_stack(
    files: &[(&str, &str)],
    config: Config,
    only: Option<&indexmap::IndexSet<sightline_core::findings::Rel>>,
    answers: RsAnswers,
) -> (TempDir, RsStack) {
    let mut all: Vec<(&str, &str)> = Vec::with_capacity(files.len() + 1);
    if !files.iter().any(|(rel, _)| *rel == "Cargo.toml") {
        all.push(("Cargo.toml", MANIFEST));
    }
    all.extend_from_slice(files);
    let dir = make_repo(&all);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let listing = walk::discover(root, &config);
    let built = sightline_rs_facts::build::build_facts(root, &config, &listing, only);
    let stack = RsStack::new(built, answers, Default::default());
    (dir, stack)
}

/// The mini repo built with the toolchain on, its cargo target under the
/// repo's own temp dir so no build lands in the user's cache. A test that
/// calls this spawns cargo and loads the index: `#[ignore]`, run by
/// `xtask check`.
pub fn build_rs_oracle(files: &[(&str, &str)]) -> (TempDir, RsStack) {
    let config = Config::new();
    let dir = make_repo(files);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let listing = walk::discover(root, &config);
    let built = sightline_rs_facts::build::build_facts(root, &config, &listing, None);
    let target = root.join("_target");
    let facts = built.borrow_dependent();
    let oracle = RsOracle::new(root, &config, &facts.crates, Some(&target));
    let rust = answers_of(oracle, facts);
    let stack = RsStack::new(built, rust, Default::default());
    (dir, stack)
}

/// The graph rows a prover reads in place of the degraded run's empty one,
/// and the members a splice is verifiable in. Only a member's name is
/// read; its dir and kind are the index's, so they stay empty here.
pub fn rs_answers(edges: &[(&str, &str, &str, u32, bool)], checked: &[&str]) -> RsAnswers {
    RsAnswers {
        graph: rs_graph(edges),
        checked: checked
            .iter()
            .map(|name| RsMember {
                name: (*name).to_string(),
                dir: String::new(),
                kind: String::new(),
            })
            .collect(),
        ..RsAnswers::default()
    }
}

/// The rows the oracle's graph would have answered, `(caller, callee, rel,
/// line, is-a-call)` each: what a prover or rule that reads edges gets in
/// place of the degraded run's empty graph.
pub fn rs_graph(edges: &[(&str, &str, &str, u32, bool)]) -> RsGraph {
    RsGraph::new(
        edges
            .iter()
            .map(|(caller, callee, rel, line, call)| RsEdge {
                caller: (*caller).to_string(),
                callee: (*callee).to_string(),
                rel: (*rel).to_string(),
                line: *line,
                call: *call,
                open: false,
            })
            .collect(),
        Default::default(),
    )
}

/// Write a mini repo: `(posix rel, source)`, parent directories created,
/// the bytes as given (LF stays LF, so a test that pins line endings gets
/// what it wrote). The tree lives as long as the returned handle.
pub fn make_repo(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("a temp dir for the mini repo");
    for (rel, src) in files {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("the mini repo's parent directories");
        }
        fs::write(&path, src.as_bytes()).expect("the mini repo's files");
    }
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    use sightline_core::lang::{FactsView, Language, Stack};

    #[test]
    fn make_repo_writes_nested_paths_and_keeps_the_bytes() {
        let dir = make_repo(&[("src/m.p", "x = 1\n"), ("notes.md", "one\r\ntwo")]);
        let root = dir.path();
        assert_eq!(fs::read(root.join("src/m.p")).unwrap(), b"x = 1\n".to_vec());
        assert_eq!(
            fs::read(root.join("notes.md")).unwrap(),
            b"one\r\ntwo".to_vec()
        );
    }

    #[test]
    fn the_synthetic_languages_reach_the_testkit() {
        let dir = make_repo(&[("P.toml", "")]);
        let root = camino::Utf8Path::from_path(dir.path()).unwrap();
        assert!(P.detect(root));
        assert!(!Q.detect(root));
        let stack = SyntheticStack::new(&P, &[("m.p", "x\n")]);
        assert_eq!(stack.neutral().languages(), ["p"]);
        assert_eq!(registry().by_id("11").unwrap().slug, "structural-clones");
    }
}
