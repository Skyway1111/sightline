//! Family P (#41): the perf shape catalog, emitted
//! only in the hot set (`py_provers::hotness`). Every entry ships a committed
//! micro-bench proving 2x or better at its pinned n (`xtask perf-catalog`).
//! Shapes ruff PERF covers are excluded. REPORT forever: only bench-ratchet
//! metrics gate.

use std::collections::HashSet;

use ruff_python_ast::{CmpOp, Expr, ExprCall, Stmt, StmtFunctionDef};

use sightline_core::findings::{Evidence, Finding, Sink};
use sightline_core::rule::{Posture, RuleRecord, Scope as RuleScope};
use sightline_py_facts::astutil::{attr_on, chain_root, subnodes};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::{Kind, is_stmt};
use sightline_py_facts::model::{NodeIndex, RepoFacts};
use sightline_py_facts::module::Module;
use sightline_py_facts::unparse;
use sightline_py_provers::Provers;
use sightline_py_provers::catalog::{HTTP_CALLS, RE_CALLS, SUBPROCESS_CALLS};
use sightline_py_provers::hotness::loop_depth;
use sightline_py_provers::scope::Scope;

use crate::model::{MatchCtx, Rule, Shape};
use crate::util::{iter_functions, library_name, node_site};

const LINEAR_ATTR_METHODS: [&str; 3] = ["insert", "remove", "index"];
const GROWERS: [&str; 3] = ["append", "extend", "insert"];
const CHUNKED_READS: [&str; 4] = ["read", "readinto", "iter_content", "iter_lines"];
/// Write kinds a name-store takes: `k = v` / `del k`, then writes through a
/// reference (`k.f = v`, `k[i] = v`). A mutator call is not a store.
const REBOUND: [&str; 2] = ["name", "del"];
const TOUCHED: [&str; 4] = ["name", "del", "attr", "subscript"];

/// A predicate on one call site, as `call_in_loop` hands it over.
type CallPred = fn(&ExprCall, NodeIndex, &Scope, &Module<'_>, &RepoFacts<'_>) -> bool;

/// The function's own scope. `MatchCtx` holds no `Provers`, so this builds
/// one rather than reading `Provers::scope_of`'s memo.
fn scope_of(ctx: &MatchCtx<'_, '_>) -> Option<Scope> {
    Scope::new(ctx.facts, &ctx.sym.qname)
}

/// A hot caller's loop counts as the shape's loop.
fn in_loop(ctx: &MatchCtx<'_, '_>, node: NodeIndex) -> bool {
    loop_depth(ctx.module, node) + ctx.amp > 0
}

/// The name is never rebound inside the loop enclosing `node`.
fn loop_invariant_name(scope: &Scope, facts: &RepoFacts<'_>, node: NodeIndex, name: &str) -> bool {
    scope.enclosing_loop(facts, node).is_some_and(|at| {
        !scope
            .writes_in(facts, at)
            .iter()
            .any(|w| w.root.as_deref() == Some(name) && REBOUND.contains(&w.kind))
    })
}

/// Every call satisfying `pred` in a loop.
fn call_in_loop(ctx: &MatchCtx<'_, '_>, pred: CallPred) -> Vec<NodeIndex> {
    let Some(scope) = scope_of(ctx) else {
        return Vec::new();
    };
    ctx.nodes(&[Kind::Call])
        .into_iter()
        .filter(|at| {
            ctx.module
                .call_at(*at)
                .is_some_and(|call| pred(call, *at, &scope, ctx.module, ctx.facts))
                && in_loop(ctx, *at)
        })
        .collect()
}

fn invariant_deepcopy(
    call: &ExprCall,
    at: NodeIndex,
    scope: &Scope,
    _module: &Module<'_>,
    facts: &RepoFacts<'_>,
) -> bool {
    let named = matches!(&*call.func, Expr::Name(n) if n.id.as_str() == "deepcopy")
        || attr_on(&call.func, &["copy"]) == Some("deepcopy");
    named
        && matches!(call.arguments.args.first(), Some(Expr::Name(n))
            if loop_invariant_name(scope, facts, at, n.id.as_str()))
}

