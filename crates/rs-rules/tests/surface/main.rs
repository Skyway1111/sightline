//! Family B over Rust (port of `tests/rs/test_rules_surface.py`): #11 clone
//! groups, #20 repeated closures, #21 panic arms, #23 complexity, #37's two
//! arms and #48's fold candidates. Each rule gets its firing shape, the
//! silent sibling one step away, and the exemption the reading names.

mod clones;
mod closures;
mod complexity;
mod fold;
mod generality;
mod invariant;

use camino::Utf8Path;
use sightline_core::config::Config;
use sightline_core::findings::Finding;
use sightline_core::walk;
use sightline_testkit::{MANIFEST, RsStack, make_repo, rs_answers, run_rs_rule_on};

/// The body every #11 fixture repeats: six statements, past the node floor.
pub const BODY: &str = "    let a = load(1);\n    let b = load(2);\n    let c = a + b;\n    \
                        let d = c * 2;\n    let e = d - 1;\n    report(e);\n";

/// `_crate`: one manifest and one `src/lib.rs`.
pub fn krate(source: &str) -> Vec<(&str, &str)> {
    vec![("src/lib.rs", source)]
}

/// `run_rs_rule(..., edges=...)`: the rows the oracle's graph would have
/// answered. Without them a rule that reads edges sees the degraded run's
/// empty graph.
pub fn run_with_edges(
    id: &str,
    files: &[(&str, &str)],
    edges: &[(&str, &str, &str, u32, bool)],
) -> Vec<Finding> {
    let mut all: Vec<(&str, &str)> = vec![("Cargo.toml", MANIFEST)];
    all.extend_from_slice(files);
    let dir = make_repo(&all);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let config = Config::new();
    let listing = walk::discover(root, &config);
    let built = sightline_rs_facts::build::build_facts(root, &config, &listing, None);
    let stack = RsStack::new(built, rs_answers(edges, &[]), Default::default());
    run_rs_rule_on(id, &stack)
}

/// The causes of a run's findings, the assertion most tests make.
pub fn causes(found: &[Finding]) -> Vec<&str> {
    found.iter().map(|f| f.cause.as_str()).collect()
}
