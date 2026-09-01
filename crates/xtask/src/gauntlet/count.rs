//! `scripts/gauntlet_count.py`: the pinned prod-LoC counter for the gauntlet
//! corpus.
//!
//! Python prod LoC = non-blank lines of non-test, non-vendored,
//! non-generated `.py` files under the root. Rust prod LoC = the same over
//! `.rs` files, with `target/` pruned and test paths read as `tests/`,
//! `benches/`, `examples/` (an inline `#[cfg(test)]` module counts as prod:
//! the measure is path-level). This subcommand is the definition: one pinned
//! counter, arguments end.
//!
//! Bucket precedence: excluded dirs are pruned entirely; a vendored dir
//! claims everything under it; then test dirs and filenames; then generated
//! files; the rest is prod. Python adds typedness = share of prod function
//! defs with any annotation; Rust adds the crate count and whether the root
//! is a `[workspace]`. A root holding `Cargo.toml` is measured as Rust.

use std::path::Path;

use anyhow::Result;
use ruff_python_ast::visitor::{Visitor, walk_stmt};
use ruff_python_ast::{Stmt, StmtFunctionDef};
use serde_json::{Value, json};
use sightline_core::pytext::splitlines;
use sightline_py_facts::model::is_test_path;

use super::{dumps, obj, read_lossy, resolve, walk};

#[rustfmt::skip]
pub const EXCLUDED_DIRS: [&str; 12] = [
    ".git",
    ".hg",
    ".venv",
    "venv",
    ".tox",
    ".nox",
    ".eggs",
    "__pycache__",
    "node_modules",
    "build",
    "dist",
    "site-packages",
];
pub const RS_EXCLUDED_DIRS: [&str; 4] = [".git", ".hg", "node_modules", "target"];
const RS_TEST_DIRS: [&str; 3] = ["tests", "benches", "examples"];
#[rustfmt::skip]
const VENDORED_DIRS: [&str; 9] = [
    "vendor",
    "vendored",
    "_vendor",
    "_vendored",
    "third_party",
    "thirdparty",
    "extern",
    "external",
    "externals",
];
const GENERATED_SUFFIXES: [&str; 2] = ["_pb2.py", "_pb2_grpc.py"];
const RS_GENERATED_SUFFIXES: [&str; 1] = [".pb.rs"];
const GENERATED_MARKERS: [&str; 3] = ["do not edit", "@generated", "automatically generated"];

/// The four buckets, in `loc` order.
#[derive(Default)]
struct Loc {
    prod: usize,
    test: usize,
    vendored: usize,
    generated: usize,
}

impl Loc {
    fn add(&mut self, kind: &str, lines: usize) {
        match kind {
            "prod" => self.prod += lines,
            "test" => self.test += lines,
            "vendored" => self.vendored += lines,
            _ => self.generated += lines,
        }
    }

    fn total(&self) -> usize {
        self.prod + self.test + self.vendored + self.generated
    }
}

fn generated(name: &str, text: &str, suffixes: &[&str]) -> bool {
    let head: String = splitlines(text)
        .into_iter()
        .take(5)
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    suffixes.iter().any(|s| name.ends_with(s)) || GENERATED_MARKERS.iter().any(|m| head.contains(m))
}

fn dirs_of(rel: &[String]) -> Vec<String> {
    rel[..rel.len() - 1]
        .iter()
        .map(|p| p.to_lowercase())
        .collect()
}

fn bucket(rel: &[String], text: &str) -> &'static str {
    let dirs = dirs_of(rel);
    if dirs.iter().any(|d| VENDORED_DIRS.contains(&d.as_str())) {
        return "vendored";
    }
    if is_test_path(&rel.join("/")) {
        return "test";
    }
    if generated(
        &rel[rel.len() - 1].to_lowercase(),
        text,
        &GENERATED_SUFFIXES,
    ) {
        return "generated";
    }
    "prod"
}

fn rs_bucket(rel: &[String], text: &str) -> &'static str {
    let dirs = dirs_of(rel);
    if dirs.iter().any(|d| VENDORED_DIRS.contains(&d.as_str())) {
        return "vendored";
    }
    if dirs.iter().any(|d| RS_TEST_DIRS.contains(&d.as_str())) {
        return "test";
    }
    if generated(
        &rel[rel.len() - 1].to_lowercase(),
        text,
        &RS_GENERATED_SUFFIXES,
    ) {
        return "generated";
    }
    "prod"
}

fn nonblank(text: &str) -> usize {
    splitlines(text)
        .into_iter()
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// Every def in the module, nested ones included, as `ast.walk` reaches them.
#[derive(Default)]
struct Defs {
    total: usize,
    annotated: usize,
}

impl<'a> Visitor<'a> for Defs {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::FunctionDef(f) = stmt {
            self.total += 1;
            self.annotated += usize::from(annotated(f));
        }
        walk_stmt(self, stmt);
    }
}

