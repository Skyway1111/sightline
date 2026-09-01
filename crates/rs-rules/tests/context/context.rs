//! `tests/rs/test_rules_context.py`: #27, #29 and #59 over Rust facts,
//! paired firing and silent shapes per rule plus the exemption each arm
//! names.

use indexmap::IndexSet;
use tempfile::TempDir;

use sightline_core::config::Config;
use sightline_core::findings::{Finding, Rel};
use sightline_testkit::rs_fixtures::borrowed;
use sightline_testkit::{RsStack, build_rs_stack, rs_answers, run_rs_rule, run_rs_rule_on};

/// `(caller, callee, rel, line, is-a-call)`, as the graph would have
/// answered it.
type Edge = (&'static str, &'static str, &'static str, u32, bool);

const PAD: &str = "// pad\n";

fn pad(body: &str, lines: usize) -> String {
    body.to_string() + &PAD.repeat(lines.saturating_sub(body.matches('\n').count()))
}

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

fn symbols(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| &*f.site.symbol).collect()
}

/// `run_rs_rule` with the two knobs the Python fixture takes: the rows the
/// oracle's graph would have answered, and the files single-file facts are
/// built from.
fn run_on(id: &str, files: &[(&str, &str)], edges: &[Edge], only: &[&str]) -> Vec<Finding> {
    let (_dir, stack) = stack_of(files, edges, only);
    run_rs_rule_on(id, &stack)
}

fn stack_of(files: &[(&str, &str)], edges: &[Edge], only: &[&str]) -> (TempDir, RsStack) {
    let picked: IndexSet<Rel> = only.iter().map(|rel| Rel::from(*rel)).collect();
    build_rs_stack(
        files,
        Config::new(),
        (!picked.is_empty()).then_some(&picked),
        rs_answers(edges, &[]),
    )
}

// --- #27 purchase price ------------------------------------------------------

/// One hot type spanning `span` lines (#27's one-concept ceiling).
fn engine_struct(span: usize) -> String {
    format!(
        "pub struct Engine {{\n{}    a: i32,\n}}\n",
        "    // field\n".repeat(span - 2)
    )
}

/// Its one method, spanning `span` lines of the type's own price.
fn engine_impl(span: usize) -> String {
    format!(
        "impl Engine {{\n    pub fn run(&self) -> i32 {{\n{}        1\n    }}\n}}\n",
        "        // step\n".repeat(span - 3)
    )
}

const ENGINE_READER: &str = concat!(
    "use crate::engine::Engine;\n",
    "pub fn a(e: &Engine) -> i32 { Engine::run(e) }\n",
    "pub fn b(e: &Engine) -> i32 { Engine::run(e) }\n",
    "pub fn c(e: &Engine) -> i32 { Engine::run(e) }\n",
);

#[test]
fn rule_27_fires_on_a_big_module_whose_symbols_every_reader_pays_for() {
    let files = [
        ("src/lib.rs", "pub mod big;\npub mod reader;\n".to_string()),
        ("src/big.rs", pad("pub fn hot() -> i32 { 1 }\n", 520)),
        (
            "src/reader.rs",
            concat!(
                "use crate::big::hot;\n",
                "pub fn a() -> i32 { hot() }\n",
                "pub fn b() -> i32 { hot() }\n",
                "pub fn c() -> i32 { hot() }\n",
            )
            .to_string(),
        ),
    ];
    let findings = run_rs_rule("27", &borrowed(&files));

    let rows: Vec<(&str, &str, &str)> = findings
        .iter()
        .map(|f| (&*f.site.symbol, &*f.site.rel, f.cause.as_str()))
        .collect();
    assert_eq!(
        rows,
        [("demo_crate::big", "src/big.rs", "price:demo_crate::big")]
    );
    assert!(findings[0].message.contains("hot (3)"));
}

#[test]
fn rule_27_a_small_module_or_a_cold_one_is_silent() {
    let files = [
        (
            "src/lib.rs",
            "pub mod small;\npub mod cold;\npub mod reader;\n".to_string(),
        ),
        // past the fan-in bar, but a reader loads 30 lines to get it
        ("src/small.rs", pad("pub fn hot() -> i32 { 1 }\n", 30)),
        // big, but nothing outside leans on it three times
        ("src/cold.rs", pad("pub fn rare() -> i32 { 2 }\n", 520)),
        (
            "src/reader.rs",
            concat!(
                "use crate::small::hot;\nuse crate::cold::rare;\n",
                "pub fn a() -> i32 { hot() + rare() }\n",
                "pub fn b() -> i32 { hot() }\n",
                "pub fn c() -> i32 { hot() }\n",
            )
            .to_string(),
        ),
    ];

    assert_eq!(run_rs_rule("27", &borrowed(&files)), []);
}

