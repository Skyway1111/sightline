//! The `--json` output: the flat ranked list under the provenance header,
//! written through `pyjson`.

use serde_json::{Map, Value};

use crate::findings::{Evidence, Finding, Fix, precision};
use crate::pyjson;
use crate::registry::Registry;

use super::{AuditResult, provenance, span};

/// What the finding rests on, for the agent reading the JSON: a detail
/// string or the WP premises. Omitted entirely when the evidence holds
/// neither (the engine field already names the machinery).
fn evidence_json(f: &Finding) -> Map<String, Value> {
    let mut out = Map::new();
    match &f.evidence {
        Evidence::Ast { detail } | Evidence::Idx { detail } if !detail.is_empty() => {
            out.insert("detail".into(), Value::from(detail.as_str()));
        }
        Evidence::Wp { premises } if !premises.is_empty() => {
            out.insert("premises".into(), Value::from(premises.as_slice()));
        }
        _ => {}
    }
    out
}

/// The verified patch, applicable from the JSON alone: spans are
/// `edits::apply_edits`' encoding (1-based line, `[col_start, col_end)`),
/// `imports` the statements the file must gain.
fn fix_json(fix: &Fix) -> Value {
    let edits: Vec<Value> = fix
        .edits
        .iter()
        .map(|e| {
            serde_json::json!({
                "line": e.line, "col_start": e.col_start,
                "col_end": e.col_end, "text": e.text,
            })
        })
        .collect();
    serde_json::json!({ "file": &*fix.rel, "edits": edits, "imports": fix.imports })
}

fn finding_json(result: &AuditResult, f: &Finding, registry: &Registry) -> Value {
    let mut out = Map::new();
    out.insert("rule".into(), Value::from(f.rule));
    out.insert(
        "slug".into(),
        Value::from(registry.by_id(f.rule).map_or("", |r| r.slug)),
    );
    out.insert("tier".into(), Value::from(f.tier().value()));
    out.insert("engine".into(), Value::from(f.engine().value()));
    out.insert("file".into(), Value::from(&*f.site.rel));
    out.insert("line".into(), Value::from(f.site.line));
    out.insert("col".into(), Value::from(f.site.col));
    out.insert("symbol".into(), Value::from(&*f.site.symbol));
    out.insert("span".into(), Value::from(span(result, f).as_slice()));
    out.insert("message".into(), Value::from(f.message.as_str()));
    out.insert("cause".into(), Value::from(f.cause.as_str()));
    out.insert("salience".into(), Value::from(f.salience));
    // named where it is not the Python default, as the header names the
    // stacks: the sheets key a Rust finding `rs:<id>` off this
    if f.lang != "py" {
        out.insert("lang".into(), Value::from(f.lang));
    }
    if let Some(p) = precision(f) {
        out.insert("precision".into(), p.into_iter().collect());
    }
    let evidence = evidence_json(f);
    if !evidence.is_empty() {
        out.insert("evidence".into(), evidence.into());
    }
    if let Some(fix) = &f.fix {
        out.insert("fix".into(), fix_json(fix));
    }
    out.into()
}

