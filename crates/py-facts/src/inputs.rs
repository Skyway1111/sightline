//! The non-Python inputs a repo declares about itself.
//! Entry points (#32), the type-check scope (#1, #50) and what a
//! distribution packages. Parsed once at build time.

use std::collections::{BTreeSet, HashSet};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use regex::Regex;
use sightline_core::config::Config;
use sightline_core::config::spelled;
use sightline_core::findings::Qname;
use sightline_core::lang::Listing;
use sightline_core::pytext;

use crate::model::is_test_path;
use crate::qnames::project_dirs;

// --- a configparser-shaped INI reader ---------------------------------------

/// One INI file: section name -> option name (lowercased) -> value.
pub type Ini = IndexMap<String, IndexMap<String, String>>;

/// `configparser.ConfigParser.read_string` for the three files that reach
/// it: sections, `key = value` and `key: value`, continuation lines
/// indented deeper than their key, whole-line `#` and `;` comments.
/// `None` is a file Python's parser rejects, which the callers skip.
///
/// A value holding `%(` is one of those: `BasicInterpolation` runs at
/// `get()` time, past every `except configparser.Error` the callers write.
pub fn parse_ini(text: &str) -> Option<Ini> {
    let mut out: Ini = IndexMap::new();
    let mut section: Option<String> = None;
    let mut option: Option<String> = None;
    let mut indent: usize = 0;
    for raw in pytext::splitlines(text) {
        let line = pytext::strip(raw);
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let depth = raw.len() - pytext::lstrip(raw).len();
        if let (Some(sect), Some(opt)) = (&section, &option)
            && depth > indent
        {
            let value = out.get_mut(sect)?.get_mut(opt)?;
            value.push('\n');
            value.push_str(line);
            continue;
        }
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if out.contains_key(header) {
                return None; // DuplicateSectionError
            }
            out.insert(header.to_string(), IndexMap::new());
            section = Some(header.to_string());
            option = None;
            continue;
        }
        let sect = section.clone()?; // MissingSectionHeaderError
        let cut = line.find(['=', ':'])?; // ParsingError
        let name = pytext::lower(pytext::rstrip(&line[..cut]));
        if name.is_empty() {
            return None;
        }
        let value = pytext::lstrip(&line[cut + 1..]).to_string();
        if value.contains("%(") {
            return None;
        }
        let table = out.get_mut(&sect)?;
        if table.contains_key(&name) {
            return None; // DuplicateOptionError
        }
        table.insert(name.clone(), value);
        option = Some(name);
        indent = depth;
    }
    Some(out)
}

fn read_ini(path: &Utf8Path) -> Option<Ini> {
    parse_ini(&read_lossy(path)?)
}

/// A text file read as Python reads it here: UTF-8 with U+FFFD for the rest.
fn read_lossy(path: &Utf8Path) -> Option<String> {
    std::fs::read(path)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn read_toml(path: &Utf8Path) -> Option<toml::Table> {
    read_lossy(path)?.parse::<toml::Table>().ok()
}

// --- the declared type-check scope ------------------------------------------

/// The repo's own type-check scope, as the path segments it names: mypy's
/// `packages`, `modules` and `files`, and pyright's `include`. Empty where
/// the repo declares none, and then everything is in scope.
pub fn typed_scope(root: &Utf8Path) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for file in ["mypy.ini", "setup.cfg"] {
        let path = root.join(file);
        if !path.is_file() {
            continue;
        }
        let Some(ini) = read_ini(&path) else { continue };
        let Some(table) = ini.get("mypy") else {
            continue;
        };
        for key in ["packages", "modules", "files"] {
            let value = table.get(key).map(String::as_str).unwrap_or("");
            names.extend(value.replace('\n', ",").split(',').map(str::to_string));
        }
    }
    let data = read_toml(&root.join("pyproject.toml")).unwrap_or_default();
    let tool = data.get("tool").and_then(toml::Value::as_table);
    for (table, keys) in [
        ("mypy", &["packages", "modules", "files"][..]),
        ("pyright", &["include"][..]),
    ] {
        for key in keys {
            let value = tool.and_then(|t| t.get(table)).and_then(|t| t.get(*key));
            match value {
                Some(toml::Value::String(s)) => names.push(s.clone()),
                Some(toml::Value::Array(items)) => names.extend(
                    items
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_string),
                ),
                _ => {}
            }
        }
    }
    let segments: BTreeSet<String> = names.iter().filter_map(|n| scope_segment(n)).collect();
    segments.into_iter().collect()
}

/// One declared entry as a path fragment: a dotted module name becomes its
/// directories, a glob is cut at its first wildcard.
fn scope_segment(name: &str) -> Option<String> {
    let name = pytext::strip(name).replace('\\', "/");
    let name = pytext::rstrip_chars(pytext::lstrip_chars(&name, "./"), "/");
    let name = name.split('*').next().unwrap_or("");
    let name = pytext::rstrip_chars(name.split('?').next().unwrap_or(""), "/");
    let spelled = if name.contains('/') || name.ends_with(".py") {
        name.to_string()
    } else {
        name.replace('.', "/")
    };
    let out = pytext::strip_chars(&spelled, "/").to_string();
    (!out.is_empty()).then_some(out)
}