fn invariant_open(
    call: &ExprCall,
    at: NodeIndex,
    scope: &Scope,
    _module: &Module<'_>,
    facts: &RepoFacts<'_>,
) -> bool {
    if !matches!(&*call.func, Expr::Name(n) if n.id.as_str() == "open") {
        return false;
    }
    match call.arguments.args.first() {
        Some(arg) if Cn::Expr(arg).kind() == Kind::Constant => true,
        Some(Expr::Name(n)) => loop_invariant_name(scope, facts, at, n.id.as_str()),
        _ => false,
    }
}

fn constant_pattern_re(
    call: &ExprCall,
    _at: NodeIndex,
    _scope: &Scope,
    module: &Module<'_>,
    _facts: &RepoFacts<'_>,
) -> bool {
    library_name(module, &call.func).is_some_and(|name| RE_CALLS.contains(&&*name))
        && call
            .arguments
            .args
            .first()
            .is_some_and(|arg| Cn::Expr(arg).kind() == Kind::Constant)
}

fn spawns_a_process(
    call: &ExprCall,
    _at: NodeIndex,
    _scope: &Scope,
    module: &Module<'_>,
    _facts: &RepoFacts<'_>,
) -> bool {
    library_name(module, &call.func).is_some_and(|name| SUBPROCESS_CALLS.contains(&&*name))
}

fn opens_a_connection(
    call: &ExprCall,
    at: NodeIndex,
    _scope: &Scope,
    module: &Module<'_>,
    _facts: &RepoFacts<'_>,
) -> bool {
    let http = library_name(module, &call.func).is_some_and(|name| HTTP_CALLS.contains(&&*name))
        || matches!(&*call.func, Expr::Attribute(a) if a.attr.as_str() == "urlopen");
    http && !drained_chunk_wise(module, at)
}

/// The response this call opens is read chunk-wise inside its own `with`
/// body: one bulk transfer, where the handshake a session would save is noise
/// beside the payload. The N+1 shape is many small requests.
fn drained_chunk_wise(module: &Module<'_>, call: NodeIndex) -> bool {
    let block = module
        .parent_of(call)
        .and_then(|item| module.parent_of(item))
        .map(|at| module.nodes[at as usize]);
    let Some(Cn::Stmt(Stmt::With(with))) = block else {
        return false;
    };
    let bound: Vec<String> = with
        .items
        .iter()
        .filter(|item| Cn::Expr(&item.context_expr).stamped() == Some(call))
        .filter_map(|item| item.optional_vars.as_deref())
        .map(unparse::expr)
        .collect();
    with.body
        .iter()
        .flat_map(|st| subnodes(Cn::Stmt(st), |k| k == Kind::Call))
        .any(|node| {
            let Cn::Expr(Expr::Call(c)) = node else {
                return false;
            };
            let Expr::Attribute(a) = &*c.func else {
                return false;
            };
            CHUNKED_READS.contains(&a.attr.as_str())
                && bound.contains(&unparse::expr(&a.value))
                && node.stamped().is_some_and(|at| loop_depth(module, at) > 0)
        })
}

/// `self.seen` -> `"self.seen"`, `seen` -> `"seen"`: the spellings a
/// membership probe uses.
fn list_name(target: &Expr) -> Option<String> {
    match target {
        Expr::Name(n) => Some(n.id.to_string()),
        _ => attr_on(target, &["self"]).map(|_| unparse::expr(target)),
    }
}

/// A list display/comp or `list(...)` call.
fn is_list_value(value: &Expr) -> bool {
    matches!(value, Expr::List(_) | Expr::ListComp(_))
        || matches!(value, Expr::Call(c) if matches!(&*c.func, Expr::Name(n) if n.id.as_str() == "list"))
}

/// The listed name a membership test or linear-method call probes.
fn linear_use(node: Cn<'_>, lists: &HashSet<String>) -> Option<String> {
    let name = match node {
        Cn::Expr(Expr::Compare(cmp)) => {
            let probed = cmp
                .ops
                .iter()
                .any(|op| matches!(op, CmpOp::In | CmpOp::NotIn));
            probed.then(|| list_name(&cmp.comparators[0]))?
        }
        Cn::Expr(Expr::Call(call)) => match &*call.func {
            Expr::Attribute(a) => LINEAR_ATTR_METHODS
                .contains(&a.attr.as_str())
                .then(|| list_name(&a.value))?,
            _ => None,
        },
        _ => None,
    }?;
    lists.contains(&name).then_some(name)
}

