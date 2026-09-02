//! The finding model. The pipeline stages live beside it: `edits`,
//! `suppress`, `rank`.
//!
//! Engine is stamped by the evidence a prover produced; tier derives from
//! engine. Rules set neither.

use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{Map, Value, json};

use crate::precision::{Sample, rule_sample, shrunk};

/// Posix path under the repo root.
pub type Rel = Arc<str>;
/// Qualified name of a module or symbol.
pub type Qname = Arc<str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    Proved,
    Indexed,
    Heuristic,
}

impl Tier {
    // sightline-ok: 11 - an enum's match table is its own name
    pub fn value(self) -> &'static str {
        match self {
            Tier::Proved => "proved",
            Tier::Indexed => "indexed",
            Tier::Heuristic => "heuristic",
        }
    }

    /// `TIER_BAR`: the precision a tier is held to, and what `rank` assumes
    /// of a rule no round has judged. The one home for the bars
    /// `benchmarks.md` quotes.
    // sightline-ok: 11 - an enum's match table is its own name
    pub fn bar(self) -> f64 {
        match self {
            Tier::Proved => 0.95,
            Tier::Indexed => 0.8,
            Tier::Heuristic => 0.7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Engine {
    /// lift verified by a shadow oracle run
    Counterfactual,
    /// oracle diagnostic with annotation-grounded premises
    Oracle,
    /// whole-program: call graph, effects, closed world
    Wp,
    /// repo-wide symbol and ref indexes
    Idx,
    /// oracle on inferred-only premises
    OracleUngrounded,
    /// per-file heuristic
    Ast,
}

impl Engine {
    pub fn value(self) -> &'static str {
        match self {
            Engine::Counterfactual => "counterfactual",
            Engine::Oracle => "oracle",
            Engine::Wp => "wp",
            Engine::Idx => "idx",
            Engine::OracleUngrounded => "oracle-ungrounded",
            Engine::Ast => "ast",
        }
    }

    /// `TIER_BY_ENGINE`.
    pub fn tier(self) -> Tier {
        match self {
            Engine::Counterfactual | Engine::Oracle => Tier::Proved,
            Engine::Wp | Engine::Idx => Tier::Indexed,
            Engine::OracleUngrounded | Engine::Ast => Tier::Heuristic,
        }
    }
}

/// Constructed by prover and facts machinery, carried by findings.
#[derive(Debug, Clone, PartialEq)]
pub enum Evidence {
    Ast {
        detail: String,
    },
    Idx {
        detail: String,
    },
    Wp {
        premises: Vec<String>,
    },
    Oracle {
        /// the oracle's diagnostic rule id
        rule: String,
        /// enclosing signature explicitly annotated
        grounded: bool,
        message: String,
    },
    /// the `reportUnnecessary*` diagnostic that fired after the lift
    Counterfactual {
        receipt: String,
    },
}

impl Evidence {
    /// What an AST rule reports where the site is the whole evidence.
    pub fn ast() -> Evidence {
        Evidence::Ast {
            detail: String::new(),
        }
    }

    /// What an index rule reports where the site is the whole evidence.
    pub fn idx() -> Evidence {
        Evidence::Idx {
            detail: String::new(),
        }
    }

    pub fn engine(&self) -> Engine {
        match self {
            Evidence::Ast { .. } => Engine::Ast,
            Evidence::Idx { .. } => Engine::Idx,
            Evidence::Wp { .. } => Engine::Wp,
            Evidence::Oracle { grounded: true, .. } => Engine::Oracle,
            Evidence::Oracle {
                grounded: false, ..
            } => Engine::OracleUngrounded,
            Evidence::Counterfactual { .. } => Engine::Counterfactual,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Site {
    pub rel: Rel,
    /// 1-based
    pub line: u32,
    pub col: u32,
    /// qname of the enclosing symbol
    pub symbol: Qname,
}

/// Replace `[col_start, col_end)` on `line`; a pure insert when they are equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpanEdit {
    /// 1-based
    pub line: u32,
    pub col_start: u32,
    pub col_end: u32,
    pub text: String,
}

/// Mechanical patch payload carried only by counterfactually verified
/// findings; `patch::unified_diff` turns these into a diff.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fix {
    pub rel: Rel,
    pub edits: Vec<SpanEdit>,
    /// import statements the patch must add
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// rule id, as a string
    pub rule: &'static str,
    pub site: Site,
    pub message: String,
    /// canonical cause, namespaced per rule
    pub cause: String,
    pub evidence: Evidence,
    pub salience: f64,
    pub fix: Option<Fix>,
    /// the language of the rule that produced it, stamped by `run_rules`
    /// from the rule record and never by a rule. A Rust reading shares its
    /// sibling's id, so the id alone cannot key the judged sample.
    pub lang: &'static str,
}

impl Finding {
    pub fn engine(&self) -> Engine {
        self.evidence.engine()
    }

    pub fn tier(&self) -> Tier {
        self.engine().tier()
    }
}

/// Rules push in yield order.
#[derive(Debug, Default)]
pub struct Sink(pub Vec<Finding>);

impl Sink {
    pub fn new() -> Sink {
        Sink(Vec::new())
    }

    pub fn push(&mut self, f: Finding) {
        self.0.push(f);
    }
}

/// What the finding is expected to be worth: the rule's (or arm's) judged
/// sample shrunk toward its tier's bar, the bar itself where no round judged
/// one. One scale for both, so a thin perfect sample neither owns the head
/// nor sinks below the unmeasured.
pub fn p_real(f: &Finding) -> f64 {
    let bar = f.tier().bar();
    match rule_sample(f.rule, &f.cause, f.lang) {
        Some(s) => shrunk(s.tp, s.n, bar),
        None => bar,
    }
}

/// What a consumer may weight the finding by: the rule's (or arm's) own
/// judged sample, `None` where no round judged one. A tier is provenance,
/// never a measurement: its old samples were drawn over since-retired rules.
pub fn precision(f: &Finding) -> Option<IndexMap<&'static str, Value>> {
    rule_sample(f.rule, &f.cause, f.lang).map(Sample::json)
}

/// One JSON shape per `Evidence` variant.
fn evidence_json(evidence: &Evidence) -> Value {
    match evidence {
        Evidence::Wp { premises } => json!({ "premises": premises }),
        Evidence::Counterfactual { receipt } => json!({ "receipt": receipt }),
        Evidence::Ast { detail } | Evidence::Idx { detail } => json!({ "detail": detail }),
        Evidence::Oracle {
            rule,
            grounded,
            message,
        } => json!({ "rule": rule, "grounded": grounded, "message": message }),
    }
}

/// A finding's span edits, one array per edit.
fn edits_json(edits: &[SpanEdit]) -> Value {
    Value::Array(
        edits
            .iter()
            .map(|e| json!([e.line, e.col_start, e.col_end, e.text]))
            .collect(),
    )
}

/// One row of the `raw` dump layer; both stacks print it.
pub fn finding_json(f: &Finding) -> Value {
    let mut row = Map::new();
    row.insert("rule".into(), Value::from(f.rule));
    row.insert("lang".into(), Value::from(f.lang));
    row.insert("rel".into(), Value::from(&*f.site.rel));
    row.insert("line".into(), Value::from(f.site.line));
    row.insert("col".into(), Value::from(f.site.col));
    row.insert("symbol".into(), Value::from(&*f.site.symbol));
    row.insert("message".into(), Value::from(f.message.as_str()));
    row.insert("cause".into(), Value::from(f.cause.as_str()));
    row.insert("engine".into(), Value::from(f.engine().value()));
    row.insert("evidence".into(), evidence_json(&f.evidence));
    row.insert("salience".into(), Value::from(f.salience));
    row.insert(
        "fix".into(),
        match &f.fix {
            None => Value::Null,
            Some(fix) => json!({
                "rel": &*fix.rel,
                "edits": edits_json(&fix.edits),
                "imports": fix.imports,
            }),
        },
    );
    Value::Object(row)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn ast() -> Evidence {
        Evidence::ast()
    }

    pub(crate) fn idx() -> Evidence {
        Evidence::idx()
    }

    pub(crate) fn finding(rule: &'static str, evidence: Evidence) -> Finding {
        Finding {
            rule,
            site: Site {
                rel: "m.py".into(),
                line: 1,
                col: 0,
                symbol: "m.f".into(),
            },
            message: "msg".into(),
            cause: "c".into(),
            evidence,
            salience: 0.0,
            fix: None,
            lang: "py",
        }
    }

    #[test]
    fn tier_derives_from_evidence_engine() {
        let cases = [
            (ast(), Tier::Heuristic),
            (idx(), Tier::Indexed),
            (
                Evidence::Wp {
                    premises: vec!["p".into()],
                },
                Tier::Indexed,
            ),
            (
                Evidence::Oracle {
                    rule: "reportX".into(),
                    grounded: true,
                    message: String::new(),
                },
                Tier::Proved,
            ),
            (
                Evidence::Oracle {
                    rule: "reportX".into(),
                    grounded: false,
                    message: String::new(),
                },
                Tier::Heuristic,
            ),
            (
                Evidence::Counterfactual {
                    receipt: "r".into(),
                },
                Tier::Proved,
            ),
        ];
        for (evidence, tier) in cases {
            let f = finding("1", evidence);
            assert_eq!(f.tier(), tier);
            assert_eq!(f.tier(), f.engine().tier());
        }
    }

    #[test]
    fn the_bars_are_the_ones_benchmarks_quotes() {
        assert_eq!(Tier::Proved.bar(), 0.95);
        assert_eq!(Tier::Indexed.bar(), 0.8);
        assert_eq!(Tier::Heuristic.bar(), 0.7);
    }

    #[test]
    fn p_real_reads_the_arm_then_the_rule_then_the_bar() {
        // each value is `p_real` for the finding built beside it
        let arm = Finding {
            cause: "commented-code:m.f:1".into(),
            ..finding("34", ast())
        };
        assert_eq!(p_real(&arm), 0.947_826_086_956_521_8);
        assert_eq!(p_real(&finding("1", ast())), 0.819_047_619_047_619);
        assert_eq!(p_real(&finding("3", idx())), 0.8);
    }

    #[test]
    fn p_real_shrinks_toward_the_tier_bar_not_the_samples_own() {
        // #5 is 44/46 judged at bar 0.8, but its evidence is proved: the
        // prior is TIER_BAR[PROVED], (44 + 4 * 0.95) / 50
        let f = finding(
            "5",
            Evidence::Counterfactual {
                receipt: "r".into(),
            },
        );
        assert_eq!(p_real(&f), 0.956);
    }

    #[test]
    fn an_exact_arm_name_reads_the_rules_sample() {
        // `rule_sample` wants a cause *under* the arm, not the arm itself:
        // "11:clone" does not start with "11:clone:", so #11's own 232/256
        // answers.
        let f = Finding {
            cause: "clone".into(),
            ..finding("11", idx())
        };
        assert_eq!(p_real(&f), 0.904_615_384_615_384_6);
    }

    #[test]
    fn precision_is_none_where_no_round_judged_one() {
        assert!(precision(&finding("3", idx())).is_none());
        let p = precision(&finding("2", ast())).unwrap();
        assert_eq!(
            p.keys().copied().collect::<Vec<_>>(),
            ["tp", "n", "seed", "of", "bar"]
        );
        assert_eq!(p["tp"], 11);
    }

    #[test]
    fn a_rust_reading_keys_its_own_sample() {
        let rs = Finding {
            lang: "rs",
            ..finding("11", idx())
        };
        assert_eq!(p_real(&rs), 0.836_184_210_526_315_8);
        assert_eq!(p_real(&finding("11", idx())), 0.904_615_384_615_384_6);
    }
}
