//! Rs #11 (structural-clones), #20
//! (repeated-lambda), #21 (distributed-invariant), #23
//! (cognitive-complexity), #37 (speculative-generality), #48
//! (fold-candidate).
//!
//! The mining, the scorer and the digests are `RsProvers`, so this module
//! only asks and reports. The three rules with helpers of their own live in
//! `surface/`; their records stay here beside the other three.

mod fold;
mod generality;
mod invariant;

use std::collections::{BTreeMap, HashSet};

use sightline_core::findings::{Evidence, Finding, Sink};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope, owner_list};
use sightline_rs_facts::Node;
use sightline_rs_facts::model::{RsFacts, RsSymbol, is_fn_kind, text};
use sightline_rs_provers::{RsClosure, RsProvers};

use crate::Rule;
use crate::util::site;

/// A node's source on one line, elided: what the message quotes.
fn spelling(node: Node<'_>, src: &[u8]) -> String {
    const LIMIT: usize = 56;
    let out = pytext::split(&text(node, src)).join(" ");
    if out.chars().count() <= LIMIT {
        return out;
    }
    let head: String = out.chars().take(LIMIT - 3).collect();
    format!("{head}...")
}

pub const RULE_11: Rule = Rule {
    record: RuleRecord {
        id: "11",
        slug: "structural-clones",
        family: "surface",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "blind-digest T2 clone groups over `fn` bodies and repeated >=5-statement \
                  blocks, ratcheted",
        goal: "One home per fact: every extra copy is a place the next fix forgets (GitClear 8x \
               duplication; Van Eerd's migration grace).",
        lang: "rs",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_11,
};

/// Each copy as `qname L<line>`, so a reader opens the other copies without
/// a search.
fn owners<'a>(members: impl Iterator<Item = (&'a RsSymbol<'a>, Node<'a>)>) -> Vec<String> {
    let mut out: Vec<String> = members
        .map(|(sym, node)| format!("{} L{}", sym.qname, node.start_position().row + 1))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Whole-body T2 clones and, at sub-function granularity, the maximal
/// repeated statement runs the neutral mining finds. Test members count
/// toward a group but never carry a finding, and a window of one repeated
/// statement shape is a table, not a fact.
fn rule_11(facts: &RsFacts<'_>, provers: &RsProvers<'_>, out: &mut Sink) {
    let mut groups: BTreeMap<&str, Vec<&RsSymbol<'_>>> = BTreeMap::new();
    for (qname, key) in provers.function_digests() {
        groups
            .entry(key.as_str())
            .or_default()
            .push(&facts.symbols[qname]);
    }
    for (key, members) in &groups {
        if members.len() < 2 {
            continue;
        }
        let listed = owner_list(&owners(members.iter().map(|sym| (*sym, sym.node))));
        for sym in members {
            if sym.is_test {
                continue;
            }
            out.push(Finding {
                rule: "11",
                site: site(facts, sym, sym.node),
                message: format!("structural clone x{}: {listed}", members.len()),
                cause: format!("clone:{key}"),
                evidence: Evidence::Idx {
                    detail: (*key).to_string(),
                },
                salience: members.len() as f64,
                fix: None,
                lang: "rs",
            });
        }
    }
    for group in provers.block_clones() {
        let shapes: HashSet<&String> = group.shapes.iter().collect();
        if shapes.len() == 1 {
            continue;
        }
        let (count, stmts) = (group.members.len(), group.shapes.len());
        let listed = owner_list(&owners(
            group.members.iter().map(|(sym, nodes)| (*sym, nodes[0])),
        ));
        for (sym, nodes) in &group.members {
            if sym.is_test {
                continue;
            }
            out.push(Finding {
                rule: "11",
                site: site(facts, sym, nodes[0]),
                message: format!("structural block clone x{count} ({stmts} stmts): {listed}"),
                cause: format!("clone-block:{}", group.key),
                evidence: Evidence::Idx {
                    detail: group.key.clone(),
                },
                salience: (count * stmts) as f64,
                fix: None,
                lang: "rs",
            });
        }
    }
}

/// the family's bar for "a pattern": #11 and #21 count 3
const CLOSURE_COPIES: usize = 3;
/// under it the body is a field read, not a predicate
const CLOSURE_NODES: usize = 5;

pub const RULE_20: Rule = Rule {
    record: RuleRecord {
        id: "20",
        slug: "repeated-lambda",
        family: "surface",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "same nontrivial closure body >=3 times in a module",
        goal: "Interface symmetry (Sean Parent): a predicate written three times drifts; name \
               it once.",
        lang: "rs",
        scope: Scope::File,
        complement: "",
    },
    run: rule_20,
};

/// A second copy is a coincidence a reader still holds in one glance; the
/// third is the pattern. Copies key on their content with the closure's own
/// parameters renamed by position, so two comparators sorting on different
/// fields stay apart. A body that only forwards names to a call decides
/// nothing and has a name already. A test copy counts toward the pattern the
/// way #11's does, and never anchors the finding.
fn rule_20(facts: &RsFacts<'_>, provers: &RsProvers<'_>, out: &mut Sink) {
    for (qname, module) in &facts.modules {
        let mut by_key: BTreeMap<&str, Vec<(&RsSymbol<'_>, &RsClosure<'_>)>> = BTreeMap::new();
        for sym in facts.symbols_of(qname) {
            if !is_fn_kind(sym.kind) {
                continue;
            }
            for closure in &provers.body(&sym.qname).closures {
                if closure.size >= CLOSURE_NODES && !closure.forwards {
                    by_key
                        .entry(closure.key.as_str())
                        .or_default()
                        .push((sym, closure));
                }
            }
        }
        for (key, copies) in &by_key {
            let prod: Vec<(&RsSymbol<'_>, &RsClosure<'_>)> = copies
                .iter()
                .filter(|(sym, _)| !sym.is_test)
                .copied()
                .collect();
            if copies.len() < CLOSURE_COPIES {
                continue;
            }
            // `min` keeps the first of equals: the earliest (row, byte col)
            let Some(&(sym, first)) = prod.iter().min_by_key(|(_, c)| c.node.start_position())
            else {
                continue;
            };
            out.push(Finding {
                rule: "20",
                site: site(facts, sym, first.node),
                message: format!(
                    "closure `{}` appears {}x in {} - name it once",
                    spelling(first.node, first.src),
                    copies.len(),
                    module.qname
                ),
                cause: format!("closure:{}:{}", module.qname, &key[..8]),
                evidence: Evidence::ast(),
                salience: copies.len() as f64,
                fix: None,
                lang: "rs",
            });
        }
    }
}

pub const RULE_21: Rule = Rule {
    record: RuleRecord {
        id: "21",
        slug: "distributed-invariant",
        family: "surface",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "a `match` arm whose whole body is `unreachable!`/`panic!` on a variant of an \
                  enum this repo declares",
        goal: "Encapsulate the invariant (Smith's CaseInsensitiveMap): a rule enforced at every \
               call site belongs in the type. An arm saying a variant never arrives here says \
               the scrutinee's type is wider than the code it feeds, and every other reader of \
               that enum holds the same rule by hand.",
        lang: "rs",
        scope: Scope::Repo,
        complement: "clippy `unreachable` (restriction, off by default) bans the macro wherever \
                     it is written, not the arm that stands for a type",
    },
    run: invariant::rule_21,
};

pub const RULE_23: Rule = Rule {
    record: RuleRecord {
        id: "23",
        slug: "cognitive-complexity",
        family: "surface",
        engine_class: "AST",
        posture: Posture::Report,
        meaning: "cognitive complexity >= 15; also the ranking prior",
        goal: "Complexity predicts comprehension time (meta-analysis); REPORT-tier only - a \
               gate here would push authors to extract helpers to dodge it.",
        lang: "rs",
        scope: Scope::File,
        complement: "",
    },
    run: rule_23,
};

fn rule_23(facts: &RsFacts<'_>, provers: &RsProvers<'_>, out: &mut Sink) {
    for sym in facts.symbols.values() {
        if !is_fn_kind(sym.kind) {
            continue;
        }
        let cc = provers.complexity(&sym.qname);
        let threshold = facts.config.complexity_threshold;
        if cc < threshold {
            continue;
        }
        out.push(Finding {
            rule: "23",
            site: site(facts, sym, sym.node),
            message: format!(
                "{} has cognitive complexity {cc} (threshold {threshold})",
                sym.qname
            ),
            cause: format!("cognitive-complexity:{}", sym.qname),
            evidence: Evidence::ast(),
            salience: f64::from(cc),
            fix: None,
            lang: "rs",
        });
    }
}

pub const RULE_37: Rule = Rule {
    record: RuleRecord {
        id: "37",
        slug: "speculative-generality",
        family: "surface",
        engine_class: "IDX",
        posture: Posture::Report,
        meaning: "a non-public trait with exactly one `impl ... for` in the repo, on a type the \
                  repo owns; a type parameter every use names the same",
        goal: "Flexibility no one exercises is debt (inverse of #14): an interface with one \
               implementation, a parameter with one argument. Judged 4 real / 0 fp over three \
               rounds, four rows and no sample, so gating it would block on nothing measured: \
               REPORT until one prices it.",
        lang: "rs",
        scope: Scope::Repo,
        complement: "",
    },
    run: generality::rule_37,
};

pub const RULE_48: Rule = Rule {
    record: RuleRecord {
        id: "48",
        slug: "fold-candidate",
        family: "surface",
        engine_class: "WP",
        posture: Posture::Report,
        meaning: "private `fn` with one prod call edge and no other reference, body on one line \
                  and nothing but the line to it: fold it into its caller",
        goal: "A name is a promise of reuse (Ousterhout's shallow module): a helper one reader \
               calls once costs a hop and a signature for nothing. The round that judged the \
               reading is the seed its exemptions were written against, so gating it would \
               block on a precision of its own making: REPORT until a later round measures \
               it.",
        lang: "rs",
        scope: Scope::Repo,
        complement: "",
    },
    run: fold::rule_48,
};
