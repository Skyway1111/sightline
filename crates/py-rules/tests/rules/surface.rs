//! `tests/rules/test_surface.py`: family B's paired positive/negative
//! fixtures for #11, #14, #18, #20, #21, #23, #37, #48, #54 and #55, plus
//! `tests/test_fixes.py`'s `fold_splice` cases.
//!
//! `clippy.toml` bans a process spawn inside the crate so no rule reads the
//! world; `aged_run` is a fixture writing a git history for #11's migration
//! grace, and holds the one `allow` in this file.

use sightline_core::findings::Finding;
use sightline_core::git::GitAges;
use sightline_py_rules::surface::fold_splice;
use sightline_testkit::{build, run_rule, run_rule_on};

fn rule(id: &str, files: &[(&str, String)]) -> Vec<Finding> {
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(r, s)| (*r, s.as_str())).collect();
    run_rule(id, &borrowed)
}

fn causes(findings: &[Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.cause.as_str()).collect()
}

fn arm<'a>(findings: &'a [Finding], prefix: &str) -> Vec<&'a Finding> {
    findings
        .iter()
        .filter(|f| f.cause.starts_with(prefix))
        .collect()
}

// --- #11 structural clones ---------------------------------------------------

const CLONE_BODY: &str = concat!(
    "    total = 0\n",
    "    for row in rows:\n",
    "        if row.active:\n",
    "            total += row.value * row.weight\n",
    "    return total\n",
);

const ARRANGE: &str = concat!(
    "    conn = pool.acquire()\n",
    "    conn.auth(user)\n",
    "    rows = conn.fetch(query)\n",
    "    conn.release()\n",
);

/// A five-call chain over the given locals and callees (#11's blind-name
/// shape).
fn blind_body(names: &str, calls: &str) -> String {
    let n: Vec<&str> = names.split_whitespace().collect();
    let c: Vec<&str> = calls.split_whitespace().collect();
    format!(
        "    {} = {}(v)\n    {} = {}({})\n    {} = {}({}, 1)\n    {} = {}({}, 9)\n    return {}({})\n",
        n[0], c[0], n[1], c[1], n[0], n[2], c[2], n[1], n[3], c[3], n[2], c[4], n[3]
    )
}

/// Short arrange blocks in test files are not clones; whole-function test
/// twins count toward the group but never carry findings.
#[test]
fn test_members_carry_no_findings() {
    let findings = rule(
        "11",
        &[
            ("m.py", "x = 1\n".to_string()),
            (
                "tests/test_a.py",
                format!(
                    "def test_one(pool, user, query):\n{ARRANGE}    assert rows\n    \
                     assert pool.closed\n    assert user\n"
                ),
            ),
            (
                "tests/test_b.py",
                format!(
                    "def test_two(pool, user, query):\n{ARRANGE}    assert not rows\n    \
                     assert query\n    assert conn\n"
                ),
            ),
        ],
    );
    assert!(arm(&findings, "clone-block:").is_empty());

    let twins = rule(
        "11",
        &[
            ("m.py", "x = 1\n".to_string()),
            (
                "tests/test_a.py",
                format!("def test_one(pool, user, query):\n{ARRANGE}"),
            ),
            (
                "tests/test_b.py",
                format!("def test_two(pool, user, query):\n{ARRANGE}"),
            ),
        ],
    );
    assert!(twins.is_empty());

    // a prod twin of test functions is reported once, at the prod site
    let mixed = rule(
        "11",
        &[
            ("m.py", format!("def fetch(pool, user, query):\n{ARRANGE}")),
            (
                "tests/test_a.py",
                format!("def test_one(pool, user, query):\n{ARRANGE}"),
            ),
            (
                "tests/test_b.py",
                format!("def test_two(pool, user, query):\n{ARRANGE}"),
            ),
        ],
    );
    let rows: Vec<(&str, &str)> = mixed
        .iter()
        .map(|f| (&*f.site.rel, f.message.as_str()))
        .collect();
    assert_eq!(
        rows,
        [(
            "m.py",
            "structural clone x3: m.fetch, test_a.test_one, test_b.test_two"
        )]
    );
}

/// Both prod copies are priced, and the salience is `count * age`. The stub
/// `_StubAges` has no place here: `Provers.git_ages` holds a `GitAges`, so the
/// test commits the copies ten days back and leaves HEAD at today.
#[test]
fn migration_grace_prices_every_prod_copy() {
    let files = [
        ("a.py", format!("def score_users(rows):\n{CLONE_BODY}")),
        ("b.py", format!("def score_items(rows):\n{CLONE_BODY}")),
    ];
    let Some(findings) = aged_run(&files, &["a.py", "b.py"]) else {
        return; // no git on this machine
    };
    assert_eq!(findings.len(), 2);
    // two members, ten days: `float(count * age)`, not `float(count)`
    assert!(findings.iter().all(|f| f.salience == 20.0), "{findings:?}");
}

/// The age is asked of the reported copies alone: a test twin committed today
/// earns the prod copy no grace. Priced with the test span the group would be
/// nought days old and every salience zero.
#[test]
fn migration_grace_prices_only_the_copies_a_reader_must_move() {
    let files = [
        ("a.py", format!("def score_users(rows):\n{CLONE_BODY}")),
        (
            "tests/test_a.py",
            format!("def test_score(rows):\n{CLONE_BODY}"),
        ),
    ];
    let Some(findings) = aged_run(&files, &["a.py"]) else {
        return;
    };
    assert_eq!(findings.len(), 1);
    assert_eq!(&*findings[0].site.rel, "a.py");
    assert_eq!(findings[0].salience, 20.0); // x2 members, one priced, ten days
}

/// #11 over a repo whose `old` files were committed ten days ago and whose
/// remaining files landed at HEAD today. `None` where git cannot answer.
/// `core::git` reads the committer time, which only the environment sets, so
/// this fixture spawns git itself.
#[allow(clippy::disallowed_types, clippy::disallowed_methods)]
fn aged_run(files: &[(&str, String)], old: &[&str]) -> Option<Vec<Finding>> {
    use std::process::Command;

    let borrowed: Vec<(&str, &str)> = files.iter().map(|(r, s)| (*r, s.as_str())).collect();
    let (dir, mut stack) = build(&borrowed);
    let root = camino::Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let back = format!(
        "{} +0000",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs()
            - 10 * 24 * 3600
    );
    let run = |args: &[&str], date: Option<&str>| {
        let mut cmd = Command::new("git");
        cmd.current_dir(root).args(args);
        cmd.env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t");
        cmd.env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t");
        if let Some(date) = date {
            cmd.env("GIT_AUTHOR_DATE", date)
                .env("GIT_COMMITTER_DATE", date);
        }
        cmd.output().ok().is_some_and(|out| out.status.success())
    };
    let mut ok = run(&["init", "-q"], None);
    for rel in old {
        ok = ok && run(&["add", "--", rel], None);
    }
    ok = ok
        && run(&["commit", "-q", "-m", "copies"], Some(&back))
        && run(&["add", "-A"], None)
        && run(&["commit", "-q", "--allow-empty", "-m", "head"], None);
    if !ok {
        return None;
    }
    let git = GitAges::new(root);
    if !git.available() {
        return None;
    }
    stack.provers.git_ages = Some(git);
    Some(run_rule_on("11", &stack))
}

/// A prod block whose twin lives in a test file is real duplication: findings
/// land only at the prod member.
#[test]
fn prod_test_twin_reports_prod_site_only() {
    let findings = rule(
        "11",
        &[
            (
                "cache.py",
                format!(
                    "def prod_fetch(pool, user, query):\n    setup()\n{ARRANGE}    return rows\n"
                ),
            ),
            (
                "tests/test_srv.py",
                format!(
                    "def test_fetch(pool, user, query):\n    fixture()\n{ARRANGE}    assert rows\n"
                ),
            ),
        ],
    );
    let blocks = arm(&findings, "clone-block:");
    assert_eq!(blocks.len(), 1);
    assert_eq!(&*blocks[0].site.rel, "cache.py");
}

