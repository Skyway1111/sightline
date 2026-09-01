//! `scripts/gauntlet_precision.py`: gauntlet precision sheets. One row per
//! finding, a judge's verdict per row, rates per rule and per arm.
//!
//! The key `file:line:rule:cause` is stable across engine versions on a
//! pinned tree, so a re-audit brings every unchanged verdict forward. The
//! `rule` column is the precision table's own key (`precision::key_of`): the
//! bare id for Python, `<lang>:<id>` for a reading in another language, so a
//! tally row drops straight into the table.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde_json::Value;
use sightline_core::findings::Tier;
use sightline_core::precision::{key_of as precision_key, key_parts};

const COLUMNS: [&str; 11] = [
    "key", "rule", "slug", "arm", "tier", "file", "line", "symbol", "message", "verdict", "why",
];
const VERDICTS: [&str; 2] = ["real", "fp"];

type Row = IndexMap<String, String>;

fn field<'a>(f: &'a Value, name: &str) -> &'a str {
    f[name].as_str().unwrap_or_default()
}

/// The sheet's `rule` column and the precision table's key are one string.
fn rule_key(f: &Value) -> String {
    let lang = f.get("lang").and_then(Value::as_str).unwrap_or("py");
    precision_key(field(f, "rule"), lang)
}

fn key_of(f: &Value) -> String {
    format!(
        "{}:{}:{}:{}",
        field(f, "file"),
        f["line"],
        field(f, "rule"),
        field(f, "cause")
    )
}

fn arm_of(f: &Value) -> &str {
    field(f, "cause").split(':').next().unwrap_or_default()
}

/// One TSV field as `csv.writer(delimiter="\t")` writes it: minimal quoting.
fn quote(value: &str) -> String {
    if value.contains('\t') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn read_sheet(path: &Path) -> Result<Vec<Row>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
    Ok(lines
        .filter(|l| !l.is_empty())
        .map(|line| {
            header
                .iter()
                .zip(split_row(line))
                .map(|(k, v)| ((*k).to_string(), v))
                .collect()
        })
        .collect())
}

/// The reader half of `quote`.
fn split_row(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut field, mut quoted, mut chars) = (String::new(), false, line.chars().peekable());
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            '\t' if !quoted => out.push(std::mem::take(&mut field)),
            other => field.push(other),
        }
    }
    out.push(field);
    out
}

