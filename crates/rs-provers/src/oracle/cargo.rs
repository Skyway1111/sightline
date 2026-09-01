//! `cargo metadata --no-deps` and `cargo check --workspace --all-targets
//! --keep-going` per project root (`rs/oracle.py`'s cargo half): the members,
//! the base diagnostics, the unchecked set and the versions. One subprocess
//! per call, and a spawn that fails ends that project root, never the audit.

use std::collections::{BTreeSet, HashSet};
use std::ffi::OsStr;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::diagnostic::{Diagnostic, DiagnosticLevel};
use cargo_metadata::{Message, Metadata, Package, Target};
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use crate::oracle::{RsDiag, RsMember, RsOracle, SURFACE, absolute};

// `--keep-going` because cargo stops scheduling after a failing crate, so
// without it the set a world diffs against moves between runs. `xtask
// fix-check` judges a patch with the same argv, so this is its one home.
#[rustfmt::skip]
pub const CHECK: [&str; 6] = [
    "check", "--workspace", "--all-targets", "--offline",
    "--message-format=json", "--keep-going",
];
#[rustfmt::skip]
const METADATA: [&str; 5] = ["metadata", "--no-deps", "--offline", "--format-version", "1"];

/// The `ra_ap_*` version this binary compiled in; `crates/rs-provers/
/// Cargo.toml` pins the same string and a test holds the two together.
pub const RA_AP: &str = "0.0.328";

/// `[tool.sightline.rust-toolchain]` from `sightline.toml`, embedded so a
/// released binary holds it: the one home for the toolchain pin.
pub fn pinned() -> IndexMap<String, String> {
    let text = include_str!("../../../../sightline.toml");
    let found = toml::from_str::<toml::Value>(text).ok().and_then(|table| {
        let pin = table.get("tool")?.get("sightline")?.get("rust-toolchain")?;
        let rows = pin.as_table()?.iter();
        Some(
            rows.filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect(),
        )
    });
    found.unwrap_or_default()
}

/// What the cargo passes memoize between calls, and the exe they run.
#[derive(Default)]
pub struct Cargo {
    /// `cargo` on PATH; a test points this at a program of its own
    pub exe: Option<Utf8PathBuf>,
    members: OnceLock<Vec<RsMember>>,
    diagnostics: OnceLock<Vec<RsDiag>>,
    unchecked: OnceLock<BTreeSet<String>>,
    versions: OnceLock<IndexMap<String, String>>,
    dependents: OnceLock<IndexMap<Utf8PathBuf, HashSet<Utf8PathBuf>>>,
    memo: Mutex<Memo>,
}

/// `_reads` (a project root's path deps' roots), `_failed` ((member, target,
/// kind) per target that errored) and `_built` ((member, kind) per target
/// cargo finished).
#[derive(Default)]
struct Memo {
    reads: IndexMap<Utf8PathBuf, IndexSet<Utf8PathBuf>>,
    failed: BTreeSet<(String, String, String)>,
    built: HashSet<(String, String)>,
}

impl Cargo {
    pub fn new(path: Option<&OsStr>) -> Cargo {
        Cargo {
            exe: find_cargo(path),
            ..Cargo::default()
        }
    }
}

/// `cargo` on the given search path (`PATH`'s value), None where the oracle
/// must say it is missing.
pub fn find_cargo(path: Option<&OsStr>) -> Option<Utf8PathBuf> {
    let name = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    let mut found = std::env::split_paths(path?).map(|dir| dir.join(name));
    Utf8PathBuf::from_path_buf(found.find(|exe| exe.is_file())?).ok()
}

impl RsOracle {
    /// One toolchain subprocess for the project root `at` (None for a call no
    /// root owns), as `(stdout, stderr)`. A spawn that fails ends that root.
    pub(super) fn run(
        &self,
        args: &[&str],
        cwd: &Utf8Path,
        label: &str,
        at: Option<&Utf8Path>,
    ) -> Option<(String, String)> {
        let exe = self.cargo.exe.as_ref()?;
        if at.is_some_and(|p| self.is_down(p)) {
            return None;
        }
        let started = Instant::now();
        let mut cmd = Command::new(exe);
        match cmd.args(args).current_dir(cwd).envs(self.env(at)).output() {
            Ok(out) => {
                self.event(label, started);
                let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
                Some((text(&out.stdout), text(&out.stderr)))
            }
            Err(e) => {
                self.fail(at, &format!("{label}: {e}"));
                None
            }
        }
    }

