//! The `fix` verb: verified findings' span edits into
//! one git-apply-able unified diff against the file's raw bytes, line
//! endings preserved. Each rule that proposes a patch owns the splice that
//! builds it; this module batches them through one counterfactual pass, so
//! no rule pays the worlds at audit time, and prints the result. It writes
//! text, never the tree.

use std::collections::{BTreeSet, HashMap, HashSet};

use indexmap::IndexMap;
use ruff_python_ast::{Expr, Operator, Stmt};

use sightline_core::edits::{apply_edits, char_slice, takes_line};
use sightline_core::findings::{Finding, Fix, Rel, SpanEdit};
use sightline_core::patch::{compose, unified_diff as diff_lines};
use sightline_core::text::lookup;
use sightline_py_facts::astutil::{fn_body, walk};
use sightline_py_facts::build::{parses, raw_lines};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{RepoFacts, Symbol};
use sightline_py_facts::module::Module;
use sightline_py_provers::Provers;
use sightline_py_provers::counterfactual::{Splice, from_home, merge_imports};
use sightline_py_provers::import_effects::import_time;
use sightline_py_provers::typestrings::union_members;

// --- the fix table -----------------------------------------------------------

/// A rule's own splice builder, keyed by the finding's cause.
type Splicer = fn(&str, &RepoFacts<'_>, &Provers) -> Option<Splice>;

/// The rules that propose a patch. Every entry but #33's lives with the rule
/// that reads the cause.
const SPLICERS: &[(&str, Splicer)] = &[
    ("32", crate::dead::dead_symbol_splice),
    ("33", return_splice),
    ("35", crate::imports::hoist_splice),
    ("39", crate::comments::comment_splice),
    ("48", crate::surface::fold_splice),
];

/// Findings whose rule proposes a patch gain a `Fix` where one world pass
/// verifies it; the rest pass through untouched, evidence and message
/// included. A vetoed splice yields no fix, never a partial one; `compose`
/// then settles the fixes that share a site.
pub fn attach_fixes(
    findings: Vec<Finding>,
    facts: &RepoFacts<'_>,
    provers: &Provers,
) -> Vec<Finding> {
    if provers.no_oracle() {
        return findings; // no worlds: every finding keeps its own claim
    }
    let splices: Vec<Splice> = findings
        .iter()
        .filter_map(|f| lookup(SPLICERS, f.rule).and_then(|make| make(&f.cause, facts, provers)))
        .collect();
    let verified = provers.verify_splice(facts, &splices);
    let fixed: Vec<Finding> = findings
        .into_iter()
        .map(|mut f| {
            if let Some((_, fix)) = verified.get(&f.cause) {
                f.fix = Some(fix.clone());
            }
            f
        })
        .collect();
    compose(fixed)
}

// --- #33 return fixes --------------------------------------------------------
// A None-path lie gets ` | None`, a mixed return ` -> R` from the oracle's
// reveal (typing spellings without runtime PEP 604); a veto keeps the
// finding, drops the patch. #36 has no arm: Any/Unknown admits no verifiable
// annotation.

/// (edit, the spelling whose names must import).
type Edit = (SpanEdit, String);

/// The annotation this #33 finding asks for, keyed by the finding's own
/// cause so the verdict comes back to it.
fn return_splice(cause: &str, facts: &RepoFacts<'_>, provers: &Provers) -> Option<Splice> {
    let (kind, qname) = cause.split_once(':')?;
    let sym = facts.symbols.get(qname)?;
    let module = facts.modules.get(&*sym.module)?;
    let (edit, spelling) = if kind == "lying-return" {
        none_path_splice(module, sym)?
    } else {
        mixed_splice(module, sym, provers.ret_types(facts).return_type(qname))?
    };
    Some(Splice {
        id: cause.to_string(),
        owner: qname.to_string(),
        edits: vec![edit],
        spelling,
        imports: Vec::new(),
        param: String::new(),
    })
}

/// R15 writes a `# type:` return onto the def with the def's own position
/// (`py_facts::typecomments` lifts it), so an annotation with no node of its
/// own reads the def's span.
fn full_span(module: &Module<'_>, node: Option<&Expr>, fallback: u32) -> Option<[u32; 4]> {
    let at = node.and_then(|e| Cn::Expr(e).stamped()).unwrap_or(fallback);
    let span = module.span(at)?;
    let mut parts = [0u32; 4];
    for (slot, value) in parts.iter_mut().zip(span) {
        *slot = value?;
    }
    Some(parts)
}

/// ` | None` after a single-line `-> X` (`Optional[X]` over it without
/// runtime PEP 604); string annotations and `-> None` lies get no fix.
fn none_path_splice(module: &Module<'_>, sym: &Symbol) -> Option<Edit> {
    let ret = module.returns(sym.node)?;
    if matches!(
        ret,
        Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::NumberLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::EllipsisLiteral(_)
    ) {
        return None;
    }
    let [line, col, end_line, end_col] = full_span(module, Some(ret), sym.node)?;
    if line != end_line {
        return None;
    }
    if runtime_evidence(module).contains("pep604") {
        return Some((
            SpanEdit {
                line,
                col_start: end_col,
                col_end: end_col,
                text: " | None".to_string(),
            },
            "None".to_string(),
        ));
    }
    let src = char_slice(
        module.lines.get(line as usize - 1)?,
        col as usize,
        end_col as usize,
    );
    Some((
        SpanEdit {
            line,
            col_start: col,
            col_end: end_col,
            text: format!("Optional[{src}]"),
        },
        "Optional".to_string(),
    ))
}

/// ` -> R` at the header end: R the revealed return, deliteralized, None
/// kept and last, builtins/abc names only.
fn mixed_splice(module: &Module<'_>, sym: &Symbol, revealed: Option<&str>) -> Option<Edit> {
    let members = revealed.and_then(union_members);
    let parts: BTreeSet<&str> = members
        .iter()
        .flatten()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if parts.is_empty() || !parts.contains("None") || parts.len() == 1 {
        return None;
    }
    let rest: Vec<&str> = parts.into_iter().filter(|m| *m != "None").collect();
    // an unspellable reveal (`A & ~B`) is `spell`'s to refuse
    let spelled = spell(&rest, &runtime_evidence(module))?;
    let Cn::Stmt(Stmt::FunctionDef(func)) = module.nodes[sym.node as usize] else {
        return None;
    };
    let (line, col) = module
        .header_end(func)
        .unwrap_or_else(|| panic!("no header end for {}", func.name));
    Some((
        SpanEdit {
            line,
            col_start: col,
            col_end: col,
            text: format!(" -> {spelled}"),
        },
        spelled,
    ))
}

/// `X | None` under runtime PEP 604, else `Optional[X]` / `Union[X, Y,
/// None]`; subscripted members need PEP 585 evidence too.
fn spell(members: &[&str], evidence: &HashSet<&'static str>) -> Option<String> {
    if evidence.contains("pep604") {
        let mut all: Vec<&str> = members.to_vec();
        all.push("None");
        return Some(all.join(" | "));
    }
    if members.iter().any(|m| m.contains('[')) && !evidence.contains("pep585") {
        return None;
    }
    if members.len() == 1 {
        return Some(format!("Optional[{}]", members[0]));
    }
    let mut all: Vec<&str> = members.to_vec();
    all.push("None");
    Some(format!("Union[{}]", all.join(", ")))
}

const BUILTIN_GENERICS: &[&str] = &["list", "dict", "set", "frozenset", "tuple", "type"];

/// Annotation syntax the module executes (or defers): "pep604" (`X | None`),
/// "pep585" (`list[int]`). A patch runs at import, and ty flags neither
/// under an old requires-python (probed at 3.8).
pub fn runtime_evidence(module: &Module<'_>) -> HashSet<&'static str> {
    let body = &module.parsed.syntax().body;
    let defers = body.iter().any(|st| match st {
        Stmt::ImportFrom(i) => {
            i.module
                .as_ref()
                .is_some_and(|m| m.as_str() == "__future__")
                && i.names.iter().any(|a| a.name.as_str() == "annotations")
        }
        _ => false,
    });
    if defers {
        return HashSet::from(["pep604", "pep585"]);
    }
    // params, returns, every `x: T` the import evaluates (module or class
    // level, under an `if`/`try` too; a function-local one never runs)
    let mut evaluated: Vec<&Expr> = Vec::new();
    for at in module.nodes(&[Kind::Arg], None, false) {
        evaluated.extend(module.annotation(at));
    }
    for at in module.nodes(&[Kind::FunctionDef, Kind::AsyncFunctionDef], None, false) {
        evaluated.extend(module.returns(at));
    }
    let mut walked: Vec<Cn<'_>> = Vec::new();
    import_time(module.nodes[0], &mut walked);
    for node in walked {
        if let Cn::Stmt(Stmt::AnnAssign(a)) = node {
            evaluated.push(&a.annotation);
        }
    }
    evaluated
        .into_iter()
        .flat_map(|ann| walk(Cn::Expr(ann)))
        .filter_map(annotation_syntax)
        .collect()
}

fn annotation_syntax(node: Cn<'_>) -> Option<&'static str> {
    match node {
        Cn::Expr(Expr::BinOp(b)) if b.op == Operator::BitOr => Some("pep604"),
        Cn::Expr(Expr::Subscript(s)) => match &*s.value {
            Expr::Name(n) if BUILTIN_GENERICS.contains(&n.id.as_str()) => Some("pep585"),
            _ => None,
        },
        _ => None,
    }
}

// --- the diff ----------------------------------------------------------------

/// Trim the blank run a deletion left at `out[at]` to two lines: the hole is
/// the patch's, not the file's. Returns how many of the removed lines sat
/// above `idx`, the import landing.
fn close_gap(out: &mut Vec<String>, at: usize, idx: usize) -> usize {
    let mut start = at.min(out.len());
    let mut end = start;
    while start > 0 && out[start - 1].trim().is_empty() {
        start -= 1;
    }
    while end < out.len() && out[end].trim().is_empty() {
        end += 1;
    }
    if end - start <= 2 {
        return 0;
    }
    out.drain(start + 2..end);
    end.min(idx).saturating_sub(start + 2)
}

fn patched_lines(module: &Module<'_>, lines: &[String], fixes: &[&Fix]) -> Vec<String> {
    let mut out: Vec<String> = lines.to_vec();
    let edits: Vec<SpanEdit> = fixes.iter().flat_map(|f| f.edits.iter().cloned()).collect();
    apply_edits(&mut out, &edits);
    let needed = merge_imports(fixes.iter().flat_map(|f| f.imports.iter().cloned()));
    // after the last top-of-file import, else the docstring, else line 0
    let top = &module.parsed.syntax().body;
    let body = fn_body(top);
    let heads = (top.len() - body.len())
        + body
            .iter()
            .take_while(|st| matches!(st, Stmt::Import(_) | Stmt::ImportFrom(_)))
            .count();
    let mut idx = if heads == 0 {
        0
    } else {
        module.lines_of(Cn::Stmt(&top[heads - 1])).1 as usize
    };
    // an emptied line is a deletion in the patch, where applied text does
    // move (a world keeps it: its diagnostic diff is line-keyed)
    let mut dead: Vec<u32> = edits
        .iter()
        .filter(|e| takes_line(e) && out[e.line as usize - 1].trim().is_empty())
        .map(|e| e.line)
        .collect();
    dead.sort_unstable();
    dead.dedup();
    dead.reverse();
    for line in &dead {
        out.remove(*line as usize - 1);
        idx -= usize::from(*line as usize <= idx);
    }
    let seams: BTreeSet<usize> = dead
        .iter()
        .map(|line| *line as usize - 1 - dead.iter().filter(|d| *d < line).count())
        .collect();
    for at in seams.iter().rev() {
        idx -= close_gap(&mut out, *at, idx);
    }
    if needed.is_empty() {
        return out;
    }
    let reference = out.get(idx).or_else(|| out.last());
    let eol = match reference {
        Some(line) if line.ends_with("\r\n") => "\r\n",
        _ => "\n",
    };
    // a home the file already imports from grows that line; the rest land
    let mut homes: HashMap<&str, usize> = HashMap::new();
    for (at, line) in out.iter().enumerate().take(idx.min(out.len())) {
        if let Some(home) = from_home(line.trim()) {
            homes.insert(home, at);
        }
    }
    let mut fresh: Vec<String> = Vec::new();
    let mut grown: Vec<(usize, String)> = Vec::new();
    for stmt in needed {
        match from_home(&stmt).and_then(|home| homes.get(home)) {
            None => fresh.push(stmt),
            Some(&at) => {
                let merged = merge_imports([out[at].trim().to_string(), stmt]);
                grown.push((at, merged[0].clone() + eol));
            }
        }
    }
    for (at, line) in grown {
        out[at] = line;
    }
    out.splice(idx..idx, fresh.into_iter().map(|line| line + eol));
    out
}

/// Unified diff over every finding with a `Fix`; the empty string when
/// nothing is fixable. Leading `# sightline-fix:` lines name what the patch
/// discharges (`git apply` ignores text before the first diff header).
pub fn unified_diff(findings: &[Finding], facts: &RepoFacts<'_>) -> String {
    let mut by_rel: IndexMap<Rel, Vec<&Finding>> = IndexMap::new();
    for f in findings {
        if let Some(fix) = &f.fix {
            by_rel.entry(fix.rel.clone()).or_default().push(f);
        }
    }
    let mut rels: Vec<Rel> = by_rel.keys().cloned().collect();
    rels.sort();

    let mut headers: Vec<String> = Vec::new();
    let mut body = String::new();
    for rel in rels {
        let group = &by_rel[&rel];
        let Some(module) = facts.module_by_rel(&rel) else {
            continue;
        };
        // raw read: line endings preserved so `git apply` matches the
        // on-disk bytes, split on `\n` only (AST lines count `\n`, never
        // `\f`), ends kept
        let old = raw_lines(&module.path);
        let fixes: Vec<&Fix> = group.iter().filter_map(|f| f.fix.as_ref()).collect();
        let new = patched_lines(module, &old, &fixes);
        // the emitter parses what it emits: deletions can meet (two siblings
        // emptying their block)
        if !parses(&new.concat()) {
            continue; // a patch this file cannot hold: none of it ships
        }
        headers.extend(
            group
                .iter()
                .map(|f| format!("# sightline-fix: {} {}\n", f.rule, f.cause)),
        );
        body.push_str(&diff_lines(&old, &new, &rel));
    }
    if body.is_empty() {
        return String::new();
    }
    headers.sort();
    headers.concat() + &body
}