/// The local is appended to inside the loop probing it, the bench's dedup
/// shape. A local a loop only reads is sized elsewhere.
fn grown_in_loop(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    scope: &Scope,
    node: NodeIndex,
    name: &str,
) -> bool {
    let Some(at) = scope.enclosing_loop(facts, node) else {
        return false;
    };
    subnodes(module.nodes[at as usize], |k| k == Kind::Call)
        .into_iter()
        .any(|n| match n {
            Cn::Expr(Expr::Call(c)) => matches!(&*c.func, Expr::Attribute(a)
                if GROWERS.contains(&a.attr.as_str())
                    && matches!(&*a.value, Expr::Name(id) if id.id.as_str() == name)),
            _ => false,
        })
}

/// `x in self.seen` / self.seen.insert-remove-index in a loop, where the class
/// initializes `self.seen` as a list, or `x in seen` on a local the function
/// opens empty (`seen = []`) and grows in that loop: the bench's dedup
/// accumulator, whose n is the loop's.
fn match_list_membership(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    let Some(scope) = scope_of(ctx) else {
        return Vec::new();
    };
    let cls = ctx
        .sym
        .parent
        .as_deref()
        .filter(|p| ctx.facts.classes.contains_key(*p));
    let own = ctx.nodes(&[Kind::Assign]);
    let assigns = cls
        .map(|c| ctx.module.nodes(&[Kind::Assign], Some(c), true))
        .unwrap_or_default();
    let assign_at = |at: &NodeIndex| match ctx.module.nodes[*at as usize] {
        Cn::Stmt(Stmt::Assign(a)) => Some(a),
        _ => None,
    };
    // self attrs assigned a list anywhere in the class
    let mut attrs: HashSet<String> = HashSet::new();
    for assign in assigns.iter().chain(own.iter()).filter_map(assign_at) {
        if !is_list_value(&assign.value) {
            continue;
        }
        for name in assign.targets.iter().filter_map(list_name) {
            if name.contains('.') {
                attrs.insert(name);
            }
        }
    }
    // locals assigned exactly `[]`
    let mut scratch: HashSet<String> = HashSet::new();
    for assign in own.iter().filter_map(assign_at) {
        if !matches!(&*assign.value, Expr::List(l) if l.elts.is_empty()) {
            continue;
        }
        for target in &assign.targets {
            if let Expr::Name(n) = target {
                scratch.insert(n.id.to_string());
            }
        }
    }
    if attrs.is_empty() && scratch.is_empty() {
        return Vec::new();
    }
    let lists: HashSet<String> = attrs.union(&scratch).cloned().collect();
    ctx.nodes(&[Kind::Compare, Kind::Call])
        .into_iter()
        .filter(|at| {
            linear_use(ctx.module.nodes[*at as usize], &lists).is_some_and(|name| {
                in_loop(ctx, *at)
                    && (attrs.contains(&name)
                        || grown_in_loop(ctx.facts, ctx.module, &scope, *at, &name))
            })
        })
        .collect()
}

/// `sorted(xs)[0]` / `sorted(xs)[-1]` in a loop (`key=` / `reverse=` kept): a
/// full sort for one extreme. `[:k]` is out, heapq's win depends on k.
fn match_sorted_head(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    ctx.nodes(&[Kind::Subscript])
        .into_iter()
        .filter(|at| {
            let Cn::Expr(Expr::Subscript(sub)) = ctx.module.nodes[*at as usize] else {
                return false;
            };
            let Expr::Call(call) = &*sub.value else {
                return false;
            };
            matches!(&*call.func, Expr::Name(n) if n.id.as_str() == "sorted")
                && call.arguments.args.len() == 1
                && ["0", "-1"].contains(&unparse::expr(&sub.slice).as_str())
                && in_loop(ctx, *at)
        })
        .collect()
}

/// `a` or `a.f` for a loop target a.
fn target_of<'e>(expr: &'e Expr, targets: &HashSet<&str>) -> Option<&'e str> {
    let root = match expr {
        Expr::Attribute(a) => &*a.value,
        other => other,
    };
    match root {
        Expr::Name(n) if targets.contains(n.id.as_str()) => Some(n.id.as_str()),
        _ => None,
    }
}

