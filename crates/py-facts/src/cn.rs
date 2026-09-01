//! `Cn`, one CPython node over ruff's tree: its class (`Kind`), its CPython
//! range (R1) and the `NodeIndex` slot facts stamp. The child order lives
//! in `order.rs`.

use crate::kinds::Kind;
use ruff_python_ast::{
    Alias, Comprehension, ElifElseClause, ExceptHandlerExceptHandler, Expr, ExprGenerator,
    HasNodeIndex, InterpolatedElement, InterpolatedStringFormatSpec,
    InterpolatedStringLiteralElement, Keyword, MatchCase, ModModule, NodeIndex, Parameter,
    Parameters, Pattern, Stmt, TypeParam, WithItem,
};
use ruff_python_trivia::{SimpleTokenKind, SimpleTokenizer};
use ruff_text_size::{Ranged, TextRange, TextSize};

#[derive(Clone, Copy)]
pub enum Cn<'a> {
    Module(&'a ModModule),
    Stmt(&'a Stmt),
    /// The CPython `If` an `elif` becomes: `rest[0]` is its clause, and the
    /// chain's last clause gives its end.
    Elif(&'a [ElifElseClause]),
    Expr(&'a Expr),
    Params(&'a Parameters),
    Param(&'a Parameter),
    Handler(&'a ExceptHandlerExceptHandler),
    Comp(&'a Comprehension),
    Item(&'a WithItem),
    Case(&'a MatchCase),
    Pattern(&'a Pattern),
    TypeParam(&'a TypeParam),
    Alias(&'a Alias),
    Keyword(&'a Keyword),
    /// A run of adjacent f-string literal chunks: one CPython `Constant`.
    /// `owner` is the first chunk when that chunk is a ruff node.
    FConst {
        range: TextRange,
        owner: Option<&'a InterpolatedStringLiteralElement>,
    },
    /// CPython `FormattedValue` (f-string) or `Interpolation` (t-string).
    Interp(&'a InterpolatedElement, bool),
    /// CPython `JoinedStr` for a format spec. Its range starts one byte
    /// earlier than ruff's: CPython counts the `:`.
    Spec(&'a InterpolatedStringFormatSpec),
    /// A generator expression written without parentheses of its own, as the
    /// sole argument of a call. CPython ranges it by the call's parentheses.
    CallGen(&'a ExprGenerator, TextRange),
    /// `Module.type_ignores[i]`, with only a line number.
    TypeIgnore(u32),
}

impl<'a> Cn<'a> {
    pub fn kind(self) -> Kind {
        match self {
            Cn::Module(_) => Kind::Module,
            Cn::Stmt(s) => stmt_kind(s),
            Cn::Elif(_) => Kind::If,
            Cn::Expr(e) => expr_kind(e),
            Cn::Params(_) => Kind::Arguments,
            Cn::Param(_) => Kind::Arg,
            Cn::Handler(_) => Kind::ExceptHandler,
            Cn::Comp(_) => Kind::Comprehension,
            Cn::Item(_) => Kind::WithItem,
            Cn::Case(_) => Kind::MatchCase,
            Cn::Pattern(p) => pattern_kind(p),
            Cn::TypeParam(t) => match t {
                TypeParam::TypeVar(_) => Kind::TypeVar,
                TypeParam::ParamSpec(_) => Kind::ParamSpec,
                TypeParam::TypeVarTuple(_) => Kind::TypeVarTuple,
            },
            Cn::Alias(_) => Kind::Alias,
            Cn::Keyword(_) => Kind::Keyword,
            Cn::FConst { .. } => Kind::Constant,
            Cn::Interp(_, template) => {
                if template {
                    Kind::Interpolation
                } else {
                    Kind::FormattedValue
                }
            }
            Cn::Spec(_) => Kind::JoinedStr,
            Cn::CallGen(..) => Kind::GeneratorExp,
            Cn::TypeIgnore(_) => Kind::TypeIgnore,
        }
    }

    /// The CPython range, `None` where CPython gives the class no `lineno`.
    /// `source` is read only for a decorated def, whose CPython range starts
    /// at the `def` / `async` / `class` keyword and ruff's at the first `@`.
    pub fn range(self, source: &str) -> Option<TextRange> {
        Some(match self {
            Cn::Module(_) | Cn::Params(_) | Cn::Comp(_) | Cn::Item(_) | Cn::Case(_) => return None,
            Cn::TypeIgnore(_) => return None,
            Cn::Stmt(s) => {
                let start = keyword_start(s, source).unwrap_or(s.range().start());
                let end = match s {
                    Stmt::Assign(_) => crate::typecomments::assign_end(s.range().end(), source)
                        .unwrap_or(s.range().end()),
                    _ => s.range().end(),
                };
                TextRange::new(start, end)
            }
            Cn::Elif(rest) => {
                TextRange::new(rest[0].range().start(), rest[rest.len() - 1].range().end())
            }
            Cn::Expr(e) => e.range(),
            Cn::CallGen(_, range) => range,
            // CPython's `arg` starts at the name; ruff's `Parameter` takes in
            // the `*` of a vararg and the `**` of a kwarg.
            Cn::Param(p) => TextRange::new(p.name.range().start(), p.range().end()),
            Cn::Handler(h) => h.range(),
            Cn::Pattern(p) => p.range(),
            Cn::TypeParam(t) => t.range(),
            Cn::Alias(a) => a.range(),
            Cn::Keyword(k) => k.range(),
            Cn::FConst { range, .. } => range,
            Cn::Interp(i, _) => i.range(),
            Cn::Spec(s) => TextRange::new(s.range().start() - TextSize::from(1), s.range().end()),
        })
    }

    /// Facts' dense index, stamped on the ruff node holding this CPython
    /// node. A synthesized `Constant` whose run opens on a plain string part
    /// has no ruff node of its own and is left unstamped.
    pub fn set_index(self, index: u32) {
        if let Some(slot) = self.index_slot() {
            slot.set(NodeIndex::from(index));
        }
    }

    /// The index read back off the ruff node, for the stamping cross-check.
    pub fn stamped(self) -> Option<u32> {
        self.index_slot().and_then(|s| s.load().as_u32())
    }

    fn index_slot(self) -> Option<&'a ruff_python_ast::AtomicNodeIndex> {
        Some(match self {
            Cn::Module(m) => m.node_index(),
            Cn::Stmt(s) => s.node_index(),
            Cn::Elif(rest) => rest[0].node_index(),
            Cn::Expr(e) => e.node_index(),
            Cn::Params(p) => p.node_index(),
            Cn::Param(p) => p.node_index(),
            Cn::Handler(h) => h.node_index(),
            Cn::Comp(c) => c.node_index(),
            Cn::Item(w) => w.node_index(),
            Cn::Case(c) => c.node_index(),
            Cn::Pattern(p) => p.node_index(),
            Cn::TypeParam(t) => t.node_index(),
            Cn::Alias(a) => a.node_index(),
            Cn::Keyword(k) => k.node_index(),
            Cn::Interp(i, _) => i.node_index(),
            Cn::Spec(s) => s.node_index(),
            Cn::CallGen(g, _) => g.node_index(),
            Cn::FConst { owner: Some(l), .. } => l.node_index(),
            Cn::FConst { owner: None, .. } | Cn::TypeIgnore(_) => return None,
        })
    }

    /// The def or class statement this node is, for the scope marks.
    pub fn def_key(self) -> Option<usize> {
        match self {
            Cn::Stmt(s @ (Stmt::FunctionDef(_) | Stmt::ClassDef(_))) => {
                Some(s as *const Stmt as usize)
            }
            _ => None,
        }
    }
}

/// The `def` / `async` / `class` keyword of a decorated def, which is where
/// CPython starts the node and ruff starts at the first decorator's `@`.
fn keyword_start(s: &Stmt, source: &str) -> Option<TextSize> {
    let decorators = match s {
        Stmt::FunctionDef(n) => &n.decorator_list,
        Stmt::ClassDef(n) => &n.decorator_list,
        _ => return None,
    };
    let last = decorators.last()?;
    SimpleTokenizer::starts_at(last.range().end(), source)
        .skip_trivia()
        .find(|t| {
            matches!(
                t.kind(),
                SimpleTokenKind::Async | SimpleTokenKind::Def | SimpleTokenKind::Class
            )
        })
        .map(|t| t.range().start())
}

fn either(flag: bool, yes: Kind, no: Kind) -> Kind {
    if flag { yes } else { no }
}

fn stmt_kind(s: &Stmt) -> Kind {
    match s {
        Stmt::FunctionDef(n) => either(n.is_async, Kind::AsyncFunctionDef, Kind::FunctionDef),
        Stmt::For(n) => either(n.is_async, Kind::AsyncFor, Kind::For),
        Stmt::With(n) => either(n.is_async, Kind::AsyncWith, Kind::With),
        Stmt::Try(n) => either(n.is_star, Kind::TryStar, Kind::Try),
        Stmt::ClassDef(_) => Kind::ClassDef,
        Stmt::Return(_) => Kind::Return,
        Stmt::Delete(_) => Kind::Delete,
        Stmt::TypeAlias(_) => Kind::TypeAlias,
        Stmt::Assign(_) => Kind::Assign,
        Stmt::AugAssign(_) => Kind::AugAssign,
        Stmt::AnnAssign(_) => Kind::AnnAssign,
        Stmt::While(_) => Kind::While,
        Stmt::If(_) => Kind::If,
        Stmt::Match(_) => Kind::Match,
        Stmt::Raise(_) => Kind::Raise,
        Stmt::Assert(_) => Kind::Assert,
        Stmt::Import(_) => Kind::Import,
        Stmt::ImportFrom(_) => Kind::ImportFrom,
        Stmt::Global(_) => Kind::Global,
        Stmt::Nonlocal(_) => Kind::Nonlocal,
        Stmt::Expr(_) => Kind::Expr,
        Stmt::Pass(_) => Kind::Pass,
        Stmt::Break(_) => Kind::Break,
        Stmt::Continue(_) => Kind::Continue,
        Stmt::IpyEscapeCommand(_) => Kind::Expr,
    }
}

fn expr_kind(e: &Expr) -> Kind {
    match e {
        Expr::BoolOp(_) => Kind::BoolOp,
        Expr::Named(_) => Kind::NamedExpr,
        Expr::BinOp(_) => Kind::BinOp,
        Expr::UnaryOp(_) => Kind::UnaryOp,
        Expr::Lambda(_) => Kind::Lambda,
        Expr::If(_) => Kind::IfExp,
        Expr::Dict(_) => Kind::Dict,
        Expr::Set(_) => Kind::Set,
        Expr::ListComp(_) => Kind::ListComp,
        Expr::SetComp(_) => Kind::SetComp,
        Expr::DictComp(_) => Kind::DictComp,
        Expr::Generator(_) => Kind::GeneratorExp,
        Expr::Await(_) => Kind::Await,
        Expr::Yield(_) => Kind::Yield,
        Expr::YieldFrom(_) => Kind::YieldFrom,
        Expr::Compare(_) => Kind::Compare,
        Expr::Call(_) => Kind::Call,
        Expr::FString(_) => Kind::JoinedStr,
        Expr::TString(_) => Kind::TemplateStr,
        Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_) => Kind::Constant,
        Expr::Attribute(_) => Kind::Attribute,
        Expr::Subscript(_) => Kind::Subscript,
        Expr::Starred(_) => Kind::Starred,
        Expr::Name(_) => Kind::Name,
        Expr::List(_) => Kind::List,
        Expr::Tuple(_) => Kind::Tuple,
        Expr::Slice(_) => Kind::Slice,
        Expr::IpyEscapeCommand(_) => Kind::Constant,
    }
}

fn pattern_kind(p: &Pattern) -> Kind {
    match p {
        Pattern::MatchValue(_) => Kind::MatchValue,
        Pattern::MatchSingleton(_) => Kind::MatchSingleton,
        Pattern::MatchSequence(_) => Kind::MatchSequence,
        Pattern::MatchMapping(_) => Kind::MatchMapping,
        Pattern::MatchClass(_) => Kind::MatchClass,
        Pattern::MatchStar(_) => Kind::MatchStar,
        Pattern::MatchAs(_) => Kind::MatchAs,
        Pattern::MatchOr(_) => Kind::MatchOr,
    }
}