/// Every hot symbol is a method of one struct, and the type costs 452 of the
/// file's 600 lines: there is nothing to lift out from under it.
#[test]
fn rule_27_a_module_that_is_one_small_type_is_already_the_smallest_unit() {
    let files = [
        (
            "src/lib.rs",
            "pub mod engine;\npub mod reader;\n".to_string(),
        ),
        (
            "src/engine.rs",
            pad(&(engine_struct(200) + &engine_impl(252)), 600),
        ),
        ("src/reader.rs", ENGINE_READER.to_string()),
    ];

    assert_eq!(run_rs_rule("27", &borrowed(&files)), []);
}

/// One type again, but its own span is a module's worth: the file is not the
/// smallest unit of anything, the type is what to split.
#[test]
fn rule_27_a_type_that_is_itself_a_module_is_the_thing_to_lift_from() {
    let files = [
        (
            "src/lib.rs",
            "pub mod engine;\npub mod reader;\n".to_string(),
        ),
        (
            "src/engine.rs",
            pad(&(engine_struct(2) + &engine_impl(550)), 600),
        ),
        ("src/reader.rs", ENGINE_READER.to_string()),
    ];

    assert_eq!(
        symbols(&run_rs_rule("27", &borrowed(&files))),
        ["demo_crate::engine"]
    );
}

#[test]
fn rule_27_a_small_type_beside_free_functions_is_more_than_one_concept() {
    let files = [
        (
            "src/lib.rs",
            "pub mod engine;\npub mod reader;\n".to_string(),
        ),
        (
            "src/engine.rs",
            pad(
                &(engine_struct(100) + &engine_impl(10) + "pub fn free() -> i32 { 2 }\n"),
                600,
            ),
        ),
        (
            "src/reader.rs",
            ENGINE_READER.to_string()
                + concat!(
                    "use crate::engine::free;\n",
                    "pub fn d() -> i32 { free() }\n",
                    "pub fn e() -> i32 { free() }\n",
                    "pub fn f() -> i32 { free() }\n",
                ),
        ),
    ];

    assert_eq!(
        symbols(&run_rs_rule("27", &borrowed(&files))),
        ["demo_crate::engine"]
    );
}

/// Both hot, one line apart across the bar: 500 pays, 499 does not.
#[test]
fn rule_27_the_price_bar_is_the_line_it_is() {
    let files = [
        (
            "src/lib.rs",
            "pub mod at_bar;\npub mod under_bar;\npub mod reader;\n".to_string(),
        ),
        ("src/at_bar.rs", pad("pub fn hot_a() -> i32 { 1 }\n", 500)),
        (
            "src/under_bar.rs",
            pad("pub fn hot_b() -> i32 { 2 }\n", 499),
        ),
        (
            "src/reader.rs",
            concat!(
                "use crate::at_bar::hot_a;\nuse crate::under_bar::hot_b;\n",
                "pub fn a() -> i32 { hot_a() + hot_b() }\n",
                "pub fn b() -> i32 { hot_a() + hot_b() }\n",
                "pub fn c() -> i32 { hot_a() + hot_b() }\n",
            )
            .to_string(),
        ),
    ];
    let findings = run_rs_rule("27", &borrowed(&files));

    assert_eq!(symbols(&findings), ["demo_crate::at_bar"]);
    assert!(findings[0].message.contains("is 500 lines"));
}

// --- #29 top-loading ---------------------------------------------------------

#[test]
fn rule_29_fires_on_a_big_module_with_no_module_doc() {
    let files = [
        (
            "src/lib.rs",
            "pub mod wide;\npub mod configured;\n".to_string(),
        ),
        ("src/wide.rs", pad("pub fn a() {}\npub fn b() {}\n", 160)),
        // rustdoc configuration is not a header: it says nothing about what
        // the module is
        (
            "src/configured.rs",
            pad(
                concat!(
                    "#![doc(html_logo_url = \"https://example.com/logo.svg\")]\n",
                    "pub fn a() {}\n",
                ),
                160,
            ),
        ),
    ];
    let findings = run_rs_rule("29", &borrowed(&files));

    let mut found = symbols(&findings);
    found.sort_unstable();
    assert_eq!(found, ["demo_crate::configured", "demo_crate::wide"]);
    assert!(findings.iter().all(|f| f.site.line == 1));
    let wide = findings
        .iter()
        .find(|f| &*f.site.symbol == "demo_crate::wide")
        .expect("the wide module fired");
    assert!(wide.message.contains("160 lines, 2 top-level items"));
}

