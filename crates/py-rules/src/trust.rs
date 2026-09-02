//! Family A: what the code claims about itself (#1, #2, #3, #5, #6, #7, #9,
//! #10, #40, #49, #50). Thin queries over facts and provers; evidence objects
//! stamp the engine.

use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use regex::Regex;
use ruff_python_ast::{CmpOp, Expr, ExprCall, ExprName, Number, Operator, Parameter, Stmt};

use sightline_core::catalog::CONTAINER;
use sightline_core::findings::{Evidence, Finding, Qname, Sink, Site, SpanEdit};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::astutil::{
    RECEIVERS, fn_args, fn_defaults, fn_pos_args, is_call_stmt, is_mutable_init, subnodes,
    without_receiver,
};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{
    FUNCTION_KINDS, NodeIndex, RefKind, RepoFacts, Span, Symbol, is_test_path,
};
use sightline_py_facts::module::Module;
use sightline_py_facts::qnames::resolve_dotted_expr;
use sightline_py_facts::unparse;
use sightline_py_provers::Provers;
use sightline_py_provers::annotations::{annotation_names, none_inclusive, weakness};
use sightline_py_provers::catalog::{IMPORT_TIME_MUTATORS, classes_of};
use sightline_py_provers::counterfactual::Splice;
use sightline_py_provers::grounding::{
    broken_declaration, container_shape_check, grounding, none_default_lie,
};
use sightline_py_provers::imports::{importers, under_main_guard};
use sightline_py_provers::scope::Footprint;
use sightline_py_provers::typestrings::{
    deliteral, generic_base, join, split_union, union_members,
};

use crate::framework::is_override_fixed;
use crate::model::Rule;
use crate::util::{
    enclosing_at_line, fn_of, in_typed_scope, is_boundary, is_exported, is_nested, iter_functions,
    iter_prod_functions, node_site, raw_docstring,
};

/// A parameter's declared type (R15: never `Parameter.annotation`).
fn annotation_of<'a>(module: &'a Module<'_>, param: &Parameter) -> Option<&'a Expr> {
    module.annotation(param_at(param))
}

/// A parameter's own node index; every `arg` the traversal reached has one.
fn param_at(param: &Parameter) -> NodeIndex {
    Cn::Param(param).stamped().unwrap_or(0)
}

fn is_name(node: Option<&Expr>, name: &str) -> bool {
    matches!(node, Some(Expr::Name(n)) if n.id.as_str() == name)
}

/// Every expression of one kind under a node, `ast.walk` order.
fn exprs_under<'a>(root: Cn<'a>, kind: Kind) -> impl Iterator<Item = &'a Expr> {
    subnodes(root, move |k| k == kind)
        .into_iter()
        .filter_map(|n| match n {
            Cn::Expr(e) => Some(e),
            _ => None,
        })
}

/// Every `Name` under a node, `ast.walk` order.
fn names_under<'a>(root: Cn<'a>) -> Vec<&'a ExprName> {
    exprs_under(root, Kind::Name)
        .filter_map(Expr::as_name_expr)
        .collect()
}

fn calls_under<'a>(root: Cn<'a>) -> Vec<&'a ExprCall> {
    exprs_under(root, Kind::Call)
        .filter_map(Expr::as_call_expr)
        .collect()
}

// --- #2 locally-redundant check (oracle) -------------------------------------

