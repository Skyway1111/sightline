//! All of sightline's Python analysis: the machinery rules read, over
//! `py-facts`. One module per prover family, plus `oracle.rs`, the only code
//! in the workspace that talks to the ty checker. `Provers` is the container:
//! one per build, every product memoized in a `OnceLock` cell (R20), every
//! accessor taking the facts it reads.
//!
//! file-length-ok: the crate's facade. The `Provers` memo table and its
//! accessors live at the root; each prover family already has its own
//! module, so a split by size would cut through the container.

pub mod annotations;
pub mod argtypes;
pub mod callgraph;
pub mod catalog;
pub mod clones;
pub mod closed_world;
pub mod comments;
pub mod counterfactual;
pub mod dump;
pub mod effects;
pub mod grounding;
pub mod handlers;
pub mod hotness;
pub mod import_effects;
pub mod imports;
pub mod layers;
pub mod liveness;
pub mod oracle;
pub mod records;
pub mod rettypes;
pub mod scope;
pub mod shipping;
pub mod spend;
pub mod typestrings;

use std::collections::{BTreeSet, HashSet};
use std::sync::{Mutex, OnceLock};

use camino::Utf8Path;
use indexmap::IndexMap;
use serde_json::{Value, json};

use sightline_core::config::Config;
use sightline_core::findings::{Evidence, Fix, Qname, Rel};
use sightline_core::git::GitAges;
use sightline_py_facts::model::RepoFacts;
use sightline_py_facts::module::Module;

use crate::argtypes::ArgTypes;
use crate::callgraph::CallGraph;
use crate::closed_world::ClosedWorld;
use crate::counterfactual::{Outcome, Proposal, Splice, evidence_of};
use crate::effects::Effects;
use crate::hotness::HotSet;
use crate::import_effects::ReceiverTypes;
use crate::imports::ImportGraph;
use crate::liveness::{Live, Reexports, Unseen};
use crate::oracle::{Oracle, OracleDiag, UnresolvedImports, WARNING_VERDICTS};
use crate::records::Records;
use crate::rettypes::RetTypes;
use crate::scope::Scope;

/// What an absent oracle costs, named in the provenance header: no degraded
/// run may report anything the run it degrades from does not.
pub const NO_ORACLE_NOTE: &str = "rules #2/#5/#10/#58 silent, #36 loses its Any-laundering arm \
     and #40 its inferred-return arm";

/// A provenance note's producer. Each keeps its notes in its own cell and
/// the header reads the cells in this order, so no note is
/// pushed onto a shared list by whichever rule's thread touched the lazy
/// accessor first. The `audit` layer pins the order.
#[derive(Clone, Copy)]
pub enum Producer {
    Build,
    Git,
    Lossy,
    Calls,
    RecvTypes,
    Hot,
    Oracle,
}

const PRODUCERS: usize = 7;

impl Producer {
    /// What the product loses without the oracle, for the producers whose
    /// loss is one fixed sentence.
    fn without_oracle(self) -> Option<&'static str> {
        match self {
            Producer::Calls => Some(
                "no oracle: a plain receiver's call resolves by method name alone \
                 (effects, hotness, #37 and #48 inherit it)",
            ),
            Producer::RecvTypes => Some(
                "no oracle: an import-time method call on a module global or on a \
                 call's result stays import-time work (#32 and #35 move fewer imports)",
            ),
            _ => None,
        }
    }
}

/// One `verify_splice` call: the proposals it placed and their verdicts.
pub type SplicePass = (Vec<Proposal>, IndexMap<String, Outcome>);

