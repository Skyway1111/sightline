//! CPython's `ast.iter_child_nodes` order over ruff's tree.
//!
//! `Cn` is one CPython node. Ruff nodes with no CPython class (`Decorator`,
//! `ParameterWithDefault`, `Arguments`, `TypeParams`, `Identifier`,
//! `ElifElseClause`, string parts) are never a `Cn`: they are expanded where
//! their parent lists them. Two CPython nodes have no ruff node at all, so
//! they are synthesized here: the `If` an `elif` becomes, and the `Constant`
//! a run of adjacent f-string literal chunks becomes.

use ruff_python_ast::{
    ElifElseClause, ExceptHandler, Expr, FStringPart, InterpolatedStringElement,
    InterpolatedStringElements, InterpolatedStringLiteralElement, Pattern, Stmt, TypeParam,
};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::cn::Cn;

/// Push `node`'s children in CPython's `_fields` order.
pub fn children<'a>(node: Cn<'a>, out: &mut Vec<Cn<'a>>) {
    match node {
        Cn::Module(m) => out.extend(m.body.iter().map(Cn::Stmt)),
        Cn::Stmt(s) => stmt_children(s, out),
        Cn::Elif(rest) => {
            let clause = &rest[0];
            if let Some(test) = &clause.test {
                out.push(Cn::Expr(test));
            }
            out.extend(clause.body.iter().map(Cn::Stmt));
            orelse(&rest[1..], out);
        }
        Cn::Expr(e) => expr_children(e, out),
        Cn::Params(p) => {
            for arg in p.posonlyargs.iter().chain(p.args.iter()) {
                out.push(Cn::Param(&arg.parameter));
            }
            if let Some(v) = &p.vararg {
                out.push(Cn::Param(v));
            }
            for arg in &p.kwonlyargs {
                out.push(Cn::Param(&arg.parameter));
            }
            for arg in &p.kwonlyargs {
                if let Some(d) = &arg.default {
                    out.push(Cn::Expr(d));
                }
            }
            if let Some(k) = &p.kwarg {
                out.push(Cn::Param(k));
            }
            for arg in p.posonlyargs.iter().chain(p.args.iter()) {
                if let Some(d) = &arg.default {
                    out.push(Cn::Expr(d));
                }
            }
        }
        Cn::Param(p) => out.extend(p.annotation.iter().map(|a| Cn::Expr(a))),
        Cn::Handler(h) => {
            out.extend(h.type_.iter().map(|t| Cn::Expr(t)));
            out.extend(h.body.iter().map(Cn::Stmt));
        }
        Cn::Comp(c) => {
            out.push(Cn::Expr(&c.target));
            out.push(Cn::Expr(&c.iter));
            out.extend(c.ifs.iter().map(Cn::Expr));
        }
        Cn::Item(w) => {
            out.push(Cn::Expr(&w.context_expr));
            out.extend(w.optional_vars.iter().map(|v| Cn::Expr(v)));
        }
        Cn::Case(c) => {
            out.push(Cn::Pattern(&c.pattern));
            out.extend(c.guard.iter().map(|g| Cn::Expr(g)));
            out.extend(c.body.iter().map(Cn::Stmt));
        }
        Cn::Pattern(p) => pattern_children(p, out),
        Cn::TypeParam(t) => match t {
            TypeParam::TypeVar(n) => {
                out.extend(n.bound.iter().map(|b| Cn::Expr(b)));
                out.extend(n.default.iter().map(|d| Cn::Expr(d)));
            }
            TypeParam::ParamSpec(n) => out.extend(n.default.iter().map(|d| Cn::Expr(d))),
            TypeParam::TypeVarTuple(n) => out.extend(n.default.iter().map(|d| Cn::Expr(d))),
        },
        Cn::Alias(_) | Cn::TypeIgnore(_) | Cn::FConst { .. } => {}
        Cn::CallGen(g, _) => {
            out.push(Cn::Expr(&g.elt));
            out.extend(g.generators.iter().map(Cn::Comp));
        }
        Cn::Keyword(k) => out.push(Cn::Expr(&k.value)),
        Cn::Interp(i, template) => {
            out.push(Cn::Expr(&i.expression));
            if let Some(spec) = &i.format_spec {
                let _ = template;
                out.push(Cn::Spec(spec));
            }
        }
        Cn::Spec(s) => {
            let mut run = Run::default();
            run.elements(&s.elements, false, out);
            run.flush(out);
        }
    }
}

