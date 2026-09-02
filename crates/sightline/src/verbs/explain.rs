//! `sightline explain`: every
//! record under an id, or, for an id no rule holds, its burial from the
//! trail. Without an id it prints the roster, the registry read straight
//! out. Both tables are embedded, so a released binary
//! answers without a checkout.

use std::io::Write;
use std::sync::LazyLock;

use anyhow::Result;
use serde::Deserialize;

use sightline_core::findings::Engine;
use sightline_core::precision::{key_of, rule_recall, rule_samples};
use sightline_core::pytext::repr_str;
use sightline_core::registry::Registry;
use sightline_core::rule::RuleRecord;

/// One burial row of `corpus-ext/decisions.tsv`, as `xtask retired`
/// extracted it.
#[derive(Deserialize)]
struct Burial {
    id: String,
    decision: String,
    why: String,
    evidence: String,
}

#[derive(Deserialize)]
struct Retired {
    retired: Vec<Burial>,
}

fn burials() -> &'static [Burial] {
    static TABLE: LazyLock<Retired> = LazyLock::new(|| {
        toml::from_str(include_str!("../../../../data/retired.toml"))
            .expect("data/retired.toml is malformed")
    });
    &TABLE.retired
}

pub fn run(registry: &Registry, rule: Option<&str>, json: bool) -> Result<u8> {
    if json {
        std::io::stdout().write_all(self::json(registry).as_bytes())?;
        return Ok(0);
    }
    let Some(rule) = rule else {
        std::io::stdout().write_all(roster(registry).as_bytes())?;
        return Ok(0);
    };
    // the roster prints a slug beside every id, and `--rules` takes either,
    // so this verb takes either (`docs/reference.md`)
    let rule = registry.id_by_slug.get(rule).map_or(rule, String::as_str);
    let records: Vec<&RuleRecord> = registry.rules.iter().filter(|r| r.id == rule).collect();
    if records.is_empty() {
        return retired(registry, rule);
    }
    let mut out = String::new();
    for record in records {
        explain(record, &mut out);
    }
    std::io::stdout().write_all(out.as_bytes())?;
    Ok(0)
}

/// The engine a record documents, `None` where the rule mixes them.
fn engine_of(record: &RuleRecord) -> Option<Engine> {
    let spelled = record.engine_class.to_lowercase();
    [
        Engine::Counterfactual,
        Engine::Oracle,
        Engine::Wp,
        Engine::Idx,
        Engine::OracleUngrounded,
        Engine::Ast,
    ]
    .into_iter()
    .find(|e| e.value() == spelled)
}

/// What `rank` assumes of a rule no round has judged: the documented
/// engine's tier bar, unnamed where the rule mixes engines (each finding's
/// own engine decides then).
fn unjudged_bar(record: &RuleRecord) -> String {
    match engine_of(record).map(Engine::tier) {
        Some(tier) => format!("rank reads the {} bar {:.2}", tier.value(), tier.bar()),
        None => "its tier bar".to_string(),
    }
}

/// The roster: every reading the registry holds, one line each, so a reader
/// sees what this tool checks without a second list to drift from it. The
/// tier is the documented engine's, `mixed` where each finding's own engine
/// decides; the precision column is the judged sample where a round left
/// one.
fn roster(registry: &Registry) -> String {
    let width = registry
        .rules
        .iter()
        .map(|r| r.slug.len())
        .max()
        .unwrap_or(0);
    let line = |cell: [&str; 7]| {
        format!(
            "{:>4}  {:width$}  {:4}  {:6}  {:7}  {:9}  {}\n",
            cell[0], cell[1], cell[2], cell[3], cell[4], cell[5], cell[6],
        )
    };
    let mut out = line([
        "id",
        "slug",
        "lang",
        "family",
        "posture",
        "tier",
        "precision",
    ]);
    for r in &registry.rules {
        // the rule's own row where a round judged the whole rule, else the
        // first arm it judged, named so the number is not read as the rule's
        let judged = match rule_samples(r.id, r.lang).first() {
            Some(("", s)) => format!("{}/{}", s.tp, s.n),
            Some((arm, s)) => format!("{}/{} ({arm})", s.tp, s.n),
            None => "unmeasured".to_string(),
        };
        out.push_str(&line([
            &format!("#{}", r.id),
            r.slug,
            r.lang,
            r.family,
            r.posture.value(),
            engine_of(r).map_or("mixed", |e| e.tier().value()),
            &judged,
        ]));
    }
    out
}

/// Every reading as one JSON array, each with its judged rows: what
/// `cargo xtask rules-doc` renders `docs/rules.md` from, so the catalog and
/// `explain` cannot disagree.
fn json(registry: &Registry) -> String {
    let rows: Vec<serde_json::Value> = registry
        .rules
        .iter()
        .map(|r| {
            let precision: Vec<serde_json::Value> = rule_samples(r.id, r.lang)
                .into_iter()
                .map(|(arm, s)| {
                    serde_json::json!({"arm": arm, "tp": s.tp, "n": s.n, "seed": s.seed, "of": s.of})
                })
                .collect();
            let recall = rule_recall(r.id, r.lang)
                .map(|c| serde_json::json!({"covered": c.covered, "sites": c.sites, "of": c.of}));
            serde_json::json!({
                "id": r.id,
                "slug": r.slug,
                "lang": r.lang,
                "family": r.family,
                "engine": r.engine_class,
                "tier": engine_of(r).map_or("mixed", |e| e.tier().value()),
                "posture": r.posture.value(),
                "meaning": r.meaning,
                "goal": r.goal,
                "complement": r.complement,
                "precision": precision,
                "recall": recall,
            })
        })
        .collect();
    let mut out = serde_json::to_string_pretty(&rows).expect("records serialize");
    out.push('\n');
    out
}

