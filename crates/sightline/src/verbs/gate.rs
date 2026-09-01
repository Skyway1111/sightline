//! `sightline gate` (port of `gate.py`'s gate half): the one blocking
//! surface, and the only reader that acts on posture. Fast mode: changed
//! files, single-file facts, the `scope="file"` GATE and RATCHET rules of
//! the file's language; no oracle, sub-second. Full mode: the whole audit
//! pipeline.

use std::collections::BTreeSet;
use std::io::Write;

use anyhow::Result;
use camino::Utf8Path;
use indexmap::IndexSet;

use sightline_core::config::Config;
use sightline_core::findings::{Finding, Rel, Sink};
use sightline_core::lang::{BuildMode, FactsView};
use sightline_core::ratchet;
use sightline_core::registry::Registry;
use sightline_core::rule::{Posture, RuleSet, Scope};
use sightline_core::suppress::suppress;
use sightline_core::walk;

use crate::pipeline::{self, Languages};

pub struct GateResult {
    pub blocking: Vec<Finding>,
    /// rel paths actually gated
    pub checked: Vec<String>,
    pub suppressed: usize,
    pub absorbed: u32,
    pub notes: Vec<String>,
    /// Changed files whose parse error is a verdict on the edit (Python's):
    /// they carry no findings at all, so the fast gate blocks rather than
    /// green-light a syntactically broken edit. A language whose parser is
    /// narrower than its grammar puts them in `notes` instead (`run_fast`).
    /// `--full` only names them: the ratchet absorbs existing state and a
    /// parse error has no key to be absorbed by, so a vendored unparsable
    /// file would fail every run forever.
    pub unparsable: Vec<String>,
}

impl GateResult {
    fn new(
        blocking: Vec<Finding>,
        checked: Vec<String>,
        suppressed: usize,
        absorbed: u32,
        mut notes: Vec<String>,
        unparsable: Vec<String>,
    ) -> GateResult {
        notes.extend(unparsable_notes(&unparsable));
        GateResult {
            blocking,
            checked,
            suppressed,
            absorbed,
            notes,
            unparsable,
        }
    }

    pub fn passed(&self) -> bool {
        self.blocking.is_empty() && self.unparsable.is_empty()
    }
}

/// `facts.errors` as gate notes (the wording, in one place).
pub fn unparsable_notes(errors: &[String]) -> Vec<String> {
    errors.iter().map(|e| format!("unparsable: {e}")).collect()
}

/// The baseline's only keys (GATE blocks unbaselined, REPORT never).
pub fn ratcheted(findings: &[Finding], registry: &Registry) -> Vec<Finding> {
    findings
        .iter()
        .filter(|f| registry.posture_of(f.rule, f.lang) == Some(Posture::Ratchet))
        .cloned()
        .collect()
}

/// `(location-sorted blockers, absorbed count)`: GATE blocks wherever it
/// ran, RATCHET blocks new-vs-baseline.
fn blockers(
    root: &Utf8Path,
    findings: &[Finding],
    registry: &Registry,
) -> Result<(Vec<Finding>, u32)> {
    let baseline = ratchet::load(&root.join(ratchet::BASELINE_NAME))?;
    let (ratchet_side, absorbed) = match baseline {
        Some(baseline) => ratchet::diff(ratcheted(findings, registry), &baseline.counts),
        None => (ratcheted(findings, registry), 0),
    };
    let mut blocking: Vec<Finding> = findings
        .iter()
        .filter(|f| registry.posture_of(f.rule, f.lang) == Some(Posture::Gate))
        .cloned()
        .collect();
    blocking.extend(ratchet_side);
    let key = |f: &Finding| {
        (
            f.site.rel.clone(),
            f.site.line,
            f.site.col,
            f.rule.parse::<u32>().unwrap_or(0),
            f.cause.clone(),
        )
    };
    blocking.sort_by_key(key);
    Ok((blocking, absorbed))
}

