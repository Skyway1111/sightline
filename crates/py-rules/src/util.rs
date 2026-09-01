//! Facts-aware helpers shared by rule fns (port of `rules/util.py`).
//! Pure-AST predicates live in `py_facts::astutil`; what an annotation says
//! in `py_provers::annotations`.

use std::sync::LazyLock;

use regex::Regex;
use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};

use sightline_core::edits::{blank, char_slice};
use sightline_core::findings::{Site, SpanEdit};
use sightline_core::pytext;
use sightline_py_facts::astutil::chain_root;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{
    FUNCTION_KINDS, NodeIndex, RepoFacts, Step, Symbol, class_walk, is_test_path,
};
use sightline_py_facts::module::Module;
use sightline_py_facts::unparse;

/// The name a call spells through the module's import bindings; an unbound
/// chain (a function-local import) stands for itself. The one spelling a
/// catalog is matched against (#41, #42).
pub fn library_name(module: &Module<'_>, func: &Expr) -> Option<String> {
    if let Some(name) = module.dotted_name(func) {
        return Some(name);
    }
    chain_root(func, &[Kind::Attribute]).map(|_| unparse::expr(func))
}

pub fn node_site(facts: &RepoFacts<'_>, module: &Module<'_>, node: NodeIndex) -> Site {
    let span = module.span(node);
    Site {
        rel: module.rel.clone(),
        line: span.and_then(|s| s[0]).unwrap_or(1),
        col: span.and_then(|s| s[1]).unwrap_or(0),
        symbol: facts.enclosing(module, node),
    }
}

/// The statements a `body` field holds, for the block-emptying guard in
/// `deletion`. Python reads `getattr(holder, "body", ())`, so a node with
/// no `body` counts as none and an `else` body's holder is still the `If`.
fn body_len(node: Cn<'_>) -> usize {
    match node {
        Cn::Module(m) => m.body.len(),
        Cn::Elif(rest) => rest[0].body.len(),
        Cn::Handler(h) => h.body.len(),
        Cn::Case(c) => c.body.len(),
        Cn::Stmt(Stmt::FunctionDef(n)) => n.body.len(),
        Cn::Stmt(Stmt::ClassDef(n)) => n.body.len(),
        Cn::Stmt(Stmt::For(n)) => n.body.len(),
        Cn::Stmt(Stmt::While(n)) => n.body.len(),
        Cn::Stmt(Stmt::If(n)) => n.body.len(),
        Cn::Stmt(Stmt::With(n)) => n.body.len(),
        Cn::Stmt(Stmt::Try(n)) => n.body.len(),
        _ => 0,
    }
}

/// The edits that delete a statement whole, decorators included. Empty
/// where its lines carry anything else (only indentation may precede it,
/// only a comment follow) or where taking it would empty a block.
///
/// R17: the Python site slices the line by `col_offset`, a byte count, as
/// if it were a code point count, so the port slices by code point too.
pub fn deletion(module: &Module<'_>, node: NodeIndex) -> Vec<SpanEdit> {
    let Some(span) = module.span(node) else {
        return Vec::new();
    };
    let (line, col) = (span[0].unwrap_or(0), span[1].unwrap_or(0));
    let end_line = span[2].unwrap_or(line);
    let end_col = span[3].unwrap_or(0);
    let head = char_slice(module.lines[line as usize - 1], 0, col as usize);
    let tail = pytext::strip(char_slice(
        module.lines[end_line as usize - 1],
        end_col as usize,
        usize::MAX,
    ));
    let holder = module.parent_of(node).map(|p| module.nodes[p as usize]);
    let empties_a_block = match holder {
        Some(Cn::Module(_)) => false,
        Some(h) => body_len(h) < 2,
        None => true,
    };
    if !pytext::strip(head).is_empty()
        || (!tail.is_empty() && !tail.starts_with('#'))
        || empties_a_block
    {
        return Vec::new();
    }
    let first = decorator_lines(module, node)
        .into_iter()
        .chain(std::iter::once(line))
        .min()
        .unwrap_or(line);
    blank(&module.lines, first, end_line)
}

