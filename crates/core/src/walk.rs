//! The discovery walk every language shares. Dot-directories, the environment
//! dirs and linked directories (a junction up the tree is a cycle) are never
//! entered. The order is `sorted(Path.iterdir())`'s: case-insensitive on
//! Windows, byte order elsewhere.

use camino::{Utf8Path, Utf8PathBuf};

use crate::config::{Config, DEFAULT_EXCLUDE_DIRS};
use crate::pytext;

/// One listed child: its name, its path, and the type the directory entry
/// already holds. That type is `symlink_metadata`'s, so it never follows, and
/// on Windows it costs no syscall of its own.
struct Child {
    name: String,
    path: Utf8PathBuf,
    kind: std::fs::FileType,
}

impl Child {
    /// `Path.is_file()`, which follows a link. Only a link pays the stat.
    fn is_file(&self) -> bool {
        match self.kind.is_symlink() {
            true => self.path.is_file(),
            false => self.kind.is_file(),
        }
    }
}

/// One directory entry as a `Child`; `None` for a path the walk cannot spell.
fn child(e: std::fs::DirEntry) -> Option<Child> {
    let path = Utf8PathBuf::from_path_buf(e.path()).ok()?;
    Some(Child {
        name: path.file_name()?.to_string(),
        path,
        kind: e.file_type().ok()?,
    })
}

/// The children of `dir` in Python's `Path` order. An unreadable directory
/// lists nothing.
fn sorted_children(dir: &Utf8Path) -> Vec<Child> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Child> = entries.flatten().filter_map(child).collect();
    if cfg!(windows) {
        out.sort_by_key(|a| pytext::lower(&a.name));
    } else {
        out.sort_by(|a, b| a.name.cmp(&b.name));
    }
    out
}

/// A directory the walk enters: not a dot-dir, not an environment dir, and not
/// a link. On Windows the entry's type reads a mount-point reparse tag too, so
/// a junction is a link here as it is to Python's `Path.is_junction`
/// (`tests/discover_tree.rs` proves it).
fn enterable(child: &Child) -> bool {
    !child.name.starts_with('.')
        && !DEFAULT_EXCLUDE_DIRS.contains(&child.name.as_str())
        && !child.kind.is_symlink()
}

/// Auditable files as (absolute path, path relative to the root with `/`
/// separators), in walk order.
pub fn discover(root: &Utf8Path, config: &Config) -> Vec<(Utf8PathBuf, String)> {
    walk(root, "", config)
}

/// One directory level. Subdirectories descend in parallel, since opening a
/// directory is a syscall apiece and the wall is theirs, and the results
/// concatenate in sorted-child order, so the listing is the sequential
/// walk's byte for byte.
fn walk(dir: &Utf8Path, prefix: &str, config: &Config) -> Vec<(Utf8PathBuf, String)> {
    use rayon::prelude::*;
    let per_child: Vec<Vec<(Utf8PathBuf, String)>> = sorted_children(dir)
        .into_par_iter()
        .map(|child| {
            let rel = format!("{prefix}{}", child.name);
            if excluded(&rel, &config.excludes) {
                Vec::new()
            } else if child.is_file() {
                vec![(child.path, rel)]
            } else if enterable(&child) {
                walk(&child.path, &format!("{rel}/"), config)
            } else {
                Vec::new()
            }
        })
        .collect();
    per_child.concat()
}

/// Does any file name `discover` could reach, without a config, satisfy the
/// predicate? What language detection asks a tree. Existence has no order,
/// so the walk is unsorted and stops at the first hit: a root manifest
/// answers from the first directory read.
pub fn any_name(root: &Utf8Path, pred: impl Fn(&str) -> bool) -> bool {
    let mut stack: Vec<Utf8PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for child in entries.flatten().filter_map(child) {
            if child.is_file() {
                if pred(&child.name) {
                    return true;
                }
            } else if enterable(&child) {
                stack.push(child.path);
            }
        }
    }
    false
}

/// A file as Python's `read_text` hands it over: UTF-8, with U+FFFD where
/// the bytes are not, and every line ending translated to `\n`. The flag
/// says the bytes were not UTF-8.
pub fn read_text(path: &Utf8Path) -> Option<(String, bool)> {
    let bytes = std::fs::read(path).ok()?;
    let (text, lossy) = match String::from_utf8(bytes) {
        Ok(text) => (text, false),
        // U+FFFD is 3 bytes, so byte columns here are no one else's
        Err(e) => (String::from_utf8_lossy(e.as_bytes()).into_owned(), true),
    };
    Some((translate_newlines(text), lossy))
}

/// Universal newlines, which every `Path.read_text` applies before the
/// tokenizer sees a byte.
pub fn translate_newlines(text: String) -> String {
    if !text.contains('\r') {
        return text;
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// `posixpath.join`.
pub fn posix_join(base: &str, rest: &str) -> String {
    if rest.starts_with('/') || base.is_empty() {
        return rest.to_string();
    }
    if base.ends_with('/') {
        return format!("{base}{rest}");
    }
    format!("{base}/{rest}")
}

/// `posixpath.normpath`: `.` and `..` resolved, empty parts dropped, a
/// leading slash kept, an empty result spelled `.`.
pub fn normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let rooted = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            ".." if parts.is_empty() && rooted => {}
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    match (rooted, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".to_string(),
        (false, false) => joined,
    }
}

/// A config exclude hits a whole path segment, an `fnmatch` of the relative
/// path, or a directory prefix of it.
pub fn excluded(rel: &str, patterns: &[String]) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    patterns.iter().any(|pat| {
        let norm = pytext::rstrip_chars(pat, "/");
        parts.contains(&norm) || pytext::fnmatch(rel, pat) || rel.starts_with(&format!("{norm}/"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exclude_hits_a_segment_a_glob_or_a_prefix() {
        let pats = vec![
            "corpus-ext".to_string(),
            "*.md".to_string(),
            "docs/".to_string(),
        ];
        assert!(excluded("corpus-ext/reports/x.py", &pats));
        assert!(excluded("src/a.md", &pats));
        assert!(excluded("docs/todo.py", &pats));
        assert!(!excluded("src/a.py", &pats));
        assert!(!excluded("mydocs/a.py", &pats));
    }
}
