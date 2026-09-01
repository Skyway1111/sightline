//! `tests/rules/test_oracle_rules.py`, the #2, #5 and #10 classes. Every test
//! here builds an in-process checker at the mini repo's root.

use std::collections::BTreeSet;

use camino::Utf8Path;
use sightline_core::findings::{Finding, Tier};
use sightline_py_provers::oracle::Oracle;
use sightline_testkit::{PyStack, build, run_rule, run_rule_on};
use tempfile::TempDir;

/// The mini repo with a checker at the same root (`conftest.py:run_oracle_rule`).
fn with_oracle(files: &[(&str, &str)]) -> (TempDir, PyStack) {
    let (dir, mut stack) = build(files);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let import_roots = stack.facts().import_roots.clone();
    stack.provers.oracle =
        Some(Oracle::new(root, &[], &import_roots, None).expect("an in-process checker"));
    (dir, stack)
}

/// One rule over an inline repo the checker also sees. Two fixtures in one
/// test never share the machine, so the provers close before the return.
fn oracle_rule(id: &str, files: &[(&str, &str)]) -> Vec<Finding> {
    let (_dir, mut stack) = with_oracle(files);
    let out = run_rule_on(id, &stack);
    stack.provers.close();
    out
}

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

fn symbol_tiers(findings: &[Finding]) -> Vec<(&str, Tier)> {
    findings
        .iter()
        .map(|f| (&*f.site.symbol, f.tier()))
        .collect()
}

fn symbols(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| &*f.site.symbol).collect()
}

fn tier_of(findings: &[Finding], symbol: &str) -> Option<Tier> {
    findings
        .iter()
        .find(|f| &*f.site.symbol == symbol)
        .map(Finding::tier)
}

// --- #2 locally-redundant check ----------------------------------------------

#[test]
fn only_a_type_the_repo_wrote_is_reported() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "def grounded(x: str) -> bool:\n    return isinstance(x, str)\n",
                "def ungrounded(v):\n    s = 'lit'\n    return isinstance(s, str)\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.grounded", Tier::Proved)]);
    assert_eq!(causes(&findings), ["redundant:isinstance"]);
}

/// pyright's no-overlap claim ignores `__eq__` (`set == frozenset`, `float ==
/// 0` are valid Python): a `==` / `!=` verdict is the checker's.
#[test]
fn equality_comparisons_are_not_the_repos_claim() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "def eq_case(x: int) -> bool:\n    return x == ''\n",
                "def id_case(x: int) -> bool:\n    return x is None\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.id_case", Tier::Proved)]);
}

/// `x: T = None` lies: the default proves x can be None at runtime, so the
/// checker's verdict rests on an annotation the default contradicts. #1 owns
/// the annotation defect, and #2 emits nothing at any tier.
#[test]
fn none_check_on_none_default_never_reported() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "def lying(x: int = None) -> bool:\n    return x is None\n",
                "def honest(x: int) -> bool:\n    return x is None\n",
            ),
        )],
    );
    assert!(!symbols(&findings).contains(&"m.lying"), "{findings:?}");
    assert_eq!(tier_of(&findings, "m.honest"), Some(Tier::Proved));
}

/// A dataclass field `x: T = None` lies exactly as a param does.
#[test]
fn none_check_on_none_default_field_never_reported() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "import dataclasses\n",
                "@dataclasses.dataclass\n",
                "class C:\n",
                "    lying: int = None\n",
                "    honest: int = 0\n",
                "    def check_lying(self) -> bool:\n        return self.lying is None\n",
                "    def check_honest(self) -> bool:\n        return self.honest is None\n",
            ),
        )],
    );
    assert!(
        !symbols(&findings).contains(&"m.C.check_lying"),
        "{findings:?}"
    );
    assert_eq!(tier_of(&findings, "m.C.check_honest"), Some(Tier::Proved));
}

