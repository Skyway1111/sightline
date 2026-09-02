//! The render half that needs a built Python stack rather than the synthetic
//! one `core::render`'s own tests use: the rollup over
//! real module sizes and fan-in, the SARIF rule table over the real
//! registry, the oracle header, the judged-sample keys, and a #5 patch
//! applied straight out of the JSON.

use camino::Utf8Path;
use serde_json::Value;
use sightline_core::edits::apply_edits;
use sightline_core::findings::{Evidence, Finding, Site, SpanEdit};
use sightline_core::lang::Stack;
use sightline_core::precision::rule_precision;
use sightline_core::registry::Registry;
use sightline_core::render::{AuditResult, to_json, to_sarif, to_text};
use sightline_py_facts::build::raw_lines;
use sightline_py_provers::oracle::Oracle;
use sightline_py_rules::RULES;
use sightline_testkit::{build, run_rule_on};

fn clone_at(rel: &str, line: u32, cause: &str, symbol: &str) -> Finding {
    Finding {
        rule: "11",
        site: Site {
            rel: rel.into(),
            line,
            col: 0,
            symbol: symbol.into(),
        },
        message: "structural clone x3: a.fn, b.fn, c.fn".to_string(),
        cause: cause.to_string(),
        evidence: Evidence::Idx {
            detail: "k".to_string(),
        },
        salience: 0.0,
        fix: None,
        lang: "py",
    }
}

fn py_registry() -> Registry {
    let records = RULES.iter().map(|r| r.record.clone()).collect();
    Registry::new(records).expect("the records build a registry")
}

#[test]
fn the_rollup_orders_modules_and_symbols_and_json_stays_complete() {
    let (_dir, stack) = build(&[
        ("a.py", "def fn():\n    pass\n\n\ndef other():\n    pass\n"),
        ("b.py", "def fn():\n    pass\n"),
        ("c.py", "import a\nimport b\n\nb.fn()\n"),
        ("test_z.py", "def fn():\n    pass\n"),
    ]);
    // the input is the ranked list: modules and symbols roll up in the order
    // their strongest finding ranks, and the test module rolls up last with
    // the most findings of any
    let mut findings: Vec<Finding> = (0..4)
        .map(|i| clone_at("test_z.py", 1, &format!("clone:t{i}"), "test_z.fn"))
        .collect();
    findings.extend([
        clone_at("b.py", 1, "clone:k", "b.fn"),
        clone_at("a.py", 5, "clone:other", "a.other"),
        clone_at("c.py", 4, "clone:k", "c"), // module scope
        clone_at("a.py", 1, "clone:k", "a.fn"),
        clone_at("a.py", 6, "clone:third", "a.other"),
    ]);
    let result = AuditResult::new(findings, stack.neutral());

    let text = to_text(&result);
    let heads: Vec<&str> = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with(' ') && !l.starts_with("sightline"))
        .collect();
    assert_eq!(
        heads,
        [
            "b.py  2 lines, fan-in 1 | 1 findings: #11 x1",
            "a.py  6 lines, fan-in 0 | 3 findings: #11 x3",
            "c.py  4 lines, fan-in 0 | 1 findings: #11 x1",
            "tests:",
            "test_z.py  2 lines, fan-in 0 | 4 findings: #11 x4",
        ]
    );
    let a_block: Vec<&str> = text
        .split("a.py  6 lines")
        .nth(1)
        .expect("the a.py block")
        .split("\nc.py")
        .next()
        .expect("the block ends at c.py")
        .lines()
        .collect();
    assert_eq!(a_block[1], "  other  L5-6 (2 lines)"); // ranks first
    assert_eq!(a_block[4], "  fn  L1-2 (2 lines)");
    assert!(a_block[5].starts_with("    1:0    indexed   #11  structural clone"));
    assert!(
        text.split("\nc.py")
            .nth(1)
            .expect("the c.py block")
            .contains("  (module scope)")
    );
    assert!(text.contains("findings 9")); // the header counts the real total
    assert_eq!(
        to_json(&result, &py_registry())
            .matches("\"cause\": \"clone:k\"")
            .count(),
        3
    );
}

