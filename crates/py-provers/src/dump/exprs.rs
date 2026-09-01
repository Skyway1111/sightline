//! `_fields` for an expression and for a match pattern.

use super::fields::*;
use super::*;

pub(super) fn expr_fields<'a>(e: &'a Expr, module: &'a Module<'a>) -> Vec<Row<'a>> {
    let mut out: Vec<Row<'a>> = Vec::new();
    match e {
        Expr::BoolOp(n) => {
            out.push(("op", Part::Bare(boolop(n.op))));
            push_list(&mut out, "values", children(n.values.iter(), Cn::Expr));
        }
        Expr::Named(n) => {
            out.push(("target", Part::Child(Cn::Expr(&n.target))));
            out.push(("value", Part::Child(Cn::Expr(&n.value))));
        }
        Expr::BinOp(n) => {
            out.push(("left", Part::Child(Cn::Expr(&n.left))));
            out.push(("op", Part::Bare(operator(n.op))));
            out.push(("right", Part::Child(Cn::Expr(&n.right))));
        }
        Expr::UnaryOp(n) => {
            out.push(("op", Part::Bare(unaryop(n.op))));
            out.push(("operand", Part::Child(Cn::Expr(&n.operand))));
        }
        // CPython always has an `arguments` node; ruff drops it for a bare
        // `lambda: 0`.
        Expr::Lambda(n) => {
            out.push((
                "args",
                match &n.parameters {
                    Some(p) => Part::Child(Cn::Params(p)),
                    None => Part::Bare("arguments"),
                },
            ));
            out.push(("body", Part::Child(Cn::Expr(&n.body))));
        }
        Expr::If(n) => {
            out.push(("test", Part::Child(Cn::Expr(&n.test))));
            out.push(("body", Part::Child(Cn::Expr(&n.body))));
            out.push(("orelse", Part::Child(Cn::Expr(&n.orelse))));
        }
        Expr::Dict(n) => {
            push_list(
                &mut out,
                "keys",
                n.items
                    .iter()
                    .map(|i| match &i.key {
                        Some(k) => Part::Child(Cn::Expr(k)),
                        None => Part::Raw("None".to_string()),
                    })
                    .collect(),
            );
            push_list(
                &mut out,
                "values",
                children(n.items.iter().map(|i| &i.value), Cn::Expr),
            );
        }
        Expr::Set(n) => push_list(&mut out, "elts", children(n.elts.iter(), Cn::Expr)),
        Expr::ListComp(n) => {
            out.push(("elt", Part::Child(Cn::Expr(&n.elt))));
            push_list(&mut out, "generators", children(&n.generators, Cn::Comp));
        }
        Expr::SetComp(n) => {
            out.push(("elt", Part::Child(Cn::Expr(&n.elt))));
            push_list(&mut out, "generators", children(&n.generators, Cn::Comp));
        }
        Expr::DictComp(n) => {
            if let Some(k) = &n.key {
                out.push(("key", Part::Child(Cn::Expr(k))));
            }
            out.push(("value", Part::Child(Cn::Expr(&n.value))));
            push_list(&mut out, "generators", children(&n.generators, Cn::Comp));
        }
        Expr::Generator(n) => {
            out.push(("elt", Part::Child(Cn::Expr(&n.elt))));
            push_list(&mut out, "generators", children(&n.generators, Cn::Comp));
        }
        Expr::Await(n) => out.push(("value", Part::Child(Cn::Expr(&n.value)))),
        Expr::Yield(n) => {
            if let Some(v) = &n.value {
                out.push(("value", Part::Child(Cn::Expr(v))));
            }
        }
        Expr::YieldFrom(n) => out.push(("value", Part::Child(Cn::Expr(&n.value)))),
        Expr::Compare(n) => {
            out.push(("left", Part::Child(Cn::Expr(&n.left))));
            push_list(
                &mut out,
                "ops",
                n.ops.iter().map(|o| Part::Bare(cmpop(*o))).collect(),
            );
            push_list(
                &mut out,
                "comparators",
                children(n.comparators.iter(), Cn::Expr),
            );
        }
        Expr::Call(n) => {
            out.push(("func", Part::Child(Cn::Expr(&n.func))));
            let parens = n.arguments.range();
            push_list(
                &mut out,
                "args",
                n.arguments
                    .args
                    .iter()
                    .map(|a| match a {
                        Expr::Generator(g) if !g.parenthesized => {
                            Part::Child(Cn::CallGen(g, parens))
                        }
                        other => Part::Child(Cn::Expr(other)),
                    })
                    .collect(),
            );
            push_list(
                &mut out,
                "keywords",
                n.arguments
                    .keywords
                    .iter()
                    .map(|k| Part::Child(Cn::Keyword(k)))
                    .collect(),
            );
        }
        Expr::FString(_) | Expr::TString(_) => push_list(&mut out, "values", folded(Cn::Expr(e))),
        Expr::Attribute(n) => {
            out.push(("value", Part::Child(Cn::Expr(&n.value))));
            out.push(("attr", text(n.attr.as_str())));
            out.push(("ctx", Part::Ctx(ctx_name(n.ctx))));
        }
        Expr::Subscript(n) => {
            out.push(("value", Part::Child(Cn::Expr(&n.value))));
            out.push(("slice", Part::Child(Cn::Expr(&n.slice))));
            out.push(("ctx", Part::Ctx(ctx_name(n.ctx))));
        }
        Expr::Starred(n) => {
            out.push(("value", Part::Child(Cn::Expr(&n.value))));
            out.push(("ctx", Part::Ctx(ctx_name(n.ctx))));
        }
        Expr::Name(n) => {
            out.push(("id", text(n.id.as_str())));
            out.push(("ctx", Part::Ctx(ctx_name(n.ctx))));
        }
        Expr::List(n) => {
            push_list(&mut out, "elts", children(n.elts.iter(), Cn::Expr));
            out.push(("ctx", Part::Ctx(ctx_name(n.ctx))));
        }
        Expr::Tuple(n) => {
            push_list(&mut out, "elts", children(n.elts.iter(), Cn::Expr));
            out.push(("ctx", Part::Ctx(ctx_name(n.ctx))));
        }
        Expr::Slice(n) => {
            if let Some(x) = &n.lower {
                out.push(("lower", Part::Child(Cn::Expr(x))));
            }
            if let Some(x) = &n.upper {
                out.push(("upper", Part::Child(Cn::Expr(x))));
            }
            if let Some(x) = &n.step {
                out.push(("step", Part::Child(Cn::Expr(x))));
            }
        }
        _ => out.push(("value", Part::Raw(constant(e, module)))),
    }
    out
}

