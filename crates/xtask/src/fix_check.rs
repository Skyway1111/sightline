//! `cargo xtask fix-check`: emitter honesty on the pinned corpus
//! (`scripts/fix_check.py`), criterion 7.
//!
//! file-length-ok: one ruler, one file - the apply/judge/revert spine plus a
//! suite reader per language, and a split would put the spine and its
//! readers in different homes.
//!
//! Per tree: `sightline fix`, then every emitted patch applies with `git
//! apply`, a re-audit reports no patched finding (the patch's own `#
//! sightline-fix:` headers name them), the tree's own suite passes, and `git
//! apply -R` restores it. Patched files must be locally clean first, and a
//! dirty live tree is checked in a detached worktree at HEAD. No sampling:
//! every emitted patch is applied. The suite is the language's, the tree
//! venv's pytest for Python, and for Rust a `cargo check` adding no error
//! the pre-patch check did not have then `cargo test` over the crate-targets
//! that check compiled. A language with no emitter yields an empty diff,
//! which is a verdict, not a skip. A run killed mid-check leaves its patch
//! applied; the next run reverses it first.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};

use crate::corpus::{self, Target, tail};
use crate::paths::{drop_toolchain, venv_python, workspace_root};
use crate::worktree::{self, utf8};

use sightline_rs_provers::oracle::{SURFACE, cargo::CHECK, target_dir};

/// `(member, target kind, target name)`: how a crate-target is named.
type Crate = (String, String, String);
/// One tree's receipt, and the fields a stage adds to it.
type Fields = Map<String, Value>;

/// `receipt[key] = value`: a receipt holds twenty of these.
fn put(at: &mut Fields, key: &str, value: impl Into<Value>) {
    at.insert(key.to_string(), value.into());
}

/// One git run with an optional patch on stdin.
fn git(root: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Output> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("git {}", args.join(" ")))?;
    if let Some(mut pipe) = child.stdin.take() {
        pipe.write_all(input.unwrap_or_default())?;
    }
    Ok(child.wait_with_output()?)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The resolved path, spelled as `worktree::utf8` spells one.
fn real(path: &Path) -> PathBuf {
    let found = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    utf8(&found).into_std_path_buf()
}

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// The files a diff writes to.
fn patched_rels(diff: &str) -> Vec<String> {
    let plus = pattern(r"(?m)^\+\+\+ b/(.+?)\r?$");
    let mut rels: Vec<String> = plus.captures_iter(diff).map(|c| c[1].to_string()).collect();
    rels.sort();
    rels.dedup();
    rels
}

/// `(rule, cause)` per `# sightline-fix:` header: the findings the patch
/// claims to have fixed.
fn patched_findings(diff: &str) -> BTreeSet<(String, String)> {
    pattern(r"(?m)^# sightline-fix: (\S+) (.+?)\r?$")
        .captures_iter(diff)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect()
}

/// A run killed mid-check left its patch applied: reverse it, then drop the
/// CRLF residue the reverse leaves. True when the tree was healed; a clean
/// tree or a foreign edit is left alone.
fn heal_left_patch(root: &Path, patch: &Path) -> Result<bool> {
    let Ok(raw) = std::fs::read(patch) else {
        return Ok(false);
    };
    if !git(root, &["apply", "--check", "--reverse", "-"], Some(&raw))?
        .status
        .success()
    {
        return Ok(false);
    }
    git(root, &["apply", "--reverse", "-"], Some(&raw))?;
    let rels = patched_rels(&text(&raw));
    let mut args = vec!["checkout", "--"];
    args.extend(rels.iter().map(String::as_str));
    git(root, &args, None)?;
    Ok(true)
}

