//! `tests/rules/test_trust.py`: family A's non-oracle classes, each firing on
//! its crafted positive and silent on the paired negative.

use sightline_core::findings::{Evidence, Finding, Tier};
use sightline_testkit::run_rule;

fn detail(evidence: &Evidence) -> &str {
    match evidence {
        Evidence::Ast { detail } | Evidence::Idx { detail } => detail,
        other => panic!("this arm reports AST or IDX evidence, not {other:?}"),
    }
}

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

fn sorted_causes(findings: &[Finding]) -> Vec<&str> {
    let mut out = causes(findings);
    out.sort_unstable();
    out
}

fn symbols(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| &*f.site.symbol).collect()
}

fn rels(findings: &[Finding]) -> Vec<String> {
    let mut out: Vec<String> = findings.iter().map(|f| f.site.rel.to_string()).collect();
    out.sort_unstable();
    out.dedup();
    out
}

// --- #1 weak boundary types --------------------------------------------------

#[test]
fn fires_on_weak_public_signature() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "from typing import Any\n",
                "def load(cfg: Any) -> Any:\n    return cfg\n",
                "def run(a: int, **kwargs):\n    return kwargs\n",
            ),
        )],
    );
    let causes = sorted_causes(&findings);
    assert!(causes.contains(&"weak:api.load:cfg"), "{causes:?}");
    assert!(causes.contains(&"weak:api.load:return"), "{causes:?}");
    assert!(causes.contains(&"weak:api.run:**kwargs"), "{causes:?}");
    assert!(findings.iter().all(|f| f.tier() == Tier::Heuristic));
}

/// `dict[str, Any]` is what `json.loads` hands back and what an open vendor
/// schema honestly is: the annotation named the keys and left the values open
/// on purpose. An `Any` anywhere else places nothing.
#[test]
fn an_any_valued_mapping_placed_its_any() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "from typing import Any\n",
                "def wire(body: dict[str, Any]) -> list[dict[str, Any]]:\n    return [body]\n",
                "def loose(items: list[Any]) -> dict[Any, Any]:\n    return {}\n",
            ),
        )],
    );
    assert_eq!(
        sorted_causes(&findings),
        ["weak:api.loose:items", "weak:api.loose:return"]
    );
}

/// What crosses a callback is the callback's own contract: a `Callable`
/// annotation named a callable and its arity, and this signature cannot narrow
/// another party's function. A bare `Any` beside one still names nothing.
#[test]
fn a_callable_places_its_any() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "from typing import Any, Callable, List, Optional\n",
                "def run(cb: Optional[Callable[[str, List[Any]], Any]], ",
                "fn: Callable[..., Any], raw: Any) -> None:\n    return None\n",
                "def hand() -> Callable[..., Any]:\n    return run\n",
            ),
        )],
    );
    assert_eq!(sorted_causes(&findings), ["weak:api.run:raw"]);
}

/// A method whose class answers to an external base takes the signature that
/// base dispatches on: torch's `ctx: Any` attribute bag, Django's
/// `**extra_fields`. The same def outside such a class is this repo's.
#[test]
fn a_framework_fixed_method_did_not_choose_its_signature() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "from typing import Any\n",
                "import torch\n",
                "class Op(torch.autograd.Function):\n",
                "    @staticmethod\n",
                "    def forward(ctx: Any, x: int) -> int:\n        return x\n",
                "class Plain:\n",
                "    def forward(self, ctx: Any) -> int:\n        return 0\n",
                "def forward(ctx: Any) -> int:\n    return 0\n",
            ),
        )],
    );
    assert_eq!(
        sorted_causes(&findings),
        ["weak:api.Plain.forward:ctx", "weak:api.forward:ctx"]
    );
}

/// Every load splatted into a call the repo cannot spell leaves the accepted
/// set the callee's. A repo def that names its own parameters is one the
/// forwarder could have named too.
#[test]
fn a_star_param_only_splatted_onward_is_the_callees() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "from ext import Wrapped\n",
                "def wrap(a: int, *args, **kwargs):\n    return Wrapped(a, *args, **kwargs)\n",
                "def call(fn, a: int, *args, **kwargs):\n    return fn(*args, **kwargs)\n",
                "def open_callee(a: int, *args, **kwargs):\n    return _wide(*args, **kwargs)\n",
                "def _wide(*args, **kwargs):\n    return args\n",
                "def named(a: int, *args, **kwargs):\n    return _spelled(*args, **kwargs)\n",
                "def _spelled(length: int = 1, upper: bool = False):\n    return length\n",
            ),
        )],
    );
    assert_eq!(
        sorted_causes(&findings),
        ["weak:api.named:**kwargs", "weak:api.named:*args"]
    );
}

/// A nested def has no caller outside the frame that wrote it.
#[test]
fn a_closure_is_not_a_published_boundary() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "from typing import Any\n",
                "def walk(root: str) -> None:\n",
                "    def visit(value: Any) -> None:\n        return None\n",
                "    visit(root)\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// `mypy.ini` names the package the repo type-checks; a sample tree outside it
/// publishes no signature a checker was asked to hold.
#[test]
fn a_file_outside_the_declared_type_check_scope_is_silent() {
    let weak = "from typing import Any\ndef demo(screen: Any) -> Any:\n    return screen\n";
    let findings = run_rule(
        "1",
        &[
            ("samples/demo.py", weak),
            ("pkg/api.py", weak),
            ("mypy.ini", "[mypy]\npackages = pkg\n"),
        ],
    );
    assert_eq!(rels(&findings), ["pkg/api.py"]);
}

