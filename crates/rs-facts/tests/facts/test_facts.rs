//! Port of `tests/rs/test_facts.py`: what `build_facts` indexes off a Cargo
//! root. Qnames from the file layout, symbols with their kinds, name-level
//! refs and call resolution through `use` bindings, comments from the CST,
//! the test readings, and a parse that keeps what it could read.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use camino::Utf8Path;
use indexmap::IndexSet;
use sightline_core::config::{Config, load_config};
use sightline_core::findings::{Evidence, Finding, Rel, Site};
use sightline_core::lang::{Language, Stack, detect};
use sightline_core::suppress::suppress;
use sightline_core::walk;
use sightline_rs_facts::build::{RsBuilt, build_facts};
use sightline_rs_facts::model::{RefKind, Resolution, RsFacts};
use sightline_testkit::{
    MANIFEST, PyLanguage, RsLanguage, RsStack, build_rs, build_rs_with, make_repo, registry,
};
use tempfile::TempDir;

const WORKSPACE: &str = "[workspace]\nmembers = [\"crates/other\"]\n[package]\nname = \"demo-crate\"\nversion = \"0.1.0\"\n";
const OTHER: &str = "[package]\nname = \"other-crate\"\nversion = \"0.1.0\"\n";
const LIB: &str = "[package]\nname = \"demo-crate\"\nversion = \"0.1.0\"\n[lib]\n";
const UNPUBLISHED: &str =
    "[package]\nname = \"demo-crate\"\nversion = \"0.1.0\"\npublish = false\n[lib]\n";

fn rels_to_qnames(facts: &RsFacts<'_>) -> BTreeMap<String, String> {
    facts
        .modules
        .values()
        .map(|m| (m.rel.to_string(), m.qname.to_string()))
        .collect()
}

fn module_qnames(facts: &RsFacts<'_>) -> Vec<String> {
    facts.modules.keys().map(|q| q.to_string()).collect()
}

fn published(facts: &RsFacts<'_>) -> BTreeSet<String> {
    facts.published.iter().map(|q| q.to_string()).collect()
}

/// `_resolutions`: the call's own text up to its arguments, to how it
/// resolved.
fn resolutions(facts: &RsFacts<'_>) -> HashMap<String, Resolution> {
    facts
        .call_sites
        .iter()
        .map(|c| {
            let text = facts.modules[&c.module].text(c.node);
            (
                text.split('(').next().unwrap_or_default().to_string(),
                c.resolution,
            )
        })
        .collect()
}

fn call_text(facts: &RsFacts<'_>, site: usize) -> String {
    let c = &facts.call_sites[site];
    facts.modules[&c.module].text(c.node).into_owned()
}

/// `rs_repo` without `build_rs`'s manifest: a fixture whose crates all sit
/// below the root, so no `[package]` is written at the top.
fn build_bare(files: &[(&str, &str)], config: Config) -> (TempDir, RsBuilt) {
    let dir = make_repo(files);
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let listing = walk::discover(root, &config);
    let built = build_facts(root, &config, &listing, None);
    (dir, built)
}

// --- qnames ------------------------------------------------------------------

#[test]
fn qnames_come_from_the_file_layout_and_the_crate_name() {
    let (_dir, stack) = build_rs(&[
        ("src/lib.rs", "pub mod util;\n"),
        ("src/util.rs", "pub fn a() {}\n"),
        ("src/deep/mod.rs", "pub fn b() {}\n"),
        ("src/deep/inner.rs", "pub fn c() {}\n"),
        ("src/main.rs", "fn main() {}\n"),
        ("tests/it.rs", "fn t() {}\n"),
    ]);

    assert_eq!(
        rels_to_qnames(stack.facts()),
        BTreeMap::from([
            ("src/lib.rs".into(), "demo_crate".into()),
            ("src/util.rs".into(), "demo_crate::util".into()),
            ("src/deep/mod.rs".into(), "demo_crate::deep".into()),
            ("src/deep/inner.rs".into(), "demo_crate::deep::inner".into()),
            // lib.rs took the root
            ("src/main.rs".into(), "demo_crate::main".into()),
            ("tests/it.rs".into(), "demo_crate::tests::it".into()),
        ])
    );
}

#[test]
fn main_is_the_root_where_no_lib_claims_it() {
    let (_dir, stack) = build_rs(&[("src/main.rs", "fn main() {}\n")]);
    assert_eq!(module_qnames(stack.facts()), ["demo_crate"]);
}

