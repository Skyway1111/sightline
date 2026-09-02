//! The fast gate over a Cargo root: the one place its verdict differs from Python's, and what a Rust reading's
//! posture does to the gate and the baseline. Anything that runs the whole
//! pipeline over a Cargo root drives `cargo` and is `#[ignore]`.

use sightline_testkit::make_repo;

use crate::{root, run};

const MANIFEST: &str = "[package]\nname = \"demo-crate\"\nversion = \"0.1.0\"\n";
/// tree-sitter-rust 0.24.2 rejects this valid Rust: a `#[cfg]` attribute on
/// a struct-pattern field (salvo spells it twice). The parser is narrower
/// than the grammar, so the gate may not read an ERROR node as a broken
/// edit.
const GRAMMAR_GAP: &str = "pub struct Conf { pub a: u8, pub b: u8 }\n\n\
                           pub fn read(c: Conf) -> u8 {\n\
                           \x20   let Conf { a, #[cfg(feature = \"extra\")] b } = c;\n\
                           \x20   a\n}\n";
const CLEAN: &str = "pub fn one() -> u8 { 1 }\n";

/// A Rust parse error is a grammar gap as often as a broken edit, so it is a
/// note in fast mode and never a block. The Python gate still blocks its own.
#[test]
fn unparsable_rust_is_named_and_never_blocks() {
    let dir = make_repo(&[("Cargo.toml", MANIFEST), ("src/lib.rs", GRAMMAR_GAP)]);
    let out = run(&["gate", &root(&dir), "--files", "src/lib.rs"]);

    assert_eq!(out.code, 0, "{}", out.out);
    assert!(out.out.contains("files checked 1"), "{}", out.out);
    assert!(out.out.contains("blocking 0"), "{}", out.out);
    assert!(
        out.out.contains("note: unparsable: src/lib.rs"),
        "{}",
        out.out
    );
}

/// The pole the note is measured against: the same root, a file that parses,
/// no note and no blocker.
#[test]
fn a_parsing_rust_file_is_gated_silently() {
    let dir = make_repo(&[("Cargo.toml", MANIFEST), ("src/lib.rs", CLEAN)]);
    let out = run(&["gate", &root(&dir), "--files", "src/lib.rs"]);

    assert_eq!(out.code, 0, "{}", out.out);
    assert!(out.out.contains("files checked 1"), "{}", out.out);
    assert!(!out.out.contains("unparsable"), "{}", out.out);
}

/// A file the language's suffix does not spell is never gated by it, whatever
/// the root marks.
#[test]
fn the_gate_reaches_only_the_files_of_a_detected_languages_suffix() {
    let dir = make_repo(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", CLEAN),
        ("m.py", "x = 1\n"),
        ("notes.md", "x\n"),
    ]);
    let out = run(&[
        "gate",
        &root(&dir),
        "--files",
        "src/lib.rs",
        "m.py",
        "notes.md",
        "gone.rs",
    ]);

    // a Cargo root with a `.py` file runs both stacks; the doc and the
    // deleted file carry nothing
    assert_eq!(out.code, 0, "{}", out.out);
    assert!(out.out.contains("files checked 2"), "{}", out.out);
}

#[ignore = "runs cargo over a fresh crate"]
#[test]
fn gate_full_names_an_unparsable_rust_file_without_blocking() {
    let dir = make_repo(&[("Cargo.toml", MANIFEST), ("src/lib.rs", GRAMMAR_GAP)]);
    let out = run(&["gate", &root(&dir), "--full"]);

    assert_eq!(out.code, 0, "{}", out.out);
    assert!(
        out.out.contains("note: unparsable: src/lib.rs"),
        "{}",
        out.out
    );
}

/// A static three functions of its module write (rs #9, REPORT), in a module
/// past the top-loading bar with no `//!` header (rs #29, RATCHET). Posture
/// answers per reading: the audit keeps #9, the gate never blocks on it and
/// the baseline never holds it.
#[ignore = "runs cargo over a fresh crate"]
#[test]
fn a_report_reading_stays_out_of_the_gate_and_the_baseline() {
    let mut source = String::from(
        "use std::sync::Mutex;\n\n\
         static CACHE: Mutex<Vec<u8>> = Mutex::new(Vec::new());\n\n",
    );
    for i in 0..3 {
        source.push_str(&format!(
            "pub fn f{i}() {{\n    CACHE.lock().unwrap().push({i});\n}}\n\n"
        ));
    }
    source.push_str(&"// pad\n".repeat(150));
    let dir = make_repo(&[("Cargo.toml", MANIFEST), ("src/lib.rs", &source)]);

    let audited = run(&["audit", &root(&dir), "--json"]);
    assert_eq!(audited.code, 0, "{}", audited.err);
    let rules: Vec<String> = crate::findings(&audited.out)
        .iter()
        .map(|f| f.0.clone())
        .collect();
    assert!(rules.contains(&"9".to_string()), "{rules:?}");
    assert!(rules.contains(&"29".to_string()), "{rules:?}");

    let blocking = run(&["gate", &root(&dir), "--full"]);
    assert!(blocking.out.contains("#29"), "{}", blocking.out);
    assert!(!blocking.out.contains("#9 "), "{}", blocking.out);

    assert_eq!(run(&["baseline", &root(&dir)]).code, 0);
    let counts = std::fs::read_to_string(dir.path().join(".sightline-baseline.json")).unwrap();
    assert!(counts.contains("\"29|"), "{counts}");
    assert!(!counts.contains("\"9|"), "{counts}");
}
