//! `cargo xtask gate-bench`.
//!
//! Latency: the fast gate end to end, a fresh process per run, over a
//! synthetic ten-file diff on the target tree, with a real baseline in place
//! so the suppress and baseline stages really run. Subset: every fast-gate
//! blocking finding appears in the same tree's full-audit JSON.
//! Changed-files-only: no blocking finding sits outside the diff.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::{Result, bail};
use serde_json::Value;

use crate::corpus::{self, Target};
use crate::worktree::utf8;

/// `(rule, file, line, col)`: how one finding is named on both sides.
type Key = (String, String, i64, i64);

const N_FILES: usize = 10;
const N_RUNS: usize = 5;

/// The fast gate's budget: 50 ms, the median of five runs.
const BUDGET_S: f64 = 0.050;

/// `statistics.median`: the middle wall, or the mean of the middle two.
pub fn median(mut walls: Vec<f64>) -> f64 {
    walls.sort_by(f64::total_cmp);
    let mid = walls.len() / 2;
    match walls.len().is_multiple_of(2) {
        true => (walls[mid - 1] + walls[mid]) / 2.0,
        false => walls[mid],
    }
}

fn verdict(ok: bool) -> &'static str {
    if ok { "PASS" } else { "FAIL" }
}

fn findings(doc: &Value) -> impl Iterator<Item = &Value> {
    doc["findings"].as_array().into_iter().flatten()
}

fn text(f: &Value, key: &str) -> String {
    f[key].as_str().unwrap_or_default().to_string()
}

/// `(rule, file, line, col)` of every finding a full audit reports.
fn full_keys(audit: &Value) -> Vec<Key> {
    findings(audit)
        .map(|f| {
            let at = |k| f[k].as_i64().unwrap_or(0);
            (text(f, "rule"), text(f, "file"), at("line"), at("col"))
        })
        .collect()
}

/// Ten of the language's files, spread across the tree: the audited files
/// that hold findings where there are any, so the gate really reports, else
/// the auditable files. A language whose rules are still being written must
/// still gate, and the subset half is what proves it reports nothing else.
fn synthetic_diff(root: &Path, config: Option<&Path>, audit: &Value, suffix: &str) -> Vec<String> {
    let mine = |rel: &String| rel.ends_with(suffix);
    let mut files: Vec<String> = findings(audit)
        .map(|f| text(f, "file"))
        .filter(mine)
        .collect();
    let mut source = "audited";
    if files.is_empty() {
        source = "auditable (no finding sits in a file of this suffix)";
        let here = utf8(root);
        let settings = sightline_core::config::load_config(&here, config.map(utf8).as_deref());
        let walked = sightline_core::walk::discover(&here, &settings);
        files = walked
            .into_iter()
            .map(|(_, rel)| rel)
            .filter(mine)
            .collect();
    }
    files.sort();
    files.dedup();
    let step = (files.len() / N_FILES).max(1);
    let picked: Vec<String> = files.into_iter().step_by(step).take(N_FILES).collect();
    println!(
        "synthetic diff ({} {suffix} files, {source}): {picked:?}",
        picked.len()
    );
    picked
}

