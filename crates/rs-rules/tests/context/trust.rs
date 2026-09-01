//! `tests/rs/test_rules_trust.py`: #9 and #53 over Rust facts, the firing
//! shape, the silent sibling and the exemption each arm names.

use sightline_core::findings::Finding;
use sightline_core::rule::Posture;
use sightline_testkit::run_rs_rule;

/// three writers of one cell, three readers of another
const STATICS: &str = r#"use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;

static CACHE: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static SEEN: AtomicUsize = AtomicUsize::new(0);

pub fn a() {
    CACHE.lock().unwrap().push(1);
    SEEN.load(std::sync::atomic::Ordering::Relaxed);
}

pub fn b() {
    CACHE.lock().unwrap().clear();
    SEEN.load(std::sync::atomic::Ordering::Relaxed);
}

pub fn c() {
    CACHE.lock().unwrap().pop();
    SEEN.load(std::sync::atomic::Ordering::Relaxed);
}
"#;

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

fn symbols(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| &*f.site.symbol).collect()
}

// --- #9 shared mutable state -------------------------------------------------

#[test]
fn rule_9_fires_on_a_cell_three_functions_of_its_module_write() {
    let findings = run_rs_rule("9", &[("src/lib.rs", STATICS)]);

    let rows: Vec<(&str, u32, &str)> = findings
        .iter()
        .map(|f| (&*f.site.symbol, f.site.line, f.cause.as_str()))
        .collect();
    assert_eq!(
        rows,
        [("demo_crate::CACHE", 4, "local-writers:demo_crate::CACHE")]
    );
    assert!(
        findings[0]
            .message
            .contains("demo_crate::a, demo_crate::b, demo_crate::c")
    );
}

#[test]
fn rule_9_two_writers_are_silent() {
    let source = STATICS.replace("CACHE.lock().unwrap().pop();", "()");

    assert_eq!(run_rs_rule("9", &[("src/lib.rs", &source)]), []);
}

/// A `static mut` holds shared state whatever its type; a `thread_local!`
/// gives every thread a slot of its own, so its writers reach nothing anyone
/// else holds.
#[test]
fn rule_9_a_static_mut_is_a_cell_and_a_thread_local_is_not() {
    let source = concat!(
        "static mut COUNT: u32 = 0;\n",
        "\n",
        "thread_local! {\n",
        "    static DEPTH: std::cell::RefCell<u32> = std::cell::RefCell::new(0);\n",
        "}\n",
        "\n",
        "pub fn a() {\n",
        "    unsafe { COUNT += 1; }\n",
        "    DEPTH.with(|d| *d.borrow_mut() += 1);\n",
        "}\n",
        "\n",
        "pub fn b() {\n",
        "    unsafe { COUNT = 0; }\n",
        "    DEPTH.with(|d| *d.borrow_mut() = 0);\n",
        "}\n",
        "\n",
        "pub fn c() {\n",
        "    unsafe { COUNT += 2; }\n",
        "    DEPTH.with_borrow_mut(|d| *d += 1);\n",
        "}\n",
    );

    assert_eq!(
        symbols(&run_rs_rule("9", &[("src/lib.rs", source)])),
        ["demo_crate::COUNT"]
    );
}

#[test]
fn rule_9_a_test_module_is_silent() {
    let files = [
        ("src/lib.rs", "pub fn one() -> u32 { 1 }\n"),
        ("tests/it.rs", STATICS),
    ];

    assert_eq!(run_rs_rule("9", &files), []);
}

// --- #53 error contract ------------------------------------------------------

const ERRORS_DOC: &str = r#"use std::fmt;

pub enum Error {
    NotFound,
    Io,
    Late,
}

/// Read the thing.
///
/// # Errors
///
/// Returns [`Error::NotFound`] when the thing is not there.
pub fn read(k: u8) -> Result<u8, Error> {
    if k == 0 {
        return Err(Error::NotFound);
    }
    if k == 1 {
        return Err(Error::Io);
    }
    bail!(Error::Late);
    Ok(k)
}
"#;

#[test]
fn rule_53_fires_on_a_variant_the_errors_section_never_names() {
    let findings = run_rs_rule("53", &[("src/lib.rs", ERRORS_DOC)]);

    assert_eq!(
        causes(&findings),
        [
            "raise-contract:undeclared:demo_crate::read:Io",
            "raise-contract:undeclared:demo_crate::read:Late",
        ]
    );
    assert_eq!(findings[0].site.line, 14);
}

/// No `# Errors` heading at all is `missing_errors_doc`; a section that names
/// every variant the body returns is the contract kept.
#[test]
fn rule_53_a_missing_section_is_clippys_and_a_named_variant_is_silent() {
    let panics = ERRORS_DOC.replace("# Errors", "# Panics");
    let named = ERRORS_DOC.replace(
        "Returns [`Error::NotFound`] when the thing is not there.",
        "Returns [`Error::NotFound`], [`Error::Io`] or [`Error::Late`].",
    );
    let files = [
        ("src/lib.rs", panics.as_str()),
        ("src/named.rs", named.as_str()),
    ];

    assert_eq!(run_rs_rule("53", &files), []);
}

/// `?` forwards whatever the callee named, which is the callee's contract; a
/// function outside the crate's surface has no caller to keep one with.
#[test]
fn rule_53_a_question_mark_and_a_private_function_are_unread() {
    let source = concat!(
        "pub enum Error { Io }\n",
        "\n",
        "/// # Errors\n",
        "///\n",
        "/// Never.\n",
        "pub fn forwards(v: &str) -> Result<u8, Error> {\n",
        "    Ok(v.parse().map_err(|_| Error::Io)?)\n",
        "}\n",
        "\n",
        "/// # Errors\n",
        "///\n",
        "/// Never.\n",
        "fn private() -> Result<u8, Error> {\n",
        "    Err(Error::Io)\n",
        "}\n",
    );

    assert_eq!(run_rs_rule("53", &[("src/lib.rs", source)]), []);
}

/// One and two judged rows: neither reading holds the n >= 5 a blocking
/// posture is priced on, so each reports until a fresh seed measures it
/// (`docs/todo.md`).
#[test]
fn the_family_reports_until_a_round_prices_it() {
    for id in ["9", "53"] {
        let rule = sightline_rs_rules::RULES
            .iter()
            .find(|r| r.record.id == id)
            .expect("the rule is registered");
        assert_eq!(rule.record.posture, Posture::Report, "#{id}");
    }
}
