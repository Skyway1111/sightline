//! `scope.py`'s write extraction: `Write`, `Guard`, and the free fns that
//! read a binding, a guard or an alias root off a node.

use super::*;

/// One binding or reference-write under a function body.
#[derive(Debug, Clone)]
pub struct Write {
    /// name the write reaches through (`None`: not name-rooted)
    pub root: Option<String>,
    /// name|del|attr|subscript|call|except|import|global|nonlocal
    pub kind: &'static str,
    /// the written expression, or the statement that binds
    pub node: NodeIndex,
    /// in the function's own scope, not a nested def or lambda
    pub own: bool,
    /// an AnnAssign declaration: a type, not a rebinding
    pub decl: bool,
    /// the value's alias roots (`None`: fresh)
    pub aliases: Option<BTreeSet<String>>,
}

#[derive(Debug, Clone)]
pub struct Guard {
    pub param: String,
    /// "isinstance" | "is-none" | "is-not-none"
    pub kind: &'static str,
    /// isinstance targets (empty for None checks)
    pub classes: Vec<String>,
    /// the check expression itself, whose line the layer prints
    pub node: NodeIndex,
}

pub(super) enum Key {
    Pos(usize),
    Kw(String),
}

/// `xs += [1]` mutates; `pos += 1` rebinds (every corpus alias-`+=` root).
pub(super) fn in_place(module: &Module<'_>, stmt: Option<NodeIndex>) -> bool {
    matches!(
        stmt.map(|s| module.nodes[s as usize]),
        Some(Cn::Stmt(Stmt::AugAssign(a))) if is_mutable_init(Some(&a.value))
    )
}

pub(super) fn name_id<'t>(module: &Module<'t>, node: NodeIndex) -> Option<&'t str> {
    match module.nodes[node as usize] {
        Cn::Expr(Expr::Name(n)) => Some(n.id.as_str()),
        _ => None,
    }
}

pub(super) fn parameters_of<'t>(module: &Module<'t>, node: NodeIndex) -> Option<&'t Parameters> {
    match module.nodes[node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => Some(&f.parameters),
        Cn::Expr(Expr::Lambda(l)) => l.parameters.as_deref(),
        _ => None,
    }
}

/// The test of an `If` (an `elif` included) or an `Assert`.
pub(super) fn test_of<'t>(module: &Module<'t>, node: NodeIndex) -> Option<&'t Expr> {
    match module.nodes[node as usize] {
        Cn::Stmt(Stmt::If(n)) => Some(&n.test),
        Cn::Elif(rest) => rest[0].test.as_ref(),
        Cn::Stmt(Stmt::Assert(a)) => Some(&a.test),
        _ => None,
    }
}

/// A bare name, or a chain the module binds (`models.Node`); one rooted at a
/// local object (`kinds.T`) names no class.
pub(super) fn isinstance_guard(
    module: &Module<'_>,
    expr: &Expr,
    params: &BTreeSet<&str>,
) -> Option<Guard> {
    let Expr::Call(call) = expr else { return None };
    if !matches!(&*call.func, Expr::Name(n) if n.id.as_str() == "isinstance")
        || call.arguments.args.len() != 2
    {
        return None;
    }
    let Expr::Name(subject) = &call.arguments.args[0] else {
        return None;
    };
    if !params.contains(subject.id.as_str()) {
        return None;
    }
    let target = &call.arguments.args[1];
    let elts: Vec<&Expr> = match target {
        Expr::Tuple(t) => t.elts.iter().collect(),
        other => vec![other],
    };
    if !elts
        .iter()
        .all(|e| matches!(e, Expr::Name(_)) || module.dotted_name(e).is_some())
    {
        return None;
    }
    let classes: Vec<String> = elts
        .iter()
        .map(|e| match e {
            Expr::Name(n) => n.id.to_string(),
            Expr::Attribute(a) => a.attr.to_string(),
            _ => String::new(),
        })
        .collect();
    Some(Guard {
        param: subject.id.to_string(),
        kind: "isinstance",
        classes,
        node: Cn::Expr(expr).stamped()?,
    })
}

