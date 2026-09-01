//! `_fields` for a module, a statement and every node kind that is
//! neither an expression nor a pattern (`dump.rs`'s own table, split
//! at its banner).

use super::*;

pub(super) type Row<'a> = (&'static str, Part<'a>);

pub(super) fn text(value: &str) -> Part<'static> {
    Part::Raw(pytext::repr_str(value))
}

/// A list field: absent when empty, as `show_empty=False` has it.
pub(super) fn push_list<'a>(out: &mut Vec<Row<'a>>, name: &'static str, items: Vec<Part<'a>>) {
    if !items.is_empty() {
        out.push((name, Part::List(items)));
    }
}

/// A list field's children, each wrapped as the node kind `wrap` names.
pub(super) fn children<'a, T: 'a>(
    items: impl IntoIterator<Item = &'a T>,
    wrap: fn(&'a T) -> Cn<'a>,
) -> Vec<Part<'a>> {
    items.into_iter().map(|x| Part::Child(wrap(x))).collect()
}

pub(super) fn type_params(tp: Option<&TypeParams>) -> Vec<Part<'_>> {
    tp.map_or_else(Vec::new, |tp| {
        tp.type_params
            .iter()
            .map(|t| Part::Child(Cn::TypeParam(t)))
            .collect()
    })
}

pub(super) fn param(a: &ParameterWithDefault) -> Part<'_> {
    Part::Child(Cn::Param(&a.parameter))
}

/// The `orelse` of an `if`: the next clause is either the `elif` CPython
/// nests as one `If`, or the `else` body.
pub(super) fn orelse(rest: &[ElifElseClause]) -> Vec<Part<'_>> {
    match rest.first() {
        None => Vec::new(),
        Some(next) if next.test.is_some() => vec![Part::Child(Cn::Elif(rest))],
        Some(next) => children(&next.body, Cn::Stmt),
    }
}

