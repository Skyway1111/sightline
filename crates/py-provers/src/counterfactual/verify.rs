//! What a world's answer means: the redundancy a splice's own body earned,
//! the error that vetoes it, and the grouped pass that asks the oracle as
//! few times as the verdicts allow.

use super::overlay::{files_of, union_cuts, world_content};
use super::spelling::IDENT_RE;
use super::*;

/// The redundancy diagnostics a world raised inside this callee body.
fn body_diags<'d>(p: &Proposal, added: &'d [OracleDiag]) -> Vec<&'d OracleDiag> {
    added
        .iter()
        .filter(|d| {
            UNNECESSARY_RULES.contains(&d.rule.as_str())
                && d.rel == p.rel
                && p.span.0 <= d.line
                && d.line <= p.span.1
        })
        .collect()
}

/// The body's first redundancy, where this splice is the only one in it.
fn receipt_of(p: &Proposal, added: &[OracleDiag]) -> Outcome {
    match body_diags(p, added).first() {
        Some(d) => Outcome::Receipt(d.message.clone()),
        None => Outcome::Clean,
    }
}

/// The verdict a world holding only this splice gives, the meaning every
/// grouped verdict reproduces.
fn judge(p: &Proposal, added: &[OracleDiag]) -> Outcome {
    if errored(p, added) {
        Outcome::Veto
    } else {
        receipt_of(p, added)
    }
}

/// Each splice's receipt in a callee body several splices share: the
/// redundancy belongs to the splice whose parameter its operand names. The
/// fork's message holds only the types, so the operand is read where the
/// diagnostic points, its column opening the tested expression. `None`
/// where one diagnostic names no single spliced parameter: nothing short of
/// the body's own isolated worlds settles that one.
fn by_operand(
    body: &[&Proposal],
    added: &[OracleDiag],
    params: &HashMap<&str, &str>,
    lines: &[&str],
) -> Option<IndexMap<String, Outcome>> {
    let mut owners: IndexMap<&str, &str> = IndexMap::new();
    for q in body {
        if let Some(param) = params.get(q.id.as_str()) {
            owners.insert(param, &q.id);
        }
    }
    if owners.len() < body.len() {
        return None;
    }
    let mut receipts: IndexMap<String, Outcome> = body
        .iter()
        .map(|q| (q.id.clone(), Outcome::Clean))
        .collect();
    for d in body_diags(body[0], added) {
        let line = if 0 < d.line && d.line as usize <= lines.len() {
            lines[d.line as usize - 1]
        } else {
            ""
        };
        // the checker's column is a code point offset (`LineIndex::line_column`
        // at `PositionEncoding::Utf32`), so the slice counts code points
        let tail = char_slice(line, d.col as usize, usize::MAX);
        let named: IndexSet<&str> = IDENT_RE
            .find_iter(tail)
            .map(|m| m.as_str())
            .filter(|n| owners.contains_key(n))
            .collect();
        if named.len() != 1 {
            return None;
        }
        let owner = owners[named[0]];
        if receipts[owner] == Outcome::Clean {
            receipts.insert(owner.to_string(), Outcome::Receipt(d.message.clone()));
        }
    }
    Some(receipts)
}

