//! Where the Rust oracle builds: one directory per audited root under the
//! user's cache, and the sweep that keeps the cache from growing forever.

use camino::{Utf8Path, Utf8PathBuf};
use sha1::{Digest, Sha1};

/// One build directory per project root, outside every tree it audits:
/// `base` where the harness points a worktree run at the live root's, else
/// a per-root dir under the user's cache, keyed by a digest of the root's
/// path so repeat runs share one warm build. `project` is a rel under the
/// audited root, and gets a directory of its own: two crates of one tree may
/// pin different profiles, which cargo rebuilds a shared dir's dependencies
/// to switch between.
pub fn target_dir(root: &Utf8Path, project: &str, base: Option<&Utf8Path>) -> Utf8PathBuf {
    let base = match base {
        Some(named) => named.to_path_buf(),
        None => {
            let key = format!("{:x}", Sha1::digest(root.as_str().as_bytes()));
            cache_base().join(&key[..12])
        }
    };
    if project.is_empty() {
        base
    } else {
        base.join(project.replace('/', "-"))
    }
}

/// The directory every per-root build directory sits under.
fn cache_base() -> Utf8PathBuf {
    let cache = match cfg!(windows) {
        true => local_app_data(),
        false => home().join(".cache"),
    };
    cache.join("sightline").join("cargo-target")
}

fn local_app_data() -> Utf8PathBuf {
    let named = std::env::var("LOCALAPPDATA").map(Utf8PathBuf::from);
    named.unwrap_or_else(|_| home().join("AppData/Local"))
}

fn home() -> Utf8PathBuf {
    let named = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"));
    named.map(Utf8PathBuf::from).unwrap_or_default()
}

/// The marker each audit touches in its root's build directory, and the
/// age past which a sibling no audit touched is removed.
const USED_MARKER: &str = ".sightline-used";
pub const STALE_DAYS: u64 = 30;

/// Touch `own`'s marker and remove every sibling build directory whose
/// marker is older than `STALE_DAYS`; one with no marker gets one now, so
/// a directory an older release left starts its clock here. The note names
/// what went, `None` where nothing did.
pub fn sweep(own: &Utf8Path) -> Option<String> {
    let touch = |dir: &Utf8Path| {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(dir.join(USED_MARKER), "");
    };
    touch(own);
    let stale = std::time::Duration::from_secs(STALE_DAYS * 24 * 60 * 60);
    let now = std::time::SystemTime::now();
    let mut removed = 0usize;
    for entry in std::fs::read_dir(own.parent()?).ok()?.flatten() {
        let Ok(dir) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if dir == own || !dir.is_dir() {
            continue;
        }
        let touched = std::fs::metadata(dir.join(USED_MARKER)).and_then(|m| m.modified());
        match touched {
            Ok(at) if now.duration_since(at).is_ok_and(|age| age > stale) => {
                removed += usize::from(std::fs::remove_dir_all(&dir).is_ok());
            }
            Ok(_) => {}
            Err(_) => touch(&dir),
        }
    }
    (removed > 0).then(|| {
        let what = if removed == 1 { "directory" } else { "directories" };
        format!(
            "rs oracle: removed {removed} build {what} no audit touched in {STALE_DAYS} days under {}",
            own.parent().map_or("", Utf8Path::as_str)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sweep_touches_its_own_marker_and_removes_a_stale_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let base = Utf8Path::from_path(dir.path()).unwrap();
        let (own, stale, fresh, unmarked) = (
            base.join("aaa"),
            base.join("bbb"),
            base.join("ccc"),
            base.join("ddd"),
        );
        for d in [&own, &stale, &fresh, &unmarked] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(stale.join(USED_MARKER), "").unwrap();
        let old = std::time::SystemTime::now()
            - std::time::Duration::from_secs((STALE_DAYS + 1) * 24 * 60 * 60);
        std::fs::File::options()
            .write(true)
            .open(stale.join(USED_MARKER))
            .unwrap()
            .set_modified(old)
            .unwrap();
        std::fs::write(fresh.join(USED_MARKER), "").unwrap();

        let note = sweep(&own).unwrap();
        assert!(
            note.starts_with("rs oracle: removed 1 build directory"),
            "{note}"
        );
        assert!(own.join(USED_MARKER).is_file());
        assert!(!stale.exists());
        assert!(fresh.is_dir());
        // an unmarked sibling starts its clock rather than going now
        assert!(unmarked.join(USED_MARKER).is_file());
        assert_eq!(sweep(&own), None);
    }
}
