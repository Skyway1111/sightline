//! `tests/provers/test_counterfactual.py`: group testing that keeps the
//! isolated verdict. Proposals sharing a caller file are judged
//! independently, so one proposal's breakage vetoes it alone and a receipt
//! is the splice's own. The groups only buy back the passes.
//!
//! Every test that needs a real answer builds an `Oracle` at the mini repo's
//! root. The nine of them run always: the whole file takes 2.1 s at one
//! thread, so none is over decision 17's one-second bar.

use std::collections::HashSet;

use camino::Utf8Path;
use indexmap::IndexMap;
use sightline_core::edits::{apply_edits, blank};
use sightline_core::findings::{Rel, SpanEdit};
use sightline_py_facts::model::source_lines;
use sightline_py_provers::counterfactual::{Outcome, Proposal, spell, verify};
use sightline_py_provers::oracle::Oracle;
use sightline_testkit::PyStack;
use sightline_testkit::build;
use tempfile::TempDir;

fn edit(line: u32, col_start: u32, col_end: u32, text: &str) -> SpanEdit {
    SpanEdit {
        line,
        col_start,
        col_end,
        text: text.to_string(),
    }
}

fn watching(files: &[&str]) -> Option<HashSet<String>> {
    Some(files.iter().map(|f| (*f).to_string()).collect())
}

fn proposal(id: &str, owner: &str, rel: &str, edits: Vec<SpanEdit>, span: (u32, u32)) -> Proposal {
    Proposal {
        id: id.to_string(),
        owner: owner.to_string(),
        rel: Rel::from(rel),
        edits,
        span,
        watched: Some(HashSet::new()),
        imports: Vec::new(),
        param: String::new(),
    }
}

/// The mini repo, its facts and an oracle at the same root.
fn with_oracle(files: &[(&str, &str)]) -> (TempDir, PyStack, Oracle) {
    let (dir, stack) = build(files);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let oracle = Oracle::new(root, &[], &[], None).expect("an oracle at the mini repo");
    (dir, stack, oracle)
}

/// Every pass the run cost: the worlds of each non-empty `verify_worlds`
/// batch (an empty batch answers without a pass), as the Python `worlds`
/// fixture records them.
fn passes(oracle: &Oracle) -> Vec<Vec<String>> {
    oracle
        .world_calls()
        .iter()
        .filter(|call| !call.worlds.is_empty())
        .map(|call| call.worlds.iter().map(|(id, _)| id.clone()).collect())
        .collect()
}

#[test]
fn a_breaking_proposal_does_not_veto_its_neighbour() {
    let (_dir, stack, oracle) = with_oracle(&[
        ("m.py", "def a_fn(x):\n    return str(x)\n"),
        ("n.py", "def b_fn(y):\n    return y\n"),
        (
            "caller.py",
            "import m\nimport n\ndef call() -> None:\n    m.a_fn('s')\n    n.b_fn('s')\n",
        ),
    ]);
    let shared = watching(&["caller.py"]);
    let bad = Proposal {
        watched: shared.clone(),
        ..proposal(
            "m.a_fn:x",
            "m.a_fn",
            "m.py",
            vec![edit(1, 10, 10, ": int")],
            (1, 2),
        )
    };
    let good = Proposal {
        watched: shared,
        ..proposal(
            "n.b_fn:y",
            "n.b_fn",
            "n.py",
            vec![edit(1, 10, 10, ": str")],
            (1, 2),
        )
    };

    let out = verify(stack.facts(), &[bad, good], &oracle);

    assert_eq!(out["m.a_fn:x"], Outcome::Veto);
    // same watched caller file, but the neighbour's world never saw the
    // breaking splice: verified clean, not vetoed
    assert_eq!(out["n.b_fn:y"], Outcome::Clean);
}

