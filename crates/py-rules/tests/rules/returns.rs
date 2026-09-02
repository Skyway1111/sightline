//! #53 raise-contract and #33's sentinel arm, each positive beside the
//! near-miss twin that stays silent.

use std::collections::BTreeSet;

use sightline_core::findings::Finding;
use sightline_core::text::declared_raises;
use sightline_testkit::run_rule;

const GOOGLE: &str = concat!(
    "    \"\"\"Parse.\n\n    Raises:\n        ValueError: if empty\n",
    "            and continues.\n        errors.ParseError, KeyError: on grammar\n",
    "\n    Returns:\n        A tree.\n    \"\"\"\n",
);

const DOC: &str = "    \"\"\"Run.\n\n    Raises:\n        ValueError: if bad\n    \"\"\"\n";

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

// --- `declared_raises`, which lives in `core::text` --------------------------

#[test]
fn google_numpy_and_sphinx_sections() {
    let google = GOOGLE.replacen("    ", "", 1);
    let numpy = concat!(
        "Parse.\n\n    Parameters\n    ----------\n    s : str\n\n",
        "    Raises\n    ------\n    ValueError\n        if empty\n",
        "    ParseError\n\n    Returns\n    -------\n    tree\n    ",
    );
    let sphinx = concat!(
        "Parse.\n\n    :param s: text\n    :raises ValueError: if empty\n",
        "    :raise errors.ParseError, TypeError: on grammar\n    :returns: a tree\n",
    );
    assert_eq!(
        declared_raises(&google),
        set(&["ValueError", "ParseError", "KeyError"])
    );
    assert_eq!(declared_raises(numpy), set(&["ValueError", "ParseError"]));
    assert_eq!(
        declared_raises(sphinx),
        set(&["ValueError", "ParseError", "TypeError"])
    );
}

#[test]
fn no_section_or_a_prose_head_declares_nothing() {
    assert!(declared_raises("Parse.\n\n    Args:\n        s: text.\n").is_empty());
    assert!(declared_raises("Raises:\n").is_empty());
    assert!(declared_raises("Raises:\n    when empty: see above\n").is_empty());
}

// --- #53 raise contract ------------------------------------------------------

/// KeyError, declared and never raised, is no one's claim: what an external
/// call or a subscript raises no summary sees, so there is no stale arm.
#[test]
fn undeclared_fires_and_the_declared_twin_is_silent() {
    let source = format!(
        "import errors\ndef parse(s):\n{GOOGLE}\
         \x20   if not s:\n        raise ValueError(s)\n\
         \x20   if s == 'x':\n        raise errors.ParseError(s)\n\
         \x20   if s == 'y':\n        raise TypeError(s)\n\
         \x20   return s\n\
         def declared(s):\n{GOOGLE}\
         \x20   if not s:\n        raise ValueError(s)\n\
         \x20   raise errors.ParseError(s)\n"
    );
    let findings = run_rule(
        "53",
        &[
            ("errors.py", "class ParseError(Exception):\n    pass\n"),
            ("m.py", &source),
        ],
    );
    assert_eq!(
        causes(&findings),
        ["raise-contract:undeclared:m.parse:TypeError"]
    );
}

