//! `sightline audit`: the whole pipeline, ranked,
//! rendered as text, JSON or SARIF. An exit-0 report: only `gate` blocks.

use std::io::Write;
use std::time::Instant;

use anyhow::Result;
use serde::Serialize;

use sightline_core::config::Config;
use sightline_core::lang::FactsView;
use sightline_core::rank::rank;
use sightline_core::ratchet;
use sightline_core::registry::Registry;
use sightline_core::render::{AuditResult, to_json, to_sarif, to_text};
use sightline_core::rule::RuleSet;

use crate::pipeline::{self, Languages, Walls};
use crate::verbs::{rel_prefixes, under};

pub struct Options<'a> {
    pub json: bool,
    pub sarif: bool,
    pub all: bool,
    pub paths: &'a [String],
    pub rules: Option<&'a RuleSet>,
    pub profile: Option<&'a str>,
    /// `--top N`: the N strongest findings alone
    pub top: Option<usize>,
}

pub fn run(
    root: &camino::Utf8Path,
    config: &Config,
    registry: &Registry,
    langs: &Languages,
    opts: &Options,
) -> Result<u8> {
    let paths = match rel_prefixes(root, opts.paths) {
        Ok(paths) => paths,
        Err(message) => {
            eprintln!("{message}");
            return Ok(1);
        }
    };
    let started = Instant::now();
    let collected = pipeline::collect(root, config, registry, langs, false, opts.rules)?;
    let mut kept = collected.kept;
    let mut absorbed = 0;
    match ratchet::load(root)? {
        Some(baseline) if !opts.all => {
            (kept, absorbed) = ratchet::diff(kept, &baseline.counts, &collected.repo);
        }
        _ => {}
    }
    let mut ranked = rank(kept, &collected.repo);
    // facts stay repo-wide; the filter is the last stage
    if !paths.is_empty() {
        ranked.retain(|f| under(&f.site.rel, &paths));
    }
    let findings = ranked.len();
    let cut = opts.top.map_or(0, |top| findings.saturating_sub(top));
    ranked.truncate(findings - cut);
    let (notes, provers) = pipeline::header(&collected.repo);
    let result = AuditResult {
        findings: ranked,
        suppressed: collected.suppressed.len(),
        absorbed: absorbed as usize,
        notes,
        facts: &collected.repo,
        provers,
        rules_off: config.rules_off.iter().cloned().collect(),
        rules_only: opts
            .rules
            .map(|r| r.iter().cloned().collect())
            .unwrap_or_default(),
        paths,
        cut,
    };
    let rendered = match (opts.sarif, opts.json) {
        (true, _) => to_sarif(&result, registry),
        (_, true) => to_json(&result, registry),
        _ => to_text(&result),
    };
    std::io::stdout().write_all(rendered.as_bytes())?;
    if let Some(path) = opts.profile {
        let profile = Profile::new(
            root.as_str(),
            collected.repo.modules().len(),
            findings,
            started.elapsed().as_secs_f64(),
            collected.walls,
        );
        std::fs::write(path, serde_json::to_string_pretty(&profile)? + "\n")?;
    }
    Ok(0)
}

/// One audit's walls, biggest first: the receipt `xtask profile` reads
/// (`profile_audit.py:Profile`).
#[derive(Serialize)]
struct Profile {
    root: String,
    modules: usize,
    findings: usize,
    total: f64,
    passes: Vec<(String, f64)>,
}

/// `round(x, 3)`: the reference writes every wall at millisecond grain.
fn ms(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}

impl Profile {
    fn new(root: &str, modules: usize, findings: usize, total: f64, walls: Walls) -> Profile {
        let mut passes = walls;
        passes.sort_by(|a, b| b.1.total_cmp(&a.1));
        let passes = passes.into_iter().map(|(l, secs)| (l, ms(secs))).collect();
        Profile {
            root: root.to_string(),
            modules,
            findings,
            total: ms(total),
            passes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_reads_biggest_pass_first_at_millisecond_grain() {
        let profile = Profile::new(
            "r",
            2,
            3,
            1.234_4,
            vec![
                ("facts".into(), 0.1),
                ("rule #11 structural-clones".into(), 0.500_4),
                ("provers".into(), 0.100_04),
            ],
        );
        assert_eq!(profile.total, 1.234);
        assert_eq!(
            profile.passes,
            // the sort reads the raw wall and the rounding comes after, so
            // `provers` leads two rows that print the same number
            [
                ("rule #11 structural-clones".to_string(), 0.5),
                ("provers".to_string(), 0.1),
                ("facts".to_string(), 0.1),
            ]
        );
    }
}
