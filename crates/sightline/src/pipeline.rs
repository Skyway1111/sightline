//! `collect`: the one audit collection every verb shares. Per detected language, facts and provers are built, the rules run
//! into one list, and the list is suppressed as one. The baseline diff and
//! the rank stay with the callers: `audit` and `gate` split there.
//!
//! The registry and the language table live here too, so `debug dump` and
//! the verbs read one home.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;

use sightline_core::config::Config;
use sightline_core::findings::{Finding, Rel, Sink};
use sightline_core::lang::{BuildMode, Language, Listing, Repo, Stack, detect};
use sightline_core::registry::Registry;
use sightline_core::rule::{RuleRecord, RuleSet};
use sightline_core::suppress::suppress;
use sightline_core::walk;
use sightline_py_rules::stack::PyLanguage;
use sightline_rs_rules::stack::RsLanguage;

/// `(label, seconds)` per pass: what `audit --profile` writes
/// (`profile_audit.py:Walls`).
pub type Walls = Vec<(String, f64)>;

/// What one collection leaves: the merged view, the kept findings and the
/// suppressed ones (`gate.collect`'s triple), plus the walls the run
/// measured.
pub struct Collected {
    pub repo: Repo,
    pub kept: Vec<Finding>,
    pub suppressed: Vec<Finding>,
    pub walls: Walls,
}

/// The registry `render`, `suppress` and `gate` read: each language's
/// readings off its rules crate. `Registry::new` sorts by (id, lang) and
/// rejects a duplicate reading, so the two lists become one.
pub fn registry() -> Result<Registry> {
    let mut records: Vec<RuleRecord> = sightline_py_rules::RULES
        .iter()
        .map(|r| r.record.clone())
        .collect();
    records.extend(sightline_rs_rules::RULES.iter().map(|r| r.record.clone()));
    Registry::new(records)
}

/// The language records, in registry order: a Rust
/// tree builds the Rust stack alone, an unmarked tree falls back to the
/// Python one.
pub struct Languages {
    py: PyLanguage,
    rs: RsLanguage,
}

impl Languages {
    pub fn new(registry: &Registry) -> Languages {
        Languages {
            py: PyLanguage::new(registry.id_by_slug.clone()),
            rs: RsLanguage {
                ids_by_slug: registry.id_by_slug.clone(),
            },
        }
    }

    pub fn registered(&self) -> Vec<&dyn Language> {
        vec![&self.py, &self.rs]
    }
}

/// `Path.resolve()`: the real path, without the `\\?\` prefix Windows
/// canonicalization adds, since the reference prints it as Python spells it.
pub fn resolve(path: &str) -> Result<Utf8PathBuf> {
    let real = std::fs::canonicalize(path).with_context(|| format!("no such root: {path}"))?;
    let text = real.to_string_lossy().into_owned();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text).to_string();
    Ok(Utf8PathBuf::from(text))
}

/// `collect`'s restriction: the config's own `rules-off`, plus every id
/// `--rules` left out, so the oracle wiring and the run read one set
/// (`gate.collect`: `off |= set(RULE_BY_ID) - only`).
pub fn off_set(config: &Config, registry: &Registry, only: Option<&RuleSet>) -> RuleSet {
    let mut off = config.rules_off.clone();
    if let Some(only) = only {
        off.extend(
            registry
                .rules
                .iter()
                .map(|r| r.id.to_string())
                .filter(|id| !only.contains(id)),
        );
    }
    off
}

/// The stacks a root builds, and what `detect` said of the languages it
/// skipped.
pub type Built = (Vec<Box<dyn Stack>>, Vec<String>);

/// One stack per detected language, each built over the shared walk.
pub fn build_stacks(
    root: &Utf8Path,
    config: &Config,
    langs: &Languages,
    listing: &Listing,
    only: Option<&IndexSet<Rel>>,
    off: &RuleSet,
    mode: BuildMode,
) -> Result<Built> {
    let registered = langs.registered();
    let (detected, notes) = detect(root, &registered);
    let mut stacks: Vec<Box<dyn Stack>> = Vec::new();
    for lang in detected {
        stacks.push(lang.build(root, config, listing, only, off, mode)?);
    }
    Ok((stacks, notes))
}