pub fn to_json(result: &AuditResult, registry: &Registry) -> String {
    let findings: Vec<Value> = result
        .findings
        .iter()
        .map(|f| finding_json(result, f, registry))
        .collect();
    let payload = serde_json::json!({
        "provenance": provenance(result), "findings": findings,
    });
    pyjson::dumps(&payload) + "\n"
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    use crate::findings::tests::{ast, finding, idx};
    use crate::findings::{Finding, Site, SpanEdit};
    use crate::lang::Stack;
    use crate::testing::{P, SyntheticStack, registry};

    fn by_cause(text: &str) -> HashMap<String, Value> {
        let doc: Value = serde_json::from_str(text).unwrap();
        doc["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (f["cause"].as_str().unwrap().to_string(), f.clone()))
            .collect()
    }

    fn at(rule: &'static str, line: u32, symbol: &str, cause: &str, evidence: Evidence) -> Finding {
        Finding {
            site: Site {
                rel: "a.p".into(),
                line,
                col: 0,
                symbol: symbol.into(),
            },
            cause: cause.into(),
            ..finding(rule, evidence)
        }
    }

    #[test]
    fn findings_carry_the_premise_they_rest_on() {
        // a detail string or the WP premises, and nothing
        // where the evidence holds neither
        let stack = SyntheticStack::new(&P, &[("a.p", "one\ntwo\n")]);
        let findings = vec![
            at(
                "11",
                1,
                "p::a::main",
                "clone:k",
                Evidence::Idx { detail: "k".into() },
            ),
            at(
                "6",
                1,
                "p::a::main",
                "dishonest-accessor",
                Evidence::Wp {
                    premises: vec!["io".into(), "mutates-arg".into()],
                },
            ),
            at("1", 1, "p::a::main", "weak:return", ast()),
        ];
        let out = by_cause(&to_json(
            &AuditResult::new(findings, stack.neutral()),
            &registry(),
        ));
        assert_eq!(
            out["clone:k"]["evidence"],
            serde_json::json!({"detail": "k"})
        );
        assert_eq!(
            out["dishonest-accessor"]["evidence"],
            serde_json::json!({"premises": ["io", "mutates-arg"]})
        );
        assert!(out["weak:return"].get("evidence").is_none());
    }

    #[test]
    fn a_row_carries_the_symbol_span_and_the_verified_patch() {
        // what an agent needs beyond the location: the exact lines to load,
        // and the patch #5/#10 already verified at audit time
        let stack = SyntheticStack::new(&P, &[("a.p", "def f\nbody\n\n\nX\n")]);
        let fix = Fix {
            rel: "a.p".into(),
            edits: vec![SpanEdit {
                line: 1,
                col_start: 8,
                col_end: 8,
                text: ": int".into(),
            }],
            imports: vec!["import x".into()],
        };
        let findings = vec![
            at("11", 2, "p::a::main", "clone:k", idx()),
            at("11", 5, "p::a", "clone:m", idx()),
            Finding {
                fix: Some(fix),
                ..at("11", 1, "p::a::main", "clone:p", idx())
            },
        ];
        let out = by_cause(&to_json(
            &AuditResult::new(findings, stack.neutral()),
            &registry(),
        ));
        assert_eq!(out["clone:k"]["span"], serde_json::json!([1, 6]));
        assert_eq!(out["clone:m"]["span"], serde_json::json!([5, 5]));
        assert!(out["clone:k"].get("fix").is_none());
        assert_eq!(
            out["clone:p"]["fix"],
            serde_json::json!({
                "file": "a.p",
                "edits": [{"line": 1, "col_start": 8, "col_end": 8, "text": ": int"}],
                "imports": ["import x"],
            })
        );
    }

    #[test]
    fn a_reading_names_its_language_only_where_it_is_not_python() {
        let stack = SyntheticStack::new(&P, &[("a.p", "x\n")]);
        let findings = vec![
            at("11", 1, "p::a::main", "clone:k", idx()),
            Finding {
                lang: "rs",
                ..at("11", 1, "p::a::main", "clone:j", idx())
            },
        ];
        let out = by_cause(&to_json(
            &AuditResult::new(findings, stack.neutral()),
            &registry(),
        ));
        assert!(out["clone:k"].get("lang").is_none());
        assert_eq!(out["clone:j"]["lang"], "rs");
        // one id, two readings: `by_id` answers with the first
        assert_eq!(out["clone:j"]["slug"], "structural-clones");
    }

    #[test]
    fn the_measured_sample_rides_along_where_a_round_judged_one() {
        let stack = SyntheticStack::new(&P, &[("a.p", "x\n")]);
        let findings = vec![
            at("11", 1, "p::a::main", "clone:k", idx()),
            at("42", 1, "p::a::main", "assertion-free:t", idx()),
            at("3", 1, "p::a::main", "unjudged", ast()),
        ];
        let out = by_cause(&to_json(
            &AuditResult::new(findings, stack.neutral()),
            &registry(),
        ));
        assert_eq!(out["clone:k"]["precision"]["tp"], 70);
        assert_eq!(out["assertion-free:t"]["precision"]["n"], 6);
        assert!(out["unjudged"].get("precision").is_none());
    }

    #[test]
    fn the_header_counts_the_real_total_and_the_document_ends_in_a_newline() {
        let stack = SyntheticStack::new(&P, &[("a.p", "x\n")]);
        let findings = (1..=3)
            .map(|i| at("11", i, "p::a::main", "clone", idx()))
            .collect();
        let mut result = AuditResult::new(findings, stack.neutral());
        result.suppressed = 4;
        let text = to_json(&result, &registry());
        assert!(text.ends_with("}\n"));
        let doc: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(doc["provenance"]["counts"]["findings"], 3);
        assert_eq!(doc["provenance"]["counts"]["indexed"], 3);
        assert_eq!(doc["provenance"]["counts"]["suppressed"], 4);
    }
}
