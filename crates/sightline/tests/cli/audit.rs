//! `audit`, `baseline`, `fix`, `facts` and `explain` on mini repos (port of
//! `tests/test_cli.py`).

use sightline_testkit::make_repo;

use crate::{NO_ORACLE, Out, findings, provenance, root, run};

const SOURCE: &str = "def run(data: object) -> object:\n\
                      \x20   # Step 1: load\n\
                      \x20   x = [data]\n\
                      \x20   # Step 2: emit\n\
                      \x20   return x\n";

fn repo() -> tempfile::TempDir {
    make_repo(&[("pyproject.toml", NO_ORACLE), ("m.py", SOURCE)])
}

#[test]
fn audit_prints_the_finding_line_and_two_runs_agree() {
    let dir = repo();
    let first = run(&["audit", &root(&dir)]);

    assert_eq!(first.code, 0);
    // the finding line, not the cause
    assert!(first.out.contains("#18"), "{}", first.out);
    assert!(!first.out.contains("section-comments"));
    assert!(first.out.contains("note: oracle disabled by config"));
    assert_eq!(run(&["audit", &root(&dir)]).out, first.out);
}

#[test]
fn all_skips_the_baseline() {
    let dir = repo();
    run(&["baseline", &root(&dir)]);

    let absorbed = run(&["audit", &root(&dir)]);
    assert!(absorbed.out.contains("findings 0 "), "{}", absorbed.out);
    assert!(absorbed.out.contains("baselined 2"));

    let every = run(&["audit", &root(&dir), "--all"]);
    assert!(every.out.contains("findings 2 "), "{}", every.out);
    assert!(every.out.contains("baselined 0") && every.out.contains("#18"));
}

#[test]
fn json_carries_the_provenance_and_two_runs_agree() {
    let dir = repo();
    let first = run(&["audit", &root(&dir), "--json"]);

    assert_eq!(first.code, 0);
    let prov = provenance(&first.out);
    assert_eq!(prov["oracle"]["enabled"], serde_json::json!(false));
    assert_eq!(
        prov["counts"]["findings"],
        serde_json::json!(findings(&first.out).len())
    );
    let doc: serde_json::Value = serde_json::from_str(&first.out).unwrap();
    for f in doc["findings"].as_array().unwrap() {
        for key in ["rule", "tier", "engine", "file", "line", "symbol"] {
            assert!(!f[key].is_null(), "a finding has no {key}");
        }
    }
    assert_eq!(run(&["audit", &root(&dir), "--json"]).out, first.out);
}

#[test]
fn a_baseline_absorbs_then_a_new_symbol_regresses_and_prune_restores_it() {
    let dir = repo();
    let written = run(&["baseline", &root(&dir)]);
    assert_eq!(written.code, 0);
    assert!(written.out.contains("baseline written"), "{}", written.out);

    let clean = run(&["audit", &root(&dir), "--json"]);
    assert!(findings(&clean.out).is_empty());
    assert!(
        provenance(&clean.out)["counts"]["baselined"]
            .as_u64()
            .unwrap()
            > 0
    );

    // a violation in a new symbol: every regression sits on that symbol
    let more = format!(
        "{SOURCE}def more(d):\n    # Step 1: a\n    y = 1\n    # Step 2: b\n    return y\n"
    );
    std::fs::write(dir.path().join("m.py"), &more).unwrap();
    let grown = run(&["audit", &root(&dir), "--json"]);
    let rules: Vec<String> = findings(&grown.out).iter().map(|f| f.0.clone()).collect();
    let symbols: Vec<String> = findings(&grown.out).iter().map(|f| f.2.clone()).collect();
    assert!(symbols.iter().all(|s| s == "m.more"), "{symbols:?}");
    assert!(rules.contains(&"18".to_string()), "{rules:?}");

    // reverted, prune leaves the baseline byte for byte as it was
    std::fs::write(dir.path().join("m.py"), SOURCE).unwrap();
    let baseline = dir.path().join(".sightline-baseline");
    let before = std::fs::read(&baseline).unwrap();
    let pruned = run(&["baseline", &root(&dir), "--prune"]);
    assert_eq!(pruned.code, 0);
    assert!(
        pruned.out.starts_with("pruned baseline: "),
        "{}",
        pruned.out
    );
    assert_eq!(std::fs::read(&baseline).unwrap(), before);
}

#[test]
fn prune_without_a_baseline_is_an_error() {
    let dir = repo();
    let pruned = run(&["baseline", &root(&dir), "--prune"]);
    assert_eq!(pruned.code, 1);
    assert_eq!(pruned.err.trim_end(), "no baseline to prune");
}

#[test]
fn config_reads_the_file_the_flag_names() {
    let dir = make_repo(&[("m.py", "X = 1\n")]);
    let config = dir.path().join("elsewhere.toml");
    std::fs::write(
        &config,
        "[tool.sightline]\noracle = false\nrules-off = ['29']\n",
    )
    .unwrap();

    let out = run(&[
        "audit",
        &root(&dir),
        "--json",
        "--config",
        &config.to_string_lossy(),
    ]);
    assert_eq!(provenance(&out.out)["rules_off"], serde_json::json!(["29"]));
}

