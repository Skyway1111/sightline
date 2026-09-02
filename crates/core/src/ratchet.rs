// file-length-ok: the ratchet and the tests that pin its file format and its shape match
//! Baseline ratchet: `.sightline-baseline`, one line per key.
//!
//! A line is `<rule>|<symbol qname>` with the count allowed there and the
//! shape of the symbol's body, and counts only decrease per key. A finding matches its
//! key by qname first, and a symbol that was renamed, moved into a class or
//! split into another module matches by shape, so the ratchet blocks what a
//! change adds and not what it moves. The file is one key per line so a
//! `merge=union` attribute settles a merge, and a line duplicated by one
//! keeps the larger count.

use std::collections::HashSet;
use std::fmt::Write as _;

use anyhow::Context;
use camino::Utf8Path;
use indexmap::IndexMap;
use sha2::{Digest, Sha256};

use crate::findings::Finding;
use crate::lang::FactsView;

pub const BASELINE_NAME: &str = ".sightline-baseline";
/// The 0.2 file; `load` reads it where the current one is absent, and
/// `save` removes it once the current one is written.
pub const LEGACY_NAME: &str = ".sightline-baseline.json";
const HEADER: &str = "# sightline baseline: `<rule>|<symbol> <count> [<shape>]`, one per line; \
                      `merge=union` is safe";

/// What a key allows: the count and the shape the symbol's body had.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub count: u32,
    pub shape: Option<String>,
}

/// `"<rule>|<symbol qname>"` -> what it allows.
pub type Counts = IndexMap<String, Entry>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Baseline {
    pub counts: Counts,
}

#[must_use]
pub fn key(f: &Finding) -> String {
    format!("{}|{}", f.rule, f.site.symbol)
}

/// `line` with every whole-word spelling of `name` blanked.
#[allow(
    clippy::string_slice,
    reason = "`at` is where `find` matched `name`, and `name.len()` past it is the match's end: both are char boundaries"
)]
fn blind(line: &str, name: &str) -> String {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find(name) {
        let after = at + name.len();
        let whole = rest[..at].chars().next_back().is_none_or(|c| !ident(c))
            && rest[after..].chars().next().is_none_or(|c| !ident(c));
        out.push_str(&rest[..at]);
        out.push_str(if whole { "$" } else { name });
        rest = &rest[after..];
    }
    out + rest
}

/// The shape of the finding's symbol.
///
/// A digest of its body with the whitespace, the blank lines and its own
/// name taken out, so the same body under another name, indentation or
/// module has the same shape. A module-scope finding has none.
#[allow(clippy::string_slice, reason = "a hex digest is ASCII")]
pub fn shape(facts: &dyn FactsView, f: &Finding) -> Option<String> {
    let sym = facts.symbols().get(&*f.site.symbol)?;
    let lines = facts.module_lines(&f.site.rel)?;
    if sym.end_lineno == 0 || sym.lineno == 0 {
        return None;
    }
    let name = f.site.symbol.rsplit(['.', ':']).next().unwrap_or_default();
    let body = lines
        .get(sym.lineno as usize - 1..sym.end_lineno as usize)?
        .iter()
        .map(|l| blind(l.trim(), name))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!("{:x}", Sha256::digest(body.as_bytes()))[..16].to_string())
}

pub fn snapshot(findings: &[Finding], facts: &dyn FactsView) -> Counts {
    let mut counts = Counts::new();
    for f in findings {
        let entry = counts.entry(key(f)).or_insert_with(|| Entry {
            count: 0,
            shape: shape(facts, f),
        });
        entry.count += 1;
    }
    counts
}

/// One line of the file, `key count [shape]`; the shape is last so a key
/// holding any character but a space still parses.
fn parse_line(line: &str) -> Option<(String, Entry)> {
    let mut parts = line.split(' ');
    let key = parts.next()?.to_string();
    let count = parts.next()?.parse().ok()?;
    let shape = parts.next().map(str::to_string);
    Some((key, Entry { count, shape }))
}

