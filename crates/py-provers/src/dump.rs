//! CPython's `ast.dump(annotate_fields=True, show_empty=False)` field grammar
//! over ruff nodes (R9, R13), and the node count `ast.walk` gives a subtree.
//!
//! `fields` is the one enumeration of a node's CPython `_fields` with their
//! values, so `normalize` and `size` never disagree about what a node holds:
//! `size` counts the `expr_context` and operator nodes ruff has no node for,
//! and `normalize` prints them.
//!
//! The keys hash the text `normalize` writes, so its equality classes are
//! the contract, not the spelling.

use std::collections::HashMap;

use ruff_python_ast::token::TokenKind;
use ruff_python_ast::{
    BoolOp, CmpOp, ElifElseClause, ExceptHandler, Expr, ExprContext, Number, Operator,
    ParameterWithDefault, Pattern, Singleton, Stmt, StmtFunctionDef, TypeParam, TypeParams,
    UnaryOp,
};
use ruff_text_size::{Ranged, TextSize};

use sightline_core::pytext;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::{Kind, is_expr, is_stmt};
use sightline_py_facts::model::ModuleId;
use sightline_py_facts::module::Module;
use sightline_py_facts::{order, typecomments};

/// How a shape reads identifiers. `Blind`: every identifier and literal is
/// noise, `await` is transparent and async shapes key on their sync twins, so
/// two bodies differing only in names are one shape (#11). `By`: the caller's
/// own consistent renaming (`id -> new`, `None` keeps the name), under which
/// literals stay and the shape reads as a value.
pub enum Rename<'r> {
    Blind,
    By(&'r dyn Fn(&str) -> Option<String>),
}

/// Rename mode: nothing but the structure counts.
pub const BLIND: Rename<'static> = Rename::Blind;

/// One node's identity across the repo: its module, a class tag and the
/// address of the ruff node behind it. A folded f-string `Constant` and a
/// `TypeIgnore` have no node of their own, so they key on their position.
type Key = (ModuleId, u8, usize);

/// `id(stmt | expr) -> normalized dump`, one run's memo.
pub type Dumps = HashMap<Key, String>;
/// `id(node) -> nodes in its subtree`, one run's memo.
pub type Sizes = HashMap<Key, usize>;

fn key(node: Cn<'_>, module: &Module<'_>) -> Key {
    let (tag, at) = match node {
        Cn::Module(m) => (0, std::ptr::from_ref(m) as usize),
        Cn::Stmt(s) => (1, std::ptr::from_ref(s) as usize),
        Cn::Elif(r) => (2, r.as_ptr() as usize),
        Cn::Expr(e) => (3, std::ptr::from_ref(e) as usize),
        Cn::Params(p) => (4, std::ptr::from_ref(p) as usize),
        Cn::Param(p) => (5, std::ptr::from_ref(p) as usize),
        Cn::Handler(h) => (6, std::ptr::from_ref(h) as usize),
        Cn::Comp(c) => (7, std::ptr::from_ref(c) as usize),
        Cn::Item(w) => (8, std::ptr::from_ref(w) as usize),
        Cn::Case(c) => (9, std::ptr::from_ref(c) as usize),
        Cn::Pattern(p) => (10, std::ptr::from_ref(p) as usize),
        Cn::TypeParam(t) => (11, std::ptr::from_ref(t) as usize),
        Cn::Alias(a) => (12, std::ptr::from_ref(a) as usize),
        Cn::Keyword(k) => (13, std::ptr::from_ref(k) as usize),
        Cn::FConst { range, .. } => (14, range.start().to_u32() as usize),
        Cn::Interp(i, _) => (15, std::ptr::from_ref(i) as usize),
        Cn::Spec(s) => (16, std::ptr::from_ref(s) as usize),
        Cn::CallGen(g, _) => (17, std::ptr::from_ref(g) as usize),
        Cn::TypeIgnore(line) => (18, line as usize),
    };
    (module.id, tag, at)
}

