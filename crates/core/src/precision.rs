//! Measured precision and recall, the port of `precision.py`.
//!
//! What JSON and SARIF findings, `explain` and `rank` read. The rows
//! themselves live in `data/precision.toml`, embedded here and parsed once. A
//! measurement is not a declaration: a rule record never holds one.

use std::sync::LazyLock;

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

/// Pseudo-observations the tier bar counts for.
pub const PRIOR_WEIGHT: u32 = 4;

/// P(real) a sample of n supports, on one scale with the unmeasured.
///
/// The posterior mean under a prior of `PRIOR_WEIGHT` observations at `bar`,
/// so n = 0 sits exactly at the bar, 5/5 (0.87 at a 0.7 bar) ranks below
/// 91/97 (0.93), and 4/5 (0.76) ranks above an unjudged rule.
#[must_use]
#[allow(
    clippy::suboptimal_flops,
    reason = "mul_add fuses the rounding, and this expression ranks every \
              finding: the audit is identical byte for byte at one thread and \
              at every core, with expected values pinned in tests"
)]
pub fn shrunk(tp: u32, n: u32, bar: f64) -> f64 {
    (f64::from(tp) + f64::from(PRIOR_WEIGHT) * bar) / (f64::from(n) + f64::from(PRIOR_WEIGHT))
}

/// One hand-judged seeded sample: `tp` of `n` true, judged at `bar`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Sample {
    pub tp: u32,
    pub n: u32,
    pub seed: i64,
    /// the judged population
    pub of: String,
    pub bar: Option<f64>,
}

impl Sample {
    /// The JSON body a finding reports, in the field order `precision.py`
    /// writes. Every writer sorts keys, so the order is the record's.
    #[must_use]
    pub fn json(&self) -> IndexMap<&'static str, Value> {
        let mut out = IndexMap::new();
        out.insert("tp", Value::from(self.tp));
        out.insert("n", Value::from(self.n));
        out.insert("seed", Value::from(self.seed));
        out.insert("of", Value::from(self.of.clone()));
        if let Some(bar) = self.bar {
            out.insert("bar", Value::from(bar));
        }
        out
    }
}

/// `covered` of `sites` blind judge sites a round mapped to this rule.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Recall {
    pub covered: u32,
    pub sites: u32,
    pub of: String,
}

#[derive(Deserialize)]
struct PrecisionRow {
    key: String,
    #[serde(flatten)]
    sample: Sample,
}

#[derive(Deserialize)]
struct RecallRow {
    key: String,
    #[serde(flatten)]
    recall: Recall,
}

#[derive(Deserialize)]
struct Tables {
    precision: Vec<PrecisionRow>,
    recall: Vec<RecallRow>,
}

/// The two tables of `data/precision.toml`, keyed by `key_of` (a
/// precision key is optionally suffixed `:<cause prefix>` where an arm was
/// judged alone), in file order: `rule_sample` walks it.
struct Keyed {
    precision: IndexMap<String, Sample>,
    recall: IndexMap<String, Recall>,
}

fn keyed() -> &'static Keyed {
    static KEYED: LazyLock<Keyed> = LazyLock::new(|| {
        // the file is embedded at compile time, so a malformed table is a
        // build the tests catch, never a state a run can reach
        #[allow(clippy::expect_used, reason = "compile-time embedded input")]
        let t: Tables = toml::from_str(include_str!("../../../data/precision.toml"))
            .expect("data/precision.toml is malformed");
        Keyed {
            precision: t.precision.into_iter().map(|r| (r.key, r.sample)).collect(),
            recall: t.recall.into_iter().map(|r| (r.key, r.recall)).collect(),
        }
    });
    &KEYED
}

/// `RULE_PRECISION`.
#[must_use]
pub fn rule_precision() -> &'static IndexMap<String, Sample> {
    &keyed().precision
}

/// `RULE_RECALL`: a floor, never a ceiling. Rules under 5 judged sites stay
/// out, so an absent key is unmeasured, not measured bad.
#[must_use]
pub fn rule_recall_table() -> &'static IndexMap<String, Recall> {
    &keyed().recall
}

/// The table key: a bare id for Python, `<lang>:<id>` for every other
/// language. A Rust reading keeps its sibling's id, so the id alone cannot
/// name a population.
#[must_use]
pub fn key_of(rule: &str, lang: &str) -> String {
    if lang == "py" {
        rule.to_string()
    } else {
        format!("{lang}:{rule}")
    }
}

/// A rule key's (language, id): the sort order of every table keyed by one.
/// Arm keys (`<key>:<cause>`) are not rule keys, so split those first.
#[must_use]
pub fn key_parts(key: &str) -> (&str, u32) {
    let (lang, rule) = key.rsplit_once(':').unwrap_or(("", key));
    let lang = if lang.is_empty() { "py" } else { lang };
    (lang, rule.parse().unwrap_or(0))
}

/// The rule's judged sample, an arm's own where the cause names one. One per
/// key: a fresh round replaces it, never sits beside it.
#[must_use]
pub fn rule_sample(rule: &str, cause: &str, lang: &str) -> Option<&'static Sample> {
    let key = key_of(rule, lang);
    let arm = format!("{key}:{cause}");
    let under_key = format!("{key}:");
    let table = rule_precision();
    let hit = table
        .keys()
        .find(|k| k.starts_with(&under_key) && arm.starts_with(&format!("{k}:")));
    table.get(hit.unwrap_or(&key))
}

