//! The Rust oracle (`rs/oracle.py`): cargo's members and verdicts
//! (`cargo.rs`), rust-analyzer's resolution in process (`index.rs`) and the
//! worlds (`worlds.rs`). The one place in the Rust stack that runs a
//! toolchain; `RsAnswers` is what every Rust reading asks of it, with empty
//! answers where the oracle is off or a pass of it stopped, so a degraded
//! run reports a subset of its twin.
//!
//! `project_roots` is what the tools are pointed at: the audited root where
//! it holds a manifest, else every crate root under it. Every answer is
//! re-rooted to the audited root, and a root whose tool stops degrades that
//! root alone.

pub mod cargo;
pub mod index;
pub mod worlds;

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Instant;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use sightline_core::config::Config;
use sightline_core::progress::progress;
use sightline_core::rule::RuleSet;
use sightline_rs_facts::MANIFEST;
use sightline_rs_facts::model::RsFacts;

use crate::oracle::index::RsGraph;

/// The readings that need the toolchain: the set a `rules-off` must cover
/// before an audit skips the index and the check, and what an absent oracle
/// costs, named in the provenance header.
pub const ORACLE_RULES: [&str; 4] = ["32", "48", "56", "59"];
pub const NO_ORACLE_NOTE: &str = "rules #32/#48/#56/#59 silent and no Rust fix is verified";
/// the feature set every check and every world runs
pub const FEATURES: &str = "default";
/// target kinds whose failure leaves a member unchecked
pub const SURFACE: [&str; 2] = ["lib", "bin"];
/// the harness points a worktree run at the live root's build directory
pub const TARGET_ENV: &str = "SIGHTLINE_CARGO_TARGET";

/// One workspace member: what `cargo metadata --no-deps` says of it. `name`
/// is as Cargo spells it, `dir` the posix rel of its manifest's directory,
/// `kind` "lib", "bin" or "" for a member with neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsMember {
    pub name: String,
    pub dir: String,
    pub kind: String,
}

/// One `compiler-message`. `rel` is empty for a message with no span
/// (cargo's own "could not compile"), `crate_name` the target it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsDiag {
    pub rel: String,
    pub line: u32,
    pub col: u32,
    pub code: String,
    pub level: String,
    pub message: String,
    pub crate_name: String,
}

impl RsDiag {
    /// A world's diff key: an overlay preserves line numbers.
    pub fn key(&self) -> (String, u32, String, String) {
        (
            self.rel.clone(),
            self.line,
            self.code.clone(),
            self.message.clone(),
        )
    }
}

/// `_down`, `notes` and `faults`: what the passes write, behind one lock so
/// a pass reads `&RsOracle` (the provers hold `&RsAnswers`).
#[derive(Default)]
struct State {
    /// project root -> the pass that stopped
    down: IndexMap<Utf8PathBuf, String>,
    /// degraded modes, for the header
    notes: Vec<String>,
    /// the index answered in terms we cannot read
    faults: Vec<String>,
    /// `(label, seconds)` per pass: what `audit --profile` attributes
    passes: Vec<(String, f64)>,
}

/// The toolchain pointed at one tree.
pub struct RsOracle {
    /// the audited root, absolute, native separators
    pub root: Utf8PathBuf,
    /// the config's excludes, stripped of `/`: a world copies none of them
    pub excludes: BTreeSet<String>,
    /// the directories the tools are pointed at
    pub roots: Vec<Utf8PathBuf>,
    /// one build directory outside every tree, and one per project root
    pub target: Utf8PathBuf,
    pub targets: IndexMap<Utf8PathBuf, Utf8PathBuf>,
    state: Mutex<State>,
    pub cargo: cargo::Cargo,
    pub worlds: worlds::Worlds,
}