/// `*args` is as opaque as `**kwargs`; a wholly unannotated def declares
/// nothing, and #5 owns what its callers prove.
#[test]
fn opaque_star_params_only_beside_a_declared_one() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "def pub(a, b, *args, **kwargs):\n    return a\n",
                "def pub2(a: int, *args, **kwargs) -> int:\n    return a\n",
                "def pub3(a: int, *args, **kwargs: int) -> int:\n    return a\n",
            ),
        )],
    );
    assert_eq!(
        sorted_causes(&findings),
        [
            "weak:api.pub2:**kwargs",
            "weak:api.pub2:*args",
            "weak:api.pub3:*args"
        ]
    );
}

/// Bare list/dict/tuple boundary types join #1; a non-Optional annotation
/// contradicted by its own `= None` default is #1's lie to flag, public or not.
#[test]
fn bare_containers_and_lying_none_default() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "def group(rows: list) -> dict:\n    return {'rows': rows}\n",
                "def load(path: str = None):\n    return path\n",
                "def _internal(cfg: dict = None):\n    return cfg\n",
                "def fine(path: str | None = None) -> str:\n    return path or ''\n",
            ),
        )],
    );
    assert_eq!(
        sorted_causes(&findings),
        [
            "lying-default:api._internal:cfg",
            "lying-default:api.load:path",
            "weak:api.group:return",
            "weak:api.group:rows",
        ]
    );
}

#[test]
fn signatures_in_test_files_are_not_boundaries() {
    let weak = "def check(cfg: dict):\n    return cfg\n";
    let findings = run_rule("1", &[("prod.py", weak)]);
    assert_eq!(causes(&findings), ["weak:prod.check:cfg"]);
    let findings = run_rule("1", &[("tests/test_api.py", weak)]);
    assert!(
        findings.iter().all(|f| !f.site.rel.starts_with("tests/")),
        "{findings:?}"
    );
}