/// A return annotation, or any parameter that has one. A lambda parameter can
/// never be annotated, so the Python walk into defaults changes nothing.
fn annotated(f: &StmtFunctionDef) -> bool {
    f.returns.is_some()
        || f.parameters
            .iter()
            .any(|p| p.as_parameter().annotation.is_some())
}

/// `round(x, 3)`: the double nearest the correctly rounded 3-decimal value.
fn round3(x: f64) -> f64 {
    format!("{x:.3}").parse().unwrap_or(x)
}

/// Reads and parses every non-excluded `.py` under the root: one full tree
/// walk, IO- and parse-bound, seconds on repos in the gauntlet range.
pub fn count(root: &Path) -> Vec<(&'static str, Value)> {
    let mut loc = Loc::default();
    let (mut prod_files, mut parse_errors) = (0usize, 0usize);
    let mut defs = Defs::default();
    for (rel, text) in walk(root, ".py", &EXCLUDED_DIRS) {
        let kind = bucket(&rel, &text);
        loc.add(kind, nonblank(&text));
        if kind != "prod" {
            continue;
        }
        prod_files += 1;
        match ruff_python_parser::parse_module(&text) {
            Err(_) => parse_errors += 1,
            Ok(parsed) => defs.visit_body(&parsed.syntax().body),
        }
    }
    let coverage = if defs.total == 0 {
        0.0
    } else {
        round3(defs.annotated as f64 / defs.total as f64)
    };
    vec![
        ("root", json!(root.to_string_lossy())),
        ("prod_loc", json!(loc.prod)),
        ("test_loc", json!(loc.test)),
        ("vendored_loc", json!(loc.vendored)),
        ("generated_loc", json!(loc.generated)),
        ("total_loc", json!(loc.total())),
        ("prod_files", json!(prod_files)),
        ("def_count", json!(defs.total)),
        ("annotated_defs", json!(defs.annotated)),
        ("annotation_coverage", json!(coverage)),
        ("py_typed", json!(has_py_typed(root))),
        ("parse_errors", json!(parse_errors)),
    ]
}

/// `any(root.rglob("py.typed"))`: no dir is excluded from this one.
fn has_py_typed(root: &Path) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_name() == "py.typed" {
                return true;
            }
            if entry.path().is_dir() {
                stack.push(entry.path());
            }
        }
    }
    false
}

/// Reads every non-excluded `.rs` and every `Cargo.toml` under the root: one
/// full tree walk, IO-bound. A crate is a manifest holding `[package]`,
/// example and test crates included, so the count runs ahead of the member
/// list while the `.rs` buckets read their sources as test lines.
pub fn count_rs(root: &Path) -> Vec<(&'static str, Value)> {
    let mut loc = Loc::default();
    let mut prod_files = 0usize;
    let mut crates = 0usize;
    let manifest = root.join("Cargo.toml");
    let workspace = read_lossy(&manifest).is_ok_and(|t| t.contains("[workspace]"));
    for (rel, text) in walk(root, ".rs", &RS_EXCLUDED_DIRS) {
        let kind = rs_bucket(&rel, &text);
        loc.add(kind, nonblank(&text));
        prod_files += usize::from(kind == "prod");
    }
    for (_rel, text) in walk(root, "Cargo.toml", &RS_EXCLUDED_DIRS) {
        if let Ok(table) = text.parse::<toml::Table>() {
            crates += usize::from(table.contains_key("package"));
        }
    }
    vec![
        ("root", json!(root.to_string_lossy())),
        ("prod_loc", json!(loc.prod)),
        ("test_loc", json!(loc.test)),
        ("vendored_loc", json!(loc.vendored)),
        ("generated_loc", json!(loc.generated)),
        ("total_loc", json!(loc.total())),
        ("prod_files", json!(prod_files)),
        ("crates", json!(crates)),
        ("workspace", json!(workspace)),
    ]
}

/// A root holding `Cargo.toml` is measured as Rust.
pub fn measure(root: &Path) -> Vec<(&'static str, Value)> {
    if root.join("Cargo.toml").exists() {
        count_rs(root)
    } else {
        count(root)
    }
}

/// The text form's value: Python prints a bool as `True`, a str bare.
fn text_of(value: &Value) -> String {
    match value {
        Value::Bool(b) => (if *b { "True" } else { "False" }).to_string(),
        Value::String(s) => s.clone(),
        other => super::dumps(&super::J::Leaf(other.clone()), 0),
    }
}