    /// Every project root's workspace members, in name order; one whose
    /// manifest sits outside the audited root is dropped (no file of it was
    /// indexed), and one two roots both list is one member.
    pub fn members(&self) -> &[RsMember] {
        self.cargo.members.get_or_init(|| {
            let (mut out, mut outside, mut seen) = (Vec::new(), 0usize, HashSet::new());
            for project in &self.roots {
                let label = "cargo metadata";
                let Some((stdout, _)) = self.run(&METADATA, project, label, Some(project)) else {
                    continue;
                };
                let Ok(found) = serde_json::from_str::<Metadata>(&stdout) else {
                    self.fail(Some(project), "cargo metadata: no member list");
                    continue;
                };
                for pkg in found.packages {
                    let home = self.rel(pkg.manifest_path.as_str());
                    self.note_reads(project, &pkg);
                    let Some(home) = home else {
                        outside += 1;
                        continue;
                    };
                    if seen.insert(home.clone()) {
                        let (name, kind) = (pkg.name.to_string(), surface_kind(&pkg));
                        out.push(RsMember {
                            name,
                            dir: home,
                            kind,
                        });
                    }
                }
            }
            if outside > 0 {
                let what = "workspace members sit outside the audited root";
                self.note(format!("rs oracle: {outside} {what}"));
            }
            out.sort_by(|a, b| a.name.cmp(&b.name));
            out
        })
    }

    /// Every project root a package's path dependencies reach into, which
    /// `dependents` folds: a world checks in all of them.
    fn note_reads(&self, project: &Utf8Path, pkg: &Package) {
        for dep in &pkg.dependencies {
            let read = dep.path.as_ref().and_then(|p| self.project_of(p.as_path()));
            let Some(read) = read.filter(|r| *r != project).cloned() else {
                continue;
            };
            let mut memo = self.cargo.memo.lock().unwrap();
            memo.reads
                .entry(project.to_path_buf())
                .or_default()
                .insert(read);
        }
    }

    /// A manifest's directory, relative to the audited root; `.` for the root
    /// itself, as `Path.relative_to` spells it.
    fn rel(&self, manifest: &str) -> Option<String> {
        let dir = absolute(Utf8Path::new(manifest));
        let rel = dir.parent()?.strip_prefix(&self.root).ok()?;
        match rel.as_str().replace('\\', "/") {
            home if home.is_empty() => Some(".".to_string()),
            home => Some(home),
        }
    }

    /// The member a message belongs to, by its manifest's directory: a
    /// package whose name is not its directory's is still named right.
    fn member(&self, manifest: &str) -> String {
        let dir = self.rel(manifest);
        let found = self.members().iter().find(|m| Some(&m.dir) == dir.as_ref());
        found.map(|m| m.name.clone()).unwrap_or_default()
    }

    /// Every project root's base check messages, every level kept. A check
    /// that neither finishes nor reports an error is a dead pass, not a clean
    /// tree, and stops that root alone.
    pub fn diagnostics(&self) -> &[RsDiag] {
        self.cargo.diagnostics.get_or_init(|| {
            let mut out = Vec::new();
            for project in &self.roots {
                let found = self.run(&CHECK, project, "cargo check", Some(project));
                let Some((stdout, stderr)) = found else {
                    continue;
                };
                let (rows, finished) = self.diags(&stdout, project, &self.root, true);
                if !finished && !rows.iter().any(|d| d.level == "error") {
                    let tail = stderr.trim().lines().next_back().unwrap_or("");
                    self.fail(
                        Some(project),
                        &format!("cargo check: no build-finished ({tail})"),
                    );
                    continue;
                }
                out.extend(rows);
            }
            out
        })
    }

