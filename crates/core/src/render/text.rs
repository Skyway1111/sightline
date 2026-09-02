//! The text rollup: the ranked list grouped by module, then by symbol.
//!
//! Each group sits where its strongest finding ranks, prod modules before
//! test modules. The unit of work is a module or a symbol, and everything
//! stacked on it reads in one place. The JSON is the flat ranked list.

use indexmap::IndexMap;

use crate::findings::{Finding, Rel};
use crate::lang::NeutralModule;

use super::{AuditResult, provenance};

/// `text` with every spelling of the module's own qualified prefix dropped:
/// the module header already named it, and a message that repeats it per
/// symbol runs past the terminal. Both separators, since a Rust path spells
/// `::` where a Python one spells `.`.
fn short(text: &str, module: &NeutralModule) -> String {
    text.replace(&format!("{}.", module.qname), "")
        .replace(&format!("{}::", module.qname), "")
}

fn symbol_label(result: &AuditResult, module: Option<&NeutralModule>, qname: &str) -> String {
    let Some(module) = module else {
        return "(module scope)".to_string();
    };
    if qname == &*module.qname || qname == &*module.rel {
        return "(module scope)".to_string();
    }
    let name = short(qname, module);
    match result.facts.symbols().get(qname) {
        Some(s) if s.end_lineno != 0 => {
            let lines = s.end_lineno - s.lineno + 1;
            format!("{name}  L{}-{} ({lines} lines)", s.lineno, s.end_lineno)
        }
        _ => name,
    }
}

/// One module: header with a per-rule tally, then symbols in the order
/// their strongest finding ranks, each with its findings in rank order.
#[allow(
    clippy::indexing_slicing,
    reason = "a block is built for a file that holds a finding"
)]
fn module_block(
    result: &AuditResult,
    module: Option<&NeutralModule>,
    size: &str,
    found: &[&Finding],
) -> Vec<String> {
    let mut counts: IndexMap<&str, usize> = IndexMap::new();
    for f in found {
        *counts.entry(f.rule).or_default() += 1;
    }
    let mut tally: Vec<(&str, usize)> = counts.into_iter().collect();
    tally.sort_by_key(|&(rule, n)| (std::cmp::Reverse(n), rule.parse::<i64>().unwrap_or(0)));
    let tally = tally
        .iter()
        .map(|(rule, n)| format!("#{rule} x{n}"))
        .collect::<Vec<_>>()
        .join(", ");

    let mut lines = vec![
        String::new(),
        format!(
            "{}  {size} | {} findings: {tally}",
            found[0].site.rel,
            found.len()
        ),
    ];
    // `found` is in rank order, so insertion order is the order the
    // symbols' strongest findings rank
    let mut by_symbol: IndexMap<&str, Vec<&Finding>> = IndexMap::new();
    for f in found {
        by_symbol.entry(&f.site.symbol).or_default().push(f);
    }
    for (qname, sites) in by_symbol {
        lines.push(format!("  {}", symbol_label(result, module, qname)));
        lines.extend(sites.iter().map(|f| {
            format!(
                "    {}:{:<4} {:<9} #{:<3} {}",
                f.site.line,
                f.site.col,
                f.tier().value(),
                f.rule,
                module.map_or_else(|| f.message.clone(), |m| short(&f.message, m))
            )
        }));
    }
    lines
}

