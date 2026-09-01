//! CPython's PEP 484 comment tokens, which REF's `_parse` turns on
//! (`ast.parse(text, type_comments=True)`).
//!
//! Three things follow from that parse. `Module.type_ignores` and the end of
//! an `Assign` holding a `# type:` comment are shape, so they belong to the
//! traversal. The annotations `_lift_type_comments` writes onto parameters
//! and returns are a side table here (R15, decision 11): no reader touches
//! `Parameter.annotation` directly, they ask `Module::annotation` and
//! `Module::returns`.

use std::collections::HashMap;

use ruff_python_ast::token::TokenKind;
use ruff_python_ast::{Expr, Parameter, Stmt, StmtFunctionDef};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::cn::Cn;
use crate::lines::Lines;
use crate::model::NodeIndex;
use crate::module::Module;

/// The parameter names a signature comment may leave out.
const RECEIVERS: [&str; 2] = ["self", "cls"];

/// The text after `#` [ws]* `type` `:` [ws]*, matching
/// `type_comment_prefix` in CPython's `Parser/tokenizer.c`.
pub fn strip_prefix(comment: &str) -> Option<&str> {
    let rest = comment.strip_prefix('#')?;
    let rest = rest.trim_start_matches([' ', '\t']);
    let rest = rest.strip_prefix("type")?;
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim_start_matches([' ', '\t']))
}

/// `ignore` not followed by an alphanumeric: a `TYPE_IGNORE`, not a
/// `TYPE_COMMENT`.
pub fn is_ignore(tail: &str) -> bool {
    let bytes = tail.as_bytes();
    bytes.len() >= 6
        && &bytes[..6] == b"ignore"
        && (bytes.len() == 6 || !bytes[6].is_ascii_alphanumeric())
}

/// The end CPython gives an `Assign` that a `# type:` comment follows: the
/// comment's own end, not the statement's.
pub fn assign_end(end: TextSize, source: &str) -> Option<TextSize> {
    let rest = source.get(end.to_usize()..)?;
    let gap = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    let body = &rest[gap..];
    if !body.starts_with('#') {
        return None;
    }
    let line_end = body.find(['\n', '\r']).unwrap_or(body.len());
    let tail = strip_prefix(&body[..line_end])?;
    if is_ignore(tail) {
        return None;
    }
    Some(end + TextSize::try_from(gap + line_end).ok()?)
}

// --- R15: the annotations a `# type:` comment spells -------------------------

/// The lifted annotations of one module, keyed by the parameter or def node
/// they belong to. A comment ruff cannot parse is no annotation.
pub(crate) fn annotations(module: &Module<'_>, lines: &Lines) -> HashMap<NodeIndex, Expr> {
    let source = module.source;
    let comments: Vec<(TextRange, &str)> = module
        .parsed
        .tokens()
        .iter()
        .filter(|t| t.kind() == TokenKind::Comment)
        .map(|t| (t.range(), &source[t.range()]))
        .collect();
    let mut out = HashMap::new();
    if comments.is_empty() {
        return out;
    }
    let colons: Vec<TextSize> = module
        .parsed
        .tokens()
        .iter()
        .filter(|t| t.kind() == TokenKind::Colon)
        .map(|t| t.range().start())
        .collect();
    for (index, node) in module.nodes.iter().enumerate() {
        if let Cn::Stmt(Stmt::FunctionDef(f)) = node {
            lift(f, index as NodeIndex, &comments, &colons, lines, &mut out);
        }
    }
    out
}

/// The parameters in signature order, which is the order a signature
/// comment spells its argument types in.
fn signature_params(f: &StmtFunctionDef) -> Vec<&Parameter> {
    let p = &f.parameters;
    p.posonlyargs
        .iter()
        .map(|x| &x.parameter)
        .chain(p.args.iter().map(|x| &x.parameter))
        .chain(p.vararg.iter().map(|x| &**x))
        .chain(p.kwonlyargs.iter().map(|x| &x.parameter))
        .chain(p.kwarg.iter().map(|x| &**x))
        .collect()
}