/// What the fast gate runs for one language: every rule but its file-scoped
/// blockers is off, so the stack's own runner reads one restriction
/// (`run_fast`'s `file_rules`). `off` is keyed by id alone, so the kept ids
/// are collected first: a sibling language's reading of a shared id must not
/// switch this language's off.
fn fast_off(registry: &Registry, lang: &str, config: &Config) -> RuleSet {
    let on: BTreeSet<&str> = registry
        .rules
        .iter()
        .filter(|r| {
            r.lang == lang
                && r.scope == Scope::File
                && r.posture != Posture::Report
                && !config.rules_off.contains(r.id)
        })
        .map(|r| r.id)
        .collect();
    registry
        .rules
        .iter()
        .filter(|r| !on.contains(r.id))
        .map(|r| r.id.to_string())
        .collect()
}

/// Single-file facts per changed file, and only the file-scoped rules that
/// may block: no oracle, no git. A file is gated by the language whose
/// suffix it spells.
pub fn run_fast(
    root: &Utf8Path,
    files: &[String],
    config: &Config,
    registry: &Registry,
    langs: &Languages,
) -> Result<GateResult> {
    let mut rels: BTreeSet<String> = BTreeSet::new();
    for f in files {
        // outside the tree: not ours to gate
        if let Ok(real) = pipeline::resolve(root.join(f).as_str())
            && let Ok(rel) = real.strip_prefix(root)
        {
            rels.insert(rel.as_str().replace('\\', "/"));
        }
    }
    let listing = walk::discover(root, config);
    let known: BTreeSet<&str> = listing.iter().map(|(_, rel)| rel.as_str()).collect();

    let mut findings: Vec<Finding> = Vec::new();
    let mut checked: Vec<String> = Vec::new();
    let mut unparsable: Vec<String> = Vec::new();
    let mut grammar_gaps: Vec<String> = Vec::new();
    let mut suppressed = 0;
    let registered = langs.registered();
    // Only the languages the diff spells are detected: a language with no
    // changed file of its suffix contributes nothing either way, and
    // `detect`'s no-marker fallback can gate nothing (a known `.py` file
    // marks the Python stack by existing), so neither skip changes a verdict.
    let detected = registered
        .iter()
        .copied()
        .filter(|l| rels.iter().any(|rel| rel.ends_with(l.suffix())))
        .filter(|l| l.detect(root));
    for lang in detected {
        // tree-sitter-rust 0.24 rejects valid Rust (a `#[cfg]` on a
        // struct-pattern field, a turbofish struct pattern in a parameter),
        // so an unparsable `.rs` is a grammar gap as often as a broken edit:
        // it is named and never blocks until `cargo check` is the syntax
        // verdict. An unparsable `.py` stays a broken edit.
        let blocks_on_parse_error = lang.name() == "py";
        let off = fast_off(registry, lang.name(), config);
        // deleted, excluded and other languages' files carry none. The
        // reference builds one file at a time; these build as one, which a
        // `scope="file"` rule reads the same by its own definition, and the
        // errors sort back into the reference's rel order (each spells its
        // rel first).
        let only: IndexSet<Rel> = rels
            .iter()
            .filter(|rel| known.contains(rel.as_str()) && rel.ends_with(lang.suffix()))
            .map(|rel| Rel::from(rel.as_str()))
            .collect();
        if only.is_empty() {
            continue;
        }
        checked.extend(only.iter().map(|rel| rel.to_string()));
        let stack = lang.build(root, config, &listing, Some(&only), &off, BuildMode::File)?;
        let mut errors = stack.neutral().errors.clone();
        errors.sort();
        if blocks_on_parse_error {
            unparsable.extend(errors);
        } else {
            grammar_gaps.extend(errors);
        }
        let mut sink = Sink::new();
        stack.run_rules(&off, &mut sink, None);
        let (kept, sup) = suppress(sink.0, stack.neutral(), &registry.id_by_slug);
        suppressed += sup.len();
        findings.extend(kept);
    }
    let (blocking, absorbed) = blockers(root, &findings, registry)?;
    checked.sort();
    let mut notes = vec!["fast gate: oracle and repo-scope rules not run".to_string()];
    notes.extend(unparsable_notes(&grammar_gaps));
    Ok(GateResult::new(
        blocking, checked, suppressed, absorbed, notes, unparsable,
    ))
}

