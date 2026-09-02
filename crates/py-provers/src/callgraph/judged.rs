//! The judging half of the call graph: how an oracle callee edge re-judges
//! a CHA verdict, and what a by-name guess becomes with and without one. The
//! oracle feeds the edges.

use super::*;

/// No oracle: the by-name guess is the only verdict there is.
pub(super) fn name_cha_stands(call: &CallSite) -> CallSite {
    let mut out = call.clone();
    if out.resolution == Resolution::ByName {
        out.resolution = Resolution::Resolved;
    }
    out
}

/// A by-name guess no oracle edge confirmed: the name matched, the body is not
/// the graph's to name.
pub(super) fn unconfirmed(call: &CallSite) -> CallSite {
    let mut out = call.clone();
    if out.resolution == Resolution::ByName {
        out.resolution = Resolution::Unresolved;
        out.target = None;
        out.candidates = Vec::new();
    }
    out
}

/// `CallSiteId` -> the site as an oracle edge re-resolves it.
pub(super) fn oracle_judged(
    facts: &RepoFacts<'_>,
    edges: &[CallEdge],
) -> IndexMap<CallSiteId, CallSite> {
    let def_at = definitions_by_line(facts);
    let mut site_at: HashMap<(Rel, u32, u32, u32, u32), CallSiteId> = HashMap::new();
    for (i, c) in facts.call_sites.iter().enumerate() {
        let Some(module) = facts.modules.get(&c.module) else {
            continue;
        };
        let Some(span) = module.span(c.node) else {
            continue;
        };
        let cell = |at: usize| span[at].unwrap_or(0);
        site_at.insert(
            (module.rel.clone(), cell(0), cell(1), cell(2), cell(3)),
            i as CallSiteId,
        );
    }
    let mut out: IndexMap<CallSiteId, CallSite> = IndexMap::new();
    for edge in edges {
        let key = (
            edge.rel.clone(),
            edge.line,
            edge.col,
            edge.end_line,
            edge.end_col,
        );
        let Some(at) = site_at.get(&key).copied() else {
            continue;
        };
        let call = &facts.call_sites[at as usize];
        if !UNTYPED.contains(&call.resolution) {
            continue;
        }
        if !edge.external.is_empty() {
            let mut new = call.clone();
            new.resolution = Resolution::External;
            new.target = None;
            new.candidates = edge.external.clone();
            out.insert(at, new);
            continue;
        }
        // a definition outside facts' symbols: no confirmation
        let targets: Option<Vec<Qname>> = edge
            .targets
            .iter()
            .map(|t| def_at.get(t).cloned().flatten())
            .collect();
        let Some(targets) = targets.filter(|t| !t.is_empty()) else {
            continue;
        };
        let new = judged(facts, call, &targets);
        if (new.resolution, &new.target, &new.candidates)
            != (call.resolution, &call.target, &call.candidates)
        {
            out.insert(at, new);
        }
    }
    out
}

/// `(rel, def line)` -> callable symbol; `None` on a same-line collision.
pub(super) fn definitions_by_line(facts: &RepoFacts<'_>) -> HashMap<(Rel, u32), Option<Qname>> {
    let mut out: HashMap<(Rel, u32), Option<Qname>> = HashMap::new();
    for (qname, sym) in &facts.symbols {
        if !FUNCTION_KINDS.contains(&sym.kind) && sym.kind != "class" {
            continue;
        }
        let Some(rel) = facts.rel_of(&sym.module) else {
            continue;
        };
        let key = (rel.clone(), sym.lineno);
        match out.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                e.insert(None);
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(Some(qname.clone()));
            }
        }
    }
    out
}

/// Re-resolved to the oracle's definitions plus their subclass overrides.
pub(super) fn judged(facts: &RepoFacts<'_>, call: &CallSite, targets: &[Qname]) -> CallSite {
    let mut candidates: BTreeSet<Qname> = BTreeSet::new();
    for qname in targets {
        candidates.insert(qname.clone());
        let Some(sym) = facts.symbols.get(qname) else {
            continue;
        };
        let Some(parent) = sym
            .parent
            .as_ref()
            .filter(|p| facts.classes.contains_key(*p))
        else {
            continue;
        };
        for (q, info) in class_walk(facts, parent, Step::Subclasses) {
            if q != *parent
                && let Some(over) = info.methods.get(&sym.name)
            {
                candidates.insert(over.clone());
            }
        }
    }
    let ordered: Vec<Qname> = candidates.into_iter().collect();
    let mut out = call.clone();
    if ordered.len() == 1 {
        out.resolution = Resolution::Resolved;
        out.target = Some(ordered[0].clone());
        out.candidates = Vec::new();
    } else {
        out.resolution = Resolution::Ambiguous;
        out.target = None;
        out.candidates = ordered;
    }
    out
}

// --- callers.py --------------------------------------------------------------
