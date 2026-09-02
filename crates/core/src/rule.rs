//! The rule record. A rule's whole
//! record is the `Rule` const beside the fn it describes; posture and scope
//! are declared there and in no other place. The fn half is each language's.

use std::collections::BTreeSet;

/// Rule ids an audit runs without.
pub type RuleSet = BTreeSet<String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Posture {
    /// blocks wherever it runs; never baselined
    Gate,
    /// blocks new-vs-baseline; the baseline's only keys
    Ratchet,
    /// never blocks, never baselined; audit output only
    Report,
}

impl Posture {
    // sightline-ok: 11 - an enum's match table is its own name
    pub fn value(self) -> &'static str {
        match self {
            Posture::Gate => "gate",
            Posture::Ratchet => "ratchet",
            Posture::Report => "report",
        }
    }
}

/// `File`: single-file facts provably yield the full-build findings, so the
/// fast gate runs the rule unless it reports. Anything leaning on repo-wide
/// indexes, the oracle, or cross-module resolution stays `Repo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    Repo,
    File,
}

/// What every language-blind reader needs of a rule. The fn pointer type is
/// the language's, so it is not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRecord {
    /// rule number, as a string
    pub id: &'static str,
    pub slug: &'static str,
    /// "A" | "B" | "C" | "P" | "T" | "Z"
    pub family: &'static str,
    /// documented engine: AST | IDX | ORACLE | WP | mixed
    pub engine_class: &'static str,
    pub posture: Posture,
    pub meaning: &'static str,
    /// the unenforceable goal this rule approximates (`explain` text)
    pub goal: &'static str,
    /// the language whose facts this reading is written against
    pub lang: &'static str,
    pub scope: Scope,
    /// what another linter already covers, so this rule need not
    pub complement: &'static str,
}

/// Up to three owners plus a count: per-member messages on an n-member group
/// would otherwise carry n qnames each (#11).
pub fn owner_list<S: AsRef<str>>(qnames: &[S]) -> String {
    let shown = qnames
        .iter()
        .take(3)
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(", ");
    match qnames.len().checked_sub(3) {
        Some(extra) if extra > 0 => format!("{shown} +{extra} more"),
        _ => shown,
    }
}

/// The one rule runner both language crates call: group A (every rule but
/// the world owners) under rayon with one sink each, then the world owners
/// sequentially in the order given, findings extended in `ids` order and a
/// wall per rule for `timing`. `run_one` runs the rule at an index, or
/// answers empty at zero cost for a rule the run skips.
pub fn run_split<F>(
    ids: &[&'static str],
    world_owners: &[&str],
    run_one: F,
    sink: &mut crate::findings::Sink,
    timing: crate::lang::Timing,
) where
    F: Fn(usize) -> (Vec<crate::findings::Finding>, std::time::Duration) + Sync,
{
    use rayon::prelude::*;
    use std::time::Duration;

    let mut walls: Vec<Duration> = vec![Duration::ZERO; ids.len()];
    let mut findings: Vec<Vec<crate::findings::Finding>> = ids.iter().map(|_| Vec::new()).collect();
    let group_a: Vec<(usize, Vec<crate::findings::Finding>, Duration)> = (0..ids.len())
        .into_par_iter()
        .filter(|at| !world_owners.contains(&ids[*at]))
        .map(|at| {
            let (found, wall) = run_one(at);
            (at, found, wall)
        })
        .collect();
    for (at, found, wall) in group_a {
        findings[at] = found;
        walls[at] = wall;
    }
    for id in world_owners {
        if let Some(at) = ids.iter().position(|i| i == id) {
            let (found, wall) = run_one(at);
            findings[at] = found;
            walls[at] = wall;
        }
    }

    for found in findings {
        sink.0.extend(found);
    }
    if let Some(on_rule) = timing {
        for (id, wall) in ids.iter().zip(walls) {
            on_rule(id, wall);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_list_shows_three_then_counts() {
        assert_eq!(owner_list::<&str>(&[]), "");
        assert_eq!(owner_list(&["a", "b"]), "a, b");
        assert_eq!(owner_list(&["a", "b", "c"]), "a, b, c");
        assert_eq!(owner_list(&["a", "b", "c", "d"]), "a, b, c +1 more");
        assert_eq!(owner_list(&["a", "b", "c", "d", "e"]), "a, b, c +2 more");
    }

    #[test]
    fn posture_and_scope_spell_what_the_record_declares() {
        assert_eq!(Posture::Gate.value(), "gate");
        assert_eq!(Posture::Ratchet.value(), "ratchet");
        assert_eq!(Posture::Report.value(), "report");
    }
}
