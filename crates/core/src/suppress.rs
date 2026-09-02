//! Suppression markers, and the per-path `overrides` a config declares.
//!
//! One concept, one grammar; a mark is a rule id or its slug. Code spells
//! it in its language's comment syntax, a doc in HTML. `sightline-ok` covers
//! a line, or the whole definition when the line opens one;
//! `sightline-ok-file` covers the file.

use std::collections::HashMap;
use std::sync::LazyLock;

use indexmap::IndexSet;
use regex::Regex;

use crate::config::Override;
use crate::findings::{Finding, Rel};
use crate::lang::FactsView;
use crate::walk::excluded;

const MARKS: &str = r"sightline-ok(-file)?:\s*([\w-]+(?:\s*,\s*[\w-]+)*)";

/// The table key of a file-wide marker: no line is 0.
pub const FILE_WIDE: u32 = 0;

/// The marker of a doc file (`.md`, `.rst`): `<!-- sightline-ok: ids -->`.
#[must_use]
#[allow(clippy::unwrap_used, reason = "a literal pattern")]
pub fn doc_suppress_re() -> &'static Regex {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!(r"<!--\s*{MARKS}\s*-->")).unwrap());
    &RE
}

/// The marker a language's comment syntax spells.
#[must_use]
#[allow(
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    reason = "a literal pattern around an escaped prefix"
)]
pub fn suppress_pattern(comment_prefix: &str) -> Regex {
    Regex::new(&format!(r"{}\s*{MARKS}", regex::escape(comment_prefix))).unwrap()
}

/// 1-based line -> rule ids suppressed there, slugs resolved. A comment-only
/// marker line applies to the next line; a trailing marker applies to its
/// own; a `-file` marker applies to `FILE_WIDE`.
#[allow(clippy::implicit_hasher, reason = "the registry's own map")]
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
        let i = u32::try_from(i + 1).unwrap_or(u32::MAX);
        // ceiling: `pytext::strip` (unit core-b) adds \x1c-\x1f to the
        // stripped set; `trim` covers Unicode White_Space alone.
        let target = if m.get(1).is_some() {
            FILE_WIDE
        } else if line.trim().starts_with(comment_prefix) {
            i + 1
        } else {
            i
        };
        let entry = out.entry(target).or_default();
        for part in m[2].split(',') {
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

/// Is `symbol` the definition `owner` opens, or nested in it?
fn within(symbol: &str, owner: &str) -> bool {
    symbol == owner
        || symbol
            .strip_prefix(owner)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with("::"))
}

/// The marker table of one file: a module's in its comment syntax, a doc's
/// in HTML, empty for a path no facts hold.
#[allow(clippy::implicit_hasher, reason = "the registry's own map")]
fn table_of(
    facts: &dyn FactsView,
    rel: &str,
    patterns: &mut HashMap<String, Regex>,
    ids_by_slug: &HashMap<String, String>,
) -> HashMap<u32, IndexSet<String>> {
    if let Some(lines) = facts.module_lines(rel) {
        let prefix = facts.comment_prefix(rel).to_string();
        let marker = patterns
            .entry(prefix.clone())
            .or_insert_with(|| suppress_pattern(&prefix));
        return marker_table(lines, marker, &prefix, ids_by_slug);
    }
    facts
        .doc_files()
        .get(rel)
        .map_or_else(HashMap::new, |lines| {
            marker_table(lines, doc_suppress_re(), "<!--", ids_by_slug)
        })
}

