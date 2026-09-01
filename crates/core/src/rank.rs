//! The total order (port of `findings.py` `rank`).
//!
//! Measured P(real) first (`p_real`: the rule's or arm's own judged sample,
//! its tier's bar until a round judges one), then the finding's integer rank
//! inside its own rule (0 = that rule's strongest; salience is a rule's own
//! scale and means nothing across rules), then the complexity prior of the
//! enclosing scope, then location, rule id last.
//!
//! P first because an agent reading the head wants the highest expected
//! value there, and a tier is provenance, not a measurement: a heuristic
//! rule judged 17/17 outranks a proved one judged 12/15. Within-rule
//! position second keeps equal-P rules interleaved: position, not fraction,
//! so a 481-finding rule gets no denser a ladder than a 4-finding one. The
//! rest makes the order total.

use indexmap::IndexMap;

use crate::findings::{Finding, p_real};
use crate::lang::FactsView;

/// The negation a `-key` ports as. Python compares `-0.0 == 0.0` and
/// `total_cmp` does not, so a zero stays positive.
fn neg(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { -x }
}

/// The outer key, one per finding, in the order the tuple compares.
struct Key {
    neg_p: f64,
    within: usize,
    neg_cc: i64,
    id: u32,
}

pub fn rank(findings: Vec<Finding>, facts: &dyn FactsView) -> Vec<Finding> {
    let cc: Vec<i64> = findings
        .iter()
        .map(|f| -i64::from(facts.cc_prior(&f.site.symbol)))
        .collect();

    let mut by_rule: IndexMap<&str, Vec<usize>> = IndexMap::new();
    for (i, f) in findings.iter().enumerate() {
        by_rule.entry(f.rule).or_default().push(i);
    }
    let mut within = vec![0usize; findings.len()];
    for group in by_rule.values_mut() {
        group.sort_by(|&a, &b| {
            let (fa, fb) = (&findings[a], &findings[b]);
            neg(fa.salience)
                .total_cmp(&neg(fb.salience))
                .then_with(|| cc[a].cmp(&cc[b]))
                .then_with(|| fa.site.rel.cmp(&fb.site.rel))
                .then_with(|| fa.site.line.cmp(&fb.site.line))
                .then_with(|| fa.site.col.cmp(&fb.site.col))
                .then_with(|| fa.cause.cmp(&fb.cause))
        });
        for (pos, &i) in group.iter().enumerate() {
            within[i] = pos;
        }
    }

    let keys: Vec<Key> = findings
        .iter()
        .enumerate()
        .map(|(i, f)| Key {
            neg_p: neg(p_real(f)),
            within: within[i],
            neg_cc: cc[i],
            id: f.rule.parse().unwrap_or(0),
        })
        .collect();

    let mut order: Vec<usize> = (0..findings.len()).collect();
    order.sort_by(|&a, &b| {
        let (ka, kb) = (&keys[a], &keys[b]);
        ka.neg_p
            .total_cmp(&kb.neg_p)
            .then_with(|| ka.within.cmp(&kb.within))
            .then_with(|| ka.neg_cc.cmp(&kb.neg_cc))
            .then_with(|| findings[a].site.rel.cmp(&findings[b].site.rel))
            .then_with(|| findings[a].site.line.cmp(&findings[b].site.line))
            .then_with(|| findings[a].site.col.cmp(&findings[b].site.col))
            .then_with(|| ka.id.cmp(&kb.id))
    });

    let mut slots: Vec<Option<Finding>> = findings.into_iter().map(Some).collect();
    order
        .into_iter()
        .map(|i| slots[i].take().expect("one slot per index"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::findings::tests::{ast, finding, idx};
    use crate::findings::{Evidence, Finding, Site};
    use crate::lang::{Neutral, Stack};
    use crate::suppress::suppress;
    use crate::testing::{P, SyntheticStack};
    use std::collections::HashMap;

    fn oracle() -> Evidence {
        Evidence::Oracle {
            rule: "reportX".into(),
            grounded: true,
            message: String::new(),
        }
    }

    fn at(
        rule: &'static str,
        rel: &str,
        line: u32,
        col: u32,
        symbol: &str,
        cause: &str,
        evidence: Evidence,
    ) -> Finding {
        Finding {
            site: Site {
                rel: rel.into(),
                line,
                col,
                symbol: symbol.into(),
            },
            cause: cause.into(),
            ..finding(rule, evidence)
        }
    }

    /// The priors REF's facts report for the probe's mini repo: `m.hairy`
    /// 6, every other symbol 0 (scratch/core-a/probe_order.py).
    fn stack() -> SyntheticStack {
        let mut stack = SyntheticStack::new(&P, &[("m.p", "x\n"), ("b.p", "y\n"), ("z.rs", "w\n")]);
        let cc = &mut stack.neutral_mut().cc;
        cc.clear();
        cc.insert("m.hairy".into(), 6);
        stack
    }

    fn rules_of(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(|f| f.rule).collect()
    }

    #[test]
    fn the_order_is_the_reference_tools() {
        // scratch/core-a/probe_order.py: the same ten findings through REF's
        // `sightline.findings.rank`, printed in the order it returned them
        let stack = stack();
        let neutral: &Neutral = stack.neutral();
        let input = vec![
            Finding {
                salience: 1.0,
                ..at("1", "m.p", 5, 0, "m.hairy", "weak", ast())
            },
            at("34", "m.p", 4, 2, "m.plain", "commented-code:m.f:1", ast()),
            at("2", "b.p", 1, 0, "b.g", "redundant", oracle()),
            at("3", "m.p", 1, 0, "m.plain", "guard-implied", idx()),
            Finding {
                salience: 2.0,
                ..at("11", "m.p", 3, 4, "m.hairy", "clone", idx())
            },
            Finding {
                salience: 9.0,
                ..at("11", "b.p", 2, 0, "b.g", "clone", idx())
            },
            Finding {
                salience: 2.0,
                ..at("11", "m.p", 3, 4, "m.plain", "clone-block", idx())
            },
            Finding {
                salience: 5.0,
                lang: "rs",
                ..at("11", "z.rs", 1, 0, "z::main", "clone", idx())
            },
            at(
                "60",
                "m.p",
                1,
                0,
                "m.plain",
                "dead-by-graph",
                Evidence::Wp {
                    premises: vec!["p".into()],
                },
            ),
            at(
                "5",
                "m.p",
                8,
                0,
                "m.hairy",
                "lift",
                Evidence::Counterfactual {
                    receipt: "r".into(),
                },
            ),
        ];
        let ranked = rank(input, neutral);
        let seen: Vec<(&str, &str, &str, u32, &str)> = ranked
            .iter()
            .map(|f| (f.rule, f.lang, &*f.site.rel, f.site.line, &*f.cause))
            .collect();
        assert_eq!(
            seen,
            [
                ("2", "py", "b.p", 1, "redundant"),
                ("60", "py", "m.p", 1, "dead-by-graph"),
                ("5", "py", "m.p", 8, "lift"),
                ("34", "py", "m.p", 4, "commented-code:m.f:1"),
                ("11", "py", "b.p", 2, "clone"),
                ("11", "py", "m.p", 3, "clone"),
                ("11", "py", "m.p", 3, "clone-block"),
                ("11", "rs", "z.rs", 1, "clone"),
                ("1", "py", "m.p", 5, "weak"),
                ("3", "py", "m.p", 1, "guard-implied"),
            ]
        );
    }

    #[test]
    fn the_head_is_round_robin_over_the_rules() {
        // at equal P the key is the within-rule integer rank, so a rule with
        // hundreds of findings cannot own the head the way a rank fraction
        // let it. #3 and #13 are both unjudged and indexed: one P, so the
        // two interleave.
        let stack = SyntheticStack::new(&P, &[("m.p", "x\n")]);
        let mut input: Vec<Finding> = (0..20)
            .map(|i| Finding {
                salience: -f64::from(i),
                ..at("3", "m.p", i + 1, 0, "m.f", "c", idx())
            })
            .collect();
        input.push(at("13", "m.p", 1, 0, "m.f", "d", idx()));
        assert_eq!(
            rules_of(&rank(input, stack.neutral()))[..3],
            ["3", "13", "3"]
        );
    }

    #[test]
    fn measured_precision_leads_and_the_tier_does_not() {
        // #34's commented-code arm is 19/19 heuristic (0.95 shrunk); #56 is
        // 18/22 indexed (0.82): the measured sample leads, the tier does not
        let stack = SyntheticStack::new(&P, &[("m.p", "x\n")]);
        let heuristic = at("34", "m.p", 5, 0, "m.f", "commented-code:m.f:1", ast());
        let indexed = at("56", "m.p", 9, 0, "m.f", "test-only", idx());
        assert_eq!(
            rules_of(&rank(vec![indexed, heuristic], stack.neutral())),
            ["34", "56"]
        );
    }

    #[test]
    fn an_unjudged_rule_falls_back_to_the_tier_bar() {
        // no round judged #3, so it ranks at TIER_BAR[INDEXED]: below a
        // measured rule above that bar (#34, 19/19) and above one below it
        // (#59, 9/11 at 0.7 -> 0.79)
        let stack = SyntheticStack::new(&P, &[("m.p", "x\n")]);
        let above = at("34", "m.p", 1, 0, "m.f", "commented-code:m.f:1", ast());
        let unjudged = at("3", "m.p", 1, 0, "m.f", "guard-implied", idx());
        let below = at("59", "m.p", 1, 0, "m.f", "cost-docstring", ast());
        let ranked = rank(vec![below, unjudged, above], stack.neutral());
        assert_eq!(rules_of(&ranked), ["34", "3", "59"]);
    }

    #[test]
    fn the_complexity_prior_breaks_a_tie() {
        let stack = stack();
        let plain = at("13", "m.p", 2, 0, "m.plain", "a", idx());
        let hairy = at("13", "m.p", 7, 0, "m.hairy", "b", idx());
        let ranked = rank(vec![plain, hairy], stack.neutral());
        assert_eq!(
            ranked.iter().map(|f| &*f.cause).collect::<Vec<_>>(),
            ["b", "a"]
        );
    }

    #[test]
    fn the_pipeline_order_is_ruled_and_deterministic() {
        let stack = SyntheticStack::new(&P, &[("m.p", "a\nb\nc\nd\ne\nf\ng\nh\ni\n")]);
        let fs = vec![
            at("1", "m.p", 9, 0, "m.f", "c", ast()),
            at("3", "m.p", 6, 0, "m.f", "k", ast()),
            at("2", "m.p", 6, 0, "m.f", "k", oracle()),
        ];
        let run = |fs: Vec<Finding>| {
            let kept = suppress(fs, stack.neutral(), &HashMap::new()).0;
            rules_of(&rank(kept, stack.neutral()))
        };
        let forward = run(fs.clone());
        let mut reversed = fs;
        reversed.reverse();
        assert_eq!(forward, run(reversed));
        // shrunk: #2 11/11 at .95 -> .99, #1 66/80 at .7 -> .82, and an
        // unjudged heuristic #3 at its bar .7
        assert_eq!(forward, ["2", "1", "3"]);
    }
}
