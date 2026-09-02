//! Family P (#41): hot-scoping honesty and the catalog's matchers. What
//! stands here is the rule's own reading and the header note. Nothing here
//! checks that a #41 finding stays out of the gate and the baseline; only its
//! REPORT posture is pinned.

use sightline_core::config::Config;
use sightline_core::findings::Finding;
use sightline_core::lang::Stack;
use sightline_core::render::{AuditResult, to_text};
use sightline_core::rule::Posture;
use sightline_py_rules::perf::RULE_41;
use sightline_testkit::{build_with, run_rule_on};

fn hot(roots: &[&str]) -> Config {
    Config {
        hot_roots: roots.iter().map(|r| (*r).to_string()).collect(),
        ..Config::new()
    }
}

fn run(files: &[(&str, &str)], config: Config) -> Vec<Finding> {
    let (_dir, stack) = build_with(files, config);
    run_rule_on("41", &stack)
}

fn causes(found: &[Finding]) -> Vec<&str> {
    found.iter().map(|f| f.cause.as_str()).collect()
}

fn sorted_causes(found: &[Finding]) -> Vec<&str> {
    let mut out = causes(found);
    out.sort();
    out
}

const SHAPE: &str = concat!(
    "import copy\n",
    "def hot(rows, template):\n",
    "    out = []\n",
    "    for row in rows:\n",
    "        cfg = copy.deepcopy(template)\n",
    "        out.append((row, cfg))\n",
    "    return out\n",
    "def helper(rows, template):\n",
    "    for row in rows:\n",
    "        cfg = copy.deepcopy(template)\n",
    "        row.use(cfg)\n",
    "def cold(rows, template):\n",
    "    for row in rows:\n",
    "        cfg = copy.deepcopy(template)\n",
    "        row.use(cfg)\n",
    "def entry(rows, template):\n",
    "    return hot(rows, template) or helper(rows, template)\n",
);

/// One fixture: the same shape is silent when cold, fires when hot-reachable,
/// and no config at all silences family P with a provenance note naming it.
#[test]
fn hot_scoping_is_honest() {
    let (_dir, stack) = build_with(&[("m.py", SHAPE)], Config::new());
    let none = run_rule_on("41", &stack);
    assert!(none.is_empty());
    assert!(
        stack
            .provers
            .notes()
            .iter()
            .any(|n| n.contains("family P silent")),
        "{:?}",
        stack.provers.notes()
    );
    // the header itself names it
    let mut result = AuditResult::new(Vec::new(), stack.neutral());
    result.notes = stack.provers.notes();
    assert!(to_text(&result).contains("note: family P silent"));
    // rooted at `entry`: the shape fires in hot() and helper() but not cold()
    let found = run(&[("m.py", SHAPE)], hot(&["m.entry"]));
    let mut symbols: Vec<&str> = found.iter().map(|f| &*f.site.symbol).collect();
    symbols.sort();
    symbols.dedup();
    assert_eq!(symbols, ["m.helper", "m.hot"]);
    assert!(
        found
            .iter()
            .all(|f| f.cause.starts_with("perf:deepcopy-in-loop"))
    );
}

/// Static perf findings never hold gate posture. This pins the posture
/// alone, not the gate and baseline behavior that follows from it.
#[test]
fn family_p_never_blocks() {
    assert_eq!(RULE_41.record.posture, Posture::Report);
    let found = run(&[("m.py", SHAPE)], hot(&["m.entry"]));
    assert!(!found.is_empty(), "the #41 shape must fire in audit");
}

#[test]
fn matchers_fire_on_their_shapes() {
    let found = run(
        &[(
            "m.py",
            concat!(
                "class Dedup:\n",
                "    def __init__(self):\n",
                "        self.seen = []\n",
                "    def add_all(self, items):\n",
                "        \"\"\"Hot path: called per request.\"\"\"\n",
                "        for x in items:\n",
                "            if x in self.seen:\n",
                "                continue\n",
                "            self.seen.append(x)\n",
                "def dups(xs):\n",
                "    \"\"\"Hot path: called per request.\"\"\"\n",
                "    n = 0\n",
                "    for a in xs:\n",
                "        for b in xs:\n",
                "            if a == b:\n",
                "                n += 1\n",
                "    return n\n",
                "def probe(groups):\n",
                "    \"\"\"Hot path: called per request.\"\"\"\n",
                "    for rows in groups:\n",
                "        if any([r.ok for r in rows]):\n",
                "            return rows\n",
            ),
        )],
        Config::new(),
    );
    let mut entries: Vec<&str> = found
        .iter()
        .map(|f| f.cause.split(':').nth(1).expect("perf:<entry>:..."))
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        [
            "list-attr-membership",
            "materialized-short-circuit",
            "nested-same-collection",
        ]
    );
}

