//! Port of `provers/oracle.py`, `provers/unresolved.py` and the shim
//! (`ty_pyright_shim/src/{main,batch}.rs`), codemap section 5: the Python
//! oracle in process, on a `ty_project::ProjectDatabase` built with the
//! shim's options. The only module of the workspace naming `ty_*` and
//! `ruff_db`.
//!
//! One database per audit: the base pass (`db.check()` plus the callee edges)
//! is memoized, span and member queries run on clones under rayon, worlds run
//! one after another under the lock. Every pass body runs inside
//! `ruff_db::panic::catch_unwind`; a checker panic sets `failure`, later
//! answers are empty, and `Provers::close` notes the degradation.

use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::sync::{Mutex, OnceLock, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use rayon::prelude::*;
use regex::Regex;
use ruff_db::files::{File, system_path_to_file};
use ruff_db::panic::catch_unwind;
use ruff_db::parsed::parsed_module;
use ruff_db::source::{SourceText, line_index, source_text};
use ruff_db::system::{OsSystem, System as _, SystemPath, SystemPathBuf};
use ruff_diagnostics::SourceMap;
use ruff_python_ast::AnyNodeRef;
use ruff_python_ast::find_node::covering_node;
use ruff_python_ast::{Expr, Stmt};
use ruff_ranged_value::RangedValue;
use ruff_source_file::OneIndexed;
use ruff_text_size::{Ranged, TextRange, TextSize};
use salsa::Setter as _;
use serde_json::{Value, json};
use ty_project::metadata::options::{EnvironmentOptions, Options, SrcOptions};
use ty_project::metadata::value::{RelativeGlobPattern, RelativePathBuf};
use ty_project::parallel::minimum_parallel_job_len;
use ty_project::{ProjectDatabase, ProjectMetadata};
use ty_python_semantic::lint::Level;
use ty_python_semantic::types::{Type, revealed_display};
use ty_python_semantic::{Db as _, HasType, SemanticModel};

use sightline_core::findings::Rel;
use sightline_core::progress::progress;
use sightline_core::worlds::{Diag, World};
use sightline_py_facts::model::RepoFacts;

use crate::Provers;
use crate::callgraph::CallEdge;

mod convert;
mod db;
mod edges;
mod queries;
mod worlds;

use convert::{ENABLED_RULES, convert, normalize_type_display};
use db::{database, rel_of, resolve};

/// The four redundancy verdicts a counterfactual receipt reads.
pub const UNNECESSARY_RULES: [&str; 4] = [
    "reportUnnecessaryIsInstance",
    "reportUnnecessaryComparison",
    "reportUnnecessaryContains",
    "reportUnnecessaryCast",
];

/// Verdicts the shim reports at warning severity by design: the
/// counterfactual veto (#5/#10) fires on new error-severity diagnostics, so a
/// possibly-unbound read a splice reveals reports without taking the patch
/// with it. `Provers::errors` reads them beside the errors.
pub const WARNING_VERDICTS: [&str; 1] = ["reportPossiblyUnbound"];

/// The build stamp the header prints (`oracle.build`): the fork rev baked
/// in at build.
pub const BUILD: &str = "ty-unnecessary 284831cb43bb167d149b23f0e49bcae015c4d183";

/// The directories no shadow tree is read from (`oracle.py:_SHADOW_EXCLUDES`),
/// ahead of `**/<exclude>` per config exclude.
const SHADOW_EXCLUDES: [&str; 7] = [
    "**/__pycache__",
    "**/venv",
    "**/node_modules",
    "**/site-packages",
    "**/build",
    "**/dist",
    "**/.*",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OracleDiag {
    /// posix, relative to the analyzed root
    pub rel: Rel,
    /// 1-based
    pub line: u32,
    /// 0-based code point offset within the line (`LineIndex::line_column` is
    /// UTF-32; `counterfactual::_by_operand` slices the line by code point
    /// with it). A `TypeQuery` column is a UTF-8 byte offset instead.
    pub col: u32,
    pub rule: String,
    pub message: String,
    /// "error" | "warning"
    pub severity: String,
}

impl Diag for OracleDiag {
    fn rel(&self) -> &str {
        &self.rel
    }

    fn line(&self) -> u32 {
        self.line
    }

    fn severity(&self) -> &str {
        &self.severity
    }
}

/// One span the checker is asked the type of: a single-line expression, its
/// columns UTF-8 byte offsets within the line (the CPython `ast` convention,
/// R1). `id` is the asker's own label (`q<n>` arg types, `v<n>` receivers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeQuery {
    pub id: String,
    pub rel: Rel,
    pub line: u32,
    pub col_start: u32,
    pub col_end: u32,
}

/// One `verify_worlds` call as the `verify` layer prints it: the worlds
/// asked, each with its overlay files, and what each world added.
#[derive(Debug, Clone)]
pub struct WorldCall {
    pub worlds: Vec<(String, Vec<Rel>)>,
    pub added: IndexMap<String, Vec<OracleDiag>>,
}

/// The base pass, read once: the converted diagnostics, the callee edges from
/// the same state, and the `(rel, zero-based line, rule)` keys a world diffs
/// against.
#[derive(Default)]
struct Base {
    diags: Vec<OracleDiag>,
    edges: Vec<CallEdge>,
    keys: HashSet<(Rel, u32, String)>,
}

/// The in-process checker.
pub struct Oracle {
    root: Utf8PathBuf,
    sys_root: SystemPathBuf,
    /// `None` when the construction failed: `failure` names it and every
    /// answer is empty from there, as for a shim that crashed under its first
    /// pass.
    db: Mutex<Option<ProjectDatabase>>,
    base: OnceLock<Base>,
    failure: Mutex<Option<String>>,
    calls: Mutex<Vec<WorldCall>>,
    /// `(label, seconds)` per pass, counted and printed as `oracle.py:_pass`
    /// does (`provers/__init__.py` wires the print to stderr).
    passes: Mutex<Vec<(String, f64)>>,
}

impl Oracle {
    /// The shim's construction (codemap 5): `OsSystem` at `root`,
    /// `ProjectMetadata::discover`, `apply_configuration_files`, the
    /// `Options` with `environment.python` from `python_exe`, `extra_paths`
    /// from `import_roots`, `src.exclude` from the shadow excludes plus
    /// `**/<exclude>` per config exclude, `respect_ignore_files = false`, the
    /// six enabled rules at `Level::Warn`, `ProjectDatabase::fallible`,
    /// `disable_lru`.
    pub fn new(
        root: &Utf8Path,
        excludes: &[String],
        import_roots: &[Utf8PathBuf],
        python_exe: Option<&Utf8Path>,
    ) -> anyhow::Result<Oracle> {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|d| SystemPathBuf::from_path_buf(d).ok())
            .unwrap_or_else(|| SystemPathBuf::from(root.as_str()));
        // ty canonicalizes a search path and leaves a src root as written, so a
        // root that spells one directory two ways reaches the file interner
        // twice: `edges::edge_verdict` then reads an in-repo callee as external
        // and a world's overlay on another module goes unseen. Canonicalize
        // once here, the way ty does (`System::canonicalize_path` canonicalizes
        // then simplifies, so Windows keeps `C:/x` over `\\?\C:/x`), and one
        // spelling reaches the database, every path query and every world.
        // `root()` keeps the spelling the caller passed: that is what the
        // provenance header prints.
        let sys_root = SystemPath::absolute(root.as_str(), &cwd);
        let sys_root = OsSystem::new(&sys_root)
            .canonicalize_path(&sys_root)
            .unwrap_or(sys_root);
        let oracle = Oracle {
            root: root.to_path_buf(),
            sys_root: sys_root.clone(),
            db: Mutex::new(None),
            base: OnceLock::new(),
            failure: Mutex::new(None),
            calls: Mutex::new(Vec::new()),
            passes: Mutex::new(Vec::new()),
        };
        match database(&sys_root, excludes, import_roots, python_exe) {
            Ok(db) => *oracle.db.lock().unwrap() = Some(db),
            Err(e) => *oracle.failure.lock().unwrap() = Some(format!("construction: {e}")),
        }
        Ok(oracle)
    }

    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    pub fn build(&self) -> &'static str {
        BUILD
    }

    /// `"{pass} pass: {message}"` once a checker panic was caught.
    pub fn failure(&self) -> Option<String> {
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// `(label, seconds)` per pass this oracle ran, in pass order.
    pub fn passes(&self) -> Vec<(String, f64)> {
        self.passes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// One pass over the database, under the lock and inside `catch_unwind`:
    /// `None` when the oracle never started, already failed, or panicked here.
    /// The pass is counted and its start and finish printed, as
    /// `oracle.py:_pass` reports them through `on_event`.
    fn pass<R>(&self, label: &str, body: impl FnOnce(&mut ProjectDatabase) -> R) -> Option<R> {
        let mut held = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        let db = held.as_mut()?;
        if self.failure().is_some() {
            return None;
        }
        let mut walls = self.passes.lock().unwrap_or_else(PoisonError::into_inner);
        let counted = format!("oracle pass {} ({label})", walls.len() + 1);
        progress(&format!("sightline: {counted} started"));
        let started = std::time::Instant::now();
        let answer = catch_unwind(AssertUnwindSafe(move || body(db)));
        let wall = started.elapsed().as_secs_f64();
        progress(&format!("sightline: {counted} finished in {wall:.1}s"));
        walls.push((counted, wall));
        drop(walls);
        match answer {
            Ok(answer) => Some(answer),
            Err(panic) => {
                *self.failure.lock().unwrap_or_else(PoisonError::into_inner) =
                    Some(format!("{label} pass: {}", panic.payload));
                None
            }
        }
    }

    /// The base pass, run once: `db.check()` through `convert`, then the
    /// callee edges from the same state.
    fn base(&self) -> &Base {
        self.base.get_or_init(|| {
            let root = self.sys_root.clone();
            self.pass("diagnostics+edges", |db| {
                let diags: Vec<OracleDiag> = db
                    .check()
                    .iter()
                    .filter_map(|d| convert(db, &root, d))
                    .collect();
                let keys = diags
                    .iter()
                    .map(|d| (d.rel.clone(), d.line - 1, d.rule.clone()))
                    .collect();
                let edges = edges::call_edges(db, &root);
                Base { diags, edges, keys }
            })
            .unwrap_or_default()
        })
    }

    /// The base pass (`db.check()` once) through `convert`: the six enabled
    /// rules under pyright's names, every other error-severity diagnostic
    /// under its own ty id, `RevealedType` dropped, a path outside the root
    /// dropped.
    pub fn diagnostics(&self) -> &[OracleDiag] {
        &self.base().diags
    }

    pub fn unnecessary(&self) -> Vec<&OracleDiag> {
        self.diagnostics()
            .iter()
            .filter(|d| UNNECESSARY_RULES.contains(&d.rule.as_str()))
            .collect()
    }

    /// Checker-resolved callee definitions per call site (the shim's
    /// `call_edges`, from the same base state).
    pub fn call_edges(&self) -> &[CallEdge] {
        &self.base().edges
    }

    /// Every `verify_worlds` call so far, in order.
    pub fn world_calls(&self) -> Vec<WorldCall> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The verbs' last call. The database lives as long as the `Oracle`, so a
    /// layer dump taken after `close` still reads the memoized passes.
    pub fn close(&self) {}
}

/// `detect_python_env`: the target repo's interpreter for import resolution,
/// `<root>/<configured>/Scripts/python.exe` (`bin/python` off Windows) when
/// config names one, else `<root>/.venv/...`, whichever exists.
pub fn detect_python_env(root: &Utf8Path, configured: Option<&str>) -> Option<Utf8PathBuf> {
    let exe = if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    };
    configured
        .map(|c| root.join(c).join(exe))
        .into_iter()
        .chain(std::iter::once(root.join(".venv").join(exe)))
        .find(|p| p.is_file())
}