/// A handler naming the type, a base of it, or bare catches the raise; one
/// that re-raises bare lets it out, and a raise in a handler, an `else` or a
/// `finally` sits outside the try's protection.
#[test]
fn a_raise_its_own_try_catches_never_escapes() {
    let doc = "    \"\"\"Run.\n\n    Raises:\n        ValueError: if bad\n    \"\"\"\n";
    let body = "    try:\n        raise KeyError(x)\n";
    let source = format!(
        "import logging\n\
         def caught(x):\n{doc}{body}    except KeyError:\n        return None\n\
         def caught_base(x):\n{doc}{body}    except LookupError:\n        return None\n\
         def caught_tuple(x):\n{doc}{body}    except (TypeError, KeyError):\n        return None\n\
         def caught_bare(x):\n{doc}{body}    except:\n        return None\n\
         def reraised(x):\n{doc}{body}    except KeyError:\n        logging.error(x)\n        raise\n\
         def other_handler(x):\n{doc}{body}    except TypeError:\n        return None\n\
         def in_handler(x):\n{doc}    try:\n        pass\n    except KeyError:\n        raise TypeError(x)\n\
         def in_else(x):\n{doc}    try:\n        pass\n    except KeyError:\n        pass\n    else:\n        raise IndexError(x)\n"
    );
    let findings = run_rule("53", &[("m.py", &source)]);
    assert_eq!(
        causes(&findings),
        [
            "raise-contract:undeclared:m.reraised:KeyError",
            "raise-contract:undeclared:m.other_handler:KeyError",
            "raise-contract:undeclared:m.in_handler:TypeError",
            "raise-contract:undeclared:m.in_else:IndexError",
        ]
    );
}

/// `raise make(...)` with `make -> ParseError` raises ParseError, not `make`;
/// an unannotated factory names nothing.
#[test]
fn a_raised_repo_factory_call_reads_its_return_annotation() {
    let source = format!(
        "import errors\nfrom errors import make\nfrom pkg import make2\n\
         def parse(s):\n{DOC}    if not s:\n        raise make(s)\n\
         def parse_re(s):\n{DOC}    if not s:\n        raise make2(s)\n\
         def parse_attr(s):\n{DOC}    if not s:\n        raise errors.make(s)\n\
         def loose(s):\n{DOC}    if not s:\n        raise errors.make_bare(s)\n\
         def declared(s):\n\
         \x20   \"\"\"Run.\n\n    Raises:\n        ParseError: if bad\n    \"\"\"\n\
         \x20   raise make(s)\n"
    );
    let findings = run_rule(
        "53",
        &[
            (
                "errors.py",
                concat!(
                    "class ParseError(Exception):\n    pass\n",
                    "def make(msg) -> ParseError:\n    return ParseError(msg)\n",
                    "def make_bare(msg):\n    return ParseError(msg)\n",
                ),
            ),
            ("pkg/__init__.py", "from pkg.impl import make2\n"),
            (
                "pkg/impl.py",
                "class E(Exception):\n    pass\ndef make2(msg) -> E:\n    return E(msg)\n",
            ),
            ("m.py", &source),
        ],
    );
    assert_eq!(
        causes(&findings),
        [
            "raise-contract:undeclared:m.parse:ParseError",
            "raise-contract:undeclared:m.parse_re:E",
            "raise-contract:undeclared:m.parse_attr:ParseError",
        ]
    );
}

#[test]
fn a_declared_base_names_its_subclasses() {
    let doc = "    \"\"\"Run.\n\n    Raises:\n        GateError: one of the two\n    \"\"\"\n";
    let source = format!(
        "class GateError(Exception):\n    pass\n\
         class CycleError(GateError):\n    pass\n\
         class Other(Exception):\n    pass\n\
         def gate(x):\n{doc}    if x:\n        raise CycleError(x)\n\
         def wrong(x):\n{doc}    if x:\n        raise Other(x)\n"
    );
    let findings = run_rule("53", &[("m.py", &source)]);
    assert_eq!(
        causes(&findings),
        ["raise-contract:undeclared:m.wrong:Other"]
    );
}

#[test]
fn a_declared_builtin_base_names_its_builtin_subclasses() {
    let doc =
        "    \"\"\"Run.\n\n    Raises:\n        OSError: when the file is missing\n    \"\"\"\n";
    let source = format!(
        "class MissingConfig(FileNotFoundError):\n    pass\n\
         def read(p):\n{doc}    if p:\n        raise FileNotFoundError(p)\n\
         def config(p):\n{doc}    if p:\n        raise MissingConfig(p)\n\
         def pick(p):\n{doc}    if p:\n        raise KeyError(p)\n"
    );
    let findings = run_rule("53", &[("m.py", &source)]);
    assert_eq!(
        causes(&findings),
        ["raise-contract:undeclared:m.pick:KeyError"]
    );
}

