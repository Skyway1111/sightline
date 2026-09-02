//! SARIF 2.1.0, one run: the registry as the driver's rules, `level` by
//! posture (the axis the gate blocks on), cause as the stable fingerprint.
//! GitHub annotations are a SARIF upload, so there is no second format.
//! Linear in the findings, no I/O.
//!
//! An alert's rule pane is `help.markdown`, built from the record `explain`
//! prints, so the two answers cannot drift.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Map, Value};

use crate::findings::{Finding, Fix, precision};
use crate::precision::{rule_recall, rule_samples};
use crate::pyjson;
use crate::registry::Registry;
use crate::rule::{Posture, RuleRecord};

use super::{AuditResult, VERSION, provenance};

pub const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

/// The project, as the manifest names it: the driver's `informationUri`.
pub const HOME: &str = env!("CARGO_PKG_REPOSITORY");

/// Where an alert's rule link lands. There is no page per rule, and a
/// generated one would be a second rule list: the record itself rides in
/// `help.markdown`, and the link goes to the reference, which names the
/// postures, the suppression marker and the exit codes.
const HELP_URI: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/blob/main/docs/reference.md");

pub fn sarif_level(posture: Posture) -> &'static str {
    match posture {
        Posture::Gate => "error",
        Posture::Ratchet => "warning",
        Posture::Report => "note",
    }
}

/// `urllib.parse.quote(s, safe="/")`: the always-safe run is
/// `A-Za-z0-9_.-~`, and `/` is the one character the call adds.
const QUOTE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'_')
    .remove(b'.')
    .remove(b'-')
    .remove(b'~')
    .remove(b'/');

fn uri(rel: &str) -> Value {
    Value::from(utf8_percent_encode(rel, QUOTE).to_string())
}

/// The same patch in SARIF's shape (1-based columns). Emitted only for a fix
/// needing no import transport: `replacements` have no home for the
/// statements a patch must add, and a patch missing them is a broken one.
fn sarif_fix(fix: &Fix) -> Value {
    let replacements: Vec<Value> = fix
        .edits
        .iter()
        .map(|e| {
            serde_json::json!({
                "deletedRegion": {
                    "startLine": e.line, "startColumn": e.col_start + 1,
                    "endColumn": e.col_end + 1,
                },
                "insertedContent": {"text": e.text},
            })
        })
        .collect();
    serde_json::json!([{"artifactChanges": [{
        "artifactLocation": {"uri": uri(&fix.rel), "uriBaseId": "%SRCROOT%"},
        "replacements": replacements,
    }]}])
}

/// The rule pane of an alert: the record `explain` prints, as markdown, so
/// a reader who cannot run the binary sees the same answer.
fn help_markdown(r: &RuleRecord) -> String {
    let mut out = format!(
        "`#{}` **{}** - {}, family {}, {}, {}\n\n{}\n\nGoal: {}\n",
        r.id,
        r.slug,
        r.lang,
        r.family,
        r.engine_class,
        r.posture.value(),
        r.meaning,
        r.goal,
    );
    if !r.complement.is_empty() {
        out.push_str(&format!("\nComplement: {}\n", r.complement));
    }
    let samples = rule_samples(r.id, r.lang);
    if samples.is_empty() {
        out.push_str("\nPrecision: unmeasured, no round has judged this reading\n");
    }
    for (arm, sample) in samples {
        let label = if arm.is_empty() {
            String::new()
        } else {
            format!(" ({arm} arm)")
        };
        out.push_str(&format!(
            "\nPrecision{label}: {}/{} seed {} - {}\n",
            sample.tp, sample.n, sample.seed, sample.of,
        ));
    }
    if let Some(recall) = rule_recall(r.id, r.lang) {
        out.push_str(&format!(
            "\nRecall: {}/{} - {}\n",
            recall.covered, recall.sites, recall.of,
        ));
    }
    out.push_str(&format!("\n`sightline explain {}`\n", r.id));
    out
}

/// One record per id, in registry order: `RULE_BY_ID`'s reading, the one
/// every consumer of a slug or a meaning means.
fn by_id(registry: &Registry) -> Vec<&RuleRecord> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for r in &registry.rules {
        if !seen.contains(&r.id) {
            seen.push(r.id);
            out.push(r);
        }
    }
    out
}

