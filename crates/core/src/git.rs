//! The two questions the tool asks git: how old is each clone group's youngest
//! priced copy (#11's ranking), and which files a gate run should read.
//! Anchored to HEAD's commit time, so output is deterministic for a repo state.
//! No usable history answers `None` and #11 degrades to count-only ranking.

use std::collections::{BTreeSet, HashMap};
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use rayon::prelude::*;

use crate::pytext;

/// (path relative to the root, first line, last).
pub type Span = (String, u32, u32);

fn blame_workers() -> usize {
    std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8)
}

/// git's stdout on success, decoded lossily; `None` where git is absent or the
/// call failed.
pub fn run_git(root: &Utf8Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root.as_str())
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Final line number to committer time, from `blame --line-porcelain`.
fn line_times(porcelain: &str) -> HashMap<u32, i64> {
    let mut times: HashMap<u32, i64> = HashMap::new();
    let mut commit_times: HashMap<&str, i64> = HashMap::new();
    let mut sha: Option<&str> = None;
    let mut line: u32 = 0;
    for r in pytext::splitlines(porcelain) {
        let parts: Vec<&str> = r.split(' ').collect();
        let header = (parts.len() >= 3 && parts[0].len() == 40 && pytext::is_digit(parts[1]))
            .then(|| parts[2].parse::<u32>().ok())
            .flatten();
        if let Some(at) = header {
            sha = Some(parts[0]);
            line = at;
            times.insert(line, commit_times.get(parts[0]).copied().unwrap_or(0));
        } else if let (Some(known), Some(rest)) = (sha, r.strip_prefix("committer-time ")) {
            let when = rest.parse::<i64>().unwrap_or(0);
            commit_times.insert(known, when);
            times.insert(line, when);
        }
    }
    times
}

pub struct GitAges {
    root: Utf8PathBuf,
    head_time: Option<i64>,
}

impl GitAges {
    pub fn new(root: &Utf8Path) -> GitAges {
        let head = run_git(root, &["log", "-1", "--format=%ct"]);
        let head_time = head
            .as_deref()
            .map(pytext::strip)
            .filter(|t| pytext::is_digit(t))
            .and_then(|t| t.parse::<i64>().ok());
        GitAges {
            root: root.to_path_buf(),
            head_time,
        }
    }

    pub fn available(&self) -> bool {
        self.head_time.is_some()
    }

    /// Per group, the days between HEAD and the newest commit touching any of
    /// its spans, the age of its youngest copy; `None` where a span has no
    /// history to read, and for a group with no spans. One ranged blame per
    /// distinct file, driven concurrently: the per-process floor is I/O wait,
    /// so the count of files holding a priced copy is the bill.
    pub fn youngest_ages_days(&self, groups: &[Vec<Span>]) -> Vec<Option<i64>> {
        let mut windows: IndexMap<&str, BTreeSet<(u32, u32)>> = IndexMap::new();
        for spans in groups {
            for (rel, lo, hi) in spans {
                windows.entry(rel.as_str()).or_default().insert((*lo, *hi));
            }
        }
        let work: Vec<(&str, &BTreeSet<(u32, u32)>)> =
            windows.iter().map(|(rel, ranges)| (*rel, ranges)).collect();
        let answered: Vec<Option<HashMap<u32, i64>>> = if work.is_empty() {
            Vec::new()
        } else {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(blame_workers())
                .build()
                .expect("a rayon pool with a fixed thread count");
            pool.install(|| work.par_iter().map(|w| self.blame(w.0, w.1)).collect())
        };
        let blames: HashMap<&str, Option<HashMap<u32, i64>>> =
            work.iter().map(|w| w.0).zip(answered).collect();
        groups
            .iter()
            .map(|spans| self.age(&blames, spans))
            .collect()
    }

    fn blame(&self, rel: &str, ranges: &BTreeSet<(u32, u32)>) -> Option<HashMap<u32, i64>> {
        let mut args: Vec<String> = vec!["blame".into(), "--line-porcelain".into()];
        for (lo, hi) in ranges {
            args.push("-L".into());
            args.push(format!("{lo},{hi}"));
        }
        args.push("--".into());
        args.push(rel.to_string());
        let spelled: Vec<&str> = args.iter().map(String::as_str).collect();
        run_git(&self.root, &spelled).as_deref().map(line_times)
    }

    fn age(
        &self,
        blames: &HashMap<&str, Option<HashMap<u32, i64>>>,
        spans: &[Span],
    ) -> Option<i64> {
        let head_time = self.head_time?;
        let mut newest = 0;
        for (rel, lo, hi) in spans {
            let times = blames.get(rel.as_str())?.as_ref()?;
            let window: Vec<i64> = (*lo..=*hi)
                .filter_map(|n| times.get(&n).copied())
                .filter(|t| *t != 0)
                .collect();
            let found = window.iter().copied().max()?;
            newest = newest.max(found);
        }
        (newest != 0).then(|| ((head_time - newest) / 86400).max(0))
    }
}

/// Working-tree diff against HEAD plus untracked files, plus the commits since
/// the merge base with `since` (the branch posture); `None` without git.
pub fn changed_files(root: &Utf8Path, since: Option<&str>) -> Option<Vec<String>> {
    let mut listings: Vec<Vec<String>> = vec![
        vec!["diff".into(), "--name-only".into(), "HEAD".into()],
        vec![
            "ls-files".into(),
            "--others".into(),
            "--exclude-standard".into(),
        ],
    ];
    if let Some(since) = since {
        listings.push(vec![
            "diff".into(),
            "--name-only".into(),
            format!("{since}...HEAD"),
        ]);
    }
    let mut out: BTreeSet<String> = BTreeSet::new();
    for args in &listings {
        let spelled: Vec<&str> = args.iter().map(String::as_str).collect();
        let listed = run_git(root, &spelled)?;
        out.extend(
            pytext::splitlines(&listed)
                .into_iter()
                .map(pytext::strip)
                .filter(|line| !line.is_empty())
                .map(str::to_string),
        );
    }
    Some(out.into_iter().collect())
}

