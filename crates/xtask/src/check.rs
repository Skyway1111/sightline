//! `cargo xtask check`: the one gate.
//!
//! Stages run in order under one deadline for the lane, each printing its
//! wall. A stage that outruns what is left of the budget fails there instead
//! of waiting. Exit 0 pass, 1 fail.

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use crate::corpus::{self, Target, tail};
use crate::paths::workspace_root;
use crate::worktree;
use crate::{
    banned, bench_tables, fix_check, gate_bench, perf_catalog, profile, rules_doc, third_party,
};

const FAST_S: u64 = 180;
const SLOW_S: u64 = 900;

struct Lane {
    deadline: Instant,
}

impl Lane {
    fn left(&self) -> Result<Duration> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            bail!("budget exhausted before this stage");
        }
        Ok(left)
    }

    /// One command under what is left of the lane's budget. The output is
    /// captured and printed only when the stage fails.
    fn cargo(&self, args: &[&str]) -> Result<u8> {
        let left = self.left()?;
        let mut child = Command::new("cargo")
            .args(args)
            .current_dir(workspace_root())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        // both pipes drain on their own threads: a child that fills one
        // blocks on the write and the stage then fails on its budget alone
        // (66 KB of `cargo test` names hung the lane for its whole 180 s)
        let out = drain(child.stdout.take());
        let err = drain(child.stderr.take());
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                if status.success() {
                    return Ok(0);
                }
                let text = read(out) + &read(err);
                bail!("cargo {}\n{}", args.join(" "), tail(&text));
            }
            if started.elapsed() >= left {
                let _ = child.kill();
                bail!("cargo {} timed out at the budget", args.join(" "));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> JoinHandle<String> {
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_string(&mut text);
        }
        text
    })
}

fn read(handle: JoinHandle<String>) -> String {
    handle.join().unwrap_or_default()
}

