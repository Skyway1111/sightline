//! Port of `rs/rules/context.py`: what a module costs the reader who must
//! load it (#27, #29) and what an entry point spends without saying so
//! (#59). Thresholds are the Python siblings'.

use std::sync::LazyLock;

use sightline_core::catalog::{ClassSet, MUTATES, SPAWNS, SPENDS};
use sightline_core::findings::{Evidence, Finding, Qname, Sink, Site};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_rs_facts::model::{RsFacts, RsModule, RsSymbol, is_fn_kind};
use sightline_rs_facts::nodes::{has, item_doc};
use sightline_rs_provers::catalog::effects_of;
use sightline_rs_provers::{RsBody, RsCall, RsProvers};

use crate::Rule;

// --- #27 purchase price ------------------------------------------------------
// The module-size arm only: Rust's fan-out arm needs a module graph this
// campaign's name-level resolution does not answer (campaign 2).

/// module size past which a hot symbol is expensive
const PRICE_LINES: usize = 500;
const PRICE_MIN_FANIN: usize = 3;
/// hot symbols named in the message
const PRICE_NAMED: usize = 3;
/// what a module can be the smallest container of
const PRICE_TYPES: &str = "struct enum trait";

pub const RULE_27: Rule = Rule {
    record: RuleRecord {
        id: "27",
        slug: "purchase-price",
        family: "C",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "modules big enough that their hot symbols tax every reader",
        goal: "Context economics: every fact lives in a container an agent must ingest whole, \
               so a hot symbol in a huge file taxes every task that touches it.",
        lang: "rs",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_27,
};

/// The module's own symbols other modules lean on, hardest first: what a
/// reader comes to this file for.
fn hot_symbols<'a>(facts: &'a RsFacts<'_>, module: &str) -> Vec<(&'a str, usize)> {
    let mut hot: Vec<(&str, usize)> = facts
        .symbols_of(module)
        .filter_map(|sym| {
            let n = facts
                .refs_of(&sym.qname)
                .filter(|r| &*r.module != module)
                .count();
            (n >= PRICE_MIN_FANIN).then_some((&*sym.qname, n))
        })
        .collect();
    hot.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    hot
}

/// The territory the type costs a reader: its declaration and every `impl`
/// block hung on it, docs and all.
fn type_span(facts: &RsFacts<'_>, owner: &RsSymbol<'_>) -> i64 {
    let hung: i64 = facts
        .impls
        .iter()
        .filter(|i| i.type_qname.as_str() == &*owner.qname)
        .map(|i| i.node.end_position().row as i64 + 1 - i64::from(i.lineno))
        .sum();
    i64::from(owner.end_lineno) - i64::from(owner.lineno) + hung
}

/// Is every hot symbol one type or a member of it, and is that type still
/// small enough to be a unit: then the module is the smallest container of
/// its concept. A type that is itself a module's worth is the thing to lift
/// from, not a reason to stay silent.
fn one_concept(facts: &RsFacts<'_>, qname: &str, hot: &[(&str, usize)]) -> bool {
    let prefix = format!("{qname}::");
    let mut owners: Vec<&str> = hot
        .iter()
        .map(|(t, _)| pytext::partition(pytext::removeprefix(t, &prefix), "::").0)
        .collect();
    owners.sort_unstable();
    owners.dedup();
    let [only] = owners[..] else { return false };
    let Some(owner) = facts.symbols.get(format!("{qname}::{only}").as_str()) else {
        return false;
    };
    has(PRICE_TYPES, owner.kind) && type_span(facts, owner) < PRICE_LINES as i64
}

