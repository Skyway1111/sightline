//! Family C, record contracts (port of `rules/records.py`, #57): a key a
//! closed producer writes on every return path that none of its closed sinks
//! reads, work computed and discarded, or one fact kept in two homes.

use std::collections::BTreeSet;

use indexmap::IndexMap;

use sightline_core::findings::{Evidence, Finding, Qname, Sink};
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::model::{RepoFacts, is_test_path};
use sightline_py_provers::Provers;
use sightline_py_provers::records::Edge;

use crate::model::Rule;
use crate::util::node_site;

fn keyset<'a>(keys: impl IntoIterator<Item = &'a String>) -> String {
    let sorted: BTreeSet<&String> = keys.into_iter().collect();
    let names: Vec<&str> = sorted.into_iter().map(|k| k.as_str()).collect();
    format!("{{{}}}", names.join(", "))
}

pub const RULE_57: Rule = Rule {
    record: RuleRecord {
        id: "57",
        slug: "dead-key",
        family: "C",
        engine_class: "WP",
        posture: Posture::Ratchet,
        meaning: "a dict-record key a closed producer writes on every return path \
                  that no closed sink reads (prod producers; any open sink or open \
                  world is silent)",
        goal: "A dict literal passed around is a contract no one wrote down; a key \
               every reader ignores is work computed and thrown away, or one fact \
               kept in two homes. Drop it, or name the record (TypedDict, \
               dataclass) so the reads are checked.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_57,
};

/// Producers on prod paths whose every RESOLVED call site lands in a closed
/// sink (a param or local read only by constant key), in a closed world (no
/// by-value reference, reflection, re-export, override or splat): the keys
/// every path writes minus the keys any sink reads.
fn rule_57(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let found = provers.records(facts);
    let closed_world = provers.closed_world(facts);
    let mut sinks_of: IndexMap<&Qname, Vec<&Edge>> = IndexMap::new();
    for edge in &found.edges {
        sinks_of.entry(&edge.producer).or_default().push(edge);
    }
    for (q, shapes) in &found.produced {
        let Some(sym) = facts.symbols.get(&**q) else {
            continue;
        };
        let Some(module) = facts.modules.get(&sym.module) else {
            continue;
        };
        let sinks = sinks_of.get(q).map_or(&[][..], |v| v);
        if sinks.is_empty()
            || is_test_path(&module.rel)
            || !closed_world.verdict(q).passed
            || sinks.iter().any(|e| e.reads.is_none())
        {
            continue;
        }
        let read: BTreeSet<String> = sinks
            .iter()
            .filter_map(|e| e.reads.as_ref())
            .flatten()
            .cloned()
            .collect();
        let Some(first) = shapes.first() else {
            continue;
        };
        let dead: BTreeSet<&String> = first
            .iter()
            .filter(|key| shapes.iter().all(|s| s.contains(*key)) && !read.contains(*key))
            .collect();
        let written: BTreeSet<String> = shapes.iter().map(keyset).collect();
        let premises: BTreeSet<String> = sinks
            .iter()
            .map(|e| format!("sink {}.{}", e.sink, e.name))
            .collect();
        for key in dead {
            out.push(Finding {
                rule: "57",
                site: node_site(facts, module, sym.node),
                message: format!(
                    "{q} writes '{key}' on every path, and none of its {} sinks reads it \
                     (keys: {}; read: {})",
                    sinks.len(),
                    written.iter().cloned().collect::<Vec<_>>().join(" | "),
                    keyset(&read),
                ),
                cause: format!("dead-key:{q}:{key}"),
                evidence: Evidence::Wp {
                    premises: std::iter::once(format!("producer {q}"))
                        .chain(premises.iter().cloned())
                        .collect(),
                },
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}