#[test]
fn cold_functions_with_shapes_stay_silent() {
    let found = run(
        &[(
            "m.py",
            concat!(
                "def dups(xs):\n",
                "    n = 0\n",
                "    for a in xs:\n",
                "        for b in xs:\n",
                "            if a == b:\n",
                "                n += 1\n",
                "    return n\n",
            ),
        )],
        Config::new(),
    );
    assert!(found.is_empty());
}

/// The bench proved `if a == b` inside the nested loop; an all-pairs product
/// is O(n^2) by its own contract and no dict or set groups it. The join may
/// compare one attribute of each target.
#[test]
fn nested_same_collection_needs_the_equality_join() {
    let found = run(
        &[(
            "m.py",
            concat!(
                "def all_pairs_distance(pts):\n",
                "    out = []\n",
                "    for a in pts:\n",
                "        for b in pts:\n",
                "            out.append(abs(a - b))\n",
                "    return out\n",
                "def equality_join(xs):\n",
                "    dup = 0\n",
                "    for a in xs:\n",
                "        for b in xs:\n",
                "            if a == b:\n",
                "                dup += 1\n",
                "    return dup\n",
                "def key_join(rows):\n",
                "    for a in rows:\n",
                "        for b in rows:\n",
                "            if a.key != b.key:\n",
                "                continue\n",
                "            a.link(b)\n",
                "def member_join(groups):\n",
                "    for a in groups:\n",
                "        for b in groups:\n",
                "            if a.head in b.members:\n",
                "                a.merge(b)\n",
            ),
        )],
        hot(&["m.*"]),
    );
    assert_eq!(
        sorted_causes(&found),
        [
            "perf:nested-same-collection:m.equality_join:10",
            "perf:nested-same-collection:m.key_join:16",
            "perf:nested-same-collection:m.member_join:22",
        ]
    );
}

/// `from subprocess import run` / `import re as regex` are the library call; a
/// local `def run` is not, and an unbound spelling (a function-local import)
/// stands for itself.
#[test]
fn library_calls_resolve_through_the_import_bindings() {
    let found = run(
        &[
            (
                "m.py",
                concat!(
                    "import re\n",
                    "import subprocess\n",
                    "from subprocess import run\n",
                    "from re import search\n",
                    "import re as regex\n",
                    "def spawn_bound(args):\n",
                    "    for a in args:\n",
                    "        run(['git', a])\n",
                    "def spawn_qualified(args):\n",
                    "    for a in args:\n",
                    "        subprocess.run(['git', a])\n",
                    "def regex_bound(lines):\n",
                    "    for s in lines:\n",
                    "        search('[a-z]+', s)\n",
                    "def regex_alias(lines):\n",
                    "    for s in lines:\n",
                    "        regex.search('[a-z]+', s)\n",
                    "def regex_qualified(lines):\n",
                    "    for s in lines:\n",
                    "        re.search('[a-z]+', s)\n",
                    "def regex_local_import(lines):\n",
                    "    import re\n",
                    "    for s in lines:\n",
                    "        re.search('[a-z]+', s)\n",
                ),
            ),
            (
                "n.py",
                concat!(
                    "def run(cmd):\n",
                    "    return cmd\n",
                    "def spawn_local(args):\n",
                    "    for a in args:\n",
                    "        run(['git', a])\n",
                ),
            ),
        ],
        hot(&["*"]),
    );
    assert_eq!(
        sorted_causes(&found),
        [
            "perf:re-in-loop:m.regex_alias:17",
            "perf:re-in-loop:m.regex_bound:14",
            "perf:re-in-loop:m.regex_local_import:24",
            "perf:re-in-loop:m.regex_qualified:20",
            "perf:subprocess-in-loop:m.spawn_bound:8",
            "perf:subprocess-in-loop:m.spawn_qualified:11",
        ]
    );
}

