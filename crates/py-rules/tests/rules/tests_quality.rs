//! Family T (`tests/rules/test_tests_quality.py`): #42, #44, #47, each a
//! pos/neg pair.
//!
//! file-length-ok: one file per rule family, mirroring `src/tests_quality.rs`.

use std::collections::BTreeSet;

use sightline_core::findings::{Finding, Tier};
use sightline_testkit::run_rule;

const CALC: &str = concat!(
    "def add(a, b):\n",
    "    return a + b\n",
    "def log(x):\n",
    "    print(x)\n",
    "def ident(v):\n",
    "    return v\n",
    "def delegate(v):\n",
    "    return add(v, 1)\n",
    "def fetch():\n",
    "    return 'net'\n",
    "def load():\n",
    "    return fetch().upper()\n",
);

fn symbols(found: &[Finding]) -> Vec<&str> {
    let mut names: Vec<&str> = found.iter().map(|f| &*f.site.symbol).collect();
    names.sort();
    names
}

fn sites(found: &[Finding]) -> Vec<(&str, u32)> {
    found
        .iter()
        .map(|f| (&*f.site.symbol, f.site.line))
        .collect()
}

// --- #42 assertion-free test -------------------------------------------------

#[test]
fn a_test_with_no_verdict_fires() {
    let found = run_rule(
        "42",
        &[
            ("calc.py", CALC),
            ("tests/__init__.py", ""),
            (
                "tests/test_a.py",
                concat!(
                    "import pytest\n",
                    "import unittest\n",
                    "from calc import add, ident\n",
                    "def check_sum(v):\n",
                    "    assert v == 3\n",
                    // `add` is the callee test_raises pins, whose accepting half
                    // is an oracle of its own: the bare test calls another
                    "def test_bare():\n",
                    "    ident(1)\n",
                    "def test_empty():\n",
                    "    pass\n",
                    "def test_plain():\n",
                    "    assert add(1, 2) == 3\n",
                    "def test_raises():\n",
                    "    with pytest.raises(TypeError):\n",
                    "        add('a', None)\n",
                    "def test_helper():\n",
                    "    check_sum(add(1, 2))\n",
                    "def test_mock(m):\n",
                    "    add(1, 2)\n",
                    "    m.assert_called_once()\n",
                    "def test_fail_path():\n",
                    "    if add(1, 2) != 3:\n",
                    "        pytest.fail('bad')\n",
                    "class TestAdd(unittest.TestCase):\n",
                    "    def test_unittest(self):\n",
                    "        self.assertEqual(add(1, 2), 3)\n",
                ),
            ),
        ],
    );
    assert_eq!(
        symbols(&found),
        ["tests.test_a.test_bare", "tests.test_a.test_empty"]
    );
    assert!(found.iter().all(|f| f.tier() == Tier::Indexed));
}

/// The override candidates are two, so no one body answers "does it verdict":
/// silence, not a guess. Its twin resolves and does not.
#[test]
fn a_body_the_graph_cannot_read_is_not_a_missing_verdict() {
    let ambiguous = concat!(
        "class TestBase:\n",
        "    def helper(self):\n",
        "        assert True\n",
        "    def test_uses_the_helper(self):\n",
        "        self.helper()\n",
        "class TestChild(TestBase):\n",
        "    def helper(self):\n",
        "        pass\n",
    );
    assert!(run_rule("42", &[("tests/test_c.py", ambiguous)]).is_empty());
    let both_silent = ambiguous.replace("        assert True", "        pass");
    assert!(run_rule("42", &[("tests/test_c.py", &both_silent)]).is_empty());
    let one_candidate = concat!(
        "class TestBase:\n",
        "    def helper(self):\n",
        "        pass\n",
        "    def test_uses_the_helper(self):\n",
        "        self.helper()\n",
    );
    let found = run_rule("42", &[("tests/test_c.py", one_candidate)]);
    assert_eq!(symbols(&found), ["test_c.TestBase.test_uses_the_helper"]);
}