/// One field's value, in the shapes `ast.dump` prints them.
enum Part<'a> {
    Child(Cn<'a>),
    /// an `expr_context`: `Load()` wherever the shape reads as a value
    Ctx(&'static str),
    /// a childless AST node (an operator, a bare `arguments`)
    Bare(&'static str),
    /// a scalar: no AST node, so no node count
    Raw(String),
    List(Vec<Part<'a>>),
}

/// A node that stands on its own reads as a value: its context is `Load`
/// whatever position it was written in, down a target's elements only.
fn target_element(kind: Kind) -> bool {
    matches!(kind, Kind::Tuple | Kind::List | Kind::Starred)
}

fn sync_twin(kind: Kind) -> Kind {
    match kind {
        Kind::AsyncFunctionDef => Kind::FunctionDef,
        Kind::AsyncFor => Kind::For,
        Kind::AsyncWith => Kind::With,
        other => other,
    }
}

/// `ast.dump` of one shape, renamed so copies key alike, composed from
/// memoized statement and expression strings: a node nested d deep is
/// serialized once, not d+1 times. `value` is the recursion's; the root
/// answers from `rename`.
pub fn normalize<'a>(
    node: Cn<'a>,
    module: &'a Module<'a>,
    rename: &Rename<'_>,
    memo: &mut Dumps,
    value: Option<bool>,
) -> String {
    let blind = matches!(rename, Rename::Blind);
    let value = value.unwrap_or(!blind);
    if let Cn::Expr(Expr::Name(n)) = node {
        let new = match rename {
            Rename::Blind => Some("n".to_string()),
            Rename::By(f) => f(n.id.as_str()),
        };
        let ctx = if value || new.is_some() {
            "Load"
        } else {
            ctx_name(n.ctx)
        };
        let id = match &new {
            Some(text) if !text.is_empty() => text.as_str(),
            _ => n.id.as_str(),
        };
        return format!("Name(id={}, ctx={ctx}())", pytext::repr_str(id));
    }
    let kind = node.kind();
    if blind {
        match node {
            Cn::Param(_) => return "arg(arg='n')".to_string(),
            Cn::Expr(Expr::Await(a)) => {
                return normalize(Cn::Expr(&a.value), module, rename, memo, Some(value));
            }
            _ if kind == Kind::Constant => return "Constant(value='c')".to_string(),
            _ => {}
        }
    }
    let memoized = is_stmt(kind) || is_expr(kind);
    let id = key(node, module);
    if memoized && let Some(out) = memo.get(&id) {
        return out.clone();
    }
    let inner = value && target_element(kind);
    let rendered: Vec<String> = fields(node, module)
        .iter()
        .map(|(name, part)| {
            let below = if *name == "ctx" { value } else { inner };
            format!("{name}={}", render(part, module, rename, memo, below))
        })
        .collect();
    let out = format!("{}({})", sync_twin(kind).name(), rendered.join(", "));
    if memoized {
        memo.insert(id, out.clone());
    }
    out
}

fn render(
    part: &Part<'_>,
    module: &Module<'_>,
    rename: &Rename<'_>,
    memo: &mut Dumps,
    value: bool,
) -> String {
    match part {
        Part::Child(node) => normalize(*node, module, rename, memo, Some(value)),
        Part::Ctx(name) => format!("{}()", if value { "Load" } else { name }),
        Part::Bare(name) => format!("{name}()"),
        Part::Raw(text) => text.clone(),
        Part::List(items) => {
            let rendered: Vec<String> = items
                .iter()
                .map(|p| render(p, module, rename, memo, value))
                .collect();
            format!("[{}]", rendered.join(", "))
        }
    }
}

/// Nodes in the subtree, `ast.walk`'s count: the family's "worth a name"
/// floor. The memo counts a node nested d deep once instead of once per
/// enclosing statement and scope.
pub fn size(node: Cn<'_>, module: &Module<'_>, memo: &mut Sizes) -> usize {
    let id = key(node, module);
    if let Some(total) = memo.get(&id) {
        return *total;
    }
    let total = 1 + fields(node, module)
        .iter()
        .map(|(_, part)| part_size(part, module, memo))
        .sum::<usize>();
    memo.insert(id, total);
    total
}

fn part_size(part: &Part<'_>, module: &Module<'_>, memo: &mut Sizes) -> usize {
    match part {
        Part::Child(node) => size(*node, module, memo),
        Part::Ctx(_) | Part::Bare(_) => 1,
        Part::Raw(_) => 0,
        Part::List(items) => items.iter().map(|p| part_size(p, module, memo)).sum(),
    }
}

mod exprs;
mod fields;

use exprs::{expr_fields, pattern_fields};
use fields::fields;

// --- scalars -----------------------------------------------------------------

fn ctx_name(ctx: ExprContext) -> &'static str {
    match ctx {
        ExprContext::Store => "Store",
        ExprContext::Del => "Del",
        ExprContext::Load | ExprContext::Invalid => "Load",
    }
}

fn boolop(op: BoolOp) -> &'static str {
    match op {
        BoolOp::And => "And",
        BoolOp::Or => "Or",
    }
}