/// The `orelse` of an `if`: the next clause is either the `elif` CPython
/// nests as one `If`, or the `else` body.
fn orelse<'a>(rest: &'a [ElifElseClause], out: &mut Vec<Cn<'a>>) {
    let Some(next) = rest.first() else { return };
    if next.test.is_some() {
        out.push(Cn::Elif(rest));
    } else {
        out.extend(next.body.iter().map(Cn::Stmt));
    }
}

fn stmt_children<'a>(s: &'a Stmt, out: &mut Vec<Cn<'a>>) {
    match s {
        Stmt::FunctionDef(n) => {
            out.push(Cn::Params(&n.parameters));
            out.extend(n.body.iter().map(Cn::Stmt));
            out.extend(n.decorator_list.iter().map(|d| Cn::Expr(&d.expression)));
            out.extend(n.returns.iter().map(|r| Cn::Expr(r)));
            type_params(n.type_params.as_deref(), out);
        }
        Stmt::ClassDef(n) => {
            if let Some(args) = &n.arguments {
                out.extend(args.args.iter().map(Cn::Expr));
                out.extend(args.keywords.iter().map(Cn::Keyword));
            }
            out.extend(n.body.iter().map(Cn::Stmt));
            out.extend(n.decorator_list.iter().map(|d| Cn::Expr(&d.expression)));
            type_params(n.type_params.as_deref(), out);
        }
        Stmt::Return(n) => out.extend(n.value.iter().map(|v| Cn::Expr(v))),
        Stmt::Delete(n) => out.extend(n.targets.iter().map(Cn::Expr)),
        Stmt::TypeAlias(n) => {
            out.push(Cn::Expr(&n.name));
            type_params(n.type_params.as_deref(), out);
            out.push(Cn::Expr(&n.value));
        }
        Stmt::Assign(n) => {
            out.extend(n.targets.iter().map(Cn::Expr));
            out.push(Cn::Expr(&n.value));
        }
        Stmt::AugAssign(n) => {
            out.push(Cn::Expr(&n.target));
            out.push(Cn::Expr(&n.value));
        }
        Stmt::AnnAssign(n) => {
            out.push(Cn::Expr(&n.target));
            out.push(Cn::Expr(&n.annotation));
            out.extend(n.value.iter().map(|v| Cn::Expr(v)));
        }
        Stmt::For(n) => {
            out.push(Cn::Expr(&n.target));
            out.push(Cn::Expr(&n.iter));
            out.extend(n.body.iter().map(Cn::Stmt));
            out.extend(n.orelse.iter().map(Cn::Stmt));
        }
        Stmt::While(n) => {
            out.push(Cn::Expr(&n.test));
            out.extend(n.body.iter().map(Cn::Stmt));
            out.extend(n.orelse.iter().map(Cn::Stmt));
        }
        Stmt::If(n) => {
            out.push(Cn::Expr(&n.test));
            out.extend(n.body.iter().map(Cn::Stmt));
            orelse(&n.elif_else_clauses, out);
        }
        Stmt::With(n) => {
            out.extend(n.items.iter().map(Cn::Item));
            out.extend(n.body.iter().map(Cn::Stmt));
        }
        Stmt::Match(n) => {
            out.push(Cn::Expr(&n.subject));
            out.extend(n.cases.iter().map(Cn::Case));
        }
        Stmt::Raise(n) => {
            out.extend(n.exc.iter().map(|e| Cn::Expr(e)));
            out.extend(n.cause.iter().map(|c| Cn::Expr(c)));
        }
        Stmt::Try(n) => {
            out.extend(n.body.iter().map(Cn::Stmt));
            out.extend(n.handlers.iter().map(|h| {
                let ExceptHandler::ExceptHandler(h) = h;
                Cn::Handler(h)
            }));
            out.extend(n.orelse.iter().map(Cn::Stmt));
            out.extend(n.finalbody.iter().map(Cn::Stmt));
        }
        Stmt::Assert(n) => {
            out.push(Cn::Expr(&n.test));
            out.extend(n.msg.iter().map(|m| Cn::Expr(m)));
        }
        Stmt::Import(n) => out.extend(n.names.iter().map(Cn::Alias)),
        Stmt::ImportFrom(n) => out.extend(n.names.iter().map(Cn::Alias)),
        Stmt::Expr(n) => out.push(Cn::Expr(&n.value)),
        Stmt::Global(_)
        | Stmt::Nonlocal(_)
        | Stmt::Pass(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::IpyEscapeCommand(_) => {}
    }
}