#[test]
fn every_package_manifest_is_its_own_qname_root() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", WORKSPACE),
        ("src/lib.rs", "pub fn a() {}\n"),
        ("crates/other/Cargo.toml", OTHER),
        ("crates/other/src/lib.rs", "pub fn b() {}\n"),
        ("crates/other/src/net.rs", "pub fn c() {}\n"),
    ]);
    let facts = stack.facts();

    assert_eq!(
        module_qnames(facts).into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "demo_crate".to_string(),
            "other_crate".to_string(),
            "other_crate::net".to_string(),
        ])
    );
    assert_eq!(
        facts
            .crates
            .iter()
            .map(|(c, d)| (c.clone(), d.clone()))
            .collect::<BTreeMap<_, _>>(),
        BTreeMap::from([
            ("demo_crate".to_string(), String::new()),
            ("other_crate".to_string(), "crates/other".to_string()),
        ])
    );
}

/// A clone whose crates sit below the root with no manifest and no
/// workspace there, beside a Python tree of its own (cr-sqlite's shape).
const NESTED: [(&str, &str); 7] = [
    (
        "core/rs/core/Cargo.toml",
        "[package]\nname = \"cr-core\"\nversion = \"0.1.0\"\n",
    ),
    ("core/rs/core/src/lib.rs", "pub mod tbl;\n"),
    ("core/rs/core/src/tbl.rs", "pub fn a() {}\n"),
    (
        "core/rs/core/target/debug/build/out.rs",
        "pub fn generated() {}\n",
    ),
    (
        "core/rs/bundle/Cargo.toml",
        "[package]\nname = \"cr-bundle\"\nversion = \"0.1.0\"\n",
    ),
    ("core/rs/bundle/src/lib.rs", "pub fn b() {}\n"),
    ("py/setup.py", ""),
];

#[test]
fn a_manifest_below_the_root_selects_the_stack() {
    let dir = make_repo(&NESTED);
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let (py, rs) = (PyLanguage::default(), RsLanguage::default());
    let registered: Vec<&dyn Language> = vec![&py, &rs];

    let names: Vec<&str> = detect(root, &registered).iter().map(|l| l.name()).collect();
    assert_eq!(names, ["py", "rs"]);
}

#[test]
fn every_crate_below_the_root_is_a_qname_root() {
    let (_dir, built) = build_bare(&NESTED, Config::new());
    let facts = built.borrow_dependent();

    // a build directory under a crate is no more auditable than under a root
    assert_eq!(
        rels_to_qnames(facts),
        BTreeMap::from([
            ("core/rs/bundle/src/lib.rs".into(), "cr_bundle".into()),
            ("core/rs/core/src/lib.rs".into(), "cr_core".into()),
            ("core/rs/core/src/tbl.rs".into(), "cr_core::tbl".into()),
        ])
    );
    assert_eq!(
        facts
            .crates
            .iter()
            .map(|(c, d)| (c.clone(), d.clone()))
            .collect::<BTreeMap<_, _>>(),
        BTreeMap::from([
            ("cr_bundle".to_string(), "core/rs/bundle".to_string()),
            ("cr_core".to_string(), "core/rs/core".to_string()),
        ])
    );
    assert_eq!(&*facts.symbols["cr_core::tbl::a"].module, "cr_core::tbl");
}

#[test]
fn an_inline_mod_nests_the_scope() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", "pub mod nest {\n    pub fn deep() {}\n}\n")]);
    assert!(stack.facts().symbols.contains_key("demo_crate::nest::deep"));
}

#[test]
fn a_path_attribute_is_unsupported_and_named_in_the_header() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", "#[path = \"elsewhere.rs\"]\nmod x;\n")]);

    assert_eq!(stack.notes(), ["rs: #[path] not followed: src/lib.rs:2"]);
}

// --- symbols -----------------------------------------------------------------

const SYMBOL_SOURCE: &str = concat!(
    "pub struct Point { x: i32 }\n",
    "pub(crate) enum E { A }\n",
    "pub trait Greet { fn hi(&self); }\n",
    "pub type Alias = i32;\n",
    "pub const K: i32 = 1;\n",
    "static S: i32 = 2;\n",
    "macro_rules! mac { () => {} }\n",
    "pub fn run() {}\n",
    "impl Point {\n    pub fn new() -> Self { Point { x: 0 } }\n}\n",
    "impl Greet for Point {\n    fn hi(&self) {}\n}\n",
);