/// One finding per module: the price is the module's, not each symbol's - a
/// reader pays it once, whichever hot symbol brought them in.
fn rule_27<'t>(facts: &'t RsFacts<'t>, _provers: &RsProvers<'t>, out: &mut Sink) {
    for (qname, module) in sorted_modules(facts) {
        let price = module.lines.len();
        if price < PRICE_LINES {
            continue;
        }
        let hot = hot_symbols(facts, qname);
        if hot.is_empty() || one_concept(facts, qname, &hot) {
            continue;
        }
        let prefix = format!("{qname}::");
        let named: Vec<String> = hot
            .iter()
            .take(PRICE_NAMED)
            .map(|(t, n)| format!("{} ({n})", pytext::removeprefix(t, &prefix)))
            .collect();
        let fan_in = u64::from(facts.fan_in.get(qname).copied().unwrap_or(0));
        out.push(Finding {
            rule: "27",
            site: Site {
                rel: module.rel.clone(),
                line: 1,
                col: 0,
                symbol: qname.clone(),
            },
            message: format!(
                "{qname} is {price} lines holding {} hot symbols, led by {} - every reader \
                 pays the whole file",
                hot.len(),
                named.join(", ")
            ),
            cause: format!("price:{qname}"),
            evidence: Evidence::idx(),
            salience: (price as u64 * fan_in) as f64,
            fix: None,
            lang: "rs",
        });
    }
}

/// `sorted(facts.modules.items())`: qnames are unique, so the key is whole.
pub(crate) fn sorted_modules<'a, 't>(facts: &'a RsFacts<'t>) -> Vec<(&'a Qname, &'a RsModule<'t>)> {
    let mut rows: Vec<(&Qname, &RsModule<'t>)> = facts.modules.iter().collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));
    rows
}

// --- #29 top-loading ---------------------------------------------------------

/// small files are never punished
const TOPLOAD_MIN_LINES: usize = 150;

pub const RULE_29: Rule = Rule {
    record: RuleRecord {
        id: "29",
        slug: "top-loading",
        family: "C",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "big modules with no `//!` header",
        goal: "Top-load the map: the first screen should say what a module is.",
        lang: "rs",
        scope: Scope::File,
        complement: "",
    },
    run: rule_29,
};

/// A module past the line bar whose first screen says nothing about it, in
/// either spelling of a module doc. Every fact read is the module's own, so
/// single-file facts answer it (`Scope::File`).
fn rule_29<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    for (qname, module) in &facts.modules {
        // a test file is not an entry point a reader budgets for
        if facts.is_test(&module.rel)
            || module.lines.len() < TOPLOAD_MIN_LINES
            || !provers.module_docs()[qname].is_empty()
        {
            continue;
        }
        out.push(Finding {
            rule: "29",
            site: Site {
                rel: module.rel.clone(),
                line: 1,
                col: 0,
                symbol: module.qname.clone(),
            },
            message: format!(
                "{} ({} lines, {} top-level items) has no `//!` module doc",
                module.qname,
                module.lines.len(),
                top_items(facts, module)
            ),
            cause: format!("top-loading:{}", module.qname),
            evidence: Evidence::ast(),
            salience: module.lines.len() as f64,
            fix: None,
            lang: "rs",
        });
    }
}

/// Items the file declares at module scope: what the first screen owes a
/// reader an account of. A method or a nested `mod`'s item sits deeper.
fn top_items(facts: &RsFacts<'_>, module: &RsModule<'_>) -> usize {
    let depth = module.qname.matches("::").count() + 1;
    facts
        .symbols_of(&module.qname)
        .filter(|sym| sym.parent.is_none() && sym.qname.matches("::").count() == depth)
        .count()
}

// --- #59 entry-point cost docs -----------------------------------------------

/// the sibling's: under it the whole body is the first screen
const HEAVY_SPAN: u32 = 30;
/// where a `fn main` starts a binary
const BINS: [&str; 3] = ["/main.rs", "/bin/", "/examples/"];
/// what the run leaves behind it: another machine, a process of its own,
/// data deleted. Not a spawn - a thread or a `spawn_blocking` hop runs in the
/// process the reader is already holding, and costs a doc nothing to say.
static OFF_MACHINE: LazyLock<ClassSet> =
    LazyLock::new(|| SPENDS.iter().copied().filter(|c| *c != SPAWNS).collect());

