//! The `scope` dump layer.

use super::*;

/// One footprint row of the layer.
pub(super) fn footprint_row(fp: &Footprint) -> Value {
    let mut forwarded: Vec<Value> = fp
        .forwarded
        .iter()
        .map(|(q, at)| json!([q.to_string(), at]))
        .collect();
    forwarded.sort_by_cached_key(|row| {
        (
            row[0].as_str().unwrap_or("").to_string(),
            row[1].as_u64().unwrap_or(0),
        )
    });
    json!({
        "attrs": fp.attrs,
        "called": fp.called,
        "subscripted": fp.subscripted,
        "sub_stored": fp.sub_stored,
        "iterated": fp.iterated,
        "sized": fp.sized,
        "contained": fp.contained,
        "mutated": fp.mutated,
        "forwarded": forwarded,
        "other": fp.other,
    })
}

pub(super) fn scope_row(facts: &RepoFacts<'_>, scope: &Scope) -> Value {
    let module = scope.module(facts);
    json!({
        "params": scope.params(facts),
        "declared": scope.declared(facts),
        "loops": scope.loops(facts).iter().map(|(a, b)| [a, b]).collect::<Vec<_>>(),
        "rebindings": scope.rebindings(facts).iter().map(|w| json!({
            "root": w.root,
            "kind": w.kind,
            "line": module.line_of(w.node),
            "decl": w.decl,
        })).collect::<Vec<_>>(),
        "stored": scope.stored(facts),
        "outer_names": scope.outer_names(facts),
        "alias_tainted": scope.alias_tainted(facts),
        "guards": scope.guards(facts).iter().map(|g| json!({
            "param": g.param,
            "kind": g.kind,
            "classes": g.classes,
            "line": module.line_of(g.node),
        })).collect::<Vec<_>>(),
        "footprints": scope.footprints(facts).iter()
            .map(|(p, fp)| (p.clone(), footprint_row(fp)))
            .collect::<serde_json::Map<String, Value>>(),
        "mutated_params": scope.mutated_params(facts),
        "mutates_alias": scope.mutates_alias(facts),
    })
}

/// `layer_scope`.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let rows: Vec<(String, Value)> = functions(facts)
        .into_par_iter()
        .filter_map(|q| {
            let scope = provers.scope_of(facts, q)?;
            Some((q.to_string(), scope_row(facts, scope)))
        })
        .collect();
    Some(json!({
        "functions": rows.into_iter().collect::<serde_json::Map<String, Value>>(),
    }))
}
