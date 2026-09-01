//! `cargo xtask corpus`: audit every tree of `crates/xtask/corpus.toml`,
//! record walls, prove two runs identical byte for byte, dump fire rates,
//! then check each clean pole's polarity (`scripts/corpus_run.py` and
//! `scripts/corpus_check.py`). The out dir is cleared first, so every file
//! in it is this run's.
//!
//! The table and the binary's command line live here; the seam a detached
//! worktree needs is `worktree.rs`. One reader and one writer of a corpus
//! tree at a time: `fix-check` patches those trees and reverts them, and a
//! corpus audit taken while it runs reads the applied fixes as vanished
//! findings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::paths::{drop_toolchain, siblings, workspace_root};
use crate::worktree;

/// The corpus table, embedded so no run has to find the file.
const TABLE: &str = include_str!("../corpus.toml");

/// One tree to audit: where it lives, its config, the language stack it
/// exercises, its role in that language's ladder, the commit the recorded
/// measurements were taken at, and the environment an audit of it needs. The
/// environment is empty for a live corpus tree and filled by `in_worktree`
/// for a detached copy.
#[derive(Clone, Debug)]
pub struct Target {
    pub name: String,
    /// the public repository the tree is cloned from; empty outside the table
    pub url: String,
    pub root: PathBuf,
    pub config: Option<PathBuf>,
    pub lang: String,
    pub role: String,
    /// `None` for a tree outside the table, which no measurement pins
    pub pin: Option<String>,
    pub env: Vec<(String, String)>,
}

impl Target {
    /// The suffix of the language under test, read off the seam's own
    /// `Language` records so the table has one home.
    pub fn suffix(&self) -> &'static str {
        use sightline_core::lang::Language;
        match self.lang.as_str() {
            "rs" => sightline_rs_rules::stack::RsLanguage {
                ids_by_slug: Default::default(),
            }
            .suffix(),
            _ => sightline_py_rules::stack::PyLanguage::new(Default::default()).suffix(),
        }
    }

    /// A tree outside the table: `gate-bench` and the own gate name a root
    /// and at most a config, and read no ladder role.
    pub fn bare(root: &str, config: Option<&str>) -> Target {
        Target {
            name: "bare".to_string(),
            url: String::new(),
            root: PathBuf::from(root),
            config: config.map(PathBuf::from),
            lang: String::new(),
            role: String::new(),
            pin: None,
            env: Vec::new(),
        }
    }

    /// This target as a detached worktree audits it. HEAD alone is not the
    /// tree the walls were measured on: a Cargo root keeps its lockfile and
    /// its build directory outside the commit, a Python root its
    /// interpreter. `config_out` is where the rewritten config is written.
    pub fn in_worktree(&self, tree: &Path, config_out: Option<&Path>) -> Result<Target> {
        let config = match config_out {
            Some(out) => worktree::config(&self.root, self.config.as_deref(), out)?,
            None => self.config.clone(),
        };
        Ok(Target {
            root: tree.to_path_buf(),
            config,
            env: worktree::env(&self.root, tree)?,
            ..self.clone()
        })
    }
}

/// The whole table, in ladder order.
pub fn table() -> Result<Vec<Target>> {
    let doc: toml::Table = TABLE.parse().context("crates/xtask/corpus.toml")?;
    let rows = doc
        .get("repo")
        .and_then(toml::Value::as_array)
        .context("corpus.toml holds no [[repo]] rows")?;
    let root = workspace_root();
    let mut out = Vec::new();
    for row in rows {
        let at = |key: &str| row.get(key).and_then(toml::Value::as_str);
        let field = |key: &str| {
            at(key)
                .map(str::to_string)
                .with_context(|| format!("a corpus.toml row holds no {key}"))
        };
        let name = field("name")?;
        out.push(Target {
            root: siblings().join(&name),
            url: field("url")?,
            config: at("config").map(|rel| root.join(rel)),
            lang: field("lang")?,
            role: field("role")?,
            pin: Some(field("pin")?),
            env: Vec::new(),
            name,
        });
    }
    Ok(out)
}

/// The table, filtered: `targets(Some("rs"), Some("clean"))` is the Rust
/// clean pole.
pub fn targets(lang: Option<&str>, role: Option<&str>) -> Result<Vec<Target>> {
    Ok(table()?
        .into_iter()
        .filter(|t| lang.is_none_or(|l| t.lang == l) && role.is_none_or(|r| t.role == r))
        .collect())
}

pub fn get(name: &str) -> Result<Target> {
    table()?
        .into_iter()
        .find(|t| t.name == name)
        .with_context(|| format!("not a corpus repo: {name}"))
}

// --- driving the binary -------------------------------------------------------

/// The release binary every subcommand drives.
pub fn binary() -> Result<PathBuf> {
    let name = if cfg!(windows) {
        "sightline.exe"
    } else {
        "sightline"
    };
    let path = workspace_root().join("target/release").join(name);
    if !path.is_file() {
        let build = "run `cargo build --release -p sightline`";
        bail!("{} is not built; {build}", path.display());
    }
    Ok(path)
}