#[test]
fn a_possibly_unbound_read_a_splice_reveals_never_vetoes() {
    // a deletion splice (module-owned, so every file is watched) whose only
    // new diagnostic is the dropped store leaving the read bound on some
    // paths only. The shim reports that at warning severity for exactly this
    // reason: as an error it would veto every patch that reveals one.
    let source = concat!(
        "def f(flag: bool) -> int:\n",
        "    seen = 0\n",
        "    if flag:\n",
        "        seen = 1\n",
        "    return seen\n",
    );
    let (_dir, stack, oracle) = with_oracle(&[("m.py", source)]);
    let edits = blank(&source_lines(source), 2, 2);
    let mut spliced: Vec<String> = source_lines(source).iter().map(|l| l.to_string()).collect();
    apply_edits(&mut spliced, &edits);
    let world = IndexMap::from([("m.py".to_string(), spliced.join("\n") + "\n")]);

    let added = oracle.verify_worlds(&[("probe".to_string(), world)], None);
    let seen: Vec<(&str, &str, u32)> = added["probe"]
        .iter()
        .map(|d| (d.rule.as_str(), d.severity.as_str(), d.line))
        .collect();
    assert_eq!(seen, [("reportPossiblyUnbound", "warning", 5)]);

    let drop = Proposal {
        watched: None,
        ..proposal("m:seen", "m", "m.py", edits, (1, 5))
    };
    assert_eq!(
        verify(stack.facts(), &[drop], &oracle)["m:seen"],
        Outcome::Clean
    );
}

#[test]
fn a_receipt_is_the_splices_own() {
    // `a is None` is the body's only guard: the merged world's receipt
    // belongs to `f:a`; `f:b` alone earns none and stays a clean lift
    let (_dir, stack, oracle) = with_oracle(&[(
        "two.py",
        concat!(
            "def f(a, b):\n    if a is None:\n        return 0\n    return b\n",
            "def main():\n    f(1, 2)\n    f(3, 4)\n",
        ),
    )]);
    let lift = |param: &str, col: u32| Proposal {
        param: param.to_string(),
        watched: watching(&["two.py"]),
        ..proposal(
            &format!("two.f:{param}"),
            "two.f",
            "two.py",
            vec![edit(1, col, col, ": int")],
            (1, 4),
        )
    };

    let out = verify(stack.facts(), &[lift("a", 7), lift("b", 10)], &oracle);

    assert!(matches!(out["two.f:a"], Outcome::Receipt(_)));
    assert_eq!(out["two.f:b"], Outcome::Clean);
}

#[test]
fn two_vetoes_in_one_half_are_both_found() {
    // every splice watches the one caller file, so the merged world
    // implicates all four; halving puts both breakers in the first half and
    // both clean neighbours in the second, cleared by that half's one world
    let modules: Vec<(String, String)> = (1..5)
        .map(|i| {
            (
                format!("m{i}.py"),
                format!("def f{i}(x):\n    return str(x)\n"),
            )
        })
        .collect();
    let caller = (1..5).fold(String::new(), |mut text, i| {
        text.push_str(&format!("import m{i}\n"));
        text
    }) + "def call() -> None:\n"
        + &(1..5).fold(String::new(), |mut text, i| {
            text.push_str(&format!("    m{i}.f{i}('s')\n"));
            text
        });
    let mut files: Vec<(&str, &str)> = modules
        .iter()
        .map(|(rel, src)| (rel.as_str(), src.as_str()))
        .collect();
    files.push(("caller.py", caller.as_str()));
    let (_dir, stack, oracle) = with_oracle(&files);

    let lift = |i: u32, spelling: &str| Proposal {
        watched: watching(&["caller.py"]),
        ..proposal(
            &format!("m{i}.f{i}:x"),
            &format!("m{i}.f{i}"),
            &format!("m{i}.py"),
            vec![edit(1, 8, 8, &format!(": {spelling}"))],
            (1, 2),
        )
    };
    let proposals: Vec<Proposal> = [(1, "int"), (2, "int"), (3, "str"), (4, "str")]
        .iter()
        .map(|(i, spelling)| lift(*i, spelling))
        .collect();

    let out = verify(stack.facts(), &proposals, &oracle);

    assert_eq!(out["m1.f1:x"], Outcome::Veto);
    assert_eq!(out["m2.f2:x"], Outcome::Veto);
    assert_eq!(out["m3.f3:x"], Outcome::Clean);
    assert_eq!(out["m4.f4:x"], Outcome::Clean);
    // merged, then the two half-groups, then the breaking half's two
    // singletons: the clean pair never bought a world of its own
    assert_eq!(
        passes(&oracle).iter().map(Vec::len).collect::<Vec<_>>(),
        [1, 2, 2]
    );
}

