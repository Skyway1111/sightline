//! `cargo xtask third-party`: `THIRD-PARTY.md`, the notices the binary owes.
//!
//! The graph is `cargo metadata`'s, walked from the `sightline` package over
//! every edge that is not dev-only, so no test-only crate enters it. This
//! workspace's own crates take the repository's `LICENSE` and stay out.
//!
//! A package's notice is every license file beside its manifest, or in the
//! nearest ancestor holding one, no higher than the directory cargo unpacked
//! (its `.cargo-ok`): a crate of the ty fork takes the checkout's root
//! `LICENSE` that way. One whose published source ships none is named with
//! its authors and repository under the SPDX text in `licenses/`, and one no
//! id of which that directory holds is named as unresolved. Nothing is left
//! out silently. `--check` renders and compares, so the committed file
//! cannot drift from the graph without the `check` stage failing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::paths::workspace_root;

/// The file this stage owns. `dist-workspace.toml` puts it in every archive
/// with `include = ["THIRD-PARTY.md"]`.
const DOC: &str = "THIRD-PARTY.md";

/// A file name holding one of these is a notice, whatever its case or
/// extension: `LICENSE`, `LICENSE-MIT`, `license-apache-2.0`, `UNLICENSE`,
/// `COPYING`, `NOTICE`.
const NAMES: [&str; 3] = ["LICEN", "COPYING", "NOTICE"];

/// The column a package list wraps at, so this file's diffs read.
const WIDTH: usize = 78;

struct Pkg {
    name: String,
    version: String,
    license: String,
    authors: String,
    repository: String,
    /// Every notice its source ships, LF, in file name order.
    notices: Vec<String>,
}

fn field(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn metadata() -> Result<Value> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(workspace_root())
        .args(["metadata", "--format-version", "1"])
        .output()
        .context("cargo metadata")?;
    if !out.status.success() {
        bail!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

fn is_notice(name: &str) -> bool {
    let upper = name.to_uppercase();
    NAMES.iter().any(|n| upper.contains(n))
}

fn read_lf(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes)
        .replace("\r\n", "\n")
        .trim_end()
        .to_string())
}

/// The notices beside a manifest, else those of the nearest ancestor holding
/// any, stopping at the directory cargo unpacked.
fn notices(manifest: &Path) -> Result<Vec<String>> {
    let mut dir = manifest.parent().unwrap_or(manifest);
    loop {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .is_some_and(|n| is_notice(&n.to_string_lossy()))
            })
            .collect();
        files.sort();
        if !files.is_empty() {
            return files.iter().map(|p| read_lf(p)).collect();
        }
        match dir.parent() {
            Some(up) if !dir.join(".cargo-ok").exists() => dir = up,
            _ => return Ok(Vec::new()),
        }
    }
}

/// The first id of a license expression that `licenses/` holds the text of.
/// `MIT`, `MIT OR Apache-2.0` and `MIT / Apache-2.0` all answer MIT.
fn spdx(license: &str) -> Option<(String, String)> {
    let dir = workspace_root().join("crates/xtask/licenses");
    license
        .split(|c: char| !c.is_ascii_alphanumeric() && !"-.+".contains(c))
        .filter(|id| !matches!(*id, "" | "OR" | "AND" | "WITH"))
        .find_map(|id| {
            let text = std::fs::read_to_string(dir.join(format!("{id}.txt"))).ok()?;
            Some((
                id.to_string(),
                text.replace("\r\n", "\n").trim_end().to_string(),
            ))
        })
}

