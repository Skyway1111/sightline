//! Baseline ratchet (port of `ratchet.py`): `.sightline-baseline.json`
//! holds finding counts keyed (rule, qualified symbol name), and counts only
//! decrease per key. Symbol grain survives file moves and exposes
//! fix-one-add-one churn.

use anyhow::Context;
use camino::Utf8Path;
use indexmap::IndexMap;
use serde::Deserialize;

use crate::findings::Finding;

pub const BASELINE_NAME: &str = ".sightline-baseline.json";

/// `"<rule>|<symbol qname>"` -> allowed count.
pub type Counts = IndexMap<String, u32>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Baseline {
    pub counts: Counts,
}

pub fn key(f: &Finding) -> String {
    format!("{}|{}", f.rule, f.site.symbol)
}

pub fn snapshot(findings: &[Finding]) -> Counts {
    let mut counts = Counts::new();
    for f in findings {
        *counts.entry(key(f)).or_insert(0) += 1;
    }
    counts
}

#[derive(Deserialize)]
struct Wire {
    #[serde(default)]
    counts: IndexMap<String, serde_json::Value>,
}

/// `None` where no file sits there; an unreadable or non-numeric file is an
/// error, as `json.loads` and `int(v)` are in the reference tool.
pub fn load(path: &Utf8Path) -> anyhow::Result<Option<Baseline>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    let wire: Wire = serde_json::from_str(&text).with_context(|| format!("parse {path}"))?;
    let mut counts = Counts::new();
    for (k, v) in wire.counts {
        let n = match &v {
            serde_json::Value::Number(n) => n.as_f64().map(|f| f as u32),
            serde_json::Value::String(s) => s.parse().ok(),
            _ => None,
        };
        counts.insert(
            k.clone(),
            n.with_context(|| format!("{path}: {k} is not a count"))?,
        );
    }
    Ok(Some(Baseline { counts }))
}

#[derive(serde::Serialize)]
struct Payload<'a> {
    version: u32,
    counts: std::collections::BTreeMap<&'a str, u32>,
}

pub fn save(path: &Utf8Path, baseline: &Baseline) -> anyhow::Result<()> {
    let payload = Payload {
        version: 2,
        counts: baseline
            .counts
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect(),
    };
    // ceiling: `pyjson::dumps` (unit core-b) is the writer once it lands; a
    // non-ASCII key escapes as \uXXXX there and prints raw here.
    let body = serde_json::to_string_pretty(&payload)?;
    std::fs::write(path, format!("{body}\n")).with_context(|| format!("write {path}"))
}

