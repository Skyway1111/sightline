//! The import fence: a rule reads facts and provers and nothing
//! else. Four checks, all of them over the two rules crates.
//!
//! 1. Direct dependencies through `cargo metadata` (never the transitive
//!    `cargo tree`) hold no parser or oracle crate.
//! 2. No source line of either rules crate names one of their paths.
//! 3. No provers crate re-exports one, which would smuggle it in.
//! 4. `cargo clippy` on both rules crates passes, so the `disallowed-methods`
//!    and `disallowed-types` lists beside their manifests bind.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::Value;

use crate::paths::workspace_root;

/// The crates a rule may never reach: the two parsers, the two oracles and
/// the manifest reader.
const FENCED: [&str; 5] = [
    "ruff_python_parser",
    "ty_",
    "ra_ap_",
    "cargo_metadata",
    "tree-sitter",
];

const RULES_CRATES: [&str; 2] = ["sightline-py-rules", "sightline-rs-rules"];
const PROVERS_CRATES: [&str; 2] = ["py-provers", "rs-provers"];

fn fenced_dep(name: &str) -> bool {
    let name = name.replace('-', "_");
    FENCED
        .iter()
        .any(|f| name == *f || name.starts_with(&f.replace('-', "_")))
}

/// Every `.rs` file under a directory, sorted.
fn sources(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// A fenced path in Rust source: a crate name at a word boundary, so
/// `tests_quality_` does not read as `ty_`.
fn path_probe() -> Regex {
    Regex::new(r"\b(ty_[a-z_]+|ruff_python_parser|ra_ap_[a-z_]+|cargo_metadata|tree_sitter)::")
        .expect("the fence probe compiles")
}

/// Check 1: the direct dependency lists.
fn direct_deps(root: &Path, faults: &mut Vec<String>) -> Result<usize> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("cargo metadata")?;
    if !out.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let meta: Value = serde_json::from_slice(&out.stdout)?;
    let mut checked = 0;
    for package in meta["packages"].as_array().into_iter().flatten() {
        let name = package["name"].as_str().unwrap_or_default();
        if !RULES_CRATES.contains(&name) {
            continue;
        }
        checked += 1;
        for dep in package["dependencies"].as_array().into_iter().flatten() {
            let dep_name = dep["name"].as_str().unwrap_or_default();
            let kind = dep["kind"].as_str().unwrap_or("normal");
            if kind != "dev" && fenced_dep(dep_name) {
                faults.push(format!("{name} depends on {dep_name}"));
            }
        }
    }
    if checked != RULES_CRATES.len() {
        bail!(
            "cargo metadata named {checked} of the {} rules crates",
            RULES_CRATES.len()
        );
    }
    Ok(checked)
}

/// Check 2: no source line of a rules crate names a fenced path.
fn source_paths(root: &Path, faults: &mut Vec<String>) -> Result<usize> {
    let probe = path_probe();
    let mut files = 0;
    for crate_name in RULES_CRATES {
        let dir = root
            .join("crates")
            .join(crate_name.trim_start_matches("sightline-"));
        for path in sources(&dir.join("src"))? {
            files += 1;
            let text = std::fs::read_to_string(&path)?;
            for (n, line) in text.lines().enumerate() {
                if let Some(hit) = probe.find(line) {
                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    faults.push(format!(
                        "{}:{} names {}",
                        rel.display(),
                        n + 1,
                        hit.as_str()
                    ));
                }
            }
        }
    }
    Ok(files)
}

/// Check 3: no provers crate re-exports a fenced path, which a rule could
/// then reach through its own dependency.
fn provers_reexports(root: &Path, faults: &mut Vec<String>) -> Result<usize> {
    let probe = path_probe();
    let mut lines = 0;
    for crate_name in PROVERS_CRATES {
        for path in sources(&root.join("crates").join(crate_name).join("src"))? {
            let text = std::fs::read_to_string(&path)?;
            for (n, line) in text.lines().enumerate() {
                if !line.trim_start().starts_with("pub use") {
                    continue;
                }
                lines += 1;
                if let Some(hit) = probe.find(line) {
                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    faults.push(format!(
                        "{}:{} re-exports {}",
                        rel.display(),
                        n + 1,
                        hit.as_str()
                    ));
                }
            }
        }
    }
    Ok(lines)
}

/// Check 4: the `disallowed-methods` and `disallowed-types` lists bind.
fn clippy_binds(root: &Path, faults: &mut Vec<String>) -> Result<()> {
    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(root).args(["clippy", "--quiet", "--lib"]);
    for crate_name in RULES_CRATES {
        cmd.args(["-p", crate_name]);
    }
    let out = cmd
        .args(["--", "-D", "warnings"])
        .output()
        .context("cargo clippy")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first = stderr
            .lines()
            .find(|l| l.starts_with("error"))
            .unwrap_or("clippy failed");
        faults.push(format!("clippy on the rules crates: {first}"));
    }
    Ok(())
}

pub fn main(_args: &[&str]) -> Result<u8> {
    let root = workspace_root();
    let mut faults = Vec::new();
    let crates = direct_deps(&root, &mut faults)?;
    println!("fence: {crates} rules crates, no fenced direct dependency");
    let files = source_paths(&root, &mut faults)?;
    println!("fence: {files} rules sources, no fenced path");
    let reexports = provers_reexports(&root, &mut faults)?;
    println!("fence: {reexports} provers `pub use` lines, none fenced");
    clippy_binds(&root, &mut faults)?;
    println!("fence: clippy holds the disallowed lists on both rules crates");
    for fault in &faults {
        println!("  FAULT {fault}");
    }
    if faults.is_empty() {
        println!("fence: holds");
        return Ok(0);
    }
    println!("fence: {} faults", faults.len());
    Ok(1)
}
