//! The gauntlet corpus tools: the pinned prod-LoC counter, the precision
//! sheets and the clone step of a round's manifest.
//!
//! `git` and `cargo` stay subprocesses. Sourcing a new round, meaning
//! building the candidate pool and selecting from it, is not here: the three
//! written rounds need only the clone step, and a fourth needs the selection
//! written first (`docs/todo.md`).

pub mod clone;
pub mod count;
pub mod sheet;

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

const USAGE: &str = "\
usage: cargo xtask gauntlet <command>

  count <repo-root> [--json]
  sheet <audit.json> <out.tsv> [--carry <earlier.tsv>] [--rules 1,50]
  tally <sheet.tsv>... [--bar 0.3] [--min-n 5]
  clone [--lang py|rs|rs2a] [--held-out] [--ext <dir>]
";

pub fn main(args: &[&str]) -> Result<u8> {
    let rest: Vec<&str> = args.iter().skip(1).copied().collect();
    match args.first().copied() {
        Some("count") => count::main(&rest),
        Some("sheet" | "tally") => sheet::main(args),
        Some("clone") => clone::main(&rest),
        _ => {
            eprint!("{USAGE}");
            Ok(2)
        }
    }
}

/// A JSON document in insertion order. `serde_json::Map` is a `BTreeMap`
/// here (no `preserve_order`, which `core::pyjson` relies on), so an object
/// that must keep the order the Python script wrote it in is this instead.
pub enum J {
    Leaf(Value),
    Obj(Vec<(String, J)>),
}

impl From<Value> for J {
    fn from(v: Value) -> Self {
        J::Leaf(v)
    }
}

/// An object off `(key, value)` rows, in the order given.
pub fn obj<K: Into<String>, V: Into<J>>(rows: impl IntoIterator<Item = (K, V)>) -> J {
    J::Obj(
        rows.into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
    )
}

/// `json.dumps(obj, indent=n)`: insertion order kept, non-ASCII as `\uXXXX`,
/// floats as CPython `repr`.
pub fn dumps(value: &J, indent: usize) -> String {
    let mut out = String::new();
    write_value(&mut out, value, indent, 1);
    out
}

fn write_value(out: &mut String, value: &J, indent: usize, depth: usize) {
    let pad = " ".repeat(indent * depth);
    let close = " ".repeat(indent * (depth - 1));
    match value {
        J::Obj(rows) if !rows.is_empty() => {
            out.push_str("{\n");
            for (i, (k, v)) in rows.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&pad);
                write_str(out, k);
                out.push_str(": ");
                write_value(out, v, indent, depth + 1);
            }
            out.push('\n');
            out.push_str(&close);
            out.push('}');
        }
        J::Obj(_) => out.push_str("{}"),
        J::Leaf(Value::String(s)) => write_str(out, s),
        J::Leaf(Value::Bool(b)) => out.push_str(if *b { "true" } else { "false" }),
        J::Leaf(Value::Null) => out.push_str("null"),
        J::Leaf(Value::Number(n)) => match n.as_f64() {
            Some(x) if n.is_f64() => out.push_str(&sightline_core::pytext::repr_float(x)),
            _ => out.push_str(&n.to_string()),
        },
        // an array or object already inside a `Value` keeps that value's order
        J::Leaf(other) => out.push_str(&serde_json::to_string(other).unwrap_or_default()),
    }
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    crate::text::escape_into(out, s, crate::text::Style::Json);
    out.push('"');
}

/// `(relative parts, text)` per readable file whose name ends with `suffix`,
/// outside the excluded dirs. One full tree walk, IO-bound.
pub fn walk(root: &Path, suffix: &str, excluded: &[&str]) -> Vec<(Vec<String>, String)> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), Vec::<String>::new())];
    while let Some((dir, parts)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let mut rel = parts.clone();
            rel.push(name.clone());
            if entry.path().is_dir() {
                let lower = name.to_lowercase();
                if !excluded.contains(&lower.as_str()) && !name.ends_with(".egg-info") {
                    stack.push((entry.path(), rel));
                }
            } else if name.ends_with(suffix) {
                let Ok(text) = read_lossy(&entry.path()) else {
                    continue;
                };
                out.push((rel, text));
            }
        }
    }
    out
}

/// `Path(arg).resolve()`: absolute, symlinks followed, and without the
/// Windows verbatim prefix `canonicalize` adds and CPython does not.
pub fn resolve(path: &Path) -> PathBuf {
    let full = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let text = full.to_string_lossy().into_owned();
    PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text).to_string())
}

/// `path.read_text(encoding="utf-8", errors="replace")`.
pub fn read_lossy(path: &Path) -> std::io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

/// `../gauntlet-corpus` of the main checkout: a linked worktree's own parent
/// is not the corpus.
pub fn corpus_dir() -> PathBuf {
    crate::paths::siblings().join("gauntlet-corpus")
}