/// Prover container passed to rule fns. One instance serves one
/// `RepoFacts` build; every accessor takes that build and memoizes its
/// product, so the cells hold indices and qnames, never a borrow of the
/// arena (the stack owns both side by side, no second `self_cell`).
pub struct Provers {
    /// the in-process checker, `Some` when config asks for one and it
    /// started; `oracle()` is the working one (`no_oracle` after a crash)
    pub oracle: Option<Oracle>,
    /// `None` without usable history
    pub git_ages: Option<GitAges>,
    notes: [OnceLock<Vec<String>>; PRODUCERS],
    /// by `SymbolId` (the index into `facts.symbols`), R20
    scopes: Vec<OnceLock<Option<Scope>>>,
    pub calls: OnceLock<CallGraph>,
    pub closed_world: OnceLock<ClosedWorld>,
    pub effects: OnceLock<IndexMap<Qname, Effects>>,
    pub hot: OnceLock<HotSet>,
    pub import_graph: OnceLock<ImportGraph>,
    pub recv_types: OnceLock<ReceiverTypes>,
    pub import_effects: OnceLock<BTreeSet<Qname>>,
    pub shipped: OnceLock<Vec<BTreeSet<Qname>>>,
    pub unseen: OnceLock<Unseen>,
    pub live: OnceLock<Live>,
    pub reexports: OnceLock<Reexports>,
    pub records: OnceLock<Records>,
    pub arg_types: OnceLock<ArgTypes>,
    pub ret_types: OnceLock<RetTypes>,
    pub unresolved: OnceLock<UnresolvedImports>,
    /// Every `verify_splice` call's placed proposals and their verdicts, in
    /// call order. The `verify` dump layer prints this log.
    splice_log: Mutex<Vec<SplicePass>>,
}

impl Provers {
    /// `build_provers` and `wire_oracle_queries`' lossy note: the provers for
    /// one audit. Costs the checker's construction when config asks for one
    /// (`Oracle::new`) and a git history read (`GitAges`) when `with_git`; the
    /// fast gate (`BuildMode::File`) skips the latter.
    pub fn new(root: &Utf8Path, config: &Config, facts: &RepoFacts<'_>, with_git: bool) -> Provers {
        let mut provers = Provers::bare(facts);
        if config.oracle {
            let python_exe = oracle::detect_python_env(root, config.python_env.as_deref());
            if python_exe.is_none() {
                provers.note(
                    Producer::Build,
                    vec![
                        "python-env not resolved: imports resolve against the \
                          audit environment, not the target's"
                            .to_string(),
                    ],
                );
            }
            provers.oracle = Oracle::new(
                root,
                &config.excludes,
                &facts.import_roots,
                python_exe.as_deref(),
            )
            .ok();
        } else {
            provers.note(
                Producer::Build,
                vec![format!("oracle disabled by config: {NO_ORACLE_NOTE}")],
            );
        }
        let lossy: Vec<&str> = {
            let mut rels: Vec<&str> = facts
                .modules
                .values()
                .filter(|m| m.lossy)
                .map(|m| &*m.rel)
                .collect();
            rels.sort();
            rels
        };
        if !lossy.is_empty() {
            provers.note(
                Producer::Lossy,
                vec![format!(
                    "non-UTF-8 bytes decoded lossily (no oracle span queries or fixes there): {}",
                    lossy.join(", ")
                )],
            );
        }
        if with_git {
            let git = GitAges::new(root);
            if git.available() {
                provers.git_ages = Some(git);
            } else {
                provers.note(
                    Producer::Git,
                    vec!["no usable git history: clone ranking is count-only".to_string()],
                );
            }
        }
        provers
    }

    /// `Provers()`: the container with nothing read yet (the test fixtures').
    pub fn bare(facts: &RepoFacts<'_>) -> Provers {
        Provers {
            oracle: None,
            git_ages: None,
            notes: Default::default(),
            scopes: (0..facts.symbols.len()).map(|_| OnceLock::new()).collect(),
            calls: OnceLock::new(),
            closed_world: OnceLock::new(),
            effects: OnceLock::new(),
            hot: OnceLock::new(),
            import_graph: OnceLock::new(),
            recv_types: OnceLock::new(),
            import_effects: OnceLock::new(),
            shipped: OnceLock::new(),
            unseen: OnceLock::new(),
            live: OnceLock::new(),
            reexports: OnceLock::new(),
            records: OnceLock::new(),
            arg_types: OnceLock::new(),
            ret_types: OnceLock::new(),
            unresolved: OnceLock::new(),
            splice_log: Mutex::default(),
        }
    }

    /// A producer's notes, set once when its product is built. A second
    /// call for the same producer keeps the first list.
    pub fn note(&self, producer: Producer, notes: Vec<String>) {
        let _ = self.notes[producer as usize].set(notes);
    }

    /// Every note in producer order, each once.
    pub fn notes(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for cell in &self.notes {
            for note in cell.get().into_iter().flatten() {
                if !out.contains(note) {
                    out.push(note.clone());
                }
            }
        }
        out
    }