/// `(rule, cause)` of every finding a fresh audit of the patched tree
/// reports.
fn post_audit_pairs(t: &Target) -> Result<BTreeSet<(String, String)>> {
    let root = t.root.display().to_string();
    let done = corpus::sightline(t, &["audit", &root, "--json"])?;
    if !done.status.success() {
        bail!("post-apply audit failed: {}", tail(&text(&done.stderr)));
    }
    let doc: Value = serde_json::from_slice(&done.stdout)?;
    let field = |f: &Value, key| f[key].as_str().unwrap_or_default().to_string();
    Ok(doc["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| (field(f, "rule"), field(f, "cause")))
        .collect())
}

// --- the target's own suite, one reading per language -------------------------

/// The first `name` on PATH.
fn which(name: &str) -> Option<PathBuf> {
    let exe = format!("{name}{}", if cfg!(windows) { ".exe" } else { "" });
    let sep = if cfg!(windows) { ';' } else { ':' };
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(sep)
        .map(|dir| Path::new(dir).join(&exe))
        .find(|p| p.is_file())
}

/// The interpreter that runs the tree's suite: its own `.venv`, else the one
/// its config names. A detached worktree has no `.venv`, and
/// `worktree::config` put the live root's there for exactly this reason, so
/// without the fallback a dirty tree's suite is silently not run.
fn suite_python(t: &Target) -> Option<PathBuf> {
    if let Some(found) = venv_python(&t.root.join(".venv")) {
        return Some(found);
    }
    let table: toml::Table = std::fs::read_to_string(t.config.as_ref()?)
        .ok()?
        .parse()
        .ok()?;
    let named = table.get("tool")?.get("sightline")?.get("python-env")?;
    venv_python(Path::new(named.as_str()?))
}

/// The tree's own pytest invocation, or None when it has none. Parallel
/// where its venv holds xdist: merged-calculator's 15k tests take 376 s
/// sequentially and 133 s at `-n auto`.
fn target_suite(t: &Target) -> Option<Vec<String>> {
    let python = suite_python(t)?;
    if !t.root.join("tests").is_dir() {
        return None;
    }
    let probe = Command::new(&python)
        .args([
            "-c",
            "import pytest, importlib.util as u; print(u.find_spec('xdist') is not None)",
        ])
        .output()
        .ok()?;
    if !probe.status.success() {
        return None;
    }
    let mut cmd: Vec<String> = ["-m", "pytest", "-q"].iter().map(|a| (*a).into()).collect();
    if text(&probe.stdout).trim() == "True" {
        cmd.extend(["-n".to_string(), "auto".to_string()]);
    }
    cmd.insert(0, python.to_string_lossy().into_owned());
    Some(cmd)
}

/// The Python target's own pytest run. A tree with no runnable suite is
/// reported as such, never failed on it.
fn pytest_verdict(t: &Target) -> Result<(Fields, bool)> {
    let mut out = Fields::new();
    let Some(suite) = target_suite(t) else {
        put(
            &mut out,
            "suite",
            "not runnable (no repo venv+pytest+tests)",
        );
        return Ok((out, true));
    };
    let mut cmd = Command::new(&suite[0]);
    cmd.args(&suite[1..]).current_dir(&t.root);
    // the target suites shell out to Unix tools (merged-calculator greps),
    // which the shell that launched this run need not have on PATH
    if let Some(bin) = which("git").and_then(|g| Some(g.parent()?.parent()?.join("usr/bin"))) {
        let held = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{};{held}", bin.display()));
    }
    let done = cmd.output().context("running the target's pytest")?;
    let all = match text(&done.stdout) {
        empty if empty.trim().is_empty() => text(&done.stderr),
        shown => shown,
    };
    let last = all.trim().lines().next_back().unwrap_or("");
    let code = done.status.code().unwrap_or(-1);
    put(&mut out, "suite", format!("exit {code}: {last}"));
    // the names, so a failure can be attributed to the patch or to the host
    // without re-running a suite that has taken 23 minutes on a corpus tree
    let named: Vec<&str> = all
        .lines()
        .filter_map(|l| l.strip_prefix("FAILED "))
        .map(|l| l.split(' ').next().unwrap_or(l))
        .collect();
    if !named.is_empty() {
        put(&mut out, "suite_failures", json!(named));
    }
    Ok((out, done.status.success()))
}

/// One cargo run on a corpus tree: offline, and into the build directory
/// that tree's audit uses, so its dependencies are already warm.
fn cargo(root: &Path, args: &[&str], env: &[(String, String)]) -> Result<Output> {
    let named = env.iter().find(|(k, _)| k == "SIGHTLINE_CARGO_TARGET");
    let target = match named {
        Some((_, dir)) => dir.clone(),
        None => target_dir(&utf8(&real(root)), "", None).into_string(),
    };
    let mut cmd = Command::new("cargo");
    cmd.args(args)
        .current_dir(root)
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", target)
        .envs(env.iter().map(|(k, v)| (k, v)));
    drop_toolchain(&mut cmd);
    cmd.output().context("running cargo")
}

/// What cargo said of the tree before any patch: which member owns each
/// manifest, each member's crate-targets, and the ones the check could not
/// compile. A post-apply error outside `failed` belongs to the patch.
struct CargoBase {
    member_of: BTreeMap<PathBuf, String>,
    targets: BTreeMap<String, Vec<(String, String)>>,
    failed: BTreeSet<Crate>,
}

impl CargoBase {
    /// Members whose lib or bin fails the base check: nothing about them is
    /// verifiable, so they enter neither the check nor the suite. A member
    /// whose only failing target is a test keeps the rest, the line
    /// `rs/oracle.py` draws for a world.
    fn unchecked(&self) -> BTreeSet<String> {
        self.failed
            .iter()
            .filter(|(_, kind, _)| SURFACE.contains(&kind.as_str()))
            .map(|(member, ..)| member.clone())
            .collect()
    }

    /// Per checked member, the `cargo test` flags naming the crate-targets
    /// the base check compiled: a member's broken integration test is not
    /// selected, so it never silences its crate. Doctests are out, since the
    /// check that judges the patch never compiled them.
    fn suite_selection(&self) -> Vec<(String, Vec<String>)> {
        let unchecked = self.unchecked();
        let mut out = Vec::new();
        for (member, targets) in &self.targets {
            if unchecked.contains(member) {
                continue;
            }
            let mut args: Vec<String> = Vec::new();
            for (kind, name) in targets {
                let key = (member.clone(), kind.clone(), name.clone());
                match kind.as_str() {
                    _ if self.failed.contains(&key) => {}
                    "lib" => args.push("--lib".into()),
                    // the flag is spelled the kind
                    "bin" | "test" => args.extend([format!("--{kind}"), name.clone()]),
                    _ => {}
                }
            }
            if !args.is_empty() {
                out.push((member.clone(), args));
            }
        }
        out
    }
}

/// Every crate-target a `--message-format=json` run reported an error in.
fn cargo_failures(stdout: &str, member_of: &BTreeMap<PathBuf, String>) -> BTreeSet<Crate> {
    let mut out = BTreeSet::new();
    for line in stdout.lines().filter(|l| l.starts_with('{')) {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if row["reason"] != "compiler-message" || row["message"]["level"] != "error" {
            continue;
        }
        let manifest = PathBuf::from(row["manifest_path"].as_str().unwrap_or_default());
        let home = manifest.parent().map(real).unwrap_or_default();
        let named = |at: &Value| at.as_str().unwrap_or_default().to_string();
        let member = member_of.get(&home).cloned().unwrap_or_else(|| "?".into());
        out.insert((
            member,
            named(&row["target"]["kind"][0]),
            named(&row["target"]["name"]),
        ));
    }
    out
}

/// The pre-patch reading, taken on the tree the patch will be applied to:
/// an error already there is the host's, never the emitter's.
fn cargo_base(root: &Path, env: &[(String, String)]) -> Result<CargoBase> {
    let argv = [
        "metadata",
        "--no-deps",
        "--offline",
        "--format-version",
        "1",
    ];
    let doc: Value =
        serde_json::from_slice(&cargo(root, &argv, env)?.stdout).context("cargo metadata")?;
    let (mut member_of, mut targets) = (BTreeMap::new(), BTreeMap::new());
    for p in doc["packages"].as_array().into_iter().flatten() {
        let name = p["name"].as_str().unwrap_or_default().to_string();
        let manifest = PathBuf::from(p["manifest_path"].as_str().unwrap_or_default());
        if let Some(home) = manifest.parent() {
            member_of.insert(real(home), name.clone());
        }
        let mut kinds = Vec::new();
        for t in p["targets"].as_array().into_iter().flatten() {
            let named = t["name"].as_str().unwrap_or_default().to_string();
            for k in t["kind"].as_array().into_iter().flatten() {
                kinds.push((k.as_str().unwrap_or_default().to_string(), named.clone()));
            }
        }
        targets.insert(name, kinds);
    }
    let failed = cargo_failures(&text(&cargo(root, &CHECK, env)?.stdout), &member_of);
    Ok(CargoBase {
        member_of,
        targets,
        failed,
    })
}

static TEST_RESULT: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| pattern(r"(?m)^test result: \w+\. (\d+) passed; (\d+) failed"));

