//! Family B's #18, labeled phases narrated
//! inside one `fn`; Family C's #34, code that does nothing (commented out, or
//! a `match` that returns what it matched); and Family C's #39, one doc run
//! pasted onto several items. #18 and #34 ask what a body holds that no run
//! of it reads, #39 what a doc says that its item cannot answer for, so the
//! two families share this file.

use std::collections::BTreeMap;

use indexmap::IndexMap;

use sightline_core::clones::digest;
use sightline_core::findings::{Evidence, Finding, Qname, Sink, Site};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope, owner_list};
use sightline_rs_facts::model::{RsFacts, RsSymbol, is_fn_kind};
use sightline_rs_facts::nodes::{identity_matches, is_fn};
use sightline_rs_provers::{MIN_CODE_LINES, RsProvers};

use crate::Rule;
use crate::util::site;

/// #39's floor: a run shorter than a sentence is shared prose.
const MIN_DOC_CHARS: usize = 60;

/// The innermost `fn` whose span holds this line; `None` where the line sits
/// between items, which belong to the module and not to a body. Python's
/// `max` keeps the first of equal `lineno`s, so the walk takes a strictly
/// later start only.
fn enclosing_fn<'t>(facts: &'t RsFacts<'t>, module: &str, line: u32) -> Option<&'t Qname> {
    let mut best: Option<&RsSymbol<'t>> = None;
    for sym in facts.symbols_of(module) {
        if !is_fn_kind(sym.kind) || sym.lineno > line || line > sym.end_lineno {
            continue;
        }
        if best.is_none_or(|held| sym.lineno > held.lineno) {
            best = Some(sym);
        }
    }
    best.map(|sym| &sym.qname)
}

pub const RULE_18: Rule = Rule {
    record: RuleRecord {
        id: "18",
        slug: "section-comments",
        family: "surface",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: ">=2 labeled phases narrated inside one function",
        goal: "A numbered phase comment is a function boundary spelled in prose (Smith; Van \
               Eerd V5).",
        lang: "rs",
        scope: Scope::File,
        complement: "",
    },
    run: rule_18,
};

/// A phase is a comment run heading a section of code, so a numbered
/// rationale written as one block is one label however many lines it numbers;
/// the anchor is the first label of the body.
fn rule_18<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    for module in facts.modules.values() {
        let mut per_fn: IndexMap<&Qname, Vec<u32>> = IndexMap::new();
        for block in &provers.comment_blocks()[&module.qname] {
            if !block.label {
                continue;
            }
            if let Some(owner) = enclosing_fn(facts, &module.qname, block.start) {
                per_fn.entry(owner).or_default().push(block.start);
            }
        }
        let mut ordered: Vec<(&Qname, Vec<u32>)> = per_fn.into_iter().collect();
        ordered.sort_by(|a, b| a.0.cmp(b.0));
        for (owner, lines) in ordered {
            if lines.len() < 2 {
                continue;
            }
            out.push(Finding {
                rule: "18",
                site: Site {
                    rel: module.rel.clone(),
                    line: lines[0],
                    col: 0,
                    symbol: owner.clone(),
                },
                message: format!(
                    "{owner} narrates {} labeled phases - each is a function boundary spelled \
                     in prose",
                    lines.len()
                ),
                cause: format!("sections:{owner}"),
                evidence: Evidence::ast(),
                salience: lines.len() as f64,
                fix: None,
                lang: "rs",
            });
        }
    }
}

pub const RULE_34: Rule = Rule {
    record: RuleRecord {
        id: "34",
        slug: "noop-code",
        family: "context",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "commented-out code: a non-doc comment run of >=3 lines that Rust parses; a \
                  `match` every arm of which re-returns what it matched",
        goal: "Delete dead weight: git remembers old code, a disabled block left in place \
               reads as intent no one can act on, and a `match` that rewrites nothing is a \
               shape every reader still has to check.",
        lang: "rs",
        scope: Scope::File,
        complement: "",
    },
    run: rule_34,
};