#[test]
fn rules_restricts_the_run_and_the_header_names_it() {
    let dir = repo();
    let every = run(&["audit", &root(&dir), "--all", "--json"]);
    let mut rules: Vec<String> = findings(&every.out).iter().map(|f| f.0.clone()).collect();
    rules.sort();
    rules.dedup();
    assert_eq!(rules, ["18", "32"]);

    for spec in ["18", "section-comments"] {
        // an id or its slug
        let out = run(&["audit", &root(&dir), "--all", "--json", "--rules", spec]);
        let only: Vec<String> = findings(&out.out).iter().map(|f| f.0.clone()).collect();
        assert!(only.iter().all(|r| r == "18"), "{only:?}");
        assert_eq!(
            provenance(&out.out)["rules_only"],
            serde_json::json!(["18"])
        );
    }
    let text = run(&[
        "audit",
        &root(&dir),
        "--all",
        "--rules",
        "32,section-comments",
    ]);
    assert!(text.out.contains("\n  rules: #18, #32\n"), "{}", text.out);

    assert_ne!(run(&["audit", &root(&dir), "--rules", "nope"]).code, 0);
    // a gate running fewer rules is a false pass: the flag is not declared
    assert_ne!(
        run(&["gate", &root(&dir), "--rules", "18", "--files", "m.py"]).code,
        0
    );
}

#[test]
fn paths_filters_after_rank_and_the_facts_stay_repo_wide() {
    let dir = make_repo(&[
        ("pyproject.toml", NO_ORACLE),
        ("a/m.py", SOURCE),
        ("b/m.py", SOURCE),
    ]);
    let files = |out: &Out| {
        let mut rels: Vec<String> = findings(&out.out).iter().map(|f| f.1.clone()).collect();
        rels.sort();
        rels.dedup();
        rels
    };

    let under_a = run(&["audit", &root(&dir), "--all", "--json", "--paths", "a"]);
    assert_eq!(files(&under_a), ["a/m.py"]);
    assert_eq!(provenance(&under_a.out)["paths"], serde_json::json!(["a"]));
    // facts were built repo-wide
    assert_eq!(provenance(&under_a.out)["modules"], serde_json::json!(2));

    let absolute = format!("{}/b/m.py", root(&dir));
    for spec in ["b/m.py", absolute.as_str(), "./b"] {
        let out = run(&["audit", &root(&dir), "--all", "--json", "--paths", spec]);
        assert_eq!(files(&out), ["b/m.py"], "--paths {spec}");
    }
    let two = run(&["audit", &root(&dir), "--all", "--paths", "a", "b/m.py"]);
    assert!(two.out.contains("\n  paths: a, b/m.py\n"), "{}", two.out);

    // the root is the whole tree, not nothing
    for spec in [".", "./", root(&dir).as_str()] {
        let out = run(&["audit", &root(&dir), "--all", "--json", "--paths", spec]);
        assert_eq!(files(&out), ["a/m.py", "b/m.py"], "--paths {spec}");
        assert_eq!(provenance(&out.out)["paths"], serde_json::json!([""]));
    }
    let dotted = run(&["audit", &root(&dir), "--all", "--paths", "."]);
    assert!(dotted.out.contains("\n  paths: .\n"), "{}", dotted.out);
}

#[test]
fn sarif_is_the_third_format_and_the_two_json_flags_conflict() {
    let dir = repo();
    let first = run(&["audit", &root(&dir), "--all", "--sarif"]);
    assert_eq!(first.code, 0);

    let doc: serde_json::Value = serde_json::from_str(&first.out).unwrap();
    assert_eq!(doc["version"], "2.1.0");
    let results = doc["runs"][0]["results"].as_array().unwrap();
    let mut ids: Vec<&str> = results
        .iter()
        .map(|r| r["ruleId"].as_str().unwrap())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids, ["18", "32"]);
    for r in results {
        for key in [
            "ruleId",
            "level",
            "message",
            "locations",
            "partialFingerprints",
        ] {
            assert!(!r[key].is_null(), "a result has no {key}");
        }
    }
    assert_eq!(
        run(&["audit", &root(&dir), "--all", "--sarif"]).out,
        first.out
    );
    assert_ne!(run(&["audit", &root(&dir), "--json", "--sarif"]).code, 0);
}

