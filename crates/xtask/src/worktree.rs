//! What a detached worktree needs to audit like its live tree.
//!
//! HEAD alone is not the tree the corpus walls were measured on: a Cargo
//! root keeps its lockfile and its build directory outside the commit, and a
//! Python root its interpreter. One function per language. The guard itself
//! is `core::git::Worktree`, the one `debug dump` uses.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use camino::Utf8PathBuf;

pub use sightline_core::git::Worktree;

use crate::corpus::Target;

/// A path as camino spells it, without the `\\?\` prefix Windows
/// canonicalization adds.
pub fn utf8(path: &Path) -> Utf8PathBuf {
    let shown = path.to_string_lossy().into_owned();
    Utf8PathBuf::from(shown.strip_prefix(r"\\?\").unwrap_or(&shown))
}

fn real(path: &Path) -> Result<Utf8PathBuf> {
    Ok(utf8(&std::fs::canonicalize(path)?))
}

pub fn add(live: &Path) -> Result<Worktree> {
    Worktree::add(&utf8(live)).with_context(|| format!("git worktree add on {}", live.display()))
}

pub fn head(root: &Path) -> String {
    sightline_core::git::head_sha(&utf8(root))
}

pub fn dirty(root: &Path) -> bool {
    sightline_core::git::working_tree_dirty(&utf8(root))
}

/// A worktree has no `.venv`, and without the tree's interpreter the oracle
/// leaves third-party imports unresolved: the config with `python-env`
/// naming the live root's venv.
pub fn config(live: &Path, config: Option<&Path>, out: &Path) -> Result<Option<PathBuf>> {
    let venv = live.join(".venv");
    if !venv.is_dir() {
        return Ok(config.map(Path::to_path_buf));
    }
    let mut table = match config {
        Some(path) => std::fs::read_to_string(path)?
            .parse::<toml::Table>()?
            .get("tool")
            .and_then(|t| t.get("sightline")?.as_table().cloned())
            .unwrap_or_default(),
        None => toml::Table::new(),
    };
    let posix = real(&venv)?.into_string().replace('\\', "/");
    table.insert("python-env".into(), toml::Value::String(posix));
    let mut text = String::from("[tool.sightline]\n");
    for (key, value) in &table {
        if !value.is_table() {
            text += &format!("{key} = {}\n", serde_json::to_string(value)?);
        }
    }
    std::fs::write(out, text)?;
    Ok(Some(out.to_path_buf()))
}

/// The Cargo half, as environment: copy in the `Cargo.lock` the corpus trees
/// gitignore, since offline resolution without one starts from nothing, and
/// name the live root's build directory so the worktree pays that tree's
/// warm dependencies. Empty for a root with no manifest.
pub fn env(live: &Path, tree: &Path) -> Result<Vec<(String, String)>> {
    if !live.join("Cargo.toml").is_file() {
        return Ok(Vec::new());
    }
    let lock = live.join("Cargo.lock");
    if lock.is_file() {
        std::fs::copy(&lock, tree.join("Cargo.lock"))?;
    }
    Ok(vec![(
        "SIGHTLINE_CARGO_TARGET".to_string(),
        sightline_rs_provers::oracle::target_dir(&real(live)?, "", None).into_string(),
    )])
}

/// The SHA a receipt prints must describe the audited tree, so a dirty live
/// checkout is audited through a detached worktree at HEAD.
pub fn audited_tree(t: &Target, config_out: Option<&Path>) -> Result<(Target, Option<Worktree>)> {
    if !dirty(&t.root) {
        return Ok((t.clone(), None));
    }
    let held = add(&t.root)?;
    let here = t.in_worktree(held.path.as_std_path(), config_out)?;
    Ok((here, Some(held)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A worktree config names the live root's interpreter and keeps the
    /// source table's other keys, in posix spelling.
    #[test]
    fn a_worktree_config_names_the_live_venv() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live");
        std::fs::create_dir_all(live.join(".venv")).unwrap();
        let source = dir.path().join("in.toml");
        std::fs::write(&source, "[tool.sightline]\nexcludes = [\"vendor\"]\n").unwrap();
        let out = dir.path().join("out.toml");

        let written = config(&live, Some(&source), &out).unwrap();

        let text = std::fs::read_to_string(written.unwrap()).unwrap();
        assert!(text.contains("excludes = [\"vendor\"]"), "{text}");
        assert!(text.contains("python-env = "), "{text}");
        assert!(!text.contains('\\'), "{text}");
    }

    /// A root with no venv keeps the config it was given, so a Rust tree's
    /// worktree run reads the same file the live run reads.
    #[test]
    fn a_root_without_a_venv_keeps_its_config() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("in.toml");
        std::fs::write(&source, "[tool.sightline]\n").unwrap();
        assert_eq!(
            config(dir.path(), Some(&source), &dir.path().join("out.toml")).unwrap(),
            Some(source)
        );
    }

    #[test]
    fn a_root_with_no_manifest_asks_for_no_cargo_environment() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m.py"), "x = 1\n").unwrap();
        assert_eq!(env(dir.path(), dir.path()).unwrap(), []);
    }

    /// The Cargo half copies the lockfile in and names the live root's build
    /// directory.
    #[test]
    fn the_worktree_seam_copies_the_lockfile_and_names_the_live_build_dir() {
        let dir = tempfile::tempdir().unwrap();
        let (live, tree) = (dir.path().join("live"), dir.path().join("tree"));
        for at in [&live, &tree] {
            std::fs::create_dir_all(at).unwrap();
            std::fs::write(at.join("Cargo.toml"), "[package]\nname = \"solo\"\n").unwrap();
        }
        std::fs::write(live.join("Cargo.lock"), "# pinned\n").unwrap();

        let found = env(&live, &tree).unwrap();

        assert_eq!(
            std::fs::read_to_string(tree.join("Cargo.lock")).unwrap(),
            "# pinned\n"
        );
        let want =
            sightline_rs_provers::oracle::target_dir(&real(&live).unwrap(), "", None).into_string();
        assert_eq!(found, [("SIGHTLINE_CARGO_TARGET".to_string(), want)]);
    }
}