impl RsOracle {
    /// `crates` is `facts.crates`; `target_base` is `TARGET_ENV`'s value or
    /// a test's own directory, `None` for the per-root dir under the cache.
    pub fn new(
        root: &Utf8Path,
        config: &Config,
        crates: &IndexMap<String, String>,
        target_base: Option<&Utf8Path>,
    ) -> RsOracle {
        let root = absolute(root);
        let roots = project_roots(&root, crates);
        let dir = |p: &Utf8PathBuf| target_dir(&root, &home_of(&root, p), target_base);
        let targets = roots.iter().map(|p| (p.clone(), dir(p))).collect();
        RsOracle {
            target: target_dir(&root, "", target_base),
            excludes: config
                .excludes
                .iter()
                .map(|e| e.trim_matches('/').to_string())
                .collect(),
            root,
            roots,
            targets,
            state: Mutex::new(State::default()),
            cargo: cargo::Cargo::new(std::env::var_os("PATH").as_deref()),
            worlds: worlds::Worlds::default(),
        }
    }

    /// A project root as a rel of the audited root; empty for the root.
    pub fn home(&self, project: &Utf8Path) -> String {
        home_of(&self.root, project)
    }

    /// The innermost project root holding a directory, None past them all.
    pub fn project_of(&self, path: &Utf8Path) -> Option<&Utf8PathBuf> {
        let found = absolute(path);
        let mut roots: Vec<&Utf8PathBuf> = self.roots.iter().collect();
        roots.sort_by_key(|p| std::cmp::Reverse(p.as_str().len()));
        roots
            .into_iter()
            .find(|p| found == **p || found.starts_with(p))
    }

    /// A pass that stopped: at one project root, or at every one where no
    /// root is named. The roots still up answer on, and a tree with more
    /// than one says in the header which of them went silent.
    pub fn fail(&self, at: Option<&Utf8Path>, why: &str) {
        let projects = at.map_or_else(|| self.roots.clone(), |p| vec![p.to_path_buf()]);
        let mut state = self.state.lock().unwrap();
        for project in projects {
            if state.down.contains_key(&project) {
                continue;
            }
            state.down.insert(project.clone(), why.to_string());
            if self.roots.len() > 1 {
                let home = self.home(&project);
                let what = "its findings are the degraded ones";
                state
                    .notes
                    .push(format!("rs oracle: {home} stopped ({why}); {what}"));
            }
        }
    }

    pub fn is_down(&self, project: &Utf8Path) -> bool {
        self.state.lock().unwrap().down.contains_key(project)
    }

    /// The reason where every project root is down, None while one still
    /// answers: an audit reads it as "the oracle gave nothing".
    pub fn failure(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        if state.down.len() < self.roots.len() {
            return None;
        }
        self.roots.iter().find_map(|p| state.down.get(p).cloned())
    }

    // sightline-ok: 11 - one push into each list the locked state keeps
    pub fn note(&self, note: String) {
        self.state.lock().unwrap().notes.push(note);
    }

    pub fn notes(&self) -> Vec<String> {
        self.state.lock().unwrap().notes.clone()
    }

    // sightline-ok: 11 - one push into each list the locked state keeps
    pub fn fault(&self, fault: String) {
        self.state.lock().unwrap().faults.push(fault);
    }

    pub fn faults(&self) -> Vec<String> {
        self.state.lock().unwrap().faults.clone()
    }

    /// Pass progress, as `rs/oracle.py`'s `on_event` prints it, and the wall
    /// `audit --profile` reads back.
    pub fn event(&self, label: &str, started: Instant) {
        let wall = started.elapsed().as_secs_f64();
        progress(&format!("sightline: rs {label} in {wall:.1}s"));
        self.state
            .lock()
            .unwrap()
            .passes
            .push((format!("rs {label}"), wall));
    }

    /// `(label, seconds)` per pass this oracle ran, in pass order.
    pub fn passes(&self) -> Vec<(String, f64)> {
        self.state.lock().unwrap().passes.clone()
    }

    /// The environment every toolchain call and the proc-macro server get:
    /// offline, and the project root's build directory.
    pub fn env(&self, at: Option<&Utf8Path>) -> Vec<(String, String)> {
        let target = at.and_then(|p| self.targets.get(p)).unwrap_or(&self.target);
        let row = |k: &str, v: &str| (k.to_string(), v.to_string());
        vec![
            row("CARGO_NET_OFFLINE", "true"),
            row("CARGO_TARGET_DIR", target.as_str()),
        ]
    }
}

