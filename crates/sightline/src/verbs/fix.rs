//! `sightline fix`: one diff, the languages'
//! patches concatenated. It never touches the tree.

use std::io::Write;

use anyhow::Result;
use camino::Utf8Path;

use sightline_core::config::Config;
use sightline_core::registry::Registry;
use sightline_core::rule::RuleSet;

use crate::pipeline::{self, Languages};

pub fn run(
    root: &Utf8Path,
    config: &Config,
    registry: &Registry,
    langs: &Languages,
    out: Option<&str>,
    rules: Option<&RuleSet>,
) -> Result<u8> {
    let mut collected = pipeline::collect(root, config, registry, langs, true, rules)?;
    let (diff, notes) = pipeline::fix_diff(&collected.repo, &collected.kept);
    pipeline::close(&mut collected.repo, &mut collected.walls);
    match out {
        Some(path) => std::fs::write(path, diff.as_bytes())?,
        None => std::io::stdout().write_all(diff.as_bytes())?,
    }
    for note in &notes {
        eprintln!("fix: note: {note}");
    }
    // counted off the diff: what shipped, not what verified
    let (mut verified, mut files) = (0, 0);
    for line in sightline_core::pytext::splitlines(&diff) {
        verified += usize::from(line.starts_with("# sightline-fix: "));
        files += usize::from(line.starts_with("--- a/"));
    }
    eprintln!("fix: {verified} verified finding(s) across {files} file(s)");
    Ok(0)
}
