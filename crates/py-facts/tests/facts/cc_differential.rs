//! `data/cc.json` is the Python clean pole's `cc` map at the sha the file
//! names, grouped by module. This test parses each of those modules in the
//! live root, marks its defs as `index.rs` does and scores every one; every
//! row must match.

use std::path::{Path, PathBuf};
use std::process::Command;

use camino::Utf8Path;
use ruff_python_ast::Stmt;
use serde_json::Value;
use sightline_core::config::load_config;
use sightline_core::walk;
use sightline_py_facts::build::build_facts;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::complexity::cognitive_complexity;

const DATA: &str = include_str!("../data/cc.json");

/// The directory holding this workspace and every corpus root. A worktree lane
/// hangs off the primary checkout's parent (`xtask::paths::siblings`).
fn siblings() -> PathBuf {
    if let Some(dir) = std::env::var_os("SIGHTLINE_CORPUS_ROOT") {
        return PathBuf::from(dir);
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the workspace root");
    let common = Command::new("git")
        .args([
            "-C",
            &workspace.to_string_lossy(),
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ])
        .output();
    let checkout = match common {
        Ok(out) if out.status.success() => {
            PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string())
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| workspace.to_path_buf())
        }
        _ => workspace.to_path_buf(),
    };
    checkout.parent().map(Path::to_path_buf).unwrap_or(checkout)
}

fn head(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// --- the differential --------------------------------------------------------

#[test]
fn every_clean_pole_function_scores_what_cpython_scored() {
    let doc: Value = serde_json::from_str(DATA).expect("the fixture parses");
    let root = siblings().join("powertools-lambda-python");
    if !root.join(".git").exists() {
        eprintln!("skipped: {} is not in this checkout", root.display());
        return;
    }
    let want_sha = doc["sha"].as_str().expect("a sha");
    assert_eq!(
        head(&root).as_deref(),
        Some(want_sha),
        "{} is not at the pin data/cc.json was taken at",
        root.display()
    );

    let root = Utf8Path::from_path(&root).expect("a utf-8 corpus path");
    let config = load_config(root, None);
    let listing = walk::discover(root, &config);
    let built = build_facts(root, &config, &listing, None);
    let facts = built.borrow_dependent();
    let modules = doc["modules"].as_array().expect("a module list");
    let mut checked = 0;
    for module in modules {
        let rel = module["rel"].as_str().expect("a rel");
        for (q, cc) in module["functions"]
            .as_object()
            .expect("a function map")
            .iter()
        {
            let sym = facts
                .symbols
                .get(q.as_str())
                .unwrap_or_else(|| panic!("{rel}: no symbol {q}"));
            let holder = facts
                .modules
                .get(&*sym.module)
                .expect("the symbol's module");
            let Cn::Stmt(Stmt::FunctionDef(f)) = holder.nodes[sym.node as usize] else {
                panic!("{rel}: {q} is not a def");
            };
            let want = u32::try_from(cc.as_u64().expect("a score")).expect("a score");
            assert_eq!(cognitive_complexity(f, 0), want, "{rel}: {q}");
            checked += 1;
        }
    }
    assert_eq!(
        checked,
        doc["functions"].as_u64().expect("a count") as usize,
        "every function symbol is scored"
    );
}