#[test]
fn every_symbol_kind_is_indexed_with_its_visibility() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", SYMBOL_SOURCE)]);

    let got: BTreeMap<String, (&str, bool)> = stack
        .facts()
        .symbols
        .iter()
        .map(|(q, s)| (q.to_string(), (s.kind, s.is_public)))
        .collect();
    assert_eq!(
        got,
        BTreeMap::from([
            ("demo_crate::Point".to_string(), ("struct", true)),
            // pub(crate) is not bare pub
            ("demo_crate::E".to_string(), ("enum", false)),
            ("demo_crate::Greet".to_string(), ("trait", true)),
            ("demo_crate::Alias".to_string(), ("type", true)),
            ("demo_crate::K".to_string(), ("const", true)),
            ("demo_crate::S".to_string(), ("static", false)),
            ("demo_crate::mac".to_string(), ("macro", false)),
            ("demo_crate::run".to_string(), ("function", true)),
            ("demo_crate::Point::new".to_string(), ("method", true)),
            ("demo_crate::Point::hi".to_string(), ("method", false)),
        ])
    );
}

#[test]
fn a_trait_impls_method_keys_by_type_and_records_the_trait() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", SYMBOL_SOURCE)]);
    let facts = stack.facts();

    assert_eq!(facts.symbols["demo_crate::Point::hi"].traits, ["Greet"]);
    assert!(facts.symbols["demo_crate::Point::new"].traits.is_empty());
    // the `RsProvers.trait_impls["Greet"] == ("demo_crate::Point",)` half
    // waits for the provers unit's memos
    assert_eq!(facts.impls.len(), 2);
    assert_eq!(facts.impls[1].trait_name.as_deref(), Some("Greet"));
    assert_eq!(facts.impls[1].type_qname, "demo_crate::Point");
}

/// `impl Sum for &[Attribute]` keys under the element type: without it the
/// whole block is dropped and its methods never enter the symbol table.
#[test]
fn an_impl_on_a_slice_keeps_its_methods() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        concat!(
            "pub struct Attribute;\ntrait Sum { fn total(&self) -> usize; }\n",
            "impl Sum for &'_ [Attribute] { fn total(&self) -> usize { 0 } }\n",
        ),
    )]);

    assert_eq!(
        stack.facts().symbols["demo_crate::Attribute::total"].traits,
        ["Sum"]
    );
}

#[test]
fn a_symbol_span_is_its_items_own_lines() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", "\n\npub fn a() {\n}\n")]);

    let sym = &stack.facts().symbols["demo_crate::a"];
    assert_eq!((sym.lineno, sym.end_lineno), (3, 4));
}

// --- refs and calls ----------------------------------------------------------

const CALLER: &str = concat!(
    "use crate::util::helper;\n",
    "use std::collections::HashMap;\n\n",
    "pub struct A;\n",
    "impl A { pub fn shared(&self) {} }\n\n",
    "pub fn run(v: A, m: HashMap<i32, i32>) {\n",
    "    helper();\n",
    "    crate::util::helper();\n",
    "    v.shared();\n",
    "    m.len();\n",
    "    absent();\n",
    "    String::new();\n",
    "}\n",
);

const UTIL: &str = "pub fn helper() {}\npub struct B;\nimpl B { pub fn shared(&self) {} }\n";

fn caller_repo() -> (TempDir, RsStack) {
    let lib = format!("pub mod util;\n{CALLER}");
    build_rs(&[("src/util.rs", UTIL), ("src/lib.rs", &lib)])
}

#[test]
fn a_use_binding_and_a_full_path_both_resolve() {
    let (_dir, stack) = caller_repo();
    let facts = stack.facts();
    let by_text = resolutions(facts);

    assert_eq!(by_text["helper"], Resolution::Resolved);
    assert_eq!(by_text["crate::util::helper"], Resolution::Resolved);
    let target = facts
        .call_sites
        .iter()
        .enumerate()
        .find(|(i, _)| call_text(facts, *i).starts_with("helper"))
        .map(|(_, c)| c.target.clone())
        .unwrap();
    assert_eq!(target.as_deref(), Some("demo_crate::util::helper"));
}