#[test]
fn a_body_local_breakage_clears_its_file_mates_in_one_world() {
    // every splice in the file watches the file, so one breaker makes all
    // four suspects; the error sits in its own body, and splitting there
    // names it and clears the other three together
    let source = (1..5).fold(String::new(), |mut text, i| {
        text.push_str(&format!("def f{i}(x):\n    return x.upper()\n"));
        text
    }) + "def call() -> None:\n"
        + &(1..5).fold(String::new(), |mut text, i| {
            text.push_str(&format!("    f{i}('s')\n"));
            text
        });
    let (_dir, stack, oracle) = with_oracle(&[("body.py", source.as_str())]);

    let lift = |i: u32, spelling: &str| Proposal {
        watched: watching(&["body.py"]),
        ..proposal(
            &format!("body.f{i}:x"),
            &format!("body.f{i}"),
            "body.py",
            vec![edit(2 * i - 1, 8, 8, &format!(": {spelling}"))],
            (2 * i - 1, 2 * i),
        )
    };
    let proposals: Vec<Proposal> = [(1, "int"), (2, "str"), (3, "str"), (4, "str")]
        .iter()
        .map(|(i, spelling)| lift(*i, spelling))
        .collect();

    let out = verify(stack.facts(), &proposals, &oracle);

    assert_eq!(out["body.f1:x"], Outcome::Veto);
    for i in 2..5 {
        assert_eq!(out[&format!("body.f{i}:x")], Outcome::Clean);
    }
    // merged, then the hosting body alone against the three it implicated
    assert_eq!(
        passes(&oracle).iter().map(Vec::len).collect::<Vec<_>>(),
        [1, 2]
    );
}

#[test]
fn two_receipts_in_one_body_are_told_apart_by_operand() {
    // both lifts land in one body and each makes its own guard redundant:
    // the operand the diagnostic points at names the parameter that owns it,
    // so neither splice needs a world of its own
    let (_dir, stack, oracle) = with_oracle(&[(
        "two.py",
        concat!(
            "def f(a, b):\n",
            "    if a is None:\n        return 0\n",
            "    if b is None:\n        return 1\n",
            "    return 2\n",
            "def main():\n    f(1, 'x')\n",
        ),
    )]);
    let lift = |param: &str, col: u32, spelling: &str| Proposal {
        param: param.to_string(),
        watched: watching(&["two.py"]),
        ..proposal(
            &format!("two.f:{param}"),
            "two.f",
            "two.py",
            vec![edit(1, col, col, &format!(": {spelling}"))],
            (1, 6),
        )
    };

    let out = verify(
        stack.facts(),
        &[lift("a", 7, "int"), lift("b", 10, "str")],
        &oracle,
    );

    let receipt = |id: &str| match &out[id] {
        Outcome::Receipt(diag) => diag.clone(),
        other => panic!("{id} earned {other:?}, not a receipt"),
    };
    assert!(receipt("two.f:a").contains("\"int\""));
    assert!(receipt("two.f:b").contains("\"str\""));
    assert_eq!(passes(&oracle).len(), 1); // the merged world settled both
}

#[test]
fn a_receipt_naming_two_spliced_params_falls_back_to_isolation() {
    // the near-miss twin: the guard's line holds both parameters, so no
    // operand resolves it and each splice earns its own world. `a`'s guard
    // is redundant under `a: int` alone, `b`'s lift earns nothing
    let (_dir, stack, oracle) = with_oracle(&[(
        "amb.py",
        concat!(
            "def f(a, b):\n    if a is None: return b\n    return 1\n",
            "def main():\n    f(1, 'x')\n",
        ),
    )]);
    let lift = |param: &str, col: u32, spelling: &str| Proposal {
        param: param.to_string(),
        watched: watching(&["amb.py"]),
        ..proposal(
            &format!("amb.f:{param}"),
            "amb.f",
            "amb.py",
            vec![edit(1, col, col, &format!(": {spelling}"))],
            (1, 3),
        )
    };

    let out = verify(
        stack.facts(),
        &[lift("a", 7, "int"), lift("b", 10, "str")],
        &oracle,
    );

    assert!(matches!(out["amb.f:a"], Outcome::Receipt(_)));
    assert_eq!(out["amb.f:b"], Outcome::Clean);
    let passes = passes(&oracle);
    assert_eq!(passes.len(), 2);
    assert_eq!(
        passes[1].iter().cloned().collect::<HashSet<String>>(),
        HashSet::from(["amb.f:a".to_string(), "amb.f:b".to_string()])
    );
}

