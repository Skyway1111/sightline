//! The Rust side of the language seam (`lang.py`'s `RS` record, codemap
//! 4.2): the record whole, rules beside the facts and provers they read.

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
use sightline_rs_facts::build::{RsBuilt, build_facts};
use sightline_rs_facts::model::RsFacts;
use sightline_rs_facts::{COMMENT_PREFIX, SUFFIX, crates, dump, is_test_path};
use sightline_rs_provers::oracle::{RsAnswers, build_answers};
use sightline_rs_provers::{RsProvers, layers};

/// The Rust `Language` record. It holds the registry's slug alias map
/// because the `neutral` layer resolves a suppression marker's slug.
#[derive(Default)]
pub struct RsLanguage {
    pub ids_by_slug: HashMap<String, String>,
}

impl Language for RsLanguage {
    fn name(&self) -> &'static str {
        "rs"
    }

    fn suffix(&self) -> &'static str {
        SUFFIX
    }

    fn detect(&self, root: &Utf8Path) -> bool {
        crates::detect(root)
    }

    fn build(
        &self,
        root: &Utf8Path,
        config: &Config,
        listing: &Listing,
        only: Option<&IndexSet<Rel>>,
        off: &RuleSet,
        mode: BuildMode,
    ) -> anyhow::Result<Box<dyn Stack>> {
        let started = std::time::Instant::now();
        let built = build_facts(root, config, listing, only);
        let facts_wall = started.elapsed().as_secs_f64();
        // the toolchain runs only for a rule that reads it; the fast gate
        // (`BuildMode::File`) asks it nothing at all
        let started = std::time::Instant::now();
        let rust = match mode {
            BuildMode::Full => build_answers(root, config, off, built.borrow_dependent()),
            BuildMode::File => RsAnswers::default(),
        };
        let mut stack = RsStack::new(built, rust, self.ids_by_slug.clone());
        stack.walls = vec![
            ("facts".into(), facts_wall),
            ("provers".into(), started.elapsed().as_secs_f64()),
        ];
        Ok(Box::new(stack))
    }
}

pub struct RsStack {
    built: RsBuilt,
    /// the toolchain's answers, taken once at build time
    rust: RsAnswers,
    notes: Vec<String>,
    neutral: Neutral,
    ids_by_slug: HashMap<String, String>,
    /// `profile_audit.py`'s two build stages; empty for a stack a test built.
    pub walls: Vec<(String, f64)>,
}

impl RsStack {
    /// The facts' own notes come first, as `RsProvers.__post_init__` orders
    /// them; `close` appends the toolchain's.
    pub fn new(built: RsBuilt, rust: RsAnswers, ids_by_slug: HashMap<String, String>) -> RsStack {
        let facts = built.borrow_dependent();
        let notes = facts.notes.clone();
        let neutral = neutral_view(facts);
        RsStack {
            built,
            rust,
            notes,
            neutral,
            ids_by_slug,
            walls: Vec::new(),
        }
    }

    pub fn facts(&self) -> &RsFacts<'_> {
        self.built.borrow_dependent()
    }

    /// One pass's provers, memos empty. A memo holding a `Node<'t>` cannot
    /// sit beside the arena in a covariant `self_cell` dependent, so the
    /// stack hands one out per pass: rules, a layer and `describe` each run
    /// as one pass and share every cell inside it.
    pub fn provers(&self) -> RsProvers<'_> {
        RsProvers::new(self.facts(), &self.rust)
    }
}

/// The language-blind view of a Rust build.
fn neutral_view(facts: &RsFacts<'_>) -> Neutral {
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
        lang: "rs",
        suffix: SUFFIX,
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
        fan_in: facts.fan_in.clone(),
        cc: facts.cc.clone(),
        is_test: is_test_path,
        comment_prefix: COMMENT_PREFIX,
    }
}

impl Stack for RsStack {
    fn lang(&self) -> &'static str {
        "rs"
    }

    fn run_rules(&self, off: &RuleSet, sink: &mut Sink, timing: Timing) {
        crate::run_rules(&self.provers(), off, sink, timing);
    }

    fn neutral(&self) -> &Neutral {
        &self.neutral
    }

    fn notes(&self) -> Vec<String> {
        self.notes.clone()
    }

    /// The build's two stages and every toolchain pass the oracle timed (the
    /// `ra_ap` index load among them), in `profile_audit.py`'s labels.
    fn passes(&self) -> Vec<(String, f64)> {
        let mut out = self.walls.clone();
        if let Some(oracle) = &self.rust.oracle {
            out.extend(oracle.passes());
        }
        out
    }

    fn provenance(&self) -> Value {
        self.provers().provenance(self.facts())
    }

    fn fix(&self, findings: &[Finding]) -> Option<String> {
        Some(crate::emit::fix(findings, self.facts(), &self.provers()))
    }

    fn describe(&self, findings: &[Finding], qname: &str) -> Result<String, Vec<String>> {
        crate::describe::describe(self.facts(), &self.provers(), findings, qname)
    }

    fn dump(&self, layer: &str) -> Option<Value> {
        let facts = self.facts();
        match layer {
            "listing" => Some(dump::listing(facts)),
            "neutral" => Some(neutral_layer(&self.neutral, &self.ids_by_slug)),
            "rs-facts" => Some(dump::rs_facts(facts)),
            // the findings of a full run, before suppress, in yield order per
            // rule in registry order
            "raw" => {
                let mut sink = Sink::new();
                self.run_rules(&RuleSet::new(), &mut sink, None);
                Some(json!({
                    "findings": sink.0.iter().map(finding_json).collect::<Vec<_>>(),
                }))
            }
            _ => layers::layer(layer, facts, &self.provers()),
        }
    }

    /// The parser runs in this process; what the toolchain answered reaches
    /// the header here, and its world tree ends here.
    fn close(&mut self) {
        for note in self.rust.close() {
            if !self.notes.contains(&note) {
                self.notes.push(note);
            }
        }
    }
}