#[test]
fn a_uniform_use_path_roots_at_the_module_it_names() {
    let (_dir, stack) = build_rs(&[
        ("src/dns.rs", "pub struct ToIpAddrs;\n"),
        (
            "src/lib.rs",
            "pub mod dns;\nuse dns::ToIpAddrs;\npub fn run(a: ToIpAddrs) {}\n",
        ),
    ]);
    let facts = stack.facts();

    assert_eq!(
        facts.modules["demo_crate"].bindings["ToIpAddrs"],
        "demo_crate::dns::ToIpAddrs"
    );
    // the ref joins the real qname
    assert_eq!(facts.fan_in["demo_crate::dns"], 1);
}

/// Two same-named methods pin no target, and the call is still one this repo
/// owns (rs #42's reading).
#[test]
fn a_method_call_is_by_name_where_the_repo_owns_the_name() {
    let (_dir, stack) = caller_repo();
    let facts = stack.facts();

    let shared = facts
        .call_sites
        .iter()
        .enumerate()
        .find(|(i, _)| call_text(facts, *i) == "v.shared()")
        .map(|(_, c)| c)
        .unwrap();
    assert_eq!(shared.resolution, Resolution::ByName);
    assert!(shared.target.is_none());
}

#[test]
fn a_call_is_external_only_where_a_name_says_it_lives_outside() {
    let (_dir, stack) = caller_repo();
    let by_text = resolutions(stack.facts());

    // a prelude root
    assert_eq!(by_text["String::new"], Resolution::External);
    // no candidate is not evidence
    assert_eq!(by_text["m.len"], Resolution::Unresolved);
    assert_eq!(by_text["absent"], Resolution::Unresolved);
}

#[test]
fn a_self_qualified_call_is_in_repo() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        concat!(
            "pub struct A;\n",
            "impl A {\n",
            "    pub fn new() -> A { A }\n",
            "    pub fn make() -> A { Self::new() }\n",
            "}\n",
        ),
    )]);

    assert_eq!(
        resolutions(stack.facts())["Self::new"],
        Resolution::Unresolved
    );
}

#[test]
fn refs_are_name_level_occurrences_with_their_kind() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        concat!(
            "pub fn run(a: i32) {\n    let mut b = a;\n    b = helper();\n}\n",
            "pub fn helper() -> i32 { 1 }\n",
        ),
    )]);
    let kinds: BTreeSet<(String, &str)> = stack
        .facts()
        .refs
        .iter()
        .map(|r| (r.target.clone(), r.kind.value()))
        .collect();

    assert!(kinds.contains(&("demo_crate::helper".to_string(), RefKind::Callee.value())));
    assert!(kinds.contains(&("b".to_string(), RefKind::Store.value())));
    assert!(kinds.contains(&("a".to_string(), RefKind::Load.value())));
}

#[test]
fn fan_in_counts_inbound_cross_module_refs() {
    let (_dir, stack) = build_rs(&[
        ("src/util.rs", "pub fn helper() {}\n"),
        (
            "src/lib.rs",
            "pub mod util;\nuse crate::util::helper;\npub fn run() { helper(); }\n",
        ),
    ]);

    assert_eq!(stack.facts().fan_in["demo_crate::util"], 1);
}

// --- comments ----------------------------------------------------------------

#[test]
fn comment_kinds_come_from_the_cst() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "//! what this module is\n// a plain note\n/// what `a` is\npub fn a() {}\n",
    )]);
    let module = &stack.facts().modules["demo_crate"];

    assert_eq!(
        module
            .comments
            .iter()
            .map(|c| (c.line, c.kind))
            .collect::<Vec<_>>(),
        [(1, "module-doc"), (2, "comment"), (3, "doc")]
    );
    assert_eq!(module.doc, ["what this module is"]);
}

#[test]
fn a_doc_comments_text_drops_its_marker_and_nothing_else() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "//!*the* map\n/** what `a` is */\npub fn a() {}\n",
    )]);
    let module = &stack.facts().modules["demo_crate"];

    assert_eq!(module.doc, ["*the* map"]);
    assert_eq!(
        module
            .comments
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>(),
        ["*the* map", "what `a` is"]
    );
}

#[test]
fn a_module_without_an_inner_doc_has_none() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", "/// item doc\npub fn a() {}\n")]);
    assert!(stack.facts().modules["demo_crate"].doc.is_empty());
}