/// Every package the binary links, sorted by name and version.
fn linked(meta: &Value) -> Result<Vec<Pkg>> {
    let packages = meta["packages"]
        .as_array()
        .context("cargo metadata listed no packages")?;
    let by_id: HashMap<&str, &Value> = packages
        .iter()
        .filter_map(|p| Some((p["id"].as_str()?, p)))
        .collect();
    let nodes: HashMap<&str, &Value> = meta["resolve"]["nodes"]
        .as_array()
        .context("cargo metadata resolved no graph")?
        .iter()
        .filter_map(|n| Some((n["id"].as_str()?, n)))
        .collect();
    let root = packages
        .iter()
        .find(|p| p["name"] == "sightline-lint" && p["source"].is_null())
        .context("no `sightline` package in this workspace")?;

    // sorted, so two packages of one name and version never trade places
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut stack = vec![root["id"].as_str().unwrap_or_default()];
    while let Some(next) = stack.pop() {
        if !seen.insert(next) {
            continue;
        }
        let node = nodes
            .get(next)
            .with_context(|| format!("{next} is outside the resolved graph"))?;
        for dep in node["deps"].as_array().into_iter().flatten() {
            let kinds = dep["dep_kinds"].as_array();
            // an edge cargo lists only as `dev` builds the tests and never
            // the binary; one it lists no kind for is taken as normal
            let dev_only =
                kinds.is_some_and(|k| !k.is_empty() && k.iter().all(|k| k["kind"] == "dev"));
            if !dev_only {
                stack.push(dep["pkg"].as_str().unwrap_or_default());
            }
        }
    }

    let mut out = Vec::new();
    for id in seen {
        let pkg = by_id.get(id).context("a resolved id with no manifest")?;
        if pkg["source"].is_null() {
            continue; // this workspace's own crate: the repository's LICENSE
        }
        out.push(Pkg {
            name: field(pkg, "name"),
            version: field(pkg, "version"),
            license: field(pkg, "license"),
            authors: pkg["authors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            repository: field(pkg, "repository"),
            notices: notices(Path::new(&field(pkg, "manifest_path")))?,
        });
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}

/// A comma-separated list broken at `WIDTH`, so a notice's users read in a
/// diff instead of arriving as one 2 KB line.
fn wrapped(items: &[String]) -> String {
    let mut lines: Vec<String> = vec![String::new()];
    for (i, item) in items.iter().enumerate() {
        let piece = format!("{item}{}", if i + 1 < items.len() { "," } else { "" });
        let line = lines.last_mut().expect("the first line is there");
        if line.is_empty() {
            line.push_str(&piece);
        } else if line.len() + 1 + piece.len() <= WIDTH {
            line.push(' ');
            line.push_str(&piece);
        } else {
            lines.push(piece);
        }
    }
    lines.join("\n")
}

/// A table cell: a pipe would end the column, and an author's angle brackets
/// read as a tag outside a code span.
fn cell(text: &str) -> String {
    if text.is_empty() {
        "-".to_string()
    } else {
        format!("`{}`", text.replace('|', "\\|"))
    }
}

/// A fence longer than any backtick run the text holds, so a license that
/// spells one cannot end its own block.
fn fenced(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}text\n{text}\n{fence}\n")
}

const HEAD: &str = "| Package | Version | License | Notice |\n| --- | --- | --- | --- |\n";
const SILENT_HEAD: &str = "| Package | Version | License | Authors | Repository |\n\
                           | --- | --- | --- | --- | --- |\n";

fn silent_rows(pkgs: &[&Pkg]) -> String {
    let mut out = SILENT_HEAD.to_string();
    for p in pkgs {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            p.name,
            p.version,
            declared(p),
            cell(&p.authors),
            cell(&p.repository)
        ));
    }
    out
}

fn declared(p: &Pkg) -> &str {
    if p.license.is_empty() {
        "not declared"
    } else {
        &p.license
    }
}