/// Whole audit pipeline: blocks per posture.
pub fn run_full(
    root: &Utf8Path,
    config: &Config,
    registry: &Registry,
    langs: &Languages,
) -> Result<GateResult> {
    let collected = pipeline::collect(root, config, registry, langs, false, None)?;
    let (blocking, absorbed) = blockers(root, &collected.kept, registry)?;
    let mut checked: Vec<String> = collected
        .repo
        .modules()
        .values()
        .map(|m| m.rel.to_string())
        .collect();
    checked.sort();
    let (mut notes, _) = pipeline::header(&collected.repo);
    notes.extend(unparsable_notes(collected.repo.errors()));
    Ok(GateResult::new(
        blocking,
        checked,
        collected.suppressed.len(),
        absorbed,
        notes,
        Vec::new(),
    ))
}

pub fn run(
    root: &Utf8Path,
    config: &Config,
    registry: &Registry,
    langs: &Languages,
    files: Option<&[String]>,
    since: Option<&str>,
    full: bool,
) -> Result<u8> {
    let result = if full {
        if files.is_some() || since.is_some() {
            eprintln!("--full gates the whole tree; drop --files/--since");
            return Ok(2);
        }
        run_full(root, config, registry, langs)?
    } else {
        let listed;
        let files = match files {
            Some(files) => files,
            None => match sightline_core::git::changed_files(root, since) {
                Some(found) => {
                    listed = found;
                    &listed
                }
                None => {
                    eprintln!("not a git repository; pass --files");
                    return Ok(2);
                }
            },
        };
        run_fast(root, files, config, registry, langs)?
    };
    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "sightline gate | files checked {} | blocking {} | suppressed {} | baselined {}",
        result.checked.len(),
        result.blocking.len(),
        result.suppressed,
        result.absorbed,
    )?;
    for note in &result.notes {
        writeln!(out, "  note: {note}")?;
    }
    for f in &result.blocking {
        let site = &f.site;
        writeln!(
            out,
            "{}:{}:{}  #{} {}",
            site.rel, site.line, site.col, f.rule, f.message
        )?;
    }
    Ok(u8::from(!result.passed()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use sightline_core::findings::{Evidence, Site};
    use sightline_core::ratchet::{Baseline, snapshot};
    use sightline_testkit::registry;

    /// One finding of `rule` on the symbol a baseline key names.
    fn at(rule: &'static str) -> Finding {
        Finding {
            rule,
            site: Site {
                rel: "m.py".into(),
                line: 1,
                col: 0,
                symbol: "m.f".into(),
            },
            message: "msg".to_string(),
            cause: format!("{rule}:m.f"),
            evidence: Evidence::ast(),
            salience: 0.0,
            fix: None,
            lang: "py",
        }
    }

    /// The GATE half of the posture contract, which the rule table can no
    /// longer show: no rule has held GATE since #31's burial, and the
    /// posture stays for a judged rule to earn. `core::testing`'s fixture
    /// registry holds the one GATE reading left (`99`), so the contract is
    /// read off that - a baseline never gets the key, and a baseline holding
    /// it anyway absorbs the RATCHET row beside it and not the GATE one.
    #[test]
    fn a_gate_reading_blocks_even_once_baselined_and_never_enters_the_baseline() {
        let reg = registry();
        let found = [at("99"), at("42")];
        let dir = tempfile::tempdir().expect("a temp root");
        let root = Utf8Path::from_path(dir.path()).expect("a UTF-8 temp root");

        assert_eq!(
            ratcheted(&found, &reg)
                .iter()
                .map(|f| f.rule)
                .collect::<Vec<_>>(),
            ["42"]
        );

        ratchet::save(
            &root.join(ratchet::BASELINE_NAME),
            &Baseline {
                counts: snapshot(&found),
            },
        )
        .expect("the baseline writes");
        let (blocking, absorbed) = blockers(root, &found, &reg).expect("the gate reads it");
        assert_eq!(
            blocking.iter().map(|f| f.rule).collect::<Vec<_>>(),
            ["99"],
            "GATE blocks wherever it ran; the RATCHET row is absorbed"
        );
        assert_eq!(absorbed, 1);
    }
}