#[test]
fn rule_29_a_module_doc_or_a_short_module_is_silent() {
    let files = [
        (
            "src/lib.rs",
            "pub mod wide;\npub mod attr;\npub mod thin;\n".to_string(),
        ),
        (
            "src/wide.rs",
            pad("//! What this module is.\npub fn a() {}\n", 160),
        ),
        // a doc attribute is a `//!` header by another spelling; the rustdoc
        // configuration beside it says nothing about the module
        (
            "src/attr.rs",
            pad(
                concat!(
                    "#![doc = include_str!(\"../docs/attr.md\")]\n",
                    "#![doc(html_logo_url = \"https://example.com/logo.svg\")]\n",
                    "pub fn a() {}\n",
                ),
                160,
            ),
        ),
        ("src/thin.rs", pad("pub fn b() {}\n", 40)),
    ];

    assert_eq!(run_rs_rule("29", &borrowed(&files)), []);
}

/// Neither has a header, and one line separates them across the bar.
#[test]
fn rule_29_the_size_bar_is_the_line_it_is() {
    let files = [
        (
            "src/lib.rs",
            "pub mod at_bar;\npub mod under_bar;\n".to_string(),
        ),
        ("src/at_bar.rs", pad("pub fn a() {}\n", 150)),
        ("src/under_bar.rs", pad("pub fn b() {}\n", 149)),
    ];
    let findings = run_rs_rule("29", &borrowed(&files));

    assert_eq!(symbols(&findings), ["demo_crate::at_bar"]);
    assert!(findings[0].message.contains("150 lines"));
}

/// What `Scope::File` claims: the fast gate reads one file and gets the
/// findings the whole build would report for it.
#[test]
fn rule_29_single_file_facts_answer_what_the_full_build_answers() {
    let files = [
        ("src/lib.rs", "pub mod wide;\npub mod other;\n".to_string()),
        ("src/wide.rs", pad("pub fn a() {}\npub fn b() {}\n", 160)),
        ("src/other.rs", pad("pub fn c() {}\n", 200)),
    ];
    let target = "src/wide.rs";
    let rows = borrowed(&files);

    let full: Vec<Finding> = run_rs_rule("29", &rows)
        .into_iter()
        .filter(|f| &*f.site.rel == target)
        .collect();
    let single = run_on("29", &rows, &[], &[target]);

    assert!(!full.is_empty());
    assert_eq!(single, full);
}

#[test]
fn rule_29_a_test_file_is_not_an_entry_point_a_reader_budgets_for() {
    let files = [
        ("src/lib.rs", "pub fn a() {}\n".to_string()),
        ("tests/it.rs", pad("#[test]\nfn t() {}\n", 160)),
    ];

    assert_eq!(run_rs_rule("29", &borrowed(&files)), []);
}

// --- #59 entry-point cost docs -----------------------------------------------

/// 32 lines of body, so every fixture `main` is past the heavy span.
fn filler() -> String {
    (0..32).map(|i| format!("    let v{i} = {i};\n")).collect()
}

fn main_rs(body: &str, head: &str, doc: &str) -> Vec<(&'static str, String)> {
    vec![(
        "src/main.rs",
        format!(
            "{head}use std::process::Command;\n\n{doc}fn main() {{\n{}{body}}}\n",
            filler()
        ),
    )]
}

#[test]
fn rule_59_a_heavy_main_that_spends_declares_its_cost() {
    let files = main_rs("    Command::new(\"ls\");\n", "", "");
    let findings = run_rs_rule("59", &borrowed(&files));

    assert_eq!(causes(&findings), ["cost-docstring:demo_crate::main"]);
    assert_eq!(symbols(&findings), ["demo_crate::main"]);
    assert!(findings[0].message.contains("spends (Command::new)"));
    assert_eq!(findings[0].tier().value(), "indexed");
}

