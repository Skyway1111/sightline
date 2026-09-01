//! The `verify` layer of `debug dump`.

use super::*;

/// Every splice the audit's own passes proposed, in call order and numbered
/// by the `verify_splice` call that judged it, then every `verify_worlds`
/// call in the order the passes made them (`Oracle::world_calls`). Without an
/// oracle, the empty document.
pub fn dump(_facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let Some(oracle) = provers.oracle() else {
        return Some(json!({"splices": [], "calls": []}));
    };
    let mut rows: Vec<(Proposal, Outcome, i64)> = Vec::new();
    for (pass, (proposals, outcomes)) in provers.splice_passes().into_iter().enumerate() {
        for p in proposals {
            let outcome = outcomes.get(&p.id).cloned().unwrap_or(Outcome::Clean);
            rows.push((p, outcome, pass as i64));
        }
    }
    let splices: Vec<Value> = rows
        .iter()
        .map(|(p, outcome, pass)| {
            let watched = p.watched.as_ref().map(|files| {
                let mut names: Vec<&str> = files.iter().map(String::as_str).collect();
                names.sort();
                names
            });
            json!({
                "id": p.id,
                "owner": p.owner,
                "rel": &*p.rel,
                "edits": p.edits.iter()
                    .map(|e| json!([e.line, e.col_start, e.col_end, e.text]))
                    .collect::<Vec<_>>(),
                "span": [p.span.0, p.span.1],
                "watched": watched,
                "imports": p.imports,
                "param": p.param,
                "pass": pass,
                "veto": *outcome == Outcome::Veto,
                "receipt": match outcome {
                    Outcome::Receipt(diag) => diag.as_str(),
                    _ => "clean",
                },
            })
        })
        .collect();
    let calls: Vec<Value> = oracle
        .world_calls()
        .iter()
        .map(|call| {
            let worlds: Vec<Value> = call
                .worlds
                .iter()
                .map(|(id, files)| {
                    let mut names: Vec<&str> = files.iter().map(|f| &**f).collect();
                    names.sort();
                    json!({"id": id, "files": names})
                })
                .collect();
            let added: serde_json::Map<String, Value> = call
                .added
                .iter()
                .map(|(id, diags)| {
                    let mut rows: Vec<(&str, u32, &str, &str)> = diags
                        .iter()
                        .map(|d| (&*d.rel, d.line, d.rule.as_str(), d.severity.as_str()))
                        .collect();
                    rows.sort();
                    let rows: Vec<Value> = rows
                        .into_iter()
                        .map(|(rel, line, rule, severity)| json!([rel, line, rule, severity]))
                        .collect();
                    (id.clone(), json!(rows))
                })
                .collect();
            json!({"worlds": worlds, "added": added})
        })
        .collect();
    Some(json!({"splices": splices, "calls": calls}))
}