/// A placeholder that skips, xfails or raises at once decides nothing; the
/// same calls under a condition are the test's verdict.
#[test]
fn skip_xfail_and_an_unconditional_raise_are_not_verdicts() {
    let found = run_rule(
        "42",
        &[
            ("calc.py", CALC),
            (
                "tests/test_a.py",
                concat!(
                    "import pytest\n",
                    "from calc import add\n",
                    "def test_skip_placeholder():\n",
                    "    pytest.skip('todo')\n",
                    "def test_xfail_placeholder():\n",
                    "    pytest.xfail('todo')\n",
                    "def test_raise_placeholder():\n",
                    "    raise NotImplementedError\n",
                    "def test_skip_guard_then_nothing():\n",
                    "    if add(1, 2) != 3:\n",
                    "        pytest.skip('env')\n",
                    "    add(1, 2)\n",
                    "def test_fail_guard():\n",
                    "    if add(1, 2) != 3:\n",
                    "        pytest.fail('bad')\n",
                    "def test_raise_guard():\n",
                    "    if add(1, 2) != 3:\n",
                    "        raise AssertionError\n",
                    "def test_raise_in_handler():\n",
                    "    try:\n",
                    "        add(1, 2)\n",
                    "    except TypeError:\n",
                    "        raise AssertionError('typed')\n",
                    "def _require(v):\n",
                    "    if v == 3:\n",
                    "        return\n",
                    "    raise ValueError(v)\n",
                    "def test_guard_clause_helper():\n",
                    "    _require(add(1, 2))\n",
                ),
            ),
        ],
    );
    assert_eq!(
        symbols(&found),
        [
            "test_a.test_raise_placeholder",
            "test_a.test_skip_guard_then_nothing",
            "test_a.test_skip_placeholder",
            "test_a.test_xfail_placeholder",
        ]
    );
}

/// Three carriers outside the test's own statements: the module pins the
/// rejecting cases of the same callee under `pytest.raises`, a checked child
/// process raises on the asserts its body holds, the test states the no-raise
/// oracle in words. A `deprecated_call` pin is a warning, not an exception.
#[test]
fn an_oracle_the_test_does_not_spell_itself() {
    let found = run_rule(
        "42",
        &[
            ("calc.py", CALC),
            (
                "tests/test_a.py",
                concat!(
                    "import pytest\n",
                    "import subprocess\n",
                    "from calc import add, ident, log\n",
                    "def _validate(v):\n",
                    "    add(v, 1)\n",
                    "def test_rejects():\n",
                    "    with pytest.raises(ValueError):\n",
                    "        _validate(-1)\n",
                    "def test_accepts():\n",
                    "    _validate(1)\n",
                    "def test_child_process():\n",
                    "    subprocess.run(['py', '-c', 'assert 1'], check=True)\n",
                    "def test_unchecked_child():\n",
                    "    subprocess.run(['py', '-c', 'assert 1'])\n",
                    "def test_states_its_oracle():\n",
                    "    log(1)  # must not raise\n",
                    "def test_warning_pin():\n",
                    "    with pytest.deprecated_call():\n",
                    "        ident(1)\n",
                    "def test_the_warned_call_alone():\n",
                    "    ident(1)\n",
                ),
            ),
        ],
    );
    assert_eq!(
        symbols(&found),
        [
            "test_a.test_the_warned_call_alone",
            "test_a.test_unchecked_child",
        ]
    );
}

/// pytest injects the callable: no graph edge reads its body, so the verdict
/// may be there. A module helper still resolves and verdicts.
#[test]
fn a_call_on_the_tests_own_parameter_is_unknown() {
    let found = run_rule(
        "42",
        &[
            ("calc.py", CALC),
            (
                "tests/conftest.py",
                concat!(
                    "import pytest\n",
                    "@pytest.fixture\n",
                    "def check_conftest():\n",
                    "    def _c(r):\n",
                    "        assert r == 3\n",
                    "    return _c\n",
                ),
            ),
            (
                "tests/test_a.py",
                concat!(
                    "import pytest\n",
                    "from calc import add\n",
                    "@pytest.fixture\n",
                    "def check():\n",
                    "    def _check(r):\n",
                    "        assert r == 3\n",
                    "    return _check\n",
                    "def test_fixture_check(check):\n",
                    "    check(add(1, 2))\n",
                    "def test_fixture_conftest(check_conftest):\n",
                    "    check_conftest(add(1, 2))\n",
                    "def _verify(r):\n",
                    "    assert r == 3\n",
                    "def test_module_helper():\n",
                    "    _verify(add(1, 2))\n",
                    "def test_bare():\n",
                    "    add(1, 2)\n",
                ),
            ),
        ],
    );
    assert_eq!(symbols(&found), ["test_a.test_bare"]);
}