/// `None` where no file sits there; the legacy JSON where only it does.
///
/// # Errors
///
/// A file that is there but cannot be read or parsed.
pub fn load(root: &Utf8Path) -> anyhow::Result<Option<Baseline>> {
    let path = root.join(BASELINE_NAME);
    if !path.is_file() {
        return load_legacy(&root.join(LEGACY_NAME));
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
    let mut counts = Counts::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, entry) =
            parse_line(line).with_context(|| format!("{path}: bad line {line:?}"))?;
        // a union merge can duplicate a line: the larger count holds
        let slot = counts.entry(key).or_default();
        if entry.count > slot.count {
            *slot = entry;
        }
    }
    Ok(Some(Baseline { counts }))
}

fn load_legacy(path: &Utf8Path) -> anyhow::Result<Option<Baseline>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    let wire: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("parse {path}"))?;
    let mut counts = Counts::new();
    let rows = wire.get("counts").and_then(serde_json::Value::as_object);
    for (k, v) in rows.into_iter().flatten() {
        let count = match v {
            serde_json::Value::Number(n) => n.as_u64().and_then(|f| u32::try_from(f).ok()),
            serde_json::Value::String(s) => s.parse().ok(),
            _ => None,
        };
        let count = count.with_context(|| format!("{path}: {k} is not a count"))?;
        counts.insert(k.clone(), Entry { count, shape: None });
    }
    Ok(Some(Baseline { counts }))
}

/// Writes the file in key order, LF on every platform, and removes the
/// legacy JSON beside it. Answers whether a legacy file was removed.
///
/// # Errors
///
/// The write, or the removal of the legacy file.
pub fn save(root: &Utf8Path, baseline: &Baseline) -> anyhow::Result<bool> {
    let mut rows: Vec<(&String, &Entry)> = baseline.counts.iter().collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));
    let mut body = format!("{HEADER}\n");
    for (key, entry) in rows {
        body.push_str(key);
        let _ = write!(body, " {}", entry.count);
        if let Some(shape) = &entry.shape {
            let _ = write!(body, " {shape}");
        }
        body.push('\n');
    }
    let path = root.join(BASELINE_NAME);
    std::fs::write(&path, body).with_context(|| format!("write {path}"))?;
    let legacy = root.join(LEGACY_NAME);
    if legacy.is_file() {
        std::fs::remove_file(&legacy).with_context(|| format!("remove {legacy}"))?;
        return Ok(true);
    }
    Ok(false)
}

/// Absorb up to the baselined count per key, in stable location order.
///
/// The excess are regressions and stay reported. A key the baseline lacks
/// takes the budget of an unclaimed entry of its rule with its shape, which
/// is how a renamed or moved symbol keeps its allowance. A count cannot say
/// which of a key's sites is the new one, so an over-budget key's report
/// names every site rather than pointing at the last by location.
#[allow(
    clippy::indexing_slicing,
    reason = "a group holds the finding that opened it"
)]
pub fn diff(findings: Vec<Finding>, counts: &Counts, facts: &dyn FactsView) -> (Vec<Finding>, u32) {
    let mut by_key: IndexMap<String, Vec<Finding>> = IndexMap::new();
    for f in findings {
        by_key.entry(key(&f)).or_default().push(f);
    }
    let mut claimed: HashSet<&str> = counts
        .keys()
        .filter(|k| by_key.contains_key(*k))
        .map(String::as_str)
        .collect();
    let mut kept: Vec<Finding> = Vec::new();
    let mut absorbed = 0;
    for (k, mut group) in by_key {
        let budget = counts.get(&k).map_or_else(
            || moved_budget(counts, &mut claimed, facts, &group[0]),
            |entry| entry.count,
        );
        group.sort_by(|a, b| {
            (&a.site.rel, a.site.line, a.site.col, &a.cause).cmp(&(
                &b.site.rel,
                b.site.line,
                b.site.col,
                &b.cause,
            ))
        });
        absorbed += budget.min(u32::try_from(group.len()).unwrap_or(u32::MAX));
        let lines: Vec<String> = group.iter().map(|f| f.site.line.to_string()).collect();
        let over = format!(
            " [{k}: {} sites over a baseline of {budget}; lines {}]",
            group.len(),
            lines.join(", ")
        );
        let mut excess: Vec<Finding> = group.split_off((budget as usize).min(group.len()));
        if budget > 0 {
            for f in &mut excess {
                f.message.push_str(&over);
            }
        }
        kept.extend(excess);
    }
    (kept, absorbed)
}

