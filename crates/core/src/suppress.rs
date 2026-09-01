//! Suppression markers (port of `findings.py` `_MARKS` to `suppress`).
//!
//! One concept, one grammar; a mark is a rule id or its slug. Code spells
//! it in its language's comment syntax, a doc in HTML.

use std::collections::HashMap;
use std::sync::LazyLock;

use indexmap::IndexSet;
use regex::Regex;

use crate::findings::{Finding, Rel};
use crate::lang::FactsView;

const MARKS: &str = r"([\w-]+(?:\s*,\s*[\w-]+)*)";

/// The marker of a doc file (`.md`, `.rst`): `<!-- sightline-ok: ids -->`.
pub fn doc_suppress_re() -> &'static Regex {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!(r"<!--\s*sightline-ok:\s*{MARKS}\s*-->")).unwrap());
    &RE
}

/// The marker a language's comment syntax spells.
pub fn suppress_pattern(comment_prefix: &str) -> Regex {
    Regex::new(&format!(
        r"{}\s*sightline-ok:\s*{MARKS}",
        regex::escape(comment_prefix)
    ))
    .unwrap()
}

/// 1-based line -> rule ids suppressed there, slugs resolved. A comment-only
/// marker line applies to the next line; a trailing marker applies to its own.
pub fn marker_table<S: AsRef<str>>(
    lines: &[S],
    marker: &Regex,
    comment_prefix: &str,
    ids_by_slug: &HashMap<String, String>,
) -> HashMap<u32, IndexSet<String>> {
    let mut out: HashMap<u32, IndexSet<String>> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        let line = line.as_ref();
        let Some(m) = marker.captures(line) else {
            continue;
        };
        let i = i as u32 + 1;
        // ceiling: `pytext::strip` (unit core-b) adds \x1c-\x1f to the
        // stripped set; `trim` covers Unicode White_Space alone.
        let target = if line.trim().starts_with(comment_prefix) {
            i + 1
        } else {
            i
        };
        let entry = out.entry(target).or_default();
        for part in m[1].split(',') {
            let mark = part.trim();
            entry.insert(
                ids_by_slug
                    .get(mark)
                    .cloned()
                    .unwrap_or_else(|| mark.to_string()),
            );
        }
    }
    out
}

