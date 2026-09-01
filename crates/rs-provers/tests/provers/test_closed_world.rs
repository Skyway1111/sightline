//! Port of `tests/rs/test_closed_world.py`: every closed-world escape over
//! the resolved edges, one fixture per named reason, plus the judged set the
//! rustc probe pinned. An escape dropped is a dead-weight finding claimed
//! over a caller the index cannot show, so each reason is red first: the same
//! fixture without its escape passes.

use std::collections::BTreeSet;

use sightline_rs_facts::model::RsFacts;
use sightline_rs_provers::closed_world::ClosedWorld;
use sightline_rs_provers::oracle::RsAnswers;
use sightline_rs_provers::oracle::index::{RsEdge, RsGraph};
use sightline_testkit::{build_rs, rs_answers};

const BIN: &str = "[package]\nname = \"app\"\nversion = \"0.1.0\"\n";
const LIB_UNPUBLISHED: &str =
    "[package]\nname = \"my-app\"\nversion = \"0.1.0\"\npublish = false\n";
/// `CHECKED`: the one member every fixture's base check compiled.
const CHECKED: [&str; 1] = ["app"];

/// `edge(callee, caller="app::main", call=True, rel="src/main.rs", line=1)`.
fn edge(callee: &str) -> (&'static str, String, &'static str, u32, bool) {
    ("app::main", callee.to_string(), "src/main.rs", 1, true)
}

fn answers(edges: &[(&str, String, &str, u32, bool)], checked: &[&str]) -> RsAnswers {
    let rows: Vec<(&str, &str, &str, u32, bool)> = edges
        .iter()
        .map(|(caller, callee, rel, line, call)| (*caller, callee.as_str(), *rel, *line, *call))
        .collect();
    rs_answers(&rows, checked)
}

fn reasons(facts: &RsFacts<'_>, answers: &RsAnswers, qname: &str) -> BTreeSet<String> {
    ClosedWorld::new(facts, answers)
        .verdict(qname)
        .reasons
        .into_iter()
        .collect()
}

// --- the judged set ---------------------------------------------------------

/// The probe: rustc's `dead_code` reports private, `pub(crate)` and
/// `pub`-in-private-module items and stays silent on the rest.
#[test]
fn only_the_items_rustc_leaves_alone_are_reachable() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        (
            "src/main.rs",
            "pub mod deep;\nmod hidden;\n\
             pub fn top() {}\npub(crate) fn crate_only() {}\nfn private() {}\n\
             fn main() {}\n",
        ),
        ("src/deep.rs", "pub fn reachable() {}\n"),
        ("src/hidden.rs", "pub fn buried() {}\n"),
    ]);
    let empty = answers(&[], &CHECKED);
    let world = ClosedWorld::new(stack.facts(), &empty);

    let found: BTreeSet<&str> = world.reachable().iter().map(|q| &**q).collect();
    assert_eq!(found, BTreeSet::from(["app::top", "app::deep::reachable"]));
}

#[test]
fn a_test_item_is_never_judged() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        (
            "src/main.rs",
            "#[cfg(test)]\npub fn helper() {}\nfn main() {}\n",
        ),
    ]);
    let empty = answers(&[], &CHECKED);

    assert!(
        ClosedWorld::new(stack.facts(), &empty)
            .reachable()
            .is_empty()
    );
}

// --- the escapes ------------------------------------------------------------

#[test]
fn an_item_one_call_edge_reaches_passes() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        ("src/main.rs", "pub fn f() {}\nfn main() { f(); }\n"),
    ]);
    let one = answers(&[edge("app::f")], &CHECKED);

    assert!(
        ClosedWorld::new(stack.facts(), &one)
            .verdict("app::f")
            .passed
    );
}

#[test]
fn a_published_item_escapes() {
    let (_dir, stack) = build_rs(&[("Cargo.toml", BIN), ("src/lib.rs", "pub fn shipped() {}\n")]);
    let empty = answers(&[], &CHECKED);

    assert_eq!(
        reasons(stack.facts(), &empty, "app::shipped"),
        BTreeSet::from(["published".to_string()])
    );
}

