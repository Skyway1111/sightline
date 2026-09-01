//! Family T over Rust: #42 assertion-free test, #47 sleepy test. Each rule
//! gets its firing shape, its silent sibling and the exemption the plan names.

use sightline_core::findings::Finding;
use sightline_testkit::run_rs_rule;

fn run(id: &str, body: &str) -> Vec<Finding> {
    run_rs_rule(id, &[("src/lib.rs", body)])
}

fn symbols(found: &[Finding]) -> Vec<&str> {
    found.iter().map(|f| &*f.site.symbol).collect()
}

// --- #42 assertion-free test --------------------------------------------------

#[test]
fn fires_on_a_test_that_only_calls() {
    let found = run(
        "42",
        "pub fn go() -> i32 { 1 }\n\
         #[cfg(test)]\nmod tests {\n\
         \x20   #[test]\n    fn t() { super::go(); }\n}\n",
    );

    assert_eq!(symbols(&found), ["demo_crate::tests::t"]);
    // the `fn` line: its attribute is a sibling
    assert_eq!(found[0].site.line, 5);
}

#[test]
fn is_silent_where_the_body_asserts() {
    assert!(run("42", "#[test]\nfn t() { assert_eq!(go(), 1); }\n").is_empty());
}

/// tree-sitter leaves macro tokens unparsed, so an assertion inside a
/// `try_join!` arm is an identifier in its token tree, not a macro of its own
/// - the body still asserts.
#[test]
fn reads_an_assertion_a_macro_carries() {
    assert!(
        run(
            "42",
            "#[test]\nfn t() {\n\
             \x20   join!(async { assert_eq!(reply, b\"hi\"); Ok(()) }).unwrap();\n}\n",
        )
        .is_empty()
    );
}

#[test]
fn is_silent_on_a_should_panic_test() {
    assert!(run("42", "#[test]\n#[should_panic]\nfn t() { go(); }\n").is_empty());
}

#[test]
fn reads_panic_only_under_a_condition() {
    let guarded = "#[test]\nfn t() { if go() { panic!(\"no\"); } }\n";
    let bare = "#[test]\nfn t() { panic!(\"not written yet\"); }\n";

    assert!(run("42", guarded).is_empty());
    assert_eq!(run("42", bare).len(), 1);
}

#[test]
fn reads_the_fallible_arm_only_in_a_result_test() {
    let result = "#[test]\nfn t() -> Result<(), String> { let v = go()?; Ok(()) }\n";
    let plain = "#[test]\nfn t() { go().unwrap(); }\n";

    assert!(run("42", result).is_empty());
    assert_eq!(run("42", plain).len(), 1);
}

/// `.unwrap()` on a call the repo owns is the suite's oracle - an `Err` out of
/// the code under test fails it - while the same call on stdlib only sets the
/// fixture up.
#[test]
fn reads_an_unwrap_on_the_call_under_test_as_the_verdict() {
    let setup = "#[test]\nfn t() { let _d = std::fs::read(\"in.txt\").unwrap(); }\n";
    let subject = "pub fn load(p: &str) -> Result<i32, String> { Ok(1) }\n\
                   #[test]\nfn t() { load(\"in.txt\").unwrap(); }\n";

    assert_eq!(run("42", setup).len(), 1);
    assert!(run("42", subject).is_empty());
}

#[test]
fn takes_a_test_helper_chain_by_name() {
    assert!(
        run(
            "42",
            "#[cfg(test)]\nmod tests {\n\
             \x20   fn check(v: i32) { assert!(v > 0); }\n\
             \x20   fn check_all(v: i32) { check(v); }\n\
             \x20   #[test]\n    fn t() { check_all(1); }\n}\n",
        )
        .is_empty()
    );
}

