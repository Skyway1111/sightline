//! The language seam: one `Language` per source
//! language sightline reads, the detection that picks the stacks a root
//! runs, the neutral view every stack exposes, and `Repo`, several stacks
//! as one view.
//!
//! Everything past the rules (suppress, rank, render, gate, ratchet) reads
//! a build through `FactsView` and through nothing else.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};

use crate::config::Config;
use crate::findings::{Finding, Qname, Rel, Sink};
use crate::rule::RuleSet;

/// What the shared walk lists: (absolute path, posix path under the root).
/// A stack indexes the entries whose rel spells its own suffix.
pub type Listing = Vec<(Utf8PathBuf, String)>;

/// `profile_audit`'s seam: (rule id, wall) as each rule finishes.
pub type Timing<'a> = Option<&'a mut dyn FnMut(&str, Duration)>;

/// `File`: no oracle and no git, the fast gate's build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Full,
    File,
}

/// How a root shows a language: a manifest marks it, source files alone
/// leave it loose, and a loose language beside a marked one is a stray
/// script, not a second tree to audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Marked,
    Loose,
    Absent,
}

/// One language's stack: how to find it, and how to build it.
pub trait Language: Sync {
    fn name(&self) -> &'static str;
    fn suffix(&self) -> &'static str;
    /// How this root shows the language.
    fn detect(&self, root: &Utf8Path) -> Presence;
    /// # Errors
    ///
    /// A tree the stack cannot build.
    fn build(
        &self,
        root: &Utf8Path,
        config: &Config,
        listing: &Listing,
        only: Option<&IndexSet<Rel>>,
        off: &RuleSet,
        mode: BuildMode,
    ) -> anyhow::Result<Box<dyn Stack>>;
}

/// One language's built facts and provers, with the two verbs that are a
/// per-language printout rather than shared pipeline.
pub trait Stack: Send + Sync {
    fn lang(&self) -> &'static str;
    fn run_rules(&self, off: &RuleSet, sink: &mut Sink, timing: Timing);
    fn neutral(&self) -> &Neutral;
    fn notes(&self) -> Vec<String>;
    fn provenance(&self) -> serde_json::Value;
    /// `None` is a language with no emitter, which the verb reports rather
    /// than skipping silently.
    fn fix(&self, findings: &[Finding]) -> Option<String>;
    /// # Errors
    ///
    /// The candidate qnames a miss should suggest.
    fn describe(&self, findings: &[Finding], qname: &str) -> Result<String, Vec<String>>;
    /// `(label, seconds)` per pass this build measured, for `audit
    /// --profile`. A stack that times nothing answers empty.
    fn passes(&self) -> Vec<(String, f64)> {
        Vec::new()
    }