/// The Rust target's suite: the patched tree's `cargo check` adds no error
/// the base check did not have, then `cargo test` runs every crate-target
/// that check compiled.
fn cargo_verdict(
    root: &Path,
    base: &CargoBase,
    env: &[(String, String)],
) -> Result<(Fields, bool)> {
    let after = cargo_failures(&text(&cargo(root, &CHECK, env)?.stdout), &base.member_of);
    let new: Vec<String> = after
        .difference(&base.failed)
        .map(|(m, k, t)| format!("{m}: {k} {t}"))
        .collect();
    let mut out = Fields::new();
    put(&mut out, "unchecked_members", json!(base.unchecked()));
    put(&mut out, "cargo_check_new_errors", json!(new));
    if !new.is_empty() {
        put(
            &mut out,
            "suite",
            "not run (the patched tree fails cargo check)",
        );
        return Ok((out, false));
    }
    let selection = base.suite_selection();
    let (mut passed, mut failed, mut broke) = (0u64, 0u64, Vec::new());
    for (member, flags) in &selection {
        let mut args: Vec<&str> = vec!["test", "--offline", "--no-fail-fast", "-p", member];
        args.extend(flags.iter().map(String::as_str));
        let run = cargo(root, &args, env)?;
        for m in TEST_RESULT.captures_iter(&text(&run.stdout)) {
            passed += m[1].parse::<u64>().unwrap_or(0);
            failed += m[2].parse::<u64>().unwrap_or(0);
        }
        if !run.status.success() {
            broke.push(member.clone());
        }
    }
    let mut line = format!(
        "{passed} passed, {failed} failed over {} members",
        selection.len()
    );
    if !broke.is_empty() {
        line += &format!("; cargo test exited non-zero for {broke:?}");
    }
    put(&mut out, "suite", line);
    Ok((out, broke.is_empty()))
}

