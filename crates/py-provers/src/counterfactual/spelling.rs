//! How a splice spells the type it writes: the names the file already binds,
//! the stdlib chain a bare display needs, and the import lines that carry the
//! rest.

use super::*;

const BUILTIN_TYPE_NAMES: [&str; 15] = [
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "bytearray",
    "list",
    "dict",
    "set",
    "tuple",
    "frozenset",
    "None",
    "object",
    "complex",
    "range",
];

/// `IMPORT_HOME`: where a spliced name is imported from. The abc names
/// accept typing's aliases too (`spell`).
fn import_home(name: &str) -> Option<&'static str> {
    match name {
        "Sequence" | "MutableSequence" | "Mapping" | "MutableMapping" | "Iterable" | "Iterator"
        | "Collection" | "Container" | "Reversible" | "Sized" | "Hashable" => {
            Some("collections.abc")
        }
        "Optional" | "Union" => Some("typing"),
        _ => None,
    }
}

pub(super) static IDENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("a literal pattern"));
// the checker's own displays are not annotations: `A & ~B`, `<subclass of X>`, `@Todo`
static SPELLABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9_\[\], .|]+\z").expect("a literal pattern"));
static FROM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\Afrom (\S+) import (.+)\z").expect("a literal pattern"));

/// How this file spells the annotation: each bare name mapped to its chain
/// here for a name bound only through a stdlib module (`AST` -> `ast.AST`
/// under `import ast`, since the oracle's display is bare and a wrong guess
/// is a world's veto), and the imports it needs (`import_home`). Any other
/// name rides only where the module binds it at module scope to a class (a
/// repo class, `from pathlib import Path`): no import line, no cycle to
/// judge, and a wrong class errors at every caller. `None` when the
/// spelling is unsafe: unbound everywhere (`Unknown`), or bound to
/// something else. Without an oracle no bare name has a stdlib home.
pub fn spell(
    annotation: &str,
    module: &Module<'_>,
    facts: &RepoFacts<'_>,
    oracle: Option<&Oracle>,
) -> Option<(IndexMap<String, String>, Vec<String>)> {
    let mut needed: IndexSet<&str> = IndexSet::new();
    let mut respelled: IndexMap<String, String> = IndexMap::new();
    if !annotation.is_empty() && !SPELLABLE_RE.is_match(annotation) {
        return None;
    }
    for found in IDENT_RE.find_iter(annotation) {
        let name = found.as_str();
        if BUILTIN_TYPE_NAMES.contains(&name) {
            continue;
        }
        let bound = module.bindings.get(name).map(|q| &**q);
        match (import_home(name), bound) {
            (None, None) => {
                respelled.insert(name.to_string(), stdlib_home(name, module, oracle?)?);
            }
            (None, Some(bound)) if !binds_class(bound, name, facts) => return None,
            (Some(_), None) => {
                needed.insert(name);
            }
            // the name means something else in this module
            (Some(home), Some(bound))
                if bound != format!("{home}.{name}") && bound != format!("typing.{name}") =>
            {
                return None;
            }
            _ => {}
        }
    }
    let imports = merge_imports(needed.iter().map(|n| {
        let home = import_home(n).expect("a name reaches `needed` only through its home");
        format!("from {home} import {n}")
    }));
    Some((respelled, imports))
}

/// `local.name` through the one stdlib module the file binds holding a class
/// so named, else `None`.
fn stdlib_home(name: &str, module: &Module<'_>, oracle: &Oracle) -> Option<String> {
    let mut homes: Vec<String> = Vec::new();
    for (local, target) in &module.bindings {
        let head = target.split('.').next().unwrap_or("");
        if !is_known_standard_library(14, head) || &**local == name {
            continue;
        }
        if oracle.member_is_class(&module.rel, local, name) {
            homes.push(format!("{local}.{name}"));
        }
    }
    (homes.len() == 1).then(|| homes.swap_remove(0))
}

/// The edit's text with each bare name spelled as `spell` found it.
pub fn respell(text: &str, respelled: &IndexMap<String, String>) -> String {
    IDENT_RE
        .replace_all(text, |caps: &Captures| {
            let name = &caps[0];
            respelled.get(name).map_or(name, String::as_str).to_string()
        })
        .into_owned()
}

fn binds_class(bound: &str, name: &str, facts: &RepoFacts<'_>) -> bool {
    let (kind, q) = resolve_qname(bound, facts, 0);
    match kind {
        "symbol" => facts.symbols.get(&*q).is_some_and(|s| s.kind == "class"),
        "external" => q.rsplit('.').next() == Some(name),
        _ => false,
    }
}

/// The home of a `from X import ...` line, `None` for any other import
/// statement. `emit` reads it to grow a line the file already holds.
pub fn from_home(stmt: &str) -> Option<&str> {
    FROM_RE
        .captures(stmt)
        .map(|c| c.get(1).expect("the home group").as_str())
}

/// Import statements in as few lines as possible: one `from` line per home
/// (names and homes sorted), plain `import x` lines as written.
pub fn merge_imports<I: IntoIterator<Item = String>>(stmts: I) -> Vec<String> {
    let mut homes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut plain: BTreeSet<String> = BTreeSet::new();
    for stmt in stmts {
        match FROM_RE.captures(&stmt) {
            Some(caps) => homes
                .entry(caps[1].to_string())
                .or_default()
                .extend(caps[2].split(',').map(|n| pytext::strip(n).to_string())),
            None => {
                plain.insert(stmt);
            }
        }
    }
    let mut out: Vec<String> = plain.into_iter().collect();
    out.extend(homes.into_iter().map(|(home, names)| {
        let names: Vec<String> = names.into_iter().collect();
        format!("from {home} import {}", names.join(", "))
    }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_imports_folds_one_from_line_per_home() {
        assert_eq!(
            merge_imports(
                [
                    "from typing import Optional",
                    "import os",
                    "from collections.abc import Sequence",
                    "from typing import Union",
                    "import abc",
                ]
                .map(str::to_string)
            ),
            [
                "import abc",
                "import os",
                "from collections.abc import Sequence",
                "from typing import Optional, Union",
            ]
        );
        // a `from` line already holding several names splits on the commas
        assert_eq!(
            merge_imports(["from typing import Union, Optional".to_string()]),
            ["from typing import Optional, Union"]
        );
    }

    #[test]
    fn respell_rewrites_whole_names_only() {
        let respelled = IndexMap::from([("AST".to_string(), "ast.AST".to_string())]);
        assert_eq!(
            respell(": list[AST] | AST", &respelled),
            ": list[ast.AST] | ast.AST"
        );
        assert_eq!(respell(": ASTx", &respelled), ": ASTx");
    }
}