#[test]
fn facts_answers_a_symbol_a_module_and_names_the_near_misses() {
    // every section is an accessor's own answer, in the order an agent asks
    let dir = repo();
    let symbol = run(&["facts", &root(&dir), "m.run"]);
    assert_eq!(symbol.code, 0);
    assert!(
        symbol.out.starts_with("m.run  function  m.py L1-5\n"),
        "{}",
        symbol.out
    );
    let heads: Vec<&str> = symbol
        .out
        .lines()
        .skip(1)
        .filter(|l| !l.starts_with(' '))
        .map(|l| l.split(':').next().unwrap_or_default())
        .collect();
    assert_eq!(
        heads,
        [
            "callers prod",
            "callers test",
            "effects",
            "closed world",
            "hot",
            "liveness",
            "findings",
            "fixes",
        ]
    );
    // the finding on this symbol, in rank order
    assert!(symbol.out.contains("  #18 "));
    // a double run is identical byte for byte
    assert_eq!(run(&["facts", &root(&dir), "m.run"]).out, symbol.out);

    let module = run(&["facts", &root(&dir), "m"]);
    assert!(
        module.out.starts_with("m  module  m.py L1-5\n"),
        "{}",
        module.out
    );
    assert!(module.out.contains("  #18 ") && !module.out.contains("callers"));

    let miss = run(&["facts", &root(&dir), "m.ran"]);
    assert_eq!(miss.code, 1);
    assert!(miss.err.contains("nearest: m.run"), "{}", miss.err);
}

#[test]
fn fix_writes_a_diff_and_counts_what_shipped() {
    let dir = repo();
    let out = dir.path().join("patch.diff");
    let done = run(&["fix", &root(&dir), "--out", &out.to_string_lossy()]);

    assert_eq!(done.code, 0);
    assert!(
        done.err.contains("verified finding(s) across"),
        "{}",
        done.err
    );
    // never touches the tree
    assert_eq!(
        std::fs::read_to_string(dir.path().join("m.py")).unwrap(),
        SOURCE
    );
    assert!(out.is_file());
}

#[test]
fn explain_names_the_goal_the_posture_and_the_measured_precision() {
    // explain is the rule vocabulary: the unenforceable goal it approximates,
    // the posture that decides whether it blocks, and the measured precision
    for id in 1..=61u32 {
        let out = run(&["explain", &id.to_string()]);
        assert_eq!(out.code, 0, "#{id}: {}", out.err);
        assert!(
            out.out.contains("precision:") || out.out.contains("retired"),
            "#{id} names neither a sample nor a burial"
        );
    }
    assert!(
        run(&["explain", "35"])
            .out
            .contains("precision: 8/8, 95% interval ")
    );
    // pool 2 on 28 repos: unmeasured
    assert!(run(&["explain", "3"]).out.contains("precision: unmeasured"));
    // a slug answers with the same record, as `--rules` and the roster read it
    assert_eq!(
        run(&["explain", "dead-symbols"]).out,
        run(&["explain", "32"]).out
    );
}

#[test]
fn explain_with_no_id_prints_the_roster_off_the_registry() {
    // the only answer to "what does this check": the registry read straight
    // out, one line per reading, never a second list beside it
    let out = run(&["explain"]);
    assert_eq!(out.code, 0, "{}", out.err);
    let mut lines = out.out.lines();
    assert_eq!(
        lines.next().unwrap().split_whitespace().collect::<Vec<_>>(),
        [
            "id",
            "slug",
            "lang",
            "family",
            "posture",
            "tier",
            "scope",
            "precision",
            "(95%",
            "interval)"
        ]
    );
    // the readings, then the legend that spells the columns' words
    let rows: Vec<&str> = lines
        .take_while(|l| l.trim_start().starts_with('#'))
        .collect();
    assert!(out.out.contains("\nposture: ratchet blocks"));

    let mut want: Vec<String> = sightline_py_rules::RULES
        .iter()
        .map(|r| &r.record)
        .chain(sightline_rs_rules::RULES.iter().map(|r| &r.record))
        .map(|r| format!("#{} {} {}", r.id, r.slug, r.lang))
        .collect();
    want.sort_by_key(|row| row.clone());
    let mut got: Vec<String> = rows
        .iter()
        .map(|l| l.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .collect();
    got.sort_by_key(|row| row.clone());
    assert_eq!(got, want);

    // a judged reading names its sample, an unjudged one says so
    assert!(rows.iter().any(|l| l.contains("#32  dead-symbols")
        && l.contains("ratchet")
        && l.contains("indexed")));
    assert!(rows.iter().any(|l| l.ends_with("unmeasured")));
    // a retired id is not on it
    assert!(!rows.iter().any(|l| l.starts_with(" #25 ")));
}

#[test]
fn explain_answers_every_retired_id_with_its_burial() {
    // a retired id is gone, not unknown: the trail row that cut it, with the
    // rate and the evidence pointer, is the answer a reader of an old report
    // needs
    for id in sightline_core::registry::RETIRED {
        let out = run(&["explain", id]);
        assert_eq!(out.code, 0);
        assert!(out.out.contains(&format!("#{id} retired")), "#{id}");
        assert!(
            out.out.contains("why:") && out.out.contains("evidence:"),
            "#{id} has no burial row"
        );
    }
    let cut = run(&["explain", "25"]).out;
    assert!(cut.contains("rename-delegation") && cut.contains("1 real / 8 fp"));
    assert!(run(&["explain", "28"]).out.contains("recall 0/21"));
}

#[test]
fn explain_refuses_an_id_the_numbering_never_reached() {
    let out = run(&["explain", "99"]);
    assert_eq!(out.code, 1);
    assert!(
        out.err.starts_with("unknown rule '99'; known: "),
        "{}",
        out.err
    );
}