fn node_index(slot: &ruff_python_ast::AtomicNodeIndex) -> Option<NodeIndex> {
    slot.load().as_u32()
}

/// `_lift_type_comments` for one def: a per-parameter `# type: T`, then a
/// `# type: (T, ...) -> R` signature comment's return and, where its
/// argument types are spelled out, the parameters still unannotated.
fn lift(
    f: &StmtFunctionDef,
    def: NodeIndex,
    comments: &[(TextRange, &str)],
    colons: &[TextSize],
    lines: &Lines,
    out: &mut HashMap<NodeIndex, Expr>,
) {
    use ruff_python_ast::HasNodeIndex;

    let params = signature_params(f);
    let mut lifted: Vec<bool> = Vec::with_capacity(params.len());
    for p in &params {
        let mut done = false;
        if p.annotation.is_none()
            && let Some(text) = own_comment(p.range().end(), comments, lines)
            && let Some(expr) = expression(text)
            && let Some(at) = node_index(p.node_index())
        {
            out.insert(at, expr);
            done = true;
        }
        lifted.push(done);
    }

    let header_end = match &f.returns {
        Some(r) => r.range().end(),
        None => f.parameters.range().end(),
    };
    let Some(colon) = colons.iter().find(|c| **c >= header_end) else {
        return;
    };
    let Some(text) = comments
        .iter()
        .find(|(range, _)| range.start() > *colon)
        .and_then(|(_, text)| type_comment(text))
    else {
        return;
    };
    let Some((argtypes, returns)) = func_type(text) else {
        return;
    };
    if f.returns.is_none() {
        out.insert(def, returns);
    }
    let spelled: Vec<Expr> = argtypes
        .into_iter()
        .filter(|t| !matches!(t, Expr::EllipsisLiteral(_)))
        .collect();
    let mut slots: Vec<&&Parameter> = params
        .iter()
        .zip(&lifted)
        .filter(|(p, done)| p.annotation.is_none() && !**done)
        .map(|(p, _)| p)
        .collect();
    // a receiver may be left out of the argument types
    if slots
        .first()
        .is_some_and(|p| RECEIVERS.contains(&p.name.as_str()))
        && spelled.len() + 1 == slots.len()
    {
        slots.remove(0);
    }
    if spelled.is_empty() || spelled.len() != slots.len() {
        return;
    }
    for (p, t) in slots.into_iter().zip(spelled) {
        if let Some(at) = node_index(p.node_index()) {
            out.insert(at, t);
        }
    }
}

/// The `# type:` comment on a node's own line, after it.
fn own_comment<'a>(
    end: TextSize,
    comments: &[(TextRange, &'a str)],
    lines: &Lines,
) -> Option<&'a str> {
    let line = lines.pos(end.to_u32()).0;
    comments
        .iter()
        .find(|(range, _)| range.start() >= end && lines.pos(range.start().to_u32()).0 == line)
        .and_then(|(_, text)| type_comment(text))
}

/// The type a `# type:` comment spells, or `None` for a `# type: ignore`.
fn type_comment(comment: &str) -> Option<&str> {
    let tail = strip_prefix(comment)?;
    (!is_ignore(tail)).then_some(tail)
}

fn expression(text: &str) -> Option<Expr> {
    ruff_python_parser::parse_expression(text)
        .ok()
        .map(|p| *p.into_syntax().body)
}

/// `(T, ...) -> R` split at the top-level arrow, the argument types read as
/// the tuple the parentheses spell.
fn func_type(text: &str) -> Option<(Vec<Expr>, Expr)> {
    let text = text.trim();
    if !text.starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    let mut close = None;
    for (at, c) in text.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(at);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    // a space after the arrow reads as an indent to `parse_expression`
    let rest = text[close + 1..].trim_start().strip_prefix("->")?;
    let returns = expression(rest.trim_start())?;
    let argtypes = match expression(&text[..=close])? {
        Expr::Tuple(t) => t.elts,
        other => vec![other],
    };
    Some((argtypes, returns))
}
