//! Family Z, the checker's own verdicts (#58). sightline's other oracle rules ask the checker questions of their
//! own; this one forwards what it already said. Nothing here re-derives a
//! type: the whole rule is a projection of pass 1's verdicts onto findings.

use sightline_core::findings::{Evidence, Finding, Sink, Site};
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::astutil::line_span;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{RepoFacts, is_test_path};
use sightline_py_facts::module::Module;
use sightline_py_provers::Provers;

use crate::model::Rule;
use crate::util::{IGNORE_PRAGMA_RE, enclosing_at_line, in_typed_scope};

/// The named set: verdicts about claims the repo's own code makes, where the
/// checker's error is a defect a reader must know about. Absent by intent:
/// `unresolved-import` (the provenance header counts it, and `Provers::errors`
/// drops a module blinded by one) and `unresolved-reference` (its consequence
/// inside such a module). Named as the shim maps them: ty ids, except where
/// the shim gives a lint pyright's name. The judged cuts
/// (`unresolved-attribute`, `invalid-assignment`, `invalid-argument-type`,
/// `reportPossiblyUnbound`) and their samples: `corpus-ext/decisions.tsv`.
pub const Z_RULES: [&str; 7] = [
    "invalid-return-type",
    // override incompatibilities: the Liskov breaks the judge waves found
    // (dj-stripe P1-94, authlib P1-101)
    "invalid-method-override",
    "invalid-attribute-override",
    "invalid-explicit-override",
    "invalid-dataclass-override",
    "override-of-final-method",
    "override-of-final-variable",
];

/// Does the repo carry a checker pragma on the verdict's line, or, for a
/// return statement spanning several, on any of them? The checker anchors at
/// the first line, a `# type: ignore` sits on the closing one.
fn silenced(module: &Module<'_>, line: u32) -> bool {
    let (mut lo, mut hi) = (line, line);
    for at in module.nodes(&[Kind::Return], None, false) {
        let span = line_span((module.line_of(at), module.end_line_of(at)));
        if span.0 <= line && line <= span.1 {
            (lo, hi) = span;
        }
    }
    let last = hi.min(module.lines.len() as u32);
    module.lines[lo as usize - 1..last as usize]
        .iter()
        .any(|text| IGNORE_PRAGMA_RE.is_match(text))
}

pub const RULE_58: Rule = Rule {
    record: RuleRecord {
        id: "58",
        slug: "checker-error",
        family: "checker",
        engine_class: "ORACLE",
        posture: Posture::Report,
        meaning: "the type checker's own verdicts on the repo's code: return type \
                  errors and override incompatibilities, in prod modules inside the \
                  repo's declared type-check scope, off a line the repo already \
                  silenced, outside any module an unresolved import blinded",
        goal: "An agent reading a repo inherits its live type errors. Gating them \
               would only reward deleting annotations - the checker sees more where \
               the code says more - so they are reported, never gated.",
        lang: "py",
        scope: Scope::Repo,
        complement: "ty owns these verdicts and #58 forwards them; ruff never type-checks, \
                     so its F821 covers only the name-level half and it has no reading of \
                     a possibly-unbound read at all",
    },
    run: rule_58,
};

/// The checker's verdicts, unmodified. The claim is the checker's and not the
/// repo's, so the evidence is never grounded: every #58 finding is heuristic.
fn rule_58(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    for (module, diag) in provers.errors(facts) {
        // g3 close (3 real / 17 fp on five library clones): 12 of 20 rows sat
        // in test trees the repos scope their own checkers out of, one on a
        // line already holding `# pyright: ignore`
        if is_test_path(&diag.rel)
            || !in_typed_scope(facts, &diag.rel)
            || silenced(module, diag.line)
        {
            continue;
        }
        if !Z_RULES.contains(&diag.rule.as_str()) {
            continue;
        }
        out.push(Finding {
            rule: "58",
            site: Site {
                rel: diag.rel.clone(),
                line: diag.line,
                col: diag.col,
                symbol: enclosing_at_line(facts, module, diag.line).into(),
            },
            message: format!("{}: {}", diag.rule, diag.message),
            // a call line can carry 85 argument errors: the column keys them apart
            cause: format!("{}:{}:{}:{}", diag.rule, module.qname, diag.line, diag.col),
            evidence: Evidence::Oracle {
                rule: diag.rule.clone(),
                grounded: false,
                message: diag.message.clone(),
            },
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}