/// The budget of an unclaimed entry of the finding's rule with its shape,
/// which the finding then claims; 0 where none.
fn moved_budget<'a>(
    counts: &'a Counts,
    claimed: &mut HashSet<&'a str>,
    facts: &dyn FactsView,
    first: &Finding,
) -> u32 {
    let rule = format!("{}|", first.rule);
    let shape = shape(facts, first);
    let moved = counts.iter().find(|(key, entry)| {
        key.starts_with(&rule)
            && entry.shape.is_some()
            && entry.shape == shape
            && !claimed.contains(key.as_str())
    });
    moved.map_or(0, |(key, entry)| {
        claimed.insert(key);
        entry.count
    })
}

/// Counts only decrease: lower each key to the current count, drop the
/// satisfied keys, and refresh the shapes to the bodies as they are now.
pub fn prune(findings: &[Finding], counts: &Counts, facts: &dyn FactsView) -> Counts {
    let now = snapshot(findings, facts);
    counts
        .iter()
        .filter_map(|(k, v)| {
            let current = now.get(k)?;
            Some((
                k.clone(),
                Entry {
                    count: v.count.min(current.count),
                    shape: current.shape.clone(),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::findings::tests::{ast, finding};
    use crate::findings::{Finding, Site};
    use crate::lang::{Neutral, Stack};
    use crate::testing::{P, SyntheticStack};

    fn at(rule: &'static str, symbol: &str, line: u32) -> Finding {
        Finding {
            site: Site {
                rel: "m.py".into(),
                line,
                col: 0,
                symbol: symbol.into(),
            },
            cause: format!("c{line}"),
            ..finding(rule, ast())
        }
    }

    /// A tree whose symbols the fixture holds none of: every shape is `None`.
    fn bare() -> SyntheticStack {
        let mut stack = SyntheticStack::new(&P, &[("m.p", "x\n")]);
        stack.neutral_mut().symbols.clear();
        stack
    }

    fn tmp() -> (tempfile::TempDir, camino::Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = camino::Utf8Path::from_path(dir.path())
            .unwrap()
            .to_path_buf();
        (dir, path)
    }

    fn counts(rows: &[(&str, u32)]) -> Counts {
        rows.iter()
            .map(|(k, n)| {
                (
                    k.to_string(),
                    Entry {
                        count: *n,
                        shape: None,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn snapshot_and_round_trip() {
        let (_dir, root) = tmp();
        let stack = bare();
        let fs = [at("1", "m.f", 1), at("1", "m.f", 2), at("9", "m.g", 1)];
        save(
            &root,
            &Baseline {
                counts: snapshot(&fs, stack.neutral()),
            },
        )
        .unwrap();
        let baseline = load(&root).unwrap().unwrap();
        assert_eq!(baseline.counts["1|m.f"].count, 2);
        assert_eq!(baseline.counts["9|m.g"].count, 1);
        assert_eq!(baseline.counts.len(), 2);
    }

    #[test]
    fn save_writes_one_key_per_line_in_key_order() {
        // LF on every platform, never the platform terminator
        let (_dir, root) = tmp();
        let stack = bare();
        let fs = [
            at("1", "m.f", 2),
            at("1", "m.f", 1),
            at("9", "m.g", 1),
            at("11", "pkg.mod.Cls.meth", 1),
            at("2", "m.f", 1),
        ];
        save(
            &root,
            &Baseline {
                counts: snapshot(&fs, stack.neutral()),
            },
        )
        .unwrap();
        let text = std::fs::read_to_string(root.join(BASELINE_NAME)).unwrap();
        assert_eq!(
            text,
            format!("{HEADER}\n11|pkg.mod.Cls.meth 1\n1|m.f 2\n2|m.f 1\n9|m.g 1\n")
        );
        save(&root, &Baseline::default()).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join(BASELINE_NAME)).unwrap(),
            format!("{HEADER}\n")
        );
    }

    #[test]
    fn a_union_merge_keeps_the_larger_count_and_a_comment_reads_as_nothing() {
        let (_dir, root) = tmp();
        std::fs::write(
            root.join(BASELINE_NAME),
            "# a comment\n\n1|m.f 2 abcd\n1|m.f 3 abcd\n9|m.g 1\n",
        )
        .unwrap();
        let baseline = load(&root).unwrap().unwrap();
        assert_eq!(baseline.counts["1|m.f"].count, 3);
        assert_eq!(baseline.counts["1|m.f"].shape.as_deref(), Some("abcd"));
        assert_eq!(baseline.counts["9|m.g"].shape, None);
    }

    #[test]
    fn diff_absorbs_the_baseline_and_reports_the_regressions() {
        let stack = bare();
        let baseline = counts(&[("1|m.f", 2)]);
        let same = vec![at("1", "m.f", 1), at("1", "m.f", 2)];
        let (kept, absorbed) = diff(same, &baseline, stack.neutral());
        assert!(kept.is_empty());
        assert_eq!(absorbed, 2);

        let grown = vec![at("1", "m.f", 1), at("1", "m.f", 2), at("1", "m.f", 9)];
        let (kept, absorbed) = diff(grown, &baseline, stack.neutral());
        assert_eq!(kept.iter().map(|f| f.site.line).collect::<Vec<_>>(), [9]);
        assert_eq!(absorbed, 2);
    }

    #[test]
    fn an_over_budget_key_names_every_site() {
        // a count cannot say which site is new: the report names them all,
        // never only the last by location
        let stack = bare();
        let baseline = counts(&[("1|m.f", 2)]);
        let (kept, absorbed) = diff(
            vec![at("1", "m.f", 1), at("1", "m.f", 2), at("1", "m.f", 9)],
            &baseline,
            stack.neutral(),
        );
        assert_eq!(absorbed, 2);
        assert_eq!(kept.len(), 1);
        assert_eq!(
            kept[0].message,
            "msg [1|m.f: 3 sites over a baseline of 2; lines 1, 2, 9]"
        );

        // budget 0: every site is new, and nothing is named
        let (kept, _) = diff(vec![at("1", "m.f", 5)], &Counts::new(), stack.neutral());
        assert_eq!(kept[0].message, "msg");
    }

    #[test]
    fn the_absorbed_sites_are_the_first_by_location_then_cause() {
        let stack = bare();
        let baseline = counts(&[("1|m.f", 1)]);
        let mut early = at("1", "m.f", 2);
        early.cause = "a".into();
        let mut far = at("1", "m.f", 9);
        far.site.rel = "z.py".into();
        let mut same_line = at("1", "m.f", 2);
        same_line.site.col = 4;
        let (kept, absorbed) = diff(
            vec![far, at("1", "m.f", 2), same_line, early],
            &baseline,
            stack.neutral(),
        );
        assert_eq!(absorbed, 1);
        assert_eq!(
            kept.iter()
                .map(|f| (&*f.site.rel, f.site.line, f.site.col, &*f.cause))
                .collect::<Vec<_>>(),
            [
                ("m.py", 2, 0, "c2"),
                ("m.py", 2, 4, "c2"),
                ("z.py", 9, 0, "c9")
            ]
        );
        assert!(
            kept.iter().all(|f| f.message
                == "msg [1|m.f: 4 sites over a baseline of 1; lines 2, 2, 2, 9]")
        );
    }

    #[test]
    fn a_same_count_swap_across_symbols_is_a_regression() {
        let stack = bare();
        let baseline = counts(&[("1|m.f", 1)]);
        let (kept, absorbed) = diff(vec![at("1", "m.g", 5)], &baseline, stack.neutral());
        assert_eq!(
            kept.iter()
                .map(|f| f.site.symbol.to_string())
                .collect::<Vec<_>>(),
            ["m.g"]
        );
        assert_eq!(absorbed, 0);
    }

    /// The tree these tests rename in: `m.p` holds `fn` at lines 1-2, and
    /// `renamed.p` the same body under another name at another indent.
    fn renamed() -> (SyntheticStack, Finding, Finding) {
        let mut stack = SyntheticStack::new(
            &P,
            &[
                ("m.p", "def fn(x)\n    return fn(x)\n"),
                (
                    "renamed.p",
                    "class C\n    def go(x)\n        return go(x)\n",
                ),
            ],
        );
        let neutral: &mut Neutral = stack.neutral_mut();
        neutral.symbols.clear();
        for (q, module, lo, hi) in [
            ("p::m.fn", "p::m", 1, 2),
            ("p::renamed.C.go", "p::renamed", 2, 3),
        ] {
            neutral.symbols.insert(
                q.into(),
                crate::lang::NeutralSymbol {
                    module: module.into(),
                    lineno: lo,
                    end_lineno: hi,
                    kind: "function",
                },
            );
        }
        let mut before = at("1", "p::m.fn", 1);
        before.site.rel = "m.p".into();
        let mut after = at("1", "p::renamed.C.go", 2);
        after.site.rel = "renamed.p".into();
        (stack, before, after)
    }

    #[test]
    fn a_renamed_and_moved_symbol_keeps_its_shape() {
        let (mut stack, before, after) = renamed();
        let original = shape(stack.neutral(), &before);
        assert!(original.is_some());
        assert_eq!(original, shape(stack.neutral(), &after));
        // a module-scope finding has none
        assert_eq!(shape(stack.neutral(), &at("1", "p::m", 1)), None);
        // a body that changed is another shape
        stack
            .neutral_mut()
            .modules
            .get_mut("p::renamed")
            .unwrap()
            .lines = vec![
            "class C".into(),
            "    def go(x)".into(),
            "        return 1".into(),
        ]
        .into();
        assert_ne!(shape(stack.neutral(), &after), original);
    }

    #[test]
    fn diff_matches_a_renamed_symbol_by_shape_once() {
        let (stack, before, after) = renamed();
        let facts: &Neutral = stack.neutral();
        let baseline = snapshot(std::slice::from_ref(&before), facts);
        assert!(baseline["1|p::m.fn"].shape.is_some());
        // the rename absorbs; a second copy of the same shape does not
        let (kept, absorbed) = diff(vec![after.clone()], &baseline, facts);
        assert!(kept.is_empty());
        assert_eq!(absorbed, 1);
        let twin = Finding {
            site: Site {
                rel: "m.p".into(),
                line: 1,
                col: 0,
                symbol: "p::m.fn".into(),
            },
            ..after.clone()
        };
        let (kept, absorbed) = diff(vec![twin, after.clone()], &baseline, facts);
        assert_eq!(absorbed, 1);
        assert_eq!(kept.len(), 1, "the exact key claims the entry first");
        assert_eq!(&*kept[0].site.symbol, "p::renamed.C.go");
        // another rule's entry never answers for this one
        let mut other = after;
        other.rule = "9";
        let (kept, _) = diff(vec![other], &baseline, facts);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn prune_lowers_drops_and_refreshes_the_shape() {
        let stack = bare();
        let counts = counts(&[("1|m.f", 3), ("9|m.g", 1)]);
        let pruned = prune(&[at("1", "m.f", 1)], &counts, stack.neutral());
        assert_eq!(pruned, self::counts(&[("1|m.f", 1)]));

        let (_dir, root) = tmp();
        save(&root, &Baseline { counts: pruned }).unwrap();
        let first = std::fs::read_to_string(root.join(BASELINE_NAME)).unwrap();
        save(&root, &load(&root).unwrap().unwrap()).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join(BASELINE_NAME)).unwrap(),
            first
        );
    }

    #[test]
    fn load_answers_none_for_a_missing_file_and_reads_the_legacy_json() {
        let (_dir, root) = tmp();
        assert!(load(&root).unwrap().is_none());
        std::fs::write(
            root.join(LEGACY_NAME),
            r#"{"version": 2, "counts": {"1|m.f": 2}}"#,
        )
        .unwrap();
        let legacy = load(&root).unwrap().unwrap();
        assert_eq!(legacy.counts["1|m.f"].count, 2);
        // saving writes the current file and removes the legacy one
        assert!(save(&root, &legacy).unwrap());
        assert!(!root.join(LEGACY_NAME).exists());
        assert!(!save(&root, &legacy).unwrap());
        assert_eq!(load(&root).unwrap().unwrap().counts["1|m.f"].count, 2);
    }
}
