//! The Python half of SonarSource cognitive complexity: which node is an
//! increment and how deep it sits. `core::complexity::score` sums the tree.
//!
//! CPython walks `ast.iter_fields`, and only a node that sinks its blocks
//! (a nester, or a lambda) cares which field a child came from, so this port
//! spells the block fields of those nodes and reads every other node's
//! children off `order::children`.

use ruff_python_ast::{ElifElseClause, Expr, Stmt, StmtFunctionDef, TypeParams};
use sightline_core::complexity::{Cc, score};

use crate::astutil::RECEIVERS;
use crate::cn::Cn;
use crate::kinds::Kind;
use crate::order::{self};

/// `if`/loop/`except`/ternary/`match` nest their blocks one level in.
fn is_nester(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::If
            | Kind::For
            | Kind::AsyncFor
            | Kind::While
            | Kind::ExceptHandler
            | Kind::IfExp
            | Kind::Match
    )
}

fn is_scope(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::FunctionDef | Kind::AsyncFunctionDef | Kind::Lambda
    )
}

/// `x or ""`, `item.get(k) or {}`: a trailing literal is a default, not a
/// decision.
fn is_default(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Constant | Kind::List | Kind::Dict | Kind::Set | Kind::Tuple
    )
}

/// `name(...)`, or `self.name(...)` / `cls.name(...)`: a direct recursive call.
fn recurses(func: &Expr, name: Option<&str>) -> bool {
    match func {
        Expr::Attribute(a) => {
            Some(a.attr.as_str()) == name
                && matches!(&*a.value, Expr::Name(n) if RECEIVERS.contains(&n.id.as_str()))
        }
        Expr::Name(n) => Some(n.id.as_str()) == name,
        _ => false,
    }
}

/// CPython's `orelse` of an `If`, which ruff spells as clauses: a clause with
/// a test is the `If` an `elif` becomes, one without is the `else` body.
enum Orelse<'a> {
    Elif(&'a [ElifElseClause]),
    Else(&'a [Stmt]),
}

/// `None` where the `if` has no `elif` and no `else`.
fn orelse(clauses: &[ElifElseClause]) -> Option<Orelse<'_>> {
    let first = clauses.first()?;
    Some(if first.test.is_some() {
        Orelse::Elif(clauses)
    } else {
        Orelse::Else(&first.body)
    })
}

/// The `(test, body, orelse)` of an `If`, however it is spelled.
fn if_parts<'a>(node: Cn<'a>) -> Option<(Option<&'a Expr>, &'a [Stmt], Option<Orelse<'a>>)> {
    match node {
        Cn::Stmt(Stmt::If(n)) => Some((
            Some(&n.test),
            &n.body,
            orelse(n.elif_else_clauses.as_slice()),
        )),
        Cn::Elif(rest) => Some((rest[0].test.as_ref(), &rest[0].body, orelse(&rest[1..]))),
        _ => None,
    }
}

/// A brancher's `orelse`: a lone `if` there is an elif (+1 flat, its own
/// visit); anything else is an else (+1, nested one deeper). CPython cannot
/// tell `else:` holding one `if` from `elif`, and neither may the port.
fn cc_else(tail: &Orelse<'_>, name: Option<&str>) -> Cc {
    match tail {
        Orelse::Elif(rest) => classify(Cn::Elif(rest), name, false, true),
        Orelse::Else([only @ Stmt::If(_)]) => classify(Cn::Stmt(only), name, false, true),
        Orelse::Else(body) => Cc {
            flat: 1,
            kids: body.iter().map(|k| deep(Cn::Stmt(k), name)).collect(),
            ..Cc::default()
        },
    }
}

/// A `For` or `While` `orelse`, which CPython always reads as an `else`.
fn loop_else(body: &[Stmt], name: Option<&str>) -> Cc {
    Cc {
        flat: 1,
        kids: body.iter().map(|k| deep(Cn::Stmt(k), name)).collect(),
        ..Cc::default()
    }
}

