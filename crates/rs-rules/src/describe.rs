//! Port of `rs/describe.py`: the `facts` verb's Rust printout, one symbol's
//! or module's record as the stack holds it, from existing facts and prover
//! accessors and no analysis of its own.

use sightline_core::findings::Finding;
use sightline_core::text::nearest;
use sightline_rs_facts::model::{RsFacts, RsSymbol};
use sightline_rs_provers::RsProvers;

/// `sightline facts <qname>`: one Rust symbol's record, as this stack holds
/// it. A module qname answers for the module; an unknown one answers `Err`
/// with the nearest matches.
pub fn describe(
    facts: &RsFacts<'_>,
    provers: &RsProvers<'_>,
    findings: &[Finding],
    qname: &str,
) -> Result<String, Vec<String>> {
    let (mut head, mine) = match (facts.symbols.get(qname), facts.modules.get(qname)) {
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
        (None, Some(module)) => {
            let doc = module.doc.join("; ");
            let head = vec![
                format!("{qname}  module  {} L1-{}", module.rel, module.lines.len()),
                format!("crate:        {}", module.crate_name),
                format!(
                    "doc:          {}",
                    if doc.is_empty() {
                        "none (no //! header)"
                    } else {
                        &doc
                    }
                ),
                format!("items:        {}", facts.symbols_of(qname).count()),
                format!("allows:       {}", provers.allows()[qname].len()),
            ];
            let mine: Vec<&Finding> = findings
                .iter()
                .filter(|f| f.site.rel == module.rel)
                .collect();
            (head, mine)
        }
        (Some(sym), _) => {
            let module = &facts.modules[&sym.module];
            let mut head = vec![format!(
                "{qname}  {}  {} L{}-{}",
                sym.kind, module.rel, sym.lineno, sym.end_lineno
            )];
            head.extend(symbol_lines(facts, provers, sym));
            let mine: Vec<&Finding> = findings
                .iter()
                .filter(|f| &*f.site.symbol == qname)
                .collect();
            (head, mine)
        }
    };
    head.push(format!("findings:     {}", mine.len()));
    head.extend(mine.iter().map(|f| {
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
    Ok(head.join("\n") + "\n")
}

fn symbol_lines(facts: &RsFacts<'_>, provers: &RsProvers<'_>, sym: &RsSymbol<'_>) -> Vec<String> {
    let qname = &*sym.qname;
    let body = provers.body(qname);
    let refs: Vec<_> = facts.refs_of(qname).collect();
    let inbound = refs.iter().filter(|r| r.module != sym.module).count();
    let graph = &provers.rust.graph;
    let world = provers.closed_world().verdict(qname);
    let mut reasons: Vec<&str> = world.reasons.iter().map(String::as_str).collect();
    reasons.sort_unstable();
    vec![
        format!(
            "visibility:   {}{}{}",
            if sym.is_public { "pub" } else { "private" },
            if sym.traits.is_empty() {
                String::new()
            } else {
                format!("; impl of {}", sym.traits.join(", "))
            },
            if sym.is_test { "; a test item" } else { "" }
        ),
        format!(
            "published:    {}",
            if facts.publishes(sym) { "yes" } else { "no" }
        ),
        format!(
            "attributes:   {}",
            if sym.attrs.is_empty() {
                "none".to_string()
            } else {
                sym.attrs.join(", ")
            }
        ),
        format!(
            "refs:         {} ({inbound} from other modules)",
            refs.len()
        ),
        format!(
            "resolved:     {} caller(s), {} callee(s), {} inbound edge(s)",
            graph.calls_to(qname).len(),
            graph.calls_from(qname).len(),
            graph.edges_to(qname).len()
        ),
        format!(
            "closed world: {}",
            if world.passed {
                "pass".to_string()
            } else {
                reasons.join(", ")
            }
        ),
        format!("complexity:   {}", provers.complexity(qname)),
        format!(
            "body:         {} call(s), {} macro(s), {} closure(s), {} unsafe block(s)",
            body.calls.len(),
            body.macros.len(),
            body.closures.len(),
            body.unsafe_blocks.len()
        ),
    ]
}
