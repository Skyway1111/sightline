//! What a crate's outside can name (port of `rs/exports.py`): the definition
//! a `pub use` re-exports, and the items a publishable lib crate's root
//! reaches, both off what `build.rs` collected. Whether an unreached `pub`
//! item is dead is a rule's question.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use sightline_core::findings::Qname;
use sightline_core::lang::Listing;
use sightline_core::pytext;

use crate::crates::lib_crates;
use crate::model::{RsFacts, RsSymbol, is_fn_kind};

/// A re-export chain past this is a cycle, not a chain.
const HOPS: usize = 8;

/// The definition a spelling names: its longest aliased prefix replaced,
/// until nothing moves. A cycle stops at the last spelling it reached.
pub fn follow(aliases: &IndexMap<String, String>, path: &str) -> String {
    if aliases.is_empty() {
        return path.to_string();
    }
    let mut cur = path.to_string();
    for _ in 0..HOPS {
        let mut head: &str = &cur;
        let mut rest = String::new();
        while !head.is_empty() && !aliases.contains_key(head) {
            let (before, _, last) = pytext::rpartition(head, "::");
            rest = if rest.is_empty() {
                last.to_string()
            } else {
                format!("{last}::{rest}")
            };
            head = before;
        }
        if head.is_empty() {
            return cur;
        }
        let target = &aliases[head];
        let next = if rest.is_empty() {
            target.clone()
        } else {
            format!("{target}::{rest}")
        };
        cur = next;
    }
    cur
}

/// Re-export alias qname to the definition it names, chains resolved: a
/// `pub use a::b::C` in module M makes `M::C` a second spelling of `C`. Two
/// are not aliases: a spelling the repo already defines (Rust's namespaces
/// are separate, so salvo's `pub use salvo_macros::handler` beside its own
/// `handler` module renames nothing), and one naming something outside.
pub fn aliases(facts: &RsFacts<'_>) -> IndexMap<String, String> {
    let known: HashSet<&str> = facts
        .symbols
        .keys()
        .chain(facts.modules.keys())
        .map(|q| &**q)
        .collect();
    let mut raw: IndexMap<String, String> = IndexMap::new();
    for module in facts.modules.values() {
        for (local, target) in &module.reexports {
            if local == "*" {
                continue;
            }
            let alias = format!("{}::{local}", module.qname);
            if !known.contains(alias.as_str()) {
                raw.insert(alias, target.clone());
            }
        }
    }
    let mut out: IndexMap<String, String> = IndexMap::new();
    for (alias, target) in &raw {
        let end = follow(&raw, target);
        if known.contains(end.as_str()) {
            out.insert(alias.clone(), end);
        }
    }
    out
}

/// The symbol qnames a downstream user can reach: bare-`pub` items of a
/// publishable lib crate, reachable from its root. A bin crate, a
/// `publish = false` crate and an application (a lib beside its own bin
/// that no manifest path-depends on) publish nothing; `[tool.sightline]
/// published` overrides the manifest read either way.
pub fn published(
    facts: &RsFacts<'_>,
    listing: &Listing,
    manifests: &[(String, toml::Table)],
) -> HashSet<Qname> {
    if facts.config.published == Some(false) {
        return HashSet::new();
    }
    let libs: HashSet<String> = if facts.config.published == Some(true) {
        facts.crates.keys().cloned().collect()
    } else {
        lib_crates(listing, manifests)
    };
    reachable(facts, &libs)
}

/// The bare-`pub` items the roots of `libs` reach through `pub mod` chains
/// and `pub use` re-exports. `published` is this over the crates a
/// downstream user can name; over every crate it is instead the set rustc's
/// `dead_code` stays silent on, bin crate and lib alike (#32's judged set).
pub fn reachable(facts: &RsFacts<'_>, libs: &HashSet<String>) -> HashSet<Qname> {
    let scopes = module_scopes(facts, libs);
    let mut out: HashSet<Qname> = HashSet::new();
    for (alias, end) in &facts.aliases {
        if scopes.contains(pytext::rpartition(alias, "::").0) && ships(facts, end) {
            out.insert(end.as_str().into());
        }
    }
    let mut ordered: Vec<&Qname> = facts.symbols.keys().collect();
    ordered.sort_by(|a, b| (a.matches("::").count(), &***a).cmp(&(b.matches("::").count(), &***b)));
    for qname in ordered {
        let parent = pytext::rpartition(qname, "::").0;
        let nested_in_a_type = out.contains(parent)
            && facts
                .symbols
                .get(parent)
                .is_some_and(|s| !is_fn_kind(s.kind));
        if ships(facts, qname) && (scopes.contains(parent) || nested_in_a_type) {
            out.insert(qname.clone());
        }
    }
    out
}

fn ships(facts: &RsFacts<'_>, qname: &str) -> bool {
    facts
        .symbols
        .get(qname)
        .is_some_and(|s: &RsSymbol<'_>| s.is_public && !s.is_test)
}

/// Every scope a publishable crate root reaches: the `pub mod` chains, and a
/// `pub use` naming a module (a glob's `use m::*` included), which lends
/// that module the scope re-exporting it. A `pub use` inside an inline `mod`
/// reads at the file's scope.
fn module_scopes(facts: &RsFacts<'_>, libs: &HashSet<String>) -> HashSet<String> {
    let mut children: HashMap<String, HashSet<String>> = HashMap::new();
    for module in facts.modules.values() {
        for child in &module.pub_mods {
            children
                .entry(pytext::rpartition(child, "::").0.to_string())
                .or_default()
                .insert(child.clone());
        }
        for (_local, target) in &module.reexports {
            let end = follow(&facts.aliases, target);
            if facts.modules.contains_key(end.as_str()) {
                children
                    .entry(module.qname.to_string())
                    .or_default()
                    .insert(end);
            }
        }
    }
    let mut out: HashSet<String> = facts
        .modules
        .values()
        .filter(|m| *m.qname == m.crate_name && libs.contains(&m.crate_name))
        .map(|m| m.qname.to_string())
        .collect();
    let mut frontier: Vec<String> = out.iter().cloned().collect();
    while let Some(scope) = frontier.pop() {
        for child in children.get(&scope).into_iter().flatten() {
            if out.insert(child.clone()) {
                frontier.push(child.clone());
            }
        }
    }
    out
}
