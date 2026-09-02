//! Measured precision and recall.
//!
//! What JSON and SARIF findings, `explain` and `rank` read. The rows
//! themselves live in `crates/core/data/precision.toml`, embedded here and parsed once. A
//! measurement is not a declaration: a rule record never holds one.

use std::sync::LazyLock;

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

/// Pseudo-observations the tier bar counts for.
pub const PRIOR_WEIGHT: u32 = 4;

/// The posterior over P(real) a sample of `tp` in `n` supports: `(mean,
/// sd)` under a Beta prior of `PRIOR_WEIGHT` observations at `bar`. n = 0
/// sits at the bar with the prior's own spread.
#[must_use]
#[allow(
    clippy::suboptimal_flops,
    reason = "mul_add fuses the rounding, and this expression ranks every \
              finding: the audit is identical byte for byte at one thread and \
              at every core, with expected values pinned in tests"
)]
pub fn posterior(tp: u32, n: u32, bar: f64) -> (f64, f64) {
    let a = f64::from(tp) + f64::from(PRIOR_WEIGHT) * bar;
    let b = f64::from(n - tp) + f64::from(PRIOR_WEIGHT) * (1.0 - bar);
    let mean = a / (a + b);
    (mean, (mean * (1.0 - mean) / (a + b + 1.0)).sqrt())
}

/// What `rank` sorts on: the posterior mean less one standard deviation.
///
/// A lower bound a sample of five cannot clear the way two hundred can. 5/5
/// at a 0.8 bar scores 0.82, 232/256 scores 0.89, and 0/0 scores the bar
/// less the prior's spread.
#[must_use]
pub fn score(tp: u32, n: u32, bar: f64) -> f64 {
    let (mean, sd) = posterior(tp, n, bar);
    mean - sd
}

/// The 95% interval of the posterior, clamped to the unit: what `explain`
/// and the catalog print beside a fraction.
#[must_use]
pub fn interval(tp: u32, n: u32, bar: f64) -> (f64, f64) {
    let (mean, sd) = posterior(tp, n, bar);
    let half = 1.96 * sd;
    ((mean - half).max(0.0), (mean + half).min(1.0))
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
    /// The bar this sample was judged against, the heuristic one where the
    /// row names none.
    #[must_use]
    pub fn bar(&self) -> f64 {
        self.bar.unwrap_or(0.7)
    }

    /// `lo-hi` at two decimals: how the roster and the catalog spell the
    /// interval beside `tp/n`.
    #[must_use]
    pub fn spelled_interval(&self) -> String {
        let (lo, hi) = interval(self.tp, self.n, self.bar());
        format!("{lo:.2}-{hi:.2}")
    }

    /// The JSON body a finding reports, in this record's field order.
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
        let (lo, hi) = interval(self.tp, self.n, self.bar());
        out.insert("interval", Value::from(vec![lo, hi]));
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

/// The two tables of `crates/core/data/precision.toml`, keyed by `key_of` (a
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
        let t: Tables = toml::from_str(include_str!("../data/precision.toml"))
            .expect("crates/core/data/precision.toml is malformed");
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
        assert_eq!(one.of, "judged on 5 held-out Python repositories, round 4");

        // a population with a second clause, read whole
        let rs48 = &rule_precision()["rs:48"];
        assert_eq!((rs48.tp, rs48.n, rs48.seed), (12, 14, 202_608_292));
        assert!(
            rs48.of
                .ends_with("read 0 real of 3, so the rule stays REPORT")
        );

        let rec = &rule_recall_table()["rs:29"];
        assert_eq!((rec.covered, rec.sites), (40, 41));
        assert!(
            rec.of
                .starts_with("the blind judges' lists on 5 held-out Rust libraries")
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
    fn the_score_is_the_posterior_mean_less_one_sd() {
        // no sample: the bar, less the prior's own spread
        let (mean, sd) = posterior(0, 0, 0.7);
        assert_eq!(mean, 0.7);
        assert_eq!(score(0, 0, 0.7), mean - sd);
        assert!(sd > 0.20 && sd < 0.21, "{sd}");
        // the size of the sample is what the sd prices: 5/5 sits below
        // 232/256 at one bar, and a thin sample below the bar it beat
        assert!(score(5, 5, 0.8) < score(232, 256, 0.8));
        assert!(score(5, 5, 0.8) > 0.82 && score(5, 5, 0.8) < 0.83);
        assert!(score(232, 256, 0.8) > 0.88 && score(232, 256, 0.8) < 0.89);
        assert_eq!(posterior(66, 80, 0.7).0, 0.819_047_619_047_619);
        assert_eq!(posterior(44, 46, 0.95).0, 0.956);
    }

    #[test]
    fn the_interval_is_clamped_to_the_unit() {
        let (lo, hi) = interval(5, 5, 0.8);
        assert!(lo > 0.73 && lo < 0.74, "{lo}");
        assert_eq!(hi, 1.0);
        assert_eq!(rule_precision()["50"].spelled_interval(), "0.91-0.96");
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