fn explain(record: &RuleRecord, out: &mut String) {
    out.push_str(&format!(
        "#{} {} ({}, family {}, {}, {})\nchecks:  {}\ngoal:    {}\n",
        record.id,
        record.slug,
        record.lang,
        record.family,
        record.engine_class,
        record.posture.value(),
        record.meaning,
        record.goal,
    ));
    // what another linter already covers, so this rule need not
    if !record.complement.is_empty() {
        out.push_str(&format!("complement: {}\n", record.complement));
    }
    // the rule's own judged sample and its arms', keyed `<key>:<prefix>`
    let samples = rule_samples(record.id, record.lang);
    if samples.is_empty() {
        out.push_str(&format!(
            "precision: unmeasured (no round judged {}; {})\n",
            key_of(record.id, record.lang),
            unjudged_bar(record)
        ));
    }
    for (arm, sample) in samples {
        let label = if arm.is_empty() {
            String::new()
        } else {
            format!("{arm} arm ")
        };
        out.push_str(&format!(
            "precision: {label}{}/{} seed {} - {}\n",
            sample.tp, sample.n, sample.seed, sample.of
        ));
    }
    if let Some(recall) = rule_recall(record.id, record.lang) {
        out.push_str(&format!(
            "recall:    {}/{} - {}\n",
            recall.covered, recall.sites, recall.of
        ));
    }
}

/// An id no rule holds but the numbering reaches: its burial rows from the
/// trail. Retirement is an answer, not an error.
fn retired(registry: &Registry, rule: &str) -> Result<u8> {
    // the numbering reaches one past the last live id: a reading buried on
    // its first round keeps its id retired
    let last = registry
        .rules
        .iter()
        .filter_map(|r| r.id.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    let numbered = rule
        .parse::<u32>()
        .ok()
        .filter(|_| !rule.starts_with(['+', '-']));
    if !numbered.is_some_and(|n| n > 0 && n <= last + 1) {
        // one id per entry, not one per reading: two languages share an id
        let mut known: Vec<&str> = registry.rules.iter().map(|r| r.id).collect();
        known.dedup();
        eprintln!(
            "unknown rule {}; known: {}",
            repr_str(rule),
            known.join(", ")
        );
        return Ok(1);
    }
    let mut out = format!("#{rule} retired: no rule holds this id\n");
    let rows: Vec<&Burial> = burials().iter().filter(|b| b.id == rule).collect();
    for row in &rows {
        out.push_str(&format!(
            "cut:      {}\nwhy:      {}\nevidence: {}\n",
            row.decision, row.why, row.evidence
        ));
    }
    if rows.is_empty() {
        out.push_str("burial:  no trail row names this id\n");
    }
    std::io::stdout().write_all(out.as_bytes())?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use sightline_core::rule::{Posture, Scope};

    fn record(id: &'static str, lang: &'static str, engine_class: &'static str) -> RuleRecord {
        RuleRecord {
            id,
            slug: "unjudged",
            family: "C",
            engine_class,
            posture: Posture::Report,
            meaning: "m",
            goal: "g",
            lang,
            scope: Scope::Repo,
            complement: "",
        }
    }

    /// A reading no round has judged names the key a
    /// round would fill and the bar `rank` reads.
    #[test]
    fn an_unjudged_reading_names_its_key_and_its_bar() {
        let mut out = String::new();
        explain(&record("99", "rs", "WP"), &mut out);
        assert!(out.starts_with("#99 unjudged (rs, family C, WP, report)\n"));
        assert!(out.contains(
            "precision: unmeasured (no round judged rs:99; rank reads the indexed bar 0.80)\n"
        ));
    }

    /// A rule that mixes engines has no one bar to name.
    #[test]
    fn a_mixed_engine_rule_names_no_bar() {
        assert_eq!(unjudged_bar(&record("99", "py", "mixed")), "its tier bar");
        assert_eq!(
            unjudged_bar(&record("99", "py", "ORACLE")),
            "rank reads the proved bar 0.95"
        );
    }

    /// Every retired id the registry buries has a row in the embedded table.
    #[test]
    fn every_retired_id_has_its_burial() {
        for id in sightline_core::registry::RETIRED {
            let rows: Vec<&Burial> = burials().iter().filter(|b| &b.id == id).collect();
            assert!(!rows.is_empty(), "#{id} has no burial row");
        }
        let cut = |id: &str| {
            burials()
                .iter()
                .filter(|b| b.id == id)
                .map(|b| format!("{} {} {}", b.decision, b.why, b.evidence))
                .collect::<String>()
        };
        assert!(cut("25").contains("rename-delegation") && cut("25").contains("1 real / 8 fp"));
        assert!(cut("4").contains("1 real : 13 fp"));
        assert!(cut("28").contains("recall 0/21"));
    }
}