/// A loop over the same name inside another's span is nested in it (loops
/// cannot overlap without nesting), its body joining the targets: a comparison
/// between the two (or one attribute of each) by ==/!=/in/not in is the join a
/// dict or set replaces, what the bench proved.
fn match_nested_same_collection(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    let loops: Vec<(NodeIndex, &str, &str)> = ctx
        .nodes(&[Kind::For])
        .into_iter()
        .filter_map(|at| match ctx.module.nodes[at as usize] {
            Cn::Stmt(Stmt::For(f)) => match (&*f.target, &*f.iter) {
                (Expr::Name(target), Expr::Name(iter)) => {
                    Some((at, target.id.as_str(), iter.id.as_str()))
                }
                _ => None,
            },
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for (outer, outer_target, outer_iter) in &loops {
        for (inner, inner_target, inner_iter) in &loops {
            let targets: HashSet<&str> = [*outer_target, *inner_target].into_iter().collect();
            if inner == outer
                || !(module_span(ctx.module, *outer).0 <= ctx.module.line_of(*inner)
                    && ctx.module.line_of(*inner) <= module_span(ctx.module, *outer).1)
                || inner_iter != outer_iter
            {
                continue;
            }
            let Cn::Stmt(Stmt::For(body)) = ctx.module.nodes[*inner as usize] else {
                continue;
            };
            let joins = body
                .body
                .iter()
                .flat_map(|st| subnodes(Cn::Stmt(st), |k| k == Kind::Compare))
                .any(|node| {
                    let Cn::Expr(Expr::Compare(cmp)) = node else {
                        return false;
                    };
                    if cmp.ops.len() != 1
                        || !matches!(
                            cmp.ops[0],
                            CmpOp::Eq | CmpOp::NotEq | CmpOp::In | CmpOp::NotIn
                        )
                    {
                        return false;
                    }
                    let left = target_of(&cmp.left, &targets);
                    let right = target_of(&cmp.comparators[0], &targets);
                    let pair: HashSet<&str> = [left, right].into_iter().flatten().collect();
                    left.is_some() && right.is_some() && pair == targets
                });
            if joins {
                out.push(*inner);
            }
        }
    }
    out
}

fn module_span(module: &Module<'_>, node: NodeIndex) -> (u32, u32) {
    (module.line_of(node), module.end_line_of(node))
}

/// k of `x.f <op> k` / `k <op> x.f` (one attribute on the loop target).
fn filter_key<'e>(test: &'e Expr, target: &str, op: CmpOp) -> Option<&'e Expr> {
    let Expr::Compare(cmp) = test else {
        return None;
    };
    if cmp.ops.len() != 1 || cmp.ops[0] != op {
        return None;
    }
    let (left, right) = (&*cmp.left, &cmp.comparators[0]);
    if attr_on(left, &[target]).is_some() {
        return Some(right);
    }
    attr_on(right, &[target]).is_some().then_some(left)
}

/// NotEq for `if ...: continue` opening the body, Eq for an `if` that is the
/// whole body; `None` otherwise (else-branches disqualify).
fn guard_op(body: &[Stmt]) -> Option<CmpOp> {
    let Some(Stmt::If(guard)) = body.first() else {
        return None;
    };
    if !guard.elif_else_clauses.is_empty() {
        return None;
    }
    if guard.body.len() == 1 && matches!(guard.body[0], Stmt::Continue(_)) {
        return Some(CmpOp::NotEq);
    }
    (body.len() == 1).then_some(CmpOp::Eq)
}

/// The loop scans a dict's `.values()` and opens with a filter on one
/// attribute of its (Name) target against a key invariant across the loop: a
/// dict keyed by the wrong field. Lists and tuples are out.
fn filter_scan_guard(
    facts: &RepoFacts<'_>,
    scope: &Scope,
    at: NodeIndex,
    loop_stmt: &ruff_python_ast::StmtFor,
) -> bool {
    let scans_values = matches!(&*loop_stmt.iter, Expr::Call(c)
        if c.arguments.args.is_empty()
            && matches!(&*c.func, Expr::Attribute(a) if a.attr.as_str() == "values"));
    if !scans_values {
        return false;
    }
    let Expr::Name(target) = &*loop_stmt.target else {
        return false;
    };
    let Some(op) = guard_op(&loop_stmt.body) else {
        return false;
    };
    let Some(Stmt::If(guard)) = loop_stmt.body.first() else {
        return false;
    };
    let Some(key) = filter_key(&guard.test, target.id.as_str(), op) else {
        return false;
    };
    if Cn::Expr(key).kind() == Kind::Constant {
        return true;
    }
    // the key's root is never stored in the loop
    chain_root(key, &[Kind::Attribute]).is_some_and(|root| {
        !scope
            .writes_in(facts, at)
            .iter()
            .any(|w| w.root.as_deref() == Some(root) && TOUCHED.contains(&w.kind))
    })
}

