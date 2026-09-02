//! The Rust `fix`: what the diff says. `crates/py-rules/tests/rules/emit.rs`
//! is the Python side's.

use sightline_core::edits::blank;
use sightline_core::findings::{Evidence, Finding, Fix, Site, SpanEdit};
use sightline_rs_rules::emit;
use sightline_testkit::build_rs;

const MANIFEST: &str = "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
const MAIN: &str = "/// Nothing reaches this.\n\
                    pub fn orphan() -> u32 {\n\
                    \x20   41\n\
                    }\n\
                    \n\
                    pub fn kept() -> u32 {\n\
                    \x20   42\n\
                    }\n\
                    \n\
                    pub mod deep;\n\
                    \n\
                    fn main() {\n\
                    \x20   println!(\"{}\", kept() + deep::used());\n\
                    }\n";
const DEEP: &str = "pub fn used() -> u32 {\n\
                    \x20   1\n\
                    }\n\
                    \n\
                    pub fn also_orphan() {}\n";

fn deletion(rel: &str, edits: Vec<SpanEdit>, cause: &str) -> Finding {
    Finding {
        rule: "32",
        site: Site {
            rel: rel.into(),
            line: edits[0].line,
            col: 0,
            symbol: "app::x".into(),
        },
        message: String::new(),
        cause: cause.to_string(),
        evidence: Evidence::idx(),
        salience: 0.0,
        fix: Some(Fix {
            rel: rel.into(),
            edits,
            imports: Vec::new(),
        }),
        lang: "rs",
    }
}

/// #32's own splices land with unit rs-dead; the deletions here are the spans
/// its splice writes, so the assertions are the emitter's alone.
#[test]
fn the_diff_names_what_it_discharges_and_drops_the_deleted_lines() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", MANIFEST),
        ("src/main.rs", MAIN),
        ("src/deep.rs", DEEP),
    ]);
    let facts = stack.facts();
    let found = vec![
        deletion(
            "src/main.rs",
            blank(&facts.modules["app"].lines, 1, 4),
            "dead-symbol:app::orphan",
        ),
        deletion(
            "src/deep.rs",
            blank(&facts.modules["app::deep"].lines, 5, 5),
            "dead-symbol:app::deep::also_orphan",
        ),
    ];

    let diff = emit::fix(&found, facts, &stack.provers());

    assert!(diff.starts_with("# sightline-fix: 32 dead-symbol:"));
    assert_eq!(diff.lines().filter(|l| l.starts_with("--- a/")).count(), 2);
    assert!(diff.contains("-/// Nothing reaches this."));
    assert!(diff.contains("-pub fn also_orphan() {}"));
    assert!(!diff.contains("-pub fn kept() -> u32 {"));
}

#[test]
fn no_fixable_finding_makes_no_diff() {
    let (_dir, stack) = build_rs(&[("Cargo.toml", MANIFEST), ("src/main.rs", "fn main() {}\n")]);

    assert_eq!(emit::fix(&[], stack.facts(), &stack.provers()), "");
}

/// `compose` settles the two: `apply_edits` reads spans as disjoint, and the
/// wider deletion has taken the site whole.
#[test]
fn a_patch_whose_site_another_deletes_loses_it() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", MANIFEST),
        (
            "src/main.rs",
            "pub fn outer() {\n    fn inner() {}\n}\nfn main() {}\n",
        ),
    ]);
    let facts = stack.facts();
    let lines = &facts.modules["app"].lines;
    let found = vec![
        deletion("src/main.rs", blank(lines, 1, 3), "outer"),
        deletion("src/main.rs", blank(lines, 2, 2), "inner"),
    ];

    let diff = emit::fix(&found, facts, &stack.provers());

    assert!(diff.contains("# sightline-fix: 32 outer\n"));
    assert!(!diff.contains("# sightline-fix: 32 inner\n"));
    // a deletion adds no line, and takes the inner def with the outer one
    assert_eq!(
        diff.lines()
            .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
            .count(),
        0
    );
    assert_eq!(
        diff.lines()
            .filter(|l| l.starts_with('-') && !l.starts_with("---"))
            .count(),
        3
    );
}