// --- the test readings -------------------------------------------------------

#[test]
fn is_test_reads_the_path_dirs() {
    let (_dir, stack) = build_rs(&[
        ("src/lib.rs", "pub fn a() {}\n"),
        ("tests/it.rs", "fn t() {}\n"),
        ("benches/b.rs", "fn t() {}\n"),
        ("examples/e.rs", "fn main() {}\n"),
    ]);
    let facts = stack.facts();

    assert!(!facts.is_test("src/lib.rs"));
    for rel in ["tests/it.rs", "benches/b.rs", "examples/e.rs"] {
        assert!(facts.is_test(rel), "{rel}");
    }
}

#[test]
fn a_cfg_test_item_and_a_test_fn_are_tests() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        concat!(
            "pub fn prod() {}\n\n#[cfg(test)]\nmod tests {\n",
            "    #[test]\n    fn one() {}\n    fn helper() {}\n}\n",
        ),
    )]);
    let facts = stack.facts();

    assert!(!facts.symbols["demo_crate::prod"].is_test);
    // #[test]
    assert!(facts.symbols["demo_crate::tests::one"].is_test);
    // under #[cfg(test)]
    assert!(facts.symbols["demo_crate::tests::helper"].is_test);
}

/// `#[cfg(test)] mod tests;` makes the file it declares test code, and a
/// `#[cfg(feature)]` declaration hands its cfg to every item of the file.
#[test]
fn a_cfg_on_a_file_mod_declaration_reaches_the_file() {
    let (_dir, stack) = build_rs(&[
        (
            "src/lib.rs",
            "#[cfg(test)]\nmod tests;\n#[cfg(feature = \"x\")]\npub mod gated;\n",
        ),
        ("src/tests.rs", "pub fn helper() {}\n"),
        ("src/gated.rs", "pub struct G;\nimpl G { pub fn m() {} }\n"),
    ]);
    let facts = stack.facts();

    assert!(facts.symbols["demo_crate::tests::helper"].is_test);
    assert_eq!(
        facts.symbols["demo_crate::gated::G"].attrs,
        ["cfg(feature = \"x\")"]
    );
    assert_eq!(
        facts.symbols["demo_crate::gated::G::m"].attrs,
        ["cfg(feature = \"x\")"]
    );
}

#[test]
fn an_attribute_reaches_its_item_through_a_comment() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "#[test]\n// a note between the two\nfn one() {}\n",
    )]);
    let sym = &stack.facts().symbols["demo_crate::one"];

    assert_eq!(sym.attrs, ["test"]);
    assert!(sym.is_test);
}

// --- a broken parse ----------------------------------------------------------

#[test]
fn an_error_parse_indexes_the_rest_and_joins_errors() {
    let (_dir, stack) = build_rs(&[("src/lib.rs", "pub fn a() {}\n}}}\npub fn b() {}\n")]);
    let facts = stack.facts();

    assert_eq!(facts.errors, ["src/lib.rs: parse error (line 2)"]);
    assert_eq!(
        facts
            .symbols
            .keys()
            .map(|q| q.to_string())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["demo_crate::a".to_string(), "demo_crate::b".to_string()])
    );
    assert_eq!(
        stack.provenance()["rs"]["parse_errors"],
        serde_json::json!(1)
    );
}

// --- suppression -------------------------------------------------------------

fn finding(rel: &str, line: u32) -> Finding {
    Finding {
        rule: "11",
        site: Site {
            rel: Rel::from(rel),
            line,
            col: 0,
            symbol: "".into(),
        },
        message: "x".into(),
        cause: "c".into(),
        evidence: Evidence::Idx { detail: "d".into() },
        salience: 0.0,
        fix: None,
        lang: "rs",
    }
}

#[test]
fn a_slash_marker_suppresses_by_id_and_by_slug() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        concat!(
            "pub fn a() {}  // sightline-ok: 11\n",
            "// sightline-ok: structural-clones\npub fn b() {}\n",
            "pub fn c() {}\n",
        ),
    )]);
    let findings: Vec<Finding> = [1, 3, 4]
        .iter()
        .map(|n| finding("src/lib.rs", *n))
        .collect();

    let (kept, dropped) = suppress(findings, stack.neutral(), &registry().id_by_slug);

    assert_eq!(
        dropped.iter().map(|f| f.site.line).collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(kept.iter().map(|f| f.site.line).collect::<Vec<_>>(), [4]);
}