pub fn main(args: &[&str]) -> Result<u8> {
    let Some(first) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: cargo xtask gauntlet count <repo-root> [--json]");
        return Ok(2);
    };
    let root = resolve(Path::new(first));
    let rows = measure(&root);
    if args.contains(&"--json") {
        println!("{}", dumps(&obj(rows), 2));
    } else {
        for (key, value) in rows {
            println!("{key}: {}", text_of(&value));
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tree written under a fresh temp root, then measured.
    fn counted(files: &[(&str, &str)]) -> Vec<(&'static str, Value)> {
        let dir = tempfile::tempdir().expect("a temp root");
        for (rel, text) in files {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
            std::fs::write(&path, text).expect("write");
        }
        measure(dir.path())
    }

    fn field(rows: &[(&'static str, Value)], key: &str) -> Value {
        rows.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("no {key} in the measure"))
    }

    #[test]
    fn buckets_and_typedness() {
        let got = counted(&[
            // prod: 4 non-blank lines, 2 defs, 1 annotated
            (
                "a.py",
                "def f(x: int) -> int:\n    return x\n\ndef g(x):\n    return x\n",
            ),
            ("pkg/b.py", "X = 1\n"),
            ("tests/test_a.py", "def test_a():\n    pass\n"),
            ("conftest.py", "pass\n"),
            ("b_test.py", "pass\n"),
            // pandas-style: a test tree to the rules, so to the counter
            ("testing/helpers.py", "pass\n"),
            ("vendor/inner/v.py", "V = 1\nW = 2\n"),
            ("wire_pb2.py", "P = 1\n"),
            ("gen.py", "# DO NOT EDIT\nG = 1\n"),
            // excluded entirely
            (".venv/lib.py", "H = 1\n"),
        ]);
        assert_eq!(field(&got, "prod_loc"), 5);
        assert_eq!(field(&got, "test_loc"), 5);
        assert_eq!(field(&got, "vendored_loc"), 2);
        assert_eq!(field(&got, "generated_loc"), 3);
        assert_eq!(field(&got, "total_loc"), 15);
        assert_eq!(field(&got, "prod_files"), 2);
        assert_eq!(field(&got, "def_count"), 2);
        assert_eq!(field(&got, "annotated_defs"), 1);
        assert_eq!(field(&got, "annotation_coverage"), 0.5);
        assert_eq!(field(&got, "py_typed"), false);
    }

    #[test]
    fn vendored_wins_over_test_and_generated() {
        let got = counted(&[
            ("third_party/test_x.py", "pass\n"),
            ("third_party/y_pb2.py", "pass\n"),
        ]);
        assert_eq!(field(&got, "vendored_loc"), 2);
        assert_eq!(field(&got, "test_loc"), 0);
        assert_eq!(field(&got, "generated_loc"), 0);
    }

    #[test]
    fn an_unparseable_prod_file_counts_lines() {
        let got = counted(&[("broken.py", "def f(:\n")]);
        assert_eq!(field(&got, "prod_loc"), 1);
        assert_eq!(field(&got, "parse_errors"), 1);
        assert_eq!(field(&got, "def_count"), 0);
    }

    #[test]
    fn rust_buckets_and_crates() {
        let got = counted(&[
            ("Cargo.toml", "[package]\nname = \"demo\"\n"),
            ("src/lib.rs", "pub fn a() -> u32 {\n    1\n}\n"),
            // prod: the measure is path-level, so 5
            (
                "src/inline.rs",
                "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n",
            ),
            ("tests/it.rs", "fn t() {}\n"),
            ("benches/b.rs", "fn b() {}\n"),
            ("examples/e.rs", "fn e() {}\n"),
            ("vendor/v.rs", "pub fn v() {}\n"),
            // vendored wins over the test dir
            ("vendor/tests/vt.rs", "fn vt() {}\n"),
            ("gen.rs", "// @generated\npub fn g() {}\n"),
            // pruned entirely
            ("target/debug/x.rs", "fn x() {}\n"),
        ]);
        assert_eq!(field(&got, "prod_loc"), 8);
        assert_eq!(field(&got, "test_loc"), 3);
        assert_eq!(field(&got, "vendored_loc"), 2);
        assert_eq!(field(&got, "generated_loc"), 2);
        assert_eq!(field(&got, "total_loc"), 15);
        assert_eq!(field(&got, "prod_files"), 2);
        assert_eq!(field(&got, "crates"), 1);
        assert_eq!(field(&got, "workspace"), false);
    }

    #[test]
    fn a_rust_workspace_counts_member_crates() {
        let got = counted(&[
            ("Cargo.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n"),
            ("a/Cargo.toml", "[package]\nname = \"a\"\n"),
            ("b/Cargo.toml", "[package]\nname = \"b\"\n"),
            ("target/pkg/Cargo.toml", "[package]\nname = \"junk\"\n"),
        ]);
        assert_eq!(field(&got, "crates"), 2);
        assert_eq!(field(&got, "workspace"), true);
    }
}