/// `checker.run(add(1, 2))`: the fixture-injected object's method is an oracle
/// carrier no graph edge reads, at any chain depth, only when given a resolved
/// repo call's result. `monkeypatch.setattr(calc, 'fetch', stub)` and a
/// non-repo result carry nothing.
#[test]
fn a_call_on_the_tests_own_parameter_may_verdict_when_handed_a_repo_result() {
    let found = run_rule(
        "42",
        &[
            ("calc.py", CALC),
            (
                "tests/test_a.py",
                concat!(
                    "import json\n",
                    "import calc\n",
                    "from calc import add\n",
                    "def test_injected(checker):\n",
                    "    checker.run(add(1, 2))\n",
                    "def test_injected_deep(checker):\n",
                    "    checker.results[0].check(add(1, 2))\n",
                    "def test_injected_bound(checker):\n",
                    "    r = add(1, 2)\n",
                    "    checker.run(r)\n",
                    "def test_injected_keyword(checker):\n",
                    "    checker.run(value=add(1, 2))\n",
                    "def test_patch_only(monkeypatch):\n",
                    "    monkeypatch.setattr(calc, 'fetch', lambda: 1)\n",
                    "    add(1, 2)\n",
                    "def test_handed_a_library_result(checker):\n",
                    "    checker.run(json.loads('1'))\n",
                    "def test_bare():\n",
                    "    add(1, 2)\n",
                ),
            ),
        ],
    );
    assert_eq!(
        symbols(&found),
        [
            "test_a.test_bare",
            "test_a.test_handed_a_library_result",
            "test_a.test_patch_only",
        ]
    );
}