#[test]
fn reads_the_code_under_test_as_no_helper() {
    let found = run(
        "42",
        "pub fn remove(v: i32) { debug_assert!(v > 0); }\n\
         #[cfg(test)]\nmod tests {\n\
         \x20   #[test]\n    fn t() { super::remove(1); }\n}\n",
    );

    assert_eq!(symbols(&found), ["demo_crate::tests::t"]);
}

/// A suite that names a case after the method it exercises does not lend that
/// case's verdict to every test calling the method.
#[test]
fn reads_another_test_case_as_no_helper() {
    let found = run(
        "42",
        "#[test]\nfn remove() { assert!(go().is_ok()); }\n\
         #[test]\nfn toplevel() { let n = make(); n.remove(); }\n",
    );

    assert_eq!(symbols(&found), ["demo_crate::toplevel"]);
}

#[test]
fn is_silent_on_a_compile_only_test() {
    assert!(
        run(
            "42",
            "fn debug<T: std::fmt::Debug>() {}\n\
             #[test]\nfn bounds() { debug::<Sock>(); }\n\
             #[test]\nfn surface() { let _: fn(u32) -> Sock = Sock::new; }\n\
             #[test]\nfn shapes() {\n\
             \x20   fn check(s: &Sock) { let _: usize = s.len(); }\n\
             \x20   let _ = check;\n}\n",
        )
        .is_empty()
    );
}

#[test]
fn fires_where_a_statement_does_runtime_work() {
    let found = run(
        "42",
        "#[test]\nfn t() { let _: Sock = Sock::new(); let v = go(); }\n",
    );

    assert_eq!(symbols(&found), ["demo_crate::t"]);
}

// --- #47 sleepy test ----------------------------------------------------------

#[test]
fn fires_on_a_constant_sleep_in_a_test() {
    let found = run(
        "47",
        "#[test]\nfn t() {\n\
         \x20   std::thread::sleep(std::time::Duration::from_secs(1));\n\
         \x20   tokio::time::sleep(Duration::from_millis(50));\n}\n",
    );

    assert_eq!(
        found.iter().map(|f| f.site.line).collect::<Vec<_>>(),
        [3, 4]
    );
    assert!(found[0].message.starts_with("demo_crate::t sleeps 1s"));
    assert!(found[1].message.starts_with("demo_crate::t sleeps 0.05s"));
}

#[test]
fn is_silent_on_a_zero_or_unread_duration() {
    assert!(
        run(
            "47",
            "#[test]\nfn t() {\n\
             \x20   thread::sleep(Duration::from_secs(0));\n\
             \x20   thread::sleep(TIMEOUT);\n}\n",
        )
        .is_empty()
    );
}

#[test]
fn is_silent_outside_a_test() {
    assert!(
        run(
            "47",
            "pub fn wait() { thread::sleep(Duration::from_secs(2)); }\n",
        )
        .is_empty()
    );
}

#[test]
fn is_silent_where_the_sleep_is_handed_to_a_driver() {
    assert!(
        run(
            "47",
            "#[test]\nfn t() {\n\
             \x20   sim.client(\"c\", async { sleep(Duration::from_secs(1)).await; });\n\
             \x20   spawn(move || { thread::sleep(Duration::from_millis(10)); });\n}\n",
        )
        .is_empty()
    );
}

#[test]
fn is_silent_in_a_poll_loop_that_breaks() {
    assert!(
        run(
            "47",
            "#[test]\nfn t() {\n\
             \x20   for _ in 0..10 {\n\
             \x20       if done() { break; }\n\
             \x20       thread::sleep(Duration::from_millis(50));\n\
             \x20   }\n}\n",
        )
        .is_empty()
    );
}

#[test]
fn fires_in_a_loop_that_only_runs_out() {
    let found = run(
        "47",
        "#[test]\nfn t() {\n\
         \x20   for _ in 0..10 { thread::sleep(Duration::from_millis(50)); }\n}\n",
    );

    assert_eq!(found.iter().map(|f| f.site.line).collect::<Vec<_>>(), [3]);
}
