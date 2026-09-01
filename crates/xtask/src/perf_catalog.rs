//! The authoring-time gate for the #41 perf
//! catalog. Every entry holds a slow/fast exemplar pair whose committed
//! micro-bench must prove slow/fast at 2x or better at the entry's pinned n.
//! The slow exemplar IS the matched shape (matcher pin:
//! this file's own test module). Run when the catalog changes.
//!
//! Pinned n deviates from 1000 only where argued in the exemplar's own
//! module. Refused candidates are recorded in `benchmarks.md` with their
//! measured ratios, not here. `--self-test` also runs a planted non-win pair;
//! it must be refused.
//!
//! The exemplars are `catalog/perf/*.py`, kept at the Python bytes they had;
//! `catalog/perf/_bench.py` times one pair and prints its two walls. It reads
//! the standard library only, so `--python` names any interpreter; without the
//! flag, the first of `python3` and `python` on PATH.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Result, bail};

use crate::paths::{path_python, workspace_root};

/// One micro-bench: the module its two sides live in, the two function names,
/// the setup expression evaluated with `n` bound, the pinned scale and the
/// repeat count.
pub struct Bench {
    pub name: &'static str,
    pub module: &'static str,
    pub slow: &'static str,
    pub fast: &'static str,
    pub setup: &'static str,
    pub n: u32,
    pub repeats: u32,
}

pub const ENTRIES: [Bench; 11] = [
    Bench {
        name: "list-attr-membership",
        module: "membership",
        slow: "membership_slow",
        fast: "membership_fast",
        setup: "([i % 500 for i in range(n)],)",
        n: 1000,
        repeats: 5,
    },
    Bench {
        name: "nested-same-collection",
        module: "nested",
        slow: "nested_slow",
        fast: "nested_fast",
        setup: "([i % 50 for i in range(n)],)",
        n: 1000,
        repeats: 3,
    },
    Bench {
        name: "deepcopy-in-loop",
        module: "deepcopy_loop",
        slow: "deepcopy_slow",
        fast: "deepcopy_fast",
        setup: "(list(range(n)), {\"k\": 1, \"sub\": {\"a\": [1, 2, 3]}})",
        n: 1000,
        repeats: 5,
    },
    // repeats=20: the one ratio near the bar (re caches patterns) under load
    Bench {
        name: "re-in-loop",
        module: "re_loop",
        slow: "re_slow",
        fast: "re_fast",
        setup: "([f\"abc{i}def{i % 7}\" for i in range(n)],)",
        n: 1000,
        repeats: 20,
    },
    Bench {
        name: "open-in-loop",
        module: "open_loop",
        slow: "open_slow",
        fast: "open_fast",
        setup: "_file_setup(n)",
        n: 1000,
        repeats: 3,
    },
    Bench {
        name: "str-concat-in-loop",
        module: "strconcat",
        slow: "strconcat_slow",
        fast: "strconcat_fast",
        setup: "_parts(n)",
        n: 1000,
        repeats: 20,
    },
    Bench {
        name: "subprocess-in-loop",
        module: "subprocess_loop",
        slow: "subprocess_slow",
        fast: "subprocess_fast",
        setup: "([str(i) for i in range(n)],)",
        n: 20,
        repeats: 2,
    },
    Bench {
        name: "http-in-loop",
        module: "http_loop",
        slow: "http_slow",
        fast: "http_fast",
        setup: "_http_setup(n)",
        n: 100,
        repeats: 3,
    },
    Bench {
        name: "materialized-short-circuit",
        module: "shortcircuit",
        slow: "shortcircuit_slow",
        fast: "shortcircuit_fast",
        setup: "([list(range(n))] * 10,)",
        n: 1000,
        repeats: 20,
    },
    Bench {
        name: "sorted-head",
        module: "sorted_head",
        slow: "sorted_head_slow",
        fast: "sorted_head_fast",
        setup: "([[(i * 7919) % n for i in range(n)]] * 10,)",
        n: 1000,
        repeats: 20,
    },
    Bench {
        name: "filter-scan",
        module: "filter_scan",
        slow: "filter_scan_slow",
        fast: "filter_scan_fast",
        setup: "({i: SimpleNamespace(f=i % 100) for i in range(n)}, [i % 100 for i in range(n)])",
        n: 1000,
        repeats: 5,
    },
];

const PLANTED: Bench = Bench {
    name: "planted-non-win",
    module: "planted",
    slow: "planted_slow",
    fast: "planted_fast",
    setup: "_parts(n)",
    n: 1000,
    repeats: 20,
};

pub fn data_dir() -> PathBuf {
    workspace_root().join("catalog").join("perf")
}