/// `for x in C:` opening with `if x.f != k: continue`, or `if x.f == k:`
/// around the whole body, k loop-invariant: a linear scan per probe.
fn match_filter_scan(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    let Some(scope) = scope_of(ctx) else {
        return Vec::new();
    };
    ctx.nodes(&[Kind::For, Kind::AsyncFor])
        .into_iter()
        .filter_map(|at| {
            let Cn::Stmt(Stmt::For(loop_stmt)) = ctx.module.nodes[at as usize] else {
                return None;
            };
            if !filter_scan_guard(ctx.facts, &scope, at, loop_stmt) {
                return None;
            }
            Cn::Stmt(loop_stmt.body.first()?).stamped()
        })
        .collect()
}

/// `s += part` in a loop on a local initialized to a str literal. Local loop
/// only: the accumulator is fresh per call, so a hot caller's loop never makes
/// it quadratic.
fn match_str_concat_in_loop(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    let mut str_locals: HashSet<String> = HashSet::new();
    for at in ctx.nodes(&[Kind::Assign]) {
        let Cn::Stmt(Stmt::Assign(assign)) = ctx.module.nodes[at as usize] else {
            continue;
        };
        if !matches!(&*assign.value, Expr::StringLiteral(_)) {
            continue;
        }
        for target in &assign.targets {
            if let Expr::Name(n) = target {
                str_locals.insert(n.id.to_string());
            }
        }
    }
    ctx.nodes(&[Kind::AugAssign])
        .into_iter()
        .filter(|at| {
            let Cn::Stmt(Stmt::AugAssign(aug)) = ctx.module.nodes[*at as usize] else {
                return false;
            };
            aug.op == ruff_python_ast::Operator::Add
                && matches!(&*aug.target, Expr::Name(n) if str_locals.contains(n.id.as_str()))
                && loop_depth(ctx.module, *at) > 0
        })
        .collect()
}

/// `any([...])` / `all([...])` in a loop (ruff's C419 is a comprehension lint,
/// not a perf rule; the loop makes it hot).
fn match_materialized_short_circuit(
    _fn: &StmtFunctionDef,
    ctx: &MatchCtx<'_, '_>,
) -> Vec<NodeIndex> {
    ctx.nodes(&[Kind::Call])
        .into_iter()
        .filter(|at| {
            let Some(call) = ctx.module.call_at(*at) else {
                return false;
            };
            matches!(&*call.func, Expr::Name(n) if ["any", "all"].contains(&n.id.as_str()))
                && call.arguments.args.len() == 1
                && matches!(call.arguments.args[0], Expr::ListComp(_))
                && in_loop(ctx, *at)
        })
        .collect()
}

fn match_deepcopy_in_loop(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    call_in_loop(ctx, invariant_deepcopy)
}

fn match_open_in_loop(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    call_in_loop(ctx, invariant_open)
}

fn match_re_in_loop(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    call_in_loop(ctx, constant_pattern_re)
}

fn match_subprocess_in_loop(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    call_in_loop(ctx, spawns_a_process)
}

fn match_http_in_loop(_fn: &StmtFunctionDef, ctx: &MatchCtx<'_, '_>) -> Vec<NodeIndex> {
    call_in_loop(ctx, opens_a_connection)
}

/// Inside a `raise` statement's expression: the exception path runs once per
/// call and ends it (`sorted(unknown)[0]` in a message).
fn on_the_raise_path(module: &Module<'_>, node: NodeIndex) -> bool {
    let mut cur = Some(node);
    while let Some(at) = cur {
        if is_stmt(module.nodes[at as usize].kind()) {
            return module.nodes[at as usize].kind() == Kind::Raise;
        }
        cur = module.parent_of(at);
    }
    false
}

