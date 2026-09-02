//! Family T over Rust, #42
//! assertion-free test and #47 sleepy test. Binary structural shapes over the
//! `#[test]` items, as the Python siblings are over the collected test defs.

use std::collections::{HashMap, HashSet};

use sightline_core::findings::{Evidence, Finding, Qname, Sink};
use sightline_core::pytext::format_g;
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_rs_facts::Node;
use sightline_rs_facts::model::{Resolution, RsFacts, RsSymbol, is_fn_kind, text};
use sightline_rs_facts::nodes::{ALL, ATTRS, NESTED_FN, ancestors, descend, has, statements};
use sightline_rs_provers::{RsBody, RsCall, RsProvers};

use crate::Rule;
use crate::util::site;

/// A `fn` a test harness runs: `#[test]`, and the runtimes' own spellings of
/// it (`#[tokio::test]`). A `#[cfg(test)]` module's other items are its
/// helpers, not test cases.
fn is_test_case(sym: &RsSymbol<'_>) -> bool {
    is_fn_kind(sym.kind)
        && sym
            .attrs
            .iter()
            .any(|a| a == "test" || a.ends_with("::test"))
}

fn test_fns<'a, 't>(facts: &'a RsFacts<'t>) -> impl Iterator<Item = &'a RsSymbol<'t>> {
    facts.symbols.values().filter(|sym| is_test_case(sym))
}

// --- #42 assertion-free test --------------------------------------------------

/// macros that stop the body: a verdict only where a condition reached them
const STOPPERS: &str = "panic unreachable";
/// what turns an `Err` into a failure: the `-> Result` test's report, and the
/// assertion an idiomatic suite writes on the call it is testing
const FALLIBLE: &str = "unwrap expect unwrap_err";
/// what stands between the call and the unwrap applied to its value
const AWAITED: &str = "await_expression try_expression parenthesized_expression";
/// the items of `_DECLARED`; the attribute on one is `ATTRS`
const DECLARED: &str = "function_item use_declaration struct_item enum_item union_item \
    trait_item impl_item type_item const_item static_item mod_item macro_definition";

/// A statement the compiler alone checks: an item, or the attribute on one.
fn declared(kind: &str) -> bool {
    has(ATTRS, kind) || has(DECLARED, kind)
}

/// `assert!`, `assert_eq!`, `debug_assert_ne!` and any macro a repo writes so
/// (`str.startswith(("assert", "debug_assert"))`).
fn assert_named(name: &str) -> bool {
    name.starts_with("assert") || name.starts_with("debug_assert")
}

/// Does this macro invocation assert, or carry an assertion? tree-sitter
/// leaves macro tokens unparsed, so an `assert_eq!` inside a `try_join!` or
/// `select!` arm is an identifier in the token tree, not a macro of its own.
fn asserts(macro_call: &RsCall<'_>) -> bool {
    assert_named(&macro_call.name)
        || descend(macro_call.node, ALL)
            .into_iter()
            .any(|node| node.kind() == "identifier" && assert_named(&text(node, macro_call.src)))
}

/// What the tree says about the calls a test body makes: the helpers a call
/// by name could reach, and the calls the repo owns.
struct Suite<'t> {
    helpers: HashMap<&'t str, Vec<&'t Qname>>,
    repo_calls: HashSet<usize>,
}

impl<'t> Suite<'t> {
    /// `helpers`: bare name -> every `fn` a suite wrote to be called (test
    /// code, and not itself a case the harness runs). That is what a call by
    /// name could reach, since this campaign resolves no receiver types. The
    /// code under test is never a helper - its own `debug_assert!` is the
    /// library's claim about itself, not a verdict the test made - and
    /// neither is a sibling case a suite happened to name after the method
    /// under test.
    ///
    /// `repo_calls`: the nodes of every call this repo owns. A path the
    /// name-level walk cannot follow (a re-export: `UdpSocket::bind` through
    /// a shim module) still names one of the repo's crates, and nothing
    /// outside it does.
    fn new(facts: &'t RsFacts<'t>) -> Suite<'t> {
        let mut helpers: HashMap<&'t str, Vec<&'t Qname>> = HashMap::new();
        for sym in facts.symbols.values() {
            if is_fn_kind(sym.kind) && sym.is_test && !is_test_case(sym) {
                helpers.entry(&sym.name).or_default().push(&sym.qname);
            }
        }
        let owned = |r: Resolution| matches!(r, Resolution::Resolved | Resolution::ByName);
        let repo_calls = facts
            .call_sites
            .iter()
            .filter(|call| {
                owned(call.resolution)
                    || facts.crates.contains_key(
                        call.target
                            .as_deref()
                            .unwrap_or("")
                            .split("::")
                            .next()
                            .unwrap_or(""),
                    )
            })
            .map(|call| call.node.id())
            .collect();
        Suite {
            helpers,
            repo_calls,
        }
    }
}