/// `provers/unresolved.py`: how many imports the oracle could not resolve,
/// per missing module, so the header says how much of the tree it could see.
/// Without an oracle: nothing unresolved.
#[derive(Debug, Default, Clone)]
pub struct UnresolvedImports {
    /// unresolved module -> import sites, in first-seen order
    pub modules: IndexMap<String, u32>,
}

impl UnresolvedImports {
    pub fn new(facts: &RepoFacts<'_>, oracle: Option<&Oracle>) -> UnresolvedImports {
        static MISSING: OnceLock<Regex> = OnceLock::new();
        let pattern =
            MISSING.get_or_init(|| Regex::new(r#"module "([^"]+)""#).expect("a valid pattern"));
        let mut modules: IndexMap<String, u32> = IndexMap::new();
        for diag in oracle.map(Oracle::diagnostics).unwrap_or(&[]) {
            if diag.rule != "reportMissingImports" {
                continue;
            }
            let Some(found) = pattern.captures(&diag.message) else {
                continue;
            };
            if facts.module_by_rel(&diag.rel).is_none() {
                continue;
            }
            *modules.entry(found[1].to_string()).or_default() += 1;
        }
        UnresolvedImports { modules }
    }

    pub fn count(&self) -> u32 {
        self.modules.values().sum()
    }
}

/// `layer_oracle` (`dump_layers.py:_oracle_answers`): the empty document
/// without an oracle, else the base pass, the edges and every query answer.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let Some(oracle) = provers.oracle() else {
        return Some(json!({
            "diagnostics": [], "call_edges": [], "arg_types": [], "ret_types": {},
            "recv_types": [], "unresolved": {"count": 0, "modules": {}},
        }));
    };
    let unresolved = provers.unresolved(facts);
    Some(json!({
        "diagnostics": oracle
            .diagnostics()
            .iter()
            .map(|d| json!([&*d.rel, d.line, d.col, d.rule, d.severity, d.message]))
            .collect::<Vec<_>>(),
        "call_edges": oracle
            .call_edges()
            .iter()
            .map(|e| json!({
                "rel": &*e.rel, "line": e.line, "col": e.col,
                "end_line": e.end_line, "end_col": e.end_col,
                "targets": e.targets.iter().map(|(r, l)| json!([&**r, l])).collect::<Vec<_>>(),
                "external": e.external.iter().map(|q| &**q).collect::<Vec<&str>>(),
            }))
            .collect::<Vec<_>>(),
        "arg_types": provers.arg_types(facts).dump_rows(facts),
        "ret_types": provers.ret_types(facts).dump_map(),
        "recv_types": provers.recv_types(facts).dump_rows(),
        "unresolved": {"count": unresolved.count(), "modules": unresolved.modules},
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Utf8Path, files: &[(&str, &str)]) {
        for (rel, text) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("the fixture dirs");
            std::fs::write(&path, text).expect("the fixture files");
        }
    }

    #[test]
    fn detect_python_env_reads_the_configured_env_then_dot_venv() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
        let exe = if cfg!(windows) {
            "Scripts/python.exe"
        } else {
            "bin/python"
        };
        assert_eq!(detect_python_env(root, None), None);
        write(root, &[(&format!(".venv/{exe}"), "")]);
        assert_eq!(
            detect_python_env(root, None),
            Some(root.join(".venv").join(exe))
        );
        write(root, &[(&format!("customenv/{exe}"), "")]);
        assert_eq!(
            detect_python_env(root, Some("customenv")),
            Some(root.join("customenv").join(exe))
        );
    }
}