/// `ids_by_slug` is the registry's slug alias map, passed in by the caller:
///
/// rules read findings, so findings never reads rules. `overrides` are the
/// config's per-path `rules-off`, counted as suppressed like a marker.
#[allow(
    clippy::implicit_hasher,
    clippy::indexing_slicing,
    reason = "the registry's own map, and a table inserted the line above"
)]
pub fn suppress(
    findings: Vec<Finding>,
    facts: &dyn FactsView,
    ids_by_slug: &HashMap<String, String>,
    overrides: &[Override],
) -> (Vec<Finding>, Vec<Finding>) {
    // R20: one table per rel per run, and one compiled pattern per prefix
    let mut patterns: HashMap<String, Regex> = HashMap::new();
    let mut tables: HashMap<Rel, HashMap<u32, IndexSet<String>>> = HashMap::new();
    // the definitions each file opens, by their first line, for the marker
    // that sits on a `def` or a `fn` and covers its body
    let mut defs: HashMap<&str, Vec<(&str, u32)>> = HashMap::new();
    for (qname, sym) in facts.symbols() {
        if let Some(module) = facts.modules().get(&sym.module) {
            defs.entry(&module.rel)
                .or_default()
                .push((qname, sym.lineno));
        }
    }
    let off: Vec<(&Override, Vec<String>)> = overrides
        .iter()
        .map(|o| {
            let ids = o
                .rules_off
                .iter()
                .map(|m| ids_by_slug.get(m).cloned().unwrap_or_else(|| m.clone()))
                .collect();
            (o, ids)
        })
        .collect();
    let (mut kept, mut suppressed) = (Vec::new(), Vec::new());

    for f in findings {
        let rel = &f.site.rel;
        if !tables.contains_key(rel) {
            let table = table_of(facts, rel, &mut patterns, ids_by_slug);
            tables.insert(rel.clone(), table);
        }
        let table = &tables[rel];
        let marked = |line: u32| table.get(&line).is_some_and(|marks| marks.contains(f.rule));
        let hit = marked(FILE_WIDE)
            || marked(f.site.line)
            || defs.get(&**rel).is_some_and(|owners| {
                owners
                    .iter()
                    .any(|(owner, line)| marked(*line) && within(&f.site.symbol, owner))
            })
            || off
                .iter()
                .any(|(o, ids)| ids.iter().any(|id| id == f.rule) && excluded(rel, &o.paths));
        if hit {
            suppressed.push(f);
        } else {
            kept.push(f);
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

    /// The mini repo these tests share, line for line; `P` spells its
    /// comments with `#`.
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
        // the two lists below are what `suppress` answers on that repo
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
            &[],
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
            &[],
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
            &[],
        );
        assert_eq!(kept.len(), 1);
        assert!(suppressed.is_empty());
    }

    /// A marker on the line that opens a definition covers the whole
    /// definition, nested defs included; a `-file` marker covers the file.
    #[test]
    fn a_definition_marker_covers_its_body_and_a_file_marker_the_file() {
        let mut stack = SyntheticStack::new(
            &P,
            &[(
                "m.p",
                "# sightline-ok-file: 56\ndef plain():\n    return 1\n\
                 def hairy(x):  # sightline-ok: 34\n    return 0\n",
            )],
        );
        stack.neutral_mut().symbols.insert(
            "p::m.hairy".into(),
            crate::lang::NeutralSymbol {
                module: "p::m".into(),
                lineno: 4,
                end_lineno: 5,
                kind: "function",
            },
        );
        let owned = |rule, line, cause, symbol: &str| {
            let mut f = at(rule, "m.p", line, cause);
            f.site.symbol = symbol.into();
            f
        };
        let (kept, suppressed) = suppress(
            vec![
                owned("34", 5, "a", "p::m.hairy"),
                owned("34", 5, "b", "p::m.hairy.inner"),
                owned("34", 3, "c", "p::m.plain"),
                owned("56", 2, "d", "p::m.plain"),
                owned("34", 2, "e", "p::m.plain"),
            ],
            stack.neutral(),
            &ids_by_slug(),
            &[],
        );
        let causes = |fs: &[Finding]| fs.iter().map(|f| f.cause.clone()).collect::<Vec<_>>();
        assert_eq!(causes(&suppressed), ["a", "b", "d"]);
        assert_eq!(causes(&kept), ["c", "e"]);
    }

    #[test]
    fn an_override_switches_a_rule_off_under_its_paths() {
        let stack = SyntheticStack::new(&P, &[("tests/t.p", "x\n"), ("src/m.p", "y\n")]);
        let overrides = [Override {
            paths: vec!["tests".to_string()],
            rules_off: std::collections::BTreeSet::from(["structural-clones".to_string()]),
        }];
        let (kept, suppressed) = suppress(
            vec![
                at("11", "tests/t.p", 1, "a"),
                at("34", "tests/t.p", 1, "b"),
                at("11", "src/m.p", 1, "c"),
            ],
            stack.neutral(),
            &ids_by_slug(),
            &overrides,
        );
        assert_eq!(suppressed.len(), 1);
        assert_eq!(suppressed[0].cause, "a");
        assert_eq!(kept.len(), 2);
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
