//! `sightline facts`: one audit, then the model
//! it built printed for one symbol or module.

use std::io::Write;

use anyhow::Result;
use camino::Utf8Path;

use sightline_core::config::Config;
use sightline_core::pytext::repr_str;
use sightline_core::rank::rank;
use sightline_core::registry::Registry;

use crate::pipeline::{self, Languages};

pub fn run(
    root: &Utf8Path,
    config: &Config,
    registry: &Registry,
    langs: &Languages,
    qname: &str,
) -> Result<u8> {
    let mut collected = pipeline::collect(root, config, registry, langs, true, None)?;
    let ranked = rank(collected.kept, &collected.repo);
    // the stack whose model answers for this name; the first otherwise, so
    // its near-miss list is what a typo gets back
    let holder = collected
        .repo
        .stacks
        .iter()
        .position(|s| {
            let n = s.neutral();
            n.symbols.contains_key(qname) || n.modules.contains_key(qname)
        })
        .unwrap_or(0);
    let answer = collected.repo.stacks[holder].describe(&ranked, qname);
    pipeline::close(&mut collected.repo, &mut collected.walls);
    match answer {
        Ok(out) => {
            std::io::stdout().write_all(out.as_bytes())?;
            Ok(0)
        }
        Err(near) => {
            let nearest = match near.is_empty() {
                true => String::new(),
                false => format!("; nearest: {}", near.join(", ")),
            };
            eprintln!("no symbol or module {}{nearest}", repr_str(qname));
            Ok(1)
        }
    }
}