pub const RULE_2: Rule = Rule {
    record: RuleRecord {
        id: "2",
        slug: "locally-redundant-check",
        family: "A",
        engine_class: "ORACLE",
        // not GATE: even clean code has real hits
        posture: Posture::Ratchet,
        meaning: "isinstance/comparison/contains/cast provable from annotations",
        goal: "Guards die when invariants are named (Sean Parent's Chromium \
               walkthrough): a check the types already discharge is noise.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_2,
};

/// One assumption: the type that discharges the check is a claim this repo
/// wrote (`grounding`). The checker's own inference grounds nothing, and the
/// ungrounded arm is buried (decisions.tsv, g4/cut).
fn rule_2(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    // caller evidence for the None-default veto
    let arg_types = provers.arg_types(facts);
    let rejected = provers.rejected_bindings();
    for (module, d) in provers.diagnostics(facts) {
        // the annotation is the defect (#1 owns it), the body broke it and the
        // verdict reads the declaration not the value, or the check is the
        // boundary validation the declaration rests on
        if none_default_lie(d, facts, provers)
            || broken_declaration(d, facts, provers, &rejected)
            || container_shape_check(d, facts)
            || !grounding(d, facts, provers, arg_types, &rejected)
        {
            continue;
        }
        let short = pytext::lower(pytext::removeprefix(&d.rule, "reportUnnecessary"));
        out.push(Finding {
            rule: "2",
            site: Site {
                rel: d.rel.clone(),
                line: d.line,
                col: d.col,
                symbol: enclosing_at_line(facts, module, d.line).into(),
            },
            message: d.message.clone(),
            cause: format!("redundant:{short}"),
            evidence: Evidence::Oracle {
                rule: d.rule.clone(),
                grounded: true,
                message: d.message.clone(),
            },
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

// --- #1 weak boundary types --------------------------------------------------

/// The splat's callee is a def this repo wrote which names the params the
/// splat feeds: what the forwarder leaves opaque, the repo could spell.
fn spellable(facts: &RepoFacts<'_>, module: &Module<'_>, func: &Expr, star: &str) -> bool {
    let mut q: String = resolve_dotted_expr(func, module, facts)
        .map(|q| q.to_string())
        .unwrap_or_default();
    if let Some(class) = facts.classes.get(&*q) {
        q = class
            .methods
            .get("__init__")
            .map(|m| m.to_string())
            .unwrap_or_default();
    }
    let Some(sym) = facts.symbols.get(&*q) else {
        return false;
    };
    if !FUNCTION_KINDS.contains(&sym.kind) {
        return false;
    }
    let params = &fn_of(&facts.modules[&sym.module], sym).parameters;
    if star == "*" {
        params.vararg.is_none()
    } else {
        params.kwarg.is_none()
    }
}

/// Every load of the star param is splatted into a call whose accepted set
/// this signature cannot spell: an external callee, a callable parameter, a
/// callee star-taking itself. Then the set is the callee's, and naming it here
/// would only copy a signature the repo does not own.
fn forwarded_on(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    def: Cn<'_>,
    star: &Parameter,
    prefix: &str,
) -> bool {
    let name = star.name.as_str();
    let loads = names_under(def)
        .into_iter()
        .filter(|n| n.id.as_str() == name)
        .count();
    let mut splatted: HashSet<usize> = HashSet::new();
    for call in calls_under(def) {
        let mut splats: Vec<&Expr> = call
            .arguments
            .args
            .iter()
            .filter_map(|a| match a {
                Expr::Starred(s) => Some(&*s.value),
                _ => None,
            })
            .collect();
        splats.extend(
            call.arguments
                .keywords
                .iter()
                .filter(|k| k.arg.is_none())
                .map(|k| &k.value),
        );
        let hits: HashSet<usize> = splats
            .into_iter()
            .flat_map(|expr| names_under(Cn::Expr(expr)))
            .filter(|n| n.id.as_str() == name)
            .map(|n| std::ptr::from_ref(n) as usize)
            .collect();
        if !hits.is_empty() && spellable(facts, module, &call.func, prefix) {
            return false;
        }
        splatted.extend(hits);
    }
    loads > 0 && splatted.len() == loads
}

pub const RULE_1: Rule = Rule {
    record: RuleRecord {
        id: "1",
        slug: "weak-boundary-types",
        family: "A",
        engine_class: "AST",
        // not GATE: clean code legitimately has Any/dict ML-config boundaries
        posture: Posture::Ratchet,
        meaning: "Any / bare dict / opaque *args, **kwargs in public signatures",
        goal: "The signature is the published contract: weak boundary types \
               make every caller re-derive what the function accepts.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_1,
};

/// Two arms: a `= None` default contradicting its own annotation (any prod
/// def), and weak annotations on a public boundary's params. An opaque star
/// param counts only beside a declared one. The weak arm reads published
/// boundaries the repo chose: not a closure, not a file outside its own
/// type-check scope, not a method whose class answers to an external base.
fn rule_1(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    // prod only: test signatures are not API boundaries
    for (module, sym) in iter_prod_functions(facts) {
        let fn_def = fn_of(module, sym);
        // lie arm, boundary or not: a non-Optional annotation contradicted by
        // its own `= None` default is the root defect (#2 defers here)
        for (arg, default) in fn_defaults(fn_def) {
            let Some(ann) = annotation_of(module, arg) else {
                continue;
            };
            if matches!(default, Expr::NoneLiteral(_))
                && !none_inclusive(facts, &module.bindings, ann)
            {
                out.push(Finding {
                    rule: "1",
                    site: node_site(facts, module, param_at(arg)),
                    message: format!(
                        "'{}: {} = None' in {}: the default contradicts the annotation",
                        arg.name,
                        unparse::expr(ann),
                        sym.qname
                    ),
                    cause: format!("lying-default:{}:{}", sym.qname, arg.name),
                    evidence: Evidence::Ast {
                        detail: "lying None default".to_string(),
                    },
                    salience: 0.0,
                    fix: None,
                    lang: "py",
                });
            }
        }
        if !(is_boundary(facts, sym)
            && !is_nested(facts, sym)
            && in_typed_scope(facts, &module.rel)
            && !is_override_fixed(facts, sym))
        {
            continue;
        }
        let args = fn_args(fn_def);
        for arg in &args {
            if let Some(weak) = weakness(annotation_of(module, arg)) {
                out.push(Finding {
                    rule: "1",
                    site: node_site(facts, module, param_at(arg)),
                    message: format!(
                        "public boundary param '{}' of {}: {weak}",
                        arg.name, sym.qname
                    ),
                    cause: format!("weak:{}:{}", sym.qname, arg.name),
                    evidence: Evidence::Ast { detail: weak },
                    salience: 0.0,
                    fix: None,
                    lang: "py",
                });
            }
        }
        let stars = [
            ("*", fn_def.parameters.vararg.as_deref()),
            ("**", fn_def.parameters.kwarg.as_deref()),
        ];
        let declares = args
            .iter()
            .copied()
            .chain(stars.iter().filter_map(|(_, s)| *s))
            .any(|a| annotation_of(module, a).is_some());
        for (prefix, star) in stars {
            let Some(star) = star.filter(|_| declares) else {
                continue;
            };
            if annotation_of(module, star).is_none()
                && !forwarded_on(facts, module, module.nodes[sym.node as usize], star, prefix)
            {
                out.push(Finding {
                    rule: "1",
                    site: node_site(facts, module, param_at(star)),
                    message: format!(
                        "opaque {prefix}{} on public boundary {}",
                        star.name, sym.qname
                    ),
                    cause: format!("weak:{}:{prefix}{}", sym.qname, star.name),
                    evidence: Evidence::Ast {
                        detail: format!("opaque {prefix}{}", star.name),
                    },
                    salience: 0.0,
                    fix: None,
                    lang: "py",
                });
            }
        }
        let returns = module.returns(sym.node);
        if let Some(weak_ret) = weakness(returns) {
            // a return annotation a `# type:` comment spelled has no node of
            // its own; CPython copies the def's location onto it
            let at = returns
                .and_then(|ann| Cn::Expr(ann).stamped())
                .unwrap_or(sym.node);
            out.push(Finding {
                rule: "1",
                site: node_site(facts, module, at),
                message: format!("public boundary return of {}: {weak_ret}", sym.qname),
                cause: format!("weak:{}:return", sym.qname),
                evidence: Evidence::Ast { detail: weak_ret },
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #5's candidate universe -------------------------------------------------

/// `(qname, symbol, module)` per called function whose caller set is closed:
/// #5's candidate universe. Empty without an oracle, since nothing can be
/// established that no caller said.
fn called_closed_world<'a, 't>(
    facts: &'a RepoFacts<'t>,
    provers: &Provers,
) -> Vec<(&'a Qname, &'a Symbol, &'a Module<'t>)> {
    if provers.no_oracle() {
        return Vec::new();
    }
    let world = provers.closed_world(facts);
    let calls_to = &provers.calls(facts).calls_to;
    let mut out = Vec::new();
    for (qname, sym) in facts.symbols.iter() {
        if calls_to.get(qname).is_none_or(|c| c.is_empty()) || !world.verdict(qname).passed {
            continue;
        }
        out.push((qname, sym, &facts.modules[&sym.module]));
    }
    out
}

// --- #5 proof lifting --------------------------------------------------------

const SEQ_OK_CALLS: [&str; 2] = ["count", "index"];
const MAP_OK_CALLS: [&str; 4] = ["get", "keys", "values", "items"];

/// The weakest protocol a body's footprint needs in place of the concrete
/// `base[inner]`, or `None` when the body demands the concrete type. #10's
/// ladder: what it answers is spliced and verified, never printed as advice.
fn protocol_for(base: &str, inner: &str, fp: Option<&Footprint>) -> Option<String> {
    let item = if inner.is_empty() { "object" } else { inner };
    // forwarded: the callee may demand the concrete type
    let fp = fp?;
    if fp.other || fp.mutated || fp.sub_stored || !fp.attrs.is_empty() || !fp.forwarded.is_empty() {
        return None;
    }
    let extra = |ok: &[&str]| fp.called.iter().any(|c| !ok.contains(&c.as_str()));
    if base == "list" && !extra(&SEQ_OK_CALLS) {
        if fp.subscripted {
            return Some(format!("Sequence[{item}]"));
        }
        if fp.contained {
            return Some(format!("Collection[{item}]"));
        }
        if fp.iterated || fp.sized {
            return Some(format!("Iterable[{item}]"));
        }
    } else if base == "dict" && !extra(&MAP_OK_CALLS) {
        if fp.subscripted || !fp.called.is_empty() || fp.iterated || fp.contained {
            return Some(if inner.is_empty() {
                "Mapping".to_string()
            } else {
                format!("Mapping[{inner}]")
            });
        }
    } else if base == "set" && fp.called.is_empty() {
        if fp.contained {
            return Some(format!("Collection[{item}]"));
        }
        if fp.iterated {
            return Some(format!("Iterable[{item}]"));
        }
    }
    None
}

/// The expression `param` keys into at this node: `X[param]`,
/// `X.get(param, ..)` (get/pop/setdefault), `param in X`; else `None`.
fn keyed_mapping<'a>(node: Cn<'a>, param: &str) -> Option<&'a Expr> {
    match node {
        Cn::Expr(Expr::Subscript(s)) if is_name(Some(&s.slice), param) => Some(&s.value),
        Cn::Expr(Expr::Call(c)) => {
            let Expr::Attribute(f) = &*c.func else {
                return None;
            };
            let keyed = matches!(f.attr.as_str(), "get" | "pop" | "setdefault")
                && !c.arguments.args.is_empty();
            (keyed && is_name(c.arguments.args.first(), param)).then_some(&*f.value)
        }
        Cn::Expr(Expr::Compare(c)) if is_name(Some(&c.left), param) => {
            let membership = c.ops.len() == 1 && matches!(c.ops[0], CmpOp::In | CmpOp::NotIn);
            membership.then(|| c.comparators.first()).flatten()
        }
        _ => None,
    }
}

/// CPython's `type(value).__name__` for the constants a dict literal keys on.
fn constant_type_name(key: &Expr) -> Option<&'static str> {
    Some(match key {
        Expr::StringLiteral(_) => "str",
        Expr::BytesLiteral(_) => "bytes",
        Expr::BooleanLiteral(_) => "bool",
        Expr::NoneLiteral(_) => "NoneType",
        Expr::EllipsisLiteral(_) => "ellipsis",
        Expr::NumberLiteral(n) => match n.value {
            Number::Int(_) => "int",
            Number::Float(_) => "float",
            Number::Complex { .. } => "complex",
        },
        _ => return None,
    })
}

/// The one Python type of every key of the repo-declared dict literal `expr`
/// names (a module name or `self.NAME`), else `None`.
fn literal_key_type(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    owner_q: &str,
    expr: &Expr,
) -> Option<&'static str> {
    let q: String = match expr {
        Expr::Name(n) => module.bindings.get(n.id.as_str())?.to_string(),
        Expr::Attribute(a) => {
            let Expr::Name(base) = &*a.value else {
                return None;
            };
            if !RECEIVERS.contains(&base.id.as_str()) {
                return None;
            }
            let owner = owner_q.rsplit_once('.').map_or(owner_q, |(head, _)| head);
            format!("{owner}.{}", a.attr)
        }
        _ => return None,
    };
    let sym = facts.symbols.get(&*q)?;
    let home = facts.modules.get(&sym.module)?;
    let value = match home.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::Assign(a)) => Some(&*a.value),
        Cn::Stmt(Stmt::AnnAssign(a)) => a.value.as_deref(),
        _ => None,
    };
    let Some(Expr::Dict(dict)) = value else {
        return None;
    };
    if dict.items.is_empty() {
        return None;
    }
    let mut types: BTreeSet<&'static str> = BTreeSet::new();
    for item in &dict.items {
        types.insert(constant_type_name(item.key.as_ref()?)?);
    }
    (types.len() == 1).then(|| types.into_iter().next().expect("one member"))
}

/// Body-usage widening, guard form: an isinstance arm naming a type outside
/// the caller-observed union means the lift would encode a contract the body
/// contradicts. Keying a repo dict literal whose keys share one type is the
/// same guard.
fn body_accepts_wider(
    param: &str,
    members: &[String],
    facts: &RepoFacts<'_>,
    provers: &Provers,
    module: &Module<'_>,
    owner_q: &str,
) -> bool {
    let bases: BTreeSet<&str> = members.iter().map(|m| generic_base(m)).collect();
    let Some(scope) = provers.scope_of(facts, owner_q) else {
        return false;
    };
    if scope.guards(facts).iter().any(|g| {
        g.param == param
            && g.kind == "isinstance"
            && g.classes.iter().any(|c| !bases.contains(c.as_str()))
    }) {
        return true;
    }
    module
        .nodes(
            &[Kind::Subscript, Kind::Call, Kind::Compare],
            Some(owner_q),
            true,
        )
        .into_iter()
        .any(|at| {
            keyed_mapping(module.nodes[at as usize], param)
                .and_then(|x| literal_key_type(facts, module, owner_q, x))
                .is_some_and(|t| !bases.contains(t))
        })
}

/// What one verified lift prints.
struct LiftSite {
    qname: Qname,
    param: String,
    prod_calls: usize,
    module: Qname,
    func: NodeIndex,
}

pub const RULE_5: Rule = Rule {
    record: RuleRecord {
        id: "5",
        slug: "proof-lifting",
        family: "A",
        engine_class: "WP+ORACLE",
        posture: Posture::Ratchet,
        meaning: "propose annotations callers already prove; never auto-apply",
        goal: "Name the invariant once (Sean Parent's Chromium walkthrough): \
               a lifted annotation converts global analysis into permanent \
               local checks.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_5,
};

/// Unannotated params whose prod call sites agree on one or two types,
/// spliced into a counterfactual world and reported only where the lift broke
/// nothing. The message holds the verified spelling and nothing else: a
/// protocol the body would admit is #10's to splice and prove.
fn rule_5(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let mut splices: Vec<Splice> = Vec::new();
    let mut display: IndexMap<String, LiftSite> = IndexMap::new();
    for (qname, sym, module) in called_closed_world(facts, provers) {
        let fn_def = fn_of(module, sym);
        // defaulted params are candidates too
        for arg in fn_pos_args(fn_def) {
            if annotation_of(module, arg).is_some() {
                continue;
            }
            let Some(rows) = provers.arg_types(facts).for_param(qname, arg.name.as_str()) else {
                continue;
            };
            if rows.is_empty() {
                continue;
            }
            // the default row (call None) is an invariant source: it joins the
            // union alongside prod call sites
            let prod_rows: Vec<_> = rows
                .iter()
                .filter(|r| match r.call {
                    None => true,
                    Some(at) => facts
                        .call_sites
                        .get(at as usize)
                        .and_then(|c| facts.rel_of(&c.module))
                        .is_none_or(|rel| !is_test_path(rel)),
                })
                .collect();
            let prod_calls = prod_rows.iter().filter(|r| r.call.is_some()).count();
            if prod_calls == 0 || prod_rows.iter().any(|r| r.ty.is_none()) {
                continue;
            }
            // absence of type information is no invariant
            let mut observed: Vec<String> = Vec::new();
            let mut opaque = false;
            for row in &prod_rows {
                match union_members(row.ty.as_deref().unwrap_or("")) {
                    Some(members) => observed.extend(members),
                    None => opaque = true,
                }
            }
            if opaque {
                continue;
            }
            let members = join(&observed);
            // a three-way disagreement, and only-None-observed, are no evidence
            if !(1..=2).contains(&members.len()) || members == ["None"] {
                continue;
            }
            // an isinstance arm names a type callers never sent
            if body_accepts_wider(arg.name.as_str(), &members, facts, provers, module, qname) {
                continue;
            }
            let verified = members.join(" | ");
            let pid = format!("{qname}:{}", arg.name);
            let span = module.span(param_at(arg)).unwrap_or_default();
            let end = span[3].unwrap_or(0);
            splices.push(Splice {
                id: pid.clone(),
                owner: qname.to_string(),
                spelling: verified.clone(),
                param: arg.name.to_string(),
                edits: vec![SpanEdit {
                    line: span[0].unwrap_or(0),
                    col_start: end,
                    col_end: end,
                    text: format!(": {verified}"),
                }],
                imports: Vec::new(),
            });
            display.insert(
                pid,
                LiftSite {
                    qname: qname.clone(),
                    param: arg.name.to_string(),
                    prod_calls,
                    module: module.qname.clone(),
                    func: sym.node,
                },
            );
        }
    }
    for (pid, (evidence, fix)) in provers.verify_splice(facts, &splices) {
        let Some(site) = display.get(&pid) else {
            continue;
        };
        // as the file spells it, after respelling
        let verified = pytext::removeprefix(&fix.edits[0].text, ": ");
        let receipt = match &evidence {
            Evidence::Counterfactual { receipt } => {
                format!("body check now provably redundant ({receipt})")
            }
            _ => "counterfactual application produced no caller errors".to_string(),
        };
        let module = &facts.modules[&site.module];
        out.push(Finding {
            rule: "5",
            site: node_site(facts, module, site.func),
            message: format!(
                "lift `{}: {verified}` in {} - established at all {} prod call \
                 sites; receipt: {receipt}",
                site.param, site.qname, site.prod_calls
            ),
            cause: format!("lift:{pid}"),
            evidence,
            salience: 0.0,
            fix: Some(fix),
            lang: "py",
        });
    }
}

// --- #10 over-constrained parameter ------------------------------------------

/// One widening the AST footprint proposes.
struct WidenSite {
    module: Qname,
    qname: Qname,
    param: String,
    arg: NodeIndex,
    annotation: String,
    suggestion: String,
}

/// One candidate per over-demanded param of a function with a prod caller: a
/// widening verified over no callers is vacuously verified.
fn widening_candidates(facts: &RepoFacts<'_>, provers: &Provers) -> Vec<WidenSite> {
    let calls_to = &provers.calls(facts).calls_to;
    let mut out: Vec<WidenSite> = Vec::new();
    for (module, sym) in iter_functions(facts) {
        // the dispatch contract owns the signature; no caller, no door
        let called_in_prod = calls_to
            .get(&sym.qname)
            .map_or(&[][..], |v| v)
            .iter()
            .any(|at| {
                facts
                    .call_sites
                    .get(*at as usize)
                    .and_then(|c| facts.rel_of(&c.module))
                    .is_some_and(|rel| !is_test_path(rel))
            });
        if is_override_fixed(facts, sym) || !called_in_prod {
            continue;
        }
        let fn_def = fn_of(module, sym);
        let footprints = provers
            .scope_of(facts, &sym.qname)
            .map(|s| s.footprints(facts));
        for arg in fn_args(fn_def) {
            let Some(ann) = annotation_of(module, arg) else {
                continue;
            };
            let (base, inner) = match ann {
                Expr::Name(n) => (n.id.as_str(), String::new()),
                Expr::Subscript(s) => {
                    let Expr::Name(head) = &*s.value else {
                        continue;
                    };
                    let inner = match &*s.slice {
                        // a bare tuple slice would unparse with parens (R10)
                        Expr::Tuple(t) => t
                            .elts
                            .iter()
                            .map(unparse::expr)
                            .collect::<Vec<_>>()
                            .join(", "),
                        other => unparse::expr(other),
                    };
                    (head.id.as_str(), inner)
                }
                _ => continue,
            };
            // `provers.footprint` drops the receiver's own key
            let fp = footprints
                .filter(|_| !RECEIVERS.contains(&arg.name.as_str()))
                .and_then(|f| f.get(arg.name.as_str()));
            if let Some(suggestion) = protocol_for(&pytext::lower(base), &inner, fp) {
                out.push(WidenSite {
                    module: module.qname.clone(),
                    qname: sym.qname.clone(),
                    param: arg.name.to_string(),
                    arg: param_at(arg),
                    annotation: unparse::expr(ann),
                    suggestion,
                });
            }
        }
    }
    out
}

pub const RULE_10: Rule = Rule {
    record: RuleRecord {
        id: "10",
        slug: "over-constrained-param",
        family: "A",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "concrete container demanded where a protocol suffices",
        goal: "Ask for the protocol you use (Smith's fourth): concrete demands \
               close doors LSP-compatible callers would walk through.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_10,
};

/// The span of the annotation on a parameter node. One a `# type:` comment
/// spelled has no node of its own; CPython copies the parameter's location
/// onto it, so the parameter's span is that annotation's.
fn annotation_span(module: &Module<'_>, param: NodeIndex) -> Option<Span> {
    let ann = module.annotation(param)?;
    match Cn::Expr(ann).stamped() {
        Some(at) => module.span(at),
        None => module.span(param),
    }
}

/// The AST footprint's widening candidates (prod-called defs only), verified
/// by one world pass. Silent without the oracle: an unverified widening is a
/// guess, and a degraded run may never report more than the run it degrades
/// from.
fn rule_10(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    if provers.no_oracle() {
        return;
    }
    // the AST footprint proposes, the oracle disposes: widenings applied as
    // replacement splices, vetoed on any new error
    let mut splices: Vec<Splice> = Vec::new();
    let mut meta: IndexMap<String, WidenSite> = IndexMap::new();
    for candidate in widening_candidates(facts, provers) {
        let module = &facts.modules[&candidate.module];
        let span = annotation_span(module, candidate.arg).unwrap_or_default();
        let line = span[0].unwrap_or(0);
        let end_line = match span[2] {
            Some(end) => end,
            None => line,
        };
        // a multi-line annotation is not spliceable, so not verifiable
        if line != end_line {
            continue;
        }
        let pid = format!("widen:{}:{}", candidate.qname, candidate.param);
        splices.push(Splice {
            id: pid.clone(),
            owner: candidate.qname.to_string(),
            spelling: candidate.suggestion.clone(),
            param: candidate.param.clone(),
            edits: vec![SpanEdit {
                line,
                col_start: span[1].unwrap_or(0),
                col_end: span[3].unwrap_or(0),
                text: candidate.suggestion.clone(),
            }],
            imports: Vec::new(),
        });
        meta.insert(pid, candidate);
    }
    for (pid, (evidence, fix)) in provers.verify_splice(facts, &splices) {
        let Some(site) = meta.get(&pid) else {
            continue;
        };
        let detail = match &evidence {
            Evidence::Counterfactual { receipt } => {
                format!("; widening verified (receipt: {receipt})")
            }
            _ => "; widening verified: no new errors under the wider type".to_string(),
        };
        let module = &facts.modules[&site.module];
        out.push(Finding {
            rule: "10",
            site: node_site(facts, module, site.arg),
            message: format!(
                "{} demands concrete `{}` for '{}' but the body only needs `{}`{detail}",
                site.qname, site.annotation, site.param, site.suggestion
            ),
            cause: format!("over-constrained:{}:{}", site.qname, site.param),
            evidence,
            salience: 0.0,
            fix: Some(fix),
            lang: "py",
        });
    }
}

// --- #6 honesty / effect inference -------------------------------------------

static ACCESSOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(get|is|has|read|find|list|count)(_|$)").expect("a literal pattern")
});

/// An action verb after the accessor head names the work (`get_or_compute_x`),
/// so the name hides nothing.
const ACTION_VERBS: [&str; 29] = [
    "run", "check", "validate", "verify", "apply", "process", "handle", "emit", "print", "write",
    "report", "update", "add", "remove", "drop", "merge", "sort", "close", "flush", "fmt",
    "format", "generate", "render", "draw", "plot", "build", "make", "create", "compute",
];

pub const RULE_6: Rule = Rule {
    record: RuleRecord {
        id: "6",
        slug: "dishonest-accessor",
        family: "A",
        engine_class: "WP",
        posture: Posture::Ratchet,
        meaning: "accessor-named functions with proven effects",
        goal: "Honest functions (Smith #2, Van Eerd V10): a getter with effects \
               lies about its contract, and buried effects lie deepest.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_6,
};

/// `gw:<qname>` alone: a write to state the finding can name and a reader can
/// go and check. Out: `io`, `mutates-self`, `gs:` / `slots-arg`, `gm:`, and
/// `mutates-arg` / `mutates-field`, the arm buried in g4/cut.
fn rule_6(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let effects = provers.effects(facts);
    // a double's recording is the fixture
    for (module, sym) in iter_prod_functions(facts) {
        let Some(e) = effects.get(&sym.qname) else {
            continue;
        };
        // the dispatch contract owns the name (as for #40)
        if is_override_fixed(facts, sym) {
            continue;
        }
        let proven: Vec<String> = e
            .atoms
            .iter()
            .filter(|a| a.starts_with("gw:"))
            .cloned()
            .collect();
        let names_work = sym
            .name
            .split('_')
            .skip(1)
            .any(|t| ACTION_VERBS.contains(&t));
        if !proven.is_empty() && ACCESSOR_RE.is_match(&sym.name) && !names_work {
            out.push(Finding {
                rule: "6",
                site: node_site(facts, module, sym.node),
                message: format!(
                    "{} is named like an accessor but has effects: {}",
                    sym.qname,
                    proven.join(", ")
                ),
                cause: format!("dishonest-accessor:{}", sym.qname),
                salience: proven.len() as f64,
                evidence: Evidence::Wp { premises: proven },
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #3 contract-implied guard -----------------------------------------------

/// Guarded-callee contracts: tolerant methods make an emptiness guard
/// redundant; any method not listed needs its guard.
const TOLERATES_EMPTY: [&str; 5] = ["sort", "reverse", "clear", "update", "extend"];

/// `(test, body, has orelse)` of an `If`: CPython nests an `elif` as one `If`,
/// so an `Elif` clause chain answers here too.
fn if_parts<'a>(node: Cn<'a>) -> Option<(Option<&'a Expr>, &'a [Stmt], bool)> {
    match node {
        Cn::Stmt(Stmt::If(n)) => Some((Some(&n.test), &n.body, !n.elif_else_clauses.is_empty())),
        Cn::Elif(rest) => Some((rest[0].test.as_ref(), &rest[0].body, rest.len() > 1)),
        _ => None,
    }
}

/// Python `value == 0`: `0`, `0.0`, `-0.0`, `False` and `0j` all match.
fn is_zero(node: &Expr) -> bool {
    match node {
        Expr::NumberLiteral(n) => match &n.value {
            Number::Int(i) => i.as_u64() == Some(0),
            Number::Float(f) => *f == 0.0,
            Number::Complex { real, imag } => *real == 0.0 && *imag == 0.0,
        },
        Expr::BooleanLiteral(b) => !b.value,
        _ => false,
    }
}

/// `x` / `len(x)` / `len(x) > 0` / `len(x) != 0` -> x.
fn emptiness_guard_target(test: &Expr) -> Option<&str> {
    if let Expr::Name(n) = test {
        return Some(n.id.as_str());
    }
    let mut node = test;
    if let Expr::Compare(c) = node
        && c.ops.len() == 1
        && matches!(c.ops[0], CmpOp::Gt | CmpOp::NotEq)
        && c.comparators.first().is_some_and(is_zero)
    {
        node = &c.left;
    }
    let Expr::Call(call) = node else {
        return None;
    };
    if !is_name(Some(&call.func), "len") || call.arguments.args.len() != 1 {
        return None;
    }
    match &call.arguments.args[0] {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// A display, comprehension, container constructor (`[1] * n` too) or a
/// container-returning call: a value that is never None.
fn builds_container(module: &Module<'_>, value: Option<&Expr>) -> bool {
    if let Some(Expr::BinOp(b)) = value
        && matches!(b.op, Operator::Add | Operator::Mult)
    {
        return builds_container(module, Some(&b.left)) || builds_container(module, Some(&b.right));
    }
    if let Some(Expr::Call(c)) = value {
        let name = match &*c.func {
            Expr::Attribute(a) => Some(a.attr.as_str()),
            Expr::Name(n) => Some(n.id.as_str()),
            _ => None,
        };
        return classes_of(module.dotted_name(&c.func).as_deref(), name).contains(CONTAINER);
    }
    is_mutable_init(value) || matches!(value, Some(Expr::Tuple(_)))
}

/// The `value` slot of the statement a name binding sits in.
fn bound_value<'a>(node: Cn<'a>) -> Option<&'a Expr> {
    match node {
        Cn::Stmt(Stmt::Assign(a)) => Some(&a.value),
        Cn::Stmt(Stmt::AnnAssign(a)) => a.value.as_deref(),
        Cn::Stmt(Stmt::AugAssign(a)) => Some(&a.value),
        Cn::Expr(Expr::Named(n)) => Some(&n.value),
        _ => None,
    }
}

/// The guarded name is vouched never None by its owner: a param annotated
/// without None, or a local whose every binding declares a non-None type or
/// builds a container. An unannotated param or a `.get()` local has no
/// contract for the guard to discharge; a None-inclusive one makes the guard
/// necessary, since iterating None raises.
fn non_none_contract(
    facts: &RepoFacts<'_>,
    provers: &Provers,
    module: &Module<'_>,
    node: NodeIndex,
    name: &str,
) -> bool {
    let Some(owner) = facts.enclosing_symbol(module, node) else {
        return false;
    };
    if !FUNCTION_KINDS.contains(&owner.kind) {
        return false;
    }
    let Some(scope) = provers.scope_of(facts, &owner.qname) else {
        return false;
    };
    let vouched: Vec<bool> = scope
        .writes(facts)
        .iter()
        .filter(|w| w.own && w.kind == "name" && w.root.as_deref() == Some(name))
        .map(|w| {
            let holder = module.parent_of(w.node).map(|at| module.nodes[at as usize]);
            let declared = matches!(holder, Some(Cn::Stmt(Stmt::AnnAssign(a)))
                if !none_inclusive(facts, &module.bindings, &a.annotation));
            declared || builds_container(module, holder.and_then(bound_value))
        })
        .collect();
    if scope.params(facts).iter().any(|p| p == name) {
        let ann = fn_args(fn_of(module, owner))
            .into_iter()
            .find(|a| a.name.as_str() == name)
            .and_then(|a| annotation_of(module, a));
        return ann.is_some_and(|a| !none_inclusive(facts, &module.bindings, a))
            && vouched.iter().all(|v| *v);
    }
    !vouched.is_empty() && vouched.iter().all(|v| *v)
}

pub const RULE_3: Rule = Rule {
    record: RuleRecord {
        id: "3",
        slug: "contract-implied-guard",
        family: "A",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "emptiness guard implied by the guarded call's contract",
        goal: "No redundant state checks (Sean Parent, Better Code, goal 1): \
               a guard the callee's contract already discharges is defensive \
               noise.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_3,
};

/// Emptiness guards discharged by the guarded statement's own contract: a
/// tolerant method call, or a bare for-loop over the guarded name, on a name
/// its owner vouches is never None.
fn rule_3(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    for module in facts.modules.values() {
        for at in module.nodes(&[Kind::If], None, false) {
            let Some((test, body, has_orelse)) = if_parts(module.nodes[at as usize]) else {
                continue;
            };
            if has_orelse || body.len() != 1 {
                continue;
            }
            let Some(name) = test.and_then(emptiness_guard_target) else {
                continue;
            };
            if !non_none_contract(facts, provers, module, at, name) {
                continue;
            }
            let stmt = &body[0];
            // a loop over empty runs zero times; for-else would run on empty,
            // so the guard is necessary there
            if let Stmt::For(loop_stmt) = stmt
                && loop_stmt.orelse.is_empty()
                && is_name(Some(&loop_stmt.iter), name)
            {
                out.push(Finding {
                    rule: "3",
                    site: node_site(facts, module, at),
                    message: format!(
                        "guard on '{name}' is implied by iteration - a loop over \
                         empty runs zero times"
                    ),
                    cause: format!("guard-implied:{name}.iteration"),
                    evidence: Evidence::Ast {
                        detail: "iteration".to_string(),
                    },
                    salience: 0.0,
                    fix: None,
                    lang: "py",
                });
                continue;
            }
            if !is_call_stmt(stmt) {
                continue;
            }
            let Stmt::Expr(expr) = stmt else { continue };
            let Expr::Call(call) = &*expr.value else {
                continue;
            };
            let Expr::Attribute(callee) = &*call.func else {
                continue;
            };
            if is_name(Some(&callee.value), name) && TOLERATES_EMPTY.contains(&callee.attr.as_str())
            {
                out.push(Finding {
                    rule: "3",
                    site: node_site(facts, module, at),
                    message: format!(
                        "guard on '{name}' is implied by {}()'s contract ({} \
                         tolerates empty)",
                        callee.attr, callee.attr
                    ),
                    cause: format!("guard-implied:{name}.{}", callee.attr),
                    evidence: Evidence::Ast {
                        detail: callee.attr.to_string(),
                    },
                    salience: 0.0,
                    fix: None,
                    lang: "py",
                });
            }
        }
    }
}

// --- #7 comment-borne protocol -----------------------------------------------

/// Forces `PROTOCOL_RE`'s compile, which its bounded repeats put near 2.5 ms:
/// `PyLanguage::build` runs this on a rayon worker so the compile hides
/// behind the facts build instead of landing inside #7's first match.
pub fn warm() {
    LazyLock::force(&PROTOCOL_RE);
}

/// Every `PROTOCOL_RE` arm spells one of these literals, so this is a pure
/// prefilter: a literal alternation the regex engine runs as a SIMD substring
/// scan, gating the bounded-repeat pattern below, which runs two orders of
/// magnitude slower per byte.
static PROTOCOL_HINT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)call|expect|assume").expect("a literal pattern"));

// ASCII word boundaries throughout: a Unicode `\b` beside a non-ASCII byte
// drops the whole search off the DFA onto the backtracking engine, and prose
// with an arrow or a `≤` in it pays 100x per byte. The keywords are ASCII.
static PROTOCOL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(caller (must|should|is responsible)",
        r"|must call(?-u:\b)",
        r"|(must|should) be called (before|after|first)",
        // deontic only: `we call refresh() after each batch` narrates
        r"|(?-u:\b)(should|always|needs? to|ha(s|ve) to|remember to) call .{1,40}(?-u:\b)(before|after)(?-u:\b)",
        // the bare imperative at a comment's or line's start, the callee
        // spelled with parens
        r"|(^|\n)[#\s]*call \w+(\.\w+)*\([^)\n]*\)[^\n]{0,40}(?-u:\b)(before|after)(?-u:\b)",
        r"|do not call .{0,40}(unless|before|until)",
        r"|expects? .{1,60}(to be|already)",
        r"|assumes? .{1,60}(initiali[sz]ed|already called|open|locked|loaded|sorted|validated))",
    ))
    .expect("a literal pattern")
});

pub const RULE_7: Rule = Rule {
    record: RuleRecord {
        id: "7",
        slug: "comment-borne-protocol",
        family: "A",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "'caller must ...' protocol narrated in a def's docstring",
        goal: "Contracts enforced, not narrated (Smith #5): a protocol carried \
               in prose should be a receipt type or a lifted precondition.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_7,
};

/// The docstring of a def that could carry the obligation and does not. A
/// module and a class have no signature to lift a precondition into, an
/// `__init__`'s obligations are its parameter list, and a body that opens with
/// an `assert` already checks what the prose repeats.
fn rule_7(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for module in facts.modules.values() {
        // a test narrating a protocol exercises it, not publishes it
        if is_test_path(&module.rel) {
            continue;
        }
        for at in module.nodes(&[Kind::FunctionDef, Kind::AsyncFunctionDef], None, false) {
            let Cn::Stmt(Stmt::FunctionDef(fn_def)) = module.nodes[at as usize] else {
                continue;
            };
            let doc = raw_docstring(&fn_def.body).unwrap_or("");
            let qname = facts.enclosing(module, at);
            if doc.is_empty()
                || fn_def.name.as_str() == "__init__"
                || !PROTOCOL_HINT_RE.is_match(doc)
                || !PROTOCOL_RE.is_match(doc)
                || !module
                    .nodes(&[Kind::Assert], Some(&qname), false)
                    .is_empty()
            {
                continue;
            }
            out.push(Finding {
                rule: "7",
                site: node_site(facts, module, at),
                message: format!(
                    "protocol narrated in docstring of {qname} - encode it \
                     (receipt type or lifted precondition)"
                ),
                cause: format!("protocol-doc:{}", module.line_of(at)),
                evidence: Evidence::ast(),
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #9 shared mutable module state ------------------------------------------

const ENVIRON: &str = "os.environ";
const MIN_LOCAL_WRITERS: usize = 3;

fn is_main_test(test: &Expr) -> bool {
    let Expr::Compare(c) = test else {
        return false;
    };
    std::iter::once(&*c.left)
        .chain(c.comparators.iter())
        .any(|x| is_name(Some(x), "__name__"))
}

/// `(node, mutator)` in document order for module-level statements outside a
/// main guard that call a catalogued process mutator or store into
/// `os.environ`.
fn import_time_mutators(module: &Module<'_>) -> Vec<(NodeIndex, String)> {
    let mut hits: Vec<(NodeIndex, String)> = Vec::new();
    for at in module.nodes(&[Kind::Call], Some(&module.qname), false) {
        let Cn::Expr(Expr::Call(call)) = module.nodes[at as usize] else {
            continue;
        };
        if let Some(name) = module.dotted_name(&call.func)
            && IMPORT_TIME_MUTATORS.contains(name.as_str())
        {
            hits.push((at, name));
        }
    }
    for at in module.nodes(&[Kind::Assign], Some(&module.qname), false) {
        let Cn::Stmt(Stmt::Assign(assign)) = module.nodes[at as usize] else {
            continue;
        };
        let stores = assign.targets.iter().any(|t| {
            matches!(t, Expr::Subscript(s)
                if module.dotted_name(&s.value).as_deref() == Some(ENVIRON))
        });
        if stores {
            hits.push((at, format!("{ENVIRON}[...] =")));
        }
    }
    hits.retain(|(at, _)| !under_main_guard(module, *at));
    hits.sort_by_key(|(at, _)| {
        let span = module.span(*at).unwrap_or_default();
        (span[0].unwrap_or(0), span[1].unwrap_or(0))
    });
    hits
}

pub const RULE_9: Rule = Rule {
    record: RuleRecord {
        id: "9",
        slug: "shared-mutable-state",
        family: "A",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "a module global rebound by three of its own functions; \
                  monkeypatching; import-time process mutation in imported modules",
        goal: "No shared mutable state (Sean Parent, Better Code, goal 3): a \
               module global mutated from many places is action at a \
               distance.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_9,
};

/// Three arms over the ref index: a module global rebound from three of its
/// own module's functions, a store onto a repo symbol from outside its module
/// (monkeypatching), and import-time process mutation in an imported module.
/// Rebinding only: `CACHE[k] = v` inside its own module is the memo pattern. A
/// patched name is one seam per scope, and the restore that closes it is not a
/// second one.
fn rule_9(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    for sym in facts.symbols.values() {
        if sym.kind != "variable" || sym.parent.is_some() {
            continue;
        }
        let module = &facts.modules[&sym.module];
        // fixture resets are idiomatic (symmetric with the monkeypatch arm)
        if is_test_path(&module.rel) {
            continue;
        }
        // the home module's own rebinding functions
        let writers: IndexSet<Qname> = facts
            .refs_to
            .get(&sym.qname)
            .map_or(&[][..], |v| v)
            .iter()
            .filter_map(|at| facts.refs.get(*at as usize))
            .filter(|r| r.module == sym.module && r.kind == RefKind::Store)
            .map(|r| facts.enclosing(module, r.node))
            .collect();
        let mut local: Vec<&Qname> = writers
            .iter()
            .filter(|q| {
                facts
                    .symbols
                    .get(&***q)
                    .is_some_and(|w| FUNCTION_KINDS.contains(&w.kind))
            })
            .collect();
        local.sort();
        if local.len() >= MIN_LOCAL_WRITERS {
            let listed: Vec<&str> = local.iter().map(|q| &***q).collect();
            out.push(Finding {
                rule: "9",
                site: node_site(facts, module, sym.node),
                message: format!(
                    "module-level {} rebound from {} functions of its own module: {}",
                    sym.qname,
                    local.len(),
                    listed.join(", ")
                ),
                cause: format!("local-writers:{}", sym.qname),
                evidence: Evidence::idx(),
                salience: local.len() as f64,
                fix: None,
                lang: "py",
            });
        }
    }
    // one seam per (scope, patched name), reported where it is opened: the
    // `finally: mod.f = original` that closes it is the same seam's other half
    let mut seams: IndexMap<(Qname, Qname, Qname), NodeIndex> = IndexMap::new();
    for r in &facts.refs {
        let Some(target) = facts.symbols.get(&*r.target) else {
            continue;
        };
        if r.kind != RefKind::Store
            || !matches!(target.kind, "function" | "method" | "class")
            || r.module == target.module
            || facts.rel_of(&r.module).is_some_and(|rel| is_test_path(rel))
        {
            continue;
        }
        let module = &facts.modules[&r.module];
        let key = (
            r.module.clone(),
            facts.enclosing(module, r.node),
            r.target.clone(),
        );
        let line = module.line_of(r.node);
        match seams.get(&key) {
            Some(held) if module.line_of(*held) <= line => {}
            _ => {
                seams.insert(key, r.node);
            }
        }
    }
    let mut ordered: Vec<((Qname, Qname, Qname), NodeIndex)> = seams.into_iter().collect();
    ordered.sort_by(|a, b| {
        let line = |q: &Qname, at: NodeIndex| facts.modules[q].line_of(at);
        (&a.0.0, &a.0.2, line(&a.0.0, a.1)).cmp(&(&b.0.0, &b.0.2, line(&b.0.0, b.1)))
    });
    for ((module_q, _scope, target_q), node) in ordered {
        let module = &facts.modules[&module_q];
        out.push(Finding {
            rule: "9",
            site: node_site(facts, module, node),
            message: format!("runtime monkeypatch: {module_q} rebinds {target_q}"),
            cause: format!("monkeypatch:{target_q}"),
            evidence: Evidence::idx(),
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
    let imported_by = importers(provers.import_graph(facts));
    for module in facts.modules.values() {
        let n = imported_by.get(&module.qname).map_or(0, IndexSet::len);
        // a script other scripts import is still a script
        let guarded = module
            .nodes(&[Kind::If], Some(&module.qname), false)
            .into_iter()
            .any(|at| {
                if_parts(module.nodes[at as usize])
                    .and_then(|(test, _, _)| test)
                    .is_some_and(is_main_test)
            });
        if n == 0 || is_test_path(&module.rel) || guarded {
            continue;
        }
        for (node, name) in import_time_mutators(module) {
            let plural = if n > 1 { "s" } else { "" };
            out.push(Finding {
                rule: "9",
                site: node_site(facts, module, node),
                message: format!(
                    "import-time effect: {name} runs whenever {} is imported \
                     ({n} importer{plural})",
                    module.qname
                ),
                cause: format!(
                    "import-time-effect:{}:{}",
                    module.qname,
                    module.line_of(node)
                ),
                evidence: Evidence::idx(),
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #49 mutable default -----------------------------------------------------
// #9's pure-AST arm, its own rule so the fast gate can run it. #9's test
// policy is about actors: a test may mutate prod state as its isolation
// vocabulary. A mutable default mutates nothing at a distance, so it is a
// defect of the def that owns it, judged wherever that def lives.

pub const RULE_49: Rule = Rule {
    record: RuleRecord {
        id: "49",
        slug: "mutable-default",
        family: "A",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "a mutable literal as a parameter default",
        goal: "No shared mutable state (Sean Parent, Better Code, goal 3): a \
               default built once at def time is state every call silently \
               shares.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_49,
};

fn rule_49(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_functions(facts) {
        let fn_def = fn_of(module, sym);
        for (arg, default) in fn_defaults(fn_def) {
            if !is_mutable_init(Some(default)) {
                continue;
            }
            let at = Cn::Expr(default).stamped().unwrap_or(sym.node);
            out.push(Finding {
                rule: "49",
                site: node_site(facts, module, at),
                message: format!("mutable default for '{}' in {}", arg.name, sym.qname),
                cause: format!("mutable-default:{}:{}", sym.qname, arg.name),
                evidence: Evidence::ast(),
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #40 naming proxies ------------------------------------------------------
// The cheap checkable slice of lying names. The inferred arm follows the #2
// grounding rule: its premises are never repo-written annotations, so it can
// never reach the proved tier.

static PROXY_PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(is|has|can)_").expect("a literal pattern"));
static OPAQUE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(Unknown|Any)\b").expect("a literal pattern"));

const BOOLISH: [&str; 3] = ["bool", "TypeGuard", "TypeIs"];

fn declared_verdict(module: &Module<'_>, func: NodeIndex) -> Option<String> {
    let ann = module.returns(func)?;
    let names = annotation_names(ann);
    (!names.is_empty() && !names.iter().any(|n| BOOLISH.contains(&n.as_str()))).then(|| {
        format!(
            "is named like a predicate but declares `-> {}`",
            unparse::expr(ann)
        )
    })
}

fn inferred_verdict(ret: &str) -> Option<String> {
    // no claim on what the oracle cannot see
    if OPAQUE_RE.is_match(ret) {
        return None;
    }
    let members: BTreeSet<String> = split_union(ret)
        .into_iter()
        .flat_map(deliteral)
        .map(|m| generic_base(&m).to_string())
        .collect();
    if members.is_empty() || (members.len() == 1 && members.contains("None")) {
        return None;
    }
    (!members.iter().any(|m| BOOLISH.contains(&m.as_str())))
        .then(|| format!("is named like a predicate but the oracle infers `-> {ret}`"))
}

pub const RULE_40: Rule = Rule {
    record: RuleRecord {
        id: "40",
        slug: "naming-proxies",
        family: "A",
        engine_class: "AST+ORACLE",
        posture: Posture::Ratchet,
        meaning: "is_*/has_*/can_* returning non-bool",
        goal: "Names are contracts too: a predicate name returning a payload \
               makes every call site read wrong.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_40,
};

/// Predicate-named functions returning non-bool: the annotation arm always,
/// the oracle arm on inferred types.
fn rule_40(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let ret_types = provers.ret_types(facts);
    for (module, sym) in iter_prod_functions(facts) {
        // dunders; the dispatch contract owns the name
        if sym.name.starts_with("__") || is_override_fixed(facts, sym) {
            continue;
        }
        if !PROXY_PREFIX_RE.is_match(&sym.name) {
            continue;
        }
        // the declared annotation first, the oracle-inferred return second
        let (detail, evidence) = match declared_verdict(module, sym.node) {
            Some(detail) => (
                detail,
                Evidence::Ast {
                    detail: "declared".to_string(),
                },
            ),
            None => {
                if module.returns(sym.node).is_some() {
                    continue;
                }
                let Some(ret) = ret_types.return_type(&sym.qname).filter(|r| !r.is_empty()) else {
                    continue;
                };
                let Some(detail) = inferred_verdict(ret) else {
                    continue;
                };
                (
                    detail,
                    Evidence::Oracle {
                        rule: "naming-proxy".to_string(),
                        grounded: false,
                        message: ret.to_string(),
                    },
                )
            }
        };
        out.push(Finding {
            rule: "40",
            site: node_site(facts, module, sym.node),
            message: format!("{} {detail}", sym.qname),
            cause: format!("naming-proxy:{}", sym.qname),
            evidence,
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

// --- #50 unannotated boundary ------------------------------------------------
// #1 judges the annotations a boundary wrote; #50 the ones it did not. No
// framework guard: an override's missing annotation is its own.

pub const RULE_50: Rule = Rule {
    record: RuleRecord {
        id: "50",
        slug: "unannotated-boundary",
        family: "A",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "public prod def with an unannotated param or value-returning return",
        goal: "The signature is the published contract: a slot it leaves blank \
               every caller re-derives from the body, and no checker can hold.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_50,
};

/// Inside the repo's declared type-check scope only: a `samples/` tree the
/// repo's own mypy/pyright never reads publishes no signature.
fn rule_50(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_prod_functions(facts) {
        if !(is_exported(facts, module, sym) && in_typed_scope(facts, &module.rel)) {
            continue;
        }
        let fn_def = fn_of(module, sym);
        // receiver and star params out (#1's); `return` when a value comes
        // back under no annotation
        let args = fn_args(fn_def);
        let mut slots: Vec<String> = without_receiver(&args)
            .iter()
            .filter(|a| annotation_of(module, a).is_none())
            .map(|a| a.name.to_string())
            .collect::<Vec<_>>();
        let returns_a_value = module
            .nodes(&[Kind::Return], Some(&sym.qname), false)
            .into_iter()
            .any(|at| matches!(module.nodes[at as usize], Cn::Stmt(Stmt::Return(r)) if r.value.is_some()));
        if module.returns(sym.node).is_none() && returns_a_value {
            slots.push("return".to_string());
        }
        if slots.is_empty() {
            continue;
        }
        let detail = slots.join(", ");
        out.push(Finding {
            rule: "50",
            site: node_site(facts, module, sym.node),
            message: format!("public boundary {} leaves unannotated: {detail}", sym.qname),
            cause: format!("unannotated:{}", sym.qname),
            evidence: Evidence::Ast {
                detail: detail.clone(),
            },
            salience: slots.len() as f64,
            fix: None,
            lang: "py",
        });
    }
}
