//! Family B, the idiom catalog (#12): whole-function and node-level stdlib
//! reimplementations. Port of `rules/idioms.py`. Entries are machine-validated
//! equivalences: the exemplar pairs are `catalog/idioms/*.py` and
//! `cargo xtask catalog` proves them.

use ruff_python_ast::{CmpOp, Expr, Number, Operator, Stmt, StmtFunctionDef};

use sightline_core::findings::{Evidence, Finding, Sink};
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::astutil::{fn_body, subnodes};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{NodeIndex, RepoFacts};
use sightline_py_facts::module::Module;
use sightline_py_facts::unparse;
use sightline_py_provers::Provers;
use sightline_py_provers::scope::Scope as FnScope;

use crate::model::{MatchCtx, Rule, Shape};
use crate::util::{iter_functions, node_site};

/// `(lo + hi) // 2` or `(hi - lo) // 2` over the while's two bounds.
fn halving(node: &Expr, bounds: (&str, &str)) -> bool {
    let Expr::BinOp(outer) = node else {
        return false;
    };
    if outer.op != Operator::FloorDiv || !is_int(&outer.right, 2) {
        return false;
    }
    let Expr::BinOp(inner) = &*outer.left else {
        return false;
    };
    if !matches!(inner.op, Operator::Add | Operator::Sub) {
        return false;
    }
    let (Expr::Name(a), Expr::Name(b)) = (&*inner.left, &*inner.right) else {
        return false;
    };
    pair(a.id.as_str(), b.id.as_str()) == pair(bounds.0, bounds.1)
}

/// A two-name set as `{a, b}` compares: order out, duplicates folded.
fn pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

/// A `Constant` equal to the integer, as CPython's `==` reads it: `2.0`
/// matches `2`, and `False` matches `0`.
fn is_int(node: &Expr, want: i64) -> bool {
    match node {
        Expr::NumberLiteral(n) => match &n.value {
            Number::Int(i) => i.as_u64() == u64::try_from(want).ok(),
            Number::Float(f) => *f == want as f64,
            Number::Complex { .. } => false,
        },
        Expr::BooleanLiteral(b) => i64::from(b.value) == want,
        _ => false,
    }
}

/// Does this expression spell exactly this string literal? The three
/// callers all compare against one.
fn is_str(node: &Expr, want: &str) -> bool {
    node.as_string_literal_expr()
        .is_some_and(|s| s.value.to_str() == want)
}

fn name_of(node: &Expr) -> Option<&str> {
    node.as_name_expr().map(|n| n.id.as_str())
}

/// A call to the named builtin with exactly these positional arguments.
fn called<'a>(node: &'a Expr, name: &str, arity: usize) -> Option<&'a [Expr]> {
    let Expr::Call(c) = node else { return None };
    let Expr::Name(f) = &*c.func else { return None };
    (f.id.as_str() == name && c.arguments.args.len() == arity).then_some(&*c.arguments.args)
}

/// `while lo < hi` halving the two bounds into a name that indexes a compared
/// subscript. A heap sift divides by two and compares subscripts too, but
/// `while pos > 0` names one bound.
fn match_binary_search(_fn_def: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    for at in ctx.nodes(&[Kind::While]) {
        let Cn::Stmt(Stmt::While(w)) = ctx.module.nodes[at as usize] else {
            continue;
        };
        let Expr::Compare(test) = &*w.test else {
            continue;
        };
        if test.ops.len() != 1 || test.comparators.len() != 1 {
            continue;
        }
        let (Some(a), Some(b)) = (name_of(&test.left), name_of(&test.comparators[0])) else {
            continue;
        };
        let bounds = (a, b);
        let mids: Vec<&str> = subnodes(ctx.module.nodes[at as usize], |k| k == Kind::Assign)
            .into_iter()
            .filter_map(|reached| match reached {
                Cn::Stmt(Stmt::Assign(st)) => Some(st),
                _ => None,
            })
            .filter_map(|st| {
                let target = name_of(st.targets.first()?)?;
                let halved = subnodes(Cn::Expr(&st.value), |k| k == Kind::BinOp)
                    .into_iter()
                    .any(|n| matches!(n, Cn::Expr(e) if halving(e, bounds)));
                halved.then_some(target)
            })
            .collect();
        let indexed = subnodes(ctx.module.nodes[at as usize], |k| k == Kind::Compare)
            .into_iter()
            .filter_map(|reached| match reached {
                Cn::Expr(Expr::Compare(c)) => Some(c),
                _ => None,
            })
            .any(|c| {
                std::iter::once(&*c.left)
                    .chain(c.comparators.iter())
                    .any(|side| match side {
                        Expr::Subscript(s) => {
                            name_of(&s.slice).is_some_and(|id| mids.contains(&id))
                        }
                        _ => false,
                    })
            });
        if indexed {
            return vec![ctx.sym.node];
        }
    }
    Vec::new()
}