/// The skippable-verdict arms are cut at their bar (`docs/todo.md`):
/// loop-verdict judged four times, conditional-verdict twice. The Python file
/// keeps both fixtures under a strict xfail; here the reading is that #42
/// yields its one cause prefix and never those arms'.
#[test]
fn the_cut_skippable_verdict_arms_never_fire() {
    let loops = concat!(
        "import pytest\n",
        "import calc\n",
        "from calc import add, load\n",
        "def _scan():\n",
        "    return load()\n",
        "def _check(v):\n",
        "    assert v == 3\n",
        "def test_every_row():\n",
        "    for r in _scan():\n",
        "        assert add(r, 1) == 3\n",
        "def test_each_helper():\n",
        "    for r in _scan():\n",
        "        _check(add(r, 1))\n",
        "def test_each_fail():\n",
        "    for r in _scan():\n",
        "        if add(r, 1) != 3:\n",
        "            pytest.fail('bad')\n",
    );
    let conditionals = concat!(
        "import pytest\n",
        "from calc import add, load\n",
        "def _check(v):\n",
        "    assert v == 3\n",
        "def test_one_sided(r):\n",
        "    if isinstance(r, int):\n",
        "        assert add(r, 1) == 3\n",
        "def test_one_sided_helper(r):\n",
        "    if isinstance(r, int):\n",
        "        _check(add(r, 1))\n",
        "def test_both_paths(r):\n",
        "    if isinstance(r, int):\n",
        "        assert add(r, 1) == 3\n",
        "    else:\n",
        "        assert add(r, 1) is None\n",
    );
    for src in [loops, conditionals] {
        let found = run_rule("42", &[("calc.py", CALC), ("tests/test_a.py", src)]);
        assert!(
            found.iter().all(|f| f.cause.starts_with("assertion-free:")),
            "{:?}",
            found.iter().map(|f| f.cause.clone()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn prod_code_and_helpers_are_not_tests() {
    let prod = format!("{CALC}def test_like():\n    add(1, 2)\n");
    let found = run_rule(
        "42",
        &[
            ("calc.py", &prod),
            ("tests/helpers.py", "def build():\n    return 1\n"),
            (
                "tests/support.py",
                "def test_ref_verdict(ref):\n    return ref\n",
            ),
            (
                "tests/test_b.py",
                "from calc import add\ndef helper():\n    add(1, 2)\n",
            ),
        ],
    );
    assert!(found.is_empty(), "{:?}", symbols(&found));
}

/// A def nested in a test is a callback (a dialog's on_click), and a `test`
/// method of a double is that object's protocol: no runner sees either. The
/// collected twins still fire.
#[test]
fn only_what_a_runner_collects_is_a_test() {
    let found = run_rule(
        "42",
        &[
            ("calc.py", CALC),
            ("tests/__init__.py", ""),
            (
                "tests/test_a.py",
                concat!(
                    "import unittest\n",
                    "from calc import add\n",
                    "class FakeClient:\n",
                    "    def test(self):\n",
                    "        add(1, 2)\n",
                    "class AddTests(unittest.TestCase):\n",
                    "    def test_callback_inside(self):\n",
                    "        def test_on_click(button):\n",
                    "            add(button, 1)\n",
                    "        self.assertEqual(add(1, 2), 3)\n",
                    "    def test_no_verdict(self):\n",
                    "        add(1, 2)\n",
                    "def test_module_level():\n",
                    "    add(1, 2)\n",
                ),
            ),
        ],
    );
    assert_eq!(
        symbols(&found),
        [
            "tests.test_a.AddTests.test_no_verdict",
            "tests.test_a.test_module_level",
        ]
    );
}

// --- #44 tautological assertion ----------------------------------------------

#[test]
fn identical_operands_and_mirrors_fire() {
    let found = run_rule(
        "44",
        &[
            ("calc.py", CALC),
            (
                "tests/test_a.py",
                concat!(
                    "import unittest\n",
                    "from calc import add\n",
                    "def test_self():\n",
                    "    r = add(1, 2)\n",
                    "    assert r == r\n",
                    "def test_mirror():\n",
                    "    expected = add(1, 2)\n",
                    "    assert add(1, 2) == expected\n",
                    "def test_const():\n",
                    "    assert True\n",
                    "def test_real():\n",
                    "    assert add(1, 2) == 3\n",
                    "def test_equivalence():\n",
                    "    assert add(1, 2) == add(2, 1)\n",
                    "def test_roundtrip():\n",
                    "    x = 3\n",
                    "    assert add(add(x, 1), -1) == x\n",
                    "class TestAdd(unittest.TestCase):\n",
                    "    def test_same(self):\n",
                    "        v = add(1, 2)\n",
                    "        self.assertEqual(v, v)\n",
                    "    def test_true(self):\n",
                    "        self.assertTrue(True)\n",
                    "    def test_ok(self):\n",
                    "        self.assertEqual(add(1, 2), 3)\n",
                    "def test_cached():\n",
                    "    assert add.__doc__ is add.__doc__\n",
                    "def test_deterministic():\n",
                    "    assert add(1, 2) == add(1, 2)\n",
                    "def test_mock_verification(m):\n",
                    "    m.assert_called_once_with(None)\n",
                ),
            ),
        ],
    );
    let mut seen = sites(&found);
    seen.sort();
    assert_eq!(
        seen,
        [
            ("test_a.TestAdd.test_same", 21),
            ("test_a.TestAdd.test_true", 23),
            ("test_a.test_const", 10),
            ("test_a.test_self", 5),
        ]
    );
    assert!(found.iter().all(|f| f.tier() == Tier::Heuristic));
}

/// `x == x` on an instance of a repo class that writes `__eq__`/`__ge__` runs
/// that dunder: the reflexive and equal-case boundaries of a hand-written
/// comparison, which is the code under test.
#[test]
fn a_repo_classs_comparison_dunder_is_not_call_free() {
    let found = run_rule(
        "44",
        &[
            (
                "ts.py",
                concat!(
                    "class Ts:\n",
                    "    def __init__(self, raw):\n",
                    "        self.raw = raw\n",
                    "    def __eq__(self, other):\n",
                    "        return self.raw == str(other)\n",
                    "    def __ge__(self, other):\n",
                    "        return self.raw >= str(other)\n",
                ),
            ),
            (
                "plain.py",
                "class Plain:\n    def __init__(self, v):\n        self.v = v\n",
            ),
            (
                "tests/test_ts.py",
                concat!(
                    "from ts import Ts\n",
                    "from plain import Plain\n",
                    "base = Ts('1.5')\n",
                    "plain = Plain(1)\n",
                    "def test_eq():\n",
                    "    assert base == base\n",
                    "def test_ge():\n",
                    "    assert base >= base\n",
                    "def test_plain():\n",
                    "    assert plain == plain\n",
                    "def test_row():\n",
                    "    row = {'hash': 1}\n",
                    "    assert row['hash'] == row['hash']\n",
                ),
            ),
        ],
    );
    assert_eq!(symbols(&found), ["test_ts.test_plain", "test_ts.test_row"]);
}

/// `assert False` and its falsy kin always fail, so they mark an arm the test
/// must not reach - the should-have-raised `else`, the must-not-raise except,
/// the exhaustiveness arm, a body a skip decorator never runs. Only a constant
/// that cannot be false is an assertion that cannot fail.
#[test]
fn a_falsy_constant_marks_an_arm_and_is_not_a_tautology() {
    let found = run_rule(
        "44",
        &[
            ("calc.py", CALC),
            (
                "tests/test_a.py",
                concat!(
                    "import unittest\n",
                    "from calc import add\n",
                    "def test_should_have_raised():\n",
                    "    try:\n",
                    "        add(1, 2)\n",
                    "    except ValueError:\n",
                    "        pass\n",
                    "    else:\n",
                    "        assert False, 'did not raise'\n",
                    "def test_must_not_raise():\n",
                    "    try:\n",
                    "        add(1, 2)\n",
                    "    except ValueError:\n",
                    "        assert False\n",
                    "def test_exhaustive(mode):\n",
                    "    if mode == 'a':\n",
                    "        expected = 2\n",
                    "    elif mode == 'b':\n",
                    "        expected = 3\n",
                    "    else:\n",
                    "        assert 0\n",
                    "    assert add(1, 1) == expected\n",
                    "def test_empty_string():\n",
                    "    assert ''\n",
                    "def test_none():\n",
                    "    assert None\n",
                    "def test_true():\n",
                    "    assert True\n",
                    "def test_one():\n",
                    "    assert 1\n",
                    "def test_text():\n",
                    "    assert 'x'\n",
                    "@unittest.skip('fixture data')\n",
                    "class Test(unittest.TestCase):\n",
                    "    def test_never_runs(self):\n",
                    "        assert 0\n",
                ),
            ),
        ],
    );
    assert_eq!(
        symbols(&found),
        ["test_a.test_one", "test_a.test_text", "test_a.test_true"]
    );
}

/// Only `assertTrue` and `assertFalse` put the truth value under test in their
/// one argument. Every other one-argument `assert*` takes a spec - a logger
/// name, a query count, a template, an exception class - and a constant there
/// is no assertion of a constant.
#[test]
fn only_the_truth_asserts_read_their_one_argument_as_a_constant() {
    let found = run_rule(
        "44",
        &[
            ("calc.py", CALC),
            (
                "tests/test_a.py",
                concat!(
                    "import unittest\n",
                    "from calc import add, log\n",
                    "class TestSpecs(unittest.TestCase):\n",
                    "    def test_logs(self):\n",
                    "        with self.assertLogs('calc', level='ERROR'):\n",
                    "            log(1)\n",
                    "    def test_no_logs(self):\n",
                    "        with self.assertNoLogs('calc'):\n",
                    "            log(1)\n",
                    "    def test_queries(self):\n",
                    "        with self.assertNumQueries(1):\n",
                    "            add(1, 2)\n",
                    "    def test_template(self):\n",
                    "        self.assertTemplateUsed('admin/index.html')\n",
                    "    def test_raises(self):\n",
                    "        with self.assertRaises(TypeError):\n",
                    "            add('a', None)\n",
                    "    def test_true(self):\n",
                    "        self.assertTrue(True)\n",
                    "    def test_false(self):\n",
                    "        self.assertFalse(0)\n",
                    "    def test_marker(self):\n",
                    "        self.assertTrue(False)\n",
                    "    def test_same(self):\n",
                    "        v = add(1, 2)\n",
                    "        self.assertEqual(v, v)\n",
                ),
            ),
        ],
    );
    assert_eq!(
        sites(&found),
        [
            ("test_a.TestSpecs.test_true", 19),
            ("test_a.TestSpecs.test_false", 21),
            ("test_a.TestSpecs.test_same", 26),
        ]
    );
}

/// An `assert_*` resolved through the module's bindings to a library home
/// compares operands like assertEqual, however it is spelled; a repo-defined
/// `assert_*` and mock's verifications on a local are not.
#[test]
fn library_assert_functions_are_operand_assertions() {
    let found = run_rule(
        "44",
        &[
            (
                "slots.py",
                "def assert_promotion_evidence(a, b):\n    return a\n",
            ),
            (
                "tests/test_a.py",
                concat!(
                    "import numpy as np\n",
                    "from numpy import testing\n",
                    "from numpy.testing import assert_allclose\n",
                    "from slots import assert_promotion_evidence\n",
                    "def assert_shape(a, b):\n",
                    "    assert a == b\n",
                    "def test_allclose_self(y):\n",
                    "    assert_allclose(y, y)\n",
                    "def test_allclose_real(y, expected):\n",
                    "    assert_allclose(y, expected)\n",
                    "def test_attr_spelling_self(y):\n",
                    "    np.testing.assert_array_equal(y, y)\n",
                    "    testing.assert_equal(y, y)\n",
                    "def test_repo_validator(y):\n",
                    "    assert_promotion_evidence(y, y)\n",
                    "    assert_shape(y, y)\n",
                    "def test_mock_verification(m):\n",
                    "    m.assert_called_with(m)\n",
                    "async def test_await_verification(m):\n",
                    "    m.assert_awaited_once_with(m)\n",
                ),
            ),
        ],
    );
    assert_eq!(
        sites(&found),
        [
            ("test_a.test_allclose_self", 8),
            ("test_a.test_attr_spelling_self", 12),
            ("test_a.test_attr_spelling_self", 13),
        ]
    );
}

// --- #47 sleepy test ----------------------------------------------------------

#[test]
fn positive_constant_sleeps_fire() {
    let calc = format!("{CALC}import time\ndef slow():\n    time.sleep(1)\n");
    let found = run_rule(
        "47",
        &[
            ("calc.py", &calc),
            (
                "tests/test_a.py",
                concat!(
                    "import asyncio\n",
                    "import time\n",
                    "from time import sleep\n",
                    "from calc import add\n",
                    "def test_module_sleep():\n",
                    "    time.sleep(0.5)\n",
                    "    assert add(1, 2) == 3\n",
                    "def test_bare_sleep():\n",
                    "    sleep(1)\n",
                    "    assert add(1, 2) == 3\n",
                    "async def test_async_sleep():\n",
                    "    await asyncio.sleep(2)\n",
                    "    assert add(1, 2) == 3\n",
                    "def test_yield():\n",
                    "    time.sleep(0)\n",
                    "    assert add(1, 2) == 3\n",
                    "def test_variable(delay):\n",
                    "    time.sleep(delay)\n",
                    "    assert add(1, 2) == 3\n",
                    "def test_slow_stub(monkeypatch):\n",
                    "    def slow():\n",
                    "        time.sleep(0.5)\n",
                    "        return 'x'\n",
                    "    monkeypatch.setattr('calc.fetch', slow)\n",
                    "    assert add(1, 2) == 3\n",
                ),
            ),
        ],
    );
    let mut seen = sites(&found);
    seen.sort();
    assert_eq!(
        seen,
        [
            ("test_a.test_async_sleep", 12),
            ("test_a.test_bare_sleep", 9),
            ("test_a.test_module_sleep", 6),
        ]
    );
    let messages: BTreeSet<&str> = found.iter().map(|f| f.message.as_str()).collect();
    assert!(messages.contains(
        "test_a.test_module_sleep sleeps 0.5s - wall-clock waits make tests slow and flaky"
    ));
}
