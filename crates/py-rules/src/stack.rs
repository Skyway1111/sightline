//! The Python side of the language seam (`lang.py`'s `PY` record, codemap
//! 4.2): facts, provers and rules side by side.

use std::collections::HashMap;
use std::sync::Arc;

use camino::Utf8Path;
use indexmap::{IndexMap, IndexSet};
use serde_json::{Value, json};

use sightline_core::config::Config;
use sightline_core::findings::{Finding, Qname, Rel, Sink, finding_json};
use sightline_core::lang::{
    BuildMode, Language, Listing, Neutral, NeutralModule, NeutralSymbol, Stack, Timing,
    neutral_layer,
};
use sightline_core::rule::RuleSet;
use sightline_core::walk;
use sightline_py_facts::build::{PyBuilt, build_facts};
use sightline_py_facts::dump;
use sightline_py_facts::model::{RepoFacts, is_test_path};
use sightline_py_provers::Provers;
use sightline_py_provers::layers;

/// The Python `Language` record. It holds the registry's slug alias map
/// because the `neutral` layer resolves a suppression marker's slug, and
/// the binary builds one registry over every language's readings.
pub struct PyLanguage {
    pub ids_by_slug: HashMap<String, String>,
}

impl PyLanguage {
    pub fn new(ids_by_slug: HashMap<String, String>) -> PyLanguage {
        PyLanguage { ids_by_slug }
    }
}

impl Default for PyLanguage {
    fn default() -> PyLanguage {
        PyLanguage::new(HashMap::new())
    }
}

impl Language for PyLanguage {
    fn name(&self) -> &'static str {
        "py"
    }

    fn suffix(&self) -> &'static str {
        ".py"
    }

    fn detect(&self, root: &Utf8Path) -> bool {
        root.join("pyproject.toml").is_file()
            || root.join("setup.py").is_file()
            || walk::any_name(root, |n| n.ends_with(".py"))
    }

    fn build(
        &self,
        root: &Utf8Path,
        config: &Config,
        listing: &Listing,
        only: Option<&IndexSet<Rel>>,
        _off: &RuleSet,
        mode: BuildMode,
    ) -> anyhow::Result<Box<dyn Stack>> {
        rayon::spawn(crate::trust::warm);
        let started = std::time::Instant::now();
        let built = build_facts(root, config, listing, only);
        let facts_wall = started.elapsed().as_secs_f64();
        let started = std::time::Instant::now();
        // `lang.py:PY.file_provers`: the fast gate reads per-facts caches
        // alone, no checker and no git history
        let provers = match mode {
            BuildMode::Full => Provers::new(root, config, built.borrow_dependent(), true),
            BuildMode::File => Provers::bare(built.borrow_dependent()),
        };
        let mut stack = PyStack::new(built, provers, self.ids_by_slug.clone());
        stack.walls = vec![
            ("facts".into(), facts_wall),
            ("provers".into(), started.elapsed().as_secs_f64()),
        ];
        Ok(Box::new(stack))
    }
}

pub struct PyStack {
    built: PyBuilt,
    pub provers: Provers,
    neutral: Neutral,
    ids_by_slug: HashMap<String, String>,
    /// `profile_audit.py`'s two build stages; empty for a stack a test built.
    pub walls: Vec<(String, f64)>,
}

impl PyStack {
    pub fn new(built: PyBuilt, provers: Provers, ids_by_slug: HashMap<String, String>) -> PyStack {
        let neutral = neutral_view(built.borrow_dependent());
        PyStack {
            built,
            provers,
            neutral,
            ids_by_slug,
            walls: Vec::new(),
        }
    }

    pub fn facts(&self) -> &RepoFacts<'_> {
        self.built.borrow_dependent()
    }
}

