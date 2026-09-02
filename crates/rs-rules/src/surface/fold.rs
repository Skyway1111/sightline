//! #48, the fold candidate.

use sightline_core::findings::{Evidence, Finding, Qname, Sink};
use sightline_rs_facts::Node;
use sightline_rs_facts::model::{RsFacts, RsSymbol, text};
use sightline_rs_facts::nodes::{
    ALL, COMMENTS, LITERALS, ancestors, children, descend, has, is_fn, item_doc, named_children,
    statements,
};
use sightline_rs_provers::RsProvers;
use sightline_rs_provers::oracle::index::RsEdge;

use crate::util::site;

/// the one-line bound, less an `a = 1; b = 2` run on one line
const FOLD_MAX_STMTS: usize = 4;
/// literals in one display: the name is what the table means
const VOCABULARY: usize = 3;
const TABLES: &str = "array_expression tuple_expression";
/// a name outside the crate can reach whatever its visibility says
const EXPORTED: [&str; 2] = ["no_mangle", "export_name"];
const SELF_TYPE: &str = "Self";
/// a `match` arm's `if`: the call sits in the guard
const GUARD: &str = "match_pattern";
const BOOLEAN: &str = "&& || !";

/// Is the body written on one line? What a reader pays for the hop is the
/// line, and a statement count is not it.
fn one_line(body: Node<'_>) -> bool {
    let stmts = statements(body);
    match (stmts.first(), stmts.last()) {
        (Some(first), Some(last)) if stmts.len() <= FOLD_MAX_STMTS => {
            first.start_position().row == last.end_position().row
        }
        _ => false,
    }
}

/// The body spells a literal table. The name is the table's meaning, and the
/// call site would read its members instead.
fn names_a_table(body: Node<'_>) -> bool {
    descend(body, ALL)
        .into_iter()
        .filter(|n| has(TABLES, n.kind()))
        .any(|n| {
            named_children(n)
                .iter()
                .filter(|c| has(LITERALS, c.kind()))
                .count()
                >= VOCABULARY
        })
}

/// Prose written about this name: a `///` run above the item, or a comment
/// inside the body. The fold deletes it. The call site is a statement in
/// someone else's story and holds no room for what the helper had to say
/// about itself (a `# Safety` obligation, the regex a predicate replaced,
/// the pair it belongs to).
fn carries_prose(sym: &RsSymbol<'_>, body: Node<'_>) -> bool {
    item_doc(sym.node) || descend(body, ALL).iter().any(|n| has(COMMENTS, n.kind()))
}

/// The body spells its receiver's state or its own type: a field of `self`,
/// or `Self`. Such a fn is the type's own machinery, the sanctioned reader
/// of a private field, a lock or an atomic, or a place its value is built,
/// and folding it hands that reach to a caller with no business holding it.
/// A method call on `self` reaches nothing of the kind: it is a hop like any
/// other.
fn reaches_own_type(body: Node<'_>, src: &[u8]) -> bool {
    descend(body, ALL)
        .into_iter()
        .any(|n| text(n, src) == SELF_TYPE || (n.kind() == "self" && !method_receiver(n)))
}

/// Is this `self` the receiver of a method call, `self.f(..)`, which reads
/// no state of its own, rather than the base of a field it reads?
fn method_receiver(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let call = parent
        .parent()
        .and_then(|grand| grand.child_by_field_name("function"));
    parent.kind() == "field_expression" && call.is_some_and(|c| c.id() == parent.id())
}

/// The one call site is an operand of a boolean operator or a `match` arm's
/// guard. The body inlined there needs parens, and the reader gets a
/// compound condition where a name used to say the whole clause in a word.
fn compound_call(facts: &RsFacts<'_>, qname: &str, edge: &RsEdge) -> bool {
    facts
        .refs_of(qname)
        .filter(|r| r.lineno == edge.line && facts.rel_of(&r.module) == edge.rel)
        .any(|r| {
            ancestors(r.node)
                .into_iter()
                .any(|n| n.kind() == GUARD || children(n).iter().any(|c| has(BOOLEAN, c.kind())))
        })
}

/// No `pub` of any kind, and no other way out of the crate: a trait impl's
/// method is reached through the trait, and an exported symbol through the
/// linker.
fn private(sym: &RsSymbol<'_>) -> bool {
    !children(sym.node)
        .iter()
        .any(|c| c.kind() == "visibility_modifier")
        && sym.traits.is_empty()
        && !sym.attrs.iter().any(|a| EXPORTED.contains(&a.as_str()))
}

/// A private `fn` whose whole body is one line, reached by exactly one
/// resolved edge and that edge a call from prod code: the fold is a
/// substitution, so the name adds a hop and a signature for a single reader
/// and nothing else. A private item's callers are all in the crate, so the
/// edge count is the whole story, except where a macro body or an
/// attribute's string names it, both of which expand past every index.
///
/// A substitution the caller comes out worse from is no fold, and the four
/// shapes the judged round named are each a way to be worse off: prose the
/// call site cannot hold, a reach into the receiver's own state, a body that
/// is where the type is built, and a call site whose boolean condition the
/// body would join.
pub(super) fn rule_48(facts: &RsFacts<'_>, provers: &RsProvers<'_>, out: &mut Sink) {
    let graph = &provers.rust.graph;
    let unindexed = provers.unindexed_names();
    let mut order: Vec<&Qname> = facts.symbols.keys().collect();
    order.sort();
    for qname in order {
        let sym = &facts.symbols[qname];
        let body = if is_fn(sym) {
            sym.node.child_by_field_name("body")
        } else {
            None
        };
        let Some(body) = body else { continue };
        let src = facts.modules[&sym.module].bytes;
        if sym.is_test
            || !private(sym)
            || unindexed.contains(&sym.name)
            || !one_line(body)
            || names_a_table(body)
            || carries_prose(sym, body)
            || reaches_own_type(body, src)
        {
            continue;
        }
        let edges = graph.edges_to(qname);
        let [edge] = edges[..] else { continue };
        let caller = facts.symbols.get(edge.caller.as_str());
        if !edge.call
            || edge.caller.as_str() == &**qname
            || facts.is_test(&edge.rel)
            || caller.is_some_and(|c| c.is_test)
            || compound_call(facts, qname, edge)
        {
            continue;
        }
        out.push(Finding {
            rule: "48",
            site: site(facts, sym, sym.node),
            message: format!(
                "{qname} (one line) is called once, from {}: fold it into the caller",
                edge.caller
            ),
            cause: format!("fold:{qname}"),
            evidence: Evidence::Wp {
                premises: vec![
                    "prod-callers:1".to_string(),
                    format!("caller:{}", edge.caller),
                ],
            },
            salience: 0.0,
            fix: None,
            lang: "rs",
        });
    }
}
