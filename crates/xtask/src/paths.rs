//! Where the trees this workspace measures live.
//!
//! A worktree lane has no corpus siblings of its own: the roots hang off the
//! primary checkout's parent. `SIGHTLINE_CORPUS_ROOT` overrides it.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// `--name value` from an argv slice: the one reader every subcommand uses.
pub fn flag<'a>(args: &'a [&'a str], name: &str) -> Option<&'a str> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1])
}

pub fn workspace_root() -> PathBuf {
    // crates/xtask/ -> crates/ -> the workspace
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the manifest sits two levels under the workspace root")
        .to_path_buf()
}

/// The primary checkout of this workspace, so a worktree lane still finds
/// the siblings.
fn main_checkout() -> PathBuf {
    let root = workspace_root();
    let out = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let dir = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string());
            dir.parent().unwrap_or(&root).to_path_buf()
        }
        _ => root,
    }
}

/// The directory holding this workspace and every corpus root.
pub fn siblings() -> PathBuf {
    if let Some(dir) = std::env::var_os("SIGHTLINE_CORPUS_ROOT") {
        return PathBuf::from(dir);
    }
    let checkout = main_checkout();
    checkout.parent().map(Path::to_path_buf).unwrap_or(checkout)
}

/// The interpreter inside a virtual environment, on either platform layout.
pub fn venv_python(venv: &Path) -> Option<PathBuf> {
    ["Scripts/python.exe", "bin/python"]
        .iter()
        .map(|rel| venv.join(rel))
        .find(|path| path.is_file())
}

/// The interpreter the two catalog checkers run their exemplars under when
/// `--python` names none: the first of these that answers on PATH. The #12
/// gate also wants CrossHair installed on it.
pub fn path_python() -> Result<PathBuf> {
    ["python3", "python"]
        .into_iter()
        .find(|name| {
            Command::new(name)
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
        })
        .map(PathBuf::from)
        .context("no python3 or python on PATH; name one with --python")
}

/// Cargo hands a child `RUSTUP_TOOLCHAIN=1.97.1` (this workspace's pin) and
/// its own `CARGO`/`RUSTC`. Under that pin `rust-analyzer` answers `Unknown
/// binary`, so an audit driven from `cargo xtask` loses its Rust oracle
/// silently. Any child that runs a Rust tool at another root drops them and
/// takes that tree's toolchain.
pub fn drop_toolchain(cmd: &mut Command) {
    for name in [
        "RUSTUP_TOOLCHAIN",
        "CARGO",
        "RUSTC",
        "RUSTDOC",
        "RUSTC_WRAPPER",
    ] {
        cmd.env_remove(name);
    }
}

pub fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("git {} in {}", args.join(" "), root.display()))?;
    if !out.status.success() {
        bail!(
            "git {} in {} failed: {}",
            args.join(" "),
            root.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn head(root: &Path) -> Result<String> {
    git(root, &["rev-parse", "HEAD"])
}