// --- single-file facts -------------------------------------------------------

const SINGLE: [(&str, &str); 3] = [
    ("Cargo.toml", MANIFEST),
    ("src/util.rs", "// note\npub fn helper() -> i32 { 1 }\n"),
    (
        "src/lib.rs",
        "pub mod util;\nuse crate::util::helper;\npub fn run() { helper(); }\n",
    ),
];

type Shape = (
    Vec<(String, &'static str, u32, u32, bool)>,
    Vec<(String, &'static str, u32)>,
    Vec<(u32, &'static str, String)>,
);

fn shape(facts: &RsFacts<'_>, qname: &str) -> Shape {
    let mut symbols: Vec<(String, &'static str, u32, u32, bool)> = facts
        .symbols
        .values()
        .filter(|s| &*s.module == qname)
        .map(|s| {
            (
                s.qname.to_string(),
                s.kind,
                s.lineno,
                s.end_lineno,
                s.is_public,
            )
        })
        .collect();
    symbols.sort();
    let mut refs: Vec<(String, &'static str, u32)> = facts
        .refs
        .iter()
        .filter(|r| &*r.module == qname)
        .map(|r| (r.target.clone(), r.kind.value(), r.lineno))
        .collect();
    refs.sort();
    let comments = facts.modules[qname]
        .comments
        .iter()
        .map(|c| (c.line, c.kind, c.text.clone()))
        .collect();
    (symbols, refs, comments)
}

fn build_only(dir: &TempDir, only: &str) -> RsBuilt {
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let config = Config::new();
    let listing = walk::discover(root, &config);
    let set: IndexSet<Rel> = IndexSet::from([Rel::from(only)]);
    build_facts(root, &config, &listing, Some(&set))
}

#[test]
fn single_file_facts_equal_the_full_builds_for_that_file() {
    let (dir, stack) = build_rs(&SINGLE);
    let full = stack.facts();

    // `src/lib.rs` is the module with a child: its scope must not need the
    // child loaded
    for (rel, qname) in [
        ("src/util.rs", "demo_crate::util"),
        ("src/lib.rs", "demo_crate"),
    ] {
        let one = build_only(&dir, rel);
        let facts = one.borrow_dependent();

        assert_eq!(module_qnames(facts), [qname]);
        assert_eq!(shape(facts, qname), shape(full, qname));
    }
}

// --- config ------------------------------------------------------------------

#[test]
fn a_sightline_toml_configures_a_root_with_no_pyproject() {
    let dir = make_repo(&[
        (
            "sightline.toml",
            "[tool.sightline]\nexcludes = [\"src/skip\"]\n",
        ),
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn a() {}\n"),
        ("src/skip/gone.rs", "pub fn b() {}\n"),
    ]);
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let config = load_config(root, None);
    let listing = walk::discover(root, &config);
    let built = build_facts(root, &config, &listing, None);

    assert_eq!(module_qnames(built.borrow_dependent()), ["demo_crate"]);
}

#[test]
fn target_is_never_walked() {
    let (_dir, stack) = build_rs(&[
        ("src/lib.rs", "pub fn a() {}\n"),
        ("target/debug/build/gen.rs", "pub fn generated() {}\n"),
    ]);

    assert_eq!(module_qnames(stack.facts()), ["demo_crate"]);
}

// --- published and re-exports ------------------------------------------------

const LIB_TREE: [(&str, &str); 3] = [
    (
        "src/lib.rs",
        concat!(
            "pub mod api;\n",
            "mod hidden;\n",
            "pub use hidden::Deep;\n",
            "pub fn root_fn() {}\n",
            "pub(crate) fn crate_fn() {}\n",
        ),
    ),
    (
        "src/api.rs",
        concat!(
            "pub fn open() {}\n",
            "pub struct Handle;\n",
            "impl Handle { pub fn go(&self) {} fn shut(&self) {} }\n",
        ),
    ),
    ("src/hidden.rs", "pub struct Deep;\npub fn buried() {}\n"),
];

const BIN_TREE: [(&str, &str); 1] = [("src/main.rs", "pub fn helper() {}\nfn main() {}\n")];

fn with_manifest(
    manifest: &'static str,
    files: &[(&'static str, &'static str)],
) -> Vec<(&'static str, &'static str)> {
    let mut out = vec![("Cargo.toml", manifest)];
    out.extend_from_slice(files);
    out
}

#[test]
fn a_lib_crate_publishes_what_its_root_reaches() {
    let (_dir, stack) = build_rs(&with_manifest(LIB, &LIB_TREE));
    let facts = stack.facts();

    assert_eq!(
        published(facts),
        BTreeSet::from([
            "demo_crate::root_fn".to_string(),
            "demo_crate::api::open".to_string(),
            "demo_crate::api::Handle".to_string(),
            "demo_crate::api::Handle::go".to_string(),
            // the `pub use` names the definition
            "demo_crate::hidden::Deep".to_string(),
        ])
    );
    assert!(facts.publishes(&facts.symbols["demo_crate::api::Handle::go"]));
    assert!(!facts.publishes(&facts.symbols["demo_crate::hidden::buried"]));
    assert!(!facts.publishes(&facts.symbols["demo_crate::crate_fn"]));
}

#[test]
fn a_glob_re_export_publishes_the_module_it_names() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", LIB),
        ("src/lib.rs", "mod inner;\npub use inner::*;\n"),
        (
            "src/inner.rs",
            "pub fn shown() {}\npub(crate) fn kept() {}\n",
        ),
    ]);

    assert_eq!(
        published(stack.facts()),
        BTreeSet::from(["demo_crate::inner::shown".to_string()])
    );
}

#[test]
fn a_bin_crate_publishes_nothing() {
    let (_dir, stack) = build_rs(&with_manifest(MANIFEST, &BIN_TREE));
    assert!(stack.facts().published.is_empty());
}

#[test]
fn a_crate_that_says_publish_false_publishes_nothing() {
    let (_dir, stack) = build_rs(&with_manifest(UNPUBLISHED, &LIB_TREE));
    assert!(stack.facts().published.is_empty());
}

#[test]
fn the_config_override_silences_a_lib_crate() {
    let config = Config {
        published: Some(false),
        ..Config::new()
    };
    let (_dir, stack) = build_rs_with(&with_manifest(LIB, &LIB_TREE), config);
    assert!(stack.facts().published.is_empty());
}

#[test]
fn the_config_override_publishes_a_crate_whose_manifest_is_silent() {
    let config = Config {
        published: Some(true),
        ..Config::new()
    };
    let (_dir, stack) = build_rs_with(&with_manifest(MANIFEST, &BIN_TREE), config);

    assert_eq!(
        published(stack.facts()),
        BTreeSet::from(["demo_crate::helper".to_string()])
    );
}

#[test]
fn a_workspace_publishes_per_crate() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/lib/Cargo.toml",
            "[package]\nname = \"lib-one\"\nversion = \"0.1.0\"\n",
        ),
        ("crates/lib/src/lib.rs", "pub fn shipped() {}\n"),
        (
            "crates/app/Cargo.toml",
            "[package]\nname = \"app-one\"\nversion = \"0.1.0\"\n",
        ),
        (
            "crates/app/src/main.rs",
            "pub fn local() {}\nfn main() {}\n",
        ),
    ]);

    assert_eq!(
        published(stack.facts()),
        BTreeSet::from(["lib_one::shipped".to_string()])
    );
}

