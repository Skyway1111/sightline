//! Family A, the return contract (#33) and the raise contract (#53).
//! `return_contract_finding` is #34's overlap oracle: a handler #33 reports
//! is #33's alone.

use std::collections::BTreeSet;

use ruff_python_ast::{
    ElifElseClause, ExceptHandler, ExceptHandlerExceptHandler, Expr, Number, Pattern, Stmt,
    StmtFunctionDef,
};

use sightline_core::findings::{Evidence, Finding, Sink};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_core::text::declared_raises;
use sightline_py_facts::astutil::{fn_args, fn_body, is_call_stmt, without_receiver};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::literal::{Literal, literal};
use sightline_py_facts::model::{FUNCTION_KINDS, NodeIndex, RepoFacts, Step, Symbol, class_walk};
use sightline_py_facts::module::Module;
use sightline_py_facts::order;
use sightline_py_facts::qnames::resolve_dotted_expr;
use sightline_py_facts::unparse;
use sightline_py_provers::Provers;
use sightline_py_provers::annotations::{annotation_names, none_inclusive};
use sightline_py_provers::effects::{UNNAMED, raised_name};
use sightline_py_provers::handlers::exception_is;

use crate::model::Rule;
use crate::util::{
    decorator_names, fn_of, is_exported, iter_functions, iter_prod_functions, node_site,
    raw_docstring,
};

/// The out-of-band value a bare numeric contract admits. `""` under `-> str`
/// was a legitimate text value at 12 of 12 sampled sites: cut, seed 20260842.
fn sentinel_of(name: &str) -> Option<i128> {
    matches!(name, "int" | "float").then_some(-1)
}

fn no_return(ann: Option<&Expr>) -> bool {
    ann.is_some_and(|a| {
        annotation_names(a)
            .iter()
            .any(|n| n == "NoReturn" || n == "Never")
    })
}

/// CPython `bool(Constant.value)`; `None` for anything that is no `Constant`.
fn const_truth(test: &Expr) -> Option<bool> {
    Some(match test {
        Expr::NumberLiteral(n) => match &n.value {
            Number::Int(i) => i.as_u64().is_none_or(|v| v != 0),
            Number::Float(f) => *f != 0.0,
            Number::Complex { real, imag } => *real != 0.0 || *imag != 0.0,
        },
        Expr::StringLiteral(s) => !s.value.is_empty(),
        Expr::BytesLiteral(b) => b.value.bytes().next().is_some(),
        Expr::BooleanLiteral(b) => b.value,
        Expr::NoneLiteral(_) => false,
        Expr::EllipsisLiteral(_) => true,
        _ => return None,
    })
}

fn const_true(test: &Expr) -> bool {
    const_truth(test) == Some(true)
}

/// A `break` the loop itself owns: the walk stops at a nested loop, def or
/// lambda, whose `break` belongs to that scope.
fn has_own_break(body: &[Stmt]) -> bool {
    let mut stack: Vec<Cn<'_>> = body.iter().map(Cn::Stmt).collect();
    let mut kids: Vec<Cn<'_>> = Vec::new();
    while let Some(node) = stack.pop() {
        if matches!(node, Cn::Stmt(Stmt::Break(_))) {
            return true;
        }
        let opaque = matches!(
            node,
            Cn::Stmt(Stmt::For(_) | Stmt::While(_) | Stmt::FunctionDef(_))
                | Cn::Expr(Expr::Lambda(_))
        );
        if !opaque {
            kids.clear();
            order::children(node, &mut kids);
            stack.extend(kids.iter().copied());
        }
    }
    false
}

/// False = fall-through is provable; true = it is not (terminates, or an
/// unknown tail). Not a termination proof: a trailing call to an unseen
/// callee (cross-module, method, builtin) is true, the only sound reading for
/// #33, which fires on provable fall-through.
fn terminates(stmts: &[Stmt], facts: &RepoFacts<'_>, module: &Module<'_>) -> bool {
    let Some(last) = stmts.last() else {
        return false;
    };
    match last {
        Stmt::Return(_) | Stmt::Raise(_) => true,
        Stmt::Assert(a) => const_truth(&a.test) == Some(false),
        Stmt::Expr(e) if is_call_stmt(last) => tail_call_terminates(e, facts, module),
        Stmt::If(n) => terminates_if(Some(&n.test), &n.body, &n.elif_else_clauses, facts, module),
        Stmt::While(n) => const_true(&n.test) && !has_own_break(&n.body),
        Stmt::For(n) => terminates(&n.orelse, facts, module),
        Stmt::With(n) => terminates(&n.body, facts, module),
        Stmt::Try(t) if !t.is_star => {
            terminates(&t.finalbody, facts, module)
                || ((terminates(&t.body, facts, module) || terminates(&t.orelse, facts, module))
                    && t.handlers.iter().all(|h| {
                        let ExceptHandler::ExceptHandler(h) = h;
                        terminates(&h.body, facts, module)
                    }))
        }
        Stmt::Match(m) => {
            m.cases.iter().all(|c| terminates(&c.body, facts, module))
                && m.cases.iter().any(|c| {
                    matches!(&c.pattern, Pattern::MatchAs(p) if p.pattern.is_none())
                        && c.guard.is_none()
                })
        }
        _ => false,
    }
}

