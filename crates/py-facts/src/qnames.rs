//! `facts/build.py`: the module naming, over the listing and the file
//! system. Path logic only, so the gate computes full-build identities
//! without parsing.

use crate::model::RepoFacts;
use crate::module::Module;
use ruff_python_ast::Expr;
use sightline_core::findings::Qname;
use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexMap;
use sightline_core::lang::Listing;

/// The root and every directory below it holding its own `pyproject.toml`,
/// which is a uv or hatch workspace member.
pub fn project_dirs(root: &Utf8Path, listing: &Listing) -> Vec<Utf8PathBuf> {
    let mut out = vec![root.to_path_buf()];
    for (_, rel) in listing {
        if let Some(dir) = rel.strip_suffix("/pyproject.toml") {
            out.push(root.join(dir));
        }
    }
    out
}

/// A directory an import resolves against: it exists and is not itself a
/// package. `src.calculator` is how a repo whose `src` is a package spells
/// its own imports.
fn import_root(d: &Utf8Path) -> Option<Utf8PathBuf> {
    (d.is_dir() && !d.join("__init__.py").is_file()).then(|| d.to_path_buf())
}

/// The paths an import in this tree resolves against: the root and its
/// `src/`, plus, per workspace member, the member's `src/` or, in a flat
/// layout, the member itself. Deepest first, since the oracle names a
/// file's module by the first search path holding it.
pub fn import_roots(root: &Utf8Path, listing: &Listing) -> Vec<Utf8PathBuf> {
    let mut out = vec![root.to_path_buf()];
    out.extend(import_root(&root.join("src")));
    for d in project_dirs(root, listing) {
        if d != root {
            match import_root(&d.join("src")) {
                Some(src) => out.push(src),
                None => out.extend(import_root(&d)),
            }
        }
    }
    // stable, so equal depths keep discovery order
    out.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    out
}

/// rel -> module qname for every `.py` file, the collision fallback applied
/// in discovery order. The walk up stops at an `import_roots` entry; below
/// one, a directory without `__init__.py` is a namespace package only where
/// it holds a regular package.
pub fn module_qname_map(root: &Utf8Path, listing: &Listing) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    let mut taken: HashSet<String> = HashSet::new();
    let roots: HashSet<Utf8PathBuf> = import_roots(root, listing).into_iter().collect();
    let packaged: HashSet<&str> = listing
        .iter()
        .filter(|(_, rel)| rel.ends_with("/__init__.py") && rel.matches('/').count() >= 2)
        .map(|(_, rel)| {
            let cut = rel.len() - "/__init__.py".len();
            &rel[..rel[..cut].rfind('/').unwrap_or(0)]
        })
        .collect();

    // a directory is a package when the walk listed its `__init__.py`: the
    // listing already holds every reachable file, so the walk up costs no
    // stat per directory
    let init_dirs: HashSet<&Utf8Path> = listing
        .iter()
        .filter(|(_, rel)| rel.as_str() == "__init__.py" || rel.ends_with("/__init__.py"))
        .filter_map(|(path, _)| path.parent())
        .collect();
    for (path, rel) in listing {
        if !rel.ends_with(".py") {
            continue;
        }
        let mut parts: Vec<&str> = Vec::new();
        let mut dir = path.parent();
        while let Some(d) = dir {
            if roots.contains(d) {
                break;
            }
            let package = init_dirs.contains(d);
            // `packaged` holds posix rels, and a Windows `strip_prefix`
            // answers with the separator the join used
            let under = || {
                d.strip_prefix(root)
                    .ok()
                    .map(|u| u.as_str().replace('\\', "/"))
            };
            if !(package
                || !parts.is_empty()
                || under().is_some_and(|u| packaged.contains(u.as_str())))
            {
                break;
            }
            parts.push(d.file_name().unwrap_or(""));
            dir = d.parent();
        }
        parts.reverse();
        let name = path.file_name().unwrap_or("");
        let stem = name.strip_suffix(".py").unwrap_or(name);
        if name != "__init__.py" {
            parts.push(stem);
        }
        let mut q = parts.join(".");
        if q.is_empty() {
            q = stem.to_string();
        }
        if taken.contains(&q) {
            // collision: fall back to the path-derived name, which for a
            // module at the root is the colliding name again (`pkg.py`
            // beside a `pkg/` package), so keep the suffix and then number
            // until the name is free. A qname is the key of
            // `facts.modules`, and a second module claiming one costs the
            // first its entry.
            let path = rel.replace('/', ".");
            q = path[..path.len() - 3].to_string();
            let mut n = 1;
            while taken.contains(&q) {
                q = if n == 1 {
                    path.clone()
                } else {
                    format!("{path}.{n}")
                };
                n += 1;
            }
        }
        taken.insert(q.clone());
        out.insert(rel.clone(), q);
    }
    out
}