fn deep(node: Cn<'_>, name: Option<&str>) -> Cc {
    classify(node, name, true, false)
}

fn flat(node: Cn<'_>, name: Option<&str>) -> Cc {
    classify(node, name, false, false)
}

/// A subtree scoring nothing is not in the tree: the score reads the
/// decisions, not the source.
fn keep(kids: &mut Vec<Cc>, c: Cc) {
    if c.flat != 0 || c.nests || !c.kids.is_empty() {
        kids.push(c);
    }
}

/// `complexity.py:_classify`. `name` is the enclosing def's, for the
/// recursive-call increment; `inner` puts this node one level in from its
/// parent; `elif_` makes it flat instead of nesting.
pub fn classify(node: Cn<'_>, name: Option<&str>, inner: bool, elif_: bool) -> Cc {
    let kind = node.kind();
    // a nested def scores as its own finding and not twice
    if matches!(kind, Kind::FunctionDef | Kind::AsyncFunctionDef) {
        return Cc::default();
    }
    let nester = is_nester(kind);
    let increment = if nester {
        u32::from(elif_)
    } else {
        match node {
            Cn::Expr(Expr::BoolOp(b)) => u32::from(
                b.values
                    .last()
                    .is_some_and(|v| !is_default(Cn::Expr(v).kind())),
            ),
            Cn::Expr(Expr::Call(c)) => u32::from(recurses(&c.func, name)),
            _ => 0,
        }
    };
    // a lambda sinks its body without scoring
    let name = if kind == Kind::Lambda { None } else { name };
    let mut kids: Vec<Cc> = Vec::new();
    if let Some((test, body, tail)) = if_parts(node) {
        if let Some(test) = test {
            keep(&mut kids, flat(Cn::Expr(test), name));
        }
        for st in body {
            keep(&mut kids, deep(Cn::Stmt(st), name));
        }
        if let Some(tail) = tail {
            keep(&mut kids, cc_else(&tail, name));
        }
    } else {
        blocks(node, name, &mut kids);
    }
    Cc {
        flat: increment,
        nests: nester && !elif_,
        inner,
        kids,
    }
}

/// Every child but an `If`'s, split into the fields CPython nests (`body`,
/// `orelse`, `finalbody`, `handlers`, `cases`) and the rest.
fn blocks(node: Cn<'_>, name: Option<&str>, kids: &mut Vec<Cc>) {
    match node {
        Cn::Stmt(Stmt::For(n)) => {
            keep(kids, flat(Cn::Expr(&n.target), name));
            keep(kids, flat(Cn::Expr(&n.iter), name));
            for st in &n.body {
                keep(kids, deep(Cn::Stmt(st), name));
            }
            if !n.orelse.is_empty() {
                keep(kids, loop_else(&n.orelse, name));
            }
        }
        Cn::Stmt(Stmt::While(n)) => {
            keep(kids, flat(Cn::Expr(&n.test), name));
            for st in &n.body {
                keep(kids, deep(Cn::Stmt(st), name));
            }
            if !n.orelse.is_empty() {
                keep(kids, loop_else(&n.orelse, name));
            }
        }
        Cn::Stmt(Stmt::Match(n)) => {
            keep(kids, flat(Cn::Expr(&n.subject), name));
            for case in &n.cases {
                keep(kids, deep(Cn::Case(case), name));
            }
        }
        Cn::Handler(h) => {
            if let Some(t) = &h.type_ {
                keep(kids, flat(Cn::Expr(t), name));
            }
            for st in &h.body {
                keep(kids, deep(Cn::Stmt(st), name));
            }
        }
        Cn::Expr(Expr::If(n)) => {
            keep(kids, flat(Cn::Expr(&n.test), name));
            keep(kids, deep(Cn::Expr(&n.body), name));
            keep(kids, deep(Cn::Expr(&n.orelse), name));
        }
        Cn::Expr(Expr::Lambda(n)) => {
            if let Some(p) = &n.parameters {
                keep(kids, flat(Cn::Params(p), name));
            }
            keep(kids, deep(Cn::Expr(&n.body), name));
        }
        // Nothing else sinks a block, so every child sits at its own depth.
        other => {
            let mut children = Vec::new();
            order::children(other, &mut children);
            for child in children {
                keep(kids, flat(child, name));
            }
        }
    }
}