/// (x, below) of `if x < b: return b`: the value and whether it crossed the
/// bound downward. `sign` returns names too, but compares against constants.
fn clamp_arm(test: &Expr, body: &[Stmt]) -> Option<(String, bool)> {
    let Expr::Compare(test) = test else {
        return None;
    };
    if test.ops.len() != 1 || test.comparators.len() != 1 {
        return None;
    }
    let lt = match test.ops[0] {
        CmpOp::Lt => true,
        CmpOp::Gt => false,
        _ => return None,
    };
    let left = name_of(&test.left)?;
    let right = name_of(&test.comparators[0])?;
    let ret = match body {
        [Stmt::Return(r)] => name_of(r.value.as_deref()?)?,
        _ => return None,
    };
    if left == right || (ret != left && ret != right) {
        return None;
    }
    let value = if ret == right { left } else { right };
    Some((value.to_string(), lt == (ret == right)))
}

fn match_clamp(fn_def: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    let [Stmt::If(first), Stmt::Return(last)] = fn_body(&fn_def.body) else {
        return Vec::new();
    };
    let Some(x) = last.value.as_deref().and_then(name_of) else {
        return Vec::new();
    };
    // CPython's `If(orelse=[If(orelse=[])])`: one `elif` ending the chain, or
    // one `else` holding a single elseless `if`
    let [only] = first.elif_else_clauses.as_slice() else {
        return Vec::new();
    };
    let second: (&Expr, &[Stmt]) = match &only.test {
        Some(test) => (test, &only.body),
        None => match only.body.as_slice() {
            [Stmt::If(inner)] if inner.elif_else_clauses.is_empty() => (&inner.test, &inner.body),
            _ => return Vec::new(),
        },
    };
    let arms = [
        clamp_arm(&first.test, &first.body),
        clamp_arm(second.0, second.1),
    ];
    let wanted = [Some((x.to_string(), true)), Some((x.to_string(), false))];
    if arms[0] != arms[1] && arms.iter().all(|a| wanted.contains(a)) {
        return vec![ctx.sym.node];
    }
    Vec::new()
}

/// `chr(ord(c) + 32)` or `chr(ord(c) + (ord("a") - ord("A")))`.
fn lower_shift(node: &Expr, c: &str) -> bool {
    let Some([arg]) = called(node, "chr", 1) else {
        return false;
    };
    let Expr::BinOp(shift) = arg else {
        return false;
    };
    if shift.op != Operator::Add {
        return false;
    }
    let Some([inner]) = called(&shift.left, "ord", 1) else {
        return false;
    };
    if name_of(inner) != Some(c) {
        return false;
    }
    if is_int(&shift.right, 32) {
        return true;
    }
    let Expr::BinOp(gap) = &*shift.right else {
        return false;
    };
    let (Some([lo]), Some([hi])) = (called(&gap.left, "ord", 1), called(&gap.right, "ord", 1))
    else {
        return false;
    };
    gap.op == Operator::Sub && is_str(lo, "a") && is_str(hi, "A")
}

/// `"A" <= c <= "Z"` (either bound) or `c.isupper()`.
fn case_guard(node: &Expr, c: &str) -> bool {
    match node {
        Expr::Compare(cmp) => {
            let sides: Vec<&Expr> = std::iter::once(&*cmp.left)
                .chain(&cmp.comparators)
                .collect();
            sides.iter().any(|e| name_of(e) == Some(c))
                && sides.iter().any(|e| is_str(e, "A") || is_str(e, "Z"))
        }
        Expr::Call(call) => {
            let Expr::Attribute(a) = &*call.func else {
                return false;
            };
            a.attr.as_str() == "isupper"
                && name_of(&a.value) == Some(c)
                && call.arguments.args.is_empty()
        }
        _ => false,
    }
}

/// The loop variable shifted by the case gap under a case guard on it; a
/// Caesar shift by k has neither.
fn match_tolower(_fn_def: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    for at in ctx.nodes(&[Kind::For]) {
        let Cn::Stmt(Stmt::For(loop_)) = ctx.module.nodes[at as usize] else {
            continue;
        };
        let Some(c) = name_of(&loop_.target) else {
            continue;
        };
        let inner = subnodes(ctx.module.nodes[at as usize], |_| true);
        let exprs: Vec<&Expr> = inner
            .iter()
            .filter_map(|n| match n {
                Cn::Expr(e) => Some(*e),
                _ => None,
            })
            .collect();
        if exprs.iter().any(|e| lower_shift(e, c)) && exprs.iter().any(|e| case_guard(e, c)) {
            return vec![ctx.sym.node];
        }
    }
    Vec::new()
}

