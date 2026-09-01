//! `verify_worlds` (`rs/oracle.py`'s world half): overlays written into one
//! copy of the tree without `.git`, `target` and the config excludes, `cargo
//! check` per owning root, and the diagnostics each overlay adds over the
//! base check keyed by `RsDiag::key`. Every severity is passed through: a
//! filter by code disarms every veto that reads them.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::sync::Mutex;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use tempfile::TempDir;

use crate::oracle::cargo::CHECK;
use crate::oracle::{RsDiag, RsOracle};

/// a world is the tree without these
const SKIP: [&str; 2] = [".git", "target"];

/// The world tree, copied once per oracle, and what it currently overlays.
#[derive(Default)]
pub struct Worlds {
    tree: Mutex<Option<TempDir>>,
    overlaid: Mutex<IndexSet<String>>,
}

impl RsOracle {
    /// worlds: (id, {rel: full replacement content}). Per world, the
    /// diagnostics its overlay adds over the base check. One copy of the tree
    /// serves them all, so a world after the first pays only what its overlay
    /// changed. A world checks in the project root owning each file its
    /// overlay replaces, so an overlay across two roots is checked in both.
    pub fn verify_worlds(
        &self,
        worlds: &[(String, IndexMap<String, String>)],
    ) -> IndexMap<String, Vec<RsDiag>> {
        let mut out = IndexMap::new();
        if worlds.is_empty() || self.cargo.exe.is_none() {
            return out;
        }
        // one caller at a time: the overlay set is the tree's real state, so
        // the guard holds across the checks that read it
        let mut overlaid = self.worlds.overlaid.lock().unwrap();
        let base: HashSet<_> = self.diagnostics().iter().map(RsDiag::key).collect();
        for (wid, overlay) in worlds {
            let Some(tree) = self.world_tree() else {
                return out;
            };
            if self.lay(&tree, overlay, &mut overlaid).is_err() {
                return out;
            }
            let mut found = Vec::new();
            for project in self.owners(overlay) {
                let at = tree.join(self.home(&project));
                let label = format!("cargo check world {wid}");
                let ran = self.run(&CHECK, &at, &label, Some(&project));
                let Some((stdout, _)) = ran else { return out };
                let (rows, finished) = self.diags(&stdout, &at, &tree, false);
                if !finished && !rows.iter().any(|d| d.level == "error") {
                    self.fail(Some(&project), &format!("{label}: no build-finished"));
                    return out;
                }
                found.extend(rows);
            }
            // one record per fact: `--all-targets` compiles a lib and its test
            // target apart, so an error in the lib is reported under both
            let mut fresh = base.clone();
            out.insert(
                wid.clone(),
                found
                    .into_iter()
                    .filter(|d| fresh.insert(d.key()))
                    .collect(),
            );
        }
        out
    }

    /// The overlay in the world tree: the files the last world overlaid and
    /// this one does not are restored from the root first, then every file of
    /// this overlay is written with the bytes given.
    fn lay(
        &self,
        tree: &Utf8Path,
        overlay: &IndexMap<String, String>,
        held: &mut IndexSet<String>,
    ) -> io::Result<()> {
        let dropped = held.iter().filter(|rel| !overlay.contains_key(*rel));
        let mut stale: Vec<String> = dropped.cloned().collect();
        stale.sort();
        let mut rels: Vec<&String> = overlay.keys().collect();
        rels.sort();
        let restore = |rel: &String| fs::copy(self.root.join(rel), tree.join(rel)).map(|_| ());
        let write = |rel: &&String| fs::write(tree.join(rel), overlay[*rel].as_bytes());
        let laid = stale
            .iter()
            .try_for_each(restore)
            .and_then(|()| rels.iter().try_for_each(write));
        if let Err(e) = &laid {
            self.fail(None, &format!("world overlay: {e}"));
        }
        *held = overlay.keys().cloned().collect();
        laid
    }

    /// The project roots a world is checked in: the innermost root holding
    /// each file the overlay replaces, plus every root reading one of those.
    /// A file under no root is every root's, so an overlay the layout cannot
    /// place is verified against the whole tree rather than against nothing.
    fn owners(&self, overlay: &IndexMap<String, String>) -> Vec<Utf8PathBuf> {
        if self.roots.len() == 1 {
            return self.roots.clone();
        }
        let mut homes: Vec<(String, &Utf8PathBuf)> =
            self.roots.iter().map(|p| (self.home(p), p)).collect();
        homes.sort_by_key(|(home, _)| std::cmp::Reverse(home.len()));
        let mut found: HashSet<Utf8PathBuf> = HashSet::new();
        for rel in overlay.keys() {
            let holds = |(home, _): &&(String, &Utf8PathBuf)| {
                home.is_empty() || rel.starts_with(&format!("{home}/"))
            };
            let Some((_, owner)) = homes.iter().find(holds) else {
                return self.roots.clone();
            };
            found.insert((*owner).clone());
            found.extend(self.dependents().get(*owner).into_iter().flatten().cloned());
        }
        let picked: Vec<Utf8PathBuf> = self
            .roots
            .iter()
            .filter(|p| found.contains(*p))
            .cloned()
            .collect();
        if picked.is_empty() {
            self.roots.clone()
        } else {
            picked
        }
    }

    /// The tree without its history, its build output and the dirs the config
    /// excludes, copied once per oracle: a cargo build reads none of those,
    /// and a world that lost one fails closed.
    fn world_tree(&self) -> Option<Utf8PathBuf> {
        let mut held = self.worlds.tree.lock().unwrap();
        if held.is_none() {
            match self.copy_once() {
                Ok(dir) => *held = Some(dir),
                Err(why) => {
                    drop(held);
                    self.fail(None, &format!("world copy: {why}"));
                    return None;
                }
            }
        }
        let at = held.as_ref()?.path().to_path_buf();
        Some(Utf8PathBuf::from_path_buf(at).ok()?.join("tree"))
    }

    fn copy_once(&self) -> Result<TempDir, String> {
        let named = tempfile::Builder::new()
            .prefix("sightline-rs-world-")
            .tempdir();
        let dir = named.map_err(|e| e.to_string())?;
        let at = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .map_err(|p| format!("world path is not utf-8: {}", p.display()))?;
        self.copy_world(&self.root, &at.join("tree"), "")
            .map_err(|e| e.to_string())?;
        Ok(dir)
    }

    /// One directory of the copy. `base` is the source directory as a posix
    /// rel with its trailing slash, the spelling the excludes use.
    fn copy_world(&self, from: &Utf8Path, to: &Utf8Path, base: &str) -> io::Result<()> {
        fs::create_dir_all(to)?;
        for entry in from.read_dir_utf8()? {
            let entry = entry?;
            let name = entry.file_name().to_string();
            let rel = format!("{base}{name}");
            if SKIP.contains(&name.as_str()) || self.excludes.contains(&rel) {
                continue;
            }
            let (src, dst) = (entry.path(), to.join(&name));
            match src.is_dir() {
                true => self.copy_world(src, &dst, &format!("{rel}/"))?,
                false => drop(fs::copy(src, &dst)?),
            }
        }
        Ok(())
    }

    /// Drop the world tree; the passes hold no process of their own.
    pub fn close(&mut self) {
        self.worlds.tree.lock().unwrap().take();
    }
}
