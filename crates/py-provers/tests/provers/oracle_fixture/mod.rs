//! An in-process checker at a mini repo's root, for the tests that need a
//! real answer (`COMMON.md`, "Test fixtures").

#![allow(dead_code)]

use camino::{Utf8Path, Utf8PathBuf};
use sightline_py_provers::oracle::Oracle;
use sightline_testkit::PyStack;
use tempfile::TempDir;

/// Build an `Oracle` at the repo's root and hand it to the stack's provers.
pub fn attach(dir: &TempDir, stack: &mut PyStack) {
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let import_roots: Vec<Utf8PathBuf> = stack.facts().import_roots.clone();
    let oracle = Oracle::new(root, &[], &import_roots, None).expect("an in-process checker");
    stack.provers.oracle = Some(oracle);
}