pub fn to_sarif(result: &AuditResult, registry: &Registry) -> String {
    let records = by_id(registry);
    let rules: Vec<Value> = records
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.slug,
                "shortDescription": {"text": r.meaning},
                "fullDescription": {"text": r.goal},
                "help": {
                    "text": format!("{} Goal: {}", r.meaning, r.goal),
                    "markdown": help_markdown(r),
                },
                "helpUri": HELP_URI,
                "defaultConfiguration": {"level": sarif_level(r.posture)},
                "properties": {"family": r.family, "posture": r.posture.value()},
            })
        })
        .collect();
    let driver = serde_json::json!({
        "name": "sightline", "version": VERSION,
        "informationUri": HOME, "rules": rules,
    });

    let results: Vec<Value> = result
        .findings
        .iter()
        .map(|f| finding_result(f, registry, &records))
        .collect();
    let payload = serde_json::json!({
        "$schema": SARIF_SCHEMA,
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": driver},
            "results": results,
            "properties": provenance(result),
        }],
    });
    pyjson::dumps(&payload) + "\n"
}

fn finding_result(f: &Finding, registry: &Registry, records: &[&crate::rule::RuleRecord]) -> Value {
    let index = records.iter().position(|r| r.id == f.rule);
    let level = registry
        .posture_of(f.rule, f.lang)
        .map(sarif_level)
        .unwrap_or("note");
    let mut properties = Map::new();
    properties.insert("tier".into(), Value::from(f.tier().value()));
    properties.insert("engine".into(), Value::from(f.engine().value()));
    if let Some(p) = precision(f) {
        properties.insert("precision".into(), p.into_iter().collect());
    }

    let mut out = Map::new();
    out.insert("ruleId".into(), Value::from(f.rule));
    out.insert("ruleIndex".into(), Value::from(index));
    out.insert("level".into(), Value::from(level));
    out.insert("message".into(), serde_json::json!({"text": f.message}));
    out.insert(
        "locations".into(),
        serde_json::json!([{
            "physicalLocation": {
                "artifactLocation": {"uri": uri(&f.site.rel), "uriBaseId": "%SRCROOT%"},
                "region": {"startLine": f.site.line, "startColumn": f.site.col + 1},
            },
            "logicalLocations": [{"fullyQualifiedName": &*f.site.symbol}],
        }]),
    );
    out.insert(
        "partialFingerprints".into(),
        serde_json::json!({"cause": f.cause}),
    );
    if let Some(fix) = &f.fix
        && fix.imports.is_empty()
    {
        out.insert("fixes".into(), sarif_fix(fix));
    }
    out.insert("properties".into(), properties.into());
    out.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::findings::tests::{ast, finding, idx};
    use crate::findings::{Evidence, Finding, Site, SpanEdit};
    use crate::lang::Stack;
    use crate::testing::{P, SyntheticStack, registry};

    fn at(rule: &'static str, rel: &str, cause: &str, evidence: Evidence) -> Finding {
        Finding {
            site: Site {
                rel: rel.into(),
                line: 2,
                col: 4,
                symbol: "p::a::main".into(),
            },
            cause: cause.into(),
            ..finding(rule, evidence)
        }
    }

    #[test]
    fn sarif_is_the_ranked_list_with_level_by_posture() {
        // one driver rule per id, level by the reading's
        // own posture, and a uri that is a URI
        let stack = SyntheticStack::new(&P, &[("a.p", "x\ny\n")]);
        let findings = vec![
            at("99", "a.p", "gate-fixture:a", ast()),
            at("11", "a.p", "clone:k", idx()),
            at(
                "41",
                "a.p",
                "perf:filter-scan",
                Evidence::Wp {
                    premises: vec!["hot-reachable".into()],
                },
            ),
            at("11", "my dir/b.p", "clone:j", idx()),
        ];
        let reg = registry();
        let result = AuditResult::new(findings, stack.neutral());
        let text = to_sarif(&result, &reg);
        let doc: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(doc["version"], "2.1.0");
        assert!(
            doc["$schema"]
                .as_str()
                .unwrap()
                .ends_with("sarif-2.1.0.json")
        );
        let run = &doc["runs"][0];
        let rules = run["tool"]["driver"]["rules"].as_array().unwrap();
        let ids: Vec<&str> = rules.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["1", "3", "6", "11", "41", "42", "99"]);
        let results = run["results"].as_array().unwrap();
        let levels: Vec<&str> = results
            .iter()
            .map(|r| r["level"].as_str().unwrap())
            .collect();
        assert_eq!(levels, ["error", "warning", "note", "warning"]);
        let indexed: Vec<&str> = results
            .iter()
            .map(|r| {
                rules[r["ruleIndex"].as_u64().unwrap() as usize]["id"]
                    .as_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(indexed, ["99", "11", "41", "11"]);
        let loc = &results[1]["locations"][0]["physicalLocation"];
        assert_eq!(loc["artifactLocation"]["uri"], "a.p");
        assert_eq!(
            results[3]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "my%20dir/b.p"
        );
        // SARIF columns are 1-based
        assert_eq!(
            loc["region"],
            serde_json::json!({"startLine": 2, "startColumn": 5})
        );
        assert_eq!(
            results[1]["partialFingerprints"],
            serde_json::json!({"cause": "clone:k"})
        );
        assert_eq!(results[1]["properties"]["tier"], "indexed");
        assert_eq!(results[1]["properties"]["precision"]["tp"], 70);
        // the provenance header rides along
        assert_eq!(run["properties"]["counts"]["findings"], 4);
        assert_eq!(text, to_sarif(&result, &reg));
    }

    /// What a GitHub alert links back to: the driver names the project, and
    /// every rule holds the record `explain` prints plus the page that tells
    /// a reader what to do with it.
    #[test]
    fn the_driver_and_every_rule_link_back() {
        let stack = SyntheticStack::new(&P, &[("a.p", "x\n")]);
        let doc: Value = serde_json::from_str(&to_sarif(
            &AuditResult::new(Vec::new(), stack.neutral()),
            &registry(),
        ))
        .unwrap();
        let driver = &doc["runs"][0]["tool"]["driver"];
        assert_eq!(driver["informationUri"], HOME);
        assert!(HOME.starts_with("https://"), "{HOME}");

        let rules = driver["rules"].as_array().unwrap();
        assert!(rules.iter().all(|r| r["helpUri"] == HELP_URI));
        assert!(HELP_URI.starts_with(HOME));

        let eleven = rules.iter().find(|r| r["id"] == "11").unwrap();
        let markdown = eleven["help"]["markdown"].as_str().unwrap();
        assert!(markdown.starts_with("`#11` **structural-clones** - py, family B, AST, ratchet\n"));
        assert!(markdown.contains("\nGoal: the goal it approximates\n"));
        // the judged rows `explain` prints, its arms named
        assert!(markdown.contains("\nPrecision: 232/256 seed "));
        assert!(markdown.contains("\nPrecision (clone arm): 70/83 seed "));
        assert!(markdown.ends_with("\n`sightline explain 11`\n"));
        // a plain-text `help` is what the format asks of every rule
        assert!(
            rules
                .iter()
                .all(|r| !r["help"]["text"].as_str().unwrap_or_default().is_empty())
        );

        // a rule no round judged says so rather than going silent
        let three = rules.iter().find(|r| r["id"] == "3").unwrap();
        assert!(
            three["help"]["markdown"]
                .as_str()
                .unwrap()
                .contains("\nPrecision: unmeasured, ")
        );
    }

    #[test]
    fn a_rust_reading_blocks_on_its_own_posture() {
        // #11 ratchets for py and reports for rs: the finding's own reading
        // decides, not the record `by_id` answers with
        let stack = SyntheticStack::new(&P, &[("a.p", "x\n")]);
        let findings = vec![Finding {
            lang: "rs",
            ..at("11", "a.p", "clone:k", idx())
        }];
        let doc: Value = serde_json::from_str(&to_sarif(
            &AuditResult::new(findings, stack.neutral()),
            &registry(),
        ))
        .unwrap();
        assert_eq!(doc["runs"][0]["results"][0]["level"], "note");
        // the driver still names the reading `by_id` answers with
        let rules = doc["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        let eleven = rules.iter().find(|r| r["id"] == "11").unwrap();
        assert_eq!(eleven["defaultConfiguration"]["level"], "warning");
        assert_eq!(eleven["properties"]["posture"], "ratchet");
    }

    #[test]
    fn a_patch_needing_an_import_stays_out_of_the_replacements() {
        // `replacements` have no home for an import transport, so that fix
        // is JSON-only; an import-free one rides along
        let stack = SyntheticStack::new(&P, &[("a.p", "x\n")]);
        let edits = vec![SpanEdit {
            line: 1,
            col_start: 8,
            col_end: 8,
            text: ": int".into(),
        }];
        let transported = Fix {
            rel: "a.p".into(),
            edits: edits.clone(),
            imports: vec!["import x".into()],
        };
        let plain = Fix {
            rel: "a.p".into(),
            edits,
            imports: Vec::new(),
        };
        let render = |fix: Fix| -> Value {
            let findings = vec![Finding {
                fix: Some(fix),
                ..at("11", "a.p", "clone:p", idx())
            }];
            serde_json::from_str(&to_sarif(
                &AuditResult::new(findings, stack.neutral()),
                &registry(),
            ))
            .unwrap()
        };
        assert!(
            render(transported)["runs"][0]["results"][0]
                .get("fixes")
                .is_none()
        );
        let doc = render(plain);
        let change = &doc["runs"][0]["results"][0]["fixes"][0]["artifactChanges"][0];
        assert_eq!(change["artifactLocation"]["uri"], "a.p");
        assert_eq!(
            change["replacements"][0],
            serde_json::json!({
                // SARIF columns are 1-based
                "deletedRegion": {"startLine": 1, "startColumn": 9, "endColumn": 9},
                "insertedContent": {"text": ": int"},
            })
        );
    }
}
