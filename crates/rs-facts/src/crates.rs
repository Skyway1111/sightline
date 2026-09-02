//! What Cargo says about a tree's layout: whether
//! it runs the Rust stack, where each crate is rooted, and the qname a
//! file's path spells. Path readings only, so `only=` names a module as the
//! full build does.

use std::collections::HashSet;

use camino::Utf8Path;
use indexmap::IndexMap;
use sightline_core::lang::Listing;
use sightline_core::pytext::join_path;
use sightline_core::walk::{any_name, normpath, posix_join};

use crate::nodes::nonempty;
use crate::{MANIFEST, SUFFIX};

/// A Cargo manifest anywhere the audit walk reaches selects the stack: a
/// tree whose crates sit below the root with no manifest and no
/// `[workspace]` at the top is a Rust tree too.
pub fn detect(root: &Utf8Path) -> bool {
    any_name(root, |n| n == MANIFEST)
}

/// A crate name as its module path spells it.
pub fn ident(name: &str) -> String {
    name.replace('-', "_")
}

fn read_table(path: &Utf8Path) -> Option<toml::Table> {
    let bytes = std::fs::read(path).ok()?;
    String::from_utf8_lossy(&bytes).parse::<toml::Table>().ok()
}

/// The rel directory a manifest sits in: `rel[: -len(MANIFEST) - 1]`.
fn home_of(rel: &str) -> &str {
    &rel[..rel.len().saturating_sub(MANIFEST.len() + 1)]
}

/// Every parsed manifest as `(home rel dir, table)`, in listing order. The
/// one read: `crate_roots` and `lib_crates` both consume this, so a tree of
/// many crates parses each `Cargo.toml` once per build, not once per reader.
/// The reads run under rayon, a file read and a TOML parse per member, and
/// collect in listing order.
pub fn manifests(listing: &Listing) -> Vec<(String, toml::Table)> {
    use rayon::prelude::*;
    listing
        .par_iter()
        .filter(|(_, rel)| rel.ends_with(MANIFEST))
        .filter_map(|(path, rel)| Some((home_of(rel).to_string(), read_table(path)?)))
        .collect()
}

/// Crate name to the rel dir it is rooted at. Every `[package]` manifest is
/// a root, wherever it sits, so no member list is read; a virtual manifest
/// (`[workspace]` alone) is not one. A tree whose manifests name no package
/// still gets a root, so its files keep a qname.
pub fn crate_roots(
    root: &Utf8Path,
    manifests: &[(String, toml::Table)],
) -> IndexMap<String, String> {
    let mut out: IndexMap<String, String> = IndexMap::new();
    for (home, table) in manifests {
        if let Some(name) = table
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(toml::Value::as_str)
            .filter(nonempty)
        {
            out.entry(ident(name)).or_insert_with(|| home.clone());
        }
    }
    if out.is_empty() {
        out.insert(ident(root.file_name().unwrap_or_default()), String::new());
    }
    out
}

/// The crates whose library a downstream user can name: a lib target (a
/// `[lib]` table or a `src/lib.rs` under the manifest), no `publish = false`
/// (nor the empty registry list, which says the same), and either no bin
/// target or a manifest in the listing that path-depends on it. An
/// application's `lib.rs` is there for its own bin and tests, not for a
/// downstream user.
pub fn lib_crates(listing: &Listing, manifests: &[(String, toml::Table)]) -> HashSet<String> {
    let rels: HashSet<&str> = listing.iter().map(|(_, rel)| rel.as_str()).collect();
    let homes: HashSet<&str> = manifests.iter().map(|(h, _)| h.as_str()).collect();
    let mut depended: HashSet<String> = HashSet::new();
    for (home, table) in manifests {
        for spelled in dep_paths(table) {
            let dep = normpath(&posix_join(home, &spelled));
            if homes.contains(dep.as_str()) && dep != *home {
                depended.insert(dep);
            }
        }
    }
    let mut out: HashSet<String> = HashSet::new();
    for (home, table) in manifests {
        let package = table.get("package").and_then(toml::Value::as_table);
        let Some(name) = package
            .and_then(|p| p.get("name"))
            .and_then(toml::Value::as_str)
            .filter(nonempty)
        else {
            continue;
        };
        let publishes = match package.and_then(|p| p.get("publish")) {
            None => true,
            Some(toml::Value::Boolean(b)) => *b,
            Some(toml::Value::Array(a)) => !a.is_empty(),
            Some(_) => true,
        };
        let under = if home.is_empty() {
            String::new()
        } else {
            format!("{home}/")
        };
        let has_lib =
            table.contains_key("lib") || rels.contains(format!("{under}src/lib.rs").as_str());
        if publishes && has_lib && (depended.contains(home) || !has_bin(table, &under, &rels)) {
            out.insert(ident(name));
        }
    }
    out
}