/// The `values` of a `JoinedStr` / `TemplateStr` / format spec: the folded
/// literal runs and the interpolations, in `order::children`'s reading.
pub(super) fn folded(node: Cn<'_>) -> Vec<Part<'_>> {
    let mut kids = Vec::new();
    order::children(node, &mut kids);
    kids.into_iter().map(Part::Child).collect()
}

/// Every field of `node` that survives `ast.dump`'s omission rules: an absent
/// field, a `None` whose class default is `None`, and an empty list are
/// dropped.
pub(super) fn fields<'a>(node: Cn<'a>, module: &'a Module<'a>) -> Vec<Row<'a>> {
    let mut out: Vec<Row<'a>> = Vec::new();
    match node {
        // `Module.type_ignores` is the index's, never a shape: no clone
        // member is a module.
        Cn::Module(m) => push_list(&mut out, "body", children(&m.body, Cn::Stmt)),
        Cn::Stmt(s) => return stmt_fields(s, node, module),
        Cn::Elif(rest) => {
            let clause = &rest[0];
            if let Some(test) = &clause.test {
                out.push(("test", Part::Child(Cn::Expr(test))));
            }
            push_list(&mut out, "body", children(&clause.body, Cn::Stmt));
            push_list(&mut out, "orelse", orelse(&rest[1..]));
        }
        Cn::Expr(e) => return expr_fields(e, module),
        Cn::Params(p) => {
            push_list(
                &mut out,
                "posonlyargs",
                p.posonlyargs.iter().map(param).collect(),
            );
            push_list(&mut out, "args", p.args.iter().map(param).collect());
            if let Some(v) = &p.vararg {
                out.push(("vararg", Part::Child(Cn::Param(v))));
            }
            push_list(
                &mut out,
                "kwonlyargs",
                p.kwonlyargs.iter().map(param).collect(),
            );
            push_list(
                &mut out,
                "kw_defaults",
                p.kwonlyargs
                    .iter()
                    .map(|a| match &a.default {
                        Some(d) => Part::Child(Cn::Expr(d)),
                        None => Part::Raw("None".to_string()),
                    })
                    .collect(),
            );
            if let Some(k) = &p.kwarg {
                out.push(("kwarg", Part::Child(Cn::Param(k))));
            }
            push_list(
                &mut out,
                "defaults",
                p.posonlyargs
                    .iter()
                    .chain(p.args.iter())
                    .filter_map(|a| a.default.as_deref())
                    .map(|d| Part::Child(Cn::Expr(d)))
                    .collect(),
            );
        }
        Cn::Param(p) => {
            out.push(("arg", text(p.name.as_str())));
            let lifted = node.stamped().and_then(|at| module.annotation(at));
            if let Some(a) = lifted.or(p.annotation.as_deref()) {
                out.push(("annotation", Part::Child(Cn::Expr(a))));
            }
            if let Some(t) = same_line_comment(module, p.range().end()) {
                out.push(("type_comment", text(t)));
            }
        }
        Cn::Handler(h) => {
            if let Some(t) = &h.type_ {
                out.push(("type", Part::Child(Cn::Expr(t))));
            }
            if let Some(name) = &h.name {
                out.push(("name", text(name.as_str())));
            }
            push_list(&mut out, "body", children(&h.body, Cn::Stmt));
        }
        Cn::Comp(c) => {
            out.push(("target", Part::Child(Cn::Expr(&c.target))));
            out.push(("iter", Part::Child(Cn::Expr(&c.iter))));
            push_list(&mut out, "ifs", children(c.ifs.iter(), Cn::Expr));
            out.push(("is_async", Part::Raw(u32::from(c.is_async).to_string())));
        }
        Cn::Item(w) => {
            out.push(("context_expr", Part::Child(Cn::Expr(&w.context_expr))));
            if let Some(v) = &w.optional_vars {
                out.push(("optional_vars", Part::Child(Cn::Expr(v))));
            }
        }
        Cn::Case(c) => {
            out.push(("pattern", Part::Child(Cn::Pattern(&c.pattern))));
            if let Some(g) = &c.guard {
                out.push(("guard", Part::Child(Cn::Expr(g))));
            }
            push_list(&mut out, "body", children(&c.body, Cn::Stmt));
        }
        Cn::Pattern(p) => return pattern_fields(p),
        Cn::TypeParam(t) => {
            let (name, bound, default) = match t {
                TypeParam::TypeVar(n) => (&n.name, n.bound.as_deref(), n.default.as_deref()),
                TypeParam::ParamSpec(n) => (&n.name, None, n.default.as_deref()),
                TypeParam::TypeVarTuple(n) => (&n.name, None, n.default.as_deref()),
            };
            out.push(("name", text(name.as_str())));
            if let Some(b) = bound {
                out.push(("bound", Part::Child(Cn::Expr(b))));
            }
            if let Some(d) = default {
                out.push(("default_value", Part::Child(Cn::Expr(d))));
            }
        }
        Cn::Alias(a) => {
            out.push(("name", text(a.name.as_str())));
            if let Some(asname) = &a.asname {
                out.push(("asname", text(asname.as_str())));
            }
        }
        Cn::Keyword(k) => {
            if let Some(arg) = &k.arg {
                out.push(("arg", text(arg.as_str())));
            }
            out.push(("value", Part::Child(Cn::Expr(&k.value))));
        }
        // The cooked text of a run spanning several chunks is phase 5's: the
        // blind reading, which every phase-3 layer takes, blinds it.
        Cn::FConst { owner, .. } => out.push(("value", text(owner.map_or("", |l| &l.value)))),
        Cn::Interp(i, template) => {
            out.push(("value", Part::Child(Cn::Expr(&i.expression))));
            if template {
                out.push(("str", text(&module.source[i.expression.range()])));
            }
            out.push(("conversion", Part::Raw((i.conversion as i8).to_string())));
            if let Some(spec) = &i.format_spec {
                out.push(("format_spec", Part::Child(Cn::Spec(spec))));
            }
        }
        Cn::Spec(_) => push_list(&mut out, "values", folded(node)),
        Cn::CallGen(g, _) => {
            out.push(("elt", Part::Child(Cn::Expr(&g.elt))));
            push_list(&mut out, "generators", children(&g.generators, Cn::Comp));
        }
        Cn::TypeIgnore(line) => {
            out.push(("lineno", Part::Raw(line.to_string())));
            out.push(("tag", text("")));
        }
    }
    out
}

