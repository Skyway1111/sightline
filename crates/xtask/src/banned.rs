//! The prose lint: every text file of the tree, against three lists.
//!
//! The em dash, a word list, and the citation patterns. The words are the
//! ones this repository's prose reached for as filler or drama in place of
//! a mechanism or a number; each earned its row by appearing twice. The
//! citation patterns name a planning document and a Python tool that are
//! not in this repository, so a comment that cites one points a reader at
//! nothing. One line per hit, `path:line: <match>`. No suppression: an
//! escape hatch would make this a limit that only tightens. Not scanned:
//! the judges' and burials' ledgers, test data, and this file, which holds
//! the lists.

use std::path::{Path, PathBuf};

use anyhow::Result;
use regex::Regex;

use crate::paths::{git, workspace_root};

/// Words that stand in for a mechanism or a number. A word joins the list
/// on its second sighting in a review.
pub const WORDS: &[&str] = &[
    "deliberate",
    "deliberately",
    "asymmetry",
    "disagreed",
    "nowhere",
    "carrying",
    "carries",
    "died",
    "handed",
    "genuine",
    "genuinely",
    "ruling",
    "byte-identical",
    "outright",
    "pre-fix",
    "refuses",
    "premise",
    "nobody",
    "asserted",
    "halves",
    "re-derived",
    "survived",
    "refusal",
    "quietly",
    "plainly",
    "load-bearing",
    "envelope",
    "bites",
    "buys",
];

/// Citations of the plan the rewrite was built under and of the Python tool
/// it replaced. Neither is in this repository, so a comment that cites one
/// sends a reader to nothing; the comment says what its own code does instead.
pub const POINTERS: &[&str] = &[
    r"codemap",
    r"dump_layers",
    r"[Pp]ort of .*\.py",
    r"criterion \d",
    r"decision \d",
    r"`scripts/[a-z_]+\.py`",
    r"rewrite contract",
    r"plan\.md",
    r"docs/todo\.md",
];

/// The burials ledger, the judges' ledger, `data/retired.toml` (the same
/// ledger rows, extracted by `xtask retired`), this file, which holds the
/// lists, and the two files of third-party license text, whose words are
/// their authors' and not this repository's prose.
const SKIP: &[&str] = &[
    "corpus-ext/",
    "docs/review/",
    "data/retired.toml",
    "crates/xtask/src/banned.rs",
    "crates/xtask/licenses/",
    "THIRD-PARTY.md",
];

/// Recorded probe inputs, whose strings are data.
const SKIP_DIR: &str = "/tests/data/";

fn pattern() -> Regex {
    let words: Vec<String> = WORDS.iter().map(|w| regex::escape(w)).collect();
    Regex::new(&format!(
        r"(?i)\u{{2014}}|\b(?:{})\b|(?-i:{})",
        words.join("|"),
        POINTERS.join("|")
    ))
    .expect("the word list builds a valid pattern")
}

fn hits(root: &Path, rel: &str, re: &Regex) -> Vec<String> {
    if SKIP.iter().any(|s| rel.starts_with(s)) || rel.contains(SKIP_DIR) {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
        return Vec::new(); // binary, or gone
    };
    text.lines()
        .enumerate()
        .flat_map(|(i, line)| {
            re.find_iter(line)
                .map(move |m| format!("{rel}:{}: {}", i + 1, m.as_str()))
        })
        .collect()
}

fn rel_of(root: &Path, arg: &str) -> String {
    let path = PathBuf::from(arg);
    let full = path.canonicalize().unwrap_or(path);
    let under = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    full.strip_prefix(&under)
        .unwrap_or(&full)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn main(args: &[&str]) -> Result<u8> {
    let root = workspace_root();
    let re = pattern();
    let rels: Vec<String> = if args == ["--tree"] {
        git(&root, &["ls-files"])?
            .lines()
            .map(str::to_string)
            .collect()
    } else if args.is_empty() {
        eprintln!("usage: xtask banned <file>... | xtask banned --tree");
        return Ok(2);
    } else {
        args.iter().map(|a| rel_of(&root, a)).collect()
    };
    let found: Vec<String> = rels.iter().flat_map(|r| hits(&root, r, &re)).collect();
    for line in &found {
        println!("{line}");
    }
    Ok(u8::from(!found.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_em_dash_and_a_listed_word_are_hits_and_the_case_does_not_matter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.md"),
            "one \u{2014} two\nNOBODY here\nclean\n",
        )
        .unwrap();
        let got = hits(dir.path(), "a.md", &pattern());
        assert_eq!(got, ["a.md:1: \u{2014}", "a.md:2: NOBODY"]);
    }

    #[test]
    fn a_citation_of_the_retired_plan_or_tool_is_a_hit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.rs"),
            "//! Port of `x.py` (codemap 3.3)\n// see criterion 5 and decision 17\n// `scripts/run.py`\n",
        )
        .unwrap();
        let got = hits(dir.path(), "a.rs", &pattern());
        assert_eq!(
            got,
            [
                "a.rs:1: Port of `x.py",
                "a.rs:1: codemap",
                "a.rs:2: criterion 5",
                "a.rs:2: decision 1",
                "a.rs:3: `scripts/run.py`",
            ]
        );
        // a fixture path under tests/data is data, not prose
        let data = "crates/core/tests/data/x.json";
        let path = dir.path().join(data);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "docs/todo.md\n").unwrap();
        assert!(hits(dir.path(), data, &pattern()).is_empty());
    }

    #[test]
    fn the_ledgers_and_this_file_are_not_scanned() {
        let dir = tempfile::tempdir().unwrap();
        for rel in ["corpus-ext/decisions.tsv", "corpus-ext/BRIEF.md"] {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "nobody \u{2014}\n").unwrap();
            assert!(hits(dir.path(), rel, &pattern()).is_empty());
        }
    }

    #[test]
    fn the_tree_scan_is_clean() {
        assert_eq!(main(&["--tree"]).unwrap(), 0);
    }
}