/// The lines a def or class spells its decorators on; none for any other
/// statement (`getattr(node, "decorator_list", ())`).
pub fn decorator_lines(module: &Module<'_>, node: NodeIndex) -> Vec<u32> {
    let decorators = match module.nodes[node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => &f.decorator_list,
        Cn::Stmt(Stmt::ClassDef(c)) => &c.decorator_list,
        _ => return Vec::new(),
    };
    decorators
        .iter()
        .filter_map(|d| Cn::Expr(&d.expression).stamped())
        .map(|at| module.line_of(at))
        .collect()
}

/// Every function and method symbol, in facts order.
pub fn iter_functions<'a, 't>(
    facts: &'a RepoFacts<'t>,
) -> impl Iterator<Item = (&'a Module<'t>, &'a Symbol)> + 'a {
    facts
        .symbols
        .values()
        .filter(|sym| FUNCTION_KINDS.contains(&sym.kind))
        .map(|sym| (&facts.modules[&sym.module], sym))
}

pub fn iter_prod_functions<'a, 't>(
    facts: &'a RepoFacts<'t>,
) -> impl Iterator<Item = (&'a Module<'t>, &'a Symbol)> + 'a {
    iter_functions(facts).filter(|(module, _)| !is_test_path(&module.rel))
}

/// The class or function symbol enclosing `sym`; `None` at module level.
pub fn owner_of<'a>(facts: &'a RepoFacts<'_>, sym: &Symbol) -> Option<&'a Symbol> {
    facts.symbols.get(sym.parent.as_deref()?)
}

/// Test functions the runners collect: a `test*` def at module level of a
/// `test*.py` / `*_test.py` file, or a `test*` method of a class they
/// collect (pytest's `Test*` prefix, or a `unittest.TestCase` subclass,
/// whose own name is free). Helpers in support modules under `tests/` are
/// not tests even when named `test_*`; a `test` method of a double is that
/// object's protocol; and a def nested in a function is a callback whatever
/// its name, since no runner ever sees it.
pub fn iter_test_functions<'a, 't>(
    facts: &'a RepoFacts<'t>,
) -> impl Iterator<Item = (&'a Module<'t>, &'a Symbol)> + 'a {
    iter_functions(facts).filter(move |(module, sym)| {
        let name = module.rel.rsplit('/').next().unwrap_or("");
        if !(sym.name.starts_with("test")
            && is_test_path(&module.rel)
            && (name.starts_with("test") || name.ends_with("_test.py")))
        {
            return false;
        }
        match owner_of(facts, sym) {
            None => true,
            Some(owner) => {
                owner.kind == "class"
                    && (owner.name.starts_with("Test")
                        || class_walk(facts, &owner.qname, Step::Bases)
                            .iter()
                            .flat_map(|(_, info)| &info.external_bases)
                            .any(|b| b.rsplit('.').next() == Some("TestCase")))
            }
        }
    })
}

/// `(type, repr)` identity of a literal worth matching across sites (#38
/// duplication). Strings and bytes of length 3 or more only: numbers are
/// per-module domain facts that coincidentally share a value.
pub fn nontrivial_literal(value: Option<&Expr>) -> Option<(&'static str, String)> {
    match value? {
        Expr::StringLiteral(s) => {
            let text = s.value.to_str();
            (text.chars().count() >= 3).then(|| ("str", pytext::repr_str(text)))
        }
        Expr::BytesLiteral(b) => {
            let bytes: Vec<u8> = b.value.bytes().collect();
            (bytes.len() >= 3).then(|| ("bytes", pytext::repr_bytes(&bytes)))
        }
        _ => None,
    }
}

/// Public API surface: a public symbol, and for a method a public class too.
pub fn is_boundary(facts: &RepoFacts<'_>, sym: &Symbol) -> bool {
    if sym.kind == "method"
        && let Some(parent) = sym.parent.as_deref().and_then(|p| facts.symbols.get(p))
    {
        return sym.is_public && parent.is_public;
    }
    sym.is_public
}

/// Inside a function at any depth (a method of a class local to a function
/// is nested): a closure publishes no contract.
pub fn is_nested(facts: &RepoFacts<'_>, sym: &Symbol) -> bool {
    let mut parent = owner_of(facts, sym);
    while let Some(p) = parent {
        if FUNCTION_KINDS.contains(&p.kind) {
            return true;
        }
        parent = owner_of(facts, p);
    }
    false
}

/// A def reachable by a public name (#50, #53): a boundary in a module no
/// dotted part of which is `_`-prefixed, nested in no function.
/// `published` does not narrow it, since an unpublished module's public def
/// is still the boundary its own package calls.
pub fn is_exported(facts: &RepoFacts<'_>, module: &Module<'_>, sym: &Symbol) -> bool {
    is_boundary(facts, sym)
        && !module.qname.split('.').any(|part| part.starts_with('_'))
        && !is_nested(facts, sym)
}

/// A silencing pragma: what #36 counts and #58 honours on the diagnostic's
/// line (R7).
pub static IGNORE_PRAGMA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(type|pyright|mypy):\s*ignore\b").expect("a valid pattern"));

/// Inside the type-check scope the repo declared (#1, #50)? A repo that
/// named its packages excused every file outside them; one that declared no
/// scope keeps all of them.
pub fn in_typed_scope(facts: &RepoFacts<'_>, rel: &str) -> bool {
    facts.typed_scope.is_empty()
        || facts
            .typed_scope
            .iter()
            .any(|seg| rel == seg || format!("/{rel}").contains(&format!("/{seg}/")))
}

/// A function symbol's def node, typed.
pub fn fn_of<'t>(module: &Module<'t>, sym: &Symbol) -> &'t StmtFunctionDef {
    match module.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => f,
        _ => panic!("a function symbol's node is a def"),
    }
}