    /// cargo's json stream to records, plus whether the build finished. Cargo
    /// spells a file relative to the project root it ran in, so every one is
    /// re-rooted to `base` (the audited root, or a world's copy of it).
    /// `collect` fills `failed` and `built`: a member cargo skipped because a
    /// dependency of it failed lands in neither.
    pub(super) fn diags(
        &self,
        stdout: &str,
        project: &Utf8Path,
        base: &Utf8Path,
        collect: bool,
    ) -> (Vec<RsDiag>, bool) {
        let (mut out, mut finished) = (Vec::new(), false);
        let (mut failed, mut built) = (Vec::new(), Vec::new());
        for line in stdout.lines().filter(|l| l.starts_with('{')) {
            let Ok(entry) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            // cargo names the manifest on every message; `cargo_metadata`
            // models it on an artifact alone, so it is read off the raw row
            let named = entry.get("manifest_path").and_then(Value::as_str);
            let manifest = named.unwrap_or("").to_string();
            match serde_json::from_value::<Message>(entry) {
                Ok(Message::BuildFinished(_)) => finished = true,
                Ok(Message::CompilerArtifact(a)) if collect => {
                    let member = self.member(&manifest);
                    let kinds = a
                        .target
                        .kind
                        .iter()
                        .map(|k| (member.clone(), k.to_string()));
                    built.extend(kinds);
                }
                Ok(Message::CompilerMessage(m)) => {
                    if collect && m.message.level == DiagnosticLevel::Error {
                        let name = m.target.name.clone();
                        failed.push((self.member(&manifest), name, first_kind(&m.target)));
                    }
                    let found = diag(&m.message, &m.target.name);
                    out.push(RsDiag {
                        rel: reroot(&found.rel, project, base),
                        ..found
                    });
                }
                _ => {}
            }
        }
        if collect {
            let mut memo = self.cargo.memo.lock().unwrap();
            memo.failed.extend(failed);
            memo.built.extend(built);
        }
        (out, finished)
    }

    /// Members whose lib or bin the base check did not compile clean: one
    /// that errored, and one cargo never reached because a dependency of it
    /// failed. They enter no world. A member whose only failing target is a
    /// test or an example stays checked, and the note names that target.
    pub fn unchecked(&self) -> &BTreeSet<String> {
        self.cargo.unchecked.get_or_init(|| {
            // the check runs before the member list, as `rs/oracle.py` forces
            // them, and `members` is forced before the lock it also takes
            if self.diagnostics().is_empty() && self.failure().is_some() {
                return BTreeSet::new();
            }
            let members = self.members();
            let memo = self.cargo.memo.lock().unwrap();
            let unbuilt = |m: &&RsMember| {
                !m.kind.is_empty() && !memo.built.contains(&(m.name.clone(), m.kind.clone()))
            };
            let named = |(m, ..): &(String, String, String)| m.clone();
            let skipped: BTreeSet<String> = members
                .iter()
                .filter(unbuilt)
                .map(|m| m.name.clone())
                .collect();
            let errored: BTreeSet<String> = memo.failed.iter().map(named).collect();
            let surface = memo
                .failed
                .iter()
                .filter(|(.., k)| SURFACE.contains(&k.as_str()));
            let mut out: BTreeSet<String> = surface.map(named).collect();
            out.extend(skipped.iter().cloned());
            let never: Vec<&str> = skipped.difference(&errored).map(String::as_str).collect();
            if !never.is_empty() {
                let why = "(a dependency failed); unchecked";
                let reached = never.join(", ");
                self.note(format!(
                    "rs oracle: cargo check never reached {reached} {why}"
                ));
            }
            if !memo.failed.is_empty() {
                self.note(errors_note(&memo.failed, &out));
            }
            out
        })
    }

    /// Per project root, the roots whose own build reads it: one holding a
    /// path dependency into it, and their dependents in turn. A world checks
    /// in all of them, so a splice only a downstream crate breaks is vetoed
    /// rather than verified (`members` reads the edges).
    pub fn dependents(&self) -> &IndexMap<Utf8PathBuf, HashSet<Utf8PathBuf>> {
        self.cargo.dependents.get_or_init(|| {
            let _ = self.members();
            let reads = self.cargo.memo.lock().unwrap().reads.clone();
            let blank = self.roots.iter().map(|p| (p.clone(), HashSet::new()));
            let mut out: IndexMap<Utf8PathBuf, HashSet<Utf8PathBuf>> = blank.collect();
            for start in &self.roots {
                let (mut stack, mut seen) = (vec![start.clone()], HashSet::new());
                while let Some(at) = stack.pop() {
                    for read in reads.get(&at).into_iter().flatten() {
                        if seen.insert(read.clone()) {
                            stack.push(read.clone());
                        }
                    }
                }
                for read in &seen {
                    out.entry(read.clone()).or_default().insert(start.clone());
                }
            }
            out
        })
    }

