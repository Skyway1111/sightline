//! Fixtures the crate's own tests and `sightline-testkit` share.
//!
//! Two languages that do not exist (`p` and `q`, `test_lang.py`), whose whole
//! model is the neutral attributes every facts type holds, and a small
//! registry for the readers that need rule records.
//!
//! Behind `feature = "testing"`, so a release build holds none of it.

use std::collections::HashMap;
use std::sync::Arc;

use camino::Utf8Path;
use indexmap::{IndexMap, IndexSet};

use crate::config::Config;
use crate::findings::{Finding, Qname, Rel, Sink};
use crate::lang::{
    BuildMode, Language, Listing, Neutral, NeutralModule, NeutralSymbol, Repo, Stack, Timing,
};
use crate::registry::Registry;
use crate::rule::{Posture, RuleRecord, RuleSet, Scope};

pub struct Synthetic {
    pub name: &'static str,
    pub suffix: &'static str,
    pub marker: &'static str,
    pub comment_prefix: &'static str,
}

pub static P: Synthetic = Synthetic {
    name: "p",
    suffix: ".p",
    marker: "P.toml",
    comment_prefix: "#",
};
pub static Q: Synthetic = Synthetic {
    name: "q",
    suffix: ".q",
    marker: "Q.toml",
    comment_prefix: "//",
};

impl Language for Synthetic {
    fn name(&self) -> &'static str {
        self.name
    }

    fn suffix(&self) -> &'static str {
        self.suffix
    }

    fn detect(&self, root: &Utf8Path) -> bool {
        root.join(self.marker).is_file()
    }

    fn build(
        &self,
        _root: &Utf8Path,
        _config: &Config,
        _listing: &Listing,
        _only: Option<&IndexSet<Rel>>,
        _off: &RuleSet,
        _mode: BuildMode,
    ) -> anyhow::Result<Box<dyn Stack>> {
        Ok(Box::new(SyntheticStack::new(self, &[])))
    }
}

pub struct SyntheticStack {
    neutral: Neutral,
}

fn starts_with_t_(rel: &str) -> bool {
    rel.rsplit('/').next().is_some_and(|n| n.starts_with("t_"))
}

impl SyntheticStack {
    /// One module per `(rel, source)`, named `<lang>::<rel without
    /// suffix>::`, with a single `main` symbol over the whole file.
    pub fn new(lang: &Synthetic, files: &[(&str, &str)]) -> Self {
        let mut modules = IndexMap::new();
        let mut module_by_rel = HashMap::new();
        let mut symbols = IndexMap::new();
        let mut doc_files = IndexMap::new();
        for (rel, src) in files {
            let lines: Arc<[Box<str>]> = src.split('\n').map(Box::from).collect();
            let Some(stem) = rel.strip_suffix(lang.suffix) else {
                doc_files.insert(Rel::from(*rel), lines);
                continue;
            };
            let qname = Qname::from(format!("{}::{}", lang.name, stem.replace('/', "::")));
            module_by_rel.insert(Rel::from(*rel), qname.clone());
            symbols.insert(
                Qname::from(format!("{qname}::main")),
                NeutralSymbol {
                    module: qname.clone(),
                    lineno: 1,
                    #[allow(
                        clippy::cast_possible_truncation,
                        reason = "a fixture source, never 4 billion lines"
                    )]
                    end_lineno: lines.len() as u32,
                    kind: "function",
                },
            );
            modules.insert(
                qname.clone(),
                NeutralModule {
                    qname,
                    rel: Rel::from(*rel),
                    lines,
                },
            );
        }
        let cc = symbols.keys().map(|q| (q.clone(), 3)).collect();
        Self {
            neutral: Neutral {
                lang: lang.name,
                suffix: lang.suffix,
                modules,
                module_by_rel,
                symbols,
                doc_files,
                errors: Vec::new(),
                fan_in: HashMap::new(),
                cc,
                is_test: starts_with_t_,
                comment_prefix: lang.comment_prefix,
            },
        }
    }

    /// A test pins a prior or an error the constructor does not build.
    pub const fn neutral_mut(&mut self) -> &mut Neutral {
        &mut self.neutral
    }
}

impl Stack for SyntheticStack {
    fn lang(&self) -> &'static str {
        self.neutral.lang
    }

    fn run_rules(&self, _off: &RuleSet, _sink: &mut Sink, _timing: Timing) {}

    fn neutral(&self) -> &Neutral {
        &self.neutral
    }

    fn notes(&self) -> Vec<String> {
        vec![format!("{}: tree-sitter 9.9", self.neutral.lang)]
    }

    fn provenance(&self) -> serde_json::Value {
        serde_json::json!({ "parser": "9.9" })
    }

    fn fix(&self, _findings: &[Finding]) -> Option<String> {
        None
    }

    fn describe(&self, _findings: &[Finding], _qname: &str) -> Result<String, Vec<String>> {
        Err(Vec::new())
    }

    fn dump(&self, _layer: &str) -> Option<serde_json::Value> {
        None
    }

    fn close(&mut self) {}
}

#[must_use]
pub fn two_language_repo() -> Repo {
    Repo::new(vec![
        Box::new(SyntheticStack::new(
            &P,
            &[("m.p", "x = 1\n"), ("notes.md", "x\n")],
        )),
        Box::new(SyntheticStack::new(
            &Q,
            &[("m.q", "fn main() {}\n"), ("t_x.q", "fn t() {}\n")],
        )),
    ])
}

/// Eight readings: every posture, one id read by two languages, and one id
/// (#3) no round has judged. `(id, slug, family, posture, lang)` are the
/// Python tree's for those readings; render reads no other field. The
/// exception is `99`, an id the registry hands out to no rule: GATE stays in
/// the product for a judged rule to earn and none holds it since #31's
/// burial, so the fixture is the only place render and `gate` meet one.
const RECORDS: &[(&str, &str, &str, Posture, &str)] = &[
    ("1", "weak-boundary-types", "A", Posture::Ratchet, "py"),
    ("3", "contract-implied-guard", "A", Posture::Ratchet, "py"),
    ("6", "dishonest-accessor", "A", Posture::Ratchet, "py"),
    ("11", "structural-clones", "B", Posture::Ratchet, "py"),
    ("11", "structural-clones", "B", Posture::Report, "rs"),
    ("41", "perf-catalog", "P", Posture::Report, "py"),
    ("42", "assertion-free-test", "T", Posture::Ratchet, "py"),
    ("99", "gate-fixture", "C", Posture::Gate, "py"),
];

/// # Panics
///
/// If `RECORDS` above stops being a well-formed registry.
#[must_use]
#[allow(clippy::expect_used, reason = "a fixture built from a const table")]
pub fn registry() -> Registry {
    let records = RECORDS
        .iter()
        .map(|&(id, slug, family, posture, lang)| RuleRecord {
            id,
            slug,
            family,
            engine_class: "AST",
            posture,
            meaning: "what the rule reads",
            goal: "the goal it approximates",
            lang,
            scope: Scope::Repo,
            complement: "",
        })
        .collect();
    Registry::new(records).expect("the fixture registry is well formed")
}