/// A `lib.rs` beside the crate's own bin is there so the bin and the
/// integration tests can share modules, not for a downstream user.
#[test]
fn an_application_publishes_nothing() {
    let mut files = with_manifest(LIB, &LIB_TREE);
    files.push(("src/main.rs", "fn main() {}\n"));
    let (_dir, stack) = build_rs(&files);

    assert!(stack.facts().published.is_empty());
}

/// A dependent inside the tree is a downstream user, bin target or not. The
/// second row is the workspace spelling, which a member inherits by
/// `workspace = true`.
#[test]
fn a_crate_a_sibling_path_depends_on_publishes_beside_its_bin() {
    for (root_dep, member_dep) in [
        ("", "\n[dependencies]\nlib-one = { path = \"../lib\" }\n"),
        (
            "\n[workspace.dependencies]\nlib-one = { path = \"crates/lib\" }\n",
            "\n[dependencies]\nlib-one = { workspace = true }\n",
        ),
    ] {
        let root = format!("[workspace]\nmembers = [\"crates/*\"]\n{root_dep}");
        let app = format!("[package]\nname = \"app-one\"\nversion = \"0.1.0\"\n{member_dep}");
        let (_dir, stack) = build_rs(&[
            ("Cargo.toml", root.as_str()),
            (
                "crates/lib/Cargo.toml",
                "[package]\nname = \"lib-one\"\nversion = \"0.1.0\"\n\n[[bin]]\nname = \"one\"\npath = \"src/bin/one.rs\"\n",
            ),
            ("crates/lib/src/lib.rs", "pub fn shipped() {}\n"),
            ("crates/lib/src/bin/one.rs", "fn main() {}\n"),
            ("crates/app/Cargo.toml", app.as_str()),
            ("crates/app/src/main.rs", "fn main() {}\n"),
        ]);

        assert_eq!(
            published(stack.facts()),
            BTreeSet::from(["lib_one::shipped".to_string()]),
            "{member_dep}"
        );
    }
}

