//! Family T, tests quality: #42
//! assertion-free test, #44 tautological assertion, #47 sleepy test. Binary,
//! structural shapes only.
//!
//! file-length-ok: one file per rule family is this crate's shape
//! (`surface.rs`), and a RuleRecord lives beside the function it describes.

use std::collections::HashSet;

use ruff_python_ast::comparable::ComparableExpr;
use ruff_python_ast::{CmpOp, Expr, ExprCall, Number, Stmt};

use sightline_core::findings::{Evidence, Finding, Sink};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::astutil::{CHAIN, chain_root, fn_args, subnodes, without_receiver};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::literal::{Literal, literal};
use sightline_py_facts::model::{
    FUNCTION_KINDS, NodeIndex, RepoFacts, Resolution, Step, Symbol, class_walk,
};
use sightline_py_facts::module::Module;
use sightline_py_facts::qnames::resolve_dotted_expr;
use sightline_py_facts::unparse;
use sightline_py_provers::Provers;
use sightline_py_provers::callgraph::CallGraph;
use sightline_py_provers::catalog::SUBPROCESS_CALLS;
use sightline_py_provers::comments::declares_no_raise;
use sightline_py_provers::handlers::{call_name, carries_verdict};

use crate::model::Rule;
use crate::util::{iter_test_functions, library_name, node_site};

const CONDITIONAL: [Kind; 4] = [Kind::If, Kind::Match, Kind::Try, Kind::TryStar];

fn is_call(kind: Kind) -> bool {
    kind == Kind::Call
}

/// Enclosing nodes from `node` up to, excluding, its function.
fn ancestors(module: &Module<'_>, node: NodeIndex, func: NodeIndex) -> Vec<NodeIndex> {
    let mut out = Vec::new();
    let mut cur = module.parent_of(node);
    while let Some(at) = cur {
        if at == func {
            break;
        }
        out.push(at);
        cur = module.parent_of(at);
    }
    out
}

/// A raise decides only under a condition, an enclosing if/match/try or an
/// earlier return (a guard clause): a placeholder's bare `raise
/// NotImplementedError` fails whatever the code does.
fn scope_has_verdict(module: &Module<'_>, sym: &Symbol) -> bool {
    let first_return = module
        .nodes(&[Kind::Return], Some(&sym.qname), false)
        .into_iter()
        .map(|at| module.line_of(at))
        .min();
    let judged = module
        .nodes(
            &[Kind::Assert, Kind::Raise, Kind::Call],
            Some(&sym.qname),
            true,
        )
        .into_iter()
        .filter(|at| {
            let node = module.nodes[*at as usize];
            node.kind() != Kind::Raise
                || first_return.is_some_and(|first| first < module.line_of(*at))
                || ancestors(module, *at, sym.node)
                    .into_iter()
                    .any(|a| CONDITIONAL.contains(&module.nodes[a as usize].kind()))
        })
        .map(|at| module.nodes[at as usize]);
    carries_verdict(judged)
}

/// The repo function a call site runs, per the upgraded graph. A site the
/// oracle could not confirm has no verdict, and a guess is not one.
fn callee<'a>(
    graph: &CallGraph,
    facts: &'a RepoFacts<'_>,
    module: &Module<'_>,
    call: NodeIndex,
) -> Option<&'a Symbol> {
    let site = graph.by_node(facts, module.id, call)?;
    if site.resolution != Resolution::Resolved {
        return None;
    }
    facts
        .symbols
        .get(site.target.as_deref()?)
        .filter(|sym| FUNCTION_KINDS.contains(&sym.kind))
}

/// Could this call carry the test's verdict? A repo helper whose own body
/// verdicts does, and so does a repo body the graph could not read.
fn may_verdict(
    graph: &CallGraph,
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    call: NodeIndex,
) -> bool {
    let Some(judged) = graph.by_node(facts, module.id, call) else {
        return false;
    };
    if judged.resolution != Resolution::Resolved {
        let seen = facts
            .call_index
            .get(&(module.id, call))
            .and_then(|at| facts.call_sites.get(*at as usize))
            .map(|site| site.resolution);
        return judged.resolution == Resolution::Ambiguous
            || matches!(seen, Some(Resolution::ByName | Resolution::Ambiguous));
    }
    callee(graph, facts, module, call).is_some_and(|target| {
        facts
            .modules
            .get(&target.module)
            .is_some_and(|home| scope_has_verdict(home, target))
    })
}

