//! `cargo xtask bench-tables`: `benchmarks.md`'s measured tables, regenerated
//! from a corpus out-dir, never pasted by hand. One per-rule fire-rate table
//! per language (its repos are the corpus table, one column each, keyed by
//! initial), the Rust corpus table and merged-calculator's per-pass profile.
//!
//! Each table sits between `<!-- generated: <name> -->` and
//! `<!-- /generated: <name> -->` markers; a table whose input the out-dir
//! does not hold is left exactly as it stands.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde_json::Value;

use crate::corpus::read_json;
use crate::paths::workspace_root;

/// The corpus table's rows for one language, in ladder order
/// (`crates/xtask/corpus.toml`, the one home).
fn targets(lang: &str) -> Vec<(String, String)> {
    crate::corpus::targets(Some(lang), None)
        .unwrap_or_default()
        .into_iter()
        .map(|t| (t.name, t.role))
        .collect()
}

/// One column per corpus repo of the language, keyed by its initial:
/// L / R / M for Python, D / T / S for Rust.
fn columns_of(lang: &str) -> Vec<(String, String)> {
    targets(lang)
        .into_iter()
        .map(|(name, _)| (name[..1].to_uppercase(), name))
        .collect()
}

/// Python's `f"{n:,}"`.
fn commas(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Python's `f"{x:.0%}"`.
fn percent(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

fn tally<K: std::hash::Hash + Eq>(audit: &Value, key: impl Fn(&Value) -> K) -> IndexMap<K, i64> {
    let mut out = IndexMap::new();
    for f in audit["findings"].as_array().into_iter().flatten() {
        *out.entry(key(f)).or_insert(0) += 1;
    }
    out
}

fn arm_of(f: &Value) -> &str {
    f["cause"]
        .as_str()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
}

/// Per-rule counts in three side-by-side rule columns, then the totals (tier
/// counts from each header) and every multi-arm rule's arms.
fn fire_rates(results: &Path, lang: &str) -> Result<String> {
    let columns = columns_of(lang);
    let mut audits: Vec<(String, Value)> = Vec::new();
    for (initial, name) in &columns {
        audits.push((
            initial.clone(),
            read_json(&results.join(format!("{name}.json")))?,
        ));
    }
    let by_rule: Vec<IndexMap<String, i64>> = audits
        .iter()
        .map(|(_, a)| tally(a, |f| f["rule"].as_str().unwrap_or_default().to_string()))
        .collect();
    let by_arm: Vec<IndexMap<(String, String), i64>> = audits
        .iter()
        .map(|(_, a)| {
            tally(a, |f| {
                (
                    f["rule"].as_str().unwrap_or_default().to_string(),
                    arm_of(f).to_string(),
                )
            })
        })
        .collect();

    let mut rules: Vec<String> = by_rule.iter().flat_map(IndexMap::keys).cloned().collect();
    rules.sort_by_key(|r| r.parse::<i64>().unwrap_or(0));
    rules.dedup();

    let mut lines: Vec<String> = Vec::new();
    if !rules.is_empty() {
        let cells: Vec<String> = rules
            .iter()
            .map(|r| {
                let counts: Vec<String> = by_rule
                    .iter()
                    .map(|c| c.get(r).copied().unwrap_or(0).to_string())
                    .collect();
                format!("{r} | {}", counts.join(" | "))
            })
            .collect();
        let per = cells.len().div_ceil(3);
        let cols: Vec<&[String]> = cells.chunks(per).collect();
        let blank = [""; 4].join(" | ");
        let heads: Vec<String> = cols
            .iter()
            .map(|_| {
                let names: Vec<&str> = columns.iter().map(|(i, _)| i.as_str()).collect();
                format!("Rule | {}", names.join(" | "))
            })
            .collect();
        lines.push(format!("| {} |", heads.join(" | ")));
        lines.push(format!("| {} |", vec!["---"; 4 * cols.len()].join(" | ")));
        for i in 0..per {
            let row: Vec<&str> = cols
                .iter()
                .map(|col| col.get(i).map_or(blank.as_str(), String::as_str))
                .collect();
            lines.push(format!("| {} |", row.join(" | ")));
        }
        lines.push(String::new());
    }

    let counts: Vec<&Value> = audits
        .iter()
        .map(|(_, a)| &a["provenance"]["counts"])
        .collect();
    let slash = |field: &str| -> String {
        counts
            .iter()
            .map(|c| c[field].to_string())
            .collect::<Vec<_>>()
            .join(" / ")
    };
    let totals: Vec<String> = columns
        .iter()
        .zip(&counts)
        .map(|((initial, _), c)| format!("{initial} {}", c["findings"]))
        .collect();
    lines.push(format!(
        "Totals: {} findings (proved {}; indexed {}).",
        totals.join(", "),
        slash("proved"),
        slash("indexed")
    ));

    let mut arms: Vec<String> = Vec::new();
    for rule in &rules {
        let mut names: Vec<&str> = by_arm
            .iter()
            .flat_map(IndexMap::keys)
            .filter(|(r, _)| r == rule)
            .map(|(_, a)| a.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        if names.len() > 1 {
            let body: Vec<String> = names
                .iter()
                .map(|a| {
                    let key = (rule.clone(), (*a).to_string());
                    let per: Vec<String> = by_arm
                        .iter()
                        .map(|c| c.get(&key).copied().unwrap_or(0).to_string())
                        .collect();
                    format!("`{a}` {}", per.join("/"))
                })
                .collect();
            arms.push(format!("#{rule} {}", body.join(", ")));
        }
    }
    if !arms.is_empty() {
        let initials: Vec<&str> = columns.iter().map(|(i, _)| i.as_str()).collect();
        lines.push(format!(
            "Arms ({}): {}.",
            initials.join("/"),
            arms.join("; ")
        ));
    }
    Ok(lines.join("\n"))
}

/// merged-calculator's passes at 1 % or more of the facts-to-rules total.
fn profile_table(results: &Path) -> Result<String> {
    let p = read_json(&results.join("profile-merged-calculator.json"))?;
    let total = p["total"].as_f64().unwrap_or(0.0);
    let passes: Vec<(&str, f64)> = p["passes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            (
                row[0].as_str().unwrap_or_default(),
                row[1].as_f64().unwrap_or(0.0),
            )
        })
        .collect();
    let oracle: f64 = passes
        .iter()
        .filter(|(label, _)| label.starts_with("oracle "))
        .map(|(_, secs)| secs)
        .sum();
    let mut lines = vec![
        "| Pass | Wall | Share |".to_string(),
        "| --- | --- | --- |".to_string(),
    ];
    for (label, secs) in &passes {
        if secs / total >= 0.01 {
            lines.push(format!(
                "| {label} | {secs:.2} s | {} |",
                percent(secs / total)
            ));
        }
    }
    lines.push(format!(
        "| **total (facts\u{2192}rules)** | **{total:.1} s** | oracle passes {oracle:.1} s ({}) |",
        percent(oracle / total)
    ));
    Ok(lines.join("\n"))
}

/// Per Rust repo: findings, the audit wall and the index the header counts
/// (documents inside and outside the root, edges); the budget is prose.
fn rs_corpus(results: &Path) -> Result<String> {
    let walls = read_json(&results.join("walls.json"))?;
    let mut lines = vec![
        "| repo | role | findings | wall | documents in / out | edges |".to_string(),
        "| --- | --- | ---: | ---: | ---: | ---: |".to_string(),
    ];
    for (name, role) in targets("rs") {
        let audit = read_json(&results.join(format!("{name}.json")))?;
        let index = &audit["provenance"]["rs"]["oracle"];
        lines.push(format!(
            "| {name} | {role} | {} | {} s | {} / {} | {} |",
            audit["findings"].as_array().map_or(0, Vec::len),
            walls[&name]["wall_s"],
            index["documents_in"],
            index["documents_out"],
            commas(index["edges"].as_i64().unwrap_or(0)),
        ));
    }
    Ok(lines.join("\n"))
}

/// Marker name -> the files that table reads.
fn table_inputs(results: &Path, name: &str) -> Vec<PathBuf> {
    match name {
        "fire-rates" | "fire-rates-rs" => {
            let lang = if name == "fire-rates" { "py" } else { "rs" };
            columns_of(lang)
                .iter()
                .map(|(_, n)| results.join(format!("{n}.json")))
                .collect()
        }
        "rs-corpus" => std::iter::once(results.join("walls.json"))
            .chain(
                targets("rs")
                    .iter()
                    .map(|(n, _)| results.join(format!("{n}.json"))),
            )
            .collect(),
        _ => vec![results.join("profile-merged-calculator.json")],
    }
}

fn build(results: &Path, name: &str) -> Result<String> {
    match name {
        "fire-rates" => fire_rates(results, "py"),
        "fire-rates-rs" => fire_rates(results, "rs"),
        "rs-corpus" => rs_corpus(results),
        _ => profile_table(results),
    }
}

const TABLES: [&str; 4] = [
    "fire-rates",
    "fire-rates-rs",
    "rs-corpus",
    "profile-merged-calculator",
];

pub fn regenerate(doc: &str, results: &Path, doc_path: &Path) -> Result<String> {
    let mut out = doc.to_string();
    for name in TABLES {
        let open = format!("<!-- generated: {name} -->\n");
        let close = format!("\n<!-- /generated: {name} -->");
        let Some(start) = out.find(&open).map(|i| i + open.len()) else {
            bail!("{}: no `generated: {name}` markers", doc_path.display());
        };
        let Some(end) = out[start..].find(&close).map(|i| i + start) else {
            bail!("{}: no `generated: {name}` markers", doc_path.display());
        };
        let missing: Vec<String> = table_inputs(results, name)
            .iter()
            .filter(|p| !p.is_file())
            .map(|p| format!("'{}'", p.file_name().unwrap_or_default().to_string_lossy()))
            .collect();
        if !missing.is_empty() {
            println!(
                "  {name}: left as it stands ({} holds no [{}])",
                results.display(),
                missing.join(", ")
            );
            continue;
        }
        let body = build(results, name)?;
        out.replace_range(start..end, &body);
    }
    Ok(out)
}

pub fn main(args: &[&str]) -> Result<u8> {
    let root = workspace_root();
    let mut doc = root.join("benchmarks.md");
    let mut results = root.join("corpus/results");
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match *arg {
            "--doc" => doc = PathBuf::from(rest.next().copied().unwrap_or_default()),
            other => results = PathBuf::from(other),
        }
    }
    // Python reads and writes the doc in text mode, so a CRLF file stays
    // CRLF: normalize in, restore on the way out.
    let raw = std::fs::read_to_string(&doc)
        .with_context(|| format!("{} holds the marked tables", doc.display()))?;
    let crlf = raw.contains("\r\n");
    let body = regenerate(&raw.replace("\r\n", "\n"), &results, &doc)?;
    std::fs::write(
        &doc,
        if crlf {
            body.replace('\n', "\r\n")
        } else {
            body
        },
    )?;
    println!(
        "{} tables regenerated from {}",
        doc.file_name().unwrap_or_default().to_string_lossy(),
        results.display()
    );
    Ok(0)
}