/// An absolute path in the platform's own spelling, never the `\\?\`
/// verbatim form `canonicalize` gives on Windows: the target dir key must
/// equal the Python tool's `str(root.resolve())`.
pub fn absolute(path: &Utf8Path) -> Utf8PathBuf {
    let found = std::path::absolute(path.as_std_path()).ok();
    let utf8 = found.and_then(|p| Utf8PathBuf::from_path_buf(p).ok());
    utf8.unwrap_or_else(|| path.to_path_buf())
}

fn home_of(root: &Utf8Path, project: &Utf8Path) -> String {
    let rel = project.strip_prefix(root);
    rel.map(|rel| rel.as_str().replace('\\', "/"))
        .unwrap_or_default()
}

/// The directories the tools are pointed at. The audited root where it
/// holds a manifest, a package's or a `[workspace]`'s; else every crate root
/// `crates` names that no other crate root contains, so seven sibling
/// packages under a root with no manifest are seven projects and a crate
/// nested under one of them is that project's to report.
pub fn project_roots(root: &Utf8Path, crates: &IndexMap<String, String>) -> Vec<Utf8PathBuf> {
    if root.join(MANIFEST).exists() {
        return vec![root.to_path_buf()];
    }
    let named = crates
        .values()
        .map(String::as_str)
        .filter(|d| !d.is_empty());
    let mut dirs: Vec<&str> = named.collect();
    dirs.sort_unstable();
    dirs.dedup();
    let under = |d: &&&str| !dirs.iter().any(|o| d.starts_with(&format!("{o}/")));
    let top: Vec<Utf8PathBuf> = dirs.iter().filter(under).map(|d| root.join(d)).collect();
    if top.is_empty() {
        vec![root.to_path_buf()]
    } else {
        top
    }
}

/// One build directory per project root, outside every tree it audits:
/// `base` where the harness points a worktree run at the live root's, else
/// a per-root dir under the user's cache, keyed as `rs/oracle.py:target_dir`
/// keys it so both tools share one warm build. `project` is a rel under the
/// audited root, and gets a directory of its own: two crates of one tree may
/// pin different profiles, which cargo rebuilds a shared dir's dependencies
/// to switch between.
pub fn target_dir(root: &Utf8Path, project: &str, base: Option<&Utf8Path>) -> Utf8PathBuf {
    let base = match base {
        Some(named) => named.to_path_buf(),
        None => {
            let cache = match cfg!(windows) {
                true => local_app_data(),
                false => home().join(".cache"),
            };
            let key = format!("{:x}", Sha1::digest(root.as_str().as_bytes()));
            cache
                .join("sightline")
                .join("cargo-target")
                .join(&key[..12])
        }
    };
    if project.is_empty() {
        base
    } else {
        base.join(project.replace('/', "-"))
    }
}

fn local_app_data() -> Utf8PathBuf {
    let named = std::env::var("LOCALAPPDATA").map(Utf8PathBuf::from);
    named.unwrap_or_else(|_| home().join("AppData/Local"))
}

fn home() -> Utf8PathBuf {
    let named = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"));
    named.map(Utf8PathBuf::from).unwrap_or_default()
}

/// Every answer the toolchain gave one build, taken at build time: the
/// header names every pass whatever the rules asked for, so asking late
/// saves nothing.
pub struct RsAnswers {
    pub oracle: Option<RsOracle>,
    pub graph: RsGraph,
    /// the base check, every level
    pub diagnostics: Vec<RsDiag>,
    /// members whose lib or bin errors
    pub unchecked: BTreeSet<String>,
    /// the members a splice is verifiable in
    pub checked: Vec<RsMember>,
    pub block: Value,
    pub notes: Vec<String>,
}

impl Default for RsAnswers {
    fn default() -> RsAnswers {
        RsAnswers {
            oracle: None,
            graph: RsGraph::default(),
            diagnostics: Vec::new(),
            unchecked: BTreeSet::new(),
            checked: Vec::new(),
            block: json!({"enabled": false}),
            notes: Vec::new(),
        }
    }
}

impl RsAnswers {
    fn silent(note: String) -> RsAnswers {
        RsAnswers {
            notes: vec![note],
            ..RsAnswers::default()
        }
    }

    /// The diagnostics each overlay adds; empty where no oracle answers.
    pub fn verify_worlds(
        &self,
        worlds: &[(String, IndexMap<String, String>)],
    ) -> IndexMap<String, Vec<RsDiag>> {
        match &self.oracle {
            Some(oracle) if oracle.failure().is_none() => oracle.verify_worlds(worlds),
            _ => IndexMap::new(),
        }
    }