/// Is this `.unwrap()` applied to a call of the code under test? Then an
/// `Err` out of it fails the test, which is the verdict an idiomatic Rust
/// suite writes; the same call on stdlib or fixture setup (`fs::read`,
/// `.parse()` of a literal) only stages the scenario.
fn unwraps_the_subject(call: &RsCall<'_>, repo: &HashSet<usize>) -> bool {
    let mut node = match call.node.child_by_field_name("function") {
        Some(func) if func.kind() == "field_expression" => func.child_by_field_name("value"),
        _ => None,
    };
    while let Some(inner) = node.filter(|n| has(AWAITED, n.kind())) {
        node = inner.named_child(0);
    }
    node.is_some_and(|n| repo.contains(&n.id()))
}

/// Can this body fail on what the code did? An assertion decides; a `panic!`
/// decides only under a condition, since a bare one stops whatever the code
/// did; a fallible call decides where the signature reports it, or where what
/// it unwraps is the code under test.
fn verdict(provers: &RsProvers<'_>, suite: &Suite<'_>, qname: &str, body: &RsBody<'_>) -> bool {
    let fallible = || body.calls.iter().filter(|c| has(FALLIBLE, &c.name));
    body.macros.iter().any(asserts)
        || body
            .macros
            .iter()
            .any(|m| has(STOPPERS, &m.name) && provers.guarded(m))
        || fallible().any(|c| unwraps_the_subject(c, &suite.repo_calls))
        || (provers.returns(qname).contains("Result")
            && (body.tries > 0 || fallible().next().is_some()))
}

/// Does this body verdict, or any test helper it calls by name at any depth?
/// A suite's oracle sits at the end of a helper chain as often as in the
/// first one (`test -> check_all -> check`).
fn reaches_verdict(
    provers: &RsProvers<'_>,
    suite: &Suite<'_>,
    qname: &str,
    seen: &mut HashSet<String>,
) -> bool {
    if !seen.insert(qname.to_string()) {
        return false;
    }
    let body = provers.body(qname);
    verdict(provers, suite, qname, body)
        || body.calls.iter().any(|call| {
            suite
                .helpers
                .get(call.name.as_str())
                .into_iter()
                .flatten()
                .any(|q| reaches_verdict(provers, suite, q, seen))
        })
}

/// Is the whole of this statement something the compiler checks - an item, a
/// `let` that writes the type it wants, or a body that calls nothing but what
/// its own type arguments pin (`debug::<TcpStream>()`)?
fn type_level(stmt: Node<'_>) -> bool {
    if declared(stmt.kind()) {
        return true;
    }
    if stmt.kind() == "let_declaration" && stmt.child_by_field_name("type").is_some() {
        return true;
    }
    let runs = |node: &Node<'_>| {
        node.kind() == "macro_invocation"
            || (node.kind() == "call_expression"
                && node
                    .child_by_field_name("function")
                    .is_some_and(|func| func.kind() != "generic_function"))
    };
    !runs(&stmt) && !descend(stmt, NESTED_FN).iter().any(runs)
}

/// Is the type checker this test's whole oracle? Every statement states a
/// type the compiler must accept and none runs work it could get wrong, so a
/// runtime assertion would add nothing (parity suites, type-surface
/// regression tests).
fn compile_checked(sym: &RsSymbol<'_>) -> bool {
    let stmts = match sym.node.child_by_field_name("body") {
        Some(body) => statements(body),
        None => Vec::new(),
    };
    !stmts.is_empty() && stmts.into_iter().all(type_level)
}

