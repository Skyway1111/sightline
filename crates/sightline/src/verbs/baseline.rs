//! `sightline baseline` (port of `cli.cmd_baseline`): the full audit
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
    let path = root.join(ratchet::BASELINE_NAME);
    let collected = pipeline::collect(root, config, registry, langs, false, None)?;
    let kept = ratcheted(&collected.kept, registry);
    let old = ratchet::load(&path)?;
    if prune && old.is_none() {
        eprintln!("no baseline to prune");
        return Ok(1);
    }
    let counts = match &old {
        Some(old) if prune => ratchet::prune(&kept, &old.counts),
        _ => ratchet::snapshot(&kept),
    };
    let line = match &old {
        Some(old) if prune => format!(
            "pruned baseline: {} -> {} keys\n",
            old.counts.len(),
            counts.len()
        ),
        _ => format!(
            "baseline written: {} keys, {} findings\n",
            counts.len(),
            counts.values().sum::<u32>()
        ),
    };
    ratchet::save(&path, &Baseline { counts })?;
    std::io::stdout().write_all(line.as_bytes())?;
    Ok(0)
}