/// One `sightline` invocation over a target: its config appended and its
/// environment applied. The child drops this workspace's toolchain pin, so
/// a Rust tool it spawns takes the audited tree's. Every subcommand that
/// drives the binary builds its command here.
pub fn command(t: &Target, args: &[&str]) -> Result<Command> {
    let mut cmd = Command::new(binary()?);
    cmd.args(args).envs(t.env.iter().map(|(k, v)| (k, v)));
    if let Some(config) = &t.config {
        cmd.arg("--config").arg(config);
    }
    drop_toolchain(&mut cmd);
    Ok(cmd)
}

pub fn sightline(t: &Target, args: &[&str]) -> Result<Output> {
    command(t, args)?.output().context("running sightline")
}

/// One audit to `out`, and its wall. A torn oracle answer in the header
/// fails the run: a finding may have vanished. A checkout missing the tree
/// is told where to clone it: every corpus reader funnels through here.
pub fn audit(t: &Target, out: &Path, threads: Option<usize>) -> Result<f64> {
    if !t.root.is_dir() {
        bail!(
            "corpus tree {} is not in this checkout; run `git clone {} {}`",
            t.name,
            t.url,
            t.root.display()
        );
    }
    let root = t.root.display().to_string();
    let mut args = vec!["audit", &root, "--json"];
    let spelled;
    if let Some(n) = threads {
        spelled = n.to_string();
        args.extend(["--threads", &spelled]);
    }
    let started = Instant::now();
    let done = sightline(t, &args)?;
    let wall = started.elapsed().as_secs_f64();
    let at = t.root.display();
    if !done.status.success() {
        bail!(
            "audit failed for {at}:\n{}",
            tail(&String::from_utf8_lossy(&done.stderr))
        );
    }
    std::fs::write(out, &done.stdout)?;
    let faults = oracle_faults(out)?;
    if !faults.is_empty() {
        bail!(
            "audit of {at} reports oracle faults:\n{}",
            faults.join("\n")
        );
    }
    Ok(wall)
}

/// Header notes naming a torn oracle answer: a run whose findings may have
/// vanished, so every reader of an audit fails on it.
pub fn oracle_faults(audit_json: &Path) -> Result<Vec<String>> {
    let doc = read_json(audit_json)?;
    let notes = doc["provenance"]["notes"].as_array().into_iter().flatten();
    Ok(notes
        .filter_map(Value::as_str)
        .filter(|n| n.starts_with("oracle fault"))
        .map(str::to_string)
        .collect())
}

/// The last 40 lines of a captured stream: what a failure prints.
pub fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(40)..].join("\n")
}

/// One JSON document off disk.
pub fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

// --- polarity ----------------------------------------------------------------

/// The blocking axis on a clean pole: `baseline` writes the tree's own
/// counts, and `gate --full` against them blocks nothing. This subsumes the
/// old GATE reading: a GATE rule is never baselined, so one firing on the
/// pole fails here too, and a count that moves between the two runs is a
/// nondeterminism this catches as a block. The caller hands a worktree, so
/// the baseline file never dirties the pinned tree.
pub fn polarity(t: &Target) -> Result<u8> {
    let root = t.root.display().to_string();
    let wrote = sightline(t, &["baseline", &root])?;
    if !wrote.status.success() {
        bail!(
            "baseline failed on {root}:\n{}",
            tail(&String::from_utf8_lossy(&wrote.stderr))
        );
    }
    let gated = sightline(t, &["gate", &root, "--full"])?;
    print!("{}", String::from_utf8_lossy(&gated.stdout));
    if !gated.status.success() {
        bail!(
            "the pole blocks: gate --full on {root} exited {:?}\n{}",
            gated.status.code(),
            tail(&String::from_utf8_lossy(&gated.stderr))
        );
    }
    println!("corpus check passed: the pole baselines and gate --full blocks nothing");
    Ok(0)
}

// --- the run ------------------------------------------------------------------

fn counts(doc: &Value) -> BTreeMap<i64, usize> {
    let mut out = BTreeMap::new();
    for f in doc["findings"].as_array().into_iter().flatten() {
        let id = f["rule"].as_str().unwrap_or("0").parse().unwrap_or(0);
        *out.entry(id).or_insert(0) += 1;
    }
    out
}

/// Byte identity first, which is the receipt. Per-rule deltas and the
/// provenance blocks only to explain a difference.
fn print_deltas(before: &Path, after: &Path) -> Result<()> {
    if !before.is_file() {
        println!("  deltas: no earlier capture of {}", before.display());
        return Ok(());
    }
    if std::fs::read(before)? == std::fs::read(after)? {
        println!("  identical to {}", before.display());
        return Ok(());
    }
    let (b, a) = (read_json(before)?, read_json(after)?);
    let (bc, ac) = (counts(&b), counts(&a));
    let union: std::collections::BTreeSet<&i64> = bc.keys().chain(ac.keys()).collect();
    let moved: Vec<String> = union
        .into_iter()
        .filter(|r| bc.get(r) != ac.get(r))
        .map(|r| {
            format!(
                "{r}: {} -> {}",
                bc.get(r).unwrap_or(&0),
                ac.get(r).unwrap_or(&0)
            )
        })
        .collect();
    let shown = if moved.is_empty() {
        "none".to_string()
    } else {
        moved.join(", ")
    };
    println!("  deltas vs {}: {shown}", before.display());
    for key in ["oracle", "notes"] {
        let (was, now) = (&b["provenance"][key], &a["provenance"][key]);
        if was != now {
            println!("  provenance.{key}: {was} -> {now}");
        }
    }
    Ok(())
}

