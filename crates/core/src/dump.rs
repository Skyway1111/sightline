//! The `neutral` layer of `debug dump`: what suppress, rank and render read
//! of one stack, and the marker lines they build their own table from.

use std::collections::HashMap;

use indexmap::IndexSet;
use serde_json::{Map, Value, json};

use crate::lang::Neutral;
use crate::pyjson::object;

/// The `neutral` dump layer: what suppress, rank and render read (this
/// view), plus the marker lines they build their own table from. One home
/// for both stacks: the view is all it reads.
#[must_use]
#[allow(clippy::implicit_hasher, reason = "the registry's own map")]
pub fn neutral_layer(view: &Neutral, ids_by_slug: &HashMap<String, String>) -> Value {
    let mut suppressions = Map::new();
    let mut markers = Map::new();
    let code = view
        .modules
        .values()
        .map(|m| (&*m.rel, &m.lines, view.comment_prefix, false));
    let docs = view
        .doc_files
        .iter()
        .map(|(rel, lines)| (&**rel, lines, "<!--", true));
    for (rel, lines, prefix, is_doc) in code.chain(docs) {
        let pattern = if is_doc {
            crate::suppress::doc_suppress_re().clone()
        } else {
            crate::suppress::suppress_pattern(prefix)
        };
        let table = crate::suppress::marker_table(lines, &pattern, prefix, ids_by_slug);
        if !table.is_empty() {
            let mut rows: Vec<(u32, &IndexSet<String>)> =
                table.iter().map(|(n, ids)| (*n, ids)).collect();
            rows.sort_by_key(|(n, _)| *n);
            suppressions.insert(
                rel.to_string(),
                object(rows.into_iter().map(|(n, ids)| {
                    let mut sorted: Vec<&str> = ids.iter().map(String::as_str).collect();
                    sorted.sort_unstable();
                    (n.to_string(), Value::from(sorted))
                })),
            );
        }
        let hits: Vec<(String, Value)> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains("sightline-ok"))
            .map(|(i, line)| ((i + 1).to_string(), Value::from(&**line)))
            .collect();
        if !hits.is_empty() {
            markers.insert(rel.to_string(), object(hits));
        }
    }
    let modules: Vec<Value> = view
        .modules
        .values()
        .map(|m| {
            json!({
                "qname": &*m.qname,
                "rel": &*m.rel,
                "lines": m.lines.len(),
                "comment_prefix": view.comment_prefix,
                "is_test": (view.is_test)(&m.rel),
            })
        })
        .collect();
    let symbols = object(view.symbols.iter().map(|(q, s)| {
        (
            q.to_string(),
            json!({
                "module": &*s.module,
                "kind": s.kind,
                "lineno": s.lineno,
                "end_lineno": s.end_lineno,
            }),
        )
    }));
    let doc_files = object(view.doc_files.iter().map(|(rel, lines)| {
        (
            rel.to_string(),
            Value::from(lines.iter().map(|l| &**l).collect::<Vec<_>>()),
        )
    }));
    json!({
        "languages": [view.lang],
        "modules": modules,
        "doc_files": doc_files,
        "symbols": symbols,
        "errors": view.errors,
        "fan_in": object(
            view.fan_in.iter().map(|(q, n)| (q.to_string(), Value::from(*n))),
        ),
        "cc": object(view.cc.iter().map(|(q, n)| (q.to_string(), Value::from(*n)))),
        "suppressions": suppressions,
        "markers": markers,
    })
}