/// The N+1 shape is many small requests. A response read chunk-wise in its own
/// `with` body is one bulk transfer.
#[test]
fn http_in_loop_leaves_a_response_drained_chunk_wise() {
    let found = run(
        &[(
            "m.py",
            concat!(
                "import requests\n",
                "def fetch_all(urls, path):\n",
                "    for url in urls:\n",
                "        download(url, path)\n",
                "        probe(url)\n",
                "def download(url, path):\n",
                "    with requests.get(url) as response:\n",
                "        with open(path, 'wb') as fh:\n",
                "            while chunk := response.read(8192):\n",
                "                fh.write(chunk)\n",
                "def probe(url):\n",
                "    with requests.get(url) as response:\n",
                "        return response.json()\n",
            ),
        )],
        hot(&["m.fetch_all"]),
    );
    assert_eq!(causes(&found), ["perf:http-in-loop:m.probe:12"]);
}

/// Like every catalog shape, `any([...])` is hot only in a loop, a hot
/// caller's loop included.
#[test]
fn materialized_short_circuit_needs_the_loop() {
    let files = [(
        "m.py",
        concat!(
            "def once(rows):\n",
            "    return any([r.ok for r in rows])\n",
            "def per_group(groups):\n",
            "    return [any([r.ok for r in rows]) for rows in groups]\n",
            "def looped(groups):\n",
            "    for rows in groups:\n",
            "        once(rows)\n",
        ),
    )];
    let found = run(&files, hot(&["m.once", "m.per_group"]));
    assert_eq!(
        causes(&found),
        ["perf:materialized-short-circuit:m.per_group:4"]
    );
    let found = run(&files, hot(&["m.looped"]));
    assert_eq!(causes(&found), ["perf:materialized-short-circuit:m.once:2"]);
}

const HELPERS: [(&str, &str); 1] = [(
    "m.py",
    concat!(
        "import subprocess\n",
        "def run_git(args):\n",
        "    return subprocess.run(['git', *args], capture_output=True)\n",
        "def join_parts(parts):\n",
        "    s = ''\n",
        "    s += parts[0]\n",
        "    return s\n",
        "def looped(paths):\n",
        "    for p in paths:\n",
        "        run_git(['blame', p])\n",
        "        join_parts(p)\n",
        "def straight(paths):\n",
        "    run_git(['status'])\n",
        "    join_parts(paths)\n",
    ),
)];

/// A helper with no local loop fires when a hot root calls it inside a loop
/// (amp 1) and stays silent when the same call sits outside any loop (amp 0).
/// `str +=` stays local-loop-only: its accumulator is fresh per call.
#[test]
fn amplification_supplies_the_loop() {
    let found = run(&HELPERS, hot(&["m.looped"]));
    assert_eq!(causes(&found), ["perf:subprocess-in-loop:m.run_git:3"]);
    let found = run(&HELPERS, hot(&["m.straight"]));
    assert!(found.is_empty(), "{:?}", causes(&found));
}

const FILTER: [(&str, &str); 1] = [(
    "m.py",
    concat!(
        "def by_name(rows, key):\n",
        "    \"\"\"Hot path: called per request.\"\"\"\n",
        "    for r in rows.values():\n",
        "        if r.name != key:\n",
        "            continue\n",
        "        return r\n",
        "def wrapped(rows, cfg):\n",
        "    \"\"\"Hot path: called per request.\"\"\"\n",
        "    out = []\n",
        "    for r in rows.values():\n",
        "        if cfg.key == r.name:\n",
        "            out.append(r)\n",
        "    return out\n",
        "def listed(rows, key):\n",
        "    \"\"\"Hot path: called per request.\"\"\"\n",
        "    for r in rows:\n",
        "        if r.name != key:\n",
        "            continue\n",
        "        return r\n",
        "def rekeyed(rows, key):\n",
        "    \"\"\"Hot path: called per request.\"\"\"\n",
        "    for r in rows.values():\n",
        "        if r.name != key:\n",
        "            continue\n",
        "        key = r.next\n",
        "def guarded_late(rows, key):\n",
        "    \"\"\"Hot path: called per request.\"\"\"\n",
        "    for r in rows.values():\n",
        "        r.touch()\n",
        "        if r.name != key:\n",
        "            continue\n",
        "        r.use()\n",
        "def branched(rows, key):\n",
        "    \"\"\"Hot path: called per request.\"\"\"\n",
        "    for r in rows.values():\n",
        "        if r.name == key:\n",
        "            r.use()\n",
        "        else:\n",
        "            r.skip()\n",
        "def by_value(rows, key):\n",
        "    \"\"\"Hot path: called per request.\"\"\"\n",
        "    for r in rows.values():\n",
        "        if r != key:\n",
        "            continue\n",
        "        return r\n",
        "def unpacked(pairs, key):\n",
        "    \"\"\"Hot path: called per request.\"\"\"\n",
        "    for name, r in pairs.values():\n",
        "        if r.name != key:\n",
        "            continue\n",
        "        return r\n",
    ),
)];

