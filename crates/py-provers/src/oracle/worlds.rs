//! The counterfactual worlds the oracle runs (codemap 5): every overlay
//! overridden on the one database, the check, the diagnostics the overlay
//! adds against the base pass.

use super::*;

/// Watched files past which a world takes the whole-project check instead of
/// one `check_file` each (codemap 5; phase 0b measured 4.41 s against 5.75 s
/// and 6.25 s for the pure arms).
const MAX_WATCHED: usize = 20;

impl Oracle {
    /// Per world, the diagnostics the overlays add against the base pass,
    /// keyed `(rel, line, rule)`; worlds run one after another under the
    /// lock. `files` is the union of the group's watched files plus each
    /// overlay's own file, `None` when any proposal watches every file
    /// (codemap 5: `check()` past 20 files or at `None`, `check_file` on each
    /// otherwise). Every call is logged for the `verify` layer.
    pub fn verify_worlds(
        &self,
        worlds: &[(String, World)],
        files: Option<&IndexSet<Rel>>,
    ) -> IndexMap<String, Vec<OracleDiag>> {
        let added = if worlds.is_empty() {
            IndexMap::new()
        } else {
            let root = self.sys_root.clone();
            let base = self.base();
            self.pass("counterfactual worlds", |db| {
                worlds
                    .iter()
                    .map(|(id, world)| (id.clone(), one_world(db, &root, base, world, files)))
                    .collect()
            })
            .unwrap_or_default()
        };
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(WorldCall {
                worlds: worlds
                    .iter()
                    .map(|(id, w)| {
                        (
                            id.clone(),
                            w.keys().map(|k| Rel::from(k.as_str())).collect(),
                        )
                    })
                    .collect(),
                added: added.clone(),
            });
        added
    }
}

/// One world: every overlay overridden in sorted rel order, the check, the
/// diagnostics the overlay adds against the base keys, the overrides restored
/// in reverse.
fn one_world(
    db: &mut ProjectDatabase,
    root: &SystemPath,
    base: &Base,
    world: &World,
    files: Option<&IndexSet<Rel>>,
) -> Vec<OracleDiag> {
    let mut rels: Vec<&String> = world.keys().collect();
    rels.sort();
    let mut restore: Vec<(File, Option<SourceText>)> = Vec::new();
    for rel in rels {
        let Some(file) = resolve(db, root, rel) else {
            continue;
        };
        restore.push((file, file.source_text_override(db).clone()));
        let overridden = source_text(db, file).with_text(world[rel].clone(), &SourceMap::default());
        file.set_source_text_override(db).to(Some(overridden));
    }
    let diagnostics = match files {
        Some(watched) if watched.len() <= MAX_WATCHED => watched
            .iter()
            .filter_map(|rel| resolve(db, root, rel))
            .flat_map(|file| db.check_file(file))
            .collect(),
        _ => db.check(),
    };
    let added = diagnostics
        .iter()
        .filter_map(|d| convert(db, root, d))
        .filter(|d| {
            !base
                .keys
                .contains(&(d.rel.clone(), d.line - 1, d.rule.clone()))
        })
        .collect();
    for (file, prior) in restore.into_iter().rev() {
        file.set_source_text_override(db).to(prior);
    }
    added
}
