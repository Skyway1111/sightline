//! Output: ranked text, JSON and SARIF, each with the provenance header.
//! Deterministic by construction: no timestamps, no temp paths.

pub mod json;
pub mod sarif;
pub mod text;

pub use json::to_json;
pub use sarif::{SARIF_SCHEMA, to_sarif};
pub use text::to_text;

use serde_json::{Map, Value};

use crate::findings::{Finding, Tier};
use crate::lang::FactsView;

/// The version the header prints.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AuditResult<'a> {
    /// ranked, post-pipeline
    pub findings: Vec<Finding>,
    pub suppressed: usize,
    /// baseline-diff absorptions
    pub absorbed: usize,
    /// degraded or disabled provers (banned-shortcut: must show)
    pub notes: Vec<String>,
    pub facts: &'a dyn FactsView,
    /// what each language's prover machinery reports for the header, merged
    pub provers: Map<String, Value>,
    pub rules_off: Vec<String>,
    /// `--rules`: the ids run
    pub rules_only: Vec<String>,
    /// `--paths`: rel prefixes kept
    pub paths: Vec<String>,
}

impl<'a> AuditResult<'a> {
    /// A result with no baseline, no restriction and no prover header: what
    /// a test and a replay both start from.
    // sightline-ok: 56 - the fixture constructor every render and rules test shares
    pub fn new(findings: Vec<Finding>, facts: &'a dyn FactsView) -> AuditResult<'a> {
        AuditResult {
            findings,
            suppressed: 0,
            absorbed: 0,
            notes: Vec::new(),
            facts,
            provers: Map::new(),
            rules_off: Vec::new(),
            rules_only: Vec::new(),
            paths: Vec::new(),
        }
    }
}

fn sorted(values: &[String]) -> Vec<Value> {
    let mut out: Vec<&String> = values.iter().collect();
    out.sort();
    out.into_iter().map(|s| Value::from(s.as_str())).collect()
}

pub fn provenance(result: &AuditResult) -> Map<String, Value> {
    let langs = result.facts.languages();
    let mut only: Vec<&String> = result.rules_only.iter().collect();
    only.sort_by_key(|s| s.parse::<i64>().unwrap_or(0));

    let mut out = Map::new();
    out.insert("sightline".into(), Value::from(VERSION));
    out.insert("modules".into(), Value::from(result.facts.modules().len()));
    out.insert("parse_errors".into(), sorted(result.facts.errors()).into());
    // the stacks that ran, named where they are not the Python default
    if langs != ["py"] {
        out.insert("languages".into(), Value::from(langs));
    }
    out.insert("rules_off".into(), sorted(&result.rules_off).into());
    let only: Vec<Value> = only.into_iter().map(|s| Value::from(s.as_str())).collect();
    out.insert("rules_only".into(), only.into());
    out.insert("paths".into(), sorted(&result.paths).into());
    out.insert("notes".into(), sorted(&result.notes).into());
    out.extend(result.provers.iter().map(|(k, v)| (k.clone(), v.clone())));

    let mut counts = Map::new();
    counts.insert("findings".into(), Value::from(result.findings.len()));
    for tier in [Tier::Proved, Tier::Indexed, Tier::Heuristic] {
        let n = result.findings.iter().filter(|f| f.tier() == tier).count();
        counts.insert(tier.value().into(), Value::from(n));
    }
    counts.insert("suppressed".into(), Value::from(result.suppressed));
    counts.insert("baselined".into(), Value::from(result.absorbed));
    out.insert("counts".into(), counts.into());
    out
}

/// `[first, last]` def lines of the enclosing symbol: the exact slice a
/// reader loads. A module-level site is its own line.
fn span(result: &AuditResult, f: &Finding) -> [u32; 2] {
    match result.facts.symbols().get(&*f.site.symbol) {
        Some(s) if s.end_lineno != 0 => [s.lineno, s.end_lineno],
        _ => [f.site.line, f.site.line],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::findings::tests::{ast, finding};
    use crate::findings::{Finding, Site};
    use crate::lang::Stack;
    use crate::testing::{P, SyntheticStack};

    #[test]
    fn provenance_names_the_stacks_only_where_they_are_not_python() {
        let stack = SyntheticStack::new(&P, &[("m.p", "x = 1\n")]);
        let mut result = AuditResult::new(Vec::new(), stack.neutral());
        result.notes.push("b".into());
        result.notes.push("a".into());
        let prov = provenance(&result);
        assert_eq!(prov["languages"], serde_json::json!(["p"]));
        assert_eq!(prov["notes"], serde_json::json!(["a", "b"]));
        assert_eq!(prov["modules"], 1);
        assert_eq!(prov["counts"]["findings"], 0);
    }

    #[test]
    fn rules_only_sorts_by_the_id_as_a_number() {
        let stack = SyntheticStack::new(&P, &[("m.p", "x\n")]);
        let mut result = AuditResult::new(Vec::new(), stack.neutral());
        result.rules_only = vec!["32".into(), "5".into(), "11".into()];
        result.rules_off = vec!["32".into(), "5".into(), "11".into()];
        let prov = provenance(&result);
        assert_eq!(prov["rules_only"], serde_json::json!(["5", "11", "32"]));
        // `rules_off` is a plain string sort
        assert_eq!(prov["rules_off"], serde_json::json!(["11", "32", "5"]));
    }

    #[test]
    fn the_span_is_the_enclosing_symbols_or_the_site_itself() {
        let stack = SyntheticStack::new(&P, &[("m.p", "a\nb\nc\n")]);
        let at = |symbol: &str, line: u32| Finding {
            site: Site {
                rel: "m.p".into(),
                line,
                col: 0,
                symbol: symbol.into(),
            },
            ..finding("11", ast())
        };
        let result = AuditResult::new(Vec::new(), stack.neutral());
        assert_eq!(span(&result, &at("p::m::main", 2)), [1, 4]);
        assert_eq!(span(&result, &at("p::m", 2)), [2, 2]);
    }
}
