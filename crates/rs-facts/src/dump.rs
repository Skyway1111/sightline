//! The dump layers this stack answers.

use serde_json::{Value, json};

use sightline_core::pyjson::object;

use crate::model::RsFacts;

/// `layer_listing` on a Rust tree: the shared walk, and the repo-wide
/// inputs a Rust tree has none of.
pub fn listing(facts: &RsFacts<'_>) -> Value {
    json!({
        "files": facts.all_files.iter().map(|r| &**r).collect::<Vec<_>>(),
        "import_roots": Vec::<String>::new(),
        "entry_points": Vec::<String>::new(),
        "typed_scope": Vec::<String>::new(),
        "published": Vec::<String>::new(),
    })
}

/// `layer_rs_facts`, row for row.
pub fn rs_facts(facts: &RsFacts<'_>) -> Value {
    let mut published: Vec<&str> = facts.published.iter().map(|q| &**q).collect();
    published.sort_unstable();
    let modules: Vec<Value> = facts
        .modules
        .values()
        .map(|m| {
            let mut pub_mods: Vec<&str> = m.pub_mods.iter().map(String::as_str).collect();
            pub_mods.sort_unstable();
            json!({
                "qname": &*m.qname,
                "rel": &*m.rel,
                "crate": m.crate_name,
                "lines": m.lines.len(),
                "bindings": object(
                    m.bindings.iter().map(|(k, v)| (k.clone(), Value::from(v.clone()))),
                ),
                "items": object(
                    m.items.iter().map(|(k, v)| (k.clone(), Value::from(&**v))),
                ),
                "reexports": m.reexports.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(),
                "pub_mods": pub_mods,
                "doc": m.doc,
            })
        })
        .collect();
    let symbols: Vec<Value> = facts
        .symbols
        .values()
        .map(|s| {
            json!({
                "qname": &*s.qname,
                "module": &*s.module,
                "name": s.name,
                "kind": s.kind,
                "lineno": s.lineno,
                "end_lineno": s.end_lineno,
                "is_public": s.is_public,
                "parent": s.parent.as_deref(),
                "attrs": s.attrs,
                "traits": s.traits,
                "is_test": s.is_test,
            })
        })
        .collect();
    json!({
        "crates": object(
            facts.crates.iter().map(|(k, v)| (k.clone(), Value::from(v.clone()))),
        ),
        "published": published,
        "aliases": object(
            facts.aliases.iter().map(|(k, v)| (k.clone(), Value::from(v.clone()))),
        ),
        "modules": modules,
        "symbols": symbols,
        "refs": facts.refs.iter().map(|r| json!({
            "module": &*r.module,
            "target": r.target,
            "kind": r.kind.value(),
            "line": r.lineno,
        })).collect::<Vec<_>>(),
        "call_sites": facts.call_sites.iter().map(|s| json!({
            "module": &*s.module,
            "enclosing": &*s.enclosing,
            "resolution": s.resolution.value(),
            "target": s.target,
            "line": s.lineno,
        })).collect::<Vec<_>>(),
        "impls": facts.impls.iter().map(|i| json!({
            "module": &*i.module,
            "trait": i.trait_name,
            "type_name": i.type_name,
            "type_qname": i.type_qname,
            "line": i.lineno,
        })).collect::<Vec<_>>(),
        "errors": facts.errors,
    })
}