pub(super) fn none_guard(expr: &Expr, params: &BTreeSet<&str>) -> Option<Guard> {
    let Expr::Compare(c) = expr else { return None };
    if c.ops.len() != 1 {
        return None;
    }
    let kind = match c.ops[0] {
        CmpOp::Is => "is-none",
        CmpOp::IsNot => "is-not-none",
        _ => return None,
    };
    let Expr::Name(left) = &*c.left else {
        return None;
    };
    if !params.contains(left.id.as_str())
        || !matches!(c.comparators.first(), Some(Expr::NoneLiteral(_)))
    {
        return None;
    }
    Some(Guard {
        param: left.id.to_string(),
        kind,
        classes: Vec::new(),
        node: Cn::Expr(expr).stamped()?,
    })
}

/// Names an expression's value may alias, or `None` when it is a fresh object
/// (literal, display, comprehension, call result, arithmetic). `or`/`and` and
/// walrus values return an *operand*, so they alias.
pub(super) fn alias_roots(e: Option<&Expr>) -> Option<BTreeSet<String>> {
    match e? {
        Expr::Name(n) => Some(BTreeSet::from([n.id.to_string()])),
        Expr::Attribute(a) => alias_roots(Some(&a.value)),
        Expr::Subscript(s) => alias_roots(Some(&s.value)),
        Expr::Starred(s) => alias_roots(Some(&s.value)),
        Expr::Await(a) => alias_roots(Some(&a.value)),
        Expr::Named(n) => alias_roots(Some(&n.value)),
        Expr::If(x) => union_roots([&*x.body, &*x.orelse]),
        Expr::BoolOp(b) => union_roots(b.values.iter()),
        _ => None,
    }
}

pub(super) fn union_roots<'a>(
    parts: impl IntoIterator<Item = &'a Expr>,
) -> Option<BTreeSet<String>> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for part in parts {
        out.extend(alias_roots(Some(part)).unwrap_or_default());
    }
    (!out.is_empty()).then_some(out)
}

/// Bind a target to a value, element-wise through parallel displays.
pub(super) fn pairs<'a>(target: &'a Expr, value: &'a Expr, out: &mut Vec<(&'a Expr, &'a Expr)>) {
    let elts = |e: &'a Expr| match e {
        Expr::Tuple(t) => Some(&*t.elts),
        Expr::List(l) => Some(&*l.elts),
        _ => None,
    };
    match (elts(target), elts(value)) {
        (Some(ts), Some(vs)) if ts.len() == vs.len() => {
            for (t, v) in ts.iter().zip(vs.iter()) {
                pairs(t, v, out);
            }
        }
        _ => out.push((target, value)),
    }
}

/// The writes one assignment target performs. A subscript or attribute target
/// writes *through* its root and never rebinds it.
pub(super) fn written(
    target: &Expr,
    aliases: Option<&BTreeSet<String>>,
    own: bool,
    decl: bool,
    out: &mut Vec<Write>,
) {
    match target {
        Expr::Tuple(t) => {
            for elt in &t.elts {
                written(elt, aliases, own, decl, out);
            }
        }
        Expr::List(l) => {
            for elt in &l.elts {
                written(elt, aliases, own, decl, out);
            }
        }
        Expr::Starred(s) => written(&s.value, aliases, own, decl, out),
        Expr::Name(n) => out.push(Write {
            root: Some(n.id.to_string()),
            kind: if n.ctx == ExprContext::Del {
                "del"
            } else {
                "name"
            },
            node: node_of(target),
            own,
            decl,
            aliases: aliases.cloned(),
        }),
        Expr::Attribute(_) | Expr::Subscript(_) => out.push(Write {
            root: chain_root(target, &CHAIN).map(str::to_string),
            kind: if matches!(target, Expr::Attribute(_)) {
                "attr"
            } else {
                "subscript"
            },
            node: node_of(target),
            own,
            decl,
            aliases: None,
        }),
        _ => {}
    }
}

pub(super) fn node_of(expr: &Expr) -> NodeIndex {
    Cn::Expr(expr)
        .stamped()
        .expect("the traversal stamped every node this walk reaches")
}