/// Every sample a round judged for this rule, labelled by the cause prefix
/// an arm's row names and the empty label for the rule's own row. Table
/// order, so the rule's own leads its arms.
#[must_use]
pub fn rule_samples(rule: &str, lang: &str) -> Vec<(&'static str, &'static Sample)> {
    let key = key_of(rule, lang);
    let under = format!("{key}:");
    rule_precision()
        .iter()
        .filter(|(k, _)| **k == key || k.starts_with(&under))
        .map(|(k, s)| (k.strip_prefix(&under).unwrap_or(""), s))
        .collect()
}

/// The rule's measured recall, keyed like its precision (`key_of`).
#[must_use]
pub fn rule_recall(rule: &str, lang: &str) -> Option<&'static Recall> {
    rule_recall_table().get(&key_of(rule, lang))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toml_holds_every_row_of_the_python_tables() {
        // scratch/core-a/gen_precision.py wrote the file and counted the rows;
        // #31's row left with the rule, as every burial's row has
        assert_eq!(rule_precision().len(), 80);
        assert_eq!(rule_recall_table().len(), 40);
    }

    #[test]
    fn three_rows_round_trip() {
        let one = &rule_precision()["1"];
        assert_eq!(
            (one.tp, one.n, one.seed, one.bar),
            (66, 80, 202_608_284, Some(0.7))
        );
        assert_eq!(
            one.of,
            "g4: 5 never-seen clones + top-up, all findings judged, wave 2"
        );

        // the longest `of` in the table, and the only rows with a `%` in them
        let rs48 = &rule_precision()["rs:48"];
        assert_eq!((rs48.tp, rs48.n, rs48.seed), (12, 14, 202_608_292));
        assert!(rs48.of.contains("12/17 = 71 % pooled, so REPORT stands"));
        assert!(rs48.of.ends_with("1:1 under its 3:1 bar, and reverted"));

        let rec = &rule_recall_table()["rs:29"];
        assert_eq!((rec.covered, rec.sites), (40, 41));
        assert_eq!(
            rec.of,
            "rs2 close: 8 Rust judges' blind lists (5 clones, 3 applications), same-def match"
        );
    }

    #[test]
    fn every_key_names_a_rule_id() {
        for key in rule_precision().keys() {
            let id = key
                .split(':')
                .find(|p| p.chars().all(|c| c.is_ascii_digit()));
            assert!(id.is_some(), "{key} names no rule id");
        }
        for key in rule_recall_table().keys() {
            assert!(key_parts(key).1 > 0, "{key} names no rule id");
        }
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "these pin the ranking arithmetic exactly, which is what \
                  makes an audit identical byte for byte; an epsilon here \
                  would let the expression be rewritten without a test noticing"
    )]
    fn shrunk_is_the_posterior_mean() {
        assert_eq!(shrunk(0, 0, 0.7), 0.7);
        assert_eq!(shrunk(66, 80, 0.7), 0.819_047_619_047_619);
        assert_eq!(shrunk(44, 46, 0.95), 0.956);
    }

    #[test]
    fn key_of_namespaces_every_language_but_python() {
        assert_eq!(key_of("11", "py"), "11");
        assert_eq!(key_of("11", "rs"), "rs:11");
        assert_eq!(key_parts("11"), ("py", 11));
        assert_eq!(key_parts("rs:11"), ("rs", 11));
    }

    /// The roster and the SARIF rule pane read this: the rule's own row
    /// first, its arms after, and an arm alone where no round judged the
    /// whole rule.
    #[test]
    fn rule_samples_leads_with_the_rules_own_row() {
        let arms: Vec<&str> = rule_samples("32", "py").iter().map(|(a, _)| *a).collect();
        assert_eq!(
            arms,
            ["", "dead-import", "dead-param", "dead-symbol"],
            "the bare key leads its arms"
        );
        // #9 py was judged on one arm and never as a whole rule
        let nine = rule_samples("9", "py");
        assert_eq!(nine.len(), 1);
        assert_eq!(nine[0].0, "import-time-effect");
        // a Rust reading reads its own key, not its sibling's
        assert_eq!(rule_samples("9", "rs")[0].0, "");
        assert!(rule_samples("3", "py").is_empty());
    }

    #[test]
    fn rule_sample_walks_the_table_in_order_first_match_wins() {
        // an arm's own where the cause sits under it
        let arm = rule_sample("34", "commented-code:m.f:1", "py").unwrap();
        assert_eq!((arm.tp, arm.n), (19, 19));
        // the rule's own where it does not
        let rule = rule_sample("34", "noop-try:m.f:1", "py").unwrap();
        assert_eq!((rule.tp, rule.n), (20, 20));
        // no cause at all
        assert_eq!(rule_sample("34", "", "py").unwrap().n, 20);
        // no round judged #3
        assert!(rule_sample("3", "guard-implied", "py").is_none());
        // the Rust reading of a shared id keys its own population
        assert_eq!(rule_sample("11", "clone", "rs").unwrap().n, 300);
    }

    #[test]
    fn rule_recall_keys_like_its_precision() {
        assert_eq!(rule_recall("23", "py").unwrap().covered, 18);
        assert_eq!(rule_recall("23", "rs").unwrap().covered, 32);
        assert!(rule_recall("5", "py").is_none());
    }
}
