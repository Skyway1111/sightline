//! The four dump layers the Rust provers answer: `rs-bodies`,
//! `rs-graph`, `rs-world` and `rs-clones`.

use serde_json::{Value, json};

use sightline_core::pyjson::object;
use sightline_rs_facts::model::{RsFacts, RsSymbol, is_fn_kind};

use crate::oracle::RsDiag;
use crate::{RsCall, RsProvers};

/// `_functions`: every symbol a `fn` backs, in facts order.
fn functions<'f, 't>(facts: &'f RsFacts<'t>) -> Vec<(&'f str, &'f RsSymbol<'t>)> {
    facts
        .symbols
        .iter()
        .filter(|(_, s)| is_fn_kind(s.kind))
        .map(|(q, s)| (&**q, s))
        .collect()
}

fn calls(rows: &[RsCall<'_>]) -> Value {
    rows.iter()
        .map(|c| json!([c.name, c.path, c.line]))
        .collect()
}

pub fn rs_bodies(facts: &RsFacts<'_>, provers: &RsProvers<'_>) -> Value {
    let mut bodies: Vec<(String, Value)> = Vec::new();
    let mut docs: Vec<(String, Value)> = Vec::new();
    for (qname, sym) in functions(facts) {
        let body = provers.body(qname);
        bodies.push((
            qname.to_string(),
            json!({
                "calls": calls(&body.calls),
                "macros": calls(&body.macros),
                "unsafe": body.unsafe_blocks.iter()
                    .map(|n| n.start_position().row as u32 + 1).collect::<Vec<_>>(),
                // a closure's digest and key hash this tool's own shape text
                "closures": body.closures.iter()
                    .map(|c| json!({"line": c.line, "size": c.size, "forwards": c.forwards}))
                    .collect::<Vec<_>>(),
                "allows": body.allows,
                "tries": body.tries,
                "returns": provers.returns(qname),
            }),
        ));
        let run = provers.doc_above(&facts.modules[&sym.module], sym.lineno);
        if !run.is_empty() {
            docs.push((qname.to_string(), json!(run)));
        }
    }
    json!({
        "bodies": object(bodies),
        "comment_blocks": object(provers.comment_blocks().iter()
            .filter(|(_, blocks)| !blocks.is_empty())
            .map(|(q, blocks)| (q.to_string(), blocks.iter()
                .map(|b| json!({
                    "start": b.start, "lines": b.lines, "code": b.code(), "label": b.label,
                }))
                .collect::<Value>()))),
        "docs": object(docs),
        "module_docs": object(provers.module_docs().iter()
            .filter(|(_, d)| !d.is_empty())
            .map(|(q, d)| (q.to_string(), json!(d)))),
        "trait_impls": object(provers.trait_impls().iter()
            .map(|(t, qs)| (t.clone(), json!(qs)))),
        "allows": object(provers.allows().iter()
            .filter(|(_, rows)| !rows.is_empty())
            .map(|(q, rows)| (q.to_string(), rows.iter()
                .map(|a| json!({"names": a.names, "line": a.line}))
                .collect::<Value>()))),
        "instantiations": object(provers.instantiations().iter().map(|(q, u)| (
            q.to_string(),
            json!({"params": u.params, "inferred": u.inferred, "spelled": u.spelled}),
        ))),
        "unindexed_names": provers.unindexed_names().iter().collect::<Vec<_>>(),
    })
}

pub fn rs_graph(provers: &RsProvers<'_>) -> Value {
    let answers = provers.rust;
    let mut diagnostics: Vec<(String, u32, String, String)> =
        answers.diagnostics.iter().map(RsDiag::key).collect();
    diagnostics.sort_unstable();
    json!({
        "edges": answers.graph.edges.iter()
            .map(|e| json!({
                "caller": e.caller, "callee": e.callee, "rel": e.rel, "line": e.line,
                "call": e.call, "open": e.open,
            }))
            .collect::<Vec<_>>(),
        "counts": object(answers.graph.counts.iter().map(|(k, v)| (k.clone(), json!(v)))),
        "members": answers.oracle.iter().flat_map(|o| o.members())
            .map(|m| json!({"name": m.name, "dir": m.dir, "kind": m.kind}))
            .collect::<Vec<_>>(),
        "checked": answers.checked.iter().map(|m| &m.name).collect::<Vec<_>>(),
        "unchecked": answers.unchecked.iter().collect::<Vec<_>>(),
        "diagnostics": diagnostics.iter()
            .map(|(rel, line, code, message)| json!([rel, line, code, message]))
            .collect::<Vec<_>>(),
    })
}

pub fn rs_world(facts: &RsFacts<'_>, provers: &RsProvers<'_>) -> Value {
    let world = provers.closed_world();
    let rows = functions(facts).into_iter().map(|(qname, _)| {
        let v = world.verdict(qname);
        let mut reasons: Vec<&str> = v.reasons.iter().map(String::as_str).collect();
        reasons.sort_unstable();
        (
            qname.to_string(),
            json!({"passed": v.passed, "reason": v.reason, "reasons": reasons}),
        )
    });
    let rows = object(rows.collect::<Vec<_>>());
    let mut reachable: Vec<&str> = world.reachable().iter().map(|q| &**q).collect();
    reachable.sort_unstable();
    json!({"functions": rows, "reachable": reachable})
}

/// One member of a printed group: rel, owner, first line, last line.
type Member = (String, String, u32, u32);

fn groups(mut rows: Vec<Vec<Member>>) -> Value {
    for group in &mut rows {
        group.sort();
    }
    rows.sort();
    rows.iter()
        .map(|group| {
            group
                .iter()
                .map(|(rel, owner, first, last)| json!([rel, owner, first, last]))
                .collect::<Value>()
        })
        .collect()
}

pub fn rs_clones(facts: &RsFacts<'_>, provers: &RsProvers<'_>) -> Value {
    let mut by_digest: indexmap::IndexMap<&str, Vec<Member>> = indexmap::IndexMap::new();
    for (qname, key) in provers.function_digests() {
        let sym = &facts.symbols[qname];
        by_digest.entry(key.as_str()).or_default().push((
            facts.modules[&sym.module].rel.to_string(),
            qname.to_string(),
            sym.lineno,
            sym.end_lineno,
        ));
    }
    let function_groups: Vec<Vec<Member>> = by_digest
        .into_values()
        .filter(|group| group.len() > 1)
        .collect();
    let block_groups: Vec<Vec<Member>> = provers
        .block_clones()
        .iter()
        .map(|group| {
            group
                .members
                .iter()
                .map(|(sym, nodes)| {
                    (
                        facts.modules[&sym.module].rel.to_string(),
                        sym.qname.to_string(),
                        nodes[0].start_position().row as u32 + 1,
                        nodes[nodes.len() - 1].end_position().row as u32 + 1,
                    )
                })
                .collect()
        })
        .collect();
    json!({"functions": groups(function_groups), "blocks": groups(block_groups)})
}