#[test]
fn private_nested_test_and_placeholder_defs_are_silent() {
    let indented = DOC.replace("\n    ", "\n        ");
    let source = format!(
        "from abc import abstractmethod\n\
         def _hidden(x):\n{DOC}    raise KeyError(x)\n\
         def outer(x):\n    def inner(y):\n    {indented}        raise KeyError(y)\n    return inner(x)\n\
         class Base:\n\
         \x20   @abstractmethod\n    def run(self, x):\n    {indented}        raise NotImplementedError\n\
         \x20   def stub(self, x):\n    {indented}        raise NotImplementedError('later')\n"
    );
    let findings = run_rule(
        "53",
        &[
            ("_internal/__init__.py", ""),
            (
                "_internal/impl.py",
                &format!("def load(x):\n{DOC}    raise KeyError(x)\n"),
            ),
            ("m.py", &source),
            (
                "tests/test_m.py",
                &format!("def test_it(x):\n{DOC}    raise KeyError(x)\n"),
            ),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

// --- #33 sentinel arm --------------------------------------------------------

/// `""` under `-> str` (`name`) is text, not a marker: cut at the sample.
#[test]
fn a_sentinel_fires_beside_a_computed_value_and_not_on_literals_alone() {
    let findings = run_rule(
        "33",
        &[(
            "m.py",
            concat!(
                "def find(xs, x) -> int:\n",
                "    for i, y in enumerate(xs):\n        if y == x:\n            return i\n",
                "    return -1\n",
                "def name(x) -> str:\n",
                "    if x:\n        return str(x)\n",
                "    return ''\n",
                "def code(x) -> int:\n",
                "    if x:\n        return 1\n",
                "    return -1\n",
                "def zero(xs) -> int:\n",
                "    if xs:\n        return len(xs)\n",
                "    return 0\n",
                "def optional(xs, x) -> int | None:\n",
                "    if x in xs:\n        return xs.index(x)\n",
                "    return -1\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["sentinel:m.find"]);
    assert!(
        findings[0]
            .message
            .contains("-1 beside a computed value under `-> int`")
    );
}

// --- `is_exported` in a repo that publishes ----------------------------------

/// An unpublished module's public def is still the boundary its own package
/// calls: reading only published modules dropped 96 judged #50 reals on GLM-V
/// and cerberus for 0 false positives at the todo-round-2 close.
fn exported_fixture(pyproject: &str) -> Vec<String> {
    let parse = format!("def parse(s):\n{GOOGLE}    raise TypeError(s)\n");
    let findings = run_rule(
        "53",
        &[
            ("src/mypkg/api.py", &parse),
            ("internal/tooling.py", &parse),
            ("internal/__init__.py", ""),
            ("src/mypkg/__init__.py", ""),
            ("pyproject.toml", pyproject),
        ],
    );
    let mut out: Vec<String> = findings.iter().map(|f| f.cause.clone()).collect();
    out.sort();
    out
}

const BOTH: [&str; 2] = [
    "raise-contract:undeclared:internal.tooling.parse:TypeError",
    "raise-contract:undeclared:mypkg.api.parse:TypeError",
];

#[test]
fn an_unpublished_module_still_declares_its_contract() {
    let rows = exported_fixture(
        "[project]\nname = \"mypkg\"\n\n[build-system]\nrequires = [\"setuptools\"]\n",
    );
    assert_eq!(rows, BOTH);
}

#[test]
fn an_application_exports_both() {
    let rows = exported_fixture("[project]\nname = \"mypkg\"\n");
    assert_eq!(rows, BOTH);
}