/// `if x.f != k: continue` and `if x.f == k:` around the whole body fire on a
/// dict's values with a loop-invariant key; a list, a key stored in the loop, a
/// guard that is not the first statement, an else branch, a test on the element
/// itself, and a tuple target stay silent.
#[test]
fn filter_scan_fires_on_both_guards_only() {
    let found = run(&FILTER, Config::new());
    assert_eq!(
        causes(&found),
        [
            "perf:filter-scan:m.by_name:4",
            "perf:filter-scan:m.wrapped:11"
        ]
    );
}

/// `seen = []; if x in seen: ...; seen.append(x)` in a hot loop is the
/// attribute shape on a local; a local bound to a set, a list parameter, a
/// local list the loop only reads and one seeded from another collection stay
/// silent.
#[test]
fn list_membership_covers_a_local_grown_in_the_probing_loop() {
    let found = run(
        &[(
            "m.py",
            concat!(
                "def dedup(items):\n",
                "    seen = []\n",
                "    for x in items:\n",
                "        if x in seen:\n",
                "            continue\n",
                "        seen.append(x)\n",
                "    return seen\n",
                "def dedup_set(items):\n",
                "    seen = set()\n",
                "    for x in items:\n",
                "        if x in seen:\n",
                "            continue\n",
                "        seen.add(x)\n",
                "    return seen\n",
                "def dedup_param(items, seen):\n",
                "    for x in items:\n",
                "        if x in seen:\n",
                "            continue\n",
                "        seen.append(x)\n",
                "def positions(items):\n",
                "    order = list(items)\n",
                "    for x in items:\n",
                "        order.index(x)\n",
                "        order.insert(0, x)\n",
                "def allowed_only(items, cfg):\n",
                "    allowed = list(cfg)\n",
                "    for x in items:\n",
                "        if x in allowed:\n",
                "            yield x\n",
            ),
        )],
        hot(&["m.*"]),
    );
    assert_eq!(
        sorted_causes(&found),
        ["perf:list-attr-membership:m.dedup:4"]
    );
}

/// `sorted(xs)[0]` / `[-1]` in a loop, a `key=` kept; `[:k]`, a middle index,
/// `min`, and the same shape outside any loop stay silent.
#[test]
fn sorted_head_fires_on_the_extremes_only() {
    let found = run(
        &[(
            "m.py",
            concat!(
                "def firsts(groups):\n",
                "    return [sorted(xs)[0] for xs in groups]\n",
                "def lasts(groups, key):\n",
                "    out = []\n",
                "    for xs in groups:\n",
                "        out.append(sorted(xs, key=key)[-1])\n",
                "    return out\n",
                "def heads(groups):\n",
                "    return [sorted(xs)[:3] for xs in groups]\n",
                "def medians(groups):\n",
                "    return [sorted(xs)[1] for xs in groups]\n",
                "def mins(groups):\n",
                "    return [min(xs) for xs in groups]\n",
                "def once(xs):\n",
                "    return sorted(xs)[0]\n",
            ),
        )],
        hot(&["m.*"]),
    );
    assert_eq!(
        sorted_causes(&found),
        ["perf:sorted-head:m.firsts:2", "perf:sorted-head:m.lasts:6"]
    );
}
