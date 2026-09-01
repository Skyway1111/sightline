//! Clone one split of a round's manifest into the corpus root's `gauntlet-corpus/` at its
//! pinned SHAs, and make a Rust clone offline ready. This is what a re-clone
//! of a written round needs; the steps that wrote those manifests are not
//! here (`docs/todo.md`).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::Value;

use super::corpus_dir;

/// s, the one network step of a Rust clone.
const FETCH_TIMEOUT: u64 = 600;
/// s; the widest wave-0 wall is salvo's 116 crates cold at 54 s.
const CHECK_TIMEOUT: u64 = 300;

/// A round's namespace and language: what naming its manifest and choosing
/// the offline-ready step need. The seed and strata the selection read are
/// gone with it.
pub struct Round {
    /// the counter that measured the round, and the offline-ready switch
    pub lang: &'static str,
    /// manifest suffix
    pub ns: &'static str,
}

pub fn round_of(name: &str) -> Option<Round> {
    match name {
        "py" => Some(Round {
            lang: "python",
            ns: "4",
        }),
        "rs" => Some(Round {
            lang: "rust",
            ns: "-rs2",
        }),
        // rs2a: applications alone, over round rs1's pool
        "rs2a" => Some(Round {
            lang: "rust",
            ns: "-rs2a",
        }),
        _ => None,
    }
}

impl Round {
    pub fn manifest(&self, ext: &Path) -> PathBuf {
        ext.join(format!("manifest{}.json", self.ns))
    }
}

/// One pipe read to the end on its own thread.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _: std::io::Result<usize> = pipe.read_to_end(&mut buf);
        }
        buf
    })
}

/// `subprocess.run(..., timeout=secs)`: `None` where it timed out. Both pipes
/// are drained by their own thread, so a long cargo stream cannot fill a pipe
/// and deadlock the wait.
fn run(mut cmd: Command, secs: u64) -> Result<Option<Output>> {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(Output {
                status,
                stdout: out.join().unwrap_or_default(),
                stderr: err.join().unwrap_or_default(),
            }));
        }
        if Instant::now() >= deadline {
            let _: std::io::Result<()> = child.kill();
            let _: std::io::Result<std::process::ExitStatus> = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// `git` with the terminal prompt off, as the script runs it.
fn git(args: &[&str], secs: u64) -> Result<Option<Output>> {
    let mut cmd = Command::new("git");
    cmd.args(args).env("GIT_TERMINAL_PROMPT", "0");
    run(cmd, secs)
}

fn git_out(args: &[&str]) -> String {
    match git(args, 600) {
        Ok(Some(out)) => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => String::new(),
    }
}

/// Cargo's build dir for one repo, beside the corpus: a check inside a tree
/// would leave `target/` in what an audit walks, and one dir per repo is warm
/// when the clone step re-checks it.
fn target_dir(name: &str) -> PathBuf {
    let leaf = name.rsplit('/').next().unwrap_or(name);
    corpus_dir()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join("cargo-target")
        .join(leaf)
}

/// One cargo subprocess against that build dir; `None` on timeout. The
/// toolchain cargo gives this process is dropped: the candidate's own
/// `rust-toolchain.toml` rules its tree (`paths::drop_toolchain`).
fn cargo(
    root: &Path,
    args: &[&str],
    name: &str,
    secs: u64,
    env: &[(&str, &str)],
) -> Result<Option<Output>> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root)
        .args(args)
        .env("CARGO_TARGET_DIR", target_dir(name));
    crate::paths::drop_toolchain(&mut cmd);
    for (key, value) in env {
        cmd.env(key, value);
    }
    run(cmd, secs)
}

/// What a cargo failure is worth logging: its error headlines, else the last
/// lines (cargo's own last two are the backtrace note and a warning).
fn tail(text: &str) -> String {
    let errors: Vec<&str> = text.lines().filter(|l| l.starts_with("error")).collect();
    let picked: Vec<&str> = if errors.is_empty() {
        let lines: Vec<&str> = text.trim().lines().collect();
        lines[lines.len().saturating_sub(2)..].to_vec()
    } else {
        errors.into_iter().take(2).collect()
    };
    picked.join(" | ").chars().take(300).collect()
}

fn round1(x: f64) -> f64 {
    format!("{x:.1}").parse().unwrap_or(x)
}