// --- entry points -----------------------------------------------------------

/// pyproject's `[project.scripts]`, `[project.gui-scripts]` and
/// `[project.entry-points.*]` objects as written. An installed distribution
/// reaches these over a seam no reference in the tree crosses, so liveness
/// counts them as roots (#32).
pub fn entry_points(root: &Utf8Path) -> Vec<String> {
    let Some(data) = read_toml(&root.join("pyproject.toml")) else {
        return Vec::new();
    };
    let Some(project) = data.get("project").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut groups: Vec<&toml::Value> = Vec::new();
    groups.extend(project.get("scripts"));
    groups.extend(project.get("gui-scripts"));
    if let Some(table) = project.get("entry-points").and_then(toml::Value::as_table) {
        groups.extend(table.values());
    }
    groups
        .into_iter()
        .filter_map(toml::Value::as_table)
        .flat_map(|g| g.values())
        .filter_map(toml::Value::as_str)
        .map(str::to_string)
        .collect()
}

// --- what the tree publishes ------------------------------------------------

/// Module qnames this repo publishes: those a distribution in the tree
/// packages, plus those a docs tree's autodoc names. Their callers live
/// outside the tree, so no in-repo caller set is complete for them. Empty
/// means an application. `[tool.sightline] published` overrides the read.
pub fn published<'a, I>(
    root: &Utf8Path,
    config: &Config,
    listing: &Listing,
    modules: I,
) -> HashSet<Qname>
where
    I: Iterator<Item = (&'a Qname, &'a str)> + Clone,
{
    match config.published {
        Some(false) => return HashSet::new(),
        Some(true) => {
            return modules
                .filter(|(_, rel)| !is_test_path(rel))
                .map(|(q, _)| q.clone())
                .collect();
        }
        None => {}
    }
    let dirs = packaged_dirs(root, listing);
    let named = autodoc_modules(listing);
    modules
        .filter(|(q, rel)| {
            named.contains(&***q) || dirs.iter().any(|d| rel.starts_with(&format!("{d}/")))
        })
        .map(|(q, _)| q.clone())
        .collect()
}

/// The directories the tree's distributions package, relative to the root.
/// A `[project]` with a `[build-system]`, or with a `py.typed` marker, is a
/// distribution, unless it classifies itself `Private :: Do Not Upload`.
pub fn packaged_dirs(root: &Utf8Path, listing: &Listing) -> Vec<String> {
    let typed: HashSet<&str> = listing
        .iter()
        .filter(|(_, rel)| rel.ends_with("py.typed"))
        .map(|(_, rel)| match rel.rfind('/') {
            Some(cut) => &rel[..cut],
            None => rel.as_str(),
        })
        .collect();
    let mut out: BTreeSet<String> = BTreeSet::new();
    for project in project_dirs(root, listing) {
        let Some(data) = read_toml(&project.join("pyproject.toml")) else {
            continue;
        };
        let here = if project == root {
            String::new()
        } else {
            format!("{}/", crate::qnames::under(root, &project))
        };
        let Some(meta) = data.get("project").and_then(toml::Value::as_table) else {
            continue;
        };
        if meta.is_empty() {
            continue;
        }
        let private = meta
            .get("classifiers")
            .and_then(toml::Value::as_array)
            .is_some_and(|c| {
                c.iter()
                    .any(|v| v.as_str() == Some("Private :: Do Not Upload"))
            });
        let ships = data.contains_key("build-system") || typed.iter().any(|t| t.starts_with(&here));
        if private || !ships {
            continue;
        }
        for d in backend_dirs(&project, &data, meta) {
            if d.is_dir() && d != root {
                out.insert(crate::qnames::under(root, &d));
            }
        }
    }
    out.into_iter().collect()
}