/// Every artifact of a run lands here, never a path another lane could
/// write. Kept when a stage fails.
fn work() -> Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!("sightline-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// The workspace's own `gate --full`: the binary blocks on
/// nothing in the tree that built it.
fn own_gate(_: &Lane) -> Result<u8> {
    let root = workspace_root().to_string_lossy().into_owned();
    let t = Target::bare(&root, None);
    let done = corpus::command(&t, &["gate", &root, "--full"])?.output()?;
    if done.status.success() {
        return Ok(0);
    }
    bail!("{}", tail(&String::from_utf8_lossy(&done.stdout)));
}

/// The blocking axis blocks nothing on a clean pole: `baseline`, then
/// `gate --full` against it, on a detached worktree so the pinned tree
/// stays clean. A GATE rule is never baselined, so one firing on the pole
/// still fails here the day a judged rule earns that posture.
fn polarity(name: &str) -> Result<u8> {
    let t = corpus::get(name)?;
    let held = worktree::add(&t.root)?;
    let here = t.in_worktree(
        held.path.as_std_path(),
        Some(&work()?.join(format!("{name}.polarity.toml"))),
    )?;
    corpus::polarity(&here)
}

/// Determinism (two audits identical byte for byte, all cores then one
/// thread) and the fast gate's subset property, on one detached worktree so
/// lanes never share a baseline file.
fn determinism(name: &str) -> Result<u8> {
    let t = corpus::get(name)?;
    let work = work()?;
    let held = worktree::add(&t.root)?;
    // the lockfile and build dir a Cargo root keeps outside HEAD, a Python
    // root's interpreter: the worktree audits what the live tree audits
    let tree = held.path.as_std_path();
    let here = t.in_worktree(tree, Some(&work.join(format!("{name}.toml"))))?;
    let first = work.join(format!("{name}.json"));
    let second = work.join(format!("{name}.run2.json"));
    corpus::audit(&here, &first, None)?;
    corpus::audit(&here, &second, Some(1))?;
    if std::fs::read(&first)? != std::fs::read(&second)? {
        bail!("two audits of {name} (all cores vs 1 thread) differ; the order is ruled");
    }
    gate_bench::run(&here, &first, t.suffix())
}

type Stage = (&'static str, fn(&Lane) -> Result<u8>);

const FAST: &[Stage] = &[
    ("banned", |_| banned::main(&["--tree"])),
    // the attribution the archive ships, against the graph that built it:
    // one `cargo metadata` and the registry's license files, no build
    ("third-party", |_| third_party::main(&["--check"])),
    ("fmt", |l| l.cargo(&["fmt", "--check"])),
    ("clippy", |l| {
        l.cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
    }),
    ("test", |l| l.cargo(&["test", "--workspace"])),
    // every stage below drives `target/release/sightline`: a stale one is a
    // stale verdict
    ("build", |l| {
        l.cargo(&["build", "--release", "-p", "sightline-lint"])
    }),
    // the rule catalog a reader browses before installing, against the
    // registry the binary just built
    ("rules-doc", |_| rules_doc::main(&["--check"])),
    // the rs stack end to end on this tree, then the py oracle on the Python
    // pole: the fast lane's oracle coverage on both languages
    ("own-gate", own_gate),
    ("polarity", |_| polarity("powertools-lambda-python")),
];

const SLOW: &[Stage] = &[
    // every test that spawns the oracle, cargo or a corpus tree, the whole
    // workspace at once so no crate's `#[ignore]` runs in no
    // gate.
    // the Rust pole pays two full runs (baseline, then the gate), so the
    // fast lane leaves it here
    ("rs-polarity", |_| polarity("doxx")),
    ("test-ignored", |l| {
        l.cargo(&["test", "--workspace", "--", "--ignored"])
    }),
    // the determinism pair the fast lane cannot hold: each is two full
    // audits of a fresh worktree plus five gate runs
    ("determinism", |_| determinism("sqlglot")),
    ("rs-determinism", |_| determinism("turmoil")),
    // micro-benches proving 2x per #41 entry: a ratio, so never under load
    ("perf-catalog", |_| perf_catalog::main(&[])),
    ("corpus", |_| {
        let out = workspace_root().join("corpus/results");
        corpus::main(&[&out.to_string_lossy(), "--repeat-for-determinism"])
    }),
    // emitter honesty on each language's fast target. doxx is the Rust one:
    // turmoil holds no #32 finding, so its diff is empty and nothing is
    // compiled. The scale targets are a campaign-close ruler
    // (`xtask fix-check corpus/results merged-calculator`).
    ("fix-check", |_| {
        let out = workspace_root().join("corpus/results");
        fix_check::main(&[&out.to_string_lossy(), "powertools-lambda-python", "doxx"])
    }),
    ("profile", |_| profile::main(&[])),
    ("bench-tables", benchmarks),
];

/// `benchmarks.md`'s measured tables from this run's `corpus/results`, with
/// merged-calculator's per-pass profile landing there first. That tree has
/// no committed reference, so the profile run only records.
fn benchmarks(_: &Lane) -> Result<u8> {
    let results = workspace_root().join("corpus/results");
    let profiled = results.join("profile-merged-calculator.json");
    profile::main(&["merged-calculator", "--json", &profiled.to_string_lossy()])?;
    bench_tables::main(&[&results.to_string_lossy()])
}

pub fn main(args: &[&str]) -> Result<u8> {
    let slow = args.contains(&"--slow");
    let lane = Lane {
        deadline: Instant::now() + Duration::from_secs(if slow { SLOW_S } else { FAST_S }),
    };
    let started = Instant::now();
    let mut failed: Vec<&str> = Vec::new();
    for (name, stage) in FAST.iter().chain(if slow { SLOW } else { &[] }) {
        let t0 = Instant::now();
        let verdict = stage(&lane);
        let wall = t0.elapsed().as_secs_f64();
        match verdict {
            Ok(0) => println!("PASS {wall:6.1}s  {name}"),
            Ok(n) => {
                failed.push(name);
                println!("FAIL {wall:6.1}s  {name} (exit {n})");
            }
            Err(e) => {
                failed.push(name);
                println!("FAIL {wall:6.1}s  {name}");
                for line in format!("{e:#}").lines() {
                    println!("    {line}");
                }
            }
        }
        if !failed.is_empty() && !slow {
            break;
        }
    }
    let wall = started.elapsed().as_secs_f64();
    if failed.is_empty() {
        println!("check: PASS in {wall:.0}s");
        return Ok(0);
    }
    println!("check: FAIL ({}) in {wall:.0}s", failed.join(", "));
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stages_run_in_the_documented_order() {
        let names: Vec<&str> = FAST.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            [
                "banned",
                "third-party",
                "fmt",
                "clippy",
                "test",
                "build",
                "rules-doc",
                "own-gate",
                "polarity"
            ]
        );
        assert_eq!(SLOW.len(), 9);
        // every `#[ignore]` test of the workspace runs in one slow stage, so
        // each crate's are gated
        assert!(SLOW.iter().any(|(name, _)| *name == "test-ignored"));
    }

    /// The polarity and determinism stages name trees of the corpus table,
    /// and each pole is its language's clean one.
    #[test]
    fn the_corpus_stages_name_real_trees() {
        for name in ["powertools-lambda-python", "doxx"] {
            assert_eq!(corpus::get(name).unwrap().role, "clean");
        }
        assert_eq!(corpus::get("sqlglot").unwrap().suffix(), ".py");
        assert_eq!(corpus::get("turmoil").unwrap().suffix(), ".rs");
    }

    #[test]
    fn an_exhausted_budget_fails_the_stage() {
        let lane = Lane {
            deadline: Instant::now() - Duration::from_secs(1),
        };
        assert!(lane.left().is_err());
    }
}