/// What a call is given, its args and keywords, a name standing for the value
/// the test bound it to earlier.
fn given<'t>(module: &Module<'t>, scope: &str, call: &'t ExprCall, line: u32) -> Vec<&'t Expr> {
    let mut bound: Vec<(&str, &Expr)> = Vec::new();
    for at in module.nodes(&[Kind::Assign], Some(scope), false) {
        let Cn::Stmt(Stmt::Assign(assign)) = module.nodes[at as usize] else {
            continue;
        };
        if module.line_of(at) >= line {
            continue;
        }
        for target in &assign.targets {
            if let Expr::Name(name) = target {
                bound.push((name.id.as_str(), &assign.value));
            }
        }
    }
    // the dict comprehension keeps the last binding of a name
    let latest = |id: &str| bound.iter().rev().find(|(n, _)| *n == id).map(|(_, v)| *v);
    call.arguments
        .args
        .iter()
        .chain(call.arguments.keywords.iter().map(|k| &k.value))
        .map(|e| match e {
            Expr::Name(n) => latest(n.id.as_str()).unwrap_or(e),
            _ => e,
        })
        .collect()
}

/// Calls the module exercises inside an exception pin, as they are spelled:
/// where the module pins a callee's rejecting cases, a test that calls it and
/// stops has that module's oracle, it must not raise.
fn pinned_here(module: &Module<'_>) -> HashSet<String> {
    const PINS: [&str; 3] = ["raises", "assertRaises", "assertRaisesRegex"];
    let mut out: HashSet<String> = HashSet::new();
    for at in module.nodes(&[Kind::With, Kind::AsyncWith], None, false) {
        let Cn::Stmt(Stmt::With(block)) = module.nodes[at as usize] else {
            continue;
        };
        let pins = block.items.iter().any(|item| {
            matches!(&item.context_expr, Expr::Call(c) if call_name(c).is_some_and(|n| PINS.contains(&n)))
        });
        if !pins {
            continue;
        }
        for st in &block.body {
            for node in subnodes(Cn::Stmt(st), is_call) {
                if let Cn::Expr(Expr::Call(call)) = node {
                    out.insert(unparse::expr(&call.func));
                }
            }
        }
    }
    out
}

/// The asserts run in a child process and its exit status is the verdict: a
/// checked spawn raises on the child's failure.
fn child_process_verdict(module: &Module<'_>, call: &ExprCall) -> bool {
    let Some(name) = library_name(module, &call.func) else {
        return false;
    };
    let checked = SUBPROCESS_CALLS
        .iter()
        .any(|n| *n == name && pytext::rpartition(n, ".").2.starts_with("check_"));
    checked
        || (SUBPROCESS_CALLS.contains(&&*name)
            && call.arguments.keywords.iter().any(|kw| {
                kw.arg.as_ref().is_some_and(|a| a.as_str() == "check")
                    && matches!(&kw.value, Expr::BooleanLiteral(b) if b.value)
            }))
}

