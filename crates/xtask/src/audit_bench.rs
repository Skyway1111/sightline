//! `cargo xtask audit-bench`, criteria 4 and 6.
//!
//! N cold audits of a tree at its pin: the median wall, the peak working set
//! of the process tree, and the sha256 of the JSON, which `--reference`
//! double checks against a recorded document. A dirty tree is audited
//! through a detached worktree at HEAD.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Result, bail};
use sha2::{Digest, Sha256};

use crate::corpus::{self, Target};
use crate::gate_bench::median;
use crate::paths::{flag, head, workspace_root};
use crate::worktree;

/// The sampler the walls are measured with.
const SAMPLER: &str = include_str!("../rss.ps1");
const PS: [&str; 4] = ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"];

/// The N runs: the walls, the highest peak seen, and the last JSON.
struct Bench {
    walls: Vec<f64>,
    rss_mb: u64,
    bytes: Vec<u8>,
}

/// One run, with the process tree's peak working set beside its wall. The
/// sampler is Windows-only; elsewhere the memory column reads 0.
fn run_sampled(mut cmd: Command) -> Result<(f64, u64, Vec<u8>)> {
    let child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let started = Instant::now();
    let sampler = cfg!(windows)
        .then(|| {
            Command::new("powershell")
                .args(PS)
                .arg(SAMPLER)
                .env("SIGHTLINE_ROOT_PID", child.id().to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()
        })
        .flatten();
    let done = child.wait_with_output()?;
    let wall = started.elapsed().as_secs_f64();
    if !done.status.success() {
        bail!(
            "audit failed:\n{}",
            corpus::tail(&String::from_utf8_lossy(&done.stderr))
        );
    }
    let read = |s: std::process::Child| -> Option<u64> {
        String::from_utf8_lossy(&s.wait_with_output().ok()?.stdout)
            .trim()
            .parse()
            .ok()
    };
    Ok((wall, sampler.and_then(read).unwrap_or(0), done.stdout))
}

fn bench(build: impl Fn() -> Result<Command>, n: usize) -> Result<Bench> {
    let mut out = Bench {
        walls: Vec::new(),
        rss_mb: 0,
        bytes: Vec::new(),
    };
    for _ in 0..n {
        let (wall, rss, bytes) = run_sampled(build()?)?;
        out.walls.push(wall);
        out.rss_mb = out.rss_mb.max(rss);
        out.bytes = bytes;
    }
    Ok(out)
}

fn row(t: &Target, n: usize, reference_json: Option<&Path>) -> Result<u8> {
    let out_dir = workspace_root().join("corpus/results/bench");
    std::fs::create_dir_all(&out_dir)?;
    let (here, held) = worktree::audited_tree(t, Some(&out_dir.join(format!("{}.toml", t.name))))?;
    let sha = worktree::head(&here.root);
    let audit = || corpus::command(&here, &["audit", &here.root.to_string_lossy(), "--json"]);
    let rs = bench(audit, n)?;
    std::fs::write(out_dir.join(format!("{}.json", t.name)), &rs.bytes)?;
    drop(held);
    let digest = format!("{:x}", Sha256::digest(&rs.bytes));
    let identical = reference_json
        .and_then(|p| std::fs::read(p).ok())
        .map(|want| want == rs.bytes);
    println!(
        "| {} | {} | {:.1} s | {} MB | {} | {} |",
        t.name,
        &sha[..12.min(sha.len())],
        median(rs.walls),
        rs.rss_mb,
        &digest[..16],
        identical.map_or_else(|| "-".to_string(), |ok| ok.to_string()),
    );
    Ok(u8::from(identical == Some(false)))
}

pub fn main(args: &[&str]) -> Result<u8> {
    let n = flag(args, "--n").and_then(|v| v.parse().ok()).unwrap_or(3);
    let reference_json = flag(args, "--reference").map(PathBuf::from);
    let named: Vec<&str> = args
        .iter()
        .copied()
        .filter(|a| !a.starts_with("--"))
        .filter(|a| a.parse::<usize>().is_err() && !Path::new(a).exists())
        .collect();
    let trees = match named.is_empty() {
        true => corpus::table()?,
        false => named
            .iter()
            .map(|n| corpus::get(n))
            .collect::<Result<_>>()?,
    };
    println!("| tree | sha | median | peak RSS | sha256/16 | identical |");
    println!("| --- | --- | ---: | ---: | --- | --- |");
    let mut code = 0;
    for t in &trees {
        // the wall is only comparable at the pin the bars were written for
        if let Some(pin) = &t.pin {
            let at = head(&t.root)?;
            if &at != pin {
                bail!("{} is at {at}, not its pin {pin}", t.root.display());
            }
        }
        code |= row(t, n, reference_json.as_deref())?;
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sampler names the pid it walks through the environment, so no
    /// argument has to survive quoting.
    #[test]
    fn the_sampler_reads_its_root_pid_from_the_environment() {
        assert!(SAMPLER.contains("SIGHTLINE_ROOT_PID"));
        assert!(SAMPLER.contains("WorkingSetSize"));
        assert_eq!(median(vec![9.0, 3.0, 5.0]), 5.0);
    }

    /// Every tree of the corpus table names the commit its recorded walls
    /// were measured at, so the pin check has a pin to read.
    #[test]
    fn every_corpus_tree_names_its_pin() {
        for t in corpus::table().unwrap() {
            let pin = t.pin.unwrap_or_default();
            assert_eq!(pin.len(), 40, "{}: {pin:?}", t.name);
            assert!(pin.chars().all(|c| c.is_ascii_hexdigit()), "{}", t.name);
        }
    }
}