/// `node: AST` rebound to `parent() -> AST | None` draws an
/// invalid-assignment from the checker, whose `is None` verdict then reads the
/// declaration, not the value. A valid rebinding (`x = x or 0`) keeps its
/// verdict and the unrebound entry check stays proved.
#[test]
fn verdict_on_a_declaration_the_body_broke_never_reported() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "import ast\n",
                "def parent(n: ast.AST) -> ast.AST | None:\n    return None\n",
                "def back_edge(node: ast.AST) -> bool:\n",
                "    while node is not None and not isinstance(node, ast.stmt):\n",
                "        node = parent(node)\n",
                "    return isinstance(node, ast.Raise)\n",
                "def straight(node: ast.AST) -> bool:\n",
                "    node = parent(node)\n",
                "    return node is None\n",
                "def valid(x: int | None) -> bool:\n",
                "    x = x or 0\n",
                "    return x is None\n",
                "def entry(node: ast.AST) -> bool:\n",
                "    return node is None\n",
                "def honest(node: ast.AST | None) -> bool:\n",
                "    while node is not None and not isinstance(node, ast.stmt):\n",
                "        node = parent(node)\n",
                "    return isinstance(node, ast.Raise)\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.entry", Tier::Proved)]);
}

/// `isinstance(True, numbers.Integral)` is True at runtime: bool joins the
/// numeric tower by `ABCMeta.register()`, which nominal no-overlap reasoning
/// cannot see.
#[test]
fn isinstance_vs_abc_registered_is_never_the_repos_claim() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "import numbers\n",
                "def lying(x: numbers.Integral) -> bool:\n    return isinstance(x, bool)\n",
                "def honest(x: str) -> bool:\n    return isinstance(x, str)\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.honest", Tier::Proved)]);
}

/// The rule may only rest on annotations the repo wrote. A locally-AnnAssign'd
/// field keeps its proved grounding: the pinned discriminator.
#[test]
fn inferred_attribute_is_never_the_repos_claim() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "class Head:\n",
                "    bias: int = 0\n",
                "    def __init__(self) -> None:\n        self.scale = 1\n",
                "    def check_inferred(self) -> bool:\n        return self.scale is None\n",
                "    def check_declared(self) -> bool:\n        return self.bias is None\n",
            ),
        )],
    );
    assert_eq!(
        symbol_tiers(&findings),
        [("m.Head.check_declared", Tier::Proved)]
    );
}

/// `c.limit is None` grounds only when the chain resolves root to leaf through
/// locally-annotated fields of internal classes.
#[test]
fn param_rooted_chain_grounds_through_declared_fields() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "class Layer:\n    bias: int = 0\n",
                "class Cfg:\n",
                "    limit: int = 0\n",
                "    sub: Layer\n",
                "    def __init__(self) -> None:\n        self.raw = 1\n",
                "def declared(c: Cfg) -> bool:\n    return c.limit is None\n",
                "def chained(c: Cfg) -> bool:\n    return c.sub.bias is None\n",
                "def inferred(c: Cfg) -> bool:\n    return c.raw is None\n",
            ),
        )],
    );
    assert_eq!(
        symbol_tiers(&findings),
        [("m.declared", Tier::Proved), ("m.chained", Tier::Proved)]
    );
}

/// Assigning an inferred-typed value to a local must not launder it into the
/// rule; an AnnAssign'd local is a repo-written annotation.
#[test]
fn local_temp_does_not_launder() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "class Head:\n",
                "    def __init__(self) -> None:\n        self.scale = 1\n",
                "    def laundered(self) -> bool:\n",
                "        s = self.scale\n        return s is None\n",
                "    def declared_local(self) -> bool:\n",
                "        n: int = 3\n        return n is None\n",
            ),
        )],
    );
    assert_eq!(
        symbol_tiers(&findings),
        [("m.Head.declared_local", Tier::Proved)]
    );
}

/// `path: str` with an is-None guard and a caller passing None: the annotation
/// lies, and only the caller proves it.
#[test]
fn interprocedural_none_caller_demotes() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "def save(path: str) -> int:\n",
                "    if path is None:\n        return 0\n    return 1\n",
                "def honest(path: str) -> int:\n",
                "    if path is None:\n        return 0\n    return 1\n",
                "def use() -> None:\n    save(None)\n    honest('x')\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.honest", Tier::Proved)]);
}