/// A `[[bin]]` table, a `src/main.rs` or a file under `src/bin/`.
fn has_bin(table: &toml::Table, under: &str, rels: &HashSet<&str>) -> bool {
    table.contains_key("bin")
        || rels.contains(format!("{under}src/main.rs").as_str())
        || rels
            .iter()
            .any(|rel| rel.starts_with(&format!("{under}src/bin/")) && rel.ends_with(SUFFIX))
}

/// Every `path` a dependency table spells, wherever it sits: the plain, dev,
/// build and `[target.'cfg(..)']` ones, and the `[workspace.dependencies]` a
/// member inherits by `workspace = true`.
fn dep_paths(table: &toml::Table) -> Vec<String> {
    let mut out = Vec::new();
    collect_dep_paths(table, &mut out);
    out
}

fn collect_dep_paths(table: &toml::Table, out: &mut Vec<String>) {
    for (key, value) in table {
        let Some(inner) = value.as_table() else {
            continue;
        };
        if key.ends_with("dependencies") {
            out.extend(
                inner
                    .values()
                    .filter_map(toml::Value::as_table)
                    .filter_map(|spec| spec.get("path"))
                    .filter_map(toml::Value::as_str)
                    .map(str::to_string),
            );
        } else {
            collect_dep_paths(inner, out);
        }
    }
}

/// The module path a file's location spells: `src/` is the crate root's own
/// directory, `lib.rs` and `main.rs` name the root and `mod.rs` its
/// directory. `keep` spells every segment, for a name already taken.
fn parts(stem: &str, keep: bool) -> Vec<&str> {
    let stem = stem.strip_prefix("src/").unwrap_or(stem);
    let mut parts: Vec<&str> = stem.split('/').filter(nonempty).collect();
    if keep {
        return parts;
    }
    if parts.last() == Some(&"mod") {
        parts.pop();
    } else if parts == ["lib"] || parts == ["main"] {
        parts.clear();
    }
    parts
}

/// rel to module qname for every `.rs` file. A name two layouts both spell
/// (a crate with `lib.rs` and `main.rs`, a module with both `a.rs` and
/// `a/mod.rs`) goes to the first in discovery order; the rest fall back to
/// their full path.
pub fn module_qname_map(
    crates: &IndexMap<String, String>,
    listing: &Listing,
) -> IndexMap<String, String> {
    // the home prefixes are formatted once: `find` runs per file × per crate,
    // and a root-homed crate keeps the empty catch-all spelling
    let mut homes: Vec<(String, &str)> = crates
        .iter()
        .map(|(c, d)| {
            let prefix = match d.is_empty() {
                true => String::new(),
                false => format!("{d}/"),
            };
            (prefix, c.as_str())
        })
        .collect();
    homes.sort_by_key(|(d, _)| std::cmp::Reverse(d.len()));
    let first = crates.keys().next().map(String::as_str).unwrap_or_default();
    homes.push((String::new(), first));

    let mut out: IndexMap<String, String> = IndexMap::new();
    let mut taken: HashSet<String> = HashSet::new();
    for (_path, rel) in listing {
        if !rel.ends_with(SUFFIX) {
            continue;
        }
        let (krate, inner) = homes
            .iter()
            .find(|(d, _)| d.is_empty() || rel.starts_with(d.as_str()))
            .map(|(d, c)| {
                (
                    *c,
                    if d.is_empty() {
                        rel.as_str()
                    } else {
                        &rel[d.len()..]
                    },
                )
            })
            .expect("the fallback home matches every path");
        let stem = &inner[..inner.len() - SUFFIX.len()];
        let full: Vec<&str> = rel.split('/').collect();
        let spellings = [
            join_path(krate, &parts(stem, false), "::"),
            join_path(krate, &parts(stem, true), "::"),
            join_path(krate, &full, "::"),
        ];
        let qname = spellings
            .into_iter()
            .find(|q| !taken.contains(q))
            .expect("the path spelling is unique per file");
        taken.insert(qname.clone());
        out.insert(rel.clone(), qname);
    }
    out
}
