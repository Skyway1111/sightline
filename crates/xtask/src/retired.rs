//! `data/retired.toml`: the burial rows of `docs/review/decisions.tsv`, so a
//! released binary answers `explain <retired id>` without a checkout
//! (decision 10).
//!
//! The reading is `cli.py:cmd_retired`'s: a row whose decision names the id
//! and holds `cut` or `retired`, narrowed to the rows whose decision leads
//! with the id when any does. One table per row; `explain` prints them in
//! order.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use sightline_core::registry::RETIRED;

use crate::paths::workspace_root;

pub struct Burial {
    pub id: String,
    pub ts: String,
    pub decision: String,
    pub why: String,
    pub evidence: String,
}

/// `#<id>` not followed by another digit (`cli.py:207`, R7: the lookahead is
/// a digit check on the next character).
fn names(decision: &str, id: &str) -> bool {
    let needle = format!("#{id}");
    decision.match_indices(&needle).any(|(at, _)| {
        !decision[at + needle.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    })
}

pub fn burials(tsv: &str, id: &str) -> Vec<Burial> {
    let rows: Vec<Vec<&str>> = tsv
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect())
        .filter(|row: &Vec<&str>| {
            row.len() > 4
                && names(row[2], id)
                && (row[2].contains("cut") || row[2].contains("retired"))
        })
        .collect();
    let lead = format!("#{id}");
    let chosen: Vec<&Vec<&str>> = match rows.iter().filter(|r| r[2].starts_with(&lead)).count() {
        0 => rows.iter().collect(),
        _ => rows.iter().filter(|r| r[2].starts_with(&lead)).collect(),
    };
    chosen
        .into_iter()
        .map(|row| Burial {
            id: id.to_string(),
            ts: row[0].to_string(),
            decision: row[2].to_string(),
            why: row[3].to_string(),
            evidence: row[4].to_string(),
        })
        .collect()
}

/// A TOML basic string: the only escapes a decisions row can need.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    crate::text::escape_into(&mut out, s, crate::text::Style::Toml);
    out.push('"');
    out
}

pub fn render(tsv: &str) -> Result<String> {
    let mut ids: Vec<&&str> = RETIRED.iter().collect();
    ids.sort_by_key(|id| id.parse::<u32>().unwrap_or(0));
    let mut out = String::from(
        "# Written by `cargo xtask retired` from docs/review/decisions.tsv.\n\
         # An id no rule holds; `explain <id>` prints its burial.\n",
    );
    for id in ids {
        let rows = burials(tsv, id);
        if rows.is_empty() {
            bail!("no burial row in decisions.tsv names #{id}");
        }
        for b in rows {
            out.push_str("\n[[retired]]\n");
            out.push_str(&format!("id = {}\n", quote(&b.id)));
            out.push_str(&format!("ts = {}\n", quote(&b.ts)));
            out.push_str(&format!("decision = {}\n", quote(&b.decision)));
            out.push_str(&format!("why = {}\n", quote(&b.why)));
            out.push_str(&format!("evidence = {}\n", quote(&b.evidence)));
        }
    }
    Ok(out)
}

fn source(args: &[&str]) -> PathBuf {
    args.windows(2)
        .find(|w| w[0] == "--from")
        .map(|w| PathBuf::from(w[1]))
        .unwrap_or_else(|| workspace_root().join("docs/review/decisions.tsv"))
}

pub fn main(args: &[&str]) -> Result<u8> {
    let tsv_path = source(args);
    let tsv = std::fs::read_to_string(&tsv_path)
        .with_context(|| format!("reading {}", tsv_path.display()))?;
    let text = render(&tsv)?;
    let out = workspace_root().join("data/retired.toml");
    write_lf(&out, &text)?;
    let ids = text.matches("\n[[retired]]\n").count();
    println!("{}: {ids} rows over {} ids", out.display(), RETIRED.len());
    Ok(0)
}

/// LF on every platform: the file is committed and `git diff` must be empty
/// after a re-run.
pub fn write_lf(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text.replace("\r\n", "\n").as_bytes())
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tsv() -> String {
        std::fs::read_to_string(workspace_root().join("docs/review/decisions.tsv")).unwrap()
    }

    #[test]
    fn every_retired_id_has_a_burial() {
        let tsv = tsv();
        for id in RETIRED {
            assert!(!burials(&tsv, id).is_empty(), "#{id} has no burial row");
        }
        assert_eq!(RETIRED.len(), 18);
    }

    #[test]
    fn the_committed_toml_is_what_this_writes() {
        let want = render(&tsv()).unwrap();
        let got = std::fs::read_to_string(workspace_root().join("data/retired.toml")).unwrap();
        assert_eq!(got.replace("\r\n", "\n"), want, "run `cargo xtask retired`");
    }

    #[test]
    fn the_id_match_stops_at_a_longer_number() {
        assert!(names("#4 cut", "4"));
        assert!(!names("#45 cut", "4"));
        assert!(names("round: #45 and #4 cut", "4"));
    }

    #[test]
    fn a_leading_row_wins_over_a_round_summary() {
        let tsv = "ts\tphase\tdecision\twhy\tevidence\n\
             t1\tp\tround cut #8 and #13\tw1\te1\n\
             t2\tp\t#8 cut for x\tw2\te2\n";
        let got = burials(tsv, "8");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].decision, "#8 cut for x");
        // #13 leads no row, so the summary answers for it
        assert_eq!(burials(tsv, "13")[0].ts, "t1");
    }
}
