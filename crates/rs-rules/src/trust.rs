//! Port of `rs/rules/trust.py`: #9 a shared static that several functions of
//! one module write, and #53 an `# Errors` section against what the body
//! returns. Thresholds are the Python siblings'.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use sightline_core::findings::{Evidence, Finding, Sink, Site};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_rs_facts::model::{RsFacts, RsModule, RsSymbol, is_fn_kind, text};
use sightline_rs_facts::nodes::{arg_nodes, children, written_names};
use sightline_rs_provers::RsProvers;

use crate::Rule;
use crate::context::sorted_modules;

// --- #9 shared mutable state -------------------------------------------------

/// the sibling's
const MIN_LOCAL_WRITERS: usize = 3;
/// the interior-mutability wrappers a `static` holds shared state in
const CELLS: [&str; 6] = ["Mutex", "RwLock", "OnceLock", "OnceCell", "Lazy", "Atomic"];

pub const RULE_9: Rule = Rule {
    record: RuleRecord {
        id: "9",
        slug: "shared-mutable-state",
        family: "A",
        engine_class: "IDX",
        posture: Posture::Report,
        meaning: "a shared static written by three functions of its own module",
        goal: "No shared mutable state (Parent, Better Code goal 3): a static mutated from many \
               places is action at a distance. One judged row is no sample to gate on, so this \
               reports until a fresh seed prices it (`docs/todo.md`).",
        lang: "rs",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_9,
};

/// The shared cell a `static` declares: the wrapper its type names, or
/// `static mut` whatever the type. A plain `static` is a constant.
fn cell(sym: &RsSymbol<'_>, src: &[u8]) -> Option<&'static str> {
    if children(sym.node)
        .iter()
        .any(|c| c.kind() == "mutable_specifier")
    {
        return Some("static mut");
    }
    let spelled = match sym.node.child_by_field_name("type") {
        Some(node) => text(node, src).into_owned(),
        None => String::new(),
    };
    CELLS.into_iter().find(|c| spelled.contains(c))
}

/// (name, qname, line, cell) per shared static of one module.
fn cells_of<'a>(
    facts: &'a RsFacts<'_>,
    module: &RsModule<'_>,
) -> Vec<(&'a str, &'a str, u32, &'static str)> {
    let mut rows: Vec<(&str, &str, u32, &'static str)> = facts
        .symbols_of(&module.qname)
        .filter(|sym| sym.kind == "static" && !sym.is_test)
        .filter_map(|sym| {
            cell(sym, module.bytes).map(|held| (sym.name.as_str(), &*sym.qname, sym.lineno, held))
        })
        .collect();
    rows.sort();
    rows
}

/// One arm, the sibling's local-writers one: a `static` holding a cell (or a
/// `static mut`) written from three functions of its own module. A
/// `thread_local!` is per-thread, so it holds no shared state. Writing is
/// taking the cell's write handle (`.lock`, `.write`, `.store`,
/// `.get_or_init`) or storing into a `static mut`; a read (`.load`, `.read`,
/// `.get`) is not one. Test modules are silent: fixture resets are idiomatic.
fn rule_9<'t>(facts: &'t RsFacts<'t>, _provers: &RsProvers<'t>, out: &mut Sink) {
    for (qname, module) in sorted_modules(facts) {
        if facts.is_test(&module.rel) {
            continue;
        }
        let cells = cells_of(facts, module);
        if cells.is_empty() {
            continue;
        }
        let mut writers: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
        for sym in facts.symbols_of(qname) {
            if !is_fn_kind(sym.kind) || sym.is_test {
                continue;
            }
            for name in written_names(sym.node, module.bytes) {
                writers.entry(name).or_default().insert(&sym.qname);
            }
        }
        for (name, owner, line, held) in cells {
            let local: Vec<&str> = writers
                .get(name)
                .map(|w| w.iter().copied().collect())
                .unwrap_or_default();
            if local.len() < MIN_LOCAL_WRITERS {
                continue;
            }
            out.push(Finding {
                rule: "9",
                site: Site {
                    rel: module.rel.clone(),
                    line,
                    col: 0,
                    symbol: owner.into(),
                },
                message: format!(
                    "{held} {owner} is written from {} functions of its own module: {}",
                    local.len(),
                    local.join(", ")
                ),
                cause: format!("local-writers:{owner}"),
                evidence: Evidence::idx(),
                salience: local.len() as f64,
                fix: None,
                lang: "rs",
            });
        }
    }
}

// --- #53 error contract ------------------------------------------------------

static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("the word pattern compiles"));