#[test]
fn a_reference_that_is_not_a_call_escapes() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        (
            "src/main.rs",
            "pub fn f() {}\nfn main() { let g: fn() = f; g(); }\n",
        ),
    ]);
    let read = answers(
        &[("app::main", "app::f".to_string(), "src/main.rs", 1, false)],
        &CHECKED,
    );

    assert_eq!(
        reasons(stack.facts(), &read, "app::f"),
        BTreeSet::from(["reference-escape".to_string()])
    );
}

#[test]
fn an_edge_to_a_trait_declaration_escapes() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        (
            "src/main.rs",
            "pub trait Greet { fn hi(&self); }\nfn main() {}\n",
        ),
    ]);
    // `open` is the one edge field the testkit fixture never sets
    let open = RsAnswers {
        graph: RsGraph::new(
            vec![RsEdge {
                caller: "app::main".to_string(),
                callee: "app::Greet".to_string(),
                rel: "src/main.rs".to_string(),
                line: 1,
                call: true,
                open: true,
            }],
            Default::default(),
        ),
        ..answers(&[], &CHECKED)
    };

    assert_eq!(
        reasons(stack.facts(), &open, "app::Greet"),
        BTreeSet::from(["open-dispatch".to_string()])
    );
}

/// Dispatch reaches the method through the trait, so its own edges
/// understate its callers.
#[test]
fn a_trait_impl_method_escapes() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        (
            "src/main.rs",
            "pub trait Greet { fn hi(&self); }\npub struct Loud;\n\
             impl Greet for Loud { fn hi(&self) {} }\n\
             impl Loud { pub fn own(&self) {} }\nfn main() {}\n",
        ),
    ]);
    let empty = answers(&[], &CHECKED);

    assert_eq!(
        reasons(stack.facts(), &empty, "app::Loud::hi"),
        BTreeSet::from(["open-dispatch".to_string()])
    );
    assert!(
        ClosedWorld::new(stack.facts(), &empty)
            .verdict("app::Loud::own")
            .passed
    );
}

#[test]
fn a_linker_named_item_escapes() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        (
            "src/main.rs",
            "#[no_mangle]\npub fn exported() {}\n\
             pub extern \"C\" fn abi() {}\n\
             pub fn plain() {}\nfn main() {}\n",
        ),
    ]);
    let empty = answers(&[], &CHECKED);

    assert_eq!(
        reasons(stack.facts(), &empty, "app::exported"),
        BTreeSet::from(["extern".to_string()])
    );
    assert_eq!(
        reasons(stack.facts(), &empty, "app::abi"),
        BTreeSet::from(["extern".to_string()])
    );
    assert!(
        ClosedWorld::new(stack.facts(), &empty)
            .verdict("app::plain")
            .passed
    );
}

#[test]
fn a_proc_macro_attribute_or_derive_escapes() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        (
            "src/main.rs",
            "#[derive(Parser)]\npub struct Args;\n\
             #[derive(Debug, Clone)]\npub struct Plain;\n\
             #[tokio::main]\npub fn served() {}\n\
             #[inline]\n#[allow(dead_code)]\n#[doc = \"x\"]\npub fn owned() {}\n\
             fn main() {}\n",
        ),
    ]);
    let empty = answers(&[], &CHECKED);
    let world = ClosedWorld::new(stack.facts(), &empty);

    assert_eq!(
        reasons(stack.facts(), &empty, "app::Args"),
        BTreeSet::from(["proc-macro".to_string()])
    );
    assert_eq!(
        reasons(stack.facts(), &empty, "app::served"),
        BTreeSet::from(["proc-macro".to_string()])
    );
    assert!(world.verdict("app::Plain").passed);
    assert!(world.verdict("app::owned").passed);
}

