//! The oracle-revealed return types of return-unannotated internal
//! functions (#36/#40).

use crate::oracle_fixture;

use sightline_testkit::build;

/// The `ret_types` half of a run with no oracle: no candidates, no queries,
/// and the reveal path never reaches the absent checker.
#[test]
fn ret_types_answer_empty_without_an_oracle() {
    let (_dir, stack) = build(&[(
        "m.py",
        "import absent_xyz\n\ndef f(a):\n    return absent_xyz.g(a)\n",
    )]);
    let facts = stack.facts();

    assert!(stack.provers.ret_types(facts).candidates().is_empty());
    assert!(stack.provers.ret_types(facts).return_type("m.f").is_none());
    assert_eq!(
        stack.provers.ret_types(facts).dump_map(),
        serde_json::json!({})
    );
}

/// One `module_member_types` batch per file: a return-unannotated function
/// module-level code can name reveals its inferred return.
#[test]
fn a_return_unannotated_function_reveals_its_return() {
    let (dir, mut stack) = build(&[(
        "m.py",
        "def f():\n    return 1\n\nclass C:\n    def m(self):\n        return 'a'\n\n\
         def g() -> int:\n    return 2\n",
    )]);
    oracle_fixture::attach(&dir, &mut stack);
    let facts = stack.facts();
    let rets = stack.provers.ret_types(facts);

    let candidates: Vec<&str> = rets.candidates().iter().map(|q| &***q).collect();
    assert!(candidates.contains(&"m.f"), "{candidates:?}");
    assert!(candidates.contains(&"m.C.m"), "{candidates:?}");
    // a declared return is no one's question
    assert!(!candidates.contains(&"m.g"), "{candidates:?}");
    // values pinned from a probe of `ret_types` over this fixture
    assert_eq!(rets.return_type("m.f"), Some("Literal[1]"));
    assert_eq!(rets.return_type("m.C.m"), Some("Literal[\"a\"]"));
}