    /// What the header reports the toolchain as: the token `cargo --version`
    /// names its version by (`cargo 1.95.0 (...)`) and the compiled-in
    /// `ra_ap`, each against the pin. A version off it is a note.
    pub fn versions(&self) -> &IndexMap<String, String> {
        self.cargo.versions.get_or_init(|| {
            let ran = self.run(&["--version"], &self.root, "cargo version", None);
            let spelled = ran.map(|(stdout, _)| stdout).unwrap_or_default();
            let cargo = spelled.split_whitespace().nth(1).unwrap_or("").to_string();
            let ra_ap = RA_AP.to_string();
            let out = IndexMap::from([("cargo".into(), cargo), ("ra_ap".into(), ra_ap)]);
            let pin = pinned();
            for (name, found) in &out {
                let off = pin.get(name).filter(|w| !found.is_empty() && *w != found);
                if let Some(want) = off {
                    self.note(format!("rs oracle: {name} {found} is off the {want} pin"));
                }
            }
            out
        })
    }
}

/// The first of `SURFACE` among a package's target kinds, else empty.
fn surface_kind(pkg: &Package) -> String {
    let spelled = pkg
        .targets
        .iter()
        .flat_map(|t| t.kind.iter().map(ToString::to_string));
    let kinds: HashSet<String> = spelled.collect();
    let found = SURFACE.iter().find(|k| kinds.contains(**k));
    found.map(|k| (*k).to_string()).unwrap_or_default()
}

fn first_kind(target: &Target) -> String {
    target
        .kind
        .first()
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// The header's line for a check that errored: every failing target, then the
/// members those errors left unchecked. `?` stands for a name cargo left
/// empty.
fn errors_note(failed: &BTreeSet<(String, String, String)>, out: &BTreeSet<String>) -> String {
    let blank = |name: &String| if name.is_empty() { "?" } else { name.as_str() }.to_string();
    let named: Vec<String> = failed
        .iter()
        .map(|(m, t, k)| format!("{} ({} {t})", blank(m), blank(k)))
        .collect();
    let listed: Vec<&str> = out.iter().map(String::as_str).collect();
    let tail = match listed.is_empty() {
        true => String::new(),
        false => format!("; unchecked members: {}", listed.join(", ")),
    };
    format!(
        "rs oracle: cargo check errors in {}{tail}",
        named.join(", ")
    )
}

/// A file cargo named, as `base` spells it: cargo reports it relative to the
/// project root it ran in, or absolutely for a path dependency. One outside
/// `base` keeps its absolute posix spelling, which no rel collides with; a
/// message with no span keeps its empty one.
fn reroot(name: &str, project: &Utf8Path, base: &Utf8Path) -> String {
    if name.is_empty() {
        return String::new();
    }
    let named = Utf8Path::new(name);
    let under = if named.is_absolute() {
        named.to_path_buf()
    } else {
        project.join(named)
    };
    let path = absolute(&under);
    match path.strip_prefix(absolute(base)) {
        Ok(rel) => rel.as_str().replace('\\', "/"),
        Err(_) => path.as_str().replace('\\', "/"),
    }
}

/// One `compiler-message`, at its primary span else its first.
fn diag(message: &Diagnostic, crate_name: &str) -> RsDiag {
    let span = message
        .spans
        .iter()
        .find(|s| s.is_primary)
        .or(message.spans.first());
    // cargo's own spelling of the level, so a reader keying on it agrees
    let level = serde_json::to_value(message.level).unwrap_or_default();
    RsDiag {
        rel: span.map_or(String::new(), |s| s.file_name.replace('\\', "/")),
        line: span.map_or(0, |s| s.line_start as u32),
        col: span.map_or(0, |s| s.column_start as u32),
        code: message
            .code
            .as_ref()
            .map_or("", |c| c.code.as_str())
            .to_string(),
        level: level.as_str().unwrap_or("").to_string(),
        message: message.message.clone(),
        crate_name: crate_name.to_string(),
    }
}