pub const RULE_42: Rule = Rule {
    record: RuleRecord {
        id: "42",
        slug: "assertion-free-test",
        family: "T",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "test function with no verdict in its body or a repo helper it calls",
        goal: "A test without an oracle passes whatever the code does (tsDetect Unknown Test; \
               8-77% of LLM-written suites).",
        lang: "rs",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_42,
};

/// A `#[test]` fn with no verdict of its own and none in a test helper it
/// calls by name. `#[should_panic]` is the verdict the attribute states, an
/// `.unwrap()` on the code under test is the one the suite wrote, and a body
/// the compiler checks whole has the type checker for an oracle.
fn rule_42<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    let suite = Suite::new(facts);
    for sym in test_fns(facts) {
        if sym.attrs.iter().any(|a| a.starts_with("should_panic")) {
            continue;
        }
        if compile_checked(sym) || reaches_verdict(provers, &suite, &sym.qname, &mut HashSet::new())
        {
            continue;
        }
        out.push(Finding {
            rule: "42",
            site: site(facts, sym, sym.node),
            message: format!(
                "{} asserts nothing - it can only fail by panicking",
                sym.qname
            ),
            cause: format!("assertion-free:{}", sym.qname),
            evidence: Evidence::idx(),
            salience: 0.0,
            fix: None,
            lang: "rs",
        });
    }
}

// --- #47 sleepy test ----------------------------------------------------------

/// a body the test hands to a driver runs on that driver's clock
const HANDED_OFF: &str = "closure_expression async_block";
const LOOPS: &str = "for_expression while_expression loop_expression";
/// `_LOOPS | SCOPES`: `_breaks` enters the loop itself and stops at any
/// nested loop or scope
const LOOPS_AND_SCOPES: &str = "for_expression while_expression loop_expression function_item \
    closure_expression";

/// Does the loop's own body `break` - leave on something other than the
/// clock? A nested loop's break is that loop's.
fn breaks(node: Node<'_>) -> bool {
    descend(node, LOOPS_AND_SCOPES)
        .iter()
        .any(|inner| inner.kind() == "break_expression")
}

/// Does the test itself stop here on the wall clock? A sleep inside a closure
/// or an async block is the scenario the test hands to a driver - a
/// simulation host on its virtual clock, a spawned producer staging a delayed
/// event - while the test body synchronizes elsewhere; a sleep in a loop that
/// breaks is a poll loop's backoff, which is the synchronizing on the
/// condition the rule asks for.
fn the_tests_own_wait(call: &RsCall<'_>) -> bool {
    !ancestors(call.node)
        .into_iter()
        .any(|node| has(HANDED_OFF, node.kind()) || (has(LOOPS, node.kind()) && breaks(node)))
}

pub const RULE_47: Rule = Rule {
    record: RuleRecord {
        id: "47",
        slug: "sleepy-test",
        family: "T",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "positive constant sleep inside a test",
        goal: "Wall-clock waits are slow and flaky by construction; synchronize on the \
               condition instead (developers: 62% immediate refactor).",
        lang: "rs",
        scope: Scope::File,
        complement: "",
    },
    run: rule_47,
};

/// `std::thread::sleep` or `tokio::time::sleep` given a literal `Duration`
/// where the `#[test]` fn's own body is what waits on it.
fn rule_47<'t>(facts: &'t RsFacts<'t>, provers: &RsProvers<'t>, out: &mut Sink) {
    for sym in test_fns(facts) {
        for call in &provers.body(&sym.qname).calls {
            if call.name != "sleep" || !the_tests_own_wait(call) {
                continue;
            }
            // `if secs := ...`: no finding for `None`, and none for 0.0
            let Some(secs) = provers.constant_duration(call).filter(|s| *s != 0.0) else {
                continue;
            };
            out.push(Finding {
                rule: "47",
                site: site(facts, sym, call.node),
                message: format!(
                    "{} sleeps {}s - wall-clock waits make tests slow and flaky",
                    sym.qname,
                    format_g(secs)
                ),
                cause: format!("sleepy:{}:{}", sym.qname, call.line),
                evidence: Evidence::ast(),
                salience: 0.0,
                fix: None,
                lang: "rs",
            });
        }
    }
}
