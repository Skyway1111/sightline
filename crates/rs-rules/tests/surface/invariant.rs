//! #21 over Rust: a `match` arm that panics on a variant the repo declares.

use sightline_core::findings::Tier;
use sightline_testkit::{MANIFEST, run_rs_rule};

use crate::{causes, krate};

const MODE: &str = "pub enum Mode { Draw, Overlay }\n";

/// `_match`: one enum and one `match` whose second arm the test writes.
fn matching(arm: &str, enum_item: &str) -> String {
    format!(
        "{enum_item}pub fn run(m: Mode) {{\n    match m {{\n        \
         Mode::Draw => draw(),\n{arm}    }}\n}}\n"
    )
}

#[test]
fn a_panic_arm_on_a_repo_variant_is_a_distributed_invariant() {
    // the AI tree, digitizer/src/main.rs:389
    let src = matching("        Mode::Overlay => unreachable!(),\n", MODE);
    let found = run_rs_rule("21", &krate(&src));

    assert_eq!(causes(&found), ["panic-arm:demo_crate::Mode:Overlay"]);
    assert_eq!(found[0].site.line, 5);
    assert_eq!(&*found[0].site.symbol, "demo_crate::run");
    assert_eq!(found[0].tier(), Tier::Heuristic);
    assert!(found[0].message.contains("Mode::Overlay"));
}

#[test]
fn the_message_names_the_enum_and_the_narrowing() {
    let src = matching("        Mode::Overlay => unreachable!(),\n", MODE);
    let found = run_rs_rule("21", &krate(&src));

    assert_eq!(
        found[0].message,
        "demo_crate::run panics on demo_crate::Mode::Overlay instead of holding the invariant \
         in the type - narrow the scrutinee (a second enum without the variant, or a `TryFrom`)"
    );
}

#[test]
fn a_wildcard_arm_names_no_variant() {
    let src = matching("        _ => unreachable!(),\n", MODE);

    assert!(run_rs_rule("21", &krate(&src)).is_empty());
}

#[test]
fn a_block_holding_only_the_macro_is_the_same_arm() {
    let src = matching(
        "        Mode::Overlay => { panic!(\"no overlay here\"); }\n",
        MODE,
    );
    let found = run_rs_rule("21", &krate(&src));

    assert_eq!(causes(&found), ["panic-arm:demo_crate::Mode:Overlay"]);
}

#[test]
fn an_arm_that_works_before_it_panics_is_no_bare_panic() {
    let src = matching(
        "        Mode::Overlay => { report(m); unreachable!() }\n",
        MODE,
    );

    assert!(run_rs_rule("21", &krate(&src)).is_empty());
}

#[test]
fn a_guarded_arm_panics_on_its_condition() {
    let src = matching("        Mode::Overlay if hot() => unreachable!(),\n", MODE);

    assert!(run_rs_rule("21", &krate(&src)).is_empty());
}

#[test]
fn an_or_pattern_names_a_set_one_narrowing_cannot_remove() {
    let src = matching(
        "        Mode::Draw | Mode::Overlay => unreachable!(),\n",
        MODE,
    );

    assert!(run_rs_rule("21", &krate(&src)).is_empty());
}

#[test]
fn a_variant_pattern_with_fields_is_still_the_variant() {
    // the AI tree, relief/src/bounds.rs:1186 `Owner::Crest { .. }`
    let src = "pub enum Owner { None, Crest { h: u8 }, Peak(u8) }\n\
               pub fn rank(o: Owner) {\n    match o {\n        \
               Owner::Crest { .. } => unreachable!(\"a crest holds no claim\"),\n        \
               Owner::Peak(_) => panic!(\"a peak holds no claim\"),\n        \
               Owner::None => go(),\n    }\n}\n";
    let found = run_rs_rule("21", &krate(src));

    assert_eq!(
        causes(&found),
        [
            "panic-arm:demo_crate::Owner:Crest",
            "panic-arm:demo_crate::Owner:Peak"
        ]
    );
    assert_eq!(
        found.iter().map(|f| f.salience).collect::<Vec<f64>>(),
        [2.0, 2.0]
    );
}

#[test]
fn a_stub_arm_is_work_not_done() {
    // turmoil `Type::SeqPacket => unimplemented!("SOCK_SEQPACKET connect")`
    let src = matching(
        "        Mode::Overlay => unimplemented!(\"overlay mode\"),\n",
        MODE,
    );

    assert!(run_rs_rule("21", &krate(&src)).is_empty());
}