fn render(pkgs: &[Pkg]) -> String {
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut texts: Vec<&str> = Vec::new();
    let mut users: Vec<Vec<String>> = Vec::new();
    let mut rows = HEAD.to_string();
    let mut silent: Vec<&Pkg> = Vec::new();
    for p in pkgs {
        let mut ids: Vec<String> = Vec::new();
        for notice in &p.notices {
            let i = match index.get(notice.as_str()) {
                Some(i) => *i,
                None => {
                    index.insert(notice, texts.len());
                    texts.push(notice);
                    users.push(Vec::new());
                    texts.len() - 1
                }
            };
            users[i].push(format!("{} {}", p.name, p.version));
            ids.push(format!("[{n}](#notice-{n})", n = i + 1));
        }
        if p.notices.is_empty() {
            silent.push(p);
            ids.push("none shipped".to_string());
        }
        rows.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            p.name,
            p.version,
            declared(p),
            ids.join(", ")
        ));
    }

    let mut out = format!(
        "# Third-party notices\n\n\
         The `sightline` binary links {} third-party packages into itself. Each is\n\
         listed here with its version, the license expression its manifest declares,\n\
         and the notice text its source ships. This repository's own crates are\n\
         covered by its `LICENSE` and are not listed.\n\n\
         `cargo xtask third-party` writes this file from `cargo metadata` and the\n\
         license files in the cargo registry and the git checkouts. The `third-party`\n\
         stage of `cargo xtask check` renders it again and fails when the two differ,\n\
         so a dependency this file misses stops the gate.\n\n\
         ## Packages\n\n{rows}\n\
         ## Notices\n\n{} texts, one section each. The packages sharing a text are \
         named under it.\n\n",
        pkgs.len(),
        texts.len(),
    );
    for (i, text) in texts.iter().enumerate() {
        out.push_str(&format!(
            "### Notice {}\n\n{}\n\n{}\n",
            i + 1,
            wrapped(&users[i]),
            fenced(text)
        ));
    }

    if silent.is_empty() {
        return out;
    }
    let mut groups: std::collections::BTreeMap<String, (String, Vec<&Pkg>)> =
        std::collections::BTreeMap::new();
    let mut unresolved: Vec<&Pkg> = Vec::new();
    for p in silent.iter().copied() {
        match spdx(&p.license) {
            Some((id, text)) => groups.entry(id).or_insert((text, Vec::new())).1.push(p),
            None => unresolved.push(p),
        }
    }
    out.push_str(&format!(
        "## Packages that ship no notice\n\n\
         {} of the packages above declare a license their published source holds no\n\
         copy of. Each is listed with the authors and the repository its manifest\n\
         names, under the terms it declares.\n\n",
        silent.len()
    ));
    for (id, (text, pkgs)) in &groups {
        out.push_str(&format!(
            "### {id}\n\n{}\n{}",
            silent_rows(pkgs),
            fenced(text)
        ));
    }
    if !unresolved.is_empty() {
        out.push_str(&format!(
            "### No text located\n\n\
             `crates/xtask/licenses/` holds no text for any license id these declare.\n\n{}",
            silent_rows(&unresolved)
        ));
    }
    out
}

/// The first line the committed file and the graph's render differ at, so a
/// stale file names its own drift.
fn first_diff(have: &str, want: &str) -> Option<(usize, String, String)> {
    let have: Vec<&str> = have.lines().collect();
    let want: Vec<&str> = want.lines().collect();
    let clip = |line: Option<&&str>| line.unwrap_or(&"").chars().take(60).collect::<String>();
    (0..have.len().max(want.len()))
        .find(|i| have.get(*i) != want.get(*i))
        .map(|i| (i + 1, clip(have.get(i)), clip(want.get(i))))
}