/// `case x:` stores to the param, so the declared type does not cover the
/// check; a comprehension's `x` is its own scope's.
#[test]
fn match_capture_rebinds_and_comprehension_target_does_not() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "def plain(x: int) -> int:\n",
                "    if x is None:\n        return 0\n    return 1\n",
                "def captured(x: int | None, y: int) -> int:\n",
                "    match y:\n        case x:\n            pass\n",
                "    if x is None:\n        return 0\n    return 1\n",
                "def listed(x: int, ys: list[int]) -> int:\n",
                "    zs = [x for x in ys]\n",
                "    if x is None:\n        return len(zs)\n    return 1\n",
            ),
        )],
    );
    let mut rows: Vec<(&str, Tier)> = symbol_tiers(&findings);
    rows.sort_by_key(|(s, _)| *s);
    assert_eq!(
        rows,
        [("m.listed", Tier::Proved), ("m.plain", Tier::Proved)]
    );
}

/// `isinstance(rec, dict)` on a record the annotation only claims is what
/// makes the claim true; a concrete class's check still fires.
#[test]
fn a_container_shape_check_is_the_boundary_validation() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "class Node:\n    pass\n",
                "def load(rec: dict) -> int:\n",
                "    if not isinstance(rec, dict):\n        return 0\n",
                "    return len(rec)\n",
                "def visit(n: Node) -> int:\n",
                "    if not isinstance(n, Node):\n        return 0\n",
                "    return 1\n",
            ),
        )],
    );
    assert_eq!(symbols(&findings), ["m.visit"]);
}

/// `device: Path = "cpu"` is a declaration the def's own default breaks:
/// nothing else in that signature is an enforced claim either.
#[test]
fn a_signature_the_checker_rejected_declares_nothing() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "from pathlib import Path\n",
                "def lying(n: int, device: Path = 'cpu') -> bool:\n",
                "    return isinstance(n, float)\n",
                "def honest(n: int, device: Path = Path('cpu')) -> bool:\n",
                "    return isinstance(n, float)\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.honest", Tier::Proved)]);
}

/// `f: T = field(default=None)` under a type: ignore is the same lie as
/// `f: T = None`, spelled through the dataclasses helper.
#[test]
fn a_field_defaulted_none_through_dataclasses_field() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "import dataclasses\n",
                "@dataclasses.dataclass\n",
                "class C:\n",
                "    lying: int = dataclasses.field(default=None)  # type: ignore\n",
                "    honest: int = dataclasses.field(default=0)\n",
                "    def check_lying(self) -> bool:\n        return self.lying is None\n",
                "    def check_honest(self) -> bool:\n        return self.honest is None\n",
            ),
        )],
    );
    assert_eq!(symbols(&findings), ["m.C.check_honest"]);
}

/// `if x is None: x = D` and `x = x if x is not None else D` are the def
/// saying None arrives; a guard that raises instead is the real redundancy.
#[test]
fn a_none_fallback_is_the_def_contradicting_itself() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "def defaults(n: int = 3) -> int:\n",
                "    if n is None:\n        n = 5\n    return n\n",
                "def ternary(groups: int, n: int = 3) -> int:\n",
                "    n = n if n is not None else groups\n    return n\n",
                "def guards(n: int) -> int:\n",
                "    if n is None:\n        raise ValueError('n')\n    return n\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.guards", Tier::Proved)]);
}

/// Another task runs at the suspension point: a field's narrowed type is the
/// checker's, not the repo's, past an await.
#[test]
fn an_attribute_narrowing_never_survives_an_await() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "import asyncio\n",
                "class Proc:\n    pid: int = 0\n",
                "class Runner:\n",
                "    proc: Proc | None\n",
                "    async def start(self) -> None:\n",
                "        self.proc = Proc()\n",
                "        await asyncio.sleep(0)\n",
                "        if self.proc is None:\n",
                "            raise RuntimeError('gone')\n",
                "    async def straight(self) -> None:\n",
                "        self.proc = Proc()\n",
                "        if self.proc is None:\n",
                "            raise RuntimeError('gone')\n",
            ),
        )],
    );
    assert_eq!(symbols(&findings), ["m.Runner.straight"]);
}

#[test]
fn rule_2_silent_on_clean_and_without_oracle() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            "def f(x: int | str) -> bool:\n    return isinstance(x, str)\n",
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
    let findings = run_rule(
        "2",
        &[(
            "n.py",
            "def f(x: str) -> bool:\n    return isinstance(x, str)\n",
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

// --- #2 operand grounding ----------------------------------------------------

#[test]
fn plain_local_isinstance_is_not_the_repos_claim() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "def laundered(x: int) -> bool:\n",
                "    s = str(x)\n    return isinstance(s, str)\n",
                "def declared(x: int) -> bool:\n    return isinstance(x, int)\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.declared", Tier::Proved)]);
}