/// `ids_by_slug` is the registry's slug alias map, passed in by the caller:
/// rules read findings, so findings never reads rules.
pub fn suppress(
    findings: Vec<Finding>,
    facts: &dyn FactsView,
    ids_by_slug: &HashMap<String, String>,
) -> (Vec<Finding>, Vec<Finding>) {
    // R20: one table per rel per run, and one compiled pattern per prefix
    let mut patterns: HashMap<String, Regex> = HashMap::new();
    let mut tables: HashMap<Rel, HashMap<u32, IndexSet<String>>> = HashMap::new();
    let (mut kept, mut suppressed) = (Vec::new(), Vec::new());

    for f in findings {
        let rel = &f.site.rel;
        if !tables.contains_key(rel) {
            let table = if let Some(lines) = facts.module_lines(rel) {
                let prefix = facts.comment_prefix(rel).to_string();
                let marker = patterns
                    .entry(prefix.clone())
                    .or_insert_with(|| suppress_pattern(&prefix));
                marker_table(lines, marker, &prefix, ids_by_slug)
            } else if let Some(lines) = facts.doc_files().get(rel) {
                marker_table(lines, doc_suppress_re(), "<!--", ids_by_slug)
            } else {
                HashMap::new()
            };
            tables.insert(rel.clone(), table);
        }
        let hit = tables[rel]
            .get(&f.site.line)
            .is_some_and(|marks| marks.contains(f.rule));
        if hit {
            suppressed.push(f)
        } else {
            kept.push(f)
        }
    }
    (kept, suppressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::findings::tests::{ast, finding};
    use crate::findings::{Finding, Site};
    use crate::lang::Stack;
    use crate::testing::{P, Q, SyntheticStack};

    fn ids_by_slug() -> HashMap<String, String> {
        HashMap::from([
            ("contract-implied-guard".to_string(), "3".to_string()),
            ("structural-clones".to_string(), "11".to_string()),
        ])
    }

    fn at(rule: &'static str, rel: &str, line: u32, cause: &str) -> Finding {
        Finding {
            site: Site {
                rel: rel.into(),
                line,
                col: 0,
                symbol: "m".into(),
            },
            cause: cause.into(),
            ..finding(rule, ast())
        }
    }

    /// The mini repo `scratch/core-a/probe_order.py` builds, line for line;
    /// `P` spells `#`, the prefix REF's Python facts report.
    fn probe_stack() -> SyntheticStack {
        SyntheticStack::new(
            &P,
            &[
                (
                    "m.p",
                    "def plain():
    return 1
def hairy(x):
    for i in x:
        if i:
            if i > 2:
                return i
    return 0
z = 1  # sightline-ok: 34
# sightline-ok: contract-implied-guard
w = 2
v = 3  # sightline-ok: 56, 59
u = 4  # sightline-ok: not-a-mark
",
                ),
                (
                    "docs/guide.md",
                    "a claim <!-- sightline-ok: 11 -->
<!-- sightline-ok: structural-clones -->
another claim
a third claim <!-- sightline-ok: 7 -->
",
                ),
            ],
        )
    }

    #[test]
    fn the_marker_reads_ids_slugs_and_lists_in_code_and_in_docs() {
        // the two lists below are what REF's `sightline.findings.suppress`
        // answered on this repo (scratch/core-a/probe_order.py)
        let stack = probe_stack();
        let (kept, suppressed) = suppress(
            vec![
                at("34", "m.p", 9, "a"),           // trailing id
                at("3", "m.p", 11, "b"),           // slug, marker on the line above
                at("56", "m.p", 12, "c"),          // first of a pair
                at("59", "m.p", 12, "d"),          // second of a pair
                at("34", "m.p", 13, "e"),          // an unknown mark suppresses nothing
                at("11", "docs/guide.md", 1, "f"), // doc, trailing
                at("11", "docs/guide.md", 3, "g"), // doc, slug on the line above
                at("11", "docs/guide.md", 4, "h"), // doc, wrong rule id
                at("34", "gone.p", 1, "i"),        // a path no module holds
            ],
            stack.neutral(),
            &ids_by_slug(),
        );
        let causes = |fs: &[Finding]| fs.iter().map(|f| f.cause.clone()).collect::<Vec<_>>();
        assert_eq!(causes(&kept), ["e", "h", "i"]);
        assert_eq!(causes(&suppressed), ["a", "b", "c", "d", "f", "g"]);
    }

    #[test]
    fn the_marker_takes_the_owning_languages_comment_syntax() {
        let stack = SyntheticStack::new(
            &Q,
            &[(
                "m.q",
                "fn a() {}  // sightline-ok: 34\nfn b() {}  # sightline-ok: 34\n",
            )],
        );
        let (kept, suppressed) = suppress(
            vec![at("34", "m.q", 1, "a"), at("34", "m.q", 2, "b")],
            stack.neutral(),
            &ids_by_slug(),
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(suppressed[0].cause, "a");
        assert_eq!(kept[0].cause, "b");
    }

    #[test]
    fn a_bare_marker_suppresses_nothing() {
        let stack = SyntheticStack::new(&Q, &[("m.q", "y = 2  // sightline-ok\n")]);
        let (kept, suppressed) = suppress(
            vec![at("3", "m.q", 1, "a")],
            stack.neutral(),
            &ids_by_slug(),
        );
        assert_eq!(kept.len(), 1);
        assert!(suppressed.is_empty());
    }

    #[test]
    fn the_doc_marker_wants_its_closing_bracket() {
        let table = marker_table(
            &["<!-- sightline-ok: 3", "<!-- sightline-ok: 3 -->"],
            doc_suppress_re(),
            "<!--",
            &HashMap::new(),
        );
        assert_eq!(table.keys().copied().collect::<Vec<_>>(), [3]);
    }
}