pub fn main(args: &[&str]) -> Result<u8> {
    let path = workspace_root().join(DOC);
    let pkgs = linked(&metadata()?)?;
    let text = render(&pkgs);
    let silent = pkgs.iter().filter(|p| p.notices.is_empty()).count();
    let tally = format!(
        "{} packages, {} shipping no notice of their own",
        pkgs.len(),
        silent
    );
    if !args.contains(&"--check") {
        std::fs::write(&path, &text)?;
        println!("third-party: wrote {DOC}, {tally}");
        return Ok(0);
    }
    if std::fs::read_to_string(&path).unwrap_or_default() == text {
        println!("third-party: {DOC} matches the graph, {tally}");
        return Ok(0);
    }
    println!("third-party: {DOC} no longer matches the graph, {tally}");
    if let Some((line, have, want)) = first_diff(&std::fs::read_to_string(&path)?, &text) {
        println!("  line {line}: the file has {have:?}, the graph has {want:?}");
    }
    println!("  run `cargo xtask third-party` and commit the result");
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notice_is_named_whatever_its_case_or_extension() {
        for name in ["LICENSE-MIT", "license-apache-2.0", "UNLICENSE", "COPYING"] {
            assert!(is_notice(name), "{name}");
        }
        for name in ["lib.rs", "Cargo.toml", "README.md"] {
            assert!(!is_notice(name), "{name}");
        }
    }

    /// A crate of a git checkout keeps no license file of its own: the walk
    /// climbs to the checkout's root and stops at the `.cargo-ok` cargo
    /// wrote there.
    #[test]
    fn the_walk_climbs_to_the_unpacked_root_and_no_further() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("checkout");
        let member = root.join("crates/ruff_db");
        std::fs::create_dir_all(&member).unwrap();
        std::fs::write(root.join(".cargo-ok"), "").unwrap();
        std::fs::write(root.join("LICENSE"), "MIT terms\r\nhere\n\n").unwrap();
        std::fs::write(member.join("Cargo.toml"), "").unwrap();
        // above the unpacked root, where another package's file would sit
        std::fs::write(dir.path().join("LICENSE"), "not this one").unwrap();
        assert_eq!(
            notices(&member.join("Cargo.toml")).unwrap(),
            ["MIT terms\nhere"]
        );
        // a package shipping its own takes them all, in name order
        std::fs::write(member.join("LICENSE-MIT"), "mit").unwrap();
        std::fs::write(member.join("LICENSE-APACHE"), "apache").unwrap();
        let own = notices(&member.join("Cargo.toml")).unwrap();
        assert_eq!(own, ["apache", "mit"]);
        // and one that ships none under the marker takes none
        for name in ["LICENSE-MIT", "LICENSE-APACHE"] {
            std::fs::remove_file(member.join(name)).unwrap();
        }
        std::fs::remove_file(root.join("LICENSE")).unwrap();
        assert!(notices(&member.join("Cargo.toml")).unwrap().is_empty());
    }

    #[test]
    fn the_first_id_with_a_committed_text_answers() {
        for expression in ["MIT", "MIT OR Apache-2.0", "MIT / Apache-2.0", "MIT AND X"] {
            let (id, text) = spdx(expression).expect(expression);
            assert_eq!(id, "MIT");
            assert!(text.contains("THE SOFTWARE IS PROVIDED \"AS IS\""));
        }
        assert!(spdx("WTFPL OR Beerware").is_none());
        // the keywords of an expression are never read as ids
        assert!(spdx("OR AND WITH").is_none());
    }

    #[test]
    fn a_long_list_wraps_at_the_column() {
        let items: Vec<String> = (0..9).map(|i| format!("package-{i} 1.0.0")).collect();
        let text = wrapped(&items);
        assert!(text.lines().all(|l| l.len() <= WIDTH), "{text}");
        assert_eq!(text.lines().count(), 3);
        assert_eq!(text.replace('\n', " "), items.join(", "));
        assert_eq!(wrapped(&[]), "");
    }

    #[test]
    fn a_fence_outruns_the_backticks_of_its_text() {
        assert_eq!(fenced("plain"), "```text\nplain\n```\n");
        assert_eq!(fenced("a ``` b"), "````text\na ``` b\n````\n");
    }

    #[test]
    fn a_stale_file_names_the_line_it_drifts_at() {
        assert_eq!(first_diff("a\nb\n", "a\nb\n"), None);
        let (line, have, want) = first_diff("a\nb\n", "a\nc\n").unwrap();
        assert_eq!((line, have.as_str(), want.as_str()), (2, "b", "c"));
        // a file that stops early drifts at its first missing line
        assert_eq!(first_diff("a\n", "a\nb\n").unwrap().0, 2);
    }

    /// The rendered file names every package, links each to a notice it
    /// shares, and lists one that ships none under the SPDX text.
    #[test]
    fn the_render_holds_every_package_and_one_copy_of_a_shared_text() {
        let pkg = |name: &str, notices: Vec<String>| Pkg {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            license: "MIT".to_string(),
            authors: "A | B <a@example.com>".to_string(),
            repository: String::new(),
            notices,
        };
        let shared = "MIT terms".to_string();
        let text = render(&[
            pkg("alpha", vec![shared.clone()]),
            pkg("beta", vec![shared]),
            pkg("gamma", Vec::new()),
        ]);
        assert!(text.contains("| alpha | 1.0.0 | MIT | [1](#notice-1) |"));
        assert!(text.contains("| beta | 1.0.0 | MIT | [1](#notice-1) |"));
        assert!(text.contains("| gamma | 1.0.0 | MIT | none shipped |"));
        assert_eq!(text.matches("MIT terms").count(), 1);
        assert!(text.contains("### Notice 1\n\nalpha 1.0.0, beta 1.0.0\n"));
        // the silent one takes the committed SPDX text, and its authors
        // cross a table column without ending it
        assert!(text.contains("| gamma | 1.0.0 | MIT | `A \\| B <a@example.com>` | - |"));
        assert!(text.contains("Permission is hereby granted"));
    }
}
