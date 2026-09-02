//! `cargo xtask precision-sample`: seeded stratified precision sample.
//!
//! Pools findings from every given audit, stratifies by tier (or by cause
//! prefix under `--arms`), samples `min(20, n)` per stratum with the pinned
//! seed and prints a judging sheet. Verdicts are recorded by hand.
//!
//! The seed is pinned before judging; any post-judging rule or threshold
//! tuning needs a fresh seed and a fresh sample.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::mt::Random;

const SEED: u32 = 20_260_851;
const PER_TIER: usize = 20;
const USAGE: &str = "\
usage: cargo xtask precision-sample [--rules 35,36] [--arms cause,...]
       <audit.json> <repo-root> [...pairs]
";

/// `str(Path(arg))` as CPython renders it, since the sample's sort key is
/// that string: on Windows a separator is a backslash.
fn root_key(root: &Path) -> String {
    let text = root.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.replace('/', "\\")
    } else {
        text
    }
}

fn field<'a>(f: &'a Value, name: &str) -> &'a str {
    f[name].as_str().unwrap_or_default()
}

fn line_of(f: &Value) -> i64 {
    f["line"].as_i64().unwrap_or(0)
}

/// The file as the audit saw it: `corpus_run` audits a dirty tree as a
/// worktree at HEAD, so a live-tree excerpt can sit lines off the finding.
fn at_head(root: &Path, rel: &str) -> Vec<String> {
    let shown = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show", &format!("HEAD:{rel}")])
        .output();
    match shown {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect(),
        _ => std::fs::read_to_string(root.join(rel))
            .map(|t| t.lines().map(str::to_string).collect())
            .unwrap_or_default(),
    }
}

fn excerpt(root: &Path, f: &Value) {
    let lines = at_head(root, field(f, "file"));
    if lines.is_empty() {
        return;
    }
    let line = line_of(f);
    let start = (line - 3).max(0) as usize;
    let end = (line + 2).min(lines.len() as i64).max(0) as usize;
    for (n, text) in lines.iter().enumerate().take(end).skip(start) {
        let marker = if n as i64 + 1 == line { ">>" } else { "  " };
        let body: String = text.chars().take(150).collect();
        println!("   {marker} {:5} {body}", n + 1);
    }
}

/// The pool key: the finding's tier, or its cause prefix under `--arms`.
fn stratum(f: &Value, by_arm: bool) -> String {
    if by_arm {
        field(f, "cause")
            .split(':')
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        field(f, "tier").to_string()
    }
}

pub fn main(args: &[&str]) -> Result<u8> {
    let mut rest = args;
    let mut rules: Option<Vec<&str>> = None;
    let mut groups: Vec<String> = ["proved", "indexed", "heuristic"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let mut by_arm = false;
    if let (Some(&"--rules"), Some(list)) = (rest.first(), rest.get(1)) {
        rules = Some(list.split(',').collect());
        rest = &rest[2..];
    }
    if let (Some(&"--arms"), Some(list)) = (rest.first(), rest.get(1)) {
        groups = list.split(',').map(str::to_string).collect();
        by_arm = true;
        rest = &rest[2..];
    }
    if rest.len() < 2 || !rest.len().is_multiple_of(2) {
        print!("{USAGE}");
        return Ok(2);
    }

    // pool insertion order is the pair order, then each audit's finding order
    let mut pool: indexmap::IndexMap<String, Vec<(Value, PathBuf)>> = indexmap::IndexMap::new();
    for pair in rest.chunks(2) {
        let (audit_path, root) = (Path::new(pair[0]), PathBuf::from(pair[1]));
        let text = std::fs::read_to_string(audit_path)
            .with_context(|| format!("reading {}", audit_path.display()))?;
        let data: Value = serde_json::from_str(&text)?;
        for f in data["findings"].as_array().into_iter().flatten() {
            if rules
                .as_ref()
                .is_some_and(|ids| !ids.contains(&field(f, "rule")))
            {
                continue;
            }
            pool.entry(stratum(f, by_arm))
                .or_default()
                .push((f.clone(), root.clone()));
        }
    }

    let mut rng = Random::new(SEED);
    for (gi, group) in groups.iter().enumerate() {
        let empty = Vec::new();
        let rows = pool.get(group).unwrap_or(&empty);
        let mut sample: Vec<(Value, PathBuf)> = if rows.len() <= PER_TIER {
            rows.clone()
        } else {
            rng.sample(rows.len(), PER_TIER)
                .into_iter()
                .map(|i| rows[i].clone())
                .collect()
        };
        sample.sort_by(|a, b| {
            (root_key(&a.1), field(&a.0, "file"), line_of(&a.0)).cmp(&(
                root_key(&b.1),
                field(&b.0, "file"),
                line_of(&b.0),
            ))
        });
        println!(
            "\n{}\nPOOL {}: sampled {} of {}",
            "=".repeat(72),
            group.to_uppercase(),
            sample.len(),
            rows.len()
        );
        let label = "PIHABCDEFG".chars().nth(gi).unwrap_or('?');
        for (i, (f, root)) in sample.iter().enumerate() {
            let name = root.file_name().unwrap_or_default().to_string_lossy();
            println!(
                "\n[{label}{:02}] #{} {} | {name}/{}:{} | {}",
                i + 1,
                field(f, "rule"),
                field(f, "slug"),
                field(f, "file"),
                line_of(f),
                field(f, "symbol"),
            );
            println!("      {}", field(f, "message"));
            excerpt(root, f);
        }
    }
    Ok(0)
}