#[test]
fn the_provenance_names_unresolved_modules_and_oracle_edges() {
    let (_dir, stack) = build(&[("a.py", "\"\"\"ok.\"\"\"\n")]);
    let mut result = AuditResult::new(Vec::new(), stack.neutral());
    let Value::Object(block) = serde_json::json!({
        "oracle": {
            "enabled": true, "unresolved_imports": 3,
            "unresolved_import_density": 3.0,
            "unresolved_modules": {"torch": 2, "absent": 1},
            "calls_resolved_by_oracle": 7,
        }
    }) else {
        unreachable!("a json object");
    };
    result.provers = block;
    let doc: Value =
        serde_json::from_str(&to_json(&result, &py_registry())).expect("the json document");
    let oracle = &doc["provenance"]["oracle"];
    assert_eq!(
        oracle["unresolved_modules"],
        serde_json::json!({"absent": 1, "torch": 2})
    );
    assert_eq!(oracle["calls_resolved_by_oracle"], 7);
    assert_eq!(oracle["unresolved_imports"], 3);
}

#[test]
fn the_sarif_rule_table_names_every_registered_id() {
    let (_dir, stack) = build(&[("a.py", "def fn():\n    pass\n")]);
    let result = AuditResult::new(Vec::new(), stack.neutral());
    let doc: Value =
        serde_json::from_str(&to_sarif(&result, &py_registry())).expect("the sarif document");
    let rules = doc["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("the rule table");
    let ids: Vec<&str> = rules
        .iter()
        .map(|r| r["id"].as_str().expect("an id"))
        .collect();
    let mut want: Vec<&str> = RULES.iter().map(|r| r.record.id).collect();
    want.sort_by_key(|id| id.parse::<u32>().expect("a rule id is a number"));
    want.dedup();
    assert_eq!(ids, want);
    assert!(rules.iter().all(|r| {
        !r["shortDescription"]["text"]
            .as_str()
            .unwrap_or("")
            .is_empty()
            && !r["fullDescription"]["text"]
                .as_str()
                .unwrap_or("")
                .is_empty()
    }));
}

#[test]
fn every_judged_sample_names_a_live_rule() {
    // a key is `<id>`, `<id>:<arm>`, `<lang>:<id>` or `<lang>:<id>:<arm>`
    let live: Vec<&str> = RULES.iter().map(|r| r.record.id).collect();
    for key in rule_precision().keys() {
        let id = key
            .strip_prefix("rs:")
            .unwrap_or(key)
            .split(':')
            .next()
            .expect("a key names an id");
        assert!(live.contains(&id), "no live rule holds {key}");
    }
}

#[test]
fn a_verified_lift_is_applicable_from_the_json_alone() {
    let (dir, mut stack) = build(&[(
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
    )]);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let import_roots = stack.facts().import_roots.clone();
    stack.provers.oracle =
        Some(Oracle::new(root, &[], &import_roots, None).expect("an in-process checker"));
    let findings = run_rule_on("5", &stack);
    stack.provers.close();

    let result = AuditResult::new(findings, stack.neutral());
    let doc: Value =
        serde_json::from_str(&to_json(&result, &py_registry())).expect("the json document");
    let rows = doc["findings"].as_array().expect("the findings list");
    assert_eq!(rows.len(), 1);
    let f = &rows[0];
    assert_eq!(f["cause"], "lift:m._scale:nums");
    assert_eq!(f["span"], serde_json::json!([1, 5]));

    let rel = f["fix"]["file"].as_str().expect("the patched file");
    let mut lines: Vec<String> = raw_lines(&root.join(rel))
        .iter()
        .map(|l| l.trim_end_matches('\n').trim_end_matches('\r').to_string())
        .collect();
    let edits: Vec<SpanEdit> = f["fix"]["edits"]
        .as_array()
        .expect("the edit list")
        .iter()
        .map(|e| SpanEdit {
            line: e["line"].as_u64().unwrap_or(0) as u32,
            col_start: e["col_start"].as_u64().unwrap_or(0) as u32,
            col_end: e["col_end"].as_u64().unwrap_or(0) as u32,
            text: e["text"].as_str().unwrap_or_default().to_string(),
        })
        .collect();
    apply_edits(&mut lines, &edits);
    assert_eq!(lines[0], "def _scale(nums: list[int]):");
    // the payload alone leaves the file parsing
    assert!(sightline_py_facts::build::parses(&lines.join("\n")));
}