/// The directories one distribution packages as its backend declares them:
/// setuptools `packages`, `package-dir` and `packages.find`, hatch
/// `packages` and `only-include`, flit `module`. The fallback is its own
/// name under `src/` or beside it.
fn backend_dirs(project: &Utf8Path, data: &toml::Table, meta: &toml::Table) -> Vec<Utf8PathBuf> {
    let tool = data.get("tool").and_then(toml::Value::as_table);
    let table = |name: &str| {
        tool.and_then(|t| t.get(name))
            .and_then(toml::Value::as_table)
    };
    let st = table("setuptools");
    let pkg_dir = st
        .and_then(|s| s.get("package-dir"))
        .and_then(toml::Value::as_table);
    let mut out: Vec<Utf8PathBuf> = Vec::new();
    if let Some(dirs) = pkg_dir {
        for (key, value) in dirs {
            if !key.is_empty()
                && let Some(p) = value.as_str()
            {
                out.push(project.join(p));
            }
        }
    }
    let root_dir = pkg_dir
        .and_then(|d| d.get(""))
        .and_then(toml::Value::as_str)
        .unwrap_or("");
    match st.and_then(|s| s.get("packages")) {
        Some(toml::Value::Array(pkgs)) => {
            for p in pkgs {
                let spelled = spelled(p).replace('.', "/");
                out.push(project.join(root_dir).join(spelled));
            }
        }
        Some(toml::Value::Table(pkgs)) => {
            if let Some(find) = pkgs.get("find").and_then(toml::Value::as_table) {
                let wheres: Vec<String> = match find.get("where").and_then(toml::Value::as_array) {
                    Some(items) => items.iter().map(spelled).collect(),
                    None => vec![if root_dir.is_empty() {
                        ".".to_string()
                    } else {
                        root_dir.to_string()
                    }],
                };
                for w in wheres {
                    let include = find.get("include").and_then(toml::Value::as_array);
                    let named: Vec<Utf8PathBuf> = include
                        .map(|items| {
                            items
                                .iter()
                                .map(|i| {
                                    let head = spelled(i);
                                    let head = head.split('*').next().unwrap_or("");
                                    project
                                        .join(&w)
                                        .join(pytext::strip_chars(head, ".").replace('.', "/"))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let dirs: Vec<Utf8PathBuf> = named.into_iter().filter(|d| d.is_dir()).collect();
                    if dirs.is_empty() {
                        out.push(project.join(&w));
                    } else {
                        out.extend(dirs);
                    }
                }
            }
        }
        _ => {}
    }
    let build = table("hatch")
        .and_then(|h| h.get("build"))
        .and_then(toml::Value::as_table);
    let wheel = build
        .and_then(|b| b.get("targets"))
        .and_then(toml::Value::as_table)
        .and_then(|t| t.get("wheel"))
        .and_then(toml::Value::as_table);
    for t in [build, wheel].into_iter().flatten() {
        for key in ["packages", "only-include"] {
            if let Some(items) = t.get(key).and_then(toml::Value::as_array) {
                out.extend(items.iter().map(|p| project.join(spelled(p))));
            }
        }
    }
    if !out.is_empty() {
        return out;
    }
    let flit = table("flit")
        .and_then(|f| f.get("module"))
        .and_then(toml::Value::as_table)
        .and_then(|m| m.get("name"));
    // Python chains these with `or`, so an empty flit name falls through
    let name = match flit.map(spelled) {
        Some(name) if !name.is_empty() => name,
        _ => meta.get("name").map(spelled).unwrap_or_default(),
    };
    let stem = pytext::lower(&name.replace(['-', '.'], "_"));
    [project.join("src").join(&stem), project.join(&stem)]
        .into_iter()
        .find(|d| d.is_dir())
        .into_iter()
        .collect()
}

fn autodoc_re() -> &'static Regex {
    static RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"^\s*\.\.\s+auto(module|class)::\s*([\w.]+)$").unwrap()
    });
    &RE
}

/// Modules a docs tree publishes: `automodule` or `autoclass` directives
/// under a `doc*` directory with `:members:`. An autoclass names a class, so
/// its module is the prefix. Read from the listing, not `doc_files`, which a
/// single-file gate build leaves empty.
pub fn autodoc_modules(listing: &Listing) -> HashSet<String> {
    let mut out = HashSet::new();
    for (path, rel) in listing {
        let dirs: Vec<&str> = rel.split('/').collect();
        if !rel.ends_with(".rst") || !dirs[..dirs.len() - 1].iter().any(|p| p.starts_with("doc")) {
            continue;
        }
        let Some(text) = read_lossy(path) else {
            continue;
        };
        let lines = pytext::splitlines(&text);
        for (i, line) in lines.iter().enumerate() {
            let Some(m) = autodoc_re().captures(pytext::rstrip(line)) else {
                continue;
            };
            for option in lines[i + 1..].iter().map(|l| pytext::strip(l)) {
                if option.is_empty() {
                    continue;
                }
                if !option.starts_with(':') {
                    break;
                }
                if option.starts_with(":members:") {
                    let named = &m[2];
                    out.insert(if &m[1] == "module" {
                        named.to_string()
                    } else {
                        pytext::rpartition(named, ".").0.to_string()
                    });
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ini_reader_keeps_continuations_and_lowercases_keys() {
        let ini = parse_ini("[mypy]\nPackages =\n  a\n  b\n; note\nfiles: x\n").unwrap();
        assert_eq!(ini["mypy"]["packages"], "\na\nb");
        assert_eq!(ini["mypy"]["files"], "x");
    }

    #[test]
    fn an_interpolation_makes_the_file_unreadable() {
        assert!(parse_ini("[a]\nk = %(other)s\n").is_none());
        assert!(parse_ini("k = 1\n").is_none());
        assert!(parse_ini("[a]\nk = 1\nk = 2\n").is_none());
    }

    #[test]
    fn a_scope_segment_is_a_path_fragment() {
        assert_eq!(scope_segment("pkg.sub").as_deref(), Some("pkg/sub"));
        assert_eq!(scope_segment("./src/").as_deref(), Some("src"));
        assert_eq!(scope_segment("src/**/*.py").as_deref(), Some("src"));
        assert_eq!(scope_segment("tools/x.py").as_deref(), Some("tools/x.py"));
        assert_eq!(scope_segment("  "), None);
    }
}
