//! `sightline facts <root> <qname>`: what the
//! provers already hold about one symbol, printed in the order an agent
//! asks. What it is, who calls it, what it does, whether its world is
//! closed, whether it is hot, whether it is live, what fires on it, and what
//! `fix` would patch. Every line is an existing accessor's answer; the one
//! pass of its own is the world pass that verifies the symbol's fixes
//! (`emit::attach_fixes`).

use std::collections::BTreeSet;

use sightline_core::findings::Finding;
use sightline_core::text::nearest;
use sightline_py_facts::model::{RepoFacts, Symbol};
use sightline_py_provers::Provers;
use sightline_py_provers::callgraph::callers_of;

use crate::emit;

/// Names per line: the printout is read whole, not scrolled.
const CAP: usize = 8;

fn names<I: IntoIterator<Item = String>>(items: I) -> String {
    let ordered: Vec<String> = items
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let head = ordered
        .iter()
        .take(CAP)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    match ordered.len().checked_sub(CAP) {
        Some(more) if more > 0 => format!("{head}, +{more} more"),
        _ => head,
    }
}

fn symbol_lines(facts: &RepoFacts<'_>, provers: &Provers, sym: &Symbol) -> Vec<String> {
    let qname = &*sym.qname;
    let cs = callers_of(qname, facts, provers.calls(facts));
    let effects = provers.effects(facts).get(qname);
    let cw = provers.closed_world(facts).verdict(qname);
    let hot = provers.hot(facts);
    let live = provers.live(facts);
    let unseen = provers.unseen(facts);
    let reached: Vec<&str> = [
        ("strings", &unseen.strings),
        ("kwargs", &unseen.kwargs),
        ("attrs", &unseen.attrs),
        ("tables", &unseen.tables),
        ("test_attrs", &unseen.test_attrs),
    ]
    .into_iter()
    .filter(|(_, set)| set.contains(&sym.name))
    .map(|(kind, _)| kind)
    .collect();
    let scopes = live.live.get(&sym.name);

    let from = |sites: &[&sightline_py_facts::model::CallSite]| {
        if sites.is_empty() {
            String::new()
        } else {
            format!(
                ", from {}",
                names(sites.iter().map(|c| c.enclosing.to_string()))
            )
        }
    };
    vec![
        format!("callers prod: {} sites{}", cs.prod.len(), from(&cs.prod)),
        format!("callers test: {} sites{}", cs.test.len(), from(&cs.test)),
        format!(
            "effects:      {}",
            match effects {
                None => "not summarised (not a function)".to_string(),
                Some(e) if e.clean() => "clean".to_string(),
                Some(e) => {
                    names(e.atoms.iter().cloned())
                        + if e.unknown {
                            " (+an edge escaped resolution)"
                        } else {
                            ""
                        }
                }
            }
        ),
        format!(
            "closed world: {}",
            if cw.passed {
                "closed".to_string()
            } else {
                format!("escaped - {}", names(cw.reasons.iter().cloned()))
            }
        ),
        format!(
            "hot:          amplification {}{}{}",
            hot.amplification.get(qname).copied().unwrap_or(0),
            if hot.roots.iter().any(|r| &**r == qname) {
                "; a hot root"
            } else {
                ""
            },
            if hot.roots.is_empty() {
                "; no hot roots (family P silent)"
            } else {
                ""
            }
        ),
        format!(
            "liveness:     {}{}",
            match scopes {
                Some(s) if !s.is_empty() =>
                    format!("name live in {}", names(s.iter().map(|q| q.to_string()))),
                _ => "dead by name".to_string(),
            },
            if reached.is_empty() {
                String::new()
            } else {
                format!("; unseen reaches it as {}", reached.join(", "))
            }
        ),
    ]
}

/// The findings on the symbol in rank order, then the fixes among them: one
/// world pass over these findings alone, never the repo's.
fn finding_lines(facts: &RepoFacts<'_>, provers: &Provers, mine: &[Finding]) -> Vec<String> {
    let mut lines = vec![format!("findings:     {}", mine.len())];
    lines.extend(mine.iter().map(|f| {
        format!(
            "  #{:<3} {:<9} {}:{}:{}  {}",
            f.rule,
            f.tier().value(),
            f.site.rel,
            f.site.line,
            f.site.col,
            f.message
        )
    }));
    let patched = emit::attach_fixes(mine.to_vec(), facts, provers);
    let fixed: Vec<&Finding> = patched.iter().filter(|f| f.fix.is_some()).collect();
    lines.push(format!("fixes:        {} verified", fixed.len()));
    lines.extend(fixed.iter().map(|f| {
        let fix = f.fix.as_ref().expect("a fixed finding holds its fix");
        format!(
            "  #{:<3} {}: {} edit(s) in {}{}",
            f.rule,
            f.cause,
            fix.edits.len(),
            fix.rel,
            if fix.imports.is_empty() {
                String::new()
            } else {
                format!(", imports {}", fix.imports.join(", "))
            }
        )
    }));
    lines
}

/// One symbol's whole prover record. A module qname answers for the module:
/// its header and the findings its symbols carry. An unknown qname answers
/// `Err` with the nearest matches.
pub fn describe(
    facts: &RepoFacts<'_>,
    provers: &Provers,
    findings: &[Finding],
    qname: &str,
) -> Result<String, Vec<String>> {
    let sym = facts.symbols.get(qname);
    let module = facts.modules.get(qname);
    let (head, mine) = match (sym, module) {
        (None, None) => {
            return Err(nearest(
                qname,
                facts
                    .symbols
                    .keys()
                    .chain(facts.modules.keys())
                    .map(|q| &**q),
            ));
        }
        (Some(sym), _) => {
            let module = facts
                .modules
                .get(&*sym.module)
                .expect("a symbol's module is indexed");
            let end = if sym.end_lineno == 0 {
                sym.lineno
            } else {
                sym.end_lineno
            };
            let mut head = vec![format!(
                "{qname}  {}  {} L{}-{end}",
                sym.kind, module.rel, sym.lineno
            )];
            head.extend(symbol_lines(facts, provers, sym));
            let mine: Vec<Finding> = findings
                .iter()
                .filter(|f| &*f.site.symbol == qname)
                .cloned()
                .collect();
            (head, mine)
        }
        (None, Some(module)) => {
            let head = vec![format!(
                "{qname}  module  {} L1-{}",
                module.rel,
                module.lines.len()
            )];
            let mine: Vec<Finding> = findings
                .iter()
                .filter(|f| f.site.rel == module.rel)
                .cloned()
                .collect();
            (head, mine)
        }
    };
    let mut lines = head;
    lines.extend(finding_lines(facts, provers, &mine));
    Ok(lines.join("\n") + "\n")
}