// --- one tree's receipt -------------------------------------------------------

/// One tree's fix receipt: a full `sightline fix` run and what its patch
/// claims.
pub fn check_repo(t: &Target, out_dir: &Path, allow_worktree: bool) -> Result<Fields> {
    let mut r = Fields::new();
    let sha = worktree::head(&t.root);
    put(&mut r, "repo", t.name.clone());
    put(&mut r, "sha", sha[..12.min(sha.len())].to_string());
    let patch_path = out_dir.join(format!("{}.patch", t.name));
    if heal_left_patch(&t.root, &patch_path)? {
        put(&mut r, "healed", "a previous run's patch was reversed");
    }
    let (root, out) = (
        t.root.display().to_string(),
        patch_path.display().to_string(),
    );
    let done = corpus::sightline(t, &["fix", &root, "--out", &out])?;
    if !done.status.success() {
        put(
            &mut r,
            "error",
            format!("fix failed: {}", tail(&text(&done.stderr))),
        );
        return Ok(r);
    }
    let diff = std::fs::read_to_string(&patch_path)?;
    let (patched, rels) = (patched_findings(&diff), patched_rels(&diff));
    let mut by_rule: BTreeMap<&str, usize> = BTreeMap::new();
    for (rule, _) in &patched {
        *by_rule.entry(rule.as_str()).or_insert(0) += 1;
    }
    put(&mut r, "patched_findings", json!(patched.len()));
    put(&mut r, "by_rule", json!(by_rule));
    put(&mut r, "patched_files", json!(rels.len()));
    if rels.is_empty() {
        put(&mut r, "verdict", "no patches emitted");
        // the flow ends before any apply
        put(&mut r, "tree_clean_after", true);
        return Ok(r);
    }

    let mut status = vec!["status", "--porcelain", "--"];
    status.extend(rels.iter().map(String::as_str));
    let dirty = text(&git(&t.root, &status, None)?.stdout);
    if !dirty.trim().is_empty() {
        if !allow_worktree {
            put(
                &mut r,
                "error",
                format!("patched files locally modified:\n{dirty}"),
            );
            return Ok(r);
        }
        // never touch a dirty live tree: redo the check in a detached
        // worktree at HEAD, discarded afterward
        let held = worktree::add(&t.root)?;
        let named = out_dir.join(format!("{}.toml", t.name));
        let here = t.in_worktree(held.path.as_std_path(), Some(&named))?;
        let mut sub = check_repo(&here, out_dir, false)?;
        put(
            &mut sub,
            "note",
            "live tree dirty: checked a worktree at HEAD",
        );
        return Ok(sub);
    }

    let base = match t.lang.as_str() {
        "rs" => Some(cargo_base(&t.root, &t.env)?), // before the patch
        _ => None,
    };
    let raw = diff.as_bytes();
    let check = git(&t.root, &["apply", "--check", "-"], Some(raw))?;
    put(&mut r, "git_apply_check", check.status.success());
    if !check.status.success() {
        put(&mut r, "error", tail(&text(&check.stderr)));
        return Ok(r);
    }
    if !git(&t.root, &["apply", "-"], Some(raw))?.status.success() {
        put(&mut r, "error", "apply failed after successful --check");
        return Ok(r);
    }
    let judged = judge(t, &patched, base.as_ref(), &mut r);
    let back = git(&t.root, &["apply", "-R", "-"], Some(raw))?;
    put(&mut r, "reverted", back.status.success());
    let residue = text(&git(&t.root, &status, None)?.stdout);
    put(&mut r, "tree_clean_after", residue.trim().is_empty());
    judged?;
    // A suite red on the patched tree indicts the patch only when the
    // unpatched tree is green. The reverted tree is the baseline, run only
    // on a failure, so the green path pays one suite run.
    if t.lang != "rs"
        && r.get("verdict").and_then(Value::as_str) == Some("FAIL")
        && r.get("still_reported_post_apply")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        && back.status.success()
    {
        let (unpatched, green) = pytest_verdict(t)?;
        if !green {
            if let Some(line) = unpatched.get("suite") {
                put(&mut r, "suite_unpatched", line.clone());
            }
            put(
                &mut r,
                "suite_note",
                "red at the pin, patched and unpatched alike: judged on the re-audit alone",
            );
            put(&mut r, "verdict", "PASS");
        }
    }
    Ok(r)
}