/// A cached_property the repo never annotated: the checker's descriptor
/// resolution is not a claim the repo wrote.
#[test]
fn unannotated_descriptor_chain_is_not_the_repos_claim() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "class C:\n",
                "    flag: bool = True\n",
                "    def __init__(self) -> None:\n        self.field = True\n",
                "    def inferred(self) -> bool:\n",
                "        return isinstance(self.field, bool)\n",
                "    def declared(self) -> bool:\n",
                "        return isinstance(self.flag, bool)\n",
            ),
        )],
    );
    assert_eq!(symbol_tiers(&findings), [("m.C.declared", Tier::Proved)]);
}

/// `while isinstance(node, Base): node = node.parent`: the annotation covers
/// the entry value, the loop rebinds past it. A rebinding after the check does
/// not reach it.
#[test]
fn rebound_param_is_not_the_repos_claim() {
    let findings = oracle_rule(
        "2",
        &[(
            "m.py",
            concat!(
                "class Node:\n    parent: 'Node'\n",
                "def walk(node: Node) -> int:\n",
                "    n = 0\n",
                "    while isinstance(node, Node):\n",
                "        node = node.parent\n",
                "        n += 1\n",
                "    return n\n",
                "def check(node: Node) -> bool:\n    return isinstance(node, Node)\n",
                "def later(raw: str) -> str:\n",
                "    if not isinstance(raw, str):\n        return ''\n",
                "    raw = raw.strip()\n",
                "    return raw\n",
            ),
        )],
    );
    let mut rows: Vec<(&str, Tier)> = symbol_tiers(&findings);
    rows.sort_by_key(|(s, _)| *s);
    assert_eq!(rows, [("m.check", Tier::Proved), ("m.later", Tier::Proved)]);
}