#[test]
fn silent_on_typed_private_and_unpacked() {
    let findings = run_rule(
        "1",
        &[(
            "api.py",
            concat!(
                "from typing import Any, TypedDict, Unpack\n",
                "class Opts(TypedDict):\n    depth: int\n",
                "class Report:\n    pass\n",
                "def load(cfg: Report) -> Report:\n    return cfg\n",
                "def _internal(x: Any) -> Any:\n    return x\n",
                "def run(**kwargs: Unpack[Opts]) -> None:\n    pass\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

// --- annotation aliases (`provers/annotations`) ------------------------------

const ALIASES: (&str, &str) = (
    "alias.py",
    "from typing import Optional\nMaybeStr = str | None\nOptInt = Optional[int]\nRows = list\n",
);

#[test]
fn alias_of_optional_admits_none() {
    let findings = run_rule(
        "1",
        &[
            ALIASES,
            (
                "m.py",
                "from alias import MaybeStr, OptInt\n\
                 def load(key: MaybeStr = None, n: OptInt = None):\n    return key\n",
            ),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// A quoted form is parsed and judged as written: `"str"` lies with a None
/// default or a None path, `"str | None"` admits it.
#[test]
fn string_annotations_are_read_like_aliases() {
    let findings = run_rule(
        "1",
        &[(
            "m.py",
            "def load(c: 'str | None' = None, d: 'str' = None):\n    return d\n",
        )],
    );
    assert_eq!(causes(&findings), ["lying-default:m.load:d"]);
    let findings = run_rule(
        "33",
        &[(
            "m.py",
            concat!(
                "def h(x) -> 'str':\n    if x:\n        return 'a'\n    return None\n",
                "def ok(x) -> 'str | None':\n    if x:\n        return 'a'\n    return None\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["lying-return:m.h"]);
}

#[test]
fn alias_of_a_non_optional_still_lies() {
    let findings = run_rule(
        "1",
        &[
            ("alias.py", "Key = str\n"),
            (
                "m.py",
                "from alias import Key\ndef load(key: Key = None):\n    return key\n",
            ),
        ],
    );
    assert_eq!(causes(&findings), ["lying-default:m.load:key"]);
}

#[test]
fn alias_of_optional_is_an_honest_return() {
    let findings = run_rule(
        "33",
        &[
            ALIASES,
            (
                "m.py",
                concat!(
                    "from alias import MaybeStr\n",
                    "def find(x) -> MaybeStr:\n",
                    "    if x:\n        return 'a'\n",
                    "    return None\n",
                ),
            ),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// The boundary verdict reads the spelling: `Rows` is the name #1 asks for,
/// even though it aliases a bare list.
#[test]
fn a_named_alias_is_not_a_bare_container() {
    let findings = run_rule(
        "1",
        &[
            ALIASES,
            (
                "m.py",
                "from alias import Rows\ndef report(rows: Rows) -> int:\n    return len(rows)\n",
            ),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

// --- #3 contract-implied guard -----------------------------------------------

#[test]
fn fires_on_tolerant_callee_guards() {
    let findings = run_rule(
        "3",
        &[(
            "m.py",
            concat!(
                "def f(items: list):\n",
                "    if items:\n        items.sort()\n",
                "    if len(items) > 0:\n        items.reverse()\n",
            ),
        )],
    );
    assert_eq!(
        sorted_causes(&findings),
        ["guard-implied:items.reverse", "guard-implied:items.sort"]
    );
}

/// A `.get()` local or an unannotated param may be None, so the guard has no
/// contract to discharge; a non-None annotation, or a local every binding of
/// which builds a container, is the contract.
#[test]
fn guarded_name_needs_a_non_none_contract() {
    let findings = run_rule(
        "3",
        &[(
            "m.py",
            concat!(
                "def f(d: dict[str, list[int]], k: str):\n",
                "    x = d.get(k)\n",
                "    if x:\n        for i in x:\n            print(i)\n",
                "def h_unannotated(xs):\n",
                "    if xs:\n        for i in xs:\n            print(i)\n",
                "def rebound(xs: list[int], d: dict):\n",
                "    xs = d.get('k')\n",
                "    if xs:\n        for i in xs:\n            print(i)\n",
                "def g_twin(xs: list[int]):\n",
                "    if xs:\n        for i in xs:\n            print(i)\n",
                "def m_local_literal(n: int):\n",
                "    ys = [1] * n\n",
                "    if ys:\n        for i in ys:\n            print(i)\n",
                "def m_local_declared(d: dict):\n",
                "    zs: list[int] = d.get('k', [])\n",
                "    if zs:\n        for i in zs:\n            print(i)\n",
            ),
        )],
    );
    let rows: Vec<(&str, &str)> = findings
        .iter()
        .map(|f| (&*f.site.symbol, f.cause.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("m.g_twin", "guard-implied:xs.iteration"),
            ("m.m_local_literal", "guard-implied:ys.iteration"),
            ("m.m_local_declared", "guard-implied:zs.iteration"),
        ]
    );
}

/// A local bound by a call the module resolves to a list-returning API, or by
/// a stdlib list-returning method, is never None; a repo call, a `.get()` or a
/// `.group()` holds no such contract.
#[test]
fn list_returning_api_calls_are_a_contract() {
    let findings = run_rule(
        "3",
        &[(
            "m.py",
            concat!(
                "import re\n",
                "_RE = re.compile('x')\n",
                "def own(s):\n    return [s]\n",
                "def pattern(s: str):\n",
                "    xs = _RE.findall(s)\n",
                "    if xs:\n        for x in xs:\n            print(x)\n",
                "def module_fn(s: str):\n",
                "    xs = re.findall('x', s)\n",
                "    if xs:\n        xs.sort()\n",
                "def method(s: str):\n",
                "    ws = s.split(',')\n",
                "    if ws:\n        ws.reverse()\n",
                "def builtin(d: dict):\n",
                "    ks = sorted(d)\n",
                "    if ks:\n        for k in ks:\n            print(k)\n",
                "def view(d: dict):\n",
                "    items = d.items()\n",
                "    if items:\n        for k, v in items:\n            print(k, v)\n",
                "def repo_call(s: str):\n",
                "    xs = own(s)\n",
                "    if xs:\n        for x in xs:\n            print(x)\n",
                "def group(s: str):\n",
                "    xs = _RE.match(s).group(1)\n",
                "    if xs:\n        for x in xs:\n            print(x)\n",
                "def rebound_to_get(s: str, d: dict):\n",
                "    xs = s.split(',')\n",
                "    xs = d.get(s)\n",
                "    if xs:\n        for x in xs:\n            print(x)\n",
            ),
        )],
    );
    let rows: Vec<(&str, &str)> = findings
        .iter()
        .map(|f| (&*f.site.symbol, f.cause.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("m.pattern", "guard-implied:xs.iteration"),
            ("m.module_fn", "guard-implied:xs.sort"),
            ("m.method", "guard-implied:ws.reverse"),
            ("m.builtin", "guard-implied:ks.iteration"),
            ("m.view", "guard-implied:items.iteration"),
        ]
    );
}

/// Iteration tolerates empty, unless the loop has an else (it runs on empty)
/// or the guard covers more than the loop.
#[test]
fn loop_guard_implied() {
    let findings = run_rule(
        "3",
        &[(
            "m.py",
            concat!(
                "def emit(items: list, sink):\n",
                "    if items:\n        for i in items:\n            sink.append(i)\n",
                "def has_else(items: list, sink):\n",
                "    if items:\n        for i in items:\n            sink.append(i)\n",
                "        else:\n            sink.clear()\n",
                "def wider(items: list, sink):\n",
                "    if items:\n        for i in items:\n            sink.append(i)\n",
                "        sink.reverse()\n",
                "def maybe_none(items: list | None, sink):\n",
                "    if items:\n        for i in items:\n            sink.append(i)\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["guard-implied:items.iteration"]);
}

#[test]
fn silent_on_loadbearing_guard() {
    let findings = run_rule(
        "3",
        &[(
            "m.py",
            concat!(
                "def f(items, log):\n",
                "    if items:\n        items.pop()\n",
                "    if items:\n        log.process(items)\n",
                "    if items:\n        items.sort()\n",
                "    else:\n        log.warn()\n",
                "    items.sort()\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

// --- #6 dishonest accessor ---------------------------------------------------

#[test]
fn fires_on_accessor_with_effects() {
    let findings = run_rule(
        "6",
        &[
            ("state.py", "cache = {}\n"),
            (
                "m.py",
                "from state import cache\n\
                 def get_price(k):\n    cache[k] = 1\n    return cache[k]\n",
            ),
        ],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.get_price"]);
    assert!(findings[0].message.contains("gw:state.cache"));
}

#[test]
fn silent_on_pure_accessor() {
    let findings = run_rule(
        "6",
        &[(
            "m.py",
            "def get_price(x):\n    return x * 2\ndef store(sink, v):\n    sink.append(v)\n",
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// A framework base owns `get_queryset`; the same effects under a plain
/// class's accessor name still fire.
#[test]
fn override_fixed_accessor_is_the_frameworks_name() {
    let findings = run_rule(
        "6",
        &[(
            "m.py",
            concat!(
                "import fakefw\n",
                "COUNTER = []\n",
                "class MyView(fakefw.ListView):\n",
                "    def get_queryset(self):\n        COUNTER.append(1)\n        return []\n",
                "class Plain:\n",
                "    def get_items(self):\n        COUNTER.append(1)\n        return []\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.Plain.get_items"]);
}

/// The prover cannot tell a read from a write, and a read that crosses a wire
/// is a cost (#29's); the paired positive writes a global too.
#[test]
fn io_alone_is_not_a_lie() {
    let findings = run_rule(
        "6",
        &[
            ("state.py", "cache = {}\n"),
            (
                "m.py",
                concat!(
                    "from state import cache\n",
                    "def get_rows(path):\n",
                    "    with open(path) as f:\n        return f.readlines()\n",
                    "def get_kept(path):\n",
                    "    with open(path) as f:\n        cache[path] = f.read()\n",
                    "    return cache[path]\n",
                ),
            ),
        ],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.get_kept"]);
}

/// `fetch_x` promises to go and get it; `get_` and `list_` promise a plain read.
#[test]
fn fetch_names_the_trip() {
    let findings = run_rule(
        "6",
        &[
            ("state.py", "cache = {}\n"),
            (
                "m.py",
                concat!(
                    "from state import cache\n",
                    "def fetch_price(k):\n    cache[k] = 1\n    return cache[k]\n",
                    "def get_price(k):\n    cache[k] = 2\n    return cache[k]\n",
                ),
            ),
        ],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.get_price"]);
}

#[test]
fn a_doubles_recording_is_the_fixture() {
    let findings = run_rule(
        "6",
        &[
            ("state.py", "cache = {}\n"),
            (
                "test_x.py",
                "from state import cache\n\
                 def get_calls(k):\n    cache[k] = 1\n    return cache[k]\n",
            ),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// Writing your own object's slots is what having a method means, and a fresh
/// object the caller owns is no one else's; a named global write fires.
#[test]
fn own_slots_are_bookkeeping_a_constructor_call_is_not_an_effect() {
    let findings = run_rule(
        "6",
        &[(
            "m.py",
            concat!(
                "CACHE = {}\n",
                "class P:\n",
                "    def __init__(self, v):\n        self.v = v\n",
                "    def get_x(self):\n        self._c = 1\n        return self._c\n",
                "def get_p():\n    return P(1)\n",
                "def get_kept(k):\n    CACHE[k] = 1\n    return CACHE[k]\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.get_kept"]);
}

/// `rows.append(1)` mutates what the caller passed over: real enough, but the
/// finding can only call it "mutates-arg", which names no state a reader can go
/// and check (decisions.tsv, g4/cut).
#[test]
fn an_effect_the_finding_cannot_name_is_not_evidence() {
    let findings = run_rule(
        "6",
        &[
            ("state.py", "cache = {}\n"),
            (
                "m.py",
                concat!(
                    "from state import cache\n",
                    "def get_first(rows):\n    rows.append(1)\n    return rows[0]\n",
                    "def get_kept(k):\n    cache[k] = 1\n    return cache[k]\n",
                ),
            ),
        ],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.get_kept"]);
}

/// `STORE.touch()` writes the store's own slots; reaching into it
/// (`STORE.rows.append`) is this function's write.
#[test]
fn a_receiver_handed_over_whole_is_not_this_functions_write() {
    let findings = run_rule(
        "6",
        &[(
            "m.py",
            concat!(
                "class Store:\n",
                "    def __init__(self):\n        self.rows = []\n",
                "    def touch(self):\n        self.n = 1\n",
                "STORE = Store()\n",
                "def get_head():\n    STORE.touch()\n    return 1\n",
                "def get_tail():\n    STORE.rows.append(1)\n    return 2\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.get_tail"]);
}

/// A global written only where the function tested it can only fill what the
/// test found missing; the unconditional write beside it fires.
#[test]
fn a_memo_fill_is_invisible_to_callers() {
    let findings = run_rule(
        "6",
        &[(
            "m.py",
            concat!(
                "CACHE = {}\n",
                "HITS = {}\n",
                "ENGINE = None\n",
                "FACTORY = None\n",
                "def get_conf(k):\n",
                "    if k not in CACHE:\n        CACHE[k] = 1\n",
                "    return CACHE[k]\n",
                "def get_engine():\n",
                "    global ENGINE, FACTORY\n",
                "    if ENGINE is None:\n",
                "        ENGINE = 1\n",
                "        FACTORY = 2\n",
                "    return ENGINE\n",
                "def get_hit(k):\n",
                "    HITS[k] = 1\n",
                "    return HITS[k]\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.get_hit"]);
}

#[test]
fn a_name_that_names_the_work_hides_nothing() {
    let findings = run_rule(
        "6",
        &[(
            "m.py",
            concat!(
                "CACHE = {}\n",
                "def get_or_compute(k):\n    CACHE[k] = 1\n    return CACHE[k]\n",
                "def get_value(k):\n    CACHE[k] = 2\n    return CACHE[k]\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["dishonest-accessor:m.get_value"]);
}

// --- #7 comment-borne protocol -----------------------------------------------

#[test]
fn fires_on_narrated_protocol() {
    let findings = run_rule(
        "7",
        &[(
            "m.py",
            concat!(
                "def read(conn):\n",
                "    \"\"\"The caller must ensure connect() was called first.\"\"\"\n",
                "    return conn.recv()\n",
                "def setup():\n",
                "    \"\"\"Prepare state. Must be called before read().\"\"\"\n",
                "    pass\n",
            ),
        )],
    );
    assert_eq!(symbols(&findings), ["m.read", "m.setup"]);
}

/// Narrated protocols also use should/expects/until spellings.
#[test]
fn widened_protocol_patterns() {
    let findings = run_rule(
        "7",
        &[(
            "m.py",
            concat!(
                "def a(conn):\n",
                "    '''connect() should be called before any send'''\n",
                "    return conn\n",
                "def b(idx):\n",
                "    '''Expects the index to be loaded already.'''\n",
                "    return idx\n",
                "def c(x):\n",
                "    '''Do not call until start() has run'''\n",
                "    return x\n",
            ),
        )],
    );
    assert_eq!(findings.len(), 3, "{findings:?}");
}

/// The bare imperative is the protocol as written, its callee spelled with
/// parens; the same words mid-sentence narrate what the code does, and a bare
/// name is a noun.
#[test]
fn imperative_call_before_after_at_line_start() {
    let findings = run_rule(
        "7",
        &[(
            "m.py",
            concat!(
                "def batch():\n",
                "    '''we call refresh() after each batch'''\n",
                "    return 2\n",
                "def stop():\n",
                "    '''Stop the worker.\n\n    Call join() after stop().\n    '''\n",
                "    return 3\n",
                "def recast():\n",
                "    '''Call of the Forge God can be recast after the cooldown'''\n",
                "    return 4\n",
            ),
        )],
    );
    assert_eq!(symbols(&findings), ["m.stop"]);
}

#[test]
fn silent_on_ordinary_docstrings() {
    let findings = run_rule(
        "7",
        &[(
            "m.py",
            concat!(
                "def read(conn):\n",
                "    \"\"\"Fast path: the buffer is already decoded.\"\"\"\n",
                "    return conn.recv()\n",
                "def setup():\n",
                "    \"\"\"Prepare the shared state for reads.\"\"\"\n",
                "    pass\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// A module and a class have no signature to lift a precondition into, an
/// `__init__`'s obligations are its parameter list, and a body that opens with
/// an `assert` checks what the prose repeats.
#[test]
fn only_a_def_that_could_have_carried_it_fires() {
    let narrated = "\"\"\"The caller must call connect() before reading.\"\"\"\n";
    let source = format!(
        "{narrated}\
         class Reader:\n    {narrated}\
         \x20   def __init__(self, conn):\n        {narrated}\
         \x20       self.conn = conn\n\
         \x20   def read(self, conn):\n        {narrated}\
         \x20       assert conn is not None\n        return conn\n\
         def take(conn):\n    {narrated}\
         \x20   return conn\n"
    );
    let findings = run_rule("7", &[("m.py", &source)]);
    assert_eq!(symbols(&findings), ["m.take"]);
}

/// A test narrating a protocol exercises it rather than publishing it.
#[test]
fn tests_are_skipped() {
    let obliged = "def close():\n    \"\"\"Always call flush() before close().\"\"\"\n    pass\n";
    let findings = run_rule("7", &[("tests/test_m.py", obliged), ("m.py", "x = 1\n")]);
    assert!(findings.is_empty(), "{findings:?}");
    let findings = run_rule("7", &[("m.py", obliged)]);
    assert_eq!(causes(&findings), ["protocol-doc:1"]);
}

#[test]
fn first_in_prose_is_not_a_protocol() {
    let findings = run_rule(
        "7",
        &[(
            "m.py",
            "def f():\n    \"\"\"A class call runs the first __init__ up its base chain.\"\"\"\n",
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
    let findings = run_rule(
        "7",
        &[(
            "m.py",
            "def f():\n    \"\"\"connect() must be called first.\"\"\"\n",
        )],
    );
    assert_eq!(findings.iter().map(|f| f.rule).collect::<Vec<_>>(), ["7"]);
}

// --- #33 return honesty ------------------------------------------------------

/// Explicit `return None` is the intentional Optional-lookup idiom, not a bare
/// return. A truly bare return mixed with values still fires.
#[test]
fn explicit_return_none_is_a_value() {
    let findings = run_rule(
        "33",
        &[(
            "m.py",
            concat!(
                "def lookup(d, k):\n",
                "    if k in d:\n        return d[k]\n    return None\n",
                "def sloppy(d, k):\n",
                "    if k in d:\n        return d[k]\n    return\n",
            ),
        )],
    );
    let causes = causes(&findings);
    assert!(causes.contains(&"mixed-returns:m.sloppy"), "{causes:?}");
    assert!(!causes.contains(&"mixed-returns:m.lookup"), "{causes:?}");
}

/// Find-or-None, a True-or-nothing predicate and an Optional handler all spell
/// their None by falling off the end; only a written bare return puts two
/// contracts in one body.
#[test]
fn falling_off_the_end_is_a_protocol_not_a_mixed_return() {
    let findings = run_rule(
        "33",
        &[(
            "m.py",
            concat!(
                "def find(xs, k):\n",
                "    for x in xs:\n        if x.k == k:\n            return x\n",
                "def _validate_type_odd(self, value):\n",
                "    if value % 2:\n        return True\n",
                "def handle(self, event):\n",
                "    if event.key:\n        return event\n",
                "def sloppy(self, event):\n",
                "    if event.key:\n        return event\n    return\n",
            ),
        )],
    );
    assert_eq!(sorted_causes(&findings), ["mixed-returns:m.sloppy"]);
}

#[test]
fn fires_on_lying_and_mixed_returns() {
    let findings = run_rule(
        "33",
        &[(
            "m.py",
            concat!(
                "def lies(x) -> int:\n",
                "    if x:\n        return 1\n    return None\n",
                "def falls(x) -> bool:\n",
                "    if x:\n        return True\n",
                "def none_lies(x) -> None:\n    return x + 1\n",
                "def mixed(x):\n",
                "    if x:\n        return 1\n    return\n",
            ),
        )],
    );
    assert_eq!(
        sorted_causes(&findings),
        [
            "lying-return:m.falls",
            "lying-return:m.lies",
            "lying-return:m.none_lies",
            "mixed-returns:m.mixed",
        ]
    );
}

#[test]
fn silent_on_honest_shapes() {
    let findings = run_rule(
        "33",
        &[(
            "m.py",
            concat!(
                "import abc\n",
                "from collections.abc import Iterator\n",
                "def optional(x) -> int | None:\n",
                "    if x:\n        return 1\n    return None\n",
                "def total(x) -> int:\n",
                "    if x:\n        return 1\n    return 2\n",
                "def raises(x) -> int:\n",
                "    if x:\n        return 1\n    raise ValueError(x)\n",
                "def loops() -> int:\n",
                "    while True:\n        return 1\n",
                "def gen(xs) -> Iterator[int]:\n",
                "    for x in xs:\n        yield x\n    return\n",
                "def stub(x) -> int:\n    ...\n",
                "class Base(abc.ABC):\n",
                "    @abc.abstractmethod\n",
                "    def get(self) -> int:\n        pass\n",
                "def procedure(x):\n    x.append(1)\n",
                "from typing import NoReturn\n",
                "def _fatal(msg) -> NoReturn:\n    raise SystemExit(msg)\n",
                "def bails(x) -> int:\n",
                "    if x:\n        return 1\n    _fatal('no')\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// Cross-module NoReturn, method and builtin tails are unknown-termination; a
/// visible same-module callee without NoReturn stays a provable fall-through.
#[test]
fn tail_verdict_needs_a_visible_callee() {
    let findings = run_rule(
        "33",
        &[
            (
                "errs.py",
                "from typing import NoReturn\n\
                 def fatal(msg) -> NoReturn:\n    raise SystemExit(msg)\n",
            ),
            (
                "m.py",
                concat!(
                    "from errs import fatal\n",
                    "def helper(x):\n    print(x)\n",
                    "def bails(x) -> int:\n",
                    "    if x:\n        return 1\n    fatal('no')\n",
                    "def shows(x) -> int:\n",
                    "    if x:\n        return 1\n    print(x)\n",
                    "def falls(x) -> int:\n",
                    "    if x:\n        return 1\n    helper(x)\n",
                    "class App:\n",
                    "    def stop(self) -> None:\n        raise SystemExit(0)\n",
                    "    def run(self, x) -> int:\n",
                    "        if x:\n            return 1\n        self.stop()\n",
                ),
            ),
        ],
    );
    assert_eq!(causes(&findings), ["lying-return:m.falls"]);
    assert!(findings[0].message.contains("fall off the end"));
}

/// Explicit `return None` beside value returns is the Optional-lookup idiom in
/// untyped code, but in a signature the repo typed it is an undeclared
/// Optional. Typed params plus no return annotation is the gate.
#[test]
fn undeclared_optional_in_a_typed_signature() {
    let findings = run_rule(
        "33",
        &[(
            "m.py",
            concat!(
                "def typed_lookup(d: dict, k: str):\n",
                "    if k in d:\n        return d[k]\n    return None\n",
                "def untyped_lookup(d, k):\n",
                "    if k in d:\n        return d[k]\n    return None\n",
                "def declared(d: dict, k: str) -> int | None:\n",
                "    if k in d:\n        return d[k]\n    return None\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["undeclared-optional:m.typed_lookup"]);
}

// --- #9 shared mutable state -------------------------------------------------

/// Nothing structural separates a settings object whose state two handlers
/// replace from a log service three modules call. The cross-module arm is
/// retired (docs/review/decisions.tsv, g3/cut).
#[test]
fn a_foreign_module_using_a_singleton_is_not_a_global_write() {
    let findings = run_rule(
        "9",
        &[
            ("state.py", "cache = {}\nregistry = []\n"),
            (
                "writer_a.py",
                "from state import cache\ndef wa(k, v):\n    cache[k] = v\n",
            ),
            (
                "writer_b.py",
                "import state\ndef wb(entries):\n    state.cache.update(entries)\n\
                 def wc(x):\n    state.registry.append(x)\n",
            ),
            (
                "boot.py",
                "import state\ndef boot():\n    state.cache = {'b': 2}\n",
            ),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// Runtime monkeypatch of a class from another module; test-path patching is
/// idiomatic and exempt.
#[test]
fn monkeypatch_fires_prod_only() {
    let findings = run_rule(
        "9",
        &[
            (
                "svc.py",
                "class Client:\n    def send(self, x):\n        return x\n",
            ),
            (
                "patcher.py",
                "import svc\ndef harden():\n    svc.Client.send = lambda self, x: None\n",
            ),
            (
                "tests/test_svc.py",
                "import svc\ndef test_p():\n    svc.Client.send = lambda self, x: 1\n",
            ),
        ],
    );
    assert_eq!(causes(&findings), ["monkeypatch:svc.Client.send"]);
    assert_eq!(&*findings[0].site.rel, "patcher.py");
}

/// `finally: svc.send = original` undoes the patch two lines above: one seam,
/// reported where it is opened.
#[test]
fn a_restore_is_the_same_seam() {
    let findings = run_rule(
        "9",
        &[
            ("svc.py", "def send(x):\n    return x\n"),
            (
                "patcher.py",
                concat!(
                    "import svc\n",
                    "def harden():\n",
                    "    original = svc.send\n",
                    "    try:\n",
                    "        svc.send = lambda x: None\n",
                    "    finally:\n",
                    "        svc.send = original\n",
                ),
            ),
        ],
    );
    let rows: Vec<(&str, u32)> = findings
        .iter()
        .map(|f| (f.cause.as_str(), f.site.line))
        .collect();
    assert_eq!(rows, [("monkeypatch:svc.send", 5)]);
}

#[test]
fn silent_on_reads_and_single_module_mutation() {
    let findings = run_rule(
        "9",
        &[
            ("state.py", "cache = {}\ndef put(k, v):\n    cache[k] = v\n"),
            (
                "reader.py",
                "from state import cache\ndef get(k):\n    return cache.get(k)\n",
            ),
            ("m.py", "def f(x, acc=None):\n    return acc or [x]\n"),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// `global f; f = ...` beside `from other import f` rebinds m's own name:
/// three own writers are local state, and no monkeypatch of `other.f`.
#[test]
fn rebinding_an_import_alias_is_the_modules_own_store() {
    let findings = run_rule(
        "9",
        &[
            ("other.py", "def f():\n    return 1\n"),
            (
                "m.py",
                concat!(
                    "from other import f\n",
                    "if f is None:\n    f = None\n",
                    "def a():\n    global f\n    f = 1\n",
                    "def b():\n    global f\n    f = 2\n",
                    "def c():\n    global f\n    f = 3\n",
                ),
            ),
            ("p.py", "import other\ndef h():\n    other.f = None\n"),
        ],
    );
    assert_eq!(
        causes(&findings),
        ["local-writers:m.f", "monkeypatch:other.f"]
    );
}

/// No importer needed: the assumption is the writer count, not the reach.
#[test]
fn three_own_functions_rebinding_a_global_fire() {
    let findings = run_rule(
        "9",
        &[(
            "state.py",
            concat!(
                "ACTIVE = False\n",
                "def start():\n    global ACTIVE\n    ACTIVE = True\n",
                "def stop():\n    global ACTIVE\n    ACTIVE = False\n",
                "def reset():\n    global ACTIVE\n    ACTIVE = False\n",
                "def get():\n    return ACTIVE\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["local-writers:state.ACTIVE"]);
    assert!(findings[0].message.contains("start"));
    assert_eq!(findings[0].salience, 3.0);
    assert_eq!(detail(&findings[0].evidence), "");
}

/// In-place container mutation inside the module that owns the container is
/// the memo pattern, not action at a distance.
#[test]
fn a_memo_its_own_module_fills_is_the_cache_pattern() {
    let findings = run_rule(
        "9",
        &[(
            "state.py",
            concat!(
                "CACHE = {}\n",
                "def put(k, v):\n    CACHE[k] = v\n",
                "def drop(k):\n    CACHE.pop(k)\n",
                "def reset():\n    CACHE.clear()\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn two_writers_reads_and_test_modules_stay_silent() {
    let two = concat!(
        "MODE = 0\n",
        "def put(v):\n    global MODE\n    MODE = v\n",
        "def drop():\n    global MODE\n    MODE = 0\n",
        "def get():\n    return MODE\n",
    );
    let three = concat!(
        "SEEN = 0\n",
        "def a():\n    global SEEN\n    SEEN = 1\n",
        "def b():\n    global SEEN\n    SEEN = 2\n",
        "def c():\n    global SEEN\n    SEEN = 3\n",
    );
    let findings = run_rule("9", &[("state.py", two), ("tests/test_state.py", three)]);
    assert!(findings.is_empty(), "{findings:?}");
}

/// Module-level process mutators in an imported module fire; entry points,
/// main guards, local objects and test modules stay silent.
const IMPORT_TIME_LIB: &str = concat!(
    "import sys\nimport os\nimport logging\n",
    "from warnings import filterwarnings\nimport numpy as np\n",
    "from dotenv import load_dotenv\n",
    "sys.path.insert(0, 'x')\n",
    "os.environ['A'] = '1'\n",
    "logging.basicConfig(level=1)\n",
    "filterwarnings('ignore')\n",
    "np.random.seed(0)\n",
    "try:\n    load_dotenv()\nexcept Exception:\n    pass\n",
    "rng = np.random.default_rng()\n",
    "rng.seed(1)\n",
    "def seed():\n    pass\n",
    "seed()\n",
);

#[test]
fn imported_library_fires_per_mutator() {
    let findings = run_rule(
        "9",
        &[("lib.py", IMPORT_TIME_LIB), ("app.py", "import lib\n")],
    );
    let expected: Vec<String> = [7, 8, 9, 10, 11, 13]
        .iter()
        .map(|n| format!("import-time-effect:lib:{n}"))
        .collect();
    assert_eq!(causes(&findings), expected);
    assert!(findings[0].message.contains("1 importer"));
}

#[test]
fn entry_point_main_guard_and_tests_stay_silent() {
    let findings = run_rule(
        "9",
        &[
            ("script.py", IMPORT_TIME_LIB),
            (
                "main.py",
                "import sys\nif __name__ == '__main__':\n    sys.path.insert(0, 'x')\n",
            ),
            ("boot.py", "import main\n"),
            (
                "tool.py",
                concat!(
                    "import sys\nsys.path.insert(0, 'x')\n",
                    "def run():\n    pass\n",
                    "if __name__ == '__main__':\n    run()\n",
                ),
            ),
            ("other_tool.py", "from tool import run\n"),
            (
                "tests/helpers.py",
                "import logging\nlogging.basicConfig()\n",
            ),
            ("tests/test_x.py", "from tests import helpers\n"),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

// --- #49 mutable default -----------------------------------------------------

/// #9's test exemption is for actors mutating prod state; a mutable default is
/// the def's own defect, tests included.
#[test]
fn mutable_default_fires_wherever_the_def_lives() {
    let findings = run_rule(
        "49",
        &[
            (
                "m.py",
                "def f(x, acc=[]):\n    acc.append(x)\n    return acc\n",
            ),
            ("tests/test_m.py", "def helper(seen={}):\n    return seen\n"),
        ],
    );
    assert_eq!(
        causes(&findings),
        [
            "mutable-default:m.f:acc",
            "mutable-default:test_m.helper:seen"
        ]
    );
}

// --- #40 naming proxies ------------------------------------------------------

#[test]
fn declared_nonbool_predicate_fires() {
    let findings = run_rule("40", &[("m.py", "def is_ready(x) -> str:\n    return x\n")]);
    assert_eq!(causes(&findings), ["naming-proxy:m.is_ready"]);
    assert_eq!(detail(&findings[0].evidence), "declared");
}

/// The plural arm read the last name token, so a unit suffix, a counted noun,
/// a verb-phrase object and a serialized collection all read as plural. It is
/// retired (docs/review/decisions.tsv, g3/cut).
#[test]
fn a_plural_name_is_not_a_lie_about_its_return() {
    let findings = run_rule(
        "40",
        &[(
            "m.py",
            concat!(
                "def image_retention_days() -> int:\n    return 1\n",
                "def cleanup_old_images() -> int:\n    return 1\n",
                "def get_cookies() -> str:\n    return 'a=1'\n",
                "def user_names(rows) -> int:\n    return len(rows)\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn honest_names_silent() {
    let findings = run_rule(
        "40",
        &[(
            "m.py",
            concat!(
                "from typing import TypeGuard\n",
                "def is_ready(x) -> bool:\n    return bool(x)\n",
                "def is_str(x) -> TypeGuard[str]:\n    return isinstance(x, str)\n",
                "def get_rows(db) -> list:\n    return [db]\n",
                "def get_status(db) -> str:\n    return str(db)\n",
                "def process(db) -> int:\n    return 1\n",
                "def run_pairs(db) -> int:\n    return 1\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

// --- #50 unannotated boundary ------------------------------------------------

#[test]
fn fires_per_public_def_listing_the_slots() {
    let findings = run_rule(
        "50",
        &[(
            "api.py",
            concat!(
                "def load(path, n: int = 0):\n    return path\n",
                "class Svc:\n",
                "    def run(self, x, *, flag):\n        return x\n",
                "    def stop(self) -> None:\n        return\n",
                "def make(x: int):\n    return [x]\n",
            ),
        )],
    );
    let mut rows: Vec<(&str, &str)> = findings
        .iter()
        .map(|f| (f.cause.as_str(), detail(&f.evidence)))
        .collect();
    rows.sort_unstable();
    assert_eq!(
        rows,
        [
            ("unannotated:api.Svc.run", "x, flag, return"),
            ("unannotated:api.load", "path, return"),
            ("unannotated:api.make", "return"),
        ]
    );
    assert!(findings.iter().all(|f| f.tier() == Tier::Heuristic));
}

/// Private names, private modules, `__all__`, star params, a bare return, a
/// def nested at any depth (a method of a class local to a function too) and a
/// test path are not published contracts.
#[test]
fn silent_off_the_boundary() {
    let findings = run_rule(
        "50",
        &[
            ("_internal.py", "def load(path):\n    return path\n"),
            (
                "api.py",
                concat!(
                    "__all__ = ['pub', 'pub2', 'pub3']\n",
                    "def _helper(x):\n    return x\n",
                    "def hidden(x):\n    return x\n",
                    "def pub(*args, **kwargs) -> None:\n    pass\n",
                    "def pub2(x: int):\n    if x:\n        return\n",
                    "def pub3(x: int) -> int:\n",
                    "    def key(v):\n        return v\n",
                    "    return key(x)\n",
                    "def outer(a: int) -> type:\n",
                    "    class Local:\n        def m(self, z):\n            return z\n",
                    "    return Local\n",
                    "class _Priv:\n",
                    "    def run(self, x):\n        return x\n",
                ),
            ),
            ("tests/test_api.py", "def check(cfg):\n    return cfg\n"),
        ],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// A mypy config's `packages = pkg` is the repo saying which tree it
/// type-checks: a published samples demo outside it holds no signature a
/// checker was ever asked to hold. A repo declaring no scope keeps every file.
#[test]
fn a_teaching_tree_outside_the_declared_scope_is_silent() {
    let demo = "def demo(screen):\n    return screen\n";
    let findings = run_rule("50", &[("samples/bars.py", demo), ("pkg/api.py", demo)]);
    assert_eq!(rels(&findings), ["pkg/api.py", "samples/bars.py"]);

    let findings = run_rule(
        "50",
        &[
            ("samples/bars.py", demo),
            ("pkg/api.py", demo),
            (
                "mypy.ini",
                "[mypy]\npackages = pkg\ncheck_untyped_defs = True\n",
            ),
        ],
    );
    assert_eq!(rels(&findings), ["pkg/api.py"]);
}

#[test]
fn pyright_include_declares_the_scope_too() {
    let demo = "def demo(screen):\n    return screen\n";
    let findings = run_rule(
        "50",
        &[
            ("codegen.py", demo),
            ("slack/api.py", demo),
            (
                "pyproject.toml",
                "[tool.pyright]\ninclude = [\"slack\", \"main.py\"]\n",
            ),
        ],
    );
    assert_eq!(rels(&findings), ["slack/api.py"]);
}