/// HEAD's commit, or `no-git` where there is no history to read.
pub fn head_sha(root: &Utf8Path) -> String {
    run_git(root, &["rev-parse", "HEAD"])
        .map_or_else(|| "no-git".to_string(), |out| out.trim().to_string())
}

/// Does the tree hold uncommitted changes? A dirty root is read through a
/// detached worktree at HEAD, so a receipt names the tree it really read.
pub fn working_tree_dirty(root: &Utf8Path) -> bool {
    run_git(root, &["status", "--porcelain"]).is_some_and(|out| !out.trim().is_empty())
}

/// A detached worktree at HEAD, removed when the guard drops. Nothing is
/// linked into it: `git worktree remove --force` follows a junction. One
/// home for `debug dump` and every `xtask` that reads a dirty tree.
pub struct Worktree {
    live: Utf8PathBuf,
    pub path: Utf8PathBuf,
}

impl Worktree {
    /// `None` where git refused. The directory is named for the process and
    /// the tree, so two trees of one run never share it.
    pub fn add(live: &Utf8Path) -> Option<Worktree> {
        let stem = live.file_name().unwrap_or("tree");
        let dir =
            std::env::temp_dir().join(format!("sightline-worktree-{}-{stem}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = Utf8PathBuf::from(dir.to_string_lossy().into_owned()).join("tree");
        run_git(
            live,
            &["worktree", "add", "--detach", path.as_str(), "HEAD"],
        )?;
        Some(Worktree {
            live: live.to_path_buf(),
            path,
        })
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        run_git(
            &self.live,
            &["worktree", "remove", "--force", self.path.as_str()],
        );
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "2024-01-01T00:00:00";

    fn git(root: &Utf8Path, args: &[&str], when: Option<&str>) {
        let mut cmd = Command::new("git");
        cmd.arg("-C")
            .arg(root.as_str())
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args);
        if let Some(when) = when {
            cmd.env("GIT_AUTHOR_DATE", when)
                .env("GIT_COMMITTER_DATE", when);
        }
        let out = cmd.output().expect("git is on PATH");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Two modules written at OLD; the second's line 11 rewritten at HEAD.
    fn repo(root: &Utf8Path) {
        let body: String = (0..30).map(|i| format!("x{i} = {i}\n")).collect();
        for name in ["a.py", "b.py"] {
            std::fs::write(root.join(name), &body).expect("a writable temp dir");
        }
        git(root, &["init", "-q"], Some(OLD));
        git(root, &["add", "."], Some(OLD));
        git(root, &["commit", "-qm", "one"], Some(OLD));
        let rewritten: String = (0..30)
            .map(|i| {
                if i == 10 {
                    "x10 = 100\n".to_string()
                } else {
                    format!("x{i} = {i}\n")
                }
            })
            .collect();
        std::fs::write(root.join("b.py"), rewritten).expect("a writable temp dir");
        git(root, &["commit", "-qam", "two"], None);
    }

    fn span(rel: &str, lo: u32, hi: u32) -> Span {
        (rel.to_string(), lo, hi)
    }

    #[test]
    fn group_age_is_its_newest_touched_line_across_files() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = Utf8Path::from_path(dir.path()).expect("a UTF-8 temp path");
        repo(root);
        let ages = GitAges::new(root).youngest_ages_days(&[
            vec![span("a.py", 2, 5)],                       // only the OLD commit
            vec![span("a.py", 2, 5), span("b.py", 10, 12)], // one young span prices it
            vec![span("b.py", 20, 25)],                     // the young file, an old window
        ]);
        let old = ages[0].expect("a.py has history");
        assert!(old > 300, "written at OLD, HEAD is today: {old}");
        assert_eq!(ages[1], Some(0));
        assert_eq!(ages[2], Some(old));
    }

    #[test]
    fn a_span_with_no_history_leaves_the_group_unpriced() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = Utf8Path::from_path(dir.path()).expect("a UTF-8 temp path");
        repo(root);
        std::fs::write(root.join("c.py"), "y = 1\n").expect("a writable temp dir");
        let ages = GitAges::new(root)
            .youngest_ages_days(&[vec![span("a.py", 2, 5), span("c.py", 1, 1)], vec![]]);
        assert_eq!(ages, vec![None, None]);
    }

    #[test]
    fn a_tree_without_history_is_unavailable() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = Utf8Path::from_path(dir.path()).expect("a UTF-8 temp path");
        let git = GitAges::new(root);
        assert!(!git.available());
        assert_eq!(
            git.youngest_ages_days(&[vec![span("a.py", 1, 2)]]),
            vec![None]
        );
    }

    #[test]
    fn changed_files_lists_the_working_tree_and_the_untracked() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = Utf8Path::from_path(dir.path()).expect("a UTF-8 temp path");
        repo(root);
        std::fs::write(root.join("a.py"), "edited\n").expect("a writable temp dir");
        std::fs::write(root.join("new.py"), "fresh\n").expect("a writable temp dir");
        assert_eq!(
            changed_files(root, None),
            Some(vec!["a.py".to_string(), "new.py".to_string()])
        );
    }

    #[test]
    fn a_tree_without_git_answers_none() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = Utf8Path::from_path(dir.path()).expect("a UTF-8 temp path");
        assert_eq!(changed_files(root, None), None);
    }
}
