//! Port of `provers/comments.py` (the AST half; the text predicates are
//! `core::text`'s). Comment runs as a rule may read them: whether a run of
//! standalone comments reads as Python (#34), whether the first screen
//! documents the module (#29), and whether a def says in its own words that
//! what it calls must not raise (#42).

use ruff_python_ast::{Expr, Stmt};
use ruff_python_parser::parse_module;

use sightline_core::pytext;
use sightline_core::text::{NO_RAISE_RE, reads_as_doc};
use sightline_py_facts::astutil::is_call_stmt;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::{Kind, is_def};
use sightline_py_facts::model::NodeIndex;
use sightline_py_facts::module::Module;

/// The statement classes a commented-out run has to hold one of: a run of
/// bare names or a stray expression is prose that happens to parse.
const SIGNIFICANT: [Kind; 15] = [
    Kind::Assign,
    Kind::AugAssign,
    Kind::AnnAssign,
    Kind::FunctionDef,
    Kind::AsyncFunctionDef,
    Kind::ClassDef,
    Kind::Import,
    Kind::ImportFrom,
    Kind::Return,
    Kind::Raise,
    Kind::If,
    Kind::For,
    Kind::While,
    Kind::With,
    Kind::Try,
];

/// `ast.get_docstring(node)` over a body (R11): a leading `Expr` whose value
/// is a string literal, its implicit concatenation joined, `cleandoc`ed. The
/// one reading of a docstring in this crate.
pub fn docstring(body: &[Stmt]) -> Option<String> {
    match body.first() {
        Some(Stmt::Expr(e)) => match &*e.value {
            Expr::StringLiteral(s) => Some(pytext::cleandoc(s.value.to_str())),
            _ => None,
        },
        _ => None,
    }
}

/// The def or class body behind a node index; `None` for anything else.
pub fn body_of<'t>(module: &Module<'t>, node: NodeIndex) -> Option<&'t [Stmt]> {
    match module.nodes[node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => Some(&f.body),
        Cn::Stmt(Stmt::ClassDef(c)) => Some(&c.body),
        _ => None,
    }
}

/// The def says in its own words that what it calls must not raise
/// (`walk(...)  # no sink; must not raise`), in its docstring or a comment
/// inside its span: an oracle no AST shape holds.
pub fn declares_no_raise(module: &Module<'_>, node: NodeIndex) -> bool {
    let doc = if is_def(module.nodes[node as usize].kind()) {
        body_of(module, node).and_then(docstring)
    } else {
        None
    };
    if doc.is_some_and(|d| NO_RAISE_RE.is_match(&d)) {
        return true;
    }
    let start = module.line_of(node);
    let end = match module.end_line_of(node) {
        0 => start,
        end => end,
    };
    module
        .comments
        .iter()
        .filter(|c| start <= c.line && c.line <= end)
        .any(|c| NO_RAISE_RE.is_match(&c.text))
}

/// (start line, comment lines) for runs of whole-line comments.
pub fn comment_blocks<'m>(module: &'m Module<'_>) -> Vec<(u32, Vec<&'m str>)> {
    let whole: Vec<(u32, &str)> = module
        .comments
        .iter()
        .filter(|c| module.standalone_comments.contains(&c.line))
        .map(|c| (c.line, &*c.text))
        .collect();
    let mut out: Vec<(u32, Vec<&str>)> = Vec::new();
    let mut key: i64 = i64::MIN;
    for (at, (line, text)) in whole.into_iter().enumerate() {
        let here = line as i64 - at as i64;
        match out.last_mut() {
            Some((_, run)) if key == here => run.push(text),
            _ => {
                key = here;
                out.push((line, vec![text]));
            }
        }
    }
    out
}

/// Is the module's first screen a comment block saying what the module is,
/// the map in every way but form (#29 takes it for a docstring)?
pub fn documents_module(module: &Module<'_>) -> bool {
    match comment_blocks(module).first() {
        Some((1, lines)) => reads_as_doc(lines),
        _ => false,
    }
}

/// The run parses as Python and declares or does something.
pub fn parses_as_code<S: AsRef<str>>(lines: &[S]) -> bool {
    let joined: Vec<&str> = lines
        .iter()
        .map(|raw| {
            pytext::removeprefix(pytext::lstrip_chars(pytext::strip(raw.as_ref()), "#"), " ")
        })
        .collect();
    let text = pytext::dedent(&joined.join("\n"));
    // an orphan mid-chain opener (`elif`/`else:`) needs a parent to parse
    let head = pytext::lstrip(&text);
    let text = if head.starts_with("elif") {
        text.replacen("elif", "if", 1)
    } else if head.starts_with("else") {
        text.replacen("else", "if True", 1)
    } else {
        text
    };
    let Ok(parsed) = parse_module(&text) else {
        return false;
    };
    if parsed.has_syntax_errors() {
        return false;
    }
    parsed
        .suite()
        .iter()
        .any(|st| SIGNIFICANT.contains(&Cn::Stmt(st).kind()) || is_call_stmt(st))
}