#[test]
fn rule_59_a_doc_on_the_fn_declares_it() {
    let files = main_rs("    Command::new(\"ls\");\n", "", "/// Runs the tool.\n");

    assert_eq!(run_rs_rule("59", &borrowed(&files)), []);
}

#[test]
fn rule_59_a_module_header_declares_it() {
    let files = main_rs("    Command::new(\"ls\");\n", "//! The tool.\n", "");

    assert_eq!(run_rs_rule("59", &borrowed(&files)), []);
}

#[test]
fn rule_59_a_main_under_the_heavy_span_is_one_screen() {
    let files = [(
        "src/main.rs",
        concat!(
            "use std::process::Command;\n\nfn main() {\n",
            "    Command::new(\"ls\");\n}\n",
        ),
    )];

    assert_eq!(run_rs_rule("59", &files), []);
}

#[test]
fn rule_59_a_main_that_spends_nothing_is_silent() {
    let files = main_rs("    compute();\n", "", "");

    assert_eq!(run_rs_rule("59", &borrowed(&files)), []);
}

#[test]
fn rule_59_a_spend_one_edge_hop_out_still_counts() {
    let source = format!(
        "use std::process::Command;\n\nfn main() {{\n{}    helper();\n}}\n\n\
         fn helper() {{\n    Command::new(\"ls\");\n}}\n",
        filler()
    );
    let edges: [Edge; 1] = [(
        "demo_crate::main",
        "demo_crate::helper",
        "src/main.rs",
        36,
        true,
    )];
    let findings = run_on("59", &[("src/main.rs", source.as_str())], &edges, &[]);

    assert_eq!(causes(&findings), ["cost-docstring:demo_crate::main"]);
    assert!(
        findings[0]
            .message
            .contains("spends (helper -> Command::new)")
    );
}

#[test]
fn rule_59_a_spend_three_hops_out_is_someone_elses_cost() {
    let source = format!(
        "use std::process::Command;\n\nfn main() {{\n{}    first();\n}}\n\n\
         fn first() {{\n    second();\n}}\n\n\
         fn second() {{\n    Command::new(\"ls\");\n}}\n",
        filler()
    );
    let edges: [Edge; 2] = [
        (
            "demo_crate::main",
            "demo_crate::first",
            "src/main.rs",
            36,
            true,
        ),
        (
            "demo_crate::first",
            "demo_crate::second",
            "src/main.rs",
            39,
            true,
        ),
    ];

    assert_eq!(
        run_on("59", &[("src/main.rs", source.as_str())], &edges, &[]),
        []
    );
}

#[test]
fn rule_59_a_fn_named_main_in_a_library_starts_no_binary() {
    let files = [(
        "src/lib.rs",
        format!(
            "use std::process::Command;\n\npub fn main() {{\n{}    Command::new(\"ls\");\n}}\n",
            filler()
        ),
    )];

    assert_eq!(run_rs_rule("59", &borrowed(&files)), []);
}

#[test]
fn rule_59_a_runtime_main_attribute_is_an_entry_point() {
    let files = [(
        "src/lib.rs",
        format!(
            "use std::process::Command;\n\n\
             #[tokio::main(flavor = \"current_thread\")]\n\
             pub async fn run() {{\n{}    Command::new(\"ls\");\n}}\n",
            filler()
        ),
    )];

    assert_eq!(
        causes(&run_rs_rule("59", &borrowed(&files))),
        ["cost-docstring:demo_crate::run"]
    );
}

/// doxx `main`: `spawn_blocking` runs in the process the reader holds.
#[test]
fn rule_59_a_thread_hop_is_no_cost_the_reader_walks_back() {
    let files = [(
        "src/main.rs",
        format!(
            "use tokio::task::spawn_blocking;\n\nfn main() {{\n{}    spawn_blocking(|| compute());\n}}\n",
            filler()
        ),
    )];

    assert_eq!(run_rs_rule("59", &borrowed(&files)), []);
}

/// tetro-tui `main`: a panic hook is a callback the process keeps, and the
/// reader who runs it is holding the process that changed.
#[test]
fn rule_59_a_setting_this_process_holds_is_no_spend() {
    let files = [(
        "src/main.rs",
        format!(
            "use std::panic;\n\nfn main() {{\n{}    panic::set_hook(Box::new(|_| {{}}));\n    \
             std::env::set_var(\"RUST_LOG\", \"info\");\n}}\n",
            filler()
        ),
    )];

    assert_eq!(run_rs_rule("59", &borrowed(&files)), []);
}