/// Sub-windows wholly inside a reported longer block clone add nothing.
#[test]
fn nested_windows_collapse_to_one_group() {
    let block = format!("{ARRANGE}    log(rows)\n");
    let findings = rule(
        "11",
        &[
            (
                "a.py",
                format!("def first(pool, user, query):\n    setup_a()\n{block}    return rows\n"),
            ),
            (
                "b.py",
                format!(
                    "def second(pool, user, query):\n    other_setup()\n{block}    return None\n"
                ),
            ),
        ],
    );
    let groups: std::collections::BTreeSet<&str> = arm(&findings, "clone-block:")
        .iter()
        .map(|f| f.cause.as_str())
        .collect();
    assert_eq!(groups.len(), 1);
}

#[test]
fn fires_on_structural_clone_pair() {
    let findings = rule(
        "11",
        &[
            ("a.py", format!("def score_users(rows):\n{CLONE_BODY}")),
            (
                "b.py",
                concat!(
                    "def score_items(entries):\n",
                    "    acc = 0\n",
                    "    for item in entries:\n",
                    "        if item.active:\n",
                    "            acc += item.value * item.weight\n",
                    "    return acc\n",
                )
                .to_string(),
            ),
        ],
    );
    assert_eq!(findings.len(), 2); // one per clone-group member
    let owners: std::collections::BTreeSet<&str> =
        findings.iter().map(|f| &*f.site.symbol).collect();
    assert_eq!(
        owners,
        std::collections::BTreeSet::from(["a.score_users", "b.score_items"])
    );
    // count-only without git
    assert!(findings.iter().all(|f| f.salience == 2.0));
}

/// An async clone of a sync function is the same T2 clone.
#[test]
fn async_shape_matches_sync_twin() {
    let findings = rule(
        "11",
        &[
            (
                "a.py",
                concat!(
                    "def score(rows):\n",
                    "    total = 0\n",
                    "    for row in rows:\n",
                    "        if row.active:\n",
                    "            total += row.value * row.weight\n",
                    "    return total\n",
                )
                .to_string(),
            ),
            (
                "b.py",
                concat!(
                    "async def score_async(rows):\n",
                    "    total = 0\n",
                    "    async for row in rows:\n",
                    "        if row.active:\n",
                    "            total += row.value * row.weight\n",
                    "    return total\n",
                )
                .to_string(),
            ),
        ],
    );
    assert_eq!(arm(&findings, "clone:").len(), 2);
}

/// Sub-function granularity: the same statement block inside two otherwise
/// different functions.
#[test]
fn block_clone_inside_larger_functions() {
    let block = concat!(
        "    conn = pool.acquire()\n",
        "    conn.auth(user)\n",
        "    rows = conn.fetch(query)\n",
        "    log(rows)\n",
        "    conn.release()\n",
    );
    let findings = rule(
        "11",
        &[
            (
                "a.py",
                format!(
                    "def report(pool, user, query):\n{block}    return [r.name for r in rows]\n"
                ),
            ),
            (
                "b.py",
                format!(
                    "def export(pool, user, query, sink):\n{block}    for r in rows:\n        \
                     sink.write(r)\n    sink.close()\n"
                ),
            ),
        ],
    );
    assert_eq!(arm(&findings, "clone-block:").len(), 2);
    assert!(arm(&findings, "clone:").is_empty());
}

/// A periodic statement run yields overlapping same-key windows; only
/// non-overlapping ones survive.
#[test]
fn block_arm_collapses_periodic_overlaps() {
    let period = concat!(
        "    acc = fn(acc) + fn(p)\n",
        "    for row in acc:\n",
        "        p = row.weight\n",
        "    acc = sorted(acc)\n",
    );
    let findings = rule(
        "11",
        &[(
            "a.py",
            format!(
                "def grind(fn, p, acc):\n{}    return acc\n",
                period.repeat(5)
            ),
        )],
    );
    let blocks = arm(&findings, "clone-block:");
    let mut lines: Vec<u32> = blocks.iter().map(|f| f.site.line).collect();
    lines.sort_unstable();
    assert_eq!(blocks.len(), 2);
    assert!(lines[1] - lines[0] >= 5);
}

/// Every statement of the window the same shape, only the literals differing:
/// an emit run is a table's form, not a fact with a home.
#[test]
fn one_shape_run_is_no_clone() {
    let emit: String = (0..8)
        .map(|i| format!("    out.append('row {i}')\n"))
        .collect();
    let findings = rule(
        "11",
        &[
            ("a.py", format!("def usage(out):\n{emit}    return out\n")),
            (
                "b.py",
                format!("def manual(out, sink):\n{emit}    sink.write(out)\n"),
            ),
        ],
    );
    assert!(arm(&findings, "clone-block:").is_empty());
}

/// A whole-body duplicate is the function arm's finding, not a block.
#[test]
fn block_arm_skips_whole_function_clones() {
    let findings = rule(
        "11",
        &[
            ("a.py", format!("def score(rows):\n{CLONE_BODY}")),
            ("b.py", format!("def rank(rows):\n{CLONE_BODY}")),
        ],
    );
    assert_eq!(arm(&findings, "clone:").len(), 2);
    assert!(arm(&findings, "clone-block:").is_empty());
}

/// Two scripts outside every package that nothing imports cannot import one
/// shared helper; the same pair inside a package has a home.
#[test]
fn copies_with_no_shared_home_are_one_home_each() {
    let pair = [
        (
            "demos/one/run.py",
            format!("def score_users(rows):\n{CLONE_BODY}"),
        ),
        (
            "demos/two/run.py",
            format!("def score_items(rows):\n{CLONE_BODY}"),
        ),
    ];
    assert!(rule("11", &pair).is_empty());
    let keys = concat!(
        "    seen = set()\n",
        "    for row in rows:\n",
        "        if row.active:\n",
        "            seen.add(row.key)\n",
        "    return seen\n",
    );
    let packaged = [
        ("pkg/__init__.py", String::new()),
        ("pkg/a.py", format!("def user_keys(rows):\n{keys}")),
        ("pkg/b.py", format!("def item_keys(rows):\n{keys}")),
    ];
    assert_eq!(rule("11", &packaged).len(), 2);
}

/// Blind normalization reads the two bodies as one shape; the declared type is
/// what each copy exists for, so they are not one fact.
#[test]
fn copies_declaring_different_types_are_not_one_fact() {
    let body = |t: &str| {
        format!(
            "        response: {t} = self._fetch(m)\n        if response['ok'] is False:\n            \
             raise ApiError(m, response)\n        return response\n"
        )
    };
    let facade = format!(
        "class Api:\n    def team(self, m):\n{}    def rtm(self, m):\n{}",
        body("TeamInfo"),
        body("RtmConnect")
    );
    assert!(rule("11", &[("m.py", facade.clone())]).is_empty());
    let one_type = facade.replace("RtmConnect", "TeamInfo");
    assert_eq!(rule("11", &[("m.py", one_type)]).len(), 2);
}

/// Per-member emission on an n-member group listing all n owners in every
/// message is O(n^2) report bytes. Cap at 3 plus a count.
#[test]
fn messages_cap_the_owner_list() {
    let files: Vec<(String, String)> = (0..5)
        .map(|i| {
            (
                format!("m{i}.py"),
                format!("def copy{i}(rows):\n{CLONE_BODY}"),
            )
        })
        .collect();
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(r, s)| (r.as_str(), s.as_str()))
        .collect();
    let findings = run_rule("11", &borrowed);
    assert_eq!(findings.len(), 5); // detection and ratchet grain untouched
    for f in &findings {
        assert_eq!(f.message.matches("copy").count(), 3);
        assert!(f.message.contains("+2 more"));
    }
}

#[test]
fn silent_on_first_copy_and_small_bodies() {
    let findings = rule(
        "11",
        &[
            ("a.py", format!("def score(rows):\n{CLONE_BODY}")),
            (
                "b.py",
                "def tiny_a(x):\n    return x + 1\ndef tiny_b(y):\n    return y + 1\n".to_string(),
            ),
        ],
    );
    assert!(findings.is_empty());
}