const ALIAS_TREE: [(&str, &str); 5] = [
    ("Cargo.toml", LIB),
    (
        "src/lib.rs",
        "pub mod bpe;\npub mod user;\npub use crate::bpe::Tokenizer;\n",
    ),
    (
        "src/bpe/mod.rs",
        "pub mod tiktoken;\npub use tiktoken::Tokenizer;\n",
    ),
    (
        "src/bpe/tiktoken.rs",
        "pub struct Tokenizer;\nimpl Tokenizer { pub fn new() {} }\n",
    ),
    (
        "src/user.rs",
        "use crate::Tokenizer;\npub fn go(t: &Tokenizer) { Tokenizer::new(); }\n",
    ),
];

#[test]
fn a_reference_through_a_pub_use_alias_counts_toward_the_definition() {
    let (_dir, stack) = build_rs(&ALIAS_TREE);
    let facts = stack.facts();
    let defined = "demo_crate::bpe::tiktoken::Tokenizer";

    assert_eq!(facts.aliases["demo_crate::Tokenizer"], defined);
    assert_eq!(facts.aliases["demo_crate::bpe::Tokenizer"], defined);
    // the type's own ref and the one the `Tokenizer::new()` path spells
    assert_eq!(
        facts
            .refs_of(defined)
            .filter(|r| &*r.module != "demo_crate::bpe::tiktoken")
            .map(|r| r.module.to_string())
            .collect::<Vec<_>>(),
        ["demo_crate::user"]
    );
    assert_eq!(
        facts
            .refs_of(&format!("{defined}::new"))
            .map(|r| r.module.to_string())
            .collect::<Vec<_>>(),
        ["demo_crate::user"]
    );
    assert_eq!(facts.fan_in["demo_crate::bpe::tiktoken"], 2);
}

/// Rust's namespaces are separate: salvo re-exports a `handler` macro beside
/// its own `handler` module, and the module keeps the name.
#[test]
fn a_spelling_the_repo_already_defines_is_not_an_alias() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/one/Cargo.toml",
            "[package]\nname = \"one\"\nversion = \"0.1.0\"\n",
        ),
        (
            "crates/one/src/lib.rs",
            "pub mod handler;\npub use two::handler;\n",
        ),
        ("crates/one/src/handler.rs", "pub struct Handler;\n"),
        (
            "crates/two/Cargo.toml",
            "[package]\nname = \"two\"\nversion = \"0.1.0\"\n",
        ),
        ("crates/two/src/lib.rs", "pub mod handler;\n"),
        ("crates/two/src/handler.rs", "pub fn handler() {}\n"),
        (
            "crates/one/src/user.rs",
            "use crate::handler::Handler;\npub fn go(h: Handler) {}\n",
        ),
    ]);
    let facts = stack.facts();

    assert!(!facts.aliases.contains_key("one::handler"));
    assert!(
        facts
            .refs_of("one::handler::Handler")
            .any(|r| &*r.module == "one::user")
    );
}

#[test]
fn an_alias_naming_a_definition_outside_the_repo_is_not_one() {
    let (_dir, stack) = build_rs(&[
        ("Cargo.toml", LIB),
        ("src/lib.rs", "pub use tokio::runtime::Runtime;\n"),
    ]);
    let facts = stack.facts();

    assert!(facts.aliases.is_empty());
    assert!(facts.published.is_empty());
}