#[test]
fn a_splice_lands_on_the_ast_line_past_a_form_feed() {
    // a form-feed line is one line to the AST and two to `str.splitlines`:
    // the lift must land on `f`'s def line, not the blank one before it
    let (_dir, stack, oracle) = with_oracle(&[(
        "ff.py",
        concat!(
            "def a():\n    return 1\n\x0c\n",
            "def f(a, b):\n    if a is None:\n        return 0\n    return b\n",
            "def main():\n    f(1, 2)\n",
        ),
    )]);
    let lift = Proposal {
        watched: watching(&["ff.py"]),
        ..proposal(
            "ff.f:a",
            "ff.f",
            "ff.py",
            vec![edit(4, 7, 7, ": int")],
            (4, 7),
        )
    };

    let out = verify(stack.facts(), &[lift], &oracle);

    assert!(matches!(out["ff.f:a"], Outcome::Receipt(_)));
}

#[test]
fn a_type_checking_only_import_is_not_a_binding() {
    // the guarded import never enters `module.bindings`, so the splice
    // brings a real import: the runtime NameError never held on this tree
    let (_dir, stack) = build(&[(
        "seq.py",
        concat!(
            "from typing import TYPE_CHECKING\n",
            "if TYPE_CHECKING:\n    from collections.abc import Iterable\n",
        ),
    )]);
    let facts = stack.facts();
    let module = &facts.modules["seq"];
    assert!(!module.bindings.contains_key("Iterable"));
    assert_eq!(
        spell("Iterable[int]", module, facts, None),
        Some((
            IndexMap::new(),
            vec!["from collections.abc import Iterable".to_string()]
        ))
    );
}

#[test]
fn a_class_the_module_binds_rides_without_an_import() {
    // a repo or stdlib class the file already binds at module scope needs no
    // import line (no cycle to judge); a name bound to something else, an
    // unbound one, and the oracle's `Unknown` stay splice-unsafe
    let (_dir, stack) = build(&[
        ("shapes.py", "class Box:\n    pass\n"),
        (
            "m.py",
            concat!(
                "from pathlib import Path\n",
                "from os import path as Dir\n",
                "from shapes import Box\n",
                "Node = 3\n",
                "class Own:\n    pass\n",
            ),
        ),
    ]);
    let facts = stack.facts();
    let module = &facts.modules["m"];
    let bare = Some((IndexMap::new(), Vec::new()));
    assert_eq!(spell("Box | None", module, facts, None), bare);
    assert_eq!(spell("dict[str, Path]", module, facts, None), bare);
    assert_eq!(spell("Own", module, facts, None), bare);
    assert_eq!(
        spell("list[Box] | Sequence[Own]", module, facts, None),
        Some((
            IndexMap::new(),
            vec!["from collections.abc import Sequence".to_string()]
        ))
    );
    for unsafe_name in ["Dir", "Node", "Missing", "Unknown", "Box | Unknown"] {
        assert_eq!(
            spell(unsafe_name, module, facts, None),
            None,
            "{unsafe_name}"
        );
    }
}

#[test]
fn a_bare_display_is_respelled_through_the_one_stdlib_module_binding_it() {
    // the oracle displays `AST` bare; a file that only imports `ast` spells
    // it `ast.AST` (a wrong guess is the world's veto); a name two bound
    // stdlib modules define (`re.Match`, `ast.Match`), a non-class attribute
    // and a third-party module's class stay unspellable
    let (_dir, stack, oracle) = with_oracle(&[(
        "m.py",
        "import ast\nimport re\nimport numpy as np\nfrom os import path\n",
    )]);
    let facts = stack.facts();
    let module = &facts.modules["m"];
    let ast_home = IndexMap::from([("AST".to_string(), "ast.AST".to_string())]);
    assert_eq!(
        spell("AST | None", module, facts, Some(&oracle)),
        Some((ast_home.clone(), Vec::new()))
    );
    assert_eq!(
        spell("Sequence[AST]", module, facts, Some(&oracle)),
        Some((
            ast_home,
            vec!["from collections.abc import Sequence".to_string()]
        ))
    );
    for unsafe_name in ["Match", "walk", "ndarray", "sep"] {
        assert_eq!(
            spell(unsafe_name, module, facts, Some(&oracle)),
            None,
            "{unsafe_name}"
        );
    }
}