/// `int/abs/max` and `float/round/pow` over the same shape are one T2 family.
#[test]
fn blind_callee_names_are_one_clone() {
    let findings = rule(
        "11",
        &[(
            "m.py",
            format!(
                "def to_text(v):\n{}def to_repr(v):\n{}",
                blind_body("x y z w", "int abs max min str"),
                blind_body("a b c d", "float round pow sum repr")
            ),
        )],
    );
    let heads: Vec<&str> = findings.iter().map(|f| &f.cause[..6]).collect();
    assert_eq!(heads, ["clone:", "clone:"]);
}

// --- #11 expression clones ---------------------------------------------------

/// `expr` at four prod sites in two modules, each in a different statement
/// shape so only the expression itself repeats.
fn expr_sites(expr: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "a.py",
            format!(
                "def first(node):\n    return {expr}\ndef second(node, x):\n    name = {expr}\n    \
                 return name, x\n"
            ),
        ),
        (
            "b.py",
            format!(
                "def third(node):\n    print({expr})\ndef fourth(node):\n    return [{expr}]\n"
            ),
        ),
    ]
}

#[test]
fn expr_clone_fires_on_three_attribute_walk_at_four_sites() {
    let findings = rule("11", &expr_sites("node.func.value.id"));
    let exprs = arm(&findings, "expr-clone:");
    assert_eq!(exprs.len(), 1);
    let f = exprs[0];
    assert_eq!(
        (&*f.site.rel, f.site.line, &*f.site.symbol),
        ("a.py", 2, "a.first")
    );
    assert_eq!(
        f.message,
        "expression clone x4: a.first, a.second, b.fourth +1 more"
    );
    assert_eq!(f.salience, 4.0);
}

#[test]
fn expr_twin_under_attribute_floor_is_silent() {
    assert!(arm(&rule("11", &expr_sites("node.func.id")), "expr-clone:").is_empty());
    // a method call's name is an operation, not a step of the walk
    let findings = rule("11", &expr_sites("node.items.setdefault(x, []).append(x)"));
    assert!(arm(&findings, "expr-clone:").is_empty());
}

#[test]
fn expr_clone_silent_in_one_module_and_in_tests_only() {
    let files = expr_sites("node.func.value.id");
    let joined = format!("{}{}", files[0].1, files[1].1);
    assert!(arm(&rule("11", &[("a.py", joined)]), "expr-clone:").is_empty());
    let tests_only = [
        ("m.py", "x = 1\n".to_string()),
        ("tests/test_a.py", files[0].1.clone()),
        ("tests/test_b.py", files[1].1.clone()),
    ];
    assert!(arm(&rule("11", &tests_only), "expr-clone:").is_empty());
}

/// A third-party API path or an instance's own field walk is that object's
/// vocabulary, not repo knowledge.
#[test]
fn foreign_rooted_walks_are_silent() {
    let imported: Vec<(&str, String)> = expr_sites("os.path.join(os.path.dirname(node), x)")
        .into_iter()
        .map(|(rel, src)| (rel, format!("import os\n{src}")))
        .collect();
    assert!(arm(&rule("11", &imported), "expr-clone:").is_empty());
    assert!(arm(&rule("11", &expr_sites("self.a.b.c")), "expr-clone:").is_empty());
    // await is transparent; a call to a named function is its argument list
    let awaited: Vec<(&str, String)> = expr_sites("await self.a.b.c(x)")
        .into_iter()
        .map(|(rel, src)| (rel, src.replace("def ", "async def ")))
        .collect();
    assert!(arm(&rule("11", &awaited), "expr-clone:").is_empty());
    let called = expr_sites("helper(node.a.b, node.c.d)");
    assert!(arm(&rule("11", &called), "expr-clone:").is_empty());
}

#[test]
fn sub_expression_of_reported_shape_is_not_its_own_group() {
    let findings = rule("11", &expr_sites("a.b.c.d.e"));
    let exprs = arm(&findings, "expr-clone:");
    assert_eq!(exprs.len(), 1);
    assert_eq!(exprs[0].salience, 4.0);
    let inner = rule("11", &expr_sites("a.b.c.d"));
    assert_ne!(arm(&inner, "expr-clone:")[0].cause, exprs[0].cause);
}

#[test]
fn awaited_walk_is_one_site() {
    let files: Vec<(&str, String)> = expr_sites("await node.a.b.c.d(x)")
        .into_iter()
        .map(|(rel, src)| (rel, src.replace("def ", "async def ")))
        .collect();
    let findings = rule("11", &files);
    let exprs = arm(&findings, "expr-clone:");
    assert_eq!(exprs.len(), 1);
    assert_eq!(exprs[0].salience, 4.0); // not x8
}

// --- #14 data clump ----------------------------------------------------------

/// Overlapping subset clumps must not re-count one signature at every subset
/// size: the widest group owns the anchor.
#[test]
fn one_finding_per_anchor() {
    let sig = "host: str, port: int, user: str, timeout: int";
    let findings = rule(
        "14",
        &[(
            "m.py",
            format!(
                "def open_conn({sig}):\n    return host\ndef ping({sig}):\n    return port\n\
                 def close({sig}):\n    return user\n\
                 def probe(host: str, port: int, user: str):\n    return host\n"
            ),
        )],
    );
    let mut anchors: Vec<(&str, u32)> = findings
        .iter()
        .map(|f| (&*f.site.rel, f.site.line))
        .collect();
    let count = anchors.len();
    anchors.sort_unstable();
    anchors.dedup();
    assert_eq!(anchors.len(), count);
}

#[test]
fn fires_on_recurring_param_group() {
    let typed = concat!(
        "def connect(host: str, port: int, timeout: int):\n    pass\n",
        "def ping(host: str, port: int, timeout: int):\n    pass\n",
        "def trace(host: str, port: int, timeout: int, verbose):\n    pass\n",
    );
    let findings = rule("14", &[("m.py", typed.to_string())]);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("host"));
    assert_eq!(findings[0].salience, 3.0);
    // a trio one signature leaves undeclared is not a group missing a type
    let untyped = typed.replacen("timeout: int", "timeout", 1);
    assert!(rule("14", &[("m.py", untyped)]).is_empty());
}

#[test]
fn clump_silent_below_threshold() {
    let findings = rule(
        "14",
        &[(
            "m.py",
            concat!(
                "def connect(host: str, port: int, timeout: int):\n    pass\n",
                "def ping(host: str, port: int, timeout: int):\n    pass\n",
                "def other(a, b):\n    pass\n",
            )
            .to_string(),
        )],
    );
    assert!(findings.is_empty());
}

/// A debug flag the whole module threads rides an order of magnitude more
/// signatures than the group: it travels with nothing.
#[test]
fn an_ambient_parameter_is_no_clump() {
    let ambient: String = (0..25)
        .map(|i| {
            format!("def f{i}(debug: bool, a{i}: int, b{i}: int) -> int:\n    return a{i} + b{i}\n")
        })
        .collect();
    let trio: String = (0..3)
        .map(|i| {
            format!(
                "def g{i}(debug: bool, model: str, device: str) -> str:\n    return model + device\n"
            )
        })
        .collect();
    assert!(rule("14", &[("m.py", format!("{ambient}{trio}"))]).is_empty());
}