/// A run the grammar reads as items or statements is code someone commented
/// out; prose about the code does not parse. An identity `match` is the same
/// defect written in Rust: clippy's `needless_match` reads it with types and
/// reports none of the sites here.
fn rule_34<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    for module in facts.modules.values() {
        for block in &provers.comment_blocks()[&module.qname] {
            if block.lines.len() < MIN_CODE_LINES || !block.code() {
                continue;
            }
            out.push(Finding {
                rule: "34",
                site: Site {
                    rel: module.rel.clone(),
                    line: block.start,
                    col: 0,
                    symbol: module.qname.clone(),
                },
                message: format!(
                    "{} commented-out code lines - delete them; git remembers",
                    block.lines.len()
                ),
                cause: format!("commented-code:{}:{}", module.qname, block.start),
                evidence: Evidence::ast(),
                salience: block.lines.len() as f64,
                fix: None,
                lang: "rs",
            });
        }
    }
    let mut symbols: Vec<&RsSymbol<'t>> = facts.symbols.values().collect();
    symbols.sort_by(|a, b| a.qname.cmp(&b.qname));
    for sym in symbols {
        if !is_fn(sym) {
            continue;
        }
        let src = facts.modules[&sym.module].bytes;
        for node in identity_matches(sym.node, src) {
            let lines = node.end_position().row - node.start_position().row + 1;
            out.push(Finding {
                rule: "34",
                site: site(facts, sym, node),
                message: format!(
                    "this `match` returns what it matched in every arm over {lines} lines - it \
                     decides nothing"
                ),
                cause: format!("noop-match:{}", sym.qname),
                evidence: Evidence::ast(),
                salience: lines as f64,
                fix: None,
                lang: "rs",
            });
        }
    }
}

// --- #39 comment discipline, the copied-doc arm -------------------------------

/// (the run's prose, its markers and indentation off; the lines that hold
/// any). Two items hold the same doc however deep each one sits.
fn doc_body(run: &[String]) -> (String, usize) {
    let mut lines: Vec<&str> = run
        .iter()
        .flat_map(|spelled| pytext::splitlines(spelled))
        .map(|raw| pytext::strip(pytext::lstrip_chars(pytext::strip(raw), "/*")))
        .collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let filled = lines.iter().filter(|line| !line.is_empty()).count();
    (pytext::strip(&lines.join("\n")).to_string(), filled)
}

/// The doc runs written on more than one item, keyed by their prose. A
/// one-line run is shared prose (`/// The parsed document.`), and so is a
/// short one, so both floors are asked before a run is keyed at all.
fn copied_docs<'t>(
    facts: &'t RsFacts<'t>,
    provers: &RsProvers<'t>,
) -> BTreeMap<String, Vec<&'t RsSymbol<'t>>> {
    let mut symbols: Vec<&RsSymbol<'t>> = facts.symbols.values().collect();
    symbols.sort_by(|a, b| a.qname.cmp(&b.qname));
    let mut groups: BTreeMap<String, Vec<&RsSymbol<'t>>> = BTreeMap::new();
    for sym in symbols {
        if sym.is_test {
            continue;
        }
        let run = provers.doc_above(&facts.modules[&sym.module], sym.lineno);
        let (body, filled) = doc_body(&run);
        if filled >= 2 && body.chars().count() > MIN_DOC_CHARS {
            groups.entry(body).or_default().push(sym);
        }
    }
    groups.retain(|_, syms| syms.len() > 1);
    groups
}

/// Do these items all answer to one name? Then the run is one operation
/// documented wherever Rust makes the author write it again: a builder and
/// the value it builds, a wrapper and its inner, both sides of a split, a
/// trait's contract on each impl. The doc holds for every one of them.
fn one_operation(syms: &[&RsSymbol<'_>]) -> bool {
    syms.windows(2).all(|pair| pair[0].name == pair[1].name)
}

pub const RULE_39: Rule = Rule {
    record: RuleRecord {
        id: "39",
        slug: "comment-discipline",
        family: "context",
        engine_class: "IDX",
        posture: Posture::Report,
        meaning: "one multi-line doc run written word for word on differently named items",
        goal: "Comments hold only what the code cannot: a doc that fits two operations \
               describes neither, and the reader who trusts it on the second one is reading \
               the first one's contract. Four judged rows of six are no sample to gate on, so \
               this reports until a larger round measures it.",
        lang: "rs",
        scope: Scope::Repo,
        complement: "clippy reads no doc for a copy; `doc_markdown` only spells its prose",
    },
    run: rule_39,
};

/// One `///` run pasted onto items that do different things. The anchor is
/// the first of the group, since the run is one decision however many homes
/// it has.
fn rule_39<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    for (body, syms) in copied_docs(facts, provers) {
        if one_operation(&syms) {
            continue;
        }
        let head = syms[0];
        let others: Vec<&str> = syms[1..].iter().map(|sym| &*sym.qname).collect();
        out.push(Finding {
            rule: "39",
            site: Site {
                rel: facts.modules[&head.module].rel.clone(),
                line: head.lineno,
                col: 0,
                symbol: head.qname.clone(),
            },
            message: format!(
                "the doc on {} is word for word the doc on {}",
                head.qname,
                owner_list(&others)
            ),
            cause: format!("comment-discipline:doc-copied:{}", digest(&body)),
            evidence: Evidence::idx(),
            salience: syms.len() as f64,
            fix: None,
            lang: "rs",
        });
    }
}