pub(super) fn stmt_fields<'a>(s: &'a Stmt, node: Cn<'a>, module: &'a Module<'a>) -> Vec<Row<'a>> {
    let mut out: Vec<Row<'a>> = Vec::new();
    match s {
        Stmt::FunctionDef(n) => {
            out.push(("name", text(n.name.as_str())));
            out.push(("args", Part::Child(Cn::Params(&n.parameters))));
            push_list(&mut out, "body", children(&n.body, Cn::Stmt));
            push_list(
                &mut out,
                "decorator_list",
                children(n.decorator_list.iter().map(|d| &d.expression), Cn::Expr),
            );
            let lifted = node.stamped().and_then(|at| module.returns(at));
            if let Some(r) = lifted.or(n.returns.as_deref()) {
                out.push(("returns", Part::Child(Cn::Expr(r))));
            }
            if let Some(t) = signature_comment(module, n) {
                out.push(("type_comment", text(t)));
            }
            push_list(
                &mut out,
                "type_params",
                type_params(n.type_params.as_deref()),
            );
        }
        Stmt::ClassDef(n) => {
            out.push(("name", text(n.name.as_str())));
            if let Some(args) = &n.arguments {
                push_list(&mut out, "bases", children(args.args.iter(), Cn::Expr));
                push_list(
                    &mut out,
                    "keywords",
                    args.keywords
                        .iter()
                        .map(|k| Part::Child(Cn::Keyword(k)))
                        .collect(),
                );
            }
            push_list(&mut out, "body", children(&n.body, Cn::Stmt));
            push_list(
                &mut out,
                "decorator_list",
                children(n.decorator_list.iter().map(|d| &d.expression), Cn::Expr),
            );
            push_list(
                &mut out,
                "type_params",
                type_params(n.type_params.as_deref()),
            );
        }
        Stmt::Return(n) => {
            if let Some(v) = &n.value {
                out.push(("value", Part::Child(Cn::Expr(v))));
            }
        }
        Stmt::Delete(n) => push_list(&mut out, "targets", children(n.targets.iter(), Cn::Expr)),
        Stmt::Assign(n) => {
            push_list(&mut out, "targets", children(n.targets.iter(), Cn::Expr));
            out.push(("value", Part::Child(Cn::Expr(&n.value))));
            if let Some(t) = assign_comment(module, s.range().end()) {
                out.push(("type_comment", text(t)));
            }
        }
        Stmt::TypeAlias(n) => {
            out.push(("name", Part::Child(Cn::Expr(&n.name))));
            push_list(
                &mut out,
                "type_params",
                type_params(n.type_params.as_deref()),
            );
            out.push(("value", Part::Child(Cn::Expr(&n.value))));
        }
        Stmt::AugAssign(n) => {
            out.push(("target", Part::Child(Cn::Expr(&n.target))));
            out.push(("op", Part::Bare(operator(n.op))));
            out.push(("value", Part::Child(Cn::Expr(&n.value))));
        }
        Stmt::AnnAssign(n) => {
            out.push(("target", Part::Child(Cn::Expr(&n.target))));
            out.push(("annotation", Part::Child(Cn::Expr(&n.annotation))));
            if let Some(v) = &n.value {
                out.push(("value", Part::Child(Cn::Expr(v))));
            }
            out.push(("simple", Part::Raw(u32::from(n.simple).to_string())));
        }
        Stmt::For(n) => {
            out.push(("target", Part::Child(Cn::Expr(&n.target))));
            out.push(("iter", Part::Child(Cn::Expr(&n.iter))));
            push_list(&mut out, "body", children(&n.body, Cn::Stmt));
            push_list(&mut out, "orelse", children(&n.orelse, Cn::Stmt));
        }
        Stmt::While(n) => {
            out.push(("test", Part::Child(Cn::Expr(&n.test))));
            push_list(&mut out, "body", children(&n.body, Cn::Stmt));
            push_list(&mut out, "orelse", children(&n.orelse, Cn::Stmt));
        }
        Stmt::If(n) => {
            out.push(("test", Part::Child(Cn::Expr(&n.test))));
            push_list(&mut out, "body", children(&n.body, Cn::Stmt));
            push_list(&mut out, "orelse", orelse(&n.elif_else_clauses));
        }
        Stmt::With(n) => {
            push_list(
                &mut out,
                "items",
                n.items.iter().map(|w| Part::Child(Cn::Item(w))).collect(),
            );
            push_list(&mut out, "body", children(&n.body, Cn::Stmt));
        }
        Stmt::Match(n) => {
            out.push(("subject", Part::Child(Cn::Expr(&n.subject))));
            push_list(
                &mut out,
                "cases",
                n.cases.iter().map(|c| Part::Child(Cn::Case(c))).collect(),
            );
        }
        Stmt::Raise(n) => {
            if let Some(e) = &n.exc {
                out.push(("exc", Part::Child(Cn::Expr(e))));
            }
            if let Some(c) = &n.cause {
                out.push(("cause", Part::Child(Cn::Expr(c))));
            }
        }
        Stmt::Try(n) => {
            push_list(&mut out, "body", children(&n.body, Cn::Stmt));
            push_list(
                &mut out,
                "handlers",
                n.handlers
                    .iter()
                    .map(|h| {
                        let ExceptHandler::ExceptHandler(h) = h;
                        Part::Child(Cn::Handler(h))
                    })
                    .collect(),
            );
            push_list(&mut out, "orelse", children(&n.orelse, Cn::Stmt));
            push_list(&mut out, "finalbody", children(&n.finalbody, Cn::Stmt));
        }
        Stmt::Assert(n) => {
            out.push(("test", Part::Child(Cn::Expr(&n.test))));
            if let Some(m) = &n.msg {
                out.push(("msg", Part::Child(Cn::Expr(m))));
            }
        }
        Stmt::Import(n) => push_list(
            &mut out,
            "names",
            n.names.iter().map(|a| Part::Child(Cn::Alias(a))).collect(),
        ),
        Stmt::ImportFrom(n) => {
            if let Some(m) = &n.module {
                out.push(("module", text(m.as_str())));
            }
            push_list(
                &mut out,
                "names",
                n.names.iter().map(|a| Part::Child(Cn::Alias(a))).collect(),
            );
            out.push(("level", Part::Raw(n.level.to_string())));
        }
        Stmt::Global(n) => push_list(
            &mut out,
            "names",
            n.names.iter().map(|x| text(x.as_str())).collect(),
        ),
        Stmt::Nonlocal(n) => push_list(
            &mut out,
            "names",
            n.names.iter().map(|x| text(x.as_str())).collect(),
        ),
        Stmt::Expr(n) => out.push(("value", Part::Child(Cn::Expr(&n.value)))),
        Stmt::Pass(_) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::IpyEscapeCommand(_) => {}
    }
    out
}
