//! `cargo xtask install`: build the release binary and copy it into
//! `$CARGO_HOME/bin`, so the checkout on this machine is what `sightline`
//! on PATH runs. The plugin hooks call the bare name and go stale when a
//! rebuild skips this step.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::paths::workspace_root;

fn cargo_bin() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("CARGO_HOME") {
        return Ok(PathBuf::from(dir).join("bin"));
    }
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .context("no CARGO_HOME, USERPROFILE or HOME in the environment")?;
    Ok(PathBuf::from(home).join(".cargo").join("bin"))
}

pub fn main(_args: &[&str]) -> Result<u8> {
    let root = workspace_root();
    let status = Command::new("cargo")
        .args(["build", "--release", "-p", "sightline-lint"])
        .current_dir(&root)
        .status()
        .context("spawning cargo build")?;
    if !status.success() {
        bail!("cargo build --release -p sightline-lint failed");
    }
    let name = format!("sightline{}", std::env::consts::EXE_SUFFIX);
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let built = target.join("release").join(&name);
    let dest = cargo_bin()?.join(&name);
    std::fs::copy(&built, &dest)
        .with_context(|| format!("copying {} to {}", built.display(), dest.display()))?;
    println!("installed {}", dest.display());
    Ok(0)
}
