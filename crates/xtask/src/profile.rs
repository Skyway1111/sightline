//! `cargo xtask profile`: the compare half of `scripts/profile_audit.py`.
//!
//! The binary records the per-pass walls itself (`audit --profile <json>`).
//! This reads that receipt, prints it biggest pass first, and diffs it
//! against `corpus/profile-<the table's py mid row>.json`, the committed
//! reference. A pass past twice its reference, or a total past 1.25x, fails
//! the run: a wall ruler alone cannot name which pass grew.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::corpus::{self, read_json as read};
use crate::paths::{flag, workspace_root};
use crate::worktree;

/// The committed reference for a tree, where the workspace holds one. Only
/// the mid Python repo has one, so every other tree profiles without a bar.
pub fn reference_path(tree: &str) -> PathBuf {
    workspace_root().join(format!("corpus/profile-{tree}.json"))
}

/// The mid Python repo, the tree this verb profiles when no argument names
/// one. Read off the corpus table, so a swap there is the only edit.
fn mid_py() -> Result<String> {
    let mid = corpus::targets(Some("py"), Some("mid"))?;
    let first = mid
        .first()
        .context("the corpus table holds no py mid row")?;
    Ok(first.name.clone())
}

fn passes(doc: &Value) -> Vec<(String, f64)> {
    let row = |r: &Value| {
        let label = r[0].as_str().unwrap_or_default().to_string();
        (label, r[1].as_f64().unwrap_or(0.0))
    };
    doc["passes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(row)
        .collect()
}

/// The footnote every profile receipt ends on.
const NESTED: &str =
    "  (an oracle pass or a memoized prover fold is nested in the rule that first asks for it)";

/// `profile_audit.py:table`, the receipt a reader quotes.
pub fn table(doc: &Value) -> String {
    let total = doc["total"].as_f64().unwrap_or(0.0).max(f64::MIN_POSITIVE);
    let rows = passes(doc);
    let oracle: f64 = rows
        .iter()
        .filter(|(label, _)| label.starts_with("oracle "))
        .map(|(_, secs)| secs)
        .sum();
    let (root, share) = (
        doc["root"].as_str().unwrap_or_default(),
        oracle / total * 100.0,
    );
    let head = format!(
        "{root} | modules {} | findings {} | total {total:.1}s | oracle {oracle:.1}s ({share:.0}%)",
        doc["modules"], doc["findings"]
    );
    let mut lines = vec![head, "   wall  share  pass".to_string()];
    for (label, secs) in &rows {
        lines.push(format!(
            "  {secs:5.2}s  {:3.0}%  {label}",
            secs / total * 100.0
        ));
    }
    lines.push(NESTED.to_string());
    lines.join("\n")
}

/// Every pass of `now` that outran its twin in `before`, and the total when
/// it outran 1.25x.
pub fn grown(before: &Value, now: &Value) -> Vec<String> {
    let held: std::collections::HashMap<String, f64> = passes(before).into_iter().collect();
    let grew = |(label, secs): (String, f64)| {
        let was = *held.get(&label)?;
        (secs > (2.0 * was).max(was + 0.1)).then(|| format!("  {label}: {was:.2}s -> {secs:.2}s"))
    };
    let mut out: Vec<String> = passes(now).into_iter().filter_map(grew).collect();
    let total = |d: &Value| d["total"].as_f64().unwrap_or(0.0);
    let (a, b) = (total(before), total(now));
    if b > 1.25 * a {
        out.push(format!("  total: {a:.2}s -> {b:.2}s"));
    }
    out
}

/// One `audit --profile` run over a tree, into `out`.
fn record(name: &str, out: &Path) -> Result<()> {
    let t = corpus::get(name)?;
    let dir = out.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let (here, held) = worktree::audited_tree(&t, Some(&dir.join(format!("{name}.toml"))))?;
    let root = here.root.display().to_string();
    let path = out.display().to_string();
    let done = corpus::sightline(&here, &["audit", &root, "--json", "--profile", &path])?;
    drop(held);
    if !done.status.success() {
        let why = corpus::tail(&String::from_utf8_lossy(&done.stderr));
        bail!("audit --profile failed on {name}:\n{why}");
    }
    Ok(())
}

pub fn main(args: &[&str]) -> Result<u8> {
    let named = |key: &str| flag(args, key).map(PathBuf::from);
    let mid = mid_py()?;
    let name = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .copied()
        .unwrap_or(&mid);
    let out = named("--json").unwrap_or_else(|| {
        std::env::temp_dir().join(format!("sightline-profile-{}.json", std::process::id()))
    });
    record(name, &out)?;
    let now = read(&out)?;
    println!("{}", table(&now));
    let reference = named("--reference").unwrap_or_else(|| reference_path(name));
    if !reference.is_file() {
        println!("no reference at {}", reference.display());
        return Ok(0);
    }
    let rows = grown(&read(&reference)?, &now);
    if rows.is_empty() {
        println!("profile: no pass past 2x its reference");
        return Ok(0);
    }
    println!(
        "profile: passes past 2x their reference (or total past 1.25x):\n{}",
        rows.join("\n")
    );
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(total: f64, rows: &[(&str, f64)]) -> Value {
        json!({
            "root": "r",
            "modules": 2,
            "findings": 3,
            "total": total,
            "passes": rows.iter().map(|(l, s)| json!([l, s])).collect::<Vec<_>>(),
        })
    }

    /// A pass grows only past twice its reference and a tenth of a second,
    /// so a 0.01 s pass reading 0.02 s is not a regression.
    #[test]
    fn a_pass_grows_past_twice_its_reference_and_a_tenth_of_a_second() {
        let before = doc(1.0, &[("facts", 0.5), ("rule #11 structural-clones", 0.01)]);
        assert!(grown(&before, &doc(1.0, &[("rule #11 structural-clones", 0.02)])).is_empty());
        assert!(grown(&before, &doc(1.0, &[("facts", 0.9)])).is_empty());
        assert_eq!(
            grown(&before, &doc(1.0, &[("facts", 1.2)])),
            ["  facts: 0.50s -> 1.20s"]
        );
        // a label the reference does not hold is not judged
        assert!(grown(&before, &doc(1.0, &[("provers", 9.0)])).is_empty());
        assert_eq!(grown(&before, &doc(1.3, &[])), ["  total: 1.00s -> 1.30s"]);
    }

    /// The receipt names the modules, the total and the oracle share, and
    /// one row per pass.
    #[test]
    fn the_table_reads_the_receipt_the_reference_holds() {
        let text = table(&doc(
            2.0,
            &[("facts", 1.0), ("oracle pass 1 (diagnostics+edges)", 0.5)],
        ));
        assert!(text.contains("modules 2"), "{text}");
        assert!(text.contains("total 2.0s | oracle 0.5s (25%)"), "{text}");
        assert!(text.contains("   1.00s   50%  facts"), "{text}");
    }

    /// The committed reference is a profile of the mid Python repo, with
    /// the labels the binary writes. A corpus swap leaves the row without
    /// one until the next quiet-machine pass records it, and the verb says
    /// so on every run: `no reference at <path>`.
    #[test]
    fn the_committed_reference_holds_the_labels_the_binary_writes() {
        let path = reference_path(&mid_py().unwrap());
        if !path.is_file() {
            eprintln!("skipped: no committed profile at {}", path.display());
            return;
        }
        let doc = read(&path).unwrap();
        let labels: Vec<String> = passes(&doc).into_iter().map(|(l, _)| l).collect();
        assert!(labels.contains(&"facts".to_string()));
        assert!(labels.contains(&"provers".to_string()));
        assert!(labels.contains(&"provers close".to_string()));
        assert!(labels.iter().any(|l| l.starts_with("rule #")));
        assert!(doc["total"].as_f64().unwrap() > 0.0);
    }
}