fn write_sheet(path: &Path, rows: &[Row]) -> Result<()> {
    let mut out = COLUMNS.join("\t");
    out.push('\n');
    for row in rows {
        let cells: Vec<String> = COLUMNS
            .iter()
            .map(|c| quote(row.get(*c).map_or("", String::as_str)))
            .collect();
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    std::fs::write(path, out)?;
    Ok(())
}

/// A rule filter takes ids (every language's reading) or keys (`rs:11`: one
/// reading).
fn kept(f: &Value, rules: Option<&Vec<&str>>) -> bool {
    rules.is_none_or(|ids| ids.contains(&field(f, "rule")) || ids.contains(&rule_key(f).as_str()))
}

fn sheet(audit: &Path, out: &Path, carry: Option<&Path>, rules: Option<&Vec<&str>>) -> Result<()> {
    let data: Value = serde_json::from_str(&std::fs::read_to_string(audit)?)?;
    let findings: Vec<&Value> = data["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| kept(f, rules))
        .collect();

    let mut earlier: IndexMap<String, Row> = IndexMap::new();
    if let Some(path) = carry {
        for row in read_sheet(path)? {
            let rule = row.get("rule").cloned().unwrap_or_default();
            let id = key_parts(&rule).1.to_string();
            if rules.is_none_or(|ids| ids.contains(&rule.as_str()) || ids.contains(&id.as_str())) {
                earlier.insert(row.get("key").cloned().unwrap_or_default(), row);
            }
        }
    }
    let mut by_site: IndexMap<(String, String, String, String), Vec<String>> = IndexMap::new();
    for (key, row) in &earlier {
        let cell = |name: &str| row.get(name).cloned().unwrap_or_default();
        by_site
            .entry((cell("file"), cell("line"), cell("rule"), cell("arm")))
            .or_default()
            .push(key.clone());
    }

    let mut ordered = findings;
    ordered.sort_by_key(|f| {
        let key = rule_key(f);
        let (lang, id) = key_parts(&key);
        (
            lang.to_string(),
            id,
            field(f, "file").to_string(),
            f["line"].as_i64().unwrap_or(0),
        )
    });

    let mut rows: Vec<Row> = Vec::new();
    for f in ordered {
        let site = (
            field(f, "file").to_string(),
            f["line"].to_string(),
            rule_key(f),
            arm_of(f).to_string(),
        );
        let same: Vec<String> = by_site
            .get(&site)
            .into_iter()
            .flatten()
            .filter(|k| earlier.contains_key(*k))
            .cloned()
            .collect();
        let old = earlier.shift_remove(&key_of(f)).or_else(|| {
            (same.len() == 1)
                .then(|| earlier.shift_remove(&same[0]))
                .flatten()
        });
        let carried = |name: &str| {
            old.as_ref()
                .and_then(|r| r.get(name).cloned())
                .unwrap_or_default()
        };
        rows.push(
            [
                ("key", key_of(f)),
                ("rule", rule_key(f)),
                ("slug", field(f, "slug").to_string()),
                ("arm", arm_of(f).to_string()),
                ("tier", field(f, "tier").to_string()),
                ("file", field(f, "file").to_string()),
                ("line", f["line"].to_string()),
                ("symbol", field(f, "symbol").to_string()),
                (
                    "message",
                    field(f, "message")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                ("verdict", carried("verdict")),
                ("why", carried("why")),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        );
    }
    write_sheet(out, &rows)?;
    let new = rows
        .iter()
        .filter(|r| r.get("verdict").is_none_or(String::is_empty))
        .count();
    println!(
        "{}: {} rows, {new} to judge, {} carried",
        out.file_name().unwrap_or_default().to_string_lossy(),
        rows.len(),
        rows.len() - new
    );
    if carry.is_some() {
        let mut gone: IndexMap<(String, String), usize> = IndexMap::new();
        for row in earlier.values() {
            let rule = row.get("rule").cloned().unwrap_or_default();
            let verdict = row.get("verdict").cloned().unwrap_or_default();
            let verdict = if verdict.is_empty() {
                "unjudged".to_string()
            } else {
                verdict
            };
            *gone.entry((rule, verdict)).or_insert(0) += 1;
        }
        let mut keys: Vec<&(String, String)> = gone.keys().collect();
        keys.sort_by_key(|(rule, verdict)| {
            let (lang, id) = key_parts(rule);
            (lang.to_string(), id, verdict.clone())
        });
        for key in keys {
            println!("  vanished #{} {}: {}", key.0, key.1, gone[key]);
        }
    }
    Ok(())
}

/// Pooled per rule, per arm (cause prefix) and per tier: real / fp / n, FP
/// rate, FAIL above the bar (a tier line above its tier bar: 1 - TIER_BAR),
/// THIN below min-n; blanks are counted, never read.
fn tally(sheets: &[PathBuf], bar: f64, min_n: i64) -> Result<u8> {
    let mut rows = Vec::new();
    for path in sheets {
        rows.extend(read_sheet(path)?);
    }
    let (mut by_rule, mut by_arm, mut by_tier) = (Counts::new(), Counts::new(), Counts::new());
    let mut bad = 0;
    for row in &rows {
        let cell = |name: &str| row.get(name).cloned().unwrap_or_default();
        let verdict = cell("verdict").trim().to_lowercase();
        let verdict = if verdict.is_empty() {
            "blank".to_string()
        } else {
            verdict
        };
        if !VERDICTS.contains(&verdict.as_str()) && verdict != "blank" {
            bad += 1;
        }
        let rule = cell("rule");
        bump(&mut by_rule, &rule, &verdict);
        bump(&mut by_arm, &format!("{rule}:{}", cell("arm")), &verdict);
        bump(&mut by_tier, &format!("{rule}:{}", cell("tier")), &verdict);
    }
    if bad > 0 {
        println!("{bad} rows carry a verdict outside {}", py_tuple(&VERDICTS));
    }
    println!(
        "{:<28}{:>6}{:>6}{:>6}{:>7}{:>7}  status",
        "rule/arm", "real", "fp", "n", "blank", "fp%"
    );
    let mut failing = 0;
    let mut names: Vec<&String> = by_rule.keys().collect();
    names.sort_by_key(|k| {
        let (lang, id) = key_parts(k);
        (lang.to_string(), id)
    });
    for rule in names {
        let mut failed = line(&format!("#{rule}"), &by_rule[rule], false, bar, min_n);
        let mut arms: Vec<&String> = by_arm
            .keys()
            .filter(|a| a.rsplit_once(':').is_some_and(|(head, _)| head == rule))
            .collect();
        arms.sort();
        if arms.len() > 1 {
            for arm in &arms {
                let name = arm.rsplit_once(':').map_or("", |(_, tail)| tail);
                line(name, &by_arm[*arm], true, bar, min_n);
            }
        }
        let mut tiers: Vec<&String> = by_tier
            .keys()
            .filter(|t| t.rsplit_once(':').is_some_and(|(head, _)| head == rule))
            .collect();
        tiers.sort();
        for key in tiers {
            let name = key.rsplit_once(':').map_or("", |(_, tail)| tail);
            let at = round6(1.0 - tier_bar(name));
            failed |= line(&format!("[{name}]"), &by_tier[key], true, at, min_n);
        }
        failing += i64::from(failed);
    }
    let mut total = BTreeMap::new();
    for counts in by_rule.values() {
        for (verdict, n) in counts {
            *total.entry(verdict.clone()).or_insert(0) += n;
        }
    }
    line("ALL", &total, false, bar, min_n);
    println!(
        "\n{failing} rules over the bar ({:.0}% fp, n >= {min_n})",
        bar * 100.0
    );
    Ok(u8::from(failing > 0))
}

type Counts = IndexMap<String, BTreeMap<String, i64>>;

fn bump(counts: &mut Counts, key: &str, verdict: &str) {
    *counts
        .entry(key.to_string())
        .or_default()
        .entry(verdict.to_string())
        .or_insert(0) += 1;
}

fn at(counts: &BTreeMap<String, i64>, verdict: &str) -> i64 {
    counts.get(verdict).copied().unwrap_or(0)
}

fn tier_bar(name: &str) -> f64 {
    match name {
        "proved" => Tier::Proved.bar(),
        "indexed" => Tier::Indexed.bar(),
        _ => Tier::Heuristic.bar(),
    }
}

fn round6(x: f64) -> f64 {
    format!("{x:.6}").parse().unwrap_or(x)
}

/// Python's `repr` of a tuple of strings.
fn py_tuple(items: &[&str]) -> String {
    format!(
        "({})",
        items
            .iter()
            .map(|i| format!("'{i}'"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn line(name: &str, counts: &BTreeMap<String, i64>, is_arm: bool, bar: f64, min_n: i64) -> bool {
    let (real, fp) = (at(counts, "real"), at(counts, "fp"));
    let n = real + fp;
    let rate = if n > 0 { fp as f64 / n as f64 } else { 0.0 };
    let status = if n < min_n {
        "THIN"
    } else if rate > bar {
        "FAIL"
    } else {
        "PASS"
    };
    let label = format!("{}{name}", if is_arm { "  " } else { "" });
    println!(
        "{label:<28}{real:>6}{fp:>6}{n:>6}{:>7}{:>6.0}%  {status}",
        at(counts, "blank"),
        rate * 100.0
    );
    status == "FAIL"
}

fn flag<'a>(args: &[&'a str], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| *a == name)
        .and_then(|i| args.get(i + 1))
        .copied()
}

pub fn main(args: &[&str]) -> Result<u8> {
    let rules: Option<Vec<&str>> = flag(args, "--rules").map(|list| list.split(',').collect());
    match args.first().copied() {
        Some("sheet") if args.len() >= 3 => {
            sheet(
                Path::new(args[1]),
                Path::new(args[2]),
                flag(args, "--carry").map(Path::new),
                rules.as_ref(),
            )?;
            Ok(0)
        }
        Some("tally") if args.len() >= 2 => {
            let named: Vec<&str> = ["--bar", "--min-n"]
                .iter()
                .filter_map(|f| flag(args, f))
                .collect();
            let sheets: Vec<PathBuf> = args[1..]
                .iter()
                .filter(|a| !a.starts_with("--") && !named.contains(a))
                .map(PathBuf::from)
                .collect();
            let bar = flag(args, "--bar")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3);
            let min_n = flag(args, "--min-n")
                .and_then(|v| v.parse().ok())
                .unwrap_or(5);
            tally(&sheets, bar, min_n)
        }
        _ => {
            eprintln!(
                "usage: cargo xtask gauntlet sheet <audit.json> <out.tsv> | tally <sheet.tsv>..."
            );
            Ok(2)
        }
    }
}