/// Admission ticket: the committed micro-bench proves each pair at its pinned
/// n. Refused by that bench (measured, not plausible): db-execute-in-loop at
/// 1.73x (sqlite executemany, n=1000).
pub static PERF_CATALOG: [(&str, Shape); 11] = [
    (
        "list-attr-membership",
        Shape {
            matcher: match_list_membership,
            suggestion: "membership on a list is O(n) per probe; keep a set",
            trigger: None,
        },
    ),
    (
        "sorted-head",
        Shape {
            matcher: match_sorted_head,
            suggestion: "sorted(...)[0]/[-1] sorts n log n for one extreme; min/max is linear",
            trigger: None,
        },
    ),
    (
        "nested-same-collection",
        Shape {
            matcher: match_nested_same_collection,
            suggestion: "nested loops over one collection are O(n^2); group via dict/set",
            trigger: None,
        },
    ),
    (
        "deepcopy-in-loop",
        Shape {
            matcher: match_deepcopy_in_loop,
            suggestion: "deepcopy of a loop-invariant value; hoist the copy",
            trigger: None,
        },
    ),
    (
        "re-in-loop",
        Shape {
            matcher: match_re_in_loop,
            suggestion: "re.* with a constant pattern in a loop; hoist re.compile",
            trigger: None,
        },
    ),
    (
        "open-in-loop",
        Shape {
            matcher: match_open_in_loop,
            suggestion: "reopening the same file per iteration; open once outside",
            trigger: None,
        },
    ),
    (
        "str-concat-in-loop",
        Shape {
            matcher: match_str_concat_in_loop,
            suggestion: "string += in a loop; build a list and join once",
            trigger: None,
        },
    ),
    (
        "subprocess-in-loop",
        Shape {
            matcher: match_subprocess_in_loop,
            suggestion: "process spawn per iteration; batch the invocation",
            trigger: None,
        },
    ),
    (
        "http-in-loop",
        Shape {
            matcher: match_http_in_loop,
            suggestion: "connection per request is the N+1 shape; reuse a session",
            trigger: None,
        },
    ),
    (
        "materialized-short-circuit",
        Shape {
            matcher: match_materialized_short_circuit,
            suggestion: "any/all over a list comprehension defeats short-circuit; drop []",
            trigger: None,
        },
    ),
    (
        "filter-scan",
        Shape {
            matcher: match_filter_scan,
            suggestion: "dict values scanned by f per probe; key (or index) the dict by f",
            trigger: None,
        },
    ),
];

pub const RULE_41: Rule = Rule {
    record: RuleRecord {
        id: "41",
        slug: "perf-catalog",
        family: "P",
        engine_class: "WP",
        posture: Posture::Report,
        meaning: "proven quadratic/invariant-hoist/N+1/filter-scan shapes in hot \
                  code only; a hot caller's loop counts as the shape's loop",
        goal: "Perf advice belongs where perf matters: cold glue stays \
               clean-and-simple, and only measured walls may gate.",
        lang: "py",
        scope: RuleScope::Repo,
        complement: "ruff PERF covers cold-glue perf shapes; #41 prices the hot set only",
    },
    run: rule_41,
};

/// Catalog shapes inside hot-reachable functions only; salience is the path
/// amplification plus the local loop depth.
fn rule_41(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let hot = provers.hot(facts);
    for (module, sym) in iter_functions(facts) {
        let Some(amp) = hot.amplification.get(&sym.qname).copied() else {
            continue;
        };
        let ctx = MatchCtx {
            facts,
            module,
            sym,
            amp,
        };
        let def = crate::util::fn_of(module, sym);
        for (name, shape) in &PERF_CATALOG {
            for node in (shape.matcher)(def, &ctx) {
                if on_the_raise_path(module, node) {
                    continue;
                }
                out.push(Finding {
                    rule: "41",
                    site: node_site(facts, module, node),
                    message: format!(
                        "{name} in hot {} (amplification {amp}): {}",
                        sym.qname, shape.suggestion
                    ),
                    cause: format!("perf:{name}:{}:{}", sym.qname, module.line_of(node)),
                    evidence: Evidence::Wp {
                        premises: vec!["hot-reachable".to_string(), format!("amplification:{amp}")],
                    },
                    salience: f64::from(amp + loop_depth(module, node)),
                    fix: None,
                    lang: "py",
                });
            }
        }
    }
}