fn type_params<'a>(tp: Option<&'a ruff_python_ast::TypeParams>, out: &mut Vec<Cn<'a>>) {
    if let Some(tp) = tp {
        out.extend(tp.type_params.iter().map(Cn::TypeParam));
    }
}

fn expr_children<'a>(e: &'a Expr, out: &mut Vec<Cn<'a>>) {
    match e {
        Expr::BoolOp(n) => out.extend(n.values.iter().map(Cn::Expr)),
        Expr::Named(n) => {
            out.push(Cn::Expr(&n.target));
            out.push(Cn::Expr(&n.value));
        }
        Expr::BinOp(n) => {
            out.push(Cn::Expr(&n.left));
            out.push(Cn::Expr(&n.right));
        }
        Expr::UnaryOp(n) => out.push(Cn::Expr(&n.operand)),
        Expr::Lambda(n) => {
            // CPython always has an `arguments` node; ruff drops it for a
            // bare `lambda: 0`. It has no position, so no bucket entry.
            if let Some(p) = &n.parameters {
                out.push(Cn::Params(p));
            }
            out.push(Cn::Expr(&n.body));
        }
        Expr::If(n) => {
            out.push(Cn::Expr(&n.test));
            out.push(Cn::Expr(&n.body));
            out.push(Cn::Expr(&n.orelse));
        }
        Expr::Dict(n) => {
            out.extend(n.items.iter().filter_map(|i| i.key.as_ref()).map(Cn::Expr));
            out.extend(n.items.iter().map(|i| Cn::Expr(&i.value)));
        }
        Expr::Set(n) => out.extend(n.elts.iter().map(Cn::Expr)),
        Expr::ListComp(n) => {
            out.push(Cn::Expr(&n.elt));
            out.extend(n.generators.iter().map(Cn::Comp));
        }
        Expr::SetComp(n) => {
            out.push(Cn::Expr(&n.elt));
            out.extend(n.generators.iter().map(Cn::Comp));
        }
        Expr::DictComp(n) => {
            out.extend(n.key.iter().map(|k| Cn::Expr(k)));
            out.push(Cn::Expr(&n.value));
            out.extend(n.generators.iter().map(Cn::Comp));
        }
        Expr::Generator(n) => {
            out.push(Cn::Expr(&n.elt));
            out.extend(n.generators.iter().map(Cn::Comp));
        }
        Expr::Await(n) => out.push(Cn::Expr(&n.value)),
        Expr::Yield(n) => out.extend(n.value.iter().map(|v| Cn::Expr(v))),
        Expr::YieldFrom(n) => out.push(Cn::Expr(&n.value)),
        Expr::Compare(n) => {
            out.push(Cn::Expr(&n.left));
            out.extend(n.comparators.iter().map(Cn::Expr));
        }
        Expr::Call(n) => {
            out.push(Cn::Expr(&n.func));
            let parens = n.arguments.range();
            out.extend(n.arguments.args.iter().map(|a| match a {
                Expr::Generator(g) if !g.parenthesized => Cn::CallGen(g, parens),
                other => Cn::Expr(other),
            }));
            out.extend(n.arguments.keywords.iter().map(Cn::Keyword));
        }
        Expr::FString(n) => {
            let mut run = Run::default();
            for part in n.value.iter() {
                match part {
                    FStringPart::Literal(s) => run.chunk(s.range(), None, out),
                    FStringPart::FString(f) => run.elements(&f.elements, false, out),
                }
            }
            run.flush(out);
        }
        Expr::TString(n) => {
            let mut run = Run::default();
            for part in n.value.iter() {
                run.elements(&part.elements, true, out);
            }
            run.flush(out);
        }
        Expr::Attribute(n) => out.push(Cn::Expr(&n.value)),
        Expr::Subscript(n) => {
            out.push(Cn::Expr(&n.value));
            out.push(Cn::Expr(&n.slice));
        }
        Expr::Starred(n) => out.push(Cn::Expr(&n.value)),
        Expr::List(n) => out.extend(n.elts.iter().map(Cn::Expr)),
        Expr::Tuple(n) => out.extend(n.elts.iter().map(Cn::Expr)),
        Expr::Slice(n) => {
            out.extend(n.lower.iter().map(|x| Cn::Expr(x)));
            out.extend(n.upper.iter().map(|x| Cn::Expr(x)));
            out.extend(n.step.iter().map(|x| Cn::Expr(x)));
        }
        Expr::Name(_)
        | Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::EllipsisLiteral(_)
        | Expr::IpyEscapeCommand(_) => {}
    }
}

