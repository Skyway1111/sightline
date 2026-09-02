//! `discover` and `any_name` over one tree holding a dot-dir, two of
//! `DEFAULT_EXCLUDE_DIRS`, a junction, a mixed-case sibling pair and both
//! shapes of config exclude.

use camino::{Utf8Path, Utf8PathBuf};
use sightline_core::config::Config;
use sightline_core::walk::{any_name, discover};

const FILES: &[&str] = &[
    ".hidden/x.py",
    "__pycache__/y.py",
    "build/z.py",
    "Alpha.py",
    "a.py",
    "beta.py",
    "Zeta.py",
    "linked/inside.py",
    "skipme/s.py",
    "gen/out.gen.py",
    "src/m.py",
    "src/sub/n.py",
];

fn build(root: &Utf8Path) {
    for rel in FILES {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("every entry names a directory"))
            .expect("a writable temp dir");
        std::fs::write(&path, "x = 1\n").expect("a writable temp dir");
    }
    link(&root.join("linked"), &root.join("link"));
}

#[cfg(windows)]
fn link(target: &Utf8Path, at: &Utf8Path) {
    let out = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J", at.as_str(), target.as_str()])
        .output()
        .expect("cmd is on PATH");
    assert!(
        out.status.success(),
        "mklink: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[cfg(not(windows))]
fn link(target: &Utf8Path, at: &Utf8Path) {
    std::os::unix::fs::symlink(target, at).expect("a writable temp dir");
}

#[cfg(windows)]
const LISTED: &[&str] = &[
    "a.py",
    "Alpha.py",
    "beta.py",
    "linked/inside.py",
    "src/m.py",
    "src/sub/n.py",
    "Zeta.py",
];
/// Off Windows `Path` comparison is byte order, so the capitals sort first.
#[cfg(not(windows))]
const LISTED: &[&str] = &[
    "Alpha.py",
    "Zeta.py",
    "a.py",
    "beta.py",
    "linked/inside.py",
    "src/m.py",
    "src/sub/n.py",
];

/// What a name-only walk can reach, order-free.
const NAMES: &[&str] = &[
    "a.py",
    "Alpha.py",
    "beta.py",
    "Zeta.py",
    "m.py",
    "n.py",
    "s.py",
    "inside.py",
    "out.gen.py",
];

fn root() -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a UTF-8 temp path");
    build(&root);
    (dir, root)
}

#[test]
fn discover_lists_what_the_python_walk_lists_in_the_same_order() {
    let (_dir, root) = root();
    let config = Config {
        excludes: vec!["skipme".to_string(), "*.gen.py".to_string()],
        ..Config::new()
    };
    let listed: Vec<String> = discover(&root, &config)
        .into_iter()
        .map(|(_, rel)| rel)
        .collect();
    assert_eq!(listed, LISTED);
}

/// `any_name` reaches every file `discover` could (nested dirs included,
/// config excludes ignored) and nothing behind a dot-dir or a default
/// exclude dir.
#[test]
fn any_name_reaches_what_discover_could_and_no_more() {
    let (_dir, root) = root();
    for name in NAMES {
        assert!(
            any_name(&root, |n| n == *name),
            "{name} should be reachable"
        );
    }
    for name in ["x.py", "y.py", "z.py", "absent.py"] {
        assert!(!any_name(&root, |n| n == name), "{name} should not be");
    }
}