    /// A product built once, and what it loses without the oracle
    /// (`Producer::without_oracle`), noted once by its producer.
    fn degraded<'a, T>(
        &'a self,
        cell: &'a OnceLock<T>,
        producer: Producer,
        build: impl FnOnce() -> T,
    ) -> &'a T {
        cell.get_or_init(|| {
            if self.no_oracle()
                && let Some(note) = producer.without_oracle()
            {
                self.note(producer, vec![note.to_string()]);
            }
            build()
        })
    }

    /// Every memo built, on the calling thread, before rules run under
    /// rayon. An initializer that itself runs rayon jobs (`effects`, `hot`,
    /// the oracle's passes) lets its worker steal a queued rule closure
    /// while it waits, and that closure re-enters the same `OnceLock` on the
    /// same thread: a deadlock, seen the first time 44 rules ran at once.
    pub fn warm(&self, facts: &RepoFacts<'_>) {
        self.calls(facts);
        self.closed_world(facts);
        self.effects(facts);
        self.hot(facts);
        self.import_graph(facts);
        self.recv_types(facts);
        self.import_effects(facts);
        self.shipped_subsets(facts);
        self.unseen(facts);
        self.live(facts);
        self.reexports(facts);
        self.records(facts);
        self.arg_types(facts);
        self.ret_types(facts);
        self.unresolved(facts);
    }

    /// The working oracle: `None` when config turned it off, it never
    /// started, or it crashed under a pass.
    pub fn oracle(&self) -> Option<&Oracle> {
        self.oracle.as_ref().filter(|o| o.failure().is_none())
    }

    /// `(label, seconds)` per oracle pass, in pass order: `audit --profile`
    /// reads them after `close`, which the working-oracle filter would hide.
    pub fn oracle_passes(&self) -> Vec<(String, f64)> {
        self.oracle.as_ref().map(Oracle::passes).unwrap_or_default()
    }

    /// One reading: the rules the oracle serves go silent, the header says
    /// so.
    pub fn no_oracle(&self) -> bool {
        self.oracle().is_none()
    }

    /// The verbs' last call: release the oracle and carry what it reported
    /// into the header.
    pub fn close(&mut self) {
        let Some(oracle) = &self.oracle else {
            return;
        };
        oracle.close();
        if let Some(failure) = oracle.failure() {
            self.note(
                Producer::Oracle,
                vec![format!(
                    "oracle crashed mid-run ({failure}); from that pass on {NO_ORACLE_NOTE}"
                )],
            );
        }
    }

    // Oracle-backed accessors answer empty without an oracle rather than
    // None: the degradation is the provenance note's to report, not every
    // reader's.

    /// The oracle's reportUnnecessary* diagnostics (#2's input), each paired
    /// with the module this build parsed for it.
    pub fn diagnostics<'f, 't>(
        &self,
        facts: &'f RepoFacts<'t>,
    ) -> Vec<(&'f Module<'t>, &OracleDiag)> {
        let Some(oracle) = self.oracle() else {
            return Vec::new();
        };
        oracle
            .unnecessary()
            .into_iter()
            .filter_map(|d| Some((facts.module_by_rel(&d.rel)?, d)))
            .collect()
    }

    /// The checker's own verdicts on the repo's code (#58's input), each
    /// paired with the module this build parsed for it: every error-severity
    /// diagnostic, plus the ones held at warning severity so they cannot veto
    /// a splice (`WARNING_VERDICTS`). A module with an unresolved import is
    /// dropped whole: the missing module explains its errors, and the header
    /// already counts it.
    pub fn errors<'f, 't>(&self, facts: &'f RepoFacts<'t>) -> Vec<(&'f Module<'t>, &OracleDiag)> {
        let Some(oracle) = self.oracle() else {
            return Vec::new();
        };
        let diags = oracle.diagnostics();
        let blind: HashSet<&Rel> = diags
            .iter()
            .filter(|d| d.rule == "reportMissingImports")
            .map(|d| &d.rel)
            .collect();
        diags
            .iter()
            .filter(|d| {
                (d.severity == "error" || WARNING_VERDICTS.contains(&d.rule.as_str()))
                    && !blind.contains(&d.rel)
            })
            .filter_map(|d| Some((facts.module_by_rel(&d.rel)?, d)))
            .collect()
    }

    /// `(rel, line)` of every binding the checker rejected
    /// (`invalid-assignment`, `invalid-parameter-default`): a #2 verdict on a
    /// name such an assignment rebinds rests on the declaration it broke, not
    /// on the value.
    pub fn rejected_bindings(&self) -> HashSet<(Rel, u32)> {
        let Some(oracle) = self.oracle() else {
            return HashSet::new();
        };
        oracle
            .diagnostics()
            .iter()
            .filter(|d| {
                d.severity == "error"
                    && matches!(
                        d.rule.as_str(),
                        "invalid-assignment" | "invalid-parameter-default"
                    )
            })
            .map(|d| (d.rel.clone(), d.line))
            .collect()
    }

    /// Oracle-established argument types at every closed-world call site
    /// (#5, #2's grounding, #14).
    pub fn arg_types(&self, facts: &RepoFacts<'_>) -> &ArgTypes {
        self.arg_types.get_or_init(|| ArgTypes::new(facts, self))
    }

    /// Oracle-revealed return types of return-unannotated functions (#36, #40).
    // sightline-ok: 11 - two callers of one memo helper, off the oracle
    pub fn ret_types(&self, facts: &RepoFacts<'_>) -> &RetTypes {
        self.ret_types
            .get_or_init(|| RetTypes::new(facts, self.oracle()))
    }

    /// How many imports the oracle could not resolve, per missing module.
    // sightline-ok: 11 - two callers of one memo helper, off the oracle
    pub fn unresolved(&self, facts: &RepoFacts<'_>) -> &UnresolvedImports {
        self.unresolved
            .get_or_init(|| UnresolvedImports::new(facts, self.oracle()))
    }

    /// One counterfactual pass over a batch of proposed splices (#5's lifts,
    /// #10's widenings, and every fix the `fix` verb emits): each splice no
    /// watched file errored under, mapped to what its world proved and the
    /// exact verified edits. A vetoed splice, and one whose spelling the file
    /// cannot import, is absent.
    pub fn verify_splice(
        &self,
        facts: &RepoFacts<'_>,
        splices: &[Splice],
    ) -> IndexMap<String, (Evidence, Fix)> {
        let Some(oracle) = self.oracle() else {
            return IndexMap::new();
        };
        let calls = self.calls(facts);
        let mut proposals: Vec<Proposal> = splices
            .iter()
            .filter_map(|s| counterfactual::placed(facts, calls, oracle, s))
            .collect();
        // `worlds::split` cuts groups positionally, so the order proposals
        // arrive in is the order verdicts group under: the ids, never the
        // walk, which Windows and Unix spell differently.
        proposals.sort_by(|a, b| a.id.cmp(&b.id));
        let outcomes = counterfactual::verify(facts, &proposals, oracle);
        self.splice_log
            .lock()
            .expect("the splice log")
            .push((proposals.clone(), outcomes.clone()));
        if self.no_oracle() {
            // the checker crashed under this pass: nothing was verified
            return IndexMap::new();
        }
        proposals
            .iter()
            .filter_map(|p| {
                let outcome = outcomes.get(&p.id).unwrap_or(&Outcome::Clean);
                (*outcome != Outcome::Veto).then(|| (p.id.clone(), (evidence_of(outcome), p.fix())))
            })
            .collect()
    }

    /// The `verify` layer's rows: every splice a pass judged, with the index
    /// of the pass that judged it.
    pub fn splice_passes(&self) -> Vec<SplicePass> {
        self.splice_log.lock().expect("the splice log").clone()
    }

    /// `scope_of`: a function's own view of itself, built once per symbol.
    pub fn scope_of(&self, facts: &RepoFacts<'_>, qname: &str) -> Option<&Scope> {
        let index = facts.symbols.get_index_of(qname)?;
        self.scopes[index]
            .get_or_init(|| Scope::new(facts, qname))
            .as_ref()
    }

    /// facts' CHA resolution, re-judged by the oracle wherever CHA had no
    /// typed evidence (`callgraph`); by method name alone without one.
    // sightline-ok: 11 - two callers of one memo helper, degraded
    pub fn calls(&self, facts: &RepoFacts<'_>) -> &CallGraph {
        self.degraded(&self.calls, Producer::Calls, || {
            callgraph::judged_call_graph(facts, self.oracle().map(Oracle::call_edges))
        })
    }

    pub fn closed_world(&self, facts: &RepoFacts<'_>) -> &ClosedWorld {
        self.closed_world
            .get_or_init(|| ClosedWorld::build(facts, self.calls(facts)))
    }

    pub fn effects(&self, facts: &RepoFacts<'_>) -> &IndexMap<Qname, Effects> {
        self.effects.get_or_init(|| effects::summaries(facts, self))
    }

    pub fn hot(&self, facts: &RepoFacts<'_>) -> &HotSet {
        self.hot.get_or_init(|| {
            let hot = hotness::hot_reachable(facts, self.calls(facts), self.no_oracle());
            let mut notes: Vec<String> = hot
                .missing_roots
                .iter()
                .map(|name| format!("hot-root not found: {name}"))
                .collect();
            if hot.roots.is_empty() {
                notes.push(
                    "family P silent: no hot-roots config and no cost-declaring docstrings"
                        .to_string(),
                );
            }
            self.note(Producer::Hot, notes);
            hot
        })
    }

    /// The internal import graph, built once (#9, #35 and the effects set
    /// read it).
    pub fn import_graph(&self, facts: &RepoFacts<'_>) -> &ImportGraph {
        self.import_graph
            .get_or_init(|| imports::import_graph(facts))
    }

    /// The class an import-time method call's receiver holds.
    // sightline-ok: 11 - two callers of one memo helper, degraded
    pub fn recv_types(&self, facts: &RepoFacts<'_>) -> &ReceiverTypes {
        self.degraded(&self.recv_types, Producer::RecvTypes, || {
            ReceiverTypes::new(facts, self.oracle())
        })
    }

    /// Modules whose import runs something: no emitter moves one.
    pub fn import_effects(&self, facts: &RepoFacts<'_>) -> &BTreeSet<Qname> {
        self.import_effects.get_or_init(|| {
            let graph = self.import_graph(facts);
            let receivers = self.recv_types(facts);
            import_effects::import_time_effects(facts, graph, receivers)
        })
    }

    /// Module sets the repo copies as a unit: no hoist may grow one.
    pub fn shipped_subsets(&self, facts: &RepoFacts<'_>) -> &[BTreeSet<Qname>] {
        self.shipped
            .get_or_init(|| shipping::shipped_subsets(facts))
    }

    /// Names reached with no reference the index resolves (liveness).
    pub fn unseen(&self, facts: &RepoFacts<'_>) -> &Unseen {
        self.unseen.get_or_init(|| liveness::unseen_names(facts))
    }

    /// Name-level liveness: live names by scope and the reflection patterns
    /// (#32 and #56 read one index).
    pub fn live(&self, facts: &RepoFacts<'_>) -> &Live {
        self.live.get_or_init(|| liveness::live_names(facts))
    }

    /// Per module, the names its `from M import *` readers load.
    pub fn reexports(&self, facts: &RepoFacts<'_>) -> &Reexports {
        self.reexports
            .get_or_init(|| liveness::star_reexports(facts))
    }

    /// Closed record producers and where their results flow (#57).
    pub fn records(&self, facts: &RepoFacts<'_>) -> &Records {
        self.records
            .get_or_init(|| records::build_records(facts, self))
    }

    /// What this machinery reports in the audit header, read after `close`,
    /// so a checker that crashed counts as no oracle.
    pub fn provenance(&self, facts: &RepoFacts<'_>) -> Value {
        let mut block = json!({ "enabled": !self.no_oracle() });
        if let Some(oracle) = self.oracle() {
            let unresolved = self.unresolved(facts);
            let n = unresolved.count();
            let density = n as f64 / facts.modules.len().max(1) as f64;
            block["unresolved_imports"] = json!(n);
            block["unresolved_import_density"] = json!((density * 10_000.0).round() / 10_000.0);
            block["unresolved_modules"] = json!(unresolved.modules);
            block["calls_resolved_by_oracle"] = json!(self.calls(facts).upgraded);
            block["build"] = json!(oracle.build());
        }
        json!({ "oracle": block })
    }
}