/// Absorb up to the baselined count per key (stable location order); the
/// excess are regressions and stay reported. A count cannot say which of a
/// key's sites is the new one, so an over-budget key's report names every
/// site rather than pointing at the last by location.
pub fn diff(findings: Vec<Finding>, counts: &Counts) -> (Vec<Finding>, u32) {
    let mut by_key: IndexMap<String, Vec<Finding>> = IndexMap::new();
    for f in findings {
        by_key.entry(key(&f)).or_default().push(f);
    }
    let mut kept: Vec<Finding> = Vec::new();
    let mut absorbed = 0;
    for (k, mut group) in by_key {
        let budget = counts.get(&k).copied().unwrap_or(0);
        group.sort_by(|a, b| {
            (&a.site.rel, a.site.line, a.site.col, &a.cause).cmp(&(
                &b.site.rel,
                b.site.line,
                b.site.col,
                &b.cause,
            ))
        });
        absorbed += budget.min(group.len() as u32);
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

/// Counts only decrease: lower each key to the current count, drop the
/// satisfied keys.
pub fn prune(findings: &[Finding], counts: &Counts) -> Counts {
    let now = snapshot(findings);
    counts
        .iter()
        .filter_map(|(k, v)| {
            let n = now.get(k).copied().unwrap_or(0);
            (n > 0).then(|| (k.clone(), (*v).min(n)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::findings::tests::{ast, finding};
    use crate::findings::{Finding, Site};

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

    fn tmp() -> (tempfile::TempDir, camino::Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = camino::Utf8Path::from_path(dir.path())
            .unwrap()
            .join(BASELINE_NAME);
        (dir, path)
    }

    #[test]
    fn snapshot_and_round_trip() {
        let (_dir, path) = tmp();
        let fs = [at("1", "m.f", 1), at("1", "m.f", 2), at("9", "m.g", 1)];
        save(
            &path,
            &Baseline {
                counts: snapshot(&fs),
            },
        )
        .unwrap();
        let baseline = load(&path).unwrap().unwrap();
        assert_eq!(baseline.counts["1|m.f"], 2);
        assert_eq!(baseline.counts["9|m.g"], 1);
        assert_eq!(baseline.counts.len(), 2);
    }

    #[test]
    fn save_writes_the_reference_tools_bytes() {
        // scratch/core-a/probe_ratchet.py printed these from REF's
        // `ratchet.save`. Python's `write_text` translates \n to the
        // platform terminator, so its Windows bytes carry CR; the repo
        // stores the file with LF (its .gitattributes normalizes) and this
        // port writes LF everywhere.
        let (_dir, path) = tmp();
        let fs = [
            at("1", "m.f", 2),
            at("1", "m.f", 1),
            at("9", "m.g", 1),
            at("11", "pkg.mod.Cls.meth", 1),
            at("11", "a|b", 1),
            at("2", "m.f", 1),
        ];
        save(
            &path,
            &Baseline {
                counts: snapshot(&fs),
            },
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"version\": 2,\n  \"counts\": {\n    \"11|a|b\": 1,\n\
             \x20   \"11|pkg.mod.Cls.meth\": 1,\n    \"1|m.f\": 2,\n\
             \x20   \"2|m.f\": 1,\n    \"9|m.g\": 1\n  }\n}\n"
        );

        save(&path, &Baseline::default()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"version\": 2,\n  \"counts\": {}\n}\n"
        );
    }

    #[test]
    fn diff_absorbs_the_baseline_and_reports_the_regressions() {
        let baseline: Counts = [("1|m.f".to_string(), 2)].into_iter().collect();
        let same = vec![at("1", "m.f", 1), at("1", "m.f", 2)];
        let (kept, absorbed) = diff(same, &baseline);
        assert!(kept.is_empty());
        assert_eq!(absorbed, 2);

        let grown = vec![at("1", "m.f", 1), at("1", "m.f", 2), at("1", "m.f", 9)];
        let (kept, absorbed) = diff(grown, &baseline);
        assert_eq!(kept.iter().map(|f| f.site.line).collect::<Vec<_>>(), [9]);
        assert_eq!(absorbed, 2);
    }

    #[test]
    fn an_over_budget_key_names_every_site() {
        // a count cannot say which site is new: the report names them all,
        // never only the last by location (probe_ratchet.py)
        let baseline: Counts = [("1|m.f".to_string(), 2)].into_iter().collect();
        let (kept, absorbed) = diff(
            vec![at("1", "m.f", 1), at("1", "m.f", 2), at("1", "m.f", 9)],
            &baseline,
        );
        assert_eq!(absorbed, 2);
        assert_eq!(kept.len(), 1);
        assert_eq!(
            kept[0].message,
            "msg [1|m.f: 3 sites over a baseline of 2; lines 1, 2, 9]"
        );

        // budget 0: every site is new, and nothing is named
        let (kept, _) = diff(vec![at("1", "m.f", 5)], &Counts::new());
        assert_eq!(kept[0].message, "msg");
    }

    #[test]
    fn the_absorbed_sites_are_the_first_by_location_then_cause() {
        let baseline: Counts = [("1|m.f".to_string(), 1)].into_iter().collect();
        let mut early = at("1", "m.f", 2);
        early.cause = "a".into();
        let mut far = at("1", "m.f", 9);
        far.site.rel = "z.py".into();
        let mut same_line = at("1", "m.f", 2);
        same_line.site.col = 4;
        let (kept, absorbed) = diff(vec![far, at("1", "m.f", 2), same_line, early], &baseline);
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
        let baseline: Counts = [("1|m.f".to_string(), 1)].into_iter().collect();
        let (kept, absorbed) = diff(vec![at("1", "m.g", 5)], &baseline);
        assert_eq!(
            kept.iter()
                .map(|f| f.site.symbol.to_string())
                .collect::<Vec<_>>(),
            ["m.g"]
        );
        assert_eq!(absorbed, 0);
    }

    #[test]
    fn prune_lowers_and_drops() {
        let counts: Counts = [("1|m.f".to_string(), 3), ("9|m.g".to_string(), 1)]
            .into_iter()
            .collect();
        let pruned = prune(&[at("1", "m.f", 1)], &counts);
        assert_eq!(
            pruned,
            [("1|m.f".to_string(), 1)].into_iter().collect::<Counts>()
        );

        let (_dir, path) = tmp();
        save(&path, &Baseline { counts: pruned }).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        save(&path, &load(&path).unwrap().unwrap()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    }

    #[test]
    fn load_answers_none_for_a_missing_file_and_reads_a_v1_one() {
        let (_dir, path) = tmp();
        assert!(load(&path.with_file_name("absent.json")).unwrap().is_none());
        std::fs::write(&path, r#"{"version": 1, "counts": {"1|m.f": 2}}"#).unwrap();
        assert_eq!(load(&path).unwrap().unwrap().counts["1|m.f"], 2);
    }
}