/// Group-tested worlds, every verdict the one an isolated world gives.
/// Every splice lands in one merged world first; the set that world
/// implicates, an added error in a watched file, is resolved by the split
/// (`core::worlds::vetoed`), so a neighbour's breakage never vetoes by
/// contamination. A receipt in a body several splices edit is attributed by
/// the operand the diagnostic points at; only a body no operand resolves
/// earns its splices their own worlds.
pub fn verify(
    facts: &RepoFacts<'_>,
    proposals: &[Proposal],
    oracle: &Oracle,
) -> IndexMap<String, Outcome> {
    let live: Vec<&Proposal> = proposals
        .iter()
        .filter(|p| facts.module_by_rel(&p.rel).is_some() && oracle.root().join(&*p.rel).exists())
        .collect();
    if live.is_empty() {
        return IndexMap::new();
    }
    let world = |group: &[&Proposal]| -> World {
        let mut out = World::new();
        for p in group {
            if out.contains_key(&*p.rel) {
                continue;
            }
            let Some(module) = facts.module_by_rel(&p.rel) else {
                continue;
            };
            let mates: Vec<&Proposal> = group.iter().copied().filter(|q| q.rel == p.rel).collect();
            let body = &module.parsed.syntax().body;
            out.insert(
                p.rel.to_string(),
                world_content(module.source, body, &mates),
            );
        }
        out
    };

    let merged = vec![("merged".to_string(), world(&live))];
    // an absent world: the checker crashed under the pass (`Provers::verify_splice` discards it)
    let added = oracle
        .verify_worlds(&merged, files_of(&live).as_ref())
        .swap_remove("merged")
        .unwrap_or_default();

    // `vetoed` builds every group's world before it calls the checker, so
    // the builder records each group's file cut and the checker drains it
    let cuts: RefCell<Vec<Option<IndexSet<Rel>>>> = RefCell::new(Vec::new());
    let suspects: Vec<&Proposal> = live
        .iter()
        .copied()
        .filter(|p| errored(*p, &added))
        .collect();
    let banned = vetoed(
        &suspects,
        &added,
        |group| {
            cuts.borrow_mut().push(files_of(group));
            world(group)
        },
        |batch| {
            let files = union_cuts(std::mem::take(&mut *cuts.borrow_mut()));
            oracle.verify_worlds(batch, files.as_ref())
        },
    );

    let mut bodies: IndexMap<(Rel, (u32, u32)), Vec<&Proposal>> = IndexMap::new();
    for p in &live {
        bodies.entry((p.rel.clone(), p.span)).or_default().push(p);
    }
    let params: HashMap<&str, &str> = live
        .iter()
        .filter(|p| !p.param.is_empty())
        .map(|p| (p.id.as_str(), p.param.as_str()))
        .collect();
    let mut out: IndexMap<String, Outcome> = live
        .iter()
        .filter(|p| banned.contains(&p.id))
        .map(|p| (p.id.clone(), Outcome::Veto))
        .collect();
    let mut alone: Vec<&Proposal> = Vec::new();
    for ((rel, _), body) in &bodies {
        let receipts = if body.len() > 1 {
            let content = merged[0].1.get(&**rel).map_or("", String::as_str);
            by_operand(body, &added, &params, &source_lines(content))
        } else {
            Some(IndexMap::from([(
                body[0].id.clone(),
                receipt_of(body[0], &added),
            )]))
        };
        let rest = body.iter().copied().filter(|p| !banned.contains(&p.id));
        match receipts {
            None => alone.extend(rest),
            Some(receipts) => out.extend(rest.map(|p| (p.id.clone(), receipts[&p.id].clone()))),
        }
    }

    // the checker runs even on an empty list, and the oracle logs the call
    let batch: Vec<(String, World)> = alone.iter().map(|p| (p.id.clone(), world(&[p]))).collect();
    let isolated = oracle.verify_worlds(&batch, files_of(&alone).as_ref());
    for p in &alone {
        let diags = isolated.get(&p.id).map_or(&[][..], Vec::as_slice);
        out.insert(p.id.clone(), judge(p, diags));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::counterfactual::fixtures::proposal;

    fn diag(rel: &str, line: u32, col: u32, rule: &str, message: &str) -> OracleDiag {
        OracleDiag {
            rel: Rel::from(rel),
            line,
            col,
            rule: rule.to_string(),
            message: message.to_string(),
            severity: "warning".to_string(),
        }
    }

    #[test]
    fn an_operand_names_the_splice_its_receipt_belongs_to() {
        let a = Proposal {
            param: "a".to_string(),
            ..proposal("f:a", "two.py", (1, 6), Vec::new())
        };
        let b = Proposal {
            param: "b".to_string(),
            ..proposal("f:b", "two.py", (1, 6), Vec::new())
        };
        let body = [&a, &b];
        let params = HashMap::from([("f:a", "a"), ("f:b", "b")]);
        let lines = ["def f(a, b):", "    if a is None:", "    if b is None:"];
        let added = [
            diag(
                "two.py",
                2,
                7,
                "reportUnnecessaryComparison",
                "always \"int\"",
            ),
            diag(
                "two.py",
                3,
                7,
                "reportUnnecessaryComparison",
                "always \"str\"",
            ),
        ];

        let out = by_operand(&body, &added, &params, &lines).expect("both operands resolve");
        assert_eq!(out["f:a"], Outcome::Receipt("always \"int\"".to_string()));
        assert_eq!(out["f:b"], Outcome::Receipt("always \"str\"".to_string()));

        // a line naming both spliced parameters resolves neither
        let both = ["def f(a, b):", "    if a is None: return b"];
        let one = [diag(
            "two.py",
            2,
            7,
            "reportUnnecessaryComparison",
            "always",
        )];
        assert!(by_operand(&body, &one, &params, &both).is_none());
        // a splice with no parameter leaves the body unattributable
        let bare = HashMap::from([("f:a", "a")]);
        assert!(by_operand(&body, &added, &bare, &lines).is_none());
    }

    #[test]
    fn a_body_diagnostic_is_bound_by_rule_file_and_span() {
        let p = proposal("p", "m.py", (2, 4), Vec::new());
        let added = [
            diag("m.py", 3, 0, "reportUnnecessaryIsInstance", "in"),
            diag("m.py", 9, 0, "reportUnnecessaryIsInstance", "past the span"),
            diag(
                "other.py",
                3,
                0,
                "reportUnnecessaryIsInstance",
                "another file",
            ),
            diag("m.py", 3, 0, "reportPossiblyUnbound", "another rule"),
        ];
        let found: Vec<&str> = body_diags(&p, &added)
            .iter()
            .map(|d| d.message.as_str())
            .collect();
        assert_eq!(found, ["in"]);
        assert_eq!(receipt_of(&p, &added), Outcome::Receipt("in".to_string()));
        assert_eq!(receipt_of(&p, &added[1..]), Outcome::Clean);
    }
}