#[test]
fn an_enum_from_outside_the_repo_closes_no_set_here() {
    let src = "pub fn cmp(o: std::cmp::Ordering) {\n    match o {\n        \
               std::cmp::Ordering::Less => unreachable!(),\n        _ => go(),\n    }\n}\n";

    assert!(run_rs_rule("21", &krate(src)).is_empty());
}

#[test]
fn a_non_exhaustive_enum_makes_the_fallback_the_callers_duty() {
    let open = format!("#[non_exhaustive]\n{MODE}");
    let src = matching("        Mode::Overlay => unreachable!(),\n", &open);

    assert!(run_rs_rule("21", &krate(&src)).is_empty());
}

#[test]
fn a_bare_variant_resolves_to_the_one_enum_owning_it() {
    let src = format!(
        "{MODE}use Mode::*;\npub fn run(m: Mode) {{\n    match m {{\n        \
         Draw => draw(),\n        Overlay => unreachable!(),\n    }}\n}}\n"
    );
    let found = run_rs_rule("21", &krate(&src));

    assert_eq!(causes(&found), ["panic-arm:demo_crate::Mode:Overlay"]);
}

#[test]
fn two_crates_naming_one_enum_each_resolve_in_their_own() {
    // the AI tree: digitizer and hydrograph each declare `Mode::Overlay`
    let root = format!("[workspace]\nmembers = [\"crates/other\"]\n{MANIFEST}");
    let lib = format!(
        "{MODE}pub fn run(m: Mode) {{ match m {{\n        Mode::Draw => draw(),\n        \
         Mode::Overlay => unreachable!(),\n    }} }}\n"
    );
    let found = run_rs_rule(
        "21",
        &[
            ("Cargo.toml", &root),
            ("src/lib.rs", &lib),
            (
                "crates/other/Cargo.toml",
                "[package]\nname = \"other-crate\"\nversion = \"0.1.0\"\n",
            ),
            ("crates/other/src/lib.rs", MODE),
        ],
    );

    assert_eq!(causes(&found), ["panic-arm:demo_crate::Mode:Overlay"]);
}

#[test]
fn a_variant_name_two_enums_own_names_neither() {
    let src = format!(
        "{MODE}pub enum Layer {{ Overlay, Base }}\nuse Mode::*;\npub fn run(m: Mode) {{\n    \
         match m {{\n        Draw => draw(),\n        Overlay => unreachable!(),\n    }}\n}}\n"
    );

    assert!(run_rs_rule("21", &krate(&src)).is_empty());
}

#[test]
fn the_implementor_spelling_reads_the_variant_alone() {
    // the AI tree, relief/src/build_attestation.rs:138 `Self::BuildRunner`
    let src = format!(
        "{MODE}impl Mode {{\n    pub fn go(self) {{\n        match self {{\n            \
         Self::Draw => draw(),\n            Self::Overlay => unreachable!(),\n        }}\n    \
         }}\n}}\n"
    );
    let found = run_rs_rule("21", &krate(&src));

    assert_eq!(causes(&found), ["panic-arm:demo_crate::Mode:Overlay"]);
    assert_eq!(&*found[0].site.symbol, "demo_crate::Mode::go");
}

#[test]
fn a_panic_arm_in_test_code_is_silent() {
    let src = format!(
        "{MODE}#[cfg(test)]\nmod tests {{\n    use super::Mode;\n    #[test]\n    \
         fn f(m: Mode) {{\n        match m {{\n            Mode::Draw => (),\n            \
         Mode::Overlay => unreachable!(),\n        }}\n    }}\n}}\n"
    );

    assert!(run_rs_rule("21", &krate(&src)).is_empty());
}

#[test]
fn salience_is_the_arms_the_enum_draws_repo_wide() {
    let src = format!(
        "{MODE}pub fn one(m: Mode) {{ match m {{ Mode::Overlay => unreachable!(), \
         Mode::Draw => draw() }} }}\npub fn two(m: Mode) {{ match m {{ \
         Mode::Draw => panic!(\"no draw\"), Mode::Overlay => go() }} }}\n"
    );
    let found = run_rs_rule("21", &krate(&src));

    assert_eq!(
        found
            .iter()
            .map(|f| f.site.symbol.to_string())
            .collect::<Vec<String>>(),
        ["demo_crate::one", "demo_crate::two"]
    );
    assert_eq!(
        found.iter().map(|f| f.salience).collect::<Vec<f64>>(),
        [2.0, 2.0]
    );
}