/// A repo-typed object every prod call site passes straight through under its
/// own name is context riding the signatures, never a clump member.
#[test]
fn threaded_context_is_no_clump() {
    let ctx = "class Facts:\n    pass\nclass Mod:\n    pass\n";
    let typed: String = ["a", "b", "c"]
        .iter()
        .map(|n| format!("def {n}(facts: Facts, mod: Mod, node: int):\n    return '{n}'\n"))
        .collect();
    let chain = "    return a(facts, mod, node), b(facts, mod, node), c(facts, mod, node)\n";
    let run = "def run(facts: Facts, mod: Mod, node: int):\n";
    let silent = rule("14", &[("m.py", format!("{ctx}{typed}{run}{chain}"))]);
    assert!(silent.is_empty());

    let loose: String = ["a", "b", "c"]
        .iter()
        .map(|n| format!("def {n}(host: str, port: int, timeout: int):\n    return '{n}'\n"))
        .collect();
    let fires = rule(
        "14",
        &[(
            "m.py",
            format!(
                "{loose}def run(host: str, port: int, timeout: int):\n    return a(host, port, \
                 timeout), b(host, port, timeout), c(host, port, timeout)\n"
            ),
        )],
    );
    assert_eq!(causes(&fires), ["clump:host,port,timeout"]);

    let uncalled = rule("14", &[("m.py", format!("{ctx}{typed}"))]);
    assert_eq!(causes(&uncalled), ["clump:facts,mod,node"]);
}

/// A def a decorator's wrapper calls with spelled-out arguments, and a slot the
/// framework binds through an external default, are chosen by the consumer.
#[test]
fn a_consumers_signature_is_no_clump() {
    let deco = concat!(
        "def command(name):\n",
        "    def outer(f):\n",
        "        def wrapper(buffer, args):\n",
        "            return f(buffer, args, {})\n",
        "        return wrapper\n",
        "    return outer\n",
    );
    let handlers: String = ["a", "b", "c"]
        .iter()
        .map(|n| {
            format!(
                "@command('{n}')\ndef cmd_{n}(buffer: str, args: list, options: dict):\n    \
                 return '{n}'\n"
            )
        })
        .collect();
    assert!(rule("14", &[("m.py", format!("{deco}{handlers}"))]).is_empty());

    let injected: String = ["a", "b", "c"]
        .iter()
        .map(|n| {
            format!(
                "def route_{n}(body: str, request: str, auth: str = Header(None)):\n    \
                 return '{n}'\n"
            )
        })
        .collect();
    assert!(
        rule(
            "14",
            &[("m.py", format!("from fastapi import Header\n{injected}"))]
        )
        .is_empty()
    );
}

/// A test's params are fixtures; an override's signature is the base's.
#[test]
fn fixtures_and_overrides_are_not_signatures() {
    let trio: String = ["a", "b", "c"]
        .iter()
        .map(|n| format!("def test_{n}(tmp_path, monkeypatch, capsys):\n    assert tmp_path\n"))
        .collect();
    let silent = rule(
        "14",
        &[
            ("tests/test_a.py", trio),
            (
                "views.py",
                concat!(
                    "class Renderer:\n",
                    "    def render(self, request, context, template):\n",
                    "        return template\n",
                    "class HtmlRenderer(Renderer):\n",
                    "    def render(self, request, context, template):\n",
                    "        return '<html>' + template\n",
                    "class TextRenderer(Renderer):\n",
                    "    def render(self, request, context, template):\n",
                    "        return template.strip()\n",
                )
                .to_string(),
            ),
        ],
    );
    assert!(silent.is_empty());

    let fires = rule(
        "14",
        &[(
            "views.py",
            concat!(
                "def page(request: str, context: dict, template: str):\n    return template\n",
                "def mail(request: str, context: dict, template: str):\n    return template\n",
                "def feed(request: str, context: dict, template: str):\n    return template\n",
            )
            .to_string(),
        )],
    );
    assert_eq!(causes(&fires), ["clump:context,request,template"]);
}

// --- #18 section comments ----------------------------------------------------