/// SonarSource cognitive complexity of a def's body; `nesting` prices it as if
/// it sat that deep (#48's fold). No memo here: R20 puts it in `Provers`.
pub fn cognitive_complexity(node: &StmtFunctionDef, nesting: u32) -> u32 {
    let name = Some(node.name.as_str());
    let roots: Vec<Cc> = def_children(node)
        .into_iter()
        .map(|c| flat(c, name))
        .collect();
    score(&roots, nesting)
}

/// `ast.iter_child_nodes` of a `FunctionDef` (`order::children`'s arm, which
/// needs a `&Stmt` this caller has not got). None of them sinks a block, so
/// only the membership matters.
fn def_children(node: &StmtFunctionDef) -> Vec<Cn<'_>> {
    let mut out = vec![Cn::Params(&node.parameters)];
    out.extend(node.body.iter().map(Cn::Stmt));
    out.extend(node.decorator_list.iter().map(|d| Cn::Expr(&d.expression)));
    out.extend(node.returns.iter().map(|r| Cn::Expr(r)));
    out.extend(
        node.type_params
            .as_deref()
            .into_iter()
            .flat_map(|t: &TypeParams| t.type_params.iter().map(Cn::TypeParam)),
    );
    out
}

fn same(node: Cn<'_>, expr: &Expr) -> bool {
    matches!(node, Cn::Expr(x) if std::ptr::eq(x, expr))
}

/// Is `node` a member of one of `cur`'s nested block lists? Every child that
/// is not the test (or the loop head, or the subject) is one; a ternary has no
/// list field at all, so nothing it holds counts.
fn in_block(cur: Cn<'_>, node: Cn<'_>) -> bool {
    let head: Option<&Expr> = match cur {
        Cn::Stmt(Stmt::If(n)) => Some(&n.test),
        Cn::Elif(rest) => rest[0].test.as_ref(),
        Cn::Stmt(Stmt::While(n)) => Some(&n.test),
        Cn::Stmt(Stmt::Match(n)) => Some(&n.subject),
        Cn::Handler(h) => h.type_.as_deref(),
        Cn::Stmt(Stmt::For(n)) => {
            return !same(node, &n.target) && !same(node, &n.iter);
        }
        _ => return false, // `IfExp`, whose body and orelse are single nodes
    };
    !head.is_some_and(|h| same(node, h))
}

/// Is `cur` the `If` CPython synthesizes for an `elif`? An `else:` holding one
/// `if` reads the same way, which is what CPython's `orelse == [If]` sees.
fn is_elif(cur: Cn<'_>, up: Option<Cn<'_>>) -> bool {
    if matches!(cur, Cn::Elif(_)) {
        return true;
    }
    let Cn::Stmt(stmt @ Stmt::If(_)) = cur else {
        return false;
    };
    let Some((_, _, tail)) = up.and_then(if_parts) else {
        return false;
    };
    matches!(tail, Some(Orelse::Else([only])) if std::ptr::eq(only, stmt))
}

/// SonarSource nesting depth of `node`: the nesters whose block holds it,
/// climbing `parent` up to the def; an elif's own `if` adds none.
pub fn nesting_at<'a>(node: Cn<'a>, parent: &dyn Fn(Cn<'a>) -> Option<Cn<'a>>) -> u32 {
    let mut depth = 0;
    let mut node = node;
    let mut cur = parent(node);
    while let Some(c) = cur {
        if is_scope(c.kind()) {
            break;
        }
        let up = parent(c);
        if is_nester(c.kind()) && !is_elif(c, up) && in_block(c, node) {
            depth += 1;
        }
        node = c;
        cur = up;
    }
    depth
}

#[cfg(test)]
mod tests;