/// The patched tree's two questions, between the apply and the revert: does
/// the re-audit still report a patched finding, and does the suite pass.
fn judge(
    t: &Target,
    patched: &BTreeSet<(String, String)>,
    base: Option<&CargoBase>,
    r: &mut Fields,
) -> Result<()> {
    let reported = post_audit_pairs(t)?;
    let leftover: Vec<String> = patched
        .intersection(&reported)
        .take(10)
        .map(|(rule, cause)| format!("#{rule} {cause}"))
        .collect();
    put(r, "still_reported_post_apply", json!(leftover));
    let (fields, suite_ok) = match base {
        Some(base) => cargo_verdict(&t.root, base, &t.env)?,
        None => pytest_verdict(t)?,
    };
    r.extend(fields);
    let passed = leftover.is_empty() && suite_ok;
    put(r, "verdict", if passed { "PASS" } else { "FAIL" });
    Ok(())
}

pub fn main(args: &[&str]) -> Result<u8> {
    let named: Vec<&str> = args
        .iter()
        .copied()
        .filter(|a| !a.starts_with("--"))
        .collect();
    let (out_dir, names) = match named.split_first() {
        Some((dir, rest)) => (PathBuf::from(dir), rest.to_vec()),
        None => (workspace_root().join("corpus/results"), Vec::new()),
    };
    std::fs::create_dir_all(&out_dir)?;
    // the Python ladder by default: a Rust target's suite is a cargo build,
    // so it is asked for by name
    let targets: Vec<Target> = match names.is_empty() {
        true => corpus::targets(Some("py"), None)?,
        false => names
            .iter()
            .map(|n| corpus::get(n))
            .collect::<Result<_>>()?,
    };
    let (mut ok, mut results) = (true, Vec::new());
    for t in &targets {
        let mut r = check_repo(t, &out_dir, true)?;
        r.entry("verdict").or_insert_with(|| "ERROR".into());
        println!("{}", serde_json::to_string_pretty(&r)?);
        ok &= matches!(
            r["verdict"].as_str().unwrap_or_default(),
            "PASS" | "no patches emitted"
        );
        // a flow that ended before the apply writes neither key, and a
        // missing one is not a failure
        let held = |key| r.get(key) != Some(&Value::Bool(false));
        ok &= held("tree_clean_after") && held("reverted");
        results.push(Value::Object(r));
    }
    let receipt = serde_json::to_string_pretty(&results)? + "\n";
    std::fs::write(out_dir.join("fix_check.json"), receipt)?;
    println!("coverage, verified findings per rule:");
    for r in &results {
        let per: Vec<String> = r["by_rule"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(rule, n)| format!("#{rule} {n}"))
            .collect();
        let shown = if per.is_empty() {
            "none".into()
        } else {
            per.join(", ")
        };
        let (repo, n) = (&r["repo"], &r["patched_findings"]);
        println!("  {}: {n} ({shown})", repo.as_str().unwrap_or_default());
    }
    println!("fix check: {}", if ok { "PASS" } else { "FAIL" });
    Ok(u8::from(!ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    // the fixture manifest testkit already writes: one home
    use sightline_testkit::rs_fixtures::member;

    const KEEP: &str = "pub fn keep() -> u32 {\n    1\n}\n";
    const DEAD: &str = "pub fn dead() -> u32 {\n    2\n}\n";

    #[test]
    fn the_headers_name_the_findings_and_the_files_the_patch_touches() {
        let diff = "# sightline-fix: 32 dead-symbol:good::dead\r\n\
                    # sightline-fix: 5 lift:m.f\n\
                    --- a/good/src/lib.rs\n+++ b/good/src/lib.rs\n\
                    --- a/m.py\n+++ b/m.py\n";
        assert_eq!(
            patched_findings(diff),
            [
                ("32".to_string(), "dead-symbol:good::dead".to_string()),
                ("5".to_string(), "lift:m.f".to_string()),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(patched_rels(diff), ["good/src/lib.rs", "m.py"]);
    }

    /// A member whose lib fails the base check is unchecked and enters
    /// neither the suite nor the judgement; a member whose only failure is a
    /// test keeps its other targets.
    #[test]
    fn the_base_check_decides_what_the_suite_selects() {
        let base = CargoBase {
            member_of: BTreeMap::new(),
            targets: [
                (
                    "good".to_string(),
                    vec![
                        ("lib".to_string(), "good".to_string()),
                        ("test".to_string(), "it".to_string()),
                    ],
                ),
                (
                    "broken".to_string(),
                    vec![("lib".to_string(), "broken".to_string())],
                ),
            ]
            .into_iter()
            .collect(),
            failed: [
                (
                    "broken".to_string(),
                    "lib".to_string(),
                    "broken".to_string(),
                ),
                ("good".to_string(), "test".to_string(), "it".to_string()),
            ]
            .into_iter()
            .collect(),
        };

        assert_eq!(base.unchecked(), ["broken".to_string()].into());
        assert_eq!(
            base.suite_selection(),
            [("good".to_string(), vec!["--lib".to_string()])]
        );
    }

    /// A `--message-format=json` error line names its member through the
    /// manifest directory; a warning is not a failure.
    #[test]
    fn a_compiler_error_is_keyed_by_member_kind_and_target() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("good");
        std::fs::create_dir_all(&home).unwrap();
        let manifest = home.join("Cargo.toml");
        std::fs::write(&manifest, member("good", "")).unwrap();
        let member_of: BTreeMap<PathBuf, String> = [(real(&home), "good".to_string())].into();
        let line = json!({
            "reason": "compiler-message",
            "manifest_path": manifest.to_string_lossy(),
            "message": {"level": "error"},
            "target": {"kind": ["test"], "name": "it"},
        })
        .to_string();

        assert_eq!(
            cargo_failures(&format!("{line}\nnot json\n"), &member_of),
            [("good".to_string(), "test".to_string(), "it".to_string())].into()
        );
        let warn = line.replace("\"error\"", "\"warning\"");
        assert!(cargo_failures(&warn, &member_of).is_empty());
    }

    fn bare(root: &Path, config: Option<PathBuf>) -> Target {
        Target {
            name: "t".to_string(),
            url: String::new(),
            root: root.to_path_buf(),
            config,
            lang: "py".to_string(),
            role: "mid".to_string(),
            pin: None,
            env: Vec::new(),
        }
    }

    #[test]
    fn a_tree_with_no_tests_directory_has_no_runnable_suite() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m.py"), KEEP).unwrap();
        let t = bare(dir.path(), None);
        assert!(target_suite(&t).is_none());
        let (fields, ok) = pytest_verdict(&t).unwrap();
        assert!(ok);
        assert_eq!(fields["suite"], "not runnable (no repo venv+pytest+tests)");
    }

    /// A worktree has no `.venv`, so the suite takes the interpreter its
    /// config names, the one `worktree_config` wrote there.
    #[test]
    fn a_worktree_suite_takes_the_interpreter_its_config_names() {
        let dir = tempfile::tempdir().unwrap();
        let (tree, venv) = (dir.path().join("tree"), dir.path().join("live/.venv"));
        let bin = venv.join(if cfg!(windows) { "Scripts" } else { "bin" });
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let exe = bin.join(if cfg!(windows) {
            "python.exe"
        } else {
            "python"
        });
        std::fs::write(&exe, "").unwrap();
        let config = dir.path().join("c.toml");
        let named = venv.to_string_lossy().replace('\\', "/");
        std::fs::write(
            &config,
            format!("[tool.sightline]\npython-env = \"{named}\"\n"),
        )
        .unwrap();

        assert_eq!(suite_python(&bare(&tree, Some(config))), Some(exe));
        // no config and no venv is no interpreter
        assert_eq!(suite_python(&bare(&tree, None)), None);
    }

    /// The test-result lines of several members sum into one count.
    #[test]
    fn the_suite_line_sums_every_members_test_result() {
        let stdout = "test result: ok. 3 passed; 0 failed; 1 ignored\n\
                      test result: FAILED. 1 passed; 2 failed; 0 ignored\n";
        let totals: Vec<(u64, u64)> = TEST_RESULT
            .captures_iter(stdout)
            .map(|m| (m[1].parse().unwrap(), m[2].parse().unwrap()))
            .collect();
        assert_eq!(totals, [(3, 0), (1, 2)]);
    }

    // --- the cargo half, against a real fixture workspace ---------------------

    /// A two-member workspace whose `broken` member cannot compile: the
    /// host's own failure, which no patch is judged for. No dependency, so
    /// no run touches the network.
    fn cargo_workspace(dir: &Path) -> PathBuf {
        let root = dir.join("ws");
        for (rel, body) in [
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"good\", \"broken\"]\nresolver = \"2\"\n".to_string(),
            ),
            ("good/Cargo.toml", member("good", "")),
            ("good/src/lib.rs", format!("{KEEP}\n{DEAD}")),
            (
                "good/tests/it.rs",
                "#[test]\nfn works() {\n    assert_eq!(good::keep(), 1);\n}\n".to_string(),
            ),
            ("broken/Cargo.toml", member("broken", "")),
            (
                "broken/src/lib.rs",
                "use std::nope::Missing;\npub fn bad() -> u32 { 1 }\n".to_string(),
            ),
        ] {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).unwrap();
            std::fs::write(path, body).unwrap();
        }
        root
    }

    fn fixture_env(dir: &Path) -> [(String, String); 1] {
        [(
            "SIGHTLINE_CARGO_TARGET".to_string(),
            dir.join("target").to_string_lossy().into_owned(),
        )]
    }

    /// The base check names `broken` unchecked and selects `good`'s lib and
    /// its integration test (`test_fix_check.py`'s passing case).
    #[test]
    #[ignore = "runs cargo"]
    fn the_base_check_names_the_member_the_host_cannot_compile() {
        let dir = tempfile::tempdir().unwrap();
        let root = cargo_workspace(dir.path());
        let env = fixture_env(dir.path());

        let base = cargo_base(&root, &env).unwrap();

        assert_eq!(base.unchecked(), ["broken".to_string()].into());
        assert_eq!(
            base.suite_selection(),
            [(
                "good".to_string(),
                vec!["--lib".to_string(), "--test".to_string(), "it".to_string()]
            )]
        );
        let (fields, ok) = cargo_verdict(&root, &base, &env).unwrap();
        assert!(ok, "{fields:?}");
        assert_eq!(fields["cargo_check_new_errors"], json!([]));
        let suite = fields["suite"].as_str().unwrap();
        assert!(suite.starts_with("1 passed, 0 failed"), "{suite}");
    }

    /// A deletion its own test needs fails on the error it added
    /// (`test_fix_check.py`'s failing case).
    #[test]
    #[ignore = "runs cargo"]
    fn a_deletion_its_own_test_needs_fails_on_the_error_it_added() {
        let dir = tempfile::tempdir().unwrap();
        let root = cargo_workspace(dir.path());
        let env = fixture_env(dir.path());
        let base = cargo_base(&root, &env).unwrap();
        std::fs::write(root.join("good/src/lib.rs"), DEAD).unwrap();

        let (fields, ok) = cargo_verdict(&root, &base, &env).unwrap();

        assert!(!ok);
        assert_eq!(fields["cargo_check_new_errors"], json!(["good: test it"]));
        assert_eq!(
            fields["suite"],
            "not run (the patched tree fails cargo check)"
        );
    }
}