/// One `_bench.py` subprocess: `(slow secs, fast secs, results equal)`.
fn time_pair(python: &Path, dir: &Path, bench: &Bench) -> Result<(f64, f64, bool)> {
    let out = Command::new(python)
        .current_dir(dir)
        .arg("_bench.py")
        .args([bench.module, bench.slow, bench.fast, bench.setup])
        .args([bench.n.to_string(), bench.repeats.to_string()])
        .output()?;
    if !out.status.success() {
        bail!(
            "{}: bench failed: {}",
            bench.name,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let line = String::from_utf8_lossy(&out.stdout);
    let fields: Vec<&str> = line.split_whitespace().collect();
    let [slow, fast, equal] = fields[..] else {
        bail!("{}: bench printed {line:?}", bench.name);
    };
    Ok((slow.parse()?, fast.parse()?, equal == "True"))
}

pub fn main(args: &[&str]) -> Result<u8> {
    let python = match args.iter().position(|a| *a == "--python") {
        Some(i) => PathBuf::from(args.get(i + 1).copied().unwrap_or_default()),
        None => path_python()?,
    };
    let dir = data_dir();
    let mut pairs: Vec<&Bench> = ENTRIES.iter().collect();
    if args.contains(&"--self-test") {
        pairs.push(&PLANTED);
    }
    let mut failures = 0;
    for bench in &pairs {
        let planted = bench.name == PLANTED.name;
        let (slow, fast, equal) = time_pair(&python, &dir, bench)?;
        let ratio = slow / fast;
        let proven = ratio >= 2.0 && equal;
        failures += usize::from(planted || !proven);
        println!(
            "{:<28} {:<18} n={:<5} {ratio:7.1}x{}",
            bench.name,
            crate::text::label(planted, proven, "refused"),
            bench.n,
            if equal { "" } else { " RESULTS DIFFER" }
        );
    }
    println!(
        "perf_catalog_check: {}/{} proven",
        pairs.len() - failures,
        pairs.len()
    );
    Ok(u8::from(failures > 0))
}

#[cfg(test)]
mod tests {
    use sightline_py_rules::model::MatchCtx;
    use sightline_py_rules::perf::PERF_CATALOG;
    use sightline_py_rules::util::iter_functions;

    use super::*;
    use crate::catalog::source_of;

    /// The membership pair's shape lives in its classes, not its two
    /// one-line callers, so that entry is pinned on them.
    fn carriers(bench: &Bench) -> (&'static str, &'static str) {
        if bench.name == "list-attr-membership" {
            ("MembershipSlow", "MembershipFast")
        } else {
            (bench.slow, bench.fast)
        }
    }

    /// How many nodes the entry's matcher takes in one exemplar. Suppression
    /// markers are stripped: the pin must see the shape.
    fn matches(entry: &str, source: &str) -> usize {
        let bare: Vec<&str> = source
            .lines()
            .map(|l| l.split("  # sightline-ok").next().unwrap_or(l))
            .collect();
        let (_dir, stack) = sightline_testkit::build(&[("m.py", &bare.join("\n"))]);
        let shape = PERF_CATALOG
            .iter()
            .find(|(name, _)| *name == entry)
            .map(|(_, shape)| shape)
            .unwrap_or_else(|| panic!("no #41 catalog entry {entry}"));
        let facts = stack.facts();
        iter_functions(facts)
            .map(|(module, sym)| {
                let ctx = MatchCtx {
                    facts,
                    module,
                    sym,
                    amp: 0,
                };
                (shape.matcher)(ctx.func(), &ctx).len()
            })
            .sum()
    }

    #[test]
    fn every_catalog_entry_has_a_bench() {
        let mut benched: Vec<&str> = ENTRIES.iter().map(|b| b.name).collect();
        let mut catalogued: Vec<&str> = PERF_CATALOG.iter().map(|(name, _)| *name).collect();
        benched.sort_unstable();
        catalogued.sort_unstable();
        assert_eq!(benched, catalogued);
    }

    #[test]
    fn the_slow_exemplar_matches_and_the_fast_does_not() {
        let dir = data_dir();
        for bench in &ENTRIES {
            let (slow, fast) = carriers(bench);
            assert!(
                matches(bench.name, &source_of(&dir, bench.module, slow)) > 0,
                "{}: matcher misses",
                bench.name
            );
            assert_eq!(
                matches(bench.name, &source_of(&dir, bench.module, fast)),
                0,
                "{}: fast matched",
                bench.name
            );
        }
    }

    #[test]
    fn the_planted_pair_is_the_fast_shape_on_both_sides() {
        let dir = data_dir();
        assert_eq!(
            source_of(&dir, PLANTED.module, PLANTED.slow).replace(PLANTED.slow, "F"),
            source_of(&dir, PLANTED.module, PLANTED.fast).replace(PLANTED.fast, "F")
        );
    }
}