    /// One layer of this stack's pipeline, for `debug dump`. `None`: this
    /// stack does not answer the layer.
    fn dump(&self, layer: &str) -> Option<serde_json::Value>;
    fn close(&mut self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeutralModule {
    pub qname: Qname,
    pub rel: Rel,
    pub lines: Arc<[Box<str>]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeutralSymbol {
    pub module: Qname,
    pub lineno: u32,
    pub end_lineno: u32,
    pub kind: &'static str,
}

/// One language's facts as the pipeline reads them.
///
/// `is_test` and the suppression comment syntax live here rather than on the
/// `Language` record: the pipeline always holds facts, and a second home
/// would drift.
pub struct Neutral {
    pub lang: &'static str,
    pub suffix: &'static str,
    pub modules: IndexMap<Qname, NeutralModule>,
    pub module_by_rel: HashMap<Rel, Qname>,
    pub symbols: IndexMap<Qname, NeutralSymbol>,
    pub doc_files: IndexMap<Rel, Arc<[Box<str>]>>,
    pub errors: Vec<String>,
    /// module qname -> inbound cross-module refs
    pub fan_in: HashMap<Qname, u32>,
    /// the ranking prior, precomputed for every function symbol
    pub cc: HashMap<Qname, u32>,
    pub is_test: fn(&str) -> bool,
    pub comment_prefix: &'static str,
}

/// What the findings pipeline may read of a build, and all it may read:
/// `Neutral` for one language, `Repo` for several.
pub trait FactsView {
    fn languages(&self) -> &[&'static str];
    fn modules(&self) -> &IndexMap<Qname, NeutralModule>;
    fn module_by_rel(&self) -> &HashMap<Rel, Qname>;
    fn symbols(&self) -> &IndexMap<Qname, NeutralSymbol>;
    fn doc_files(&self) -> &IndexMap<Rel, Arc<[Box<str>]>>;
    fn errors(&self) -> &[String];
    fn fan_in(&self) -> &HashMap<Qname, u32>;
    fn comment_prefix(&self, rel: &str) -> &str;
    fn is_test(&self, rel: &str) -> bool;
    fn cc_prior(&self, qname: &str) -> u32;

    /// The source lines of the module holding this path, where one does.
    fn module_lines(&self, rel: &str) -> Option<&Arc<[Box<str>]>> {
        let qname = self.module_by_rel().get(rel)?;
        Some(&self.modules().get(qname)?.lines)
    }
}

impl FactsView for Neutral {
    fn languages(&self) -> &[&'static str] {
        std::slice::from_ref(&self.lang)
    }

    fn modules(&self) -> &IndexMap<Qname, NeutralModule> {
        &self.modules
    }

    fn module_by_rel(&self) -> &HashMap<Rel, Qname> {
        &self.module_by_rel
    }

    fn symbols(&self) -> &IndexMap<Qname, NeutralSymbol> {
        &self.symbols
    }

    fn doc_files(&self) -> &IndexMap<Rel, Arc<[Box<str>]>> {
        &self.doc_files
    }

    fn errors(&self) -> &[String] {
        &self.errors
    }

    fn fan_in(&self) -> &HashMap<Qname, u32> {
        &self.fan_in
    }

    fn comment_prefix(&self, _rel: &str) -> &str {
        self.comment_prefix
    }

    fn is_test(&self, rel: &str) -> bool {
        (self.is_test)(rel)
    }

    fn cc_prior(&self, qname: &str) -> u32 {
        self.cc.get(qname).copied().unwrap_or(0)
    }
}

/// Every stack's facts as one language-blind view.
///
/// Keys are qnames and rel paths, which no two languages share, so merging
/// is a union in stack order. Facts are immutable after build, so the unions
/// are built once.
pub struct Repo {
    pub stacks: Vec<Box<dyn Stack>>,
    /// what `detect` said of the languages that did not build
    pub notes: Vec<String>,
    languages: Vec<&'static str>,
    modules: IndexMap<Qname, NeutralModule>,
    module_by_rel: HashMap<Rel, Qname>,
    symbols: IndexMap<Qname, NeutralSymbol>,
    doc_files: IndexMap<Rel, Arc<[Box<str>]>>,
    errors: Vec<String>,
    fan_in: HashMap<Qname, u32>,
}

impl Repo {
    #[must_use]
    pub fn new(stacks: Vec<Box<dyn Stack>>) -> Self {
        let mut repo = Self {
            languages: stacks.iter().map(|s| s.lang()).collect(),
            notes: Vec::new(),
            modules: IndexMap::new(),
            module_by_rel: HashMap::new(),
            symbols: IndexMap::new(),
            doc_files: IndexMap::new(),
            errors: Vec::new(),
            fan_in: HashMap::new(),
            stacks,
        };
        for stack in &repo.stacks {
            let n = stack.neutral();
            repo.modules.extend(n.modules.clone());
            repo.module_by_rel.extend(n.module_by_rel.clone());
            repo.symbols.extend(n.symbols.clone());
            repo.doc_files.extend(n.doc_files.clone());
            repo.fan_in
                .extend(n.fan_in.iter().map(|(k, v)| (k.clone(), *v)));
            repo.errors.extend(n.errors.iter().cloned());
        }
        repo
    }

    /// The facts holding this path: the stack that indexed it, else the one
    /// whose suffix it spells, else the first (a doc belongs to none).
    #[must_use]
    #[allow(clippy::indexing_slicing, reason = "`detect` always builds one stack")]
    pub fn owner(&self, rel: &str) -> &Neutral {
        for s in &self.stacks {
            if s.neutral().module_by_rel.contains_key(rel) {
                return s.neutral();
            }
        }
        for s in &self.stacks {
            if rel.ends_with(s.neutral().suffix) {
                return s.neutral();
            }
        }
        self.stacks[0].neutral()
    }
}

impl FactsView for Repo {
    fn languages(&self) -> &[&'static str] {
        &self.languages
    }

    fn modules(&self) -> &IndexMap<Qname, NeutralModule> {
        &self.modules
    }

    fn module_by_rel(&self) -> &HashMap<Rel, Qname> {
        &self.module_by_rel
    }

    fn symbols(&self) -> &IndexMap<Qname, NeutralSymbol> {
        &self.symbols
    }

    fn doc_files(&self) -> &IndexMap<Rel, Arc<[Box<str>]>> {
        &self.doc_files
    }

    fn errors(&self) -> &[String] {
        &self.errors
    }

    fn fan_in(&self) -> &HashMap<Qname, u32> {
        &self.fan_in
    }

    fn comment_prefix(&self, rel: &str) -> &str {
        self.owner(rel).comment_prefix(rel)
    }

    fn is_test(&self, rel: &str) -> bool {
        self.owner(rel).is_test(rel)
    }

    fn cc_prior(&self, qname: &str) -> u32 {
        for s in &self.stacks {
            if s.neutral().symbols.contains_key(qname) {
                return s.neutral().cc_prior(qname);
            }
        }
        0
    }
}

/// The stacks this root runs, in registry order.
///
/// The marked languages, else the loose ones, else the first alone, so an
/// empty tree still reports a header. A loose language beside a marked one
/// is skipped, and the note says so and how to mark it.
pub fn detect<'a>(
    root: &Utf8Path,
    languages: &[&'a dyn Language],
) -> (Vec<&'a dyn Language>, Vec<String>) {
    let found: Vec<(&'a dyn Language, Presence)> =
        languages.iter().map(|l| (*l, l.detect(root))).collect();
    let pick = |want: Presence| -> Vec<&'a dyn Language> {
        found
            .iter()
            .filter(|(_, p)| *p == want)
            .map(|(l, _)| *l)
            .collect()
    };
    let marked = pick(Presence::Marked);
    if marked.is_empty() {
        let loose = pick(Presence::Loose);
        let run = if loose.is_empty() {
            languages.iter().take(1).copied().collect()
        } else {
            loose
        };
        return (run, Vec::new());
    }
    let notes = pick(Presence::Loose)
        .iter()
        .map(|l| {
            format!(
                "{}: loose {} files skipped beside a marked tree; a manifest marks a {} tree to audit",
                l.name(),
                l.suffix(),
                l.name()
            )
        })
        .collect();
    (marked, notes)
}

#[cfg(test)]
mod tests {
    //! The seam proved with two languages that do not exist, `p` and `q`,
    //! which `crate::testing` builds.

    use super::*;
    use crate::testing::{P, Q, SyntheticStack, two_language_repo};

    #[test]
    fn detect_picks_the_stacks_the_root_marks_in_registry_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let registry: &[&dyn Language] = &[&P, &Q];
        let names = |root: &Utf8Path| {
            let (found, notes) = detect(root, registry);
            (found.iter().map(|l| l.name()).collect::<Vec<_>>(), notes)
        };

        // a root that marks none still reports, through the first alone
        assert_eq!(names(root), (vec!["p"], vec![]));
        // loose files run where no manifest marks a tree
        std::fs::write(root.join("tool.q"), "").unwrap();
        assert_eq!(names(root), (vec!["q"], vec![]));
        // a manifest outranks them, and the stray script is named
        std::fs::write(root.join("P.toml"), "").unwrap();
        assert_eq!(
            names(root),
            (
                vec!["p"],
                vec![
                    "q: loose .q files skipped beside a marked tree; a manifest marks a q tree to audit"
                        .to_string()
                ]
            )
        );
        std::fs::write(root.join("Q.toml"), "").unwrap();
        assert_eq!(names(root), (vec!["p", "q"], vec![]));
    }

    #[test]
    fn repo_merges_the_stacks_and_answers_by_owner() {
        let repo = two_language_repo();

        assert_eq!(repo.languages(), ["p", "q"]);
        let mut rels: Vec<&str> = repo.module_by_rel().keys().map(|r| &**r).collect();
        rels.sort_unstable();
        assert_eq!(rels, ["m.p", "m.q", "t_x.q"]);
        assert_eq!(repo.owner("m.p").lang, "p");
        assert_eq!(repo.owner("m.q").lang, "q");
        // a doc belongs to no language: the first stack owns it
        assert_eq!(repo.owner("notes.md").lang, "p");
        // a path no stack indexed, but whose suffix one spells
        assert_eq!(repo.owner("gone.q").lang, "q");
        assert!(repo.errors().is_empty());
        assert_eq!(repo.cc_prior("no::such::symbol"), 0);
    }

    #[test]
    fn suppression_and_test_paths_read_the_owning_language() {
        let repo = two_language_repo();

        assert_eq!(repo.comment_prefix("m.q"), "//");
        assert_eq!(repo.comment_prefix("m.p"), "#");
        assert!(repo.is_test("t_x.q"));
        assert!(!repo.is_test("m.q"));
        assert_eq!(repo.cc_prior("q::m::main"), 3);
    }

    #[test]
    fn a_single_stack_repo_is_the_facts_it_holds() {
        let stack = SyntheticStack::new(&Q, &[("m.q", "fn main() {}\n")]);
        let neutral_modules = stack.neutral().modules.clone();
        let repo = Repo::new(vec![Box::new(stack)]);

        assert_eq!(repo.languages(), ["q"]);
        assert_eq!(repo.modules(), &neutral_modules);
        assert_eq!(repo.owner("m.q").lang, "q");
        assert_eq!(repo.comment_prefix("m.q"), "//");
    }

    #[test]
    fn module_lines_reach_the_owning_module() {
        let repo = two_language_repo();
        assert_eq!(
            &**repo.module_lines("m.q").unwrap(),
            ["fn main() {}".into(), "".into()]
        );
        assert!(repo.module_lines("notes.md").is_none());
    }
}
