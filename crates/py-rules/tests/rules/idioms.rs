//! #12's idiom catalog: the whole-function catalog and the node-level
//! idioms.

use sightline_core::findings::Finding;
use sightline_testkit::run_rule;

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

/// An idiom inside a nested def belongs to the closure, not also to the
/// parent: one finding, owned by the closure; the parent's own idioms fire.
#[test]
fn closure_idiom_attributed_once() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            concat!(
                "def outer(d, e):\n",
                "    def inner(k):\n",
                "        return k in d.keys()\n",
                "    flag = True if e else False\n",
                "    return inner\n",
            ),
        )],
    );
    let mut found = causes(&findings);
    found.sort_unstable();
    assert_eq!(
        found,
        [
            "idiom:bool-ternary:m.outer:4:11",
            "idiom:keys-membership:m.outer.inner:3:15",
        ]
    );
}

/// Two `.keys()` membership tests on one line are two keys.
#[test]
fn two_idioms_on_one_line_are_two_keys() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            "def both(d, a, b):\n    return a in d.keys() and b in d.keys()\n",
        )],
    );
    let unique: std::collections::BTreeSet<&str> = causes(&findings).into_iter().collect();
    assert_eq!(unique.len(), 2);
    assert_eq!(findings.len(), 2);
}

#[test]
fn fires_on_binary_search_reimplementation() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            concat!(
                "def find(items, target):\n",
                "    lo, hi = 0, len(items) - 1\n",
                "    while lo <= hi:\n",
                "        mid = (lo + hi) // 2\n",
                "        if items[mid] == target:\n",
                "            return mid\n",
                "        elif items[mid] < target:\n",
                "            lo = mid + 1\n",
                "        else:\n",
                "            hi = mid - 1\n",
                "    return -1\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["idiom:binary-search:m.find"]);
    assert!(findings[0].message.contains("bisect"));
}

#[test]
fn fires_on_clamp_chain() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            concat!(
                "def clamp(x, lo, hi):\n",
                "    if x < lo:\n",
                "        return lo\n",
                "    elif x > hi:\n",
                "        return hi\n",
                "    return x\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["idiom:clamp:m.clamp"]);
}

#[test]
fn fires_on_manual_lower_loop() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            concat!(
                "def lower(s):\n",
                "    out = ''\n",
                "    for c in s:\n",
                "        if 'A' <= c <= 'Z':\n",
                "            out += chr(ord(c) + 32)\n",
                "        else:\n",
                "            out += c\n",
                "    return out\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["idiom:tolower:m.lower"]);
}

#[test]
fn fires_on_manual_sum() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            "def total(xs):\n    acc = 0\n    for x in xs:\n        acc += x\n    return acc\n",
        )],
    );
    assert_eq!(causes(&findings), ["idiom:manual-sum:m.total"]);
}

/// Expression and loop-shape idioms beyond the whole-function catalog.
#[test]
fn node_level_idioms() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            concat!(
                "def a(xs):\n",
                "    return [x for x in xs]\n",
                "def b(flag):\n",
                "    return True if flag else False\n",
                "def c(xs):\n",
                "    out = []\n",
                "    for i in range(len(xs)):\n",
                "        out.append(xs[i] * 2)\n",
                "    return out\n",
                "def d(table, k):\n",
                "    return k in table.keys()\n",
            ),
        )],
    );
    let kinds: std::collections::BTreeSet<&str> = findings
        .iter()
        .map(|f| f.cause.split(':').nth(1).expect("an idiom name"))
        .collect();
    assert_eq!(
        kinds,
        std::collections::BTreeSet::from([
            "identity-comp",
            "bool-ternary",
            "range-len",
            "keys-membership",
        ])
    );
}

#[test]
fn node_idioms_silent_on_legit() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            concat!(
                "def a(xs):\n",
                "    return [x * 2 for x in xs]\n", // transforms
                "def a2(xs):\n",
                "    return [x for x in xs if x]\n", // filters
                "def b(flag):\n",
                "    return 1 if flag else 0\n", // not a bool identity
                "def c(xs, ys):\n",
                "    out = []\n",
                "    for i in range(len(xs)):\n",
                "        out.append(ys[i])\n", // parallel lists, not enumerate
                "    return out\n",
                "def c2(xs):\n",
                "    for i in range(len(xs)):\n",
                "        xs.append(xs[i])\n", // mutates: range(len()) is fixed
                "    return xs\n",
                "def c3(xs):\n",
                "    for i in range(len(xs)):\n",
                "        xs[i] = xs[i] * 2\n", // writes through xs
                "    return xs\n",
                "def c4(xs):\n",
                "    for i in range(len(xs)):\n",
                "        if xs[i]:\n",
                "            del xs[i]\n", // resizes
                "    return xs\n",
                "def c5(xs):\n",
                "    for i in range(len(xs)):\n",
                "        i = i + 1\n", // rebinds the index
                "        print(xs[i])\n",
                "    return xs\n",
                "def d(table, k):\n",
                "    return k in table\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn silent_on_uses_of_the_idioms() {
    let findings = run_rule(
        "12",
        &[(
            "m.py",
            concat!(
                "import bisect\n",
                "def find(items, target):\n",
                "    return bisect.bisect_left(items, target)\n",
                "def clamp(x, lo, hi):\n",
                "    return min(max(x, lo), hi)\n",
                "def total(xs):\n",
                "    return sum(xs)\n",
                "def weighted(xs):\n", // sum with transformation: not manual-sum
                "    acc = 0\n",
                "    for x in xs:\n",
                "        if x.ok:\n",
                "            acc += x.v * 2\n",
                "    return acc\n",
            ),
        )],
    );
    assert!(findings.is_empty(), "{findings:?}");
}