/// A tail call falls through only for a callee every build mode sees
/// identically: a same-module plain function without `NoReturn`.
fn tail_call_terminates(
    stmt: &ruff_python_ast::StmtExpr,
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
) -> bool {
    let Expr::Call(call) = &*stmt.value else {
        return true;
    };
    let Expr::Name(f) = &*call.func else {
        return true;
    };
    if matches!(f.id.as_str(), "exit" | "quit" | "abort" | "_exit") {
        return true;
    }
    let callee = Cn::Expr(&stmt.value)
        .stamped()
        .and_then(|at| facts.call_index.get(&(module.id, at)))
        .and_then(|at| facts.call_sites.get(*at as usize))
        .and_then(|cs| cs.target.as_deref())
        .and_then(|target| facts.symbols.get(target));
    match callee {
        Some(sym) if sym.kind == "function" && sym.module == module.qname => {
            no_return(module.returns(sym.node))
        }
        _ => true,
    }
}

/// CPython nests an `elif` as one `If` in the outer `orelse`; ruff keeps the
/// clause chain flat, so the chain is walked here.
fn terminates_if(
    test: Option<&Expr>,
    body: &[Stmt],
    rest: &[ElifElseClause],
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
) -> bool {
    let tail = match rest.first() {
        None => test.is_some_and(const_true),
        Some(next) if next.test.is_some() => {
            terminates_if(next.test.as_ref(), &next.body, &rest[1..], facts, module)
        }
        Some(next) => terminates(&next.body, facts, module),
    };
    terminates(body, facts, module) && tail
}

fn is_stub_body(body: &[Stmt]) -> bool {
    body.iter().all(|st| {
        matches!(st, Stmt::Pass(_))
            || matches!(st, Stmt::Expr(e) if matches!(&*e.value, Expr::EllipsisLiteral(_)))
    })
}

/// A `return -1` under a bare `-> int` / `-> float` beside at least one
/// non-literal return. Python compares with `==`, so `return -1.0` counts.
fn returns_sentinel(module: &Module<'_>, ret: Option<&Expr>, returns: &[NodeIndex]) -> bool {
    let Some(Expr::Name(name)) = ret else {
        return false;
    };
    let Some(mark) = sentinel_of(name.id.as_str()) else {
        return false;
    };
    let values: Vec<Literal> = returns
        .iter()
        .filter_map(|at| match module.nodes[*at as usize] {
            Cn::Stmt(Stmt::Return(r)) => r.value.as_deref().map(literal),
            _ => None,
        })
        .collect();
    values.contains(&Literal::Computed)
        && values.iter().any(|v| match v {
            Literal::Int(n) => *n == mark,
            Literal::Float(f) => *f == mark as f64,
            _ => false,
        })
}