pub const RULE_59: Rule = Rule {
    record: RuleRecord {
        id: "59",
        slug: "cost-docstring",
        family: "C",
        engine_class: "IDX",
        posture: Posture::Report,
        meaning: "heavy entry points that spend off the machine without saying so",
        goal: "An entry point's first screen should say what a run costs where the reader \
               cannot walk it back: another machine, another process, data deleted. One judged \
               row survives its restriction (a thread hop and a setting the process keeps are \
               no spend), so gating it would block on an unmeasured precision: REPORT until a \
               sample prices it.",
        lang: "rs",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_59,
};

/// What a binary starts at: a runtime's `main` attribute (`#[tokio::main]`,
/// `#[async_std::main]`), or the `main` of a bin target. A `fn main` anywhere
/// else is a function that happens to be named main.
fn is_entry(sym: &RsSymbol<'_>, module: &RsModule<'_>) -> bool {
    if sym
        .attrs
        .iter()
        .any(|a| pytext::strip(a.split('(').next().unwrap_or(a)).ends_with("::main"))
    {
        return true;
    }
    let rel = format!("/{}", module.rel);
    sym.name == "main" && sym.parent.is_none() && BINS.iter().any(|b| rel.contains(b))
}

/// Is this a call the run does not take back inside the process? A `PROCESS`
/// write the process goes on holding - an env var, the panic hook - reorders
/// shared state here and nothing more, which is why the catalog classes those
/// MUTATES as well: the reader is still holding what changed. What is left
/// ends this process, starts another, reaches a machine or deletes data.
fn spends(module: &RsModule<'_>, call: &RsCall<'_>) -> bool {
    let classes = effects_of(module, call);
    !classes.is_disjoint(&OFF_MACHINE) && !classes.contains(MUTATES)
}

/// The first call of a body that spends, as spelled.
fn spent(module: &RsModule<'_>, body: &RsBody<'_>) -> Option<String> {
    body.calls
        .iter()
        .chain(&body.macros)
        .find(|c| spends(module, c))
        .map(|c| c.path.clone())
        .filter(|path| !path.is_empty())
}

/// What this entry point spends off the machine within two edge hops: in its
/// own body, else in the body of something it calls.
fn spend(facts: &RsFacts<'_>, provers: &RsProvers<'_>, sym: &RsSymbol<'_>) -> Option<String> {
    if let Some(found) = spent(&facts.modules[&sym.module], provers.body(&sym.qname)) {
        return Some(found);
    }
    for edge in provers.rust.graph.calls_from(&sym.qname) {
        let Some(callee) = facts.symbols.get(edge.callee.as_str()) else {
            continue;
        };
        if callee.qname == sym.qname {
            continue;
        }
        if let Some(found) = spent(&facts.modules[&callee.module], provers.body(&callee.qname)) {
            return Some(format!("{} -> {found}", callee.name));
        }
    }
    None
}

/// A binary's entry point past the heavy span that spends and documents
/// nothing, in its own `///` run or in the `//!` header of its module. The
/// sibling's second silencer, a signature that spells the spend, has no Rust
/// site: an entry point returns `()` or `Result` by the language's rule and
/// takes no parameters, so neither of its spellings can occur.
fn rule_59<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    let mut symbols: Vec<(&Qname, &RsSymbol<'t>)> = facts.symbols.iter().collect();
    symbols.sort_by(|a, b| a.0.cmp(b.0));
    for (qname, sym) in symbols {
        let module = &facts.modules[&sym.module];
        let span = sym.end_lineno - sym.lineno;
        if !is_fn_kind(sym.kind)
            || sym.is_test
            || facts.is_test(&module.rel)
            || !is_entry(sym, module)
            || span < HEAVY_SPAN
            || item_doc(sym.node)
            || !provers.module_docs()[&sym.module].is_empty()
        {
            continue;
        }
        let Some(found) = spend(facts, provers, sym) else {
            continue;
        };
        out.push(Finding {
            rule: "59",
            site: Site {
                rel: module.rel.clone(),
                line: sym.lineno,
                col: 0,
                symbol: qname.clone(),
            },
            message: format!(
                "heavy entry point {qname} ({span} lines) spends ({found}) and declares no \
                 cost in a doc"
            ),
            cause: format!("cost-docstring:{qname}"),
            evidence: Evidence::idx(),
            salience: f64::from(span),
            fix: None,
            lang: "rs",
        });
    }
}