fn unaryop(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Invert => "Invert",
        UnaryOp::Not => "Not",
        UnaryOp::UAdd => "UAdd",
        UnaryOp::USub => "USub",
    }
}

fn operator(op: Operator) -> &'static str {
    match op {
        Operator::Add => "Add",
        Operator::Sub => "Sub",
        Operator::Mult => "Mult",
        Operator::MatMult => "MatMult",
        Operator::Div => "Div",
        Operator::Mod => "Mod",
        Operator::Pow => "Pow",
        Operator::LShift => "LShift",
        Operator::RShift => "RShift",
        Operator::BitOr => "BitOr",
        Operator::BitXor => "BitXor",
        Operator::BitAnd => "BitAnd",
        Operator::FloorDiv => "FloorDiv",
    }
}

fn cmpop(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "Eq",
        CmpOp::NotEq => "NotEq",
        CmpOp::Lt => "Lt",
        CmpOp::LtE => "LtE",
        CmpOp::Gt => "Gt",
        CmpOp::GtE => "GtE",
        CmpOp::Is => "Is",
        CmpOp::IsNot => "IsNot",
        CmpOp::In => "In",
        CmpOp::NotIn => "NotIn",
    }
}

/// `repr(Constant.value)`. The blind reading never asks, so this serves the
/// rename mode alone.
pub fn constant(e: &Expr, module: &Module<'_>) -> String {
    match e {
        Expr::StringLiteral(s) => pytext::repr_str(s.value.to_str()),
        Expr::BytesLiteral(b) => pytext::repr_bytes(&b.value.bytes().collect::<Vec<u8>>()),
        Expr::NumberLiteral(n) => match &n.value {
            Number::Int(i) => i.to_string(),
            Number::Float(f) => pytext::repr_float(*f),
            Number::Complex { real, imag } if *real == 0.0 => {
                format!("{}j", pytext::repr_float(*imag))
            }
            Number::Complex { real, imag } => format!(
                "({}{}j)",
                pytext::repr_float(*real),
                pytext::repr_float(*imag)
            ),
        },
        Expr::BooleanLiteral(b) => if b.value { "True" } else { "False" }.to_string(),
        Expr::NoneLiteral(_) => "None".to_string(),
        Expr::EllipsisLiteral(_) => "Ellipsis".to_string(),
        other => pytext::repr_str(&module.source[other.range()]),
    }
}

// --- PEP 484 comments, which CPython leaves on the node ----------------------

/// The type a `# type:` comment on the node's own line spells, after it.
fn same_line_comment<'m>(module: &'m Module<'_>, end: TextSize) -> Option<&'m str> {
    let token = module
        .parsed
        .tokens()
        .iter()
        .find(|t| t.kind() == TokenKind::Comment && t.range().start() >= end)?;
    if module.source[end.to_usize()..token.range().start().to_usize()].contains('\n') {
        return None;
    }
    let tail = typecomments::strip_prefix(&module.source[token.range()])?;
    (!typecomments::is_ignore(tail)).then_some(tail)
}

/// An assign's own `# type:` comment, which CPython ends the statement at.
fn assign_comment<'m>(module: &'m Module<'_>, end: TextSize) -> Option<&'m str> {
    let stop = typecomments::assign_end(end, module.source)?;
    let run = &module.source[end.to_usize()..stop.to_usize()];
    typecomments::strip_prefix(run.trim_start_matches([' ', '\t']))
}

/// A def's `# type: (T, ...) -> R` comment: the first comment past the
/// header's colon, as `_lift_type_comments` reads it.
fn signature_comment<'m>(module: &'m Module<'_>, f: &StmtFunctionDef) -> Option<&'m str> {
    let header_end = f
        .returns
        .as_ref()
        .map_or(f.parameters.range().end(), |r| r.range().end());
    let colon = module
        .parsed
        .tokens()
        .iter()
        .find(|t| t.kind() == TokenKind::Colon && t.range().start() >= header_end)?
        .range()
        .start();
    let token = module
        .parsed
        .tokens()
        .iter()
        .find(|t| t.kind() == TokenKind::Comment && t.range().start() > colon)?;
    let tail = typecomments::strip_prefix(&module.source[token.range()])?;
    (!typecomments::is_ignore(tail)).then_some(tail)
}