/// One blocking line of the text gate render: `<rel>:<line>:<col>  #<rule>
/// ...`. The header and its notes are not findings.
fn blocking(out: &str) -> Vec<Key> {
    let mut rows = Vec::new();
    for line in out.lines() {
        if line.starts_with("sightline gate") || line.starts_with("  note:") {
            continue;
        }
        let Some((loc, rest)) = line.split_once("  #") else {
            continue;
        };
        // a Windows drive letter is not a column separator: take the last two
        let mut parts = loc.rsplitn(3, ':');
        let (Some(col), Some(ln), Some(rel)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let rule = rest.split(' ').next().unwrap_or_default().to_string();
        rows.push((
            rule,
            rel.to_string(),
            ln.parse().unwrap_or(0),
            col.parse().unwrap_or(0),
        ));
    }
    rows
}

/// The bench over one tree: the target to gate and its full-audit JSON.
pub fn run(t: &Target, audit_json: &Path, suffix: &str) -> Result<u8> {
    let root = t.root.as_path();
    let audit = corpus::read_json(audit_json)?;
    let keys = full_keys(&audit);
    let diff = synthetic_diff(root, t.config.as_deref(), &audit, suffix);
    if diff.is_empty() {
        println!("gate bench: FAIL (no {suffix} file in {})", root.display());
        return Ok(1);
    }
    let baseline = root.join(sightline_core::ratchet::BASELINE_NAME);
    if baseline.exists() {
        bail!("{} already has a baseline; refusing", root.display());
    }
    // a real baseline, from the audit itself, so the load and the diff really
    // run, with empty counts so blocking findings still surface for the
    // subset half
    let lines: Vec<String> = findings(&audit)
        .map(|f| format!("{}|{} 0\n", text(f, "rule"), text(f, "symbol")))
        .collect();
    std::fs::write(&baseline, lines.concat())?;
    let verdict = measure(t, &diff, &keys);
    std::fs::remove_file(&baseline)?;
    verdict
}

fn measure(t: &Target, diff: &[String], keys: &[Key]) -> Result<u8> {
    let root = t.root.to_string_lossy().into_owned();
    // the file list is greedy, so the whole of it precedes `--config`
    let mut args: Vec<&str> = vec!["gate", &root, "--files"];
    args.extend(diff.iter().map(String::as_str));
    let (mut walls, mut out) = (Vec::new(), String::new());
    for _ in 0..N_RUNS {
        let started = Instant::now();
        let done = corpus::command(t, &args)?.output()?;
        walls.push(started.elapsed().as_secs_f64());
        out = String::from_utf8_lossy(&done.stdout).into_owned();
    }
    let rounded: Vec<String> = walls.iter().map(|w| format!("{w:.3}")).collect();
    let mid = median(walls);
    println!("walls: {rounded:?} s, median {mid:.3} s");
    let latency_ok = mid <= BUDGET_S;
    let (got, bar) = (mid * 1000.0, BUDGET_S * 1000.0);
    println!(
        "latency: median {got:.0} ms vs {bar:.0} ms budget -> {}",
        verdict(latency_ok)
    );

    let found = blocking(&out);
    let missing: Vec<&Key> = found.iter().filter(|b| !keys.contains(b)).collect();
    println!(
        "subset: {} fast-gate findings, {} missing from full audit -> {}",
        found.len(),
        missing.len(),
        verdict(missing.is_empty())
    );
    for m in missing.iter().take(10) {
        println!("  not in full audit: {m:?}");
    }
    let outside: Vec<&Key> = found.iter().filter(|b| !diff.contains(&b.1)).collect();
    let n = outside.len();
    println!(
        "changed-files-only: {n} findings outside the diff -> {}",
        verdict(n == 0)
    );
    let ok = latency_ok && missing.is_empty() && outside.is_empty();
    println!("gate bench: {}", verdict(ok));
    Ok(u8::from(!ok))
}

/// The hook case, one file per edit: the four frozen files `benchmarks.md`
/// quotes, and the spawn floor `--version` pays.
const HOOK_CASES: &[(&str, &str)] = &[
    (
        "powertools-lambda-python",
        "aws_lambda_powertools/utilities/parameters/base.py",
    ),
    ("merged-calculator", "src/calculator/damage.py"),
    ("turmoil", "crates/turmoil-fs/src/lib.rs"),
    ("salvo", "crates/oapi/src/openapi/components.rs"),
];
const N_HOOK_RUNS: usize = 15;

/// Milliseconds of `N_HOOK_RUNS` warm runs, a fresh process each.
fn walls_ms(mut command: impl FnMut() -> Result<Command>) -> Result<Vec<f64>> {
    (0..N_HOOK_RUNS)
        .map(|_| {
            let started = Instant::now();
            command()?.output()?;
            Ok(started.elapsed().as_secs_f64() * 1000.0)
        })
        .collect()
}

fn report(label: &str, walls: Vec<f64>) {
    let min = walls.iter().copied().fold(f64::INFINITY, f64::min);
    let max = walls.iter().copied().fold(0.0, f64::max);
    println!(
        "{label}: median {:.1} ms (min {min:.1}, max {max:.1})",
        median(walls)
    );
}

fn hook() -> Result<u8> {
    for (name, rel) in HOOK_CASES {
        let t = corpus::get(name)?;
        let file = t.root.join(rel);
        if !file.is_file() {
            bail!("{} is not in the {name} clone", file.display());
        }
        let (root, file) = (t.root.to_string_lossy(), file.to_string_lossy());
        let walls = walls_ms(|| corpus::command(&t, &["gate", &root, "--files", &file]))?;
        report(&format!("{name} {rel}"), walls);
    }
    let bare = Target::bare(".", None);
    report(
        "spawn floor (--version)",
        walls_ms(|| corpus::command(&bare, &["--version"]))?,
    );
    Ok(0)
}

pub fn main(args: &[&str]) -> Result<u8> {
    if args == ["--hook"] {
        return hook();
    }
    let suffix = args
        .iter()
        .find_map(|a| a.strip_prefix("--suffix="))
        .unwrap_or(".py");
    let pos: Vec<&str> = args
        .iter()
        .copied()
        .filter(|a| !a.starts_with("--"))
        .collect();
    let [root, audit, rest @ ..] = pos.as_slice() else {
        bail!("usage: cargo xtask gate-bench <repo-root> <full-audit-json> [config] [--suffix=.rs]")
    };
    // an argv run gates a bare tree; `check` hands its worktree target, whose
    // environment names the live root's warm build directory
    let t = Target::bare(root, rest.first().copied());
    run(&t, Path::new(audit), suffix)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_median_of_five_is_the_middle_wall() {
        assert_eq!(median(vec![0.05, 0.01, 0.03, 0.02, 0.04]), 0.03);
        assert_eq!(median(vec![0.02, 0.04]), 0.03);
    }

    /// The gate's text render, read back into finding keys: the header and
    /// its notes are skipped, a Windows drive letter in the path is not a
    /// column separator.
    #[test]
    fn blocking_lines_parse_into_finding_keys() {
        let out = "sightline gate: 2 blocking\n  note: oracle off\n\
                   src/m.py:12:4  #32 dead-symbols dead-symbol:m.f\n\
                   C:/t/m.rs:3:1  #11 structural-clones clone:k\n";
        assert_eq!(
            blocking(out),
            [
                ("32".to_string(), "src/m.py".to_string(), 12, 4),
                ("11".to_string(), "C:/t/m.rs".to_string(), 3, 1),
            ]
        );
    }

    /// The diff is ten files spread across the audited ones.
    #[test]
    fn the_synthetic_diff_spreads_over_the_audited_files() {
        let rows: Vec<Value> = (0..40)
            .map(|i| json!({"file": format!("src/m{i:02}.py")}))
            .collect();
        let picked = synthetic_diff(Path::new("."), None, &json!({"findings": rows}), ".py");
        assert_eq!(picked.len(), N_FILES);
        assert_eq!(
            (picked[0].as_str(), picked[1].as_str()),
            ("src/m00.py", "src/m04.py")
        );
    }
}