/// Default features only: the arm this build never compiled holds references
/// cargo never read.
#[test]
fn a_cfg_other_than_test_escapes_the_item_and_its_callers() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", &format!("{BIN}\n[features]\nextra = []\n")),
        (
            "src/main.rs",
            "#[cfg(feature = \"extra\")]\npub fn gated() {}\n\
             #[cfg(feature = \"extra\")]\nfn gated_caller() { target(); }\n\
             pub fn target() {}\nfn main() {}\n",
        ),
    ]);
    let empty = answers(&[], &CHECKED);
    let from_gated = answers(
        &[(
            "app::gated_caller",
            "app::target".to_string(),
            "src/main.rs",
            1,
            true,
        )],
        &CHECKED,
    );
    let from_main = answers(&[edge("app::target")], &CHECKED);

    assert_eq!(
        reasons(stack.facts(), &empty, "app::gated"),
        BTreeSet::from(["cfg-gated".to_string()])
    );
    assert_eq!(
        reasons(stack.facts(), &from_gated, "app::target"),
        BTreeSet::from(["cfg-gated".to_string()])
    );
    assert!(
        ClosedWorld::new(stack.facts(), &from_main)
            .verdict("app::target")
            .passed
    );
}

/// Cargo compiles a `#[cfg]` module or nothing, so its items and their
/// references are as unread as an item's own cfg: inline body and file
/// declaration alike (blind audit B: a helper a gated file module called was
/// deleted by a world that never compiled the caller).
#[test]
fn a_cfg_on_the_enclosing_mod_escapes_every_item_inside() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", &format!("{BIN}\n[features]\nextra = []\n")),
        (
            "src/main.rs",
            "#[cfg(feature = \"extra\")]\nmod inline { pub fn f() { crate::target(); } }\n\
             #[cfg(feature = \"extra\")]\nmod gated;\n\
             pub fn target() {}\nfn main() {}\n",
        ),
        ("src/gated.rs", "pub fn g() { crate::target(); }\n"),
    ]);
    let empty = answers(&[], &CHECKED);
    let from_gated = answers(
        &[(
            "app::gated::g",
            "app::target".to_string(),
            "src/main.rs",
            1,
            true,
        )],
        &CHECKED,
    );

    assert_eq!(
        reasons(stack.facts(), &empty, "app::inline::f"),
        BTreeSet::from(["cfg-gated".to_string()])
    );
    assert_eq!(
        reasons(stack.facts(), &empty, "app::gated::g"),
        BTreeSet::from(["cfg-gated".to_string()])
    );
    assert_eq!(
        reasons(stack.facts(), &from_gated, "app::target"),
        BTreeSet::from(["cfg-gated".to_string()])
    );
}

/// A member whose base check errored is out of `checked`, and so is a crate
/// the workspace never enumerated (salvo's `fuzz` declares a `[workspace]` of
/// its own): no world of either can veto a deletion.
#[test]
fn an_item_the_base_check_never_compiled_escapes() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", LIB_UNPUBLISHED),
        ("src/lib.rs", "pub fn f() {}\n"),
    ]);
    let none = answers(&[], &[]);
    let member = answers(&[], &["my-app"]);

    assert_eq!(
        reasons(stack.facts(), &none, "my_app::f"),
        BTreeSet::from(["unchecked-crate".to_string()])
    );
    assert!(
        ClosedWorld::new(stack.facts(), &member)
            .verdict("my_app::f")
            .passed
    );
}

#[test]
fn an_unknown_qname_escapes() {
    let (_dir, stack) = build_rs(&[("Cargo.toml", BIN), ("src/main.rs", "fn main() {}\n")]);
    let empty = answers(&[], &CHECKED);

    assert_eq!(
        reasons(stack.facts(), &empty, "app::nope"),
        BTreeSet::from(["unknown-symbol".to_string()])
    );
}

#[test]
fn every_escape_that_holds_is_reported_and_the_first_names_it() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", BIN),
        ("src/lib.rs", "#[tokio::main]\npub fn shipped() {}\n"),
    ]);
    let empty = answers(&[], &CHECKED);
    let verdict = ClosedWorld::new(stack.facts(), &empty).verdict("app::shipped");

    assert!(!verdict.passed);
    assert_eq!(verdict.reason.as_deref(), Some("published"));
    assert_eq!(
        verdict.reasons.iter().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["published".to_string(), "proc-macro".to_string()])
    );
}