/// Every stack's rules into one list, suppressed as one. The oracle's
/// database is dropped here unless the caller still needs it (`fix`,
/// `facts`). `notes` are `build_stacks`'s, kept for the header.
pub fn collect_stacks(
    stacks: Vec<Box<dyn Stack>>,
    notes: Vec<String>,
    registry: &Registry,
    config: &Config,
    off: &RuleSet,
    keep_oracle: bool,
) -> Collected {
    let mut sink = Sink::new();
    let mut walls: Walls = Vec::new();
    for stack in &stacks {
        let mut on_rule = |id: &str, wall: Duration| {
            let slug = registry.by_id(id).map_or("", |r| r.slug);
            walls.push((format!("rule #{id} {slug}"), wall.as_secs_f64()));
        };
        stack.run_rules(off, &mut sink, Some(&mut on_rule));
    }
    let mut repo = Repo::new(stacks);
    repo.notes = notes;
    let (kept, suppressed) = suppress(sink.0, &repo, &registry.id_by_slug, &config.overrides);
    if !keep_oracle {
        close(&mut repo, &mut walls);
    }
    for stack in &repo.stacks {
        walls.extend(stack.passes());
    }
    Collected {
        repo,
        kept,
        suppressed,
        walls,
    }
}

/// `provers.close` over every stack, timed as one pass.
pub fn close(repo: &mut Repo, walls: &mut Walls) {
    let started = Instant::now();
    for stack in &mut repo.stacks {
        stack.close();
    }
    walls.push(("provers close".into(), started.elapsed().as_secs_f64()));
}

/// Per detected language: facts, provers, rules; the findings concatenated,
/// then suppressed as one list.
pub fn collect(
    root: &Utf8Path,
    config: &Config,
    registry: &Registry,
    langs: &Languages,
    keep_oracle: bool,
    only: Option<&RuleSet>,
) -> Result<Collected> {
    let off = off_set(config, registry, only);
    let listing = walk::discover(root, config);
    let (stacks, notes) = build_stacks(root, config, langs, &listing, None, &off, BuildMode::Full)?;
    Ok(collect_stacks(
        stacks,
        notes,
        registry,
        config,
        &off,
        keep_oracle,
    ))
}

/// `cli.cmd_fix`: one diff, each language's emitter over its own files. A
/// language with no emitter contributes nothing and says so.
pub fn fix_diff(repo: &Repo, kept: &[Finding]) -> (String, Vec<String>) {
    let mut parts = String::new();
    let mut notes = Vec::new();
    for stack in &repo.stacks {
        let mine: Vec<Finding> = kept
            .iter()
            .filter(|f| std::ptr::eq(repo.owner(&f.site.rel), stack.neutral()))
            .cloned()
            .collect();
        match stack.fix(&mine) {
            Some(part) => parts.push_str(&part),
            None => notes.push(format!("{}: no emitter", stack.lang())),
        }
    }
    (parts, notes)
}

/// The header's `notes` and `provers` block: every stack's, in stack order
/// (`cmd_audit`). Read after `close`, as the reference reads them.
pub fn header(repo: &Repo) -> (Vec<String>, serde_json::Map<String, serde_json::Value>) {
    let mut notes = repo.notes.clone();
    let mut provers = serde_json::Map::new();
    for stack in &repo.stacks {
        notes.extend(stack.notes());
        if let serde_json::Value::Object(block) = stack.provenance() {
            provers.extend(block);
        }
    }
    (notes, provers)
}

#[cfg(test)]
mod tests {
    use super::*;

    use sightline_core::testing::{Q, SyntheticStack};

    /// A language with no emitter contributes nothing to the
    /// diff and says so, rather than being skipped silently.
    #[test]
    fn a_language_with_no_emitter_is_an_empty_diff_and_a_note() {
        let stack = SyntheticStack::new(&Q, &[("m.q", "fn main() {}\n")]);
        let repo = Repo::new(vec![Box::new(stack)]);

        assert_eq!(
            fix_diff(&repo, &[]),
            (String::new(), vec!["q: no emitter".to_string()])
        );
    }

    /// `--rules` folds into `off` as `gate.collect` folds it: every id the
    /// caller did not name, plus the config's own.
    #[test]
    fn only_switches_off_every_id_it_does_not_name() {
        let registry = sightline_core::testing::registry();
        let mut config = Config::new();
        config.rules_off.insert("42".to_string());
        let only: RuleSet = ["11".to_string()].into_iter().collect();

        let off = off_set(&config, &registry, Some(&only));
        assert!(!off.contains("11"));
        assert!(off.contains("1") && off.contains("3") && off.contains("42"));
        assert_eq!(off_set(&config, &registry, None), config.rules_off);
    }
}