/// A path as a layer names it: posix and relative to the audited root, so a
/// worktree's temporary prefix never reaches a dump.
pub fn under(root: &Utf8Path, path: &Utf8Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) if rel.as_str().is_empty() => ".".to_string(),
        Ok(rel) => rel.as_str().replace('\\', "/"),
        Err(_) => path.as_str().replace('\\', "/"),
    }
}

// --- dotted-name resolution (`build.py:resolve_qname`) ----------------------

/// A `Name` or `Attribute` chain to the repo symbol or module it denotes
/// through the module's bindings, re-export hops included; `None` where it
/// is external or unbound.
pub fn resolve_dotted_expr(
    expr: &Expr,
    module: &Module<'_>,
    facts: &RepoFacts<'_>,
) -> Option<Qname> {
    let q = module.dotted_name(expr)?;
    match resolve_qname(&q, facts, 0) {
        ("symbol" | "module", resolved) => Some(resolved),
        _ => None,
    }
}

/// Global dotted-name resolution to `(kind, qname)`, where kind is
/// "symbol", "module", "external" or "unresolved". Follows one-hop
/// re-exports through module bindings, capped by depth.
pub fn resolve_qname(q: &str, facts: &RepoFacts<'_>, depth: u32) -> (&'static str, Qname) {
    if facts.symbols.contains_key(q) {
        return ("symbol", q.into());
    }
    if facts.modules.contains_key(q) {
        return ("module", q.into());
    }
    let parts: Vec<&str> = q.split('.').collect();
    for i in (1..parts.len()).rev() {
        let head = parts[..i].join(".");
        let Some(module) = facts.modules.get(head.as_str()) else {
            continue;
        };
        let rest = &parts[i..];
        if let Some(redirect) = module.bindings.get(rest[0])
            && depth < 10
        {
            let redirected: String = std::iter::once(&**redirect)
                .chain(rest[1..].iter().copied())
                .collect::<Vec<_>>()
                .join(".");
            if redirected != q {
                return resolve_qname(&redirected, facts, depth + 1);
            }
        }
        // an attribute of an internal module we cannot find
        return ("unresolved", q.into());
    }
    ("external", q.into())
}

/// `M.name` where `q` reaches through the import binding `name` of its
/// longest module prefix M and no symbol of M is spelled so: the re-export
/// hop `resolve_qname` follows, and a use of M's own alias (#32).
pub fn import_alias(q: &str, facts: &RepoFacts<'_>) -> Option<Qname> {
    if facts.symbols.contains_key(q) || facts.modules.contains_key(q) {
        return None;
    }
    let parts: Vec<&str> = q.split('.').collect();
    for i in (1..parts.len()).rev() {
        let head = parts[..i].join(".");
        let Some(module) = facts.modules.get(head.as_str()) else {
            continue;
        };
        let alias = format!("{head}.{}", parts[i]);
        let bound =
            !facts.symbols.contains_key(alias.as_str()) && module.bindings.contains_key(parts[i]);
        return bound.then(|| alias.as_str().into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(dir: &Utf8Path, rels: &[&str]) -> Listing {
        rels.iter().map(|r| (dir.join(r), r.to_string())).collect()
    }

    #[test]
    fn a_src_package_keeps_its_own_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(tmp.path()).unwrap();
        for rel in ["src/calculator/__init__.py", "src/calculator/core.py"] {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "").unwrap();
        }
        let listed = listing(
            root,
            &["src/calculator/__init__.py", "src/calculator/core.py"],
        );
        let map = module_qname_map(root, &listed);
        assert_eq!(map["src/calculator/__init__.py"], "calculator");
        assert_eq!(map["src/calculator/core.py"], "calculator.core");
    }

    #[test]
    fn a_loose_module_beside_a_package_shares_its_namespace() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(tmp.path()).unwrap();
        for rel in ["pkg/sub/__init__.py", "pkg/loose.py"] {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "").unwrap();
        }
        let listed = listing(root, &["pkg/loose.py", "pkg/sub/__init__.py"]);
        let map = module_qname_map(root, &listed);
        assert_eq!(map["pkg/loose.py"], "pkg.loose");
        assert_eq!(map["pkg/sub/__init__.py"], "pkg.sub");
    }
}
