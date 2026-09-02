//! The Rust half of a verify pass: an item's
//! deletion as a splice, and the worlds that judge a batch of them. The
//! batching is `core::worlds`, which is language-blind; this module builds
//! the overlays, hands them to the oracle through `RsAnswers::verify_worlds`
//! and keeps the receipt.
//!
//! Every Rust splice is a deletion, which the module owns: there is no
//! dependent set to enumerate, so an empty caller set is never "no callers"
//! and a new error in any file vetoes.

use std::cell::Cell;
use std::collections::HashSet;

use indexmap::{IndexMap, IndexSet};

use sightline_core::edits::{apply_edits, blank};
use sightline_core::findings::{Evidence, Fix, SpanEdit};
use sightline_core::worlds::{Diag, Spliced, World, errored, vetoed};
use sightline_rs_facts::model::{RsFacts, RsSymbol};

use crate::oracle::RsAnswers;

/// A comment or attribute run touching the item's first line belongs to it.
const ABOVE: [&str; 3] = ["attribute_item", "line_comment", "block_comment"];

fn clean() -> Evidence {
    Evidence::Wp {
        premises: vec!["counterfactual:clean".to_string()],
    }
}

/// One proposed edit, in the terms `core::worlds::Spliced` reads plus the
/// edits themselves. `span` is `(0, 0)` and `watched` None: a module-owned
/// splice is judged by every file's diagnostics, not a caller list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsSplice {
    pub id: String,
    pub rel: String,
    pub edits: Vec<SpanEdit>,
}

impl RsSplice {
    /// The emitter's payload: exactly the splice a world verified.
    pub fn fix(&self) -> Fix {
        Fix {
            rel: self.rel.as_str().into(),
            edits: self.edits.clone(),
            imports: Vec::new(),
        }
    }
}

impl Spliced for RsSplice {
    fn id(&self) -> &str {
        &self.id
    }

    fn rel(&self) -> &str {
        &self.rel
    }

    fn span(&self) -> (u32, u32) {
        (0, 0)
    }

    fn watched(&self) -> Option<&HashSet<String>> {
        None
    }
}

/// A cargo diagnostic in the three terms the split reads.
struct Added {
    rel: String,
    line: u32,
    severity: String,
}

impl Diag for Added {
    fn rel(&self) -> &str {
        &self.rel
    }

    fn line(&self) -> u32 {
        self.line
    }

    fn severity(&self) -> &str {
        &self.severity
    }
}

/// The item's lines emptied, the attribute and comment run above it
/// included. Emptied, never removed: a world's diagnostic diff is
/// line-keyed, so a splice may not move a line.
pub fn deletion(facts: &RsFacts<'_>, sym: &RsSymbol<'_>, sid: &str) -> Option<RsSplice> {
    let module = &facts.modules[&sym.module];
    let mut first = sym.lineno;
    let mut prev = sym.node.prev_named_sibling();
    // abuts: no blank line between
    while let Some(node) = prev.filter(|n| ABOVE.contains(&n.kind()) && last_line(*n) + 1 == first)
    {
        first = node.start_position().row as u32 + 1;
        prev = node.prev_named_sibling();
    }
    if !(1 <= first && first <= sym.end_lineno && sym.end_lineno as usize <= module.lines.len()) {
        return None;
    }
    Some(RsSplice {
        id: sid.to_string(),
        rel: module.rel.to_string(),
        edits: blank(&module.lines, first, sym.end_lineno),
    })
}

/// The 1-based line the node's text ends on. A `///` comment node ends at
/// column 0 of the next row (the newline is its own), so the row alone is
/// already 1-based there.
fn last_line(node: sightline_rs_facts::Node<'_>) -> u32 {
    let end = node.end_position();
    if end.column > 0 {
        end.row as u32 + 1
    } else {
        end.row as u32
    }
}

/// Each splice no world raised a new error under, mapped to what its world
/// proved and the exact verified edits; `answers` is `RsProvers.rust`. A
/// vetoed splice is absent, and so is every splice where no oracle answered
/// or a world went missing under the pass: silence verifies nothing.
pub fn verify_splice(
    facts: &RsFacts<'_>,
    answers: &RsAnswers,
    splices: Vec<RsSplice>,
) -> IndexMap<String, (Evidence, Fix)> {
    let live: Vec<RsSplice> = splices
        .into_iter()
        .filter(|s| facts.module_by_rel.contains_key(s.rel.as_str()))
        .collect();
    if live.is_empty() {
        return IndexMap::new();
    }
    let missing = Cell::new(false);
    let check = |worlds: &[(String, World)]| -> IndexMap<String, Vec<Added>> {
        let answered = answers.verify_worlds(worlds);
        if worlds.iter().any(|(wid, _)| !answered.contains_key(wid)) {
            missing.set(true);
        }
        answered
            .into_iter()
            .map(|(wid, diags)| {
                let rows = diags
                    .iter()
                    .map(|d| Added {
                        rel: d.rel.clone(),
                        line: d.line,
                        severity: d.level.clone(),
                    })
                    .collect();
                (wid, rows)
            })
            .collect()
    };

    let refs: Vec<&RsSplice> = live.iter().collect();
    let mut merged = check(&[("merged".to_string(), world(facts, &refs))]);
    let added = merged.shift_remove("merged").unwrap_or_default();
    let suspects: Vec<&RsSplice> = refs
        .iter()
        .copied()
        .filter(|s| errored(*s, &added))
        .collect();
    let banned = vetoed(&suspects, &added, |group| world(facts, group), &check);
    if missing.get() {
        return IndexMap::new();
    }
    live.iter()
        .filter(|s| !banned.contains(&s.id))
        .map(|s| (s.id.clone(), (clean(), s.fix())))
        .collect()
}

/// The overlay this group makes: whole replacement content per file, its line
/// count untouched. The oracle owns the tree copy; this is only text.
fn world(facts: &RsFacts<'_>, group: &[&RsSplice]) -> World {
    let mut out = World::new();
    let rels: IndexSet<&str> = group.iter().map(|s| s.rel.as_str()).collect();
    for rel in rels {
        let module = &facts.modules[&facts.module_by_rel[rel]];
        let mut lines: Vec<String> = module.lines.iter().map(|l| (*l).to_string()).collect();
        let edits: Vec<SpanEdit> = group
            .iter()
            .filter(|s| s.rel == rel)
            .flat_map(|s| s.edits.iter().cloned())
            .collect();
        apply_edits(&mut lines, &edits);
        out.insert(rel.to_string(), lines.join("\n") + "\n");
    }
    out
}