fn pattern_children<'a>(p: &'a Pattern, out: &mut Vec<Cn<'a>>) {
    match p {
        Pattern::MatchValue(n) => out.push(Cn::Expr(&n.value)),
        Pattern::MatchSingleton(_) | Pattern::MatchStar(_) => {}
        Pattern::MatchSequence(n) => out.extend(n.patterns.iter().map(Cn::Pattern)),
        Pattern::MatchMapping(n) => {
            out.extend(n.keys.iter().map(Cn::Expr));
            out.extend(n.patterns.iter().map(Cn::Pattern));
        }
        Pattern::MatchClass(n) => {
            out.push(Cn::Expr(&n.cls));
            out.extend(n.arguments.patterns.iter().map(Cn::Pattern));
            out.extend(n.arguments.keywords.iter().map(|k| Cn::Pattern(&k.pattern)));
        }
        Pattern::MatchAs(n) => out.extend(n.pattern.iter().map(|x| Cn::Pattern(x))),
        Pattern::MatchOr(n) => out.extend(n.patterns.iter().map(Cn::Pattern)),
    }
}

/// CPython folds adjacent literal chunks of an f-string into one `Constant`
/// running from the first chunk to the last. A chunk is the interpolated
/// literal element for an f-string part and the whole token for a plain
/// string part; the debug text of `f"{x=}"` is a chunk too.
#[derive(Default)]
struct Run<'a> {
    open: Option<(TextRange, Option<&'a InterpolatedStringLiteralElement>)>,
}

impl<'a> Run<'a> {
    fn chunk(
        &mut self,
        range: TextRange,
        owner: Option<&'a InterpolatedStringLiteralElement>,
        _out: &mut Vec<Cn<'a>>,
    ) {
        match &mut self.open {
            Some((open, _)) => *open = TextRange::new(open.start(), range.end()),
            slot => *slot = Some((range, owner)),
        }
    }

    fn flush(&mut self, out: &mut Vec<Cn<'a>>) {
        if let Some((range, owner)) = self.open.take() {
            out.push(Cn::FConst { range, owner });
        }
    }

    fn elements(
        &mut self,
        elements: &'a InterpolatedStringElements,
        template: bool,
        out: &mut Vec<Cn<'a>>,
    ) {
        for element in elements.iter() {
            match element {
                InterpolatedStringElement::Literal(l) => self.chunk(l.range(), Some(l), out),
                InterpolatedStringElement::Interpolation(i) => {
                    if let Some(debug) = &i.debug_text {
                        let start = i.range().start() + TextSize::from(1);
                        let len = TextSize::try_from(debug.as_str().len()).unwrap_or_default();
                        self.chunk(TextRange::at(start, len), None, out);
                    }
                    self.flush(out);
                    out.push(Cn::Interp(i, template));
                }
            }
        }
    }
}