#[must_use]
#[allow(
    clippy::indexing_slicing,
    reason = "the provenance keys `provenance` writes"
)]
pub fn to_text(result: &AuditResult) -> String {
    let prov = provenance(result);
    let counts = &prov["counts"];
    let n = |key: &str| counts[key].as_u64().unwrap_or(0);
    let list = |key: &str| -> Vec<&str> {
        prov[key]
            .as_array()
            .map(|xs| xs.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_default()
    };

    // `--top`: the count reads `shown of all`, so a cut is never mistaken
    // for a clean tree
    let findings = match n("cut") {
        0 => n("findings").to_string(),
        cut => format!("{} of {}", n("findings"), n("findings") + cut),
    };
    let mut lines = vec![format!(
        "sightline {} | modules {} | findings {findings} (proved {} / indexed {} / \
         heuristic {}) | suppressed {} | baselined {}",
        prov["sightline"].as_str().unwrap_or(""),
        prov["modules"].as_u64().unwrap_or(0),
        n("proved"),
        n("indexed"),
        n("heuristic"),
        n("suppressed"),
        n("baselined"),
    )];
    if let Some(langs) = prov.get("languages") {
        let langs: Vec<&str> = langs
            .as_array()
            .map(|xs| xs.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_default();
        lines.push(format!("  languages: {}", langs.join(", ")));
    }
    let only = list("rules_only");
    if !only.is_empty() {
        let spelled: Vec<String> = only.iter().map(|r| format!("#{r}")).collect();
        lines.push(format!("  rules: {}", spelled.join(", ")));
    }
    let paths = list("paths");
    if !paths.is_empty() {
        let spelled: Vec<&str> = paths
            .iter()
            .map(|p| if p.is_empty() { "." } else { p })
            .collect();
        lines.push(format!("  paths: {}", spelled.join(", ")));
    }
    lines.extend(list("notes").iter().map(|note| format!("  note: {note}")));
    lines.extend(
        list("parse_errors")
            .iter()
            .map(|err| format!("  parse error: {err}")),
    );

    let facts = result.facts;
    let mut by_file: IndexMap<&Rel, Vec<&Finding>> = IndexMap::new();
    for f in &result.findings {
        by_file.entry(&f.site.rel).or_default().push(f);
    }
    let module_of = |rel: &str| -> Option<&NeutralModule> {
        facts
            .module_by_rel()
            .get(rel)
            .and_then(|q| facts.modules().get(q))
    };

    // insertion order is the order each file's strongest finding ranks;
    // the stable sort only moves the test files after the rest
    let mut files: Vec<(&Rel, Vec<&Finding>)> = by_file.into_iter().collect();
    files.sort_by_key(|(rel, _)| facts.is_test(rel));

    let mut in_tests = false;
    for (rel, found) in files {
        if facts.is_test(rel) && !in_tests {
            in_tests = true;
            lines.push(String::new());
            lines.push("tests:".to_string());
        }
        let module = module_of(rel);
        let size = module.map_or_else(
            || "doc".to_string(),
            |m| {
                format!(
                    "{} lines, fan-in {}",
                    m.lines.len(),
                    facts.fan_in().get(&m.qname).copied().unwrap_or(0)
                )
            },
        );
        lines.extend(module_block(result, module, &size, &found));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::findings::tests::{finding, idx};
    use crate::findings::{Finding, Site};
    use crate::lang::Stack;
    use crate::testing::{P, SyntheticStack};

    fn at(rel: &str, line: u32, cause: &str, symbol: &str) -> Finding {
        Finding {
            site: Site {
                rel: rel.into(),
                line,
                col: 0,
                symbol: symbol.into(),
            },
            message: "structural clone x3: p::a.fn, p::b.fn, p::c.fn".into(),
            cause: cause.into(),
            ..finding("11", idx())
        }
    }

    /// The mini repo these tests render, in the synthetic language: `a.p`
    /// is the longest, `t_z.p` is the one test module.
    fn stack() -> SyntheticStack {
        let mut stack = SyntheticStack::new(
            &P,
            &[
                ("a.p", "def fn\n    pass\n\n\ndef other\n    pass\n"),
                ("b.p", "def fn\n    pass\n"),
                ("c.p", "import a\nimport b\n\nb.fn()\n"),
                ("t_z.p", "def fn\n    pass\n"),
            ],
        );
        let neutral = stack.neutral_mut();
        neutral.fan_in.insert("p::b".into(), 1);
        neutral.symbols.insert(
            "p::a.other".into(),
            crate::lang::NeutralSymbol {
                module: "p::a".into(),
                lineno: 5,
                end_lineno: 6,
                kind: "function",
            },
        );
        neutral.symbols.insert(
            "p::a.fn".into(),
            crate::lang::NeutralSymbol {
                module: "p::a".into(),
                lineno: 1,
                end_lineno: 2,
                kind: "function",
            },
        );
        stack
    }

    #[test]
    fn the_rollup_orders_modules_then_symbols_by_rank_and_puts_tests_last() {
        let stack = stack();
        // the input is the ranked list: `t_z.p` leads it and `a.p` holds the
        // most findings, and neither moves a module ahead of the one whose
        // strongest finding ranks first
        let mut findings: Vec<Finding> = (0..4)
            .map(|i| at("t_z.p", 1, &format!("clone:t{i}"), "p::t_z.fn"))
            .collect();
        findings.extend([
            at("b.p", 1, "clone:k", "p::b.fn"),
            at("a.p", 5, "clone:other", "p::a.other"),
            at("c.p", 4, "clone:k", "p::c"),
            at("a.p", 1, "clone:k", "p::a.fn"),
            at("a.p", 6, "clone:third", "p::a.other"),
        ]);
        let text = to_text(&AuditResult::new(findings, stack.neutral()));

        let heads: Vec<&str> = text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with(' ') && !l.starts_with("sightline"))
            .collect();
        assert_eq!(
            heads,
            [
                "b.p  3 lines, fan-in 1 | 1 findings: #11 x1",
                "a.p  7 lines, fan-in 0 | 3 findings: #11 x3",
                "c.p  5 lines, fan-in 0 | 1 findings: #11 x1",
                "tests:",
                "t_z.p  3 lines, fan-in 0 | 4 findings: #11 x4",
            ]
        );
        let block: Vec<&str> = text
            .split("a.p  7 lines")
            .nth(1)
            .unwrap()
            .split("\nc.p")
            .next()
            .unwrap()
            .lines()
            .collect();
        // the symbol whose finding ranks first leads, whatever its line
        assert_eq!(block[1], "  other  L5-6 (2 lines)");
        assert_eq!(block[4], "  fn  L1-2 (2 lines)");
        // the message drops the module's own prefix and keeps the others'
        assert_eq!(
            block[5],
            "    1:0    indexed   #11  structural clone x3: fn, p::b.fn, p::c.fn"
        );
        // provenance counts the real total
        assert!(text.contains("findings 9"));
        assert!(
            text.split("\nc.p")
                .nth(1)
                .unwrap()
                .contains("  (module scope)")
        );
    }

    #[test]
    fn the_header_names_the_notes_the_errors_and_the_restrictions() {
        let mut stack = SyntheticStack::new(&P, &[("m.p", "x\n")]);
        stack
            .neutral_mut()
            .errors
            .push("m.p: bad token (line 1)".into());
        let mut result = AuditResult::new(vec![], stack.neutral());
        result.notes.push("oracle: disabled".into());
        result.rules_only = vec!["11".into(), "5".into()];
        result.paths = vec![String::new(), "src".into()];
        let text = to_text(&result);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "  languages: p");
        assert_eq!(lines[2], "  rules: #5, #11");
        assert_eq!(lines[3], "  paths: ., src");
        assert_eq!(lines[4], "  note: oracle: disabled");
        assert_eq!(lines[5], "  parse error: m.p: bad token (line 1)");
    }

    #[test]
    fn a_finding_on_a_file_no_module_holds_reads_as_a_doc() {
        let stack = SyntheticStack::new(&P, &[("m.p", "x\n"), ("notes.md", "x\n")]);
        let text = to_text(&AuditResult::new(
            vec![at("notes.md", 3, "clone:k", "notes.md")],
            stack.neutral(),
        ));
        assert!(text.contains("notes.md  doc | 1 findings: #11 x1"));
        assert!(text.contains("  (module scope)"));
    }
}