/// `assertTrue(isinstance(...))`: statically always true is the point of the
/// test, not a redundant check.
#[test]
fn test_path_isinstance_is_the_subject() {
    let findings = oracle_rule(
        "2",
        &[(
            "tests/test_m.py",
            "def check(v: str) -> bool:\n    return isinstance(v, str)\n",
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

// --- #5 proof lifting --------------------------------------------------------

/// `json.dumps(cls=E)` calls `E.default` from library code the graph never
/// sees: #5 stays silent there and fires on the internal-only twin.
#[test]
fn library_dispatch_keeps_the_world_open() {
    let findings = oracle_rule(
        "5",
        &[(
            "enc.py",
            concat!(
                "import json\n",
                "class E(json.JSONEncoder):\n",
                "    def default(self, o):\n",
                "        if isinstance(o, set):\n            return sorted(o)\n",
                "        return super().default(o)\n",
                "class B:\n    pass\n",
                "class F(B):\n",
                "    def default(self, o):\n",
                "        if isinstance(o, set):\n            return sorted(o)\n",
                "        return o\n",
                "def main():\n",
                "    E().default({1, 2})\n    F().default({1, 2})\n",
                "    return json.dumps({'a': {1}}, cls=E)\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["lift:enc.F.default:o"]);
}

/// `Literal["a, b"]` is one str, not a str and a bytes.
#[test]
fn a_literals_comma_is_one_value() {
    let findings = oracle_rule(
        "5",
        &[(
            "lit.py",
            concat!(
                "def k(p):\n    if isinstance(p, str):\n        return p.upper()\n    return ''\n",
                "def main():\n    k('a, b')\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["lift:lit.k:p"]);
    assert!(findings[0].message.contains("lift `p: str` in lit.k"));
}

/// Every discovered caller passing literal None is the absence of evidence,
/// not a type: proposing `param: None` would forbid the only meaningful call.
#[test]
fn none_only_callers_never_lift() {
    let findings = oracle_rule(
        "5",
        &[(
            "m.py",
            concat!(
                "def _sink(msg):\n    return msg\n",
                "def a():\n    return _sink(None)\n",
                "def b():\n    return _sink(None)\n",
                "def _keep(msg):\n    return msg\n",
                "def c() -> str:\n    return _keep('x')\n",
                "def d() -> str:\n    return _keep('y')\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["lift:m._keep:msg"]);
}

/// Callers sent only str, but the body isinstance-accepts tuple: the lift
/// would encode a contract the code contradicts.
#[test]
fn body_isinstance_blocks_narrow_lift() {
    let findings = oracle_rule(
        "5",
        &[(
            "m.py",
            concat!(
                "def _sink(names):\n",
                "    if isinstance(names, (list, tuple)):\n",
                "        return len(names)\n",
                "    return 1\n",
                "def a() -> int:\n    return _sink('x')\n",
                "def b() -> int:\n    return _sink('y')\n",
                "def _keep(names):\n    return names\n",
                "def c() -> str:\n    return _keep('x')\n",
                "def d() -> str:\n    return _keep('y')\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["lift:m._keep:names"]);
}

#[test]
fn clean_lift_emitted_indexed_with_receipt_text() {
    let findings = oracle_rule(
        "5",
        &[(
            "m.py",
            concat!(
                "def _scale(nums):\n",
                "    out = []\n",
                "    for n in nums:\n",
                "        out.append(n * 2)\n",
                "    return out\n",
                "def use1() -> list:\n    return _scale([1, 2])\n",
                "def use2() -> list:\n    return _scale([3])\n",
            ),
        )],
    );
    assert_eq!(findings.len(), 1, "{findings:?}");
    let f = &findings[0];
    assert_eq!(f.cause, "lift:m._scale:nums");
    assert_eq!(f.tier(), Tier::Indexed);
    assert!(f.message.contains("nums: list[int]"));
    // no unverified clause: the protocol ladder is #10's
    assert!(!f.message.contains("suggest"));
    assert!(!f.message.contains("Iterable"));
    // every lift holds its receipt
    assert!(f.message.contains("receipt"));
}

#[test]
fn receipted_lift_is_proved() {
    let findings = oracle_rule(
        "5",
        &[(
            "m.py",
            concat!(
                "def _fmt(v):\n",
                "    if isinstance(v, str):\n        return v\n",
                "    return 'other'\n",
                "def a() -> str:\n    return _fmt('x')\n",
                "def b() -> str:\n    return _fmt('y')\n",
            ),
        )],
    );
    let lift = findings
        .iter()
        .find(|f| f.cause == "lift:m._fmt:v")
        .expect("the lift fires");
    assert_eq!(lift.tier(), Tier::Proved);
    assert!(lift.message.contains("provably redundant"));
}

/// The join comes from prod alone (`list[int]`); the test caller passing a str
/// then errors under the shadow annotation, so the splice is vetoed.
#[test]
fn test_caller_type_conflict_vetoes() {
    let findings = oracle_rule(
        "5",
        &[
            (
                "m.py",
                concat!(
                    "def scale(nums):\n",
                    "    return [n * 2 for n in nums]\n",
                    "def use() -> list:\n    return scale([1])\n",
                ),
            ),
            (
                "tests/test_m.py",
                "from m import scale\ndef test_bad() -> None:\n    scale('not-a-list')\n",
            ),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn three_way_disagreement_stays_silent() {
    let findings = oracle_rule(
        "5",
        &[(
            "m.py",
            concat!(
                "def _accept(v):\n    return v\n",
                "def a():\n    return _accept(1)\n",
                "def b():\n    return _accept('s')\n",
                "def c():\n    return _accept(b'x')\n",
            ),
        )],
    );
    assert!(
        !causes(&findings)
            .iter()
            .any(|c| c.starts_with("lift:m._accept")),
        "{findings:?}"
    );
}

// --- #5 depth: abc-spelled lifts and defaulted params ------------------------

const ABC_CALLEE: &str = concat!(
    "def _core(xs):\n",
    "    if xs is None:\n",
    "        return []\n",
    "    return list(xs)\n",
);
const ABC_CALLERS: &str = concat!(
    "from collections.abc import Sequence\n",
    "from m import _core\n",
    "def outer(xs: Sequence[int]) -> list:\n",
    "    return _core(xs)\n",
    "def outer2(xs: Sequence[int]) -> list:\n",
    "    return _core(xs)\n",
);

#[test]
fn abc_lift_verified_with_receipt() {
    let findings = oracle_rule("5", &[("m.py", ABC_CALLEE), ("o.py", ABC_CALLERS)]);
    let lift = findings
        .iter()
        .find(|f| f.cause == "lift:m._core:xs")
        .expect("the lift fires");
    assert!(lift.message.contains("xs: Sequence[int]"));
    // the body None-check became the receipt
    assert_eq!(lift.tier(), Tier::Proved);
    assert!(lift.message.contains("provably redundant"));
}

#[test]
fn abc_lift_veto_honored() {
    let findings = oracle_rule(
        "5",
        &[
            ("m.py", ABC_CALLEE),
            ("o.py", ABC_CALLERS),
            (
                "tests/test_m.py",
                "from m import _core\ndef test_bad() -> None:\n    _core(3)\n",
            ),
        ],
    );
    assert!(
        !causes(&findings).contains(&"lift:m._core:xs"),
        "{findings:?}"
    );
}

#[test]
fn defaulted_param_join_includes_default_type() {
    let findings = oracle_rule(
        "5",
        &[(
            "m.py",
            concat!(
                "def _fmt(v=None):\n    return v\n",
                "def a() -> None:\n    _fmt('x')\n",
                "def b() -> None:\n    _fmt()\n",
            ),
        )],
    );
    let lift = findings
        .iter()
        .find(|f| f.cause == "lift:m._fmt:v")
        .expect("the lift fires");
    // the default's type joined; a clean verify has no receipt
    assert!(lift.message.contains("v: None | str"));
    assert_eq!(lift.tier(), Tier::Indexed);
}

// --- #5 bound-class transport ------------------------------------------------

const SHAPES: &str = "class Box:\n    def __init__(self, w: int) -> None:\n        self.w = w\n";
const WIDTH_CALLERS: &str = concat!(
    "from shapes import Box\nfrom m import _width\n",
    "def use1() -> int:\n    return _width(Box(1))\n",
    "def use2() -> int:\n    return _width(Box(2))\n",
);

fn width_files(head: &str) -> [(&'static str, String); 3] {
    [
        ("shapes.py", SHAPES.to_string()),
        (
            "m.py",
            format!("{head}def _width(b):\n    if b is None:\n        return 0\n    return b.w\n"),
        ),
        ("o.py", WIDTH_CALLERS.to_string()),
    ]
}

fn run_width(head: &str) -> Vec<Finding> {
    let owned = width_files(head);
    let files: Vec<(&str, &str)> = owned.iter().map(|(r, s)| (*r, s.as_str())).collect();
    oracle_rule("5", &files)
}

#[test]
fn bound_repo_class_lifts_without_an_import() {
    let findings = run_width("from shapes import Box\n");
    let lift = findings
        .iter()
        .find(|f| f.cause == "lift:m._width:b")
        .expect("the lift fires");
    assert!(lift.message.contains("lift `b: Box` in m._width"));
    // the None-check is the receipt
    assert_eq!(lift.tier(), Tier::Proved);
    let fix = lift.fix.as_ref().expect("a verified splice holds its fix");
    assert!(fix.imports.is_empty());
}

#[test]
fn unbound_or_rebound_name_stays_silent() {
    for head in ["", "Box = 3\n"] {
        let findings = run_width(head);
        assert!(
            !causes(&findings).contains(&"lift:m._width:b"),
            "head {head:?}: {findings:?}"
        );
    }
}

/// The oracle displays `AST`; a callee file that only imports `ast` lifts to
/// `ast.AST`, the message and the patch spelling it so.
#[test]
fn a_stdlib_class_bound_only_through_its_module_is_respelled() {
    let findings = oracle_rule(
        "5",
        &[
            (
                "m.py",
                "import ast\ndef _line(node):\n    if node is None:\n        return 0\n    return 1\n",
            ),
            (
                "o.py",
                concat!(
                    "import ast\nfrom m import _line\n",
                    "def use1(n: ast.AST) -> int:\n    return _line(n)\n",
                    "def use2(n: ast.AST) -> int:\n    return _line(n)\n",
                ),
            ),
        ],
    );
    let lift = findings
        .iter()
        .find(|f| f.cause == "lift:m._line:node")
        .expect("the lift fires");
    assert!(lift.message.contains("lift `node: ast.AST` in m._line"));
    let fix = lift.fix.as_ref().expect("a verified splice holds its fix");
    assert!(fix.imports.is_empty());
    assert_eq!(fix.edits[0].text, ": ast.AST");
}

/// `typing.Any` is a binding the transport would carry; a caller whose
/// argument is Any proves nothing, so #5 never lifts to it.
#[test]
fn an_any_member_establishes_nothing() {
    for (sent, lifted) in [("str", Some("v: str")), ("Any", None)] {
        let source = format!(
            "from typing import Any\ndef _echo(v):\n    return v\n\
             def use(x: {sent}) -> None:\n    _echo(x)\n"
        );
        let findings = oracle_rule("5", &[("m.py", &source)]);
        let lifts: Vec<&str> = findings
            .iter()
            .filter(|f| f.cause == "lift:m._echo:v")
            .map(|f| f.message.as_str())
            .collect();
        match lifted {
            None => assert!(lifts.is_empty(), "{sent}: {findings:?}"),
            Some(text) => assert!(lifts[0].contains(text), "{sent}: {lifts:?}"),
        }
    }
}

// --- #5 through the oracle's call graph --------------------------------------

/// `a.scale(...)` on a typed receiver is CHA-ambiguous by name (`A.scale` vs
/// `B.scale`), so facts see no caller of `A.scale`; the oracle edge resolves
/// it, and the veto watches every file the graph reaches.
const AMBIGUOUS_RECEIVER: &str = concat!(
    "class A:\n",
    "    def scale(self, nums):\n",
    "        return [n * 2 for n in nums]\n",
    "class B:\n",
    "    def scale(self, nums: str) -> str:\n",
    "        return nums\n",
    "def use(a: A) -> list:\n",
    "    return a.scale([1])\n",
);

#[test]
fn lift_fires_through_an_oracle_resolved_caller() {
    use sightline_py_facts::model::Resolution;

    let (_dir, mut stack) = with_oracle(&[("m.py", AMBIGUOUS_RECEIVER)]);
    let findings = run_rule_on("5", &stack);
    assert_eq!(causes(&findings), ["lift:m.A.scale:nums"]);
    // facts alone: the plain receiver is a CHA guess across both `scale`
    // bodies, never a caller edge
    let sites = &stack.facts().call_sites;
    assert_eq!(sites.len(), 1, "{sites:?}");
    assert_eq!(sites[0].resolution, Resolution::Ambiguous);
    let candidates: BTreeSet<String> = sites[0].candidates.iter().map(|c| c.to_string()).collect();
    assert_eq!(
        candidates,
        BTreeSet::from(["m.A.scale".to_string(), "m.B.scale".to_string()])
    );
    stack.provers.close();
}

#[test]
fn oracle_resolved_test_caller_vetoes() {
    let findings = oracle_rule(
        "5",
        &[
            ("m.py", AMBIGUOUS_RECEIVER),
            (
                "tests/test_m.py",
                "from m import A\ndef test_bad() -> None:\n    A().scale('x')\n",
            ),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// The body keys a repo dict literal whose keys are all str: a guard naming
/// str that the callers' bool never satisfies. The key-free twin still lifts.
#[test]
fn key_use_against_a_string_keyed_dict_blocks_the_lift() {
    let findings = oracle_rule(
        "5",
        &[(
            "m.py",
            concat!(
                "EXPIRES = {'code': 10, 'token': 20}\n",
                "def _expires(kind):\n    return EXPIRES.get(kind, 0)\n",
                "def a() -> int:\n    return _expires(True)\n",
                "def b() -> int:\n    return _expires(False)\n",
                "def _plain(kind):\n    return 1 if kind else 0\n",
                "def c() -> int:\n    return _plain(True)\n",
                "def d() -> int:\n    return _plain(False)\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["lift:m._plain:kind"]);
}

/// #5 never fires on a symbol the closed world lets escape.
#[test]
fn escaped_symbols_get_no_5_findings() {
    let mut rows: Vec<&sightline_testkit::EscapeFixture> =
        sightline_testkit::ESCAPE_FIXTURES.iter().collect();
    rows.sort_by_key(|f| f.reason);
    for fixture in rows {
        let findings = oracle_rule("5", fixture.files);
        let hit: Vec<&Finding> = findings
            .iter()
            .filter(|f| f.cause.contains(fixture.symbol) || &*f.site.symbol == fixture.symbol)
            .collect();
        assert!(
            hit.is_empty(),
            "{}: #5 fired on {}",
            fixture.reason,
            fixture.symbol
        );
    }
}

// --- #10 verify arm ----------------------------------------------------------

#[test]
fn fires_on_protocol_only_use() {
    let findings = oracle_rule(
        "10",
        &[
            (
                "m.py",
                concat!(
                    "def total(xs: list[int]) -> int:\n",
                    "    acc = 0\n",
                    "    for x in xs:\n",
                    "        acc += x * 2\n",
                    "    return acc\n",
                    "def head(xs: list[str]) -> str:\n",
                    "    return xs[0]\n",
                    "def lookup(d: dict[str, int], k: str) -> int:\n",
                    "    return d.get(k, 0)\n",
                    "def orphan(xs: list[int]) -> int:\n",
                    "    return xs[0]\n",
                    "def use() -> int:\n",
                    "    return total([1]) + head(['a']) + lookup({}, 'k')\n",
                ),
            ),
            (
                "tests/test_m.py",
                "from m import orphan\ndef test_orphan() -> None:\n    assert orphan([1]) == 1\n",
            ),
        ],
    );
    let mut found: Vec<&str> = causes(&findings);
    found.sort_unstable();
    assert_eq!(
        found,
        [
            "over-constrained:m.head:xs",
            "over-constrained:m.lookup:d",
            "over-constrained:m.total:xs",
        ]
    );
    let message = |cause: &str| {
        findings
            .iter()
            .find(|f| f.cause == cause)
            .expect("the finding fires")
            .message
            .as_str()
    };
    assert!(message("over-constrained:m.total:xs").contains("Iterable[int]"));
    assert!(message("over-constrained:m.head:xs").contains("Sequence[str]"));
    assert!(message("over-constrained:m.lookup:d").contains("Mapping[str, int]"));
}

/// A framework subclass's method signature is fixed by the unseen override
/// contract; the identical free function still fires.
#[test]
fn override_fixed_signature_skipped() {
    let findings = oracle_rule(
        "10",
        &[(
            "m.py",
            concat!(
                "import fakefw\n",
                "class W(fakefw.Writer):\n",
                "    def write_rows(self, xs: list[int]) -> int:\n",
                "        n = 0\n",
                "        for x in xs:\n",
                "            n += x\n",
                "        return n\n",
                "def total(xs: list[int]) -> int:\n",
                "    n = 0\n",
                "    for x in xs:\n",
                "        n += x\n",
                "    return n\n",
                "def use() -> int:\n    return total([1]) + W().write_rows([1])\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["over-constrained:m.total:xs"]);
}

#[test]
fn silent_on_mutation_forwarding_or_no_annotation() {
    let findings = oracle_rule(
        "10",
        &[(
            "m.py",
            concat!(
                "def push(xs: list[int], v: int) -> None:\n",
                "    xs.append(v)\n",
                "def fwd(xs: list[int]) -> int:\n",
                "    return consume(xs)\n",
                "def consume(xs) -> int:\n",
                "    return len(xs)\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn verified_widening_carries_evidence_at_indexed() {
    let findings = oracle_rule(
        "10",
        &[(
            "m.py",
            concat!(
                "def total(xs: list[int]) -> int:\n",
                "    out = 0\n",
                "    for x in xs:\n",
                "        out += x\n",
                "    return out\n",
                "def use() -> int:\n    return total([1, 2])\n",
            ),
        )],
    );
    let f = findings
        .iter()
        .find(|f| f.cause == "over-constrained:m.total:xs")
        .expect("the widening fires");
    // clean verify, matching #5's clean lifts
    assert_eq!(f.tier(), Tier::Indexed);
    assert!(f.message.contains("Iterable[int]"));
    assert!(f.message.contains("widening verified"));
}

/// The footprint sees only a subscript read, so it proposes
/// `Sequence[object]`; the body arithmetic refutes it under the shadow.
#[test]
fn refuted_widening_is_dropped() {
    let findings = oracle_rule(
        "10",
        &[(
            "m.py",
            concat!(
                "def head(xs: list) -> int:\n",
                "    return xs[0] + 1\n",
                "def use() -> int:\n    return head([1])\n",
            ),
        )],
    );
    assert!(
        !causes(&findings).contains(&"over-constrained:m.head:xs"),
        "{findings:?}"
    );
}

/// An unverified widening is a guess: a degraded run may never report what the
/// run it degrades from does not.
#[test]
fn without_oracle_the_rule_is_silent() {
    let findings = run_rule(
        "10",
        &[("m.py", "def head(xs: list) -> int:\n    return xs[0] + 1\n")],
    );
    assert!(findings.is_empty(), "{findings:?}");
}