pub(super) fn pattern_fields(p: &Pattern) -> Vec<Row<'_>> {
    let mut out: Vec<Row<'_>> = Vec::new();
    match p {
        Pattern::MatchValue(n) => out.push(("value", Part::Child(Cn::Expr(&n.value)))),
        Pattern::MatchSingleton(n) => out.push((
            "value",
            Part::Raw(
                match n.value {
                    Singleton::None => "None",
                    Singleton::True => "True",
                    Singleton::False => "False",
                }
                .to_string(),
            ),
        )),
        Pattern::MatchSequence(n) => {
            push_list(&mut out, "patterns", children(&n.patterns, Cn::Pattern))
        }
        Pattern::MatchMapping(n) => {
            push_list(&mut out, "keys", children(n.keys.iter(), Cn::Expr));
            push_list(&mut out, "patterns", children(&n.patterns, Cn::Pattern));
            if let Some(rest) = &n.rest {
                out.push(("rest", text(rest.as_str())));
            }
        }
        Pattern::MatchClass(n) => {
            out.push(("cls", Part::Child(Cn::Expr(&n.cls))));
            push_list(
                &mut out,
                "patterns",
                children(&n.arguments.patterns, Cn::Pattern),
            );
            push_list(
                &mut out,
                "kwd_attrs",
                n.arguments
                    .keywords
                    .iter()
                    .map(|k| text(k.attr.as_str()))
                    .collect(),
            );
            push_list(
                &mut out,
                "kwd_patterns",
                n.arguments
                    .keywords
                    .iter()
                    .map(|k| Part::Child(Cn::Pattern(&k.pattern)))
                    .collect(),
            );
        }
        Pattern::MatchStar(n) => {
            if let Some(name) = &n.name {
                out.push(("name", text(name.as_str())));
            }
        }
        Pattern::MatchAs(n) => {
            if let Some(inner) = &n.pattern {
                out.push(("pattern", Part::Child(Cn::Pattern(inner))));
            }
            if let Some(name) = &n.name {
                out.push(("name", text(name.as_str())));
            }
        }
        Pattern::MatchOr(n) => push_list(&mut out, "patterns", children(&n.patterns, Cn::Pattern)),
    }
    out
}