pub const RULE_53: Rule = Rule {
    record: RuleRecord {
        id: "53",
        slug: "raise-contract",
        family: "A",
        engine_class: "AST",
        posture: Posture::Report,
        meaning: "an `# Errors` section missing an error the body returns",
        goal: "Honest contracts: an Errors section missing a returned error makes every caller \
               re-read the body to learn what to match on. Two judged rows are no sample to gate \
               on, so this reports until a fresh seed prices it (`docs/todo.md`).",
        lang: "rs",
        scope: Scope::Repo,
        complement: "clippy `missing_errors_doc` owns the absent section, and \
                     `missing_panics_doc` the `# Panics` half",
    },
    run: rule_53,
};

/// The words an `# Errors` section names; None where the doc holds no such
/// section, which is clippy's reading and not this one.
fn errors_section(doc: &[String]) -> Option<BTreeSet<String>> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    let (mut inside, mut seen) = (false, false);
    for line in doc {
        let head = pytext::strip(line);
        if head.starts_with('#') {
            inside = pytext::lower(head).trim_end_matches(':') == "# errors";
            seen = seen || inside;
        } else if inside {
            found.extend(WORD.find_iter(line).map(|m| m.as_str().to_string()));
        }
    }
    seen.then_some(found)
}

/// The error a returned expression names: the last path segment spelled like
/// a type, so `Error::NotFound` and `MyError::new(x)` both answer with what a
/// caller matches on. A bound value or a message names none.
fn variant(spelled: &str) -> Option<String> {
    let bare = spelled.replace(' ', "");
    let path = pytext::partition(&bare, "(").0;
    if path.contains('"') {
        return None;
    }
    path.split("::")
        .filter(|s| s.chars().next().is_some_and(char::is_uppercase))
        .last()
        .map(str::to_string)
}

/// A bare-`pub` `fn` returning `Result` whose `# Errors` section is written
/// and misses what the body returns by `Err(..)` or `bail!`. `?` forwards the
/// callee's own contract, so it is unread; an absent section is clippy's
/// finding and not a second one here.
fn rule_53<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    for (qname, module) in sorted_modules(facts) {
        let mut listed: Vec<&RsSymbol<'t>> = facts.symbols_of(qname).collect();
        listed.sort_by(|a, b| a.qname.cmp(&b.qname));
        for sym in listed {
            if !is_fn_kind(sym.kind) || sym.is_test || !sym.is_public {
                continue;
            }
            if !provers.returns(&sym.qname).contains("Result") {
                continue;
            }
            let Some(declared) = errors_section(&provers.doc_above(module, sym.lineno)) else {
                continue;
            };
            let body = provers.body(&sym.qname);
            let errs = body
                .calls
                .iter()
                .filter(|c| c.name == "Err")
                .filter_map(|c| {
                    arg_nodes(c.node)
                        .first()
                        .map(|a| text(*a, c.src).into_owned())
                });
            let bails = body
                .macros
                .iter()
                .filter(|c| c.name == "bail")
                .filter_map(|c| provers.macro_args(c).into_iter().next());
            let returned: BTreeSet<String> = errs
                .chain(bails)
                .filter_map(|spelled| variant(&spelled))
                .collect();
            for missing in returned.difference(&declared) {
                out.push(Finding {
                    rule: "53",
                    site: Site {
                        rel: module.rel.clone(),
                        line: sym.lineno,
                        col: 0,
                        symbol: sym.qname.clone(),
                    },
                    message: format!(
                        "{} returns {missing} but its `# Errors` section never names it",
                        sym.qname
                    ),
                    cause: format!("raise-contract:undeclared:{}:{missing}", sym.qname),
                    evidence: Evidence::Ast {
                        detail: "undeclared error".to_string(),
                    },
                    salience: 0.0,
                    fix: None,
                    lang: "rs",
                });
            }
        }
    }
}