fn match_manual_sum(fn_def: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    let [Stmt::Assign(init), Stmt::For(loop_), Stmt::Return(ret)] = fn_body(&fn_def.body) else {
        return Vec::new();
    };
    let (Some(acc), true) = (
        init.targets.first().and_then(name_of),
        is_int(&init.value, 0),
    ) else {
        return Vec::new();
    };
    let Some(x) = name_of(&loop_.target) else {
        return Vec::new();
    };
    let [Stmt::AugAssign(add)] = loop_.body.as_slice() else {
        return Vec::new();
    };
    if !loop_.orelse.is_empty() || add.op != Operator::Add {
        return Vec::new();
    }
    let (Some(t), Some(v)) = (name_of(&add.target), name_of(&add.value)) else {
        return Vec::new();
    };
    let Some(r) = ret.value.as_deref().and_then(name_of) else {
        return Vec::new();
    };
    if t == acc && acc == r && v == x {
        return vec![ctx.sym.node];
    }
    Vec::new()
}

/// rule_12 skips a matcher when the index holds none of its trigger node. The
/// first entry that matches wins for a function.
const CATALOG: [(&str, Shape); 4] = [
    (
        "binary-search",
        Shape {
            matcher: match_binary_search,
            suggestion: "use bisect.bisect_left/bisect_right",
            trigger: Some(Kind::While),
        },
    ),
    (
        "clamp",
        Shape {
            matcher: match_clamp,
            suggestion: "use min(max(x, lo), hi)",
            trigger: None,
        },
    ),
    (
        "tolower",
        Shape {
            matcher: match_tolower,
            suggestion: "use str.lower()/str.casefold()",
            trigger: Some(Kind::For),
        },
    ),
    (
        "manual-sum",
        Shape {
            matcher: match_manual_sum,
            suggestion: "use sum(iterable)",
            trigger: None,
        },
    ),
];

/// The X of `for i in range(len(X))` whose body subscripts `X[i]`; `None` if
/// the body rebinds `i` or `X` or writes through `X` (range is fixed at entry,
/// enumerate is live).
fn range_len(
    facts: &RepoFacts<'_>,
    scope: &FnScope,
    module: &Module<'_>,
    at: NodeIndex,
) -> Option<String> {
    let Cn::Stmt(Stmt::For(loop_)) = module.nodes[at as usize] else {
        return None;
    };
    let i = name_of(&loop_.target)?;
    let Some([len_call]) = called(&loop_.iter, "range", 1) else {
        return None;
    };
    let Some([subject]) = called(len_call, "len", 1) else {
        return None;
    };
    let xs = name_of(subject)?;
    let target = Cn::Expr(&loop_.target).stamped();
    if scope
        .writes_in(facts, at)
        .iter()
        .any(|w| w.root.as_deref().is_some_and(|r| r == i || r == xs) && Some(w.node) != target)
    {
        return None;
    }
    let (lo, hi) = (module.line_of(at), module.end_line_of(at));
    let reads = module
        .nodes(&[Kind::Subscript], Some(&scope.qname), true)
        .into_iter()
        .filter(|n| {
            let line = module.line_of(*n);
            lo <= line && line <= hi
        });
    for read in reads {
        if let Cn::Expr(Expr::Subscript(s)) = module.nodes[read as usize]
            && name_of(&s.value) == Some(xs)
            && name_of(&s.slice) == Some(i)
        {
            return Some(xs.to_string());
        }
    }
    None
}