/// The kind of `None` a `return` spells.
fn return_kind(module: &Module<'_>, at: NodeIndex) -> &'static str {
    let Cn::Stmt(Stmt::Return(r)) = module.nodes[at as usize] else {
        return "value";
    };
    match r.value.as_deref() {
        None => "bare",
        Some(Expr::NoneLiteral(_)) => "none",
        Some(_) => "value",
    }
}

/// #33 for one function: a non-Optional annotation with a None path,
/// `-> None` returning values, mixed value/bare returns, a sentinel beside
/// computed values.
pub fn return_contract_finding(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    sym: &Symbol,
) -> Option<Finding> {
    let fn_def = fn_of(module, sym);
    let decorators = decorator_names(fn_def);
    let body = fn_body(&fn_def.body);
    if decorators.contains("abstractmethod")
        || decorators.contains("overload")
        // a bare return under a yield is StopIteration, honest
        || !module
            .nodes(&[Kind::Yield, Kind::YieldFrom], Some(&sym.qname), false)
            .is_empty()
        || is_stub_body(body)
        || no_return(module.returns(sym.node))
    {
        return None;
    }
    let returns = module.nodes(&[Kind::Return], Some(&sym.qname), false);
    let kinds: BTreeSet<&str> = returns.iter().map(|at| return_kind(module, *at)).collect();
    let falls = !terminates(body, facts, module);

    let finding = |cause: &str, message: String, detail: &str| Finding {
        rule: "33",
        site: node_site(facts, module, sym.node),
        message: format!("{} {message}", sym.qname),
        cause: format!("{cause}:{}", sym.qname),
        evidence: Evidence::Ast {
            detail: detail.to_string(),
        },
        salience: 0.0,
        fix: None,
        lang: "py",
    };

    let ret = module.returns(sym.node);
    match ret {
        None => {
            if kinds == BTreeSet::from(["none", "value"])
                && !falls
                && without_receiver(&fn_args(fn_def)).iter().any(|a| {
                    Cn::Param(a)
                        .stamped()
                        .is_some_and(|at| module.annotation(at).is_some())
                })
            {
                return Some(finding(
                    "undeclared-optional",
                    "types its params but returns None beside values - an \
                     undeclared Optional"
                        .to_string(),
                    "undeclared Optional",
                ));
            }
            // explicit `return None` is an intentional value (the
            // Optional-lookup idiom), and so is falling off the end. Only a
            // written bare `return` beside a value return is one body holding
            // two contracts.
            if kinds.contains("value") && kinds.contains("bare") {
                return Some(finding(
                    "mixed-returns",
                    "mixes value returns with bare returns - callers cannot \
                     rely on the result"
                        .to_string(),
                    "inconsistent returns",
                ));
            }
        }
        Some(Expr::NoneLiteral(_)) => {
            if kinds.contains("value") {
                return Some(finding(
                    "lying-return",
                    "is annotated `-> None` but returns a value".to_string(),
                    "returns under -> None",
                ));
            }
        }
        Some(ann) => {
            let none_path = kinds.contains("bare") || kinds.contains("none");
            if !none_inclusive(facts, &module.bindings, ann) && (none_path || falls) {
                let how = if none_path {
                    "returns None"
                } else {
                    "can fall off the end"
                };
                return Some(finding(
                    "lying-return",
                    format!(
                        "declares `-> {}` but {how} - the annotation lies about None",
                        unparse::expr(ann)
                    ),
                    "None path",
                ));
            }
        }
    }
    if returns_sentinel(module, ret, &returns) {
        let Some(Expr::Name(name)) = ret else {
            return None;
        };
        return Some(finding(
            "sentinel",
            format!(
                "returns -1 beside a computed value under `-> {}` - an \
                 out-of-band result the contract admits",
                name.id
            ),
            "sentinel return",
        ));
    }
    None
}

pub const RULE_33: Rule = Rule {
    record: RuleRecord {
        id: "33",
        slug: "return-honesty",
        family: "A",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "return annotation contradicted by a None path; mixed returns",
        goal: "Honest signatures: a return contract the body breaks makes \
               every caller re-derive the truth.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_33,
};

/// Return contracts must match the body (one finding per function).
fn rule_33(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_functions(facts) {
        if let Some(f) = return_contract_finding(facts, module, sym) {
            out.push(f);
        }
    }
}

/// Does a section naming `declared` cover a raised type: by its name, by a
/// base of the repo class so named, or by a builtin base of it (`Raises:
/// OSError` covers `raise FileNotFoundError`).
fn named_by(facts: &RepoFacts<'_>, raised: &str, declared: &BTreeSet<String>) -> bool {
    let mut names: BTreeSet<String> = BTreeSet::from([raised.to_string()]);
    for q in facts.classes_by_name(raised) {
        for (_, info) in class_walk(facts, q, Step::Bases) {
            names.insert(pytext::rpartition(&info.qname, ".").2.to_string());
            names.extend(
                info.external_bases
                    .iter()
                    .map(|b| pytext::rpartition(b, ".").2.to_string()),
            );
        }
    }
    names.iter().any(|n| declared.contains(n))
        || names
            .iter()
            .any(|n| declared.iter().any(|d| exception_is(n, d)))
}

/// The types a raise names: `raised_name`'s reading, and for a raise of a
/// resolved repo function the callee's declared return (`raise make(s)` with
/// `make -> ParseError`; an unannotated factory names nothing).
fn raised(facts: &RepoFacts<'_>, module: &Module<'_>, node: NodeIndex) -> BTreeSet<String> {
    let Cn::Stmt(Stmt::Raise(stmt)) = module.nodes[node as usize] else {
        return BTreeSet::new();
    };
    let callee = match stmt.exc.as_deref() {
        Some(Expr::Call(call)) => {
            resolve_dotted_expr(&call.func, module, facts).and_then(|q| facts.symbols.get(&*q))
        }
        _ => None,
    };
    if let Some(sym) = callee.filter(|s| FUNCTION_KINDS.contains(&s.kind)) {
        let home = &facts.modules[&sym.module];
        return match home.returns(sym.node) {
            Some(ret) => annotation_names(ret),
            None => BTreeSet::new(),
        };
    }
    match raised_name(module, stmt) {
        Some(name) if name != UNNAMED => BTreeSet::from([name]),
        _ => BTreeSet::new(),
    }
}

fn handler_names(module: &Module<'_>, h: &ExceptHandlerExceptHandler) -> BTreeSet<String> {
    let elts: Vec<&Expr> = match h.type_.as_deref() {
        Some(Expr::Tuple(t)) => t.elts.iter().collect(),
        Some(other) => vec![other],
        None => Vec::new(),
    };
    elts.into_iter()
        .map(|e| {
            let spelled = module.dotted_name(e).unwrap_or_else(|| match e {
                Expr::Name(n) => n.id.to_string(),
                _ => String::new(),
            });
            pytext::rpartition(&spelled, ".").2.to_string()
        })
        .collect()
}

/// A `try` between the raise and its def whose handler names the type, a base
/// of it or is bare catches it, unless that handler re-raises bare. A raise in
/// a handler, an `else` or a `finally` sits outside the try's protection.
fn caught(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    sym: &Symbol,
    node: NodeIndex,
    name: &str,
) -> bool {
    let mut child = node;
    let mut cur = module.parent_of(node);
    while let Some(at) = cur {
        if at == sym.node {
            break;
        }
        if let Cn::Stmt(Stmt::Try(t)) = module.nodes[at as usize]
            && t.body.iter().any(|s| Cn::Stmt(s).stamped() == Some(child))
        {
            for handler in &t.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                if h.type_.is_none() || named_by(facts, name, &handler_names(module, h)) {
                    return !reraises_bare(module, sym, h);
                }
            }
        }
        child = at;
        cur = module.parent_of(at);
    }
    false
}

/// A bare `raise` on the handler's own lines lets the type back out.
fn reraises_bare(module: &Module<'_>, sym: &Symbol, h: &ExceptHandlerExceptHandler) -> bool {
    let Some(at) = Cn::Handler(h).stamped() else {
        return false;
    };
    let first = module.line_of(at);
    let last = match module.end_line_of(at) {
        0 => first,
        end => end,
    };
    module
        .nodes(&[Kind::Raise], Some(&sym.qname), false)
        .into_iter()
        .any(|r| {
            matches!(module.nodes[r as usize], Cn::Stmt(Stmt::Raise(n)) if n.exc.is_none())
                && (first..=last).contains(&module.line_of(r))
        })
}

/// An interface stub: its Raises section is the implementations'.
fn is_placeholder(fn_def: &StmtFunctionDef) -> bool {
    let body = fn_body(&fn_def.body);
    decorator_names(fn_def).contains("abstractmethod")
        || is_stub_body(body)
        || (body.len() == 1
            && matches!(&body[0], Stmt::Raise(r)
            if r.exc.as_deref().is_some_and(|e| {
                annotation_names(e).contains("NotImplementedError")
            })))
}

pub const RULE_53: Rule = Rule {
    record: RuleRecord {
        id: "53",
        slug: "raise-contract",
        family: "A",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "docstring Raises section missing a type the body raises",
        goal: "Honest contracts: a Raises section missing a raised type makes \
               every caller re-read the body to learn what to catch.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_53,
};

/// An exported prod def whose docstring declares raises. `undeclared:` is an
/// own-body raise that escapes the def and the section never names. Interface
/// stubs out. No stale arm: what an external call, a subscript, `assert` or
/// `/` raises is no one's summary.
fn rule_53(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_prod_functions(facts) {
        let fn_def = fn_of(module, sym);
        let declared = declared_raises(raw_docstring(&fn_def.body).unwrap_or(""));
        if declared.is_empty() || !is_exported(facts, module, sym) || is_placeholder(fn_def) {
            continue;
        }
        let own: BTreeSet<String> = module
            .nodes(&[Kind::Raise], Some(&sym.qname), false)
            .into_iter()
            .flat_map(|at| {
                raised(facts, module, at)
                    .into_iter()
                    .filter(move |r| !caught(facts, module, sym, at, r))
            })
            .collect();
        let site = node_site(facts, module, sym.node);
        for t in own.iter().filter(|t| !named_by(facts, t, &declared)) {
            out.push(Finding {
                rule: "53",
                site: site.clone(),
                message: format!(
                    "{} raises {t} but its Raises section never names it",
                    sym.qname
                ),
                cause: format!("raise-contract:undeclared:{}:{t}", sym.qname),
                evidence: Evidence::Ast {
                    detail: "undeclared raise".to_string(),
                },
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}