/// Every write one node performs, values paired to targets for aliasing.
pub(super) fn writes_of(module: &Module<'_>, node: NodeIndex, own: bool, out: &mut Vec<Write>) {
    let bind = |name: &str, kind: &'static str, out: &mut Vec<Write>| {
        out.push(Write {
            root: Some(name.to_string()),
            kind,
            node,
            own,
            decl: false,
            aliases: None,
        });
    };
    match module.nodes[node as usize] {
        Cn::Stmt(Stmt::Assign(a)) => {
            for target in &a.targets {
                let mut bound = Vec::new();
                pairs(target, &a.value, &mut bound);
                for (t, v) in bound {
                    written(t, alias_roots(Some(v)).as_ref(), own, false, out);
                }
            }
        }
        Cn::Stmt(Stmt::AnnAssign(a)) => written(
            &a.target,
            alias_roots(a.value.as_deref()).as_ref(),
            own,
            true,
            out,
        ),
        Cn::Expr(Expr::Named(n)) => written(
            &n.target,
            alias_roots(Some(&n.value)).as_ref(),
            own,
            false,
            out,
        ),
        Cn::Stmt(Stmt::AugAssign(a)) => written(&a.target, None, own, false, out),
        Cn::Stmt(Stmt::Delete(d)) => {
            for target in &d.targets {
                written(target, None, own, false, out);
            }
        }
        // a for/comprehension target over a shared iterable binds a shared
        // element; a comprehension is its own scope, so its target rebinds
        // nothing of the function's
        Cn::Stmt(Stmt::For(f)) => written(
            &f.target,
            alias_roots(Some(&f.iter)).as_ref(),
            own,
            false,
            out,
        ),
        Cn::Comp(c) => written(
            &c.target,
            alias_roots(Some(&c.iter)).as_ref(),
            false,
            false,
            out,
        ),
        // a capture binds (part of) the subject
        Cn::Stmt(Stmt::Match(m)) => {
            let aliases = alias_roots(Some(&m.subject));
            for case in &m.cases {
                for pat in walk(Cn::Pattern(&case.pattern)) {
                    let Cn::Pattern(pat) = pat else { continue };
                    let name = match pat {
                        Pattern::MatchAs(p) => p.name.as_ref(),
                        Pattern::MatchStar(p) => p.name.as_ref(),
                        Pattern::MatchMapping(p) => p.rest.as_ref(),
                        _ => None,
                    };
                    if let Some(name) = name {
                        out.push(Write {
                            root: Some(name.to_string()),
                            kind: "name",
                            node: Cn::Pattern(pat)
                                .stamped()
                                .expect("the traversal stamped every pattern"),
                            own,
                            decl: false,
                            aliases: aliases.clone(),
                        });
                    }
                }
            }
        }
        Cn::Stmt(Stmt::FunctionDef(f)) => bind(f.name.as_str(), "name", out),
        Cn::Stmt(Stmt::ClassDef(c)) => bind(c.name.as_str(), "name", out),
        Cn::Stmt(Stmt::With(w)) => {
            for item in &w.items {
                let Some(vars) = item.optional_vars.as_deref() else {
                    continue;
                };
                let mut bound = Vec::new();
                pairs(vars, &item.context_expr, &mut bound);
                for (t, v) in bound {
                    written(t, alias_roots(Some(v)).as_ref(), own, false, out);
                }
            }
        }
        Cn::Handler(h) => {
            if let Some(name) = &h.name {
                bind(name.as_str(), "except", out);
            }
        }
        Cn::Stmt(Stmt::Import(i)) => {
            for alias in &i.names {
                let local = alias.asname.as_ref().unwrap_or(&alias.name);
                bind(
                    local.as_str().split('.').next().unwrap_or(""),
                    "import",
                    out,
                );
            }
        }
        Cn::Stmt(Stmt::ImportFrom(i)) => {
            for alias in &i.names {
                let local = alias.asname.as_ref().unwrap_or(&alias.name);
                bind(
                    local.as_str().split('.').next().unwrap_or(""),
                    "import",
                    out,
                );
            }
        }
        Cn::Stmt(Stmt::Global(g)) => {
            for name in &g.names {
                bind(name.as_str(), "global", out);
            }
        }
        Cn::Stmt(Stmt::Nonlocal(n)) => {
            for name in &n.names {
                bind(name.as_str(), "nonlocal", out);
            }
        }
        Cn::Expr(Expr::Call(c)) => {
            if let Expr::Attribute(a) = &*c.func
                && MUTATOR_METHODS.contains(a.attr.as_str())
            {
                out.push(Write {
                    root: chain_root(&c.func, &CHAIN).map(str::to_string),
                    kind: "call",
                    node: node_of(&c.func),
                    own,
                    decl: false,
                    aliases: None,
                });
            }
        }
        _ => {}
    }
}