pub const RULE_42: Rule = Rule {
    record: RuleRecord {
        id: "42",
        slug: "assertion-free-test",
        family: "T",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "test function with no verdict in its body or a repo helper it calls",
        goal: "A test without an oracle passes whatever the code does \
               (tsDetect Unknown Test; 8-77% of LLM-written suites).",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_42,
};

/// A test with no verdict in its own body or in a repo helper it calls (one
/// hop: pytest's assert rewriting makes helpers the oracle carrier). A repo
/// body the graph could not read might hold the verdict: skipped.
fn rule_42(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let graph = provers.calls(facts);
    let mut pinned: Vec<(String, HashSet<String>)> = Vec::new();
    for (module, sym) in iter_test_functions(facts) {
        // a call on the test's own parameter is a fixture-injected callable:
        // no graph edge reads its body, so the verdict may be there; a method
        // down its chain is one only when given a repo result
        let def = crate::util::fn_of(module, sym);
        let args = fn_args(def);
        let params: HashSet<&str> = without_receiver(&args)
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        if !pinned.iter().any(|(rel, _)| *rel == *module.rel) {
            pinned.push((module.rel.to_string(), pinned_here(module)));
        }
        let here = pinned
            .iter()
            .find(|(rel, _)| *rel == *module.rel)
            .map(|(_, set)| set)
            .expect("the module's pins were just recorded");
        let verdicted = scope_has_verdict(module, sym)
            || declares_no_raise(module, sym.node)
            || module
                .nodes(&[Kind::Call], Some(&sym.qname), true)
                .into_iter()
                .any(|at| {
                    let Some(call) = module.call_at(at) else {
                        return false;
                    };
                    matches!(&*call.func, Expr::Name(n) if params.contains(n.id.as_str()))
                        || (matches!(&*call.func, Expr::Attribute(_))
                            && chain_root(&call.func, &CHAIN)
                                .is_some_and(|root| params.contains(root))
                            && given(module, &sym.qname, call, module.line_of(at))
                                .into_iter()
                                .flat_map(|e| subnodes(Cn::Expr(e), is_call))
                                .filter_map(Cn::stamped)
                                .any(|c| callee(graph, facts, module, c).is_some()))
                        || may_verdict(graph, facts, module, at)
                        || here.contains(&unparse::expr(&call.func))
                        || child_process_verdict(module, call)
                });
        if verdicted {
            continue;
        }
        out.push(Finding {
            rule: "42",
            site: node_site(facts, module, sym.node),
            message: format!(
                "{} asserts nothing - it can only fail by raising",
                sym.qname
            ),
            cause: format!("assertion-free:{}", sym.qname),
            evidence: Evidence::idx(),
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

// --- #44 tautological assertion ----------------------------------------------

/// Library homes whose `assert_*` compare operands like assertEqual; a
/// repo-defined validator named so and mock's verifications on a local resolve
/// to none of them.
const ASSERT_HOMES: [&str; 3] = ["numpy.testing.", "pandas.testing.", "torch.testing."];

const COMPARISON_DUNDERS: [&str; 6] = ["__eq__", "__ne__", "__lt__", "__le__", "__gt__", "__ge__"];

/// An `assert_*` resolved through the module's bindings to a library home.
fn library_assert(module: &Module<'_>, call: &ExprCall) -> bool {
    let dotted = module.dotted_name(&call.func).unwrap_or_default();
    ASSERT_HOMES.iter().any(|home| dotted.starts_with(home))
        && pytext::rpartition(&dotted, ".").2.starts_with("assert_")
}

/// Python's truth value of a constant, for the constants `Literal` reads;
/// anything else it calls `Computed` (a complex number, an ellipsis) is
/// truthy or unknown, and the rule keeps reporting it.
fn always_true(expr: &Expr) -> bool {
    match literal(expr) {
        Literal::Bool(b) => b,
        Literal::Int(i) => i != 0,
        Literal::Float(f) => f != 0.0,
        Literal::Str(s) => !s.is_empty(),
        Literal::Bytes(b) => !b.is_empty(),
        Literal::None => false,
        _ => true,
    }
}

/// The truth value a one-argument `assert*` call needs its argument to hold,
/// where that argument is the truth value under test as a bare `assert`
/// statement's expression is. Every other one-argument `assert*` takes a
/// subject or a spec - a logger name, a query count, a template, an exception
/// class - and a constant there says nothing about a constant.
fn truth_arg(name: &str) -> Option<bool> {
    match name {
        "assertTrue" => Some(true),
        "assertFalse" => Some(false),
        _ => None,
    }
}

/// Operands of an assert statement or assert* call: two for a comparison, none
/// for a constant the assertion cannot fail on, `None` when it is not an
/// assertion. A constant on the failing side is not an operand set:
/// `assert False` always fails, so it is an unreachability marker that holds a
/// verdict, not an assertion that cannot fail.
fn compared<'t>(module: &Module<'t>, node: Cn<'t>) -> Option<Vec<&'t Expr>> {
    let (operands, wants): (Vec<&Expr>, Option<bool>) = match node {
        Cn::Stmt(Stmt::Assert(assert)) => match &*assert.test {
            Expr::Compare(cmp) if cmp.ops.len() == 1 => {
                (vec![&cmp.left, &cmp.comparators[0]], None)
            }
            other => (vec![other], Some(true)),
        },
        Cn::Expr(Expr::Call(call)) => {
            let name = call_name(call).unwrap_or("");
            if !(name.starts_with("assert")
                && (!name.starts_with("assert_") || library_assert(module, call)))
            {
                return None;
            }
            (
                call.arguments.args.iter().take(2).collect(),
                truth_arg(name),
            )
        }
        _ => return None,
    };
    if operands.len() == 1 && Cn::Expr(operands[0]).kind() == Kind::Constant {
        return (always_true(operands[0]) == wants?).then(Vec::new);
    }
    (operands.len() == 2).then_some(operands)
}

/// The operand is a name bound to an instance of a repo class that writes a
/// comparison dunder: `x == x` runs that dunder, so the operator is a call on
/// the code under test and the equal case is its boundary.
fn compares_repo_code(facts: &RepoFacts<'_>, module: &Module<'_>, expr: &Expr) -> bool {
    let Expr::Name(name) = expr else {
        return false;
    };
    for at in module.nodes(&[Kind::Assign], None, false) {
        let Cn::Stmt(Stmt::Assign(assign)) = module.nodes[at as usize] else {
            continue;
        };
        let Expr::Call(call) = &*assign.value else {
            continue;
        };
        if !assign
            .targets
            .iter()
            .any(|t| matches!(t, Expr::Name(n) if n.id == name.id))
        {
            continue;
        }
        let Some(q) = resolve_dotted_expr(&call.func, module, facts) else {
            continue;
        };
        if facts.classes.contains_key(&*q)
            && class_walk(facts, &q, Step::Bases).iter().any(|(_, info)| {
                COMPARISON_DUNDERS
                    .iter()
                    .any(|d| info.methods.contains_key(*d))
            })
        {
            return true;
        }
    }
    false
}

/// Why the assertion cannot discriminate, or `None`. Identical operands count
/// only when call-free (`f() == f()` tests determinism, and an operator
/// dispatching to a repo dunder is a call) and, under `is`, only for names.
fn tautology(facts: &RepoFacts<'_>, module: &Module<'_>, node: Cn<'_>) -> Option<&'static str> {
    let operands = compared(module, node)?;
    if operands.is_empty() {
        return Some("asserts a constant that is always true");
    }
    let (a, b) = (operands[0], operands[1]);
    if ComparableExpr::from(a) != ComparableExpr::from(b)
        || !subnodes(Cn::Expr(a), is_call).is_empty()
        || compares_repo_code(facts, module, a)
    {
        return None;
    }
    let identity = match node {
        Cn::Stmt(Stmt::Assert(assert)) => matches!(
            &*assert.test,
            Expr::Compare(cmp) if matches!(cmp.ops[0], CmpOp::Is | CmpOp::IsNot)
        ),
        _ => false,
    };
    if identity && !matches!(a, Expr::Name(_)) {
        return None;
    }
    Some("asserts an expression against itself")
}

pub const RULE_44: Rule = Rule {
    record: RuleRecord {
        id: "44",
        slug: "tautological-assertion",
        family: "T",
        engine_class: "AST",
        posture: Posture::Report,
        meaning: "assertion of a call-free expression against itself, or of an \
                  always-true constant",
        goal: "An assertion that cannot fail specifies nothing (tsDetect \
               Redundant Assertion); the SUT-derived-expected mirror is left \
               unreported - statically it is the idempotence test's shape. \
               `assert False` always fails, so it is a marker of an arm the \
               test must not reach and no reading of this rule. It does not \
               gate: the narrowing reads 48/52 on the pool that shaped it and \
               7/33 on trees it had never seen, so a project's own assert \
               helpers outrun the predicate.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_44,
};

/// An assertion comparing a call-free expression with itself, or asserting an
/// always-true constant.
fn rule_44(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_test_functions(facts) {
        for at in module.nodes(&[Kind::Assert, Kind::Call], Some(&sym.qname), false) {
            let node = module.nodes[at as usize];
            let Some(why) = tautology(facts, module, node) else {
                continue;
            };
            out.push(Finding {
                rule: "44",
                site: node_site(facts, module, at),
                message: format!("{} {why}", sym.qname),
                cause: format!("tautology:{}:{}", sym.qname, module.line_of(at)),
                evidence: Evidence::ast(),
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #47 sleepy test ----------------------------------------------------------

pub const RULE_47: Rule = Rule {
    record: RuleRecord {
        id: "47",
        slug: "sleepy-test",
        family: "T",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "positive constant sleep inside a test",
        goal: "Wall-clock waits are slow and flaky by construction; synchronize \
               on the condition instead (developers: 62% immediate refactor).",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_47,
};

/// `str(k.value)` of a positive `int` or `float` sleep argument (R18: `True`
/// is an int too, and `repr` prints a float).
fn positive_seconds(arg: &Expr) -> Option<String> {
    match arg {
        Expr::NumberLiteral(n) => match &n.value {
            Number::Int(i) => (i.as_u64() != Some(0)).then(|| i.to_string()),
            Number::Float(f) => (*f > 0.0).then(|| pytext::repr_float(*f)),
            Number::Complex { .. } => None,
        },
        Expr::BooleanLiteral(b) => b.value.then(|| "True".to_string()),
        _ => None,
    }
}

fn rule_47(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_test_functions(facts) {
        for at in module.nodes(&[Kind::Call], Some(&sym.qname), false) {
            let Some(call) = module.call_at(at) else {
                continue;
            };
            if call_name(call) != Some("sleep") {
                continue;
            }
            let Some(seconds) = call.arguments.args.first().and_then(positive_seconds) else {
                continue;
            };
            out.push(Finding {
                rule: "47",
                site: node_site(facts, module, at),
                message: format!(
                    "{} sleeps {seconds}s - wall-clock waits make tests slow and flaky",
                    sym.qname
                ),
                cause: format!("sleepy:{}:{}", sym.qname, module.line_of(at)),
                evidence: Evidence::ast(),
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}
