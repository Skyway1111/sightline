//! `sightline baseline`: the full audit
//! pipeline, oracle passes included, then the RATCHET keys written or
//! pruned.

use std::io::Write;

use anyhow::Result;
use camino::Utf8Path;

use sightline_core::config::Config;
use sightline_core::ratchet::{self, Baseline};
use sightline_core::registry::Registry;

use crate::pipeline::{self, Languages};
use crate::verbs::gate::ratcheted;

pub fn run(
    root: &Utf8Path,
    config: &Config,
    registry: &Registry,
    langs: &Languages,
    prune: bool,
) -> Result<u8> {
    let collected = pipeline::collect(root, config, registry, langs, false, None)?;
    let kept = ratcheted(&collected.kept, registry);
    let old = ratchet::load(root)?;
    if prune && old.is_none() {
        eprintln!("no baseline to prune");
        return Ok(1);
    }
    let counts = match &old {
        Some(old) if prune => ratchet::prune(&kept, &old.counts, &collected.repo),
        _ => ratchet::snapshot(&kept, &collected.repo),
    };
    let mut line = match &old {
        Some(old) if prune => format!(
            "pruned baseline: {} -> {} keys\n",
            old.counts.len(),
            counts.len()
        ),
        _ => format!(
            "baseline written: {} keys, {} findings\n",
            counts.len(),
            counts.values().map(|e| e.count).sum::<u32>()
        ),
    };
    if ratchet::save(root, &Baseline { counts })? {
        line.push_str(&format!(
            "{} replaces {}; commit the one and the removal of the other\n",
            ratchet::BASELINE_NAME,
            ratchet::LEGACY_NAME
        ));
    }
    std::io::stdout().write_all(line.as_bytes())?;
    Ok(0)
}