/// The language-blind view of a Python build.
fn neutral_view(facts: &RepoFacts<'_>) -> Neutral {
    let mut modules: IndexMap<Qname, NeutralModule> = IndexMap::new();
    let mut module_by_rel: HashMap<Rel, Qname> = HashMap::new();
    for m in facts.modules.values() {
        module_by_rel.insert(m.rel.clone(), m.qname.clone());
        modules.insert(
            m.qname.clone(),
            NeutralModule {
                qname: m.qname.clone(),
                rel: m.rel.clone(),
                lines: m.lines.iter().map(|l| (*l).into()).collect(),
            },
        );
    }
    Neutral {
        lang: "py",
        suffix: ".py",
        modules,
        module_by_rel,
        symbols: facts
            .symbols
            .iter()
            .map(|(q, s)| {
                (
                    q.clone(),
                    NeutralSymbol {
                        module: s.module.clone(),
                        lineno: s.lineno,
                        end_lineno: s.end_lineno,
                        kind: s.kind,
                    },
                )
            })
            .collect(),
        doc_files: facts
            .doc_files
            .iter()
            .map(|(rel, lines)| {
                let lines: Arc<[Box<str>]> =
                    lines.iter().map(|l| l.as_str().into()).collect::<Arc<_>>();
                (rel.clone(), lines)
            })
            .collect(),
        errors: facts.errors.clone(),
        fan_in: facts.fan_in(),
        cc: facts.cc.clone(),
        is_test: is_test_path,
        comment_prefix: "#",
    }
}

impl Stack for PyStack {
    fn lang(&self) -> &'static str {
        "py"
    }

    fn run_rules(&self, off: &RuleSet, sink: &mut Sink, timing: Timing) {
        crate::run_rules(self.facts(), &self.provers, off, sink, timing);
    }

    fn neutral(&self) -> &Neutral {
        &self.neutral
    }

    /// The notes a whole audit produces: every note-producing accessor is
    /// forced first, as the rules force them, so a layer dump holds the
    /// same `notes` whichever layer it asked for.
    fn notes(&self) -> Vec<String> {
        let facts = self.facts();
        self.provers.calls(facts);
        self.provers.import_effects(facts);
        self.provers.hot(facts);
        self.provers.notes()
    }

    /// The build's two stages and every oracle pass, in the labels
    /// `profile_audit.py` writes.
    fn passes(&self) -> Vec<(String, f64)> {
        let mut out = self.walls.clone();
        out.extend(self.provers.oracle_passes());
        out
    }

    /// `modules` and `parse_errors` are derived by `core::render` from the
    /// neutral view; this is the provers' own block.
    fn provenance(&self) -> Value {
        self.provers.provenance(self.facts())
    }

    /// `lang.py:_py_fix`: one world pass over these findings, then the diff.
    fn fix(&self, findings: &[Finding]) -> Option<String> {
        let facts = self.facts();
        let patched = crate::emit::attach_fixes(findings.to_vec(), facts, &self.provers);
        Some(crate::emit::unified_diff(&patched, facts))
    }

    fn describe(&self, findings: &[Finding], qname: &str) -> Result<String, Vec<String>> {
        crate::describe::describe(self.facts(), &self.provers, findings, qname)
    }

    fn dump(&self, layer: &str) -> Option<Value> {
        let facts = self.facts();
        match layer {
            "listing" => Some(dump::listing(facts)),
            "facts" => Some(dump::facts(facts)),
            "traversal" => Some(dump::traversal(facts)),
            "neutral" => Some(neutral_layer(&self.neutral, &self.ids_by_slug)),
            // the findings of a full run, before suppress, in yield order per
            // rule in registry order
            "raw" => {
                let mut sink = Sink::new();
                self.run_rules(&RuleSet::new(), &mut sink, None);
                Some(json!({
                    "findings": sink.0.iter().map(finding_json).collect::<Vec<_>>(),
                }))
            }
            // the verify layer prints the passes the audit itself made, so
            // the rules run first
            "verify" => {
                self.run_rules(&RuleSet::new(), &mut Sink::new(), None);
                layers::layer(layer, facts, &self.provers)
            }
            _ => layers::layer(layer, facts, &self.provers),
        }
    }

    fn close(&mut self) {
        self.provers.close();
    }
}