    /// End the toolchain and report what the header must say of it.
    pub fn close(&mut self) -> Vec<String> {
        let Some(oracle) = &mut self.oracle else {
            return self.notes.clone();
        };
        oracle.close();
        let mut out = self.notes.clone();
        out.extend(oracle.notes());
        out.extend(
            oracle
                .faults()
                .into_iter()
                .map(|f| format!("oracle fault: {f}")),
        );
        if let Some(why) = oracle.failure() {
            out.push(format!("rs oracle stopped ({why}); {NO_ORACLE_NOTE}"));
        }
        out
    }
}

/// The toolchain's answers for one audit: an oracle where the config keeps
/// it and `cargo` resolves on PATH, else the note naming what went silent.
/// A pass that stopped leaves every answer empty and the block off,
/// whichever of them the readings would have asked for.
pub fn build_answers(
    root: &Utf8Path,
    config: &Config,
    off: &RuleSet,
    facts: &RsFacts<'_>,
) -> RsAnswers {
    let path = std::env::var_os("PATH");
    answers_with(root, config, off, facts, cargo::find_cargo(path.as_deref()))
}

/// `build_answers` with the `cargo` lookup already made: the three silent
/// arms, then the passes. A test hands it the result of a PATH it controls.
pub fn answers_with(
    root: &Utf8Path,
    config: &Config,
    off: &RuleSet,
    facts: &RsFacts<'_>,
    cargo: Option<Utf8PathBuf>,
) -> RsAnswers {
    // the toolchain runs only for a rule that reads it
    if ORACLE_RULES.iter().all(|id| off.contains(*id)) {
        let why = "rs oracle not run: every oracle rule is off";
        return RsAnswers::silent(format!("{why}; {NO_ORACLE_NOTE}"));
    }
    if !config.oracle {
        return RsAnswers::silent(format!("rs oracle disabled by config: {NO_ORACLE_NOTE}"));
    }
    if cargo.is_none() {
        let why = "rs oracle disabled: cargo not on PATH";
        return RsAnswers::silent(format!("{why}; {NO_ORACLE_NOTE}"));
    }
    let base = std::env::var(TARGET_ENV).ok().map(Utf8PathBuf::from);
    let mut oracle = RsOracle::new(root, config, &facts.crates, base.as_deref());
    oracle.cargo.exe = cargo;
    answers_of(oracle, facts)
}

/// The passes over a built oracle in the order the Python tool forces
/// them, so the notes come out in its order: the index, then the members,
/// the check, the unchecked set and the versions. A test hands an oracle
/// pointed at its own target directory.
pub fn answers_of(oracle: RsOracle, facts: &RsFacts<'_>) -> RsAnswers {
    let graph = index::graph(&oracle, facts);
    let unchecked = oracle.unchecked().clone();
    if oracle.failure().is_some() {
        return RsAnswers {
            oracle: Some(oracle),
            ..RsAnswers::default()
        };
    }
    let live = oracle
        .members()
        .iter()
        .filter(|m| !unchecked.contains(&m.name));
    let checked: Vec<RsMember> = live.cloned().collect();
    let home = |p: &Utf8PathBuf| match oracle.home(p) {
        home if home.is_empty() => ".".to_string(),
        home => home,
    };
    let projects: Vec<String> = oracle.roots.iter().map(home).collect();
    let mut block = serde_json::Map::new();
    block.insert("enabled".into(), json!(true));
    block.insert("tools".into(), json!(oracle.versions()));
    block.insert("features".into(), json!(FEATURES));
    block.insert("projects".into(), json!(projects));
    block.insert("unchecked_members".into(), json!(unchecked));
    block.insert("edges".into(), json!(graph.edges.len()));
    for (k, v) in &graph.counts {
        block.insert(k.clone(), json!(v));
    }
    let diagnostics = oracle.diagnostics().to_vec();
    RsAnswers {
        oracle: Some(oracle),
        graph,
        diagnostics,
        unchecked,
        checked,
        block: Value::Object(block),
        notes: Vec::new(),
    }
}