#[test]
fn fires_on_step_comments() {
    let findings = run_rule(
        "18",
        &[(
            "m.py",
            concat!(
                "def run(data):\n",
                "    # Step 1: load\n",
                "    x = []\n",
                "    for row in data:\n",
                "        x.append(row)\n",
                "    # Step 2: transform\n",
                "    return [i * 2 for i in x]\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["sections:m.run"]);
}

#[test]
fn phase_and_colon_labels_fire() {
    let findings = run_rule(
        "18",
        &[(
            "m.py",
            concat!(
                "def run(data):\n",
                "    # Phase 1: load\n",
                "    x = data or []\n",
                "    # Phase 2: emit\n",
                "    return x\n",
                "def walk(data):\n",
                "    # 1: collect\n",
                "    y = {}\n",
                "    # 2: link\n",
                "    return y\n",
            ),
        )],
    );
    let mut found = causes(&findings);
    found.sort_unstable();
    assert_eq!(found, ["sections:m.run", "sections:m.walk"]);
}

/// The phase is already its own function: the boundary the rule asks for
/// exists at that call, and only the label is left.
#[test]
fn a_label_on_a_call_is_no_phase() {
    let findings = run_rule(
        "18",
        &[(
            "m.py",
            concat!(
                "def run(data):\n",
                "    # Phase 1: encode\n",
                "    ctx = encode_all(data)\n",
                "    # Phase 2: decode\n",
                "    return decode_all(ctx)\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

/// Numbered lines of one comment block enumerate a rationale; a phase owns
/// code of its own.
#[test]
fn an_enumerated_rationale_is_no_phase() {
    let findings = run_rule(
        "18",
        &[(
            "m.py",
            concat!(
                "def run(data):\n",
                "    # 1. requires_grad_(True) after creation drops the flag\n",
                "    # 2. FSDP wraps mixed requires_grad badly either way\n",
                "    keep = [d for d in data if d]\n",
                "    return keep\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

#[test]
fn sections_silent_on_single_or_module_level() {
    let findings = run_rule(
        "18",
        &[(
            "m.py",
            concat!(
                "# --- module region markers are fine\n",
                "def run(data):\n",
                "    # normalize before use\n",
                "    return sorted(data)\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

// --- #20 repeated lambda -----------------------------------------------------

/// Two copies of a sort key are a coincidence a reader still holds in one
/// glance; the third is the pattern.
#[test]
fn lambda_fires_on_the_third_copy_only() {
    let key =
        |v: &str| format!("    return sorted(rows, key=lambda {v}: ({v}.date, {v}.priority))\n");
    let twice = format!("def a(rows):\n{}def b(rows):\n{}", key("r"), key("x"));
    assert!(rule("20", &[("m.py", twice.clone())]).is_empty());
    let findings = rule(
        "20",
        &[("m.py", format!("{twice}def c(rows):\n{}", key("q")))],
    );
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].salience, 3.0);
}

#[test]
fn lambda_silent_on_trivial_or_unique() {
    let findings = run_rule(
        "20",
        &[(
            "m.py",
            concat!(
                "def a(rows):\n",
                "    return sorted(rows, key=lambda r: r.date)\n",
                "    \n",
                "def b(rows):\n",
                "    return max(rows, key=lambda x: (x.status, x.id))\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

// --- #21 distributed invariant -----------------------------------------------

/// The field is the encapsulation the rule would ask for: an index, a `len`, a
/// `.get` on self's own container maintains no rule.
#[test]
fn a_read_of_the_class_own_field_is_not_an_invariant() {
    let findings = run_rule(
        "21",
        &[(
            "m.py",
            concat!(
                "class CaseMap:\n",
                "    def __init__(self):\n",
                "        self._d = {}\n",
                "    def get(self, key):\n",
                "        return self._d[key.lower()]\n",
                "    def put(self, key, v):\n",
                "        self._d[key.lower()] = v\n",
                "    def drop(self, key):\n",
                "        del self._d[key.lower()]\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

/// The helper is already the one home.
#[test]
fn a_call_to_the_class_own_helper_is_not_an_invariant() {
    let findings = run_rule(
        "21",
        &[(
            "m.py",
            concat!(
                "class Store:\n",
                "    def _norm(self, key, flag):\n",
                "        return key\n",
                "    def a(self):\n",
                "        return self._norm('k', True)\n",
                "    def b(self):\n",
                "        return self._norm('k', True)\n",
                "    def c(self):\n",
                "        return self._norm('k', True)\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

#[test]
fn fires_on_a_decision_over_self_state() {
    let findings = run_rule(
        "21",
        &[(
            "m.py",
            concat!(
                "class Job:\n",
                "    def __init__(self):\n",
                "        self._state = {}\n",
                "    def run(self):\n",
                "        return self._state.get('phase') == 'ready'\n",
                "    def stop(self):\n",
                "        if self._state.get('phase') == 'ready':\n",
                "            return 1\n",
                "        return 0\n",
                "    def poll(self):\n",
                "        return 2 if self._state.get('phase') == 'ready' else 3\n",
            ),
        )],
    );
    assert_eq!(findings.len(), 1);
    assert!(findings[0].cause.starts_with("invariant:m.Job"));
}

/// Repeated test asserts are the suite's vocabulary, not invariants.
#[test]
fn invariant_tests_exempt() {
    let cls = concat!(
        "class Job:\n",
        "    def run(self):\n",
        "        return self._state.get('phase') == 'ready'\n",
        "    def stop(self):\n",
        "        return not self._state.get('phase') == 'ready'\n",
        "    def poll(self):\n",
        "        return 2 if self._state.get('phase') == 'ready' else 3\n",
    );
    assert!(!run_rule("21", &[("m.py", cls)]).is_empty());
    assert!(run_rule("21", &[("m.py", "x = 1\n"), ("tests/test_m.py", cls)]).is_empty());
}

#[test]
fn invariant_silent_on_two_methods_only() {
    let findings = run_rule(
        "21",
        &[(
            "m.py",
            concat!(
                "class Job:\n",
                "    def run(self):\n",
                "        return self._state.get('phase') == 'ready'\n",
                "    def stop(self):\n",
                "        return not self._state.get('phase') == 'ready'\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

// --- #23 cognitive complexity ------------------------------------------------

const DEEP: &str = concat!(
    "def f(xs):\n",
    "    for a in xs:\n",
    "        if a:\n",
    "            for b in a:\n",
    "                if b:\n",
    "                    if b > 1:\n",
    "                        return b\n",
    "    return 0\n",
);

#[test]
fn cc_fires_at_threshold() {
    let findings = run_rule("23", &[("m.py", DEEP)]);
    assert_eq!(causes(&findings), ["cognitive-complexity:m.f"]);
    assert!(findings[0].salience >= 15.0);
}

#[test]
fn cc_silent_below_threshold() {
    let findings = run_rule(
        "23",
        &[(
            "m.py",
            "def f(xs):\n    for a in xs:\n        if a:\n            return a\n    return 0\n",
        )],
    );
    assert!(findings.is_empty());
}

// --- #37 speculative generality ----------------------------------------------

#[test]
fn monomorphic_param_fires_at_three_prod_sites() {
    let findings = run_rule(
        "37",
        &[
            (
                "m.py",
                concat!(
                    "def render(data, mode):\n",
                    "    return (data, mode)\n",
                    "def a(d):\n    return render(d, 'fast')\n",
                    "def b(d):\n    return render(d, 'fast')\n",
                    "def c(d):\n    return render(d, mode='fast')\n",
                ),
            ),
            // a differing TEST literal does not exercise prod flexibility
            (
                "tests/test_m.py",
                "from m import render\ndef test_render():\n    assert render([], 'slow')\n",
            ),
        ],
    );
    assert_eq!(causes(&findings), ["monomorphic:m.render:mode"]);
    assert!(findings[0].message.contains("'fast'"));
}

#[test]
fn varied_or_sparse_literals_silent() {
    let findings = run_rule(
        "37",
        &[(
            "m.py",
            concat!(
                "def render(data, mode):\n",
                "    return (data, mode)\n",
                "def a(d):\n    return render(d, 'fast')\n",
                "def b(d):\n    return render(d, 'slow')\n",
                "def c(d):\n    return render(d, 'fast')\n",
                "def two(data, mode):\n",
                "    return (data, mode)\n",
                "def d(d):\n    return two(d, 'x')\n",
                "def e(d):\n    return two(d, 'x')\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

#[test]
fn never_overridden_default_fires() {
    let findings = run_rule(
        "37",
        &[(
            "m.py",
            concat!(
                "def save(row, atomic=True):\n",
                "    return (row, atomic)\n",
                "def a(r):\n    return save(r)\n",
                "def b(r):\n    return save(r)\n",
                "def c(r):\n    return save(r)\n",
            ),
        )],
    );
    assert_eq!(causes(&findings), ["unused-default:m.save:atomic"]);
}

#[test]
fn overridden_default_and_open_world_silent() {
    let findings = run_rule(
        "37",
        &[(
            "m.py",
            concat!(
                "def save(row, atomic=True):\n",
                "    return (row, atomic)\n",
                "def a(r):\n    return save(r)\n",
                "def b(r):\n    return save(r)\n",
                "def c(r):\n    return save(r, atomic=False)\n",
                "def escaped(row, mode='x'):\n",
                "    return (row, mode)\n",
                "def d(r):\n    return escaped(r)\n",
                "def e(r):\n    return escaped(r)\n",
                "def f(r):\n    return escaped(r)\n",
                "hook = escaped\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

/// A test caller is a veto, never evidence: a default a test overrides is the
/// seam that makes the branch testable.
#[test]
fn a_test_that_passes_the_default_exercises_it() {
    let findings = run_rule(
        "37",
        &[
            (
                "m.py",
                concat!(
                    "def root(system='posix'):\n",
                    "    return system\n",
                    "def a():\n    return root()\n",
                    "def b():\n    return root()\n",
                    "def c():\n    return root()\n",
                ),
            ),
            (
                "tests/test_m.py",
                "from m import root\ndef test_root():\n    assert root(system='nt')\n",
            ),
        ],
    );
    assert!(findings.is_empty());
}

/// A drop-in shim hands its whole parameter list to one third-party call and
/// returns it: the slots are that API's.
#[test]
fn a_mirrored_third_party_signature_is_no_knob() {
    let findings = run_rule(
        "37",
        &[(
            "m.py",
            concat!(
                "import torch.nn.functional as F\n",
                "def safe_interpolate(x, size=None, mode='nearest'):\n",
                "    return F.interpolate(x, size=size, mode=mode)\n",
                "def a(t):\n    return safe_interpolate(t, mode='bilinear')\n",
                "def b(t):\n    return safe_interpolate(t, mode='bilinear')\n",
                "def c(t):\n    return safe_interpolate(t, mode='bilinear')\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

#[test]
fn single_implementation_protocol_and_abc_fire() {
    let findings = run_rule(
        "37",
        &[(
            "m.py",
            concat!(
                "from typing import Protocol\n",
                "from abc import ABC, abstractmethod\n",
                "class Sink(Protocol):\n",
                "    def push(self, x): ...\n",
                "class FileSink(Sink):\n",
                "    def push(self, x):\n",
                "        return x\n",
                "class Store(ABC):\n",
                "    @abstractmethod\n",
                "    def get(self, k): ...\n",
                "class DiskStore(Store):\n",
                "    def get(self, k):\n",
                "        return k\n",
            ),
        )],
    );
    assert_eq!(
        causes(&findings),
        ["single-impl:m.Sink", "single-impl:m.Store"]
    );
    assert!(findings[0].message.contains("FileSink"));
}

#[test]
fn multi_impl_and_zero_impl_abstractions_silent() {
    let findings = run_rule(
        "37",
        &[(
            "m.py",
            concat!(
                "from abc import ABC, abstractmethod\n",
                "class Store(ABC):\n",
                "    @abstractmethod\n",
                "    def get(self, k): ...\n",
                "class DiskStore(Store):\n",
                "    def get(self, k):\n",
                "        return k\n",
                "class RamStore(Store):\n",
                "    def get(self, k):\n",
                "        return k\n",
                "from typing import Protocol\n",
                "class Sink(Protocol):\n",
                "    def push(self, x): ...\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

/// A Protocol is implemented by shape: any prod class with its methods is an
/// implementation whether or not it names the base.
#[test]
fn structural_implementers_count() {
    let sink = concat!(
        "from typing import Protocol\n",
        "class Sink(Protocol):\n",
        "    def write(self, s: str) -> None: ...\n",
        "class FileSink(Sink):\n",
        "    def write(self, s: str) -> None:\n",
        "        print(s)\n",
    );
    let silent = rule(
        "37",
        &[(
            "m.py",
            format!(
                "{sink}class MemSink:\n    def write(self, s: str) -> None:\n        self.buf = s\n"
            ),
        )],
    );
    assert!(silent.is_empty());

    // an implementer's subclasses implement it too (inherited methods)
    let inherited = run_rule(
        "37",
        &[(
            "m.py",
            concat!(
                "from typing import Protocol\n",
                "class Sink(Protocol):\n    def write(self, s: str) -> None: ...\n",
                "class Base:\n    def write(self, s: str) -> None:\n        print(s)\n",
                "class Loud(Base):\n    def shout(self) -> None:\n        print('!')\n",
            ),
        )],
    );
    assert!(inherited.is_empty());

    let fires = rule(
        "37",
        &[(
            "m.py",
            format!("{sink}class MemSink:\n    def flush(self) -> None:\n        self.buf = ''\n"),
        )],
    );
    assert_eq!(causes(&fires), ["single-impl:m.Sink"]);
}

/// The abstraction is the seam that makes the prod path testable: a double
/// exercises it, and only the double's own file is a test.
#[test]
fn a_test_double_is_a_second_implementation() {
    let repo = concat!(
        "from abc import ABC, abstractmethod\n",
        "class Repo(ABC):\n",
        "    @abstractmethod\n",
        "    def load(self): ...\n",
        "class SqlRepo(Repo):\n",
        "    def load(self):\n        return 1\n",
    );
    let fake = "from m import Repo\nclass FakeRepo(Repo):\n    def load(self):\n        return 0\n";
    let fires = run_rule("37", &[("m.py", repo)]);
    assert_eq!(causes(&fires), ["single-impl:m.Repo"]);
    assert!(run_rule("37", &[("m.py", repo), ("tests/test_repo.py", fake)]).is_empty());
}

/// A Protocol with no methods is implemented only by its nominal subclasses; a
/// Protocol with the same methods is an abstraction, never an implementation.
#[test]
fn attribute_only_protocol_and_abstractions_are_not_implementers() {
    let has_x = "from typing import Protocol\nclass HasX(Protocol):\n    x: int\n";
    let fires = rule(
        "37",
        &[(
            "m.py",
            format!("{has_x}class Point(HasX):\n    x: int = 0\nclass Other:\n    y: int = 0\n"),
        )],
    );
    assert_eq!(causes(&fires), ["single-impl:m.HasX"]);

    let twin = run_rule(
        "37",
        &[(
            "m.py",
            concat!(
                "from typing import Protocol\n",
                "class Sink(Protocol):\n    def write(self, s: str) -> None: ...\n",
                "class Sink2(Protocol):\n    def write(self, s: str) -> None: ...\n",
                "class FileSink:\n    def write(self, s: str) -> None:\n        print(s)\n",
            ),
        )],
    );
    assert_eq!(causes(&twin), ["single-impl:m.Sink", "single-impl:m.Sink2"]);
}

// --- #48 fold candidate ------------------------------------------------------

const HELPER: &str = "def _tidy(rows):\n    return sorted(r for r in rows if r)\n";

#[test]
fn fold_fires_on_single_prod_call_site() {
    let findings = rule(
        "48",
        &[(
            "m.py",
            format!("{HELPER}def load(rows):\n    return _tidy(rows)\n"),
        )],
    );
    let rows: Vec<(&str, &str, &str)> = findings
        .iter()
        .map(|f| (f.cause.as_str(), f.message.as_str(), f.tier().value()))
        .collect();
    assert_eq!(
        rows,
        [(
            "fold:m._tidy",
            "m._tidy (one line) is called once, from m.load: fold it into the caller",
            "indexed",
        )]
    );

    let method = run_rule(
        "48",
        &[(
            "m.py",
            concat!(
                "class Loader:\n",
                "    def _tidy(self, rows):\n",
                "        return [r for r in rows if r]\n",
                "    def load(self, rows):\n",
                "        return self._tidy(rows)\n",
            ),
        )],
    );
    assert_eq!(causes(&method), ["fold:m.Loader._tidy"]);
}

#[test]
fn fold_silent_when_the_name_is_earned() {
    let cases: Vec<(&str, String)> = vec![
        (
            "two callers",
            format!("{HELPER}def a(r):\n    return _tidy(r)\ndef b(r):\n    return _tidy(r)\n"),
        ),
        (
            "called twice by one caller",
            format!("{HELPER}def a(r):\n    return _tidy(r) + _tidy(r)\n"),
        ),
        (
            "by-value reference",
            format!("{HELPER}TABLE = {{'tidy': _tidy}}\ndef a(r):\n    return _tidy(r)\n"),
        ),
        (
            "public",
            format!(
                "{}def a(r):\n    return tidy(r)\n",
                HELPER.replace("_tidy", "tidy")
            ),
        ),
        (
            "decorated",
            format!("import functools\n@functools.cache\n{HELPER}def a(r):\n    return _tidy(r)\n"),
        ),
        (
            "recursive only",
            "def _walk(n):\n    for c in n:\n        _walk(c)\n".to_string(),
        ),
        (
            "call in a comprehension",
            format!("{HELPER}def a(rs):\n    return [_tidy(r) for r in rs]\n"),
        ),
        (
            "over the statement bar",
            format!(
                "def _big(r):\n    {}\ndef a(r):\n    return _big(r)\n",
                (0..5)
                    .map(|i| format!("r = r + {i}"))
                    .collect::<Vec<String>>()
                    .join("; ")
            ),
        ),
        (
            "stub",
            "def _todo(r):\n    ...\ndef a(r):\n    return _todo(r)\n".to_string(),
        ),
    ];
    for (label, src) in &cases {
        assert!(rule("48", &[("m.py", src.clone())]).is_empty(), "{label}");
    }
    let test_only = run_rule(
        "48",
        &[
            ("m.py", HELPER),
            (
                "tests/test_m.py",
                "from m import _tidy\ndef test_t():\n    assert _tidy([1])\n",
            ),
        ],
    );
    assert!(test_only.is_empty());
    let test_pinned = rule(
        "48",
        &[
            (
                "m.py",
                format!("{HELPER}def load(rows):\n    return _tidy(rows)\n"),
            ),
            (
                "tests/test_m.py",
                "from m import _tidy\ndef test_t():\n    assert _tidy([1])\n".to_string(),
            ),
        ],
    );
    assert!(test_pinned.is_empty(), "a direct test pins the helper");
}

/// A statement count is not the size a reader pays. One line, one substitution.
#[test]
fn a_body_past_one_line_is_not_a_hop() {
    let bodies = [
        (
            "block under a loop",
            "    for r in rows:\n        if r:\n            yield r\n",
        ),
        (
            "two lines",
            "    out = [r for r in rows if r]\n    return sorted(out)\n",
        ),
        (
            "one statement, wrapped",
            "    return sorted(\n        r for r in rows if r\n    )\n",
        ),
    ];
    for (label, body) in bodies {
        let src =
            format!("def _tidy(rows):\n{body}def load(rows):\n    return list(_tidy(rows))\n");
        assert!(rule("48", &[("m.py", src)]).is_empty(), "{label}");
    }
}

/// The name is what the table means, and the call site would read the members
/// instead.
#[test]
fn a_named_vocabulary_is_not_a_hop() {
    let table = concat!(
        "def _is_allow(d):\n    return d.startswith(('allow', 'pass', 'yes'))\n",
        "def judge(d):\n    return 1 if _is_allow(d) else 0\n",
    );
    assert!(run_rule("48", &[("m.py", table)]).is_empty());
    let pair = table.replace("'allow', 'pass', 'yes'", "'allow', 'pass'");
    assert_eq!(causes(&rule("48", &[("m.py", pair)])), ["fold:m._is_allow"]);
}

/// A test reaching the name off a fixture-loaded module pins the helper; a
/// prod string table naming it is a reader too; a prod attribute the index
/// cannot resolve is not.
#[test]
fn a_test_reaching_the_name_off_a_loaded_module_pins_the_helper() {
    let prod = format!("{HELPER}def load(rows):\n    return _tidy(rows)\n");
    let prod_attr = rule(
        "48",
        &[
            ("m.py", prod.clone()),
            ("o.py", "def g(x):\n    return x._tidy\n".to_string()),
        ],
    );
    assert_eq!(causes(&prod_attr), ["fold:m._tidy"]);
    let tabled = rule(
        "48",
        &[("m.py", format!("{prod}COVERAGE = ('m._tidy',)\n"))],
    );
    assert!(tabled.is_empty());
    let pinned = rule(
        "48",
        &[
            ("m.py", prod),
            (
                "tests/test_m.py",
                "import importlib\ngs = importlib.import_module('m')\ndef test_t():\n    \
                 assert gs._tidy([1])\n"
                    .to_string(),
            ),
        ],
    );
    assert!(pinned.is_empty());
}

/// A closure with one call site is a fold like any other private def; held by
/// value, called twice or calling itself it keeps its name.
#[test]
fn nested_def_folds_into_its_one_caller() {
    let nested = concat!(
        "def load(rows):\n",
        "    def _tidy(r):\n",
        "        return [x for x in r if x]\n",
        "    return _tidy(rows)\n",
    );
    assert_eq!(
        causes(&run_rule("48", &[("m.py", nested)])),
        ["fold:m.load._tidy"]
    );
    let twins = [
        (
            "held by value",
            nested.replace("return _tidy(rows)", "return map(_tidy, rows)"),
        ),
        (
            "called twice",
            nested.replace("return _tidy(rows)", "return _tidy(rows) + _tidy(rows)"),
        ),
        (
            "recursive",
            nested.replace("[x for x in r if x]", "[_tidy(x) for x in r if x]"),
        ),
    ];
    for (label, src) in twins {
        assert!(rule("48", &[("m.py", src)]).is_empty(), "{label}");
    }
}

/// The fold deletes what the helper had to say about itself.
#[test]
fn prose_of_its_own_is_not_a_hop() {
    let caller = "def load(rows):\n    return _tidy(rows)\n";
    let helpers = [
        (
            "docstring",
            "def _tidy(rows):\n    \"\"\"Blank rows never reach the table.\"\"\"\n    return sorted(rows)\n",
        ),
        (
            "comment in the body",
            "def _tidy(rows):\n    # the wire sends blanks between frames\n    return sorted(rows)\n",
        ),
    ];
    for (label, helper) in helpers {
        assert!(
            rule("48", &[("m.py", format!("{helper}{caller}"))]).is_empty(),
            "{label}"
        );
    }
    let bare = "def _tidy(rows):\n    return sorted(rows)\n";
    assert_eq!(
        causes(&rule("48", &[("m.py", format!("{bare}{caller}"))])),
        ["fold:m._tidy"]
    );
}

/// One call, and passed as a callback the call graph never sees.
#[test]
fn a_method_handed_over_by_value_is_not_a_fold() {
    let findings = run_rule(
        "48",
        &[(
            "m.py",
            concat!(
                "class Session:\n",
                "    def _enqueue(self, x):\n",
                "        return self.queue.append(x)\n",
                "    def _enqueue_and_stop(self, x):\n",
                "        self._enqueue(x)\n",
                "        self.stop()\n",
                "    def run(self):\n",
                "        return self.loop.run(self._enqueue)\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

#[test]
fn override_fixed_method_is_not_a_fold() {
    let findings = run_rule(
        "48",
        &[(
            "m.py",
            concat!(
                "class Base:\n",
                "    def _step(self, x):\n",
                "        return x\n",
                "class Impl(Base):\n",
                "    def _step(self, x):\n",
                "        return x + 1\n",
                "    def run(self, x):\n",
                "        return self._step(x)\n",
            ),
        )],
    );
    assert!(findings.is_empty());
}

/// Folding `for ...: return x` into the caller's loop returns from the caller;
/// a single-return rewrite still folds.
#[test]
fn an_inner_return_never_lands() {
    let scan = concat!(
        "def scan(batches):\n    out = []\n    for batch in batches:\n        ",
        "v = _first_ok(batch)\n        out.append(v)\n    return out\n",
    );
    let silent = rule(
        "48",
        &[(
            "m.py",
            format!("def _first_ok(xs):\n    for x in xs: return x\n{scan}"),
        )],
    );
    assert!(silent.is_empty());
    let fires = rule(
        "48",
        &[(
            "m.py",
            format!("def _first_ok(xs):\n    return next((x for x in xs if x > 0), None)\n{scan}"),
        )],
    );
    assert_eq!(causes(&fires), ["fold:m._first_ok"]);
}

/// A branching helper folded four loops deep pays its branch at that depth.
#[test]
fn the_fold_is_priced_at_the_call_site_depth() {
    let helper = "def _h(x, y):\n    return x if x > y else y\n";
    let deep = concat!(
        "def deep(xss):\n    out = []\n    for xs in xss:\n        for ys in xs:\n",
        "            for x in ys:\n                if x:\n                    r = _h(x, x)\n",
        "                    out.append(r)\n    return out\n",
    );
    assert!(rule("48", &[("m.py", format!("{helper}{deep}"))]).is_empty());
    let fires = rule(
        "48",
        &[(
            "m.py",
            format!("{helper}def flat(x):\n    r = _h(x, x)\n    return r\n"),
        )],
    );
    assert_eq!(causes(&fires), ["fold:m._h"]);
}

/// Statements land only where the call is the whole value of a statement; a
/// single-return body substitutes anywhere; a generator's only as a `for`
/// iterable.
#[test]
fn fold_needs_a_landing_site() {
    let two = "def _ok(r):\n    n = len(r); r.seen = n > 1\n";
    let silent = rule(
        "48",
        &[(
            "m.py",
            format!("{two}def a(r):\n    if _ok(r):\n        return 1\n    return 0\n"),
        )],
    );
    assert!(silent.is_empty());
    let fires = rule(
        "48",
        &[(
            "m.py",
            format!("{two}def a(r):\n    ok = _ok(r)\n    return ok\n"),
        )],
    );
    assert_eq!(causes(&fires), ["fold:m._ok"]);

    let one = "def _ok(r):\n    return len(r) > 1\n";
    let fires = rule(
        "48",
        &[(
            "m.py",
            format!("{one}def a(r):\n    if _ok(r):\n        return 1\n    return 0\n"),
        )],
    );
    assert_eq!(causes(&fires), ["fold:m._ok"]);

    let stream = "def _items(r):\n    yield from r\n";
    let silent = rule(
        "48",
        &[(
            "m.py",
            format!("{stream}def a(r):\n    return list(_items(r))\n"),
        )],
    );
    assert!(silent.is_empty());
    let fires = rule(
        "48",
        &[(
            "m.py",
            format!("{stream}def a(r):\n    for x in _items(r):\n        print(x)\n"),
        )],
    );
    assert_eq!(causes(&fires), ["fold:m._items"]);
}

/// A branching body folded into a caller past the complexity threshold trades
/// a hop for complexity; a branchless one-liner adds none.
#[test]
fn fold_never_into_a_caller_23_flags() {
    let branching = "def _ok(r):\n    return len(r) > 1 if r else False\n";
    let flat = "def _ok(r):\n    return len(r) > 1\n";
    let deep = concat!(
        "def a(xs):\n    for x in xs:\n        if x:\n            for y in x:\n",
        "                if y:\n                    if y > 1:\n",
        "                        return _ok(y)\n    return 0\n",
    );
    assert!(rule("48", &[("m.py", format!("{branching}{deep}"))]).is_empty());
    // the bound is the fold's result
    let near = deep.replace(
        "                    if y > 1:\n                        return _ok(y)\n",
        "                    return _ok(y)\n",
    );
    assert!(rule("48", &[("m.py", format!("{branching}{near}"))]).is_empty());
    let shallow = near.replace(
        "                if y:\n                    return _ok(y)\n",
        "                return _ok(y)\n",
    );
    let pairs = [
        (flat.to_string(), deep.to_string()),
        (branching.to_string(), shallow),
        (
            branching.to_string(),
            "def a(y):\n    return _ok(y)\n".to_string(),
        ),
    ];
    for (helper, caller) in pairs {
        let fires = rule("48", &[("m.py", format!("{helper}{caller}"))]);
        assert_eq!(causes(&fires), ["fold:m._ok"]);
    }
}

// --- fold_splice (`tests/test_fixes.py`'s #48 half) --------------------------

fn splice_of(src: &str, name: &str) -> Option<sightline_py_provers::counterfactual::Splice> {
    let (_dir, stack) = build(&[("m.py", src)]);
    fold_splice(&format!("fold:m.{name}"), stack.facts(), &stack.provers)
}

const FOLD: &str = concat!(
    "def _double(x: int) -> int:\n    return x * 2\n\n\n",
    "def use(n: int) -> int:\n    return _double(n) + 1\n",
);

/// The call site takes the returned expression with each param bound to its
/// argument, and the helper's lines go with it.
#[test]
fn fold_substitutes_the_returned_expression_and_deletes_the_helper() {
    let splice = splice_of(FOLD, "_double").expect("the fold splices");
    let mut lines: Vec<String> = FOLD.lines().map(str::to_string).collect();
    sightline_core::edits::apply_edits(&mut lines, &splice.edits);
    // not a landing: parenthesized
    assert_eq!(lines[5], "    return (n * 2) + 1");
    assert_eq!(lines[0], "");
    assert_eq!(lines[1], "");
    // only a name, a literal or an attribute chain moves to the call site
    let computed = FOLD.replace("_double(n)", "_double(n + 1)");
    assert!(splice_of(&computed, "_double").is_none());
}

#[test]
fn a_body_that_is_not_one_return_has_no_splice() {
    let src = concat!(
        "def _pair(x: int) -> int:\n    y = x + 1; return y * 2\n\n\n",
        "def use(n: int) -> int:\n    return _pair(n)\n",
    );
    assert!(splice_of(src, "_pair").is_none());
}

/// `a and b` skipped the argument bound to `b`, and `b - a` ran the pair in
/// the other order. Only a name, a literal or an attribute chain off one may
/// move to where the body reads it.
#[test]
fn fold_substitutes_only_inert_arguments() {
    let short_circuit = concat!(
        "import os\nLOG = []\n\n\n",
        "def _pick(a, b):\n    return a and b\n\n\n",
        "def run(name):\n    return _pick(os.path.exists(name), LOG.pop())\n",
    );
    let reordered = concat!(
        "LOG = []\n\n\n",
        "def _order(a, b):\n    return b - a\n\n\n",
        "def run():\n    return _order(LOG.pop(), len(LOG))\n",
    );
    assert!(splice_of(short_circuit, "_pick").is_none());
    assert!(splice_of(reordered, "_order").is_none());
    let pure = concat!(
        "def _pick(a, b):\n    return a and b\n\n\n",
        "def run(x, y):\n    return _pick(x, y.flag)\n",
    );
    assert!(splice_of(pure, "_pick").is_some());
}

// --- #54 kind switch ---------------------------------------------------------

const SWITCH: &str = concat!(
    "def f(x):\n",
    "    if x.kind == 'a':\n        return 1\n",
    "    elif x.kind == 'b':\n        return 2\n",
    "    return 0\n",
);

/// `if`/`elif`, `in (...)` and `match` are one switch each; the finding sits at
/// the document-first site and names the functions.
#[test]
fn fires_on_a_literal_set_switched_in_three_functions() {
    let findings = rule(
        "54",
        &[(
            "m.py",
            format!(
                "{SWITCH}def g(k):\n    if k in ('a', 'b'):\n        return 1\n    return 0\n\
                 def h(x):\n    match x.kind:\n        case 'a' | 'c':\n            return 1\n        \
                 case 'b':\n            return 2\n    return 0\n"
            ),
        )],
    );
    assert_eq!(causes(&findings), ["kind-switch:a,b"]);
    assert_eq!(findings[0].site.line, 2);
    assert!(findings[0].message.contains("3 functions (m.f, m.g, m.h)"));
}

/// A kind tag is spelled the way a member of the enum it wants would be.
#[test]
fn silent_on_punctuation_extensions_and_numeric_strings() {
    for (label, lits) in [
        ("brackets", ("[", "]")),
        ("extensions", (".jpg", ".png")),
        ("scores", ("0.0", "1.0")),
    ] {
        let src: String = ["f", "g", "h"]
            .iter()
            .map(|n| {
                SWITCH
                    .replace("def f", &format!("def {n}"))
                    .replace("'a'", &format!("'{}'", lits.0))
                    .replace("'b'", &format!("'{}'", lits.1))
            })
            .collect();
        assert!(rule("54", &[("m.py", src)]).is_empty(), "{label}");
    }
}

#[test]
fn silent_on_two_functions_ints_or_one_shared_literal() {
    let findings = rule(
        "54",
        &[
            (
                "two.py",
                format!("{SWITCH}{}", SWITCH.replace("def f", "def g")),
            ),
            (
                "ints.py",
                concat!(
                    "def f(x):\n    if x.n == 1:\n        return 1\n    elif x.n == 2:\n        return 2\n",
                    "def g(x):\n    if x.n == 1:\n        return 1\n    elif x.n == 2:\n        return 2\n",
                    "def h(x):\n    if x.n in (1, 2):\n        return 1\n",
                )
                .to_string(),
            ),
            (
                "one.py",
                concat!(
                    "def f(x):\n    if x.kind == 'p':\n        return 1\n    elif x.kind == 'q':\n        return 2\n",
                    "def g(x):\n    if x.kind == 'p':\n        return 1\n    elif x.kind == 'r':\n        return 2\n",
                    "def h(x):\n    if x.kind == 'p':\n        return 1\n    elif x.kind == 's':\n        return 2\n",
                )
                .to_string(),
            ),
            (
                "tests/test_m.py",
                format!(
                    "{SWITCH}{}{}",
                    SWITCH.replace("def f", "def g"),
                    SWITCH.replace("def f", "def h")
                ),
            ),
        ],
    );
    assert!(findings.is_empty());
}

// --- #55 positional width ----------------------------------------------------

#[test]
fn fires_past_five_positional_params() {
    let findings = run_rule(
        "55",
        &[(
            "m.py",
            concat!(
                "def wide(a, b, c, d, e):\n    return a\n",
                "class K:\n",
                "    def m(self, a, b, c, d, e, f=1):\n        return a\n",
            ),
        )],
    );
    let rows: Vec<(&str, f64)> = findings
        .iter()
        .map(|f| (f.cause.as_str(), f.salience))
        .collect();
    assert_eq!(
        rows,
        [
            ("positional-width:m.wide", 5.0),
            ("positional-width:m.K.m", 6.0)
        ]
    );
}

#[test]
fn silent_under_the_width_past_a_marker_on_overrides_and_tests() {
    let findings = run_rule(
        "55",
        &[
            (
                "m.py",
                concat!(
                    "def four(a, b, c, d):\n    return a\n",
                    "def marked(a, b, c, d, *, e):\n    return a\n",
                    "def star(a, b, c, d, e, *rest):\n    return a\n",
                    "class Base:\n",
                    "    def run(self, a, b, c, d, e):\n        return a\n",
                    "class Child(Base):\n",
                    "    def run(self, a, b, c, d, e):\n        return b\n",
                ),
            ),
            // the same module-level signature in two prod modules is a plugin
            // contract: the dispatcher owns it, as a base owns an override
            ("plug/a.py", "def derive(a, b, c, d, e):\n    return a\n"),
            ("plug/b.py", "def derive(a, b, c, d, e):\n    return b\n"),
            (
                "tests/test_m.py",
                "def helper(a, b, c, d, e):\n    return a\n",
            ),
        ],
    );
    assert_eq!(causes(&findings), ["positional-width:m.Base.run"]);
}