/// (entry, node, suggestion) for expression and loop-shape idioms over a
/// function's own-scope nodes; nested defs are their own functions.
fn node_idioms(
    facts: &RepoFacts<'_>,
    scope: &FnScope,
    module: &Module<'_>,
) -> Vec<(&'static str, NodeIndex, String)> {
    let own = module.nodes(
        &[
            Kind::ListComp,
            Kind::SetComp,
            Kind::IfExp,
            Kind::For,
            Kind::Compare,
        ],
        Some(&scope.qname),
        false,
    );
    let mut out = Vec::new();
    for at in own {
        let Cn::Expr(node) = module.nodes[at as usize] else {
            // a `For` statement, the only non-expression kind in the list
            if let Some(xs) = range_len(facts, scope, module, at) {
                out.push(("range-len", at, format!("use enumerate({xs})")));
            }
            continue;
        };
        match node {
            Expr::ListComp(_) | Expr::SetComp(_) => {
                let (elt, generators) = match node {
                    Expr::ListComp(c) => (&c.elt, &c.generators),
                    Expr::SetComp(c) => (&c.elt, &c.generators),
                    _ => continue,
                };
                let [only] = generators.as_slice() else {
                    continue;
                };
                let (Some(e), Some(t)) = (name_of(elt), name_of(&only.target)) else {
                    continue;
                };
                if e != t || !only.ifs.is_empty() || only.is_async {
                    continue;
                }
                let ctor = if matches!(node, Expr::ListComp(_)) {
                    "list"
                } else {
                    "set"
                };
                out.push((
                    "identity-comp",
                    at,
                    format!("use {ctor}({})", unparse::expr(&only.iter)),
                ));
            }
            Expr::If(ternary) => {
                let (Expr::BooleanLiteral(b), Expr::BooleanLiteral(o)) =
                    (&*ternary.body, &*ternary.orelse)
                else {
                    continue;
                };
                if b.value == o.value {
                    continue;
                }
                let test = unparse::expr(&ternary.test);
                let suggestion = if b.value {
                    format!("use bool({test})")
                } else {
                    format!("use not ({test})")
                };
                out.push(("bool-ternary", at, suggestion));
            }
            Expr::Compare(c) => {
                if c.ops.len() != 1
                    || !matches!(c.ops[0], CmpOp::In | CmpOp::NotIn)
                    || c.comparators.len() != 1
                {
                    continue;
                }
                let Expr::Call(call) = &c.comparators[0] else {
                    continue;
                };
                let Expr::Attribute(a) = &*call.func else {
                    continue;
                };
                if a.attr.as_str() == "keys"
                    && call.arguments.args.is_empty()
                    && call.arguments.keywords.is_empty()
                {
                    out.push((
                        "keys-membership",
                        at,
                        "membership tests the dict: drop .keys()".to_string(),
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

/// A lambda is its own scope (`provers/scope.py` agrees), so an idiom inside
/// one is not the enclosing function's.
fn inside_lambda(module: &Module<'_>, node: NodeIndex, func: NodeIndex) -> bool {
    let mut cur = module.parent_of(node);
    while let Some(at) = cur {
        if at == func {
            return false;
        }
        if module.nodes[at as usize].kind() == Kind::Lambda {
            return true;
        }
        cur = module.parent_of(at);
    }
    false
}

pub const RULE_12: Rule = Rule {
    record: RuleRecord {
        id: "12",
        slug: "idiom-catalog",
        family: "B",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "whole-function reimplementations of stdlib/builtins",
        goal: "Use the vocabulary (Parent; Wheeler): a hand-rolled stdlib \
               reimplementation hides intent and keeps bugs the library fixed.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_12,
};

/// Whole-function reimplementations from the catalog (the first entry that
/// matches), plus node-level expression and loop idioms.
fn rule_12(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_functions(facts) {
        let ctx = MatchCtx {
            facts,
            module,
            sym,
            amp: 0,
        };
        let fn_def = ctx.func();
        for (name, shape) in &CATALOG {
            let triggered = match shape.trigger {
                None => true,
                Some(kind) => !ctx.nodes(&[kind]).is_empty(),
            };
            if triggered && !(shape.matcher)(fn_def, &ctx).is_empty() {
                out.push(Finding {
                    rule: "12",
                    site: node_site(facts, module, sym.node),
                    message: format!("{} reimplements {name}: {}", sym.qname, shape.suggestion),
                    cause: format!("idiom:{name}:{}", sym.qname),
                    evidence: Evidence::Ast {
                        detail: (*name).to_string(),
                    },
                    salience: 0.0,
                    fix: None,
                    lang: "py",
                });
                break;
            }
        }
        let Some(scope) = provers.scope_of(facts, &sym.qname) else {
            continue;
        };
        for (name, node, suggestion) in node_idioms(facts, scope, module) {
            if inside_lambda(module, node, sym.node) {
                continue;
            }
            let span = module.span(node).unwrap_or_default();
            out.push(Finding {
                rule: "12",
                site: node_site(facts, module, node),
                message: format!("idiom {name} in {}: {suggestion}", sym.qname),
                cause: format!(
                    "idiom:{name}:{}:{}:{}",
                    sym.qname,
                    span[0].unwrap_or(0),
                    span[1].unwrap_or(0)
                ),
                evidence: Evidence::Ast {
                    detail: name.to_string(),
                },
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}