/// `--name=value`, the spelling `corpus_run.py` takes.
fn joined<'a>(args: &'a [&'a str], prefix: &str) -> Option<&'a str> {
    args.iter()
        .find_map(|a| a.strip_prefix(prefix))
        .filter(|v| !v.is_empty())
}

pub fn main(args: &[&str]) -> Result<u8> {
    let out_dir = match args.iter().find(|a| !a.starts_with("--")) {
        Some(dir) => PathBuf::from(dir),
        None => workspace_root().join("corpus/results"),
    };
    let repeat = args.contains(&"--repeat-for-determinism");
    let diff_against = joined(args, "--diff-against=").map(PathBuf::from);
    let lang = joined(args, "--lang=");
    let corpus = targets(lang, None)?;
    if corpus.is_empty() {
        bail!("no corpus repo of language {lang:?}");
    }
    if diff_against.as_ref().is_some_and(|d| d == &out_dir) {
        bail!("--diff-against names an earlier capture, not the out dir");
    }
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir)?;

    let mut walls = serde_json::Map::new();
    for t in &corpus {
        let config_out = out_dir.join(format!("{}.worktree.toml", t.name));
        let (here, held) = worktree::audited_tree(t, Some(&config_out))?;
        let note = if held.is_some() {
            " (dirty live tree: audited a worktree at HEAD)"
        } else {
            ""
        };
        let out = out_dir.join(format!("{}.json", t.name));
        let wall = audit(&here, &out, None)?;
        let sha = worktree::head(&here.root);
        let row = serde_json::json!({
            "wall_s": (wall * 10.0).round() / 10.0,
            "sha": sha, "lang": t.lang, "role": t.role,
        });
        walls.insert(t.name.clone(), row);
        let short = &sha[..12.min(sha.len())];
        println!(
            "{}: {wall:.1}s @ {short}{note} -> {}",
            t.name,
            out.display()
        );
        if repeat {
            // JSON alone: the text render's determinism is a binary test.
            // All cores against one: an answer that varies under load
            // varies here.
            let second = out_dir.join(format!("{}.run2.json", t.name));
            audit(&here, &second, Some(1))?;
            let identical = std::fs::read(&out)? == std::fs::read(&second)?;
            let name = &t.name;
            println!("{name}: double run identical byte for byte (threads all vs 1): {identical}");
            if !identical {
                // the second run stays: a flake no one can diff is a trap
                print_deltas(&out, &second)?;
                return Ok(1);
            }
            std::fs::remove_file(&second)?;
        }
        drop(held);
        let doc = read_json(&out)?;
        println!("  fire rates: {:?}", counts(&doc));
        println!("  notes: {}", doc["provenance"]["notes"]);
        if let Some(earlier) = &diff_against {
            print_deltas(&earlier.join(format!("{}.json", t.name)), &out)?;
        }
    }
    std::fs::write(
        out_dir.join("walls.json"),
        serde_json::to_string_pretty(&Value::Object(walls))? + "\n",
    )?;
    let mut code = 0;
    for t in corpus.iter().filter(|t| t.role == "clean") {
        let held = worktree::add(&t.root)?;
        let here = t.in_worktree(
            held.path.as_std_path(),
            Some(&out_dir.join(format!("{}.polarity.toml", t.name))),
        )?;
        code |= polarity(&here)?;
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_six_corpus_trees_in_ladder_order() {
        let all = table().unwrap();
        assert_eq!(
            all.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            [
                "powertools-lambda-python",
                "sqlglot",
                "merged-calculator",
                "doxx",
                "turmoil",
                "salvo"
            ]
        );
        assert_eq!(targets(Some("rs"), Some("clean")).unwrap()[0].name, "doxx");
        assert_eq!(targets(Some("py"), None).unwrap().len(), 3);
        // the clean pole audits with the tree's own settings
        assert!(all[0].config.is_none());
        assert!(
            all[1]
                .config
                .as_ref()
                .unwrap()
                .ends_with("corpus/sqlglot.toml")
        );
        assert!(get("nope").is_err());
    }

    /// Every row names the public repository it is cloned from, which a
    /// checkout missing the tree is told to clone.
    #[test]
    fn every_row_names_its_clone_url() {
        for t in table().unwrap() {
            assert!(t.url.starts_with("https://github.com/"), "{}", t.name);
        }
    }

    /// Every config the table names is a file of this workspace.
    #[test]
    fn every_named_config_exists() {
        for t in table().unwrap() {
            if let Some(config) = &t.config {
                assert!(config.is_file(), "{}", config.display());
            }
        }
    }
}