/// `ast.get_docstring(node, clean=False)` (R11): the raw value of a leading
/// `Expr` whose value is a string literal, implicit concatenation joined. #7
/// and #53 judge the text as written, so neither takes the cleaned form.
pub fn raw_docstring(body: &[Stmt]) -> Option<&str> {
    match body.first() {
        Some(Stmt::Expr(e)) => match &*e.value {
            Expr::StringLiteral(s) => Some(s.value.to_str()),
            _ => None,
        },
        _ => None,
    }
}

/// The head name of every decorator spelled as a name or an attribute.
pub fn decorator_names(fn_def: &StmtFunctionDef) -> std::collections::HashSet<String> {
    fn_def
        .decorator_list
        .iter()
        .map(|d| match &d.expression {
            Expr::Call(c) => &*c.func,
            other => other,
        })
        .filter_map(|head| match head {
            Expr::Name(n) => Some(n.id.to_string()),
            Expr::Attribute(a) => Some(a.attr.to_string()),
            _ => None,
        })
        .collect()
}

/// Innermost function or class symbol whose span contains the line. Ties go
/// to the first in facts order (R4: Python's `min` keeps the first).
pub fn enclosing_at_line(facts: &RepoFacts<'_>, module: &Module<'_>, line: u32) -> String {
    let mut best: Option<&Symbol> = None;
    for at in facts
        .symbols_by_module
        .get(&module.qname)
        .map_or(&[][..], |v| v)
    {
        let Some((_, sym)) = facts.symbols.get_index(*at as usize) else {
            continue;
        };
        if sym.lineno > line || line > sym.end_lineno {
            continue;
        }
        let span = |s: &Symbol| s.end_lineno - s.lineno;
        if best.is_none_or(|held| span(sym) < span(held)) {
            best = Some(sym);
        }
    }
    best.map_or_else(|| module.qname.to_string(), |s| s.qname.to_string())
}
