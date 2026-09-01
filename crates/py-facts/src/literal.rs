//! `ast.literal_eval` for the subset #33's sentinel arm reads (R12).

use ruff_python_ast::{Expr, Number, UnaryOp};

/// R12: the `ast.literal_eval` subset #33's sentinel arm needs. Anything else,
/// an `Ellipsis` and a complex number included, is `Computed`.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i128),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    Bool(bool),
    None,
    Tuple(Vec<Literal>),
    List(Vec<Literal>),
    Set(Vec<Literal>),
    Dict(Vec<(Literal, Literal)>),
    Computed,
}

pub fn literal(value: &Expr) -> Literal {
    match value {
        Expr::StringLiteral(s) => Literal::Str(s.value.to_str().to_string()),
        Expr::BytesLiteral(b) => Literal::Bytes(b.value.bytes().collect()),
        Expr::BooleanLiteral(b) => Literal::Bool(b.value),
        Expr::NoneLiteral(_) => Literal::None,
        Expr::NumberLiteral(n) => number(&n.value, 1),
        Expr::UnaryOp(u) => match (u.op, &*u.operand) {
            (UnaryOp::USub, Expr::NumberLiteral(n)) => number(&n.value, -1),
            (UnaryOp::UAdd, Expr::NumberLiteral(n)) => number(&n.value, 1),
            _ => Literal::Computed,
        },
        Expr::Tuple(t) => sequence(&t.elts).map_or(Literal::Computed, Literal::Tuple),
        Expr::List(l) => sequence(&l.elts).map_or(Literal::Computed, Literal::List),
        Expr::Set(s) => sequence(&s.elts).map_or(Literal::Computed, Literal::Set),
        // `literal_eval` reads a bare `set()`, the only spelling of the empty
        // set, and no other call.
        Expr::Call(c)
            if c.arguments.args.is_empty()
                && c.arguments.keywords.is_empty()
                && matches!(&*c.func, Expr::Name(n) if n.id.as_str() == "set") =>
        {
            Literal::Set(Vec::new())
        }
        Expr::Dict(d) => {
            let mut items = Vec::with_capacity(d.items.len());
            for item in &d.items {
                // `{**x}` has no key: `literal_eval` rejects the whole dict.
                let Some(key) = &item.key else {
                    return Literal::Computed;
                };
                match (literal(key), literal(&item.value)) {
                    (Literal::Computed, _) | (_, Literal::Computed) => return Literal::Computed,
                    (k, v) => items.push((k, v)),
                }
            }
            Literal::Dict(items)
        }
        _ => Literal::Computed,
    }
}

fn number(n: &Number, sign: i128) -> Literal {
    match n {
        // Past `u64::MAX` ruff keeps the token text, which is outside R12.
        Number::Int(i) => i
            .as_u64()
            .map_or(Literal::Computed, |v| Literal::Int(sign * i128::from(v))),
        Number::Float(f) => Literal::Float(sign as f64 * f),
        Number::Complex { .. } => Literal::Computed,
    }
}

fn sequence(elts: &[Expr]) -> Option<Vec<Literal>> {
    elts.iter()
        .map(|e| match literal(e) {
            Literal::Computed => None,
            other => Some(other),
        })
        .collect()
}