/// A Rust clone's receipt that an audit of it runs offline: `cargo fetch`,
/// the lockfile it may write kept out of git through `.git/info/exclude`
/// (never `.gitignore`: the tree stays clean at its SHA), then the offline
/// check itself.
fn offline_ready(dest: &Path, name: &str) -> Result<String> {
    let root = dest.to_string_lossy().into_owned();
    let fetched = cargo(dest, &["fetch"], name, FETCH_TIMEOUT, &[])?;
    match &fetched {
        None => return Ok("fetch FAILED: timeout".to_string()),
        Some(out) if !out.status.success() => {
            return Ok(format!(
                "fetch FAILED: {}",
                tail(&String::from_utf8_lossy(&out.stderr))
            ));
        }
        Some(_) => {}
    }
    if git_out(&["-C", &root, "status", "--porcelain"]).contains("?? Cargo.lock") {
        let git_dir = git_out(&["-C", &root, "rev-parse", "--absolute-git-dir"])
            .trim()
            .to_string();
        let exclude = Path::new(&git_dir).join("info").join("exclude");
        if let Some(parent) = exclude.parent() {
            let _: std::io::Result<()> = std::fs::create_dir_all(parent);
        }
        let mut text = std::fs::read_to_string(&exclude).unwrap_or_default();
        text.push_str("Cargo.lock\n");
        std::fs::write(&exclude, text)?;
    }
    let dirty = git_out(&["-C", &root, "status", "--porcelain"]);
    let dirty = dirty.trim();
    if !dirty.is_empty() {
        let head: Vec<String> = dirty.lines().take(3).map(|l| format!("'{l}'")).collect();
        return Ok(format!("tree DIRTY after fetch: [{}]", head.join(", ")));
    }
    let start = Instant::now();
    let proc = cargo(
        dest,
        &["check", "--workspace", "--all-targets", "--offline"],
        name,
        CHECK_TIMEOUT,
        &[("CARGO_NET_OFFLINE", "true")],
    )?;
    let wall = round1(start.elapsed().as_secs_f64());
    match proc {
        None => Ok(format!("offline check TIMED OUT after {CHECK_TIMEOUT}s")),
        Some(out) if !out.status.success() => Ok(format!(
            "offline check FAILED in {wall}s: {}",
            tail(&String::from_utf8_lossy(&out.stderr))
        )),
        Some(_) => Ok(format!("clean tree, offline check ok in {wall}s")),
    }
}

/// One full clone per repo in the split into `gauntlet-corpus/`; minutes.
/// A Rust clone is then made offline ready.
fn clone_split(round: &Round, ext: &Path, want: &str) -> Result<()> {
    let path = round.manifest(ext);
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
    let corpus = corpus_dir();
    std::fs::create_dir_all(&corpus)?;
    for r in manifest["repos"].as_array().into_iter().flatten() {
        if r["split"].as_str() != Some(want) {
            continue;
        }
        let (name, sha) = (
            r["full_name"].as_str().unwrap_or_default(),
            r["sha"].as_str().unwrap_or_default(),
        );
        let dest = corpus.join(name.split('/').nth(1).unwrap_or(name));
        let dest_str = dest.to_string_lossy().into_owned();
        if dest.exists() {
            let head = git_out(&["-C", &dest_str, "rev-parse", "HEAD"])
                .trim()
                .to_string();
            let state = if head == sha {
                "ok".to_string()
            } else {
                format!("SHA MISMATCH (have {})", &head[..head.len().min(12)])
            };
            println!("{name}: exists, {state}");
            if head != sha {
                continue;
            }
        } else if !fresh_clone(name, sha, &dest_str)? {
            continue;
        }
        if round.lang == "rust" {
            println!("{name}: {}", offline_ready(&dest, name)?);
        }
    }
    Ok(())
}

fn fresh_clone(name: &str, sha: &str, dest: &str) -> Result<bool> {
    let url = format!("https://github.com/{name}.git");
    let cloned = git(&["clone", "--quiet", &url, dest], 600)?;
    if cloned.as_ref().is_none_or(|o| !o.status.success()) {
        let err = cloned.map_or_else(String::new, |o| {
            let text = String::from_utf8_lossy(&o.stderr).into_owned();
            text[text.len().saturating_sub(200)..].to_string()
        });
        println!("{name}: clone FAILED: {err}");
        return Ok(false);
    }
    let out = git(&["-C", dest, "checkout", "--detach", "--quiet", sha], 600)?;
    let ok = out.is_some_and(|o| o.status.success());
    println!(
        "{name}: cloned at {} -> {dest}{}",
        &sha[..sha.len().min(12)],
        if ok { "" } else { " CHECKOUT FAILED" }
    );
    Ok(ok)
}

/// The ledger the manifests live in: this workspace's `corpus-ext/`, or the
/// directory `--ext` names.
fn ext_dir(args: &[&str]) -> PathBuf {
    args.iter()
        .position(|a| *a == "--ext")
        .and_then(|i| args.get(i + 1))
        .map_or_else(
            || crate::paths::workspace_root().join("corpus-ext"),
            PathBuf::from,
        )
}

pub fn main(args: &[&str]) -> Result<u8> {
    let lang = args
        .iter()
        .position(|a| *a == "--lang")
        .and_then(|i| args.get(i + 1))
        .copied()
        .unwrap_or("py");
    let Some(round) = round_of(lang) else {
        eprintln!("gauntlet: no round named {lang} (py, rs, rs2a)");
        return Ok(2);
    };
    let want = if args.contains(&"--held-out") {
        "held_out"
    } else {
        "calibration"
    };
    clone_split(&round, &ext_dir(args), want)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_rounds_name_their_manifests() {
        let ext = Path::new("corpus-ext");
        for (name, lang, file) in [
            ("py", "python", "manifest4.json"),
            ("rs", "rust", "manifest-rs2.json"),
            ("rs2a", "rust", "manifest-rs2a.json"),
        ] {
            let round = round_of(name).expect("the round");
            assert_eq!(round.lang, lang);
            assert_eq!(round.manifest(ext), ext.join(file));
        }
        assert!(round_of("nope").is_none());
    }

    #[test]
    fn a_failure_is_logged_by_its_error_headline() {
        let stderr = "    Checking probe-b v0.1.0\n\
                      error: failed to run custom build command for `clang-sys v1.8.1`\n\
                      note: run with `RUST_BACKTRACE=1` for a backtrace\n\
                      warning: build failed, waiting for other jobs to finish...\n";
        assert_eq!(
            tail(stderr),
            "error: failed to run custom build command for `clang-sys v1.8.1`"
        );
        assert_eq!(tail("one\ntwo\nthree\n"), "two | three");
    }
}
