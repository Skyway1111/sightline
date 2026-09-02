//! Is an oracle verdict grounded in a claim the repo wrote? Every predicate
//! takes an `OracleDiag` or an `ArgTypes` (`operand_grounded`,
//! `none_default_lie`,
//! `container_shape_check`, `broken_declaration`, `grounding`); #2 and #58
//! read them.

use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use ruff_python_ast::{CmpOp, Expr, ExprCompare, Stmt, StmtClassDef, StmtFunctionDef};

use sightline_core::findings::{Qname, Rel};
use sightline_py_facts::astutil::{RECEIVERS, fn_args, fn_defaults, without_receiver};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{FUNCTION_KINDS, NodeIndex, RepoFacts, Symbol, is_test_path};
use sightline_py_facts::module::Module;
use sightline_py_facts::qnames::resolve_dotted_expr;

use crate::Provers;
use crate::argtypes::ArgTypes;
use crate::oracle::OracleDiag;
use crate::scope::class_fields;
use crate::typestrings::{deliteral, split_union};

mod nodes;

use nodes::*;

/// The shape-check classes: what Python code names to establish that parsed
/// data has the form its annotation claims.
const CONTAINER_CLASSES: [&str; 5] = ["dict", "list", "set", "tuple", "frozenset"];

/// Stdlib families subtyped via `ABCMeta.register()`: pyright's nominal
/// isinstance no-overlap claim is unsound against them (probe:
/// `isinstance(True, Integral)` is True).
const VIRTUAL_ABCS: [&str; 31] = [
    "Number",
    "Complex",
    "Real",
    "Rational",
    "Integral",
    "Container",
    "Hashable",
    "Iterable",
    "Iterator",
    "Reversible",
    "Generator",
    "Sized",
    "Callable",
    "Collection",
    "Sequence",
    "MutableSequence",
    "ByteString",
    "Set",
    "MutableSet",
    "Mapping",
    "MutableMapping",
    "MappingView",
    "KeysView",
    "ItemsView",
    "ValuesView",
    "Awaitable",
    "Coroutine",
    "AsyncIterable",
    "AsyncIterator",
    "AsyncGenerator",
    "Buffer",
];

static QUOTED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([^"]+)""#).expect("a quoted-name pattern"));

/// Does `a.b.c` resolve root to leaf through fields this repo annotated?
/// Root: self/cls (the enclosing class) or an unrebound annotated param
/// naming an internal class. Every intermediate hop must be an annotated
/// field of an internal class; the leaf needs any local annotation. Anything
/// else (an external stub, an inferred assignment, an unresolvable hop) is
/// not locally declared.
fn locally_annotated_chain(
    operand: &Expr,
    owner: &Symbol,
    module: &Module<'_>,
    facts: &RepoFacts<'_>,
    provers: &Provers,
) -> bool {
    let mut parts: Vec<&str> = Vec::new();
    let mut cur = operand;
    while let Expr::Attribute(a) = cur {
        parts.push(a.attr.as_str());
        cur = &a.value;
    }
    let Expr::Name(root) = cur else {
        return false;
    };
    parts.reverse();
    let rebound = provers.scope_of(facts, &owner.qname).is_some_and(|s| {
        s.rebound_before(facts, line_of(module, operand), false)
            .contains(root.id.as_str())
    });
    let mut cls_q: Option<Qname> = if RECEIVERS.contains(&root.id.as_str()) {
        owner
            .parent
            .as_ref()
            .filter(|p| facts.classes.contains_key(*p))
            .cloned()
    } else if rebound {
        None
    } else {
        let ann = func_def(module, owner).and_then(|f| {
            fn_args(f)
                .into_iter()
                .find(|a| a.name.as_str() == root.id.as_str())
                .and_then(|a| Cn::Param(a).stamped())
                .and_then(|at| module.annotation(at))
        });
        annotation_class(ann, module, facts)
    };
    let Some((leaf, hops)) = parts.split_last() else {
        return false;
    };
    for attr in hops {
        let field = fields_of(facts, cls_q.as_deref())
            .get(*attr)
            .copied()
            .flatten();
        cls_q = annotation_class(field, module, facts);
    }
    fields_of(facts, cls_q.as_deref())
        .get(*leaf)
        .copied()
        .flatten()
        .is_some()
}

/// Is the operand's type a claim this repo wrote? A declared name the scope
/// has not rebound, or a locally-annotated attribute chain read before the
/// body's first `await` (another task runs at a suspension point, so a field
/// narrowed across one is the checker's claim). Plain locals, inferred
/// fields, external descriptors and rebound params are the checker's
/// inference: the one predicate every #2 verdict rests on.
pub fn operand_grounded(
    operand: &Expr,
    owner: &Symbol,
    module: &Module<'_>,
    facts: &RepoFacts<'_>,
    provers: &Provers,
) -> bool {
    match operand {
        Expr::Name(name) => provers.scope_of(facts, &owner.qname).is_some_and(|scope| {
            let line = line_of(module, operand);
            scope.declared(facts).contains(name.id.as_str())
                && !scope
                    .rebound_before(facts, line, false)
                    .contains(name.id.as_str())
        }),
        Expr::Attribute(_) => {
            let line = line_of(module, operand);
            locally_annotated_chain(operand, owner, module, facts, provers)
                && !module
                    .nodes(&[Kind::Await], Some(&owner.qname), false)
                    .into_iter()
                    .any(|at| module.line_of(at) < line)
        }
        _ => false,
    }
}

/// A discovered caller passes `None` to a param annotated non-Optional: the
/// annotation lies and only the caller proves it.
fn caller_established_none(owner: &Symbol, name: &str, arg_types: &ArgTypes) -> bool {
    arg_types
        .for_param(&owner.qname, name)
        .unwrap_or_default()
        .iter()
        .any(|r| {
            r.ty.as_ref().is_some_and(|t| {
                split_union(t)
                    .into_iter()
                    .flat_map(deliteral)
                    .any(|m| m == "None")
            })
        })
}

/// Does the def supply its own value for a tested name on the `None` path
/// (`if x is None: x = D`, `x = x if x is not None else D`)? A branch that
/// raises or returns instead is the redundancy #2 is after.
fn fallback_for(
    cmp_at: NodeIndex,
    cmp: &ExprCompare,
    names: &BTreeSet<String>,
    module: &Module<'_>,
    owner: &Symbol,
    facts: &RepoFacts<'_>,
    provers: &Provers,
) -> bool {
    let parent = module.parent_of(cmp_at);
    if let Some(Cn::Expr(Expr::If(_))) = node_of(module, parent) {
        let holder = parent.and_then(|p| module.parent_of(p));
        return match node_of(module, holder) {
            Some(Cn::Stmt(Stmt::Assign(a))) => a.targets.iter().any(|t| match t {
                Expr::Name(n) => names.contains(n.id.as_str()),
                _ => false,
            }),
            _ => false,
        };
    }
    let orelse = matches!(cmp.ops.first(), Some(CmpOp::IsNot));
    let branch = parent
        .and_then(|p| if_branch(module, p, orelse))
        .unwrap_or_default();
    // Python's empty branch is the range (0, -1): no write sits in it
    let (Some(first), Some(last)) = (branch.first(), branch.last()) else {
        return false;
    };
    let (lo, hi) = (module.line_of(*first), module.end_line_of(*last));
    provers.scope_of(facts, &owner.qname).is_some_and(|scope| {
        scope.writes(facts).iter().any(|w| {
            w.kind == "name"
                && w.root.as_ref().is_some_and(|r| names.contains(r))
                && (lo..=hi).contains(&module.line_of(w.node))
        })
    })
}

/// The def contradicts its own declaration for the compared name: a literal
/// `None` default (param, or class field, `dataclasses.field` included), or a
/// `None` fallback the body supplies (`fallback_for`). The annotation is the
/// defect and #1 owns the default's half, so #2 skips these entirely.
pub fn none_default_lie(diag: &OracleDiag, facts: &RepoFacts<'_>, provers: &Provers) -> bool {
    if diag.rule != "reportUnnecessaryComparison" {
        return false;
    }
    let (Some(module), Some(owner)) = module_and_owner(diag, facts) else {
        return false;
    };
    let Some(cmp_at) = node_at(module, diag, Kind::Compare) else {
        return false;
    };
    let Cn::Expr(Expr::Compare(cmp)) = module.nodes[cmp_at as usize] else {
        return false;
    };
    let operands: Vec<&Expr> = std::iter::once(&*cmp.left)
        .chain(cmp.comparators.iter())
        .collect();
    let names: BTreeSet<String> = operands.iter().copied().filter_map(name_id).collect();
    let none_defaulted_param = func_def(module, owner).is_some_and(|fn_def| {
        fn_defaults(fn_def)
            .into_iter()
            .any(|(a, d)| defaults_none(Some(d)) && names.contains(a.name.as_str()))
    });
    if none_defaulted_param {
        return true;
    }
    let self_attrs: BTreeSet<String> = operands
        .iter()
        .filter_map(|o| match o {
            Expr::Attribute(a) if matches!(&*a.value, Expr::Name(n) if n.id.as_str() == "self") => {
                Some(a.attr.to_string())
            }
            _ => None,
        })
        .collect();
    let lying_field = owning_class(facts, owner)
        .is_some_and(|cls| !none_defaulted_fields(cls).is_disjoint(&self_attrs));
    if lying_field {
        return true;
    }
    !names.is_empty()
        && operands.iter().any(|o| is_none(Some(o)))
        && cmp
            .ops
            .iter()
            .all(|op| matches!(op, CmpOp::Is | CmpOp::IsNot))
        && fallback_for(cmp_at, cmp, &names, module, owner, facts, provers)
}

/// An `isinstance` against bare container classes. The annotation such a
/// check duplicates is a claim *about* the data (a record off disk, a payload
/// off the wire), not a proof of it: the check is what makes the claim true,
/// and the repo wrote it for exactly the run where the claim fails.
pub fn container_shape_check(diag: &OracleDiag, facts: &RepoFacts<'_>) -> bool {
    if diag.rule != "reportUnnecessaryIsInstance" {
        return false;
    }
    let (Some(module), _) = module_and_owner(diag, facts) else {
        return false;
    };
    let Some(call) = node_at(module, diag, Kind::Call).and_then(|at| module.call_at(at)) else {
        return false;
    };
    if call.arguments.args.len() < 2 {
        return false;
    }
    let named: Vec<&Expr> = match &call.arguments.args[1] {
        Expr::Tuple(t) => t.elts.iter().collect(),
        other => vec![other],
    };
    named.iter().all(|c| match c {
        Expr::Name(n) => CONTAINER_CLASSES.contains(&n.id.as_str()),
        _ => false,
    })
}

/// The tested name is rebound on a path to the check (`rebound_before`, the
/// demotion predicate) and one of its rebindings is an assignment the checker
/// rejected (`rejected`: the oracle's invalid-assignment sites). The verdict
/// is then the declaration's and the value at the check the assignment's, so
/// #2 skips it at every tier: the annotation the body outgrew is its own
/// defect. A valid rebinding (`x = x or 0`) keeps its verdict.
pub fn broken_declaration(
    diag: &OracleDiag,
    facts: &RepoFacts<'_>,
    provers: &Provers,
    rejected: &HashSet<(Rel, u32)>,
) -> bool {
    let (Some(module), Some(owner)) = module_and_owner(diag, facts) else {
        return false;
    };
    let names: BTreeSet<String> = tested(module, diag)
        .into_iter()
        .filter_map(name_id)
        .collect();
    let Some(scope) = provers.scope_of(facts, &owner.qname) else {
        return false;
    };
    let rebound = scope.rebound_before(facts, diag.line, false);
    scope.rebindings(facts).into_iter().any(|w| {
        w.root
            .as_ref()
            .is_some_and(|r| rebound.contains(r) && names.contains(r))
            && rejected.contains(&(diag.rel.clone(), module.line_of(w.node)))
    })
}

/// Annotation-grounded proxy: every param of the enclosing signature is
/// annotated and none of those annotations is contradicted by its own default
/// (`device: torch.device = "cpu"`, an `invalid-parameter-default` in
/// `rejected`): a signature the def itself breaks declares nothing. Module
/// level is ungrounded.
///
/// Carve-outs kept heuristic (each fixture-pinned): `==`/`!=` (no-overlap
/// ignores `__eq__`); isinstance against ABC-registered stdlib types, in a
/// test path (the test's subject) or on `Never` (the checker's own emptied
/// narrowing); identity-None and isinstance operands only where the repo
/// wrote the type (`operand_grounded`); caller-passes-None evidence.
/// None-defaulted params never reach here: rule #2 skips them via
/// `none_default_lie`, and #1 owns that defect.
pub fn grounding(
    diag: &OracleDiag,
    facts: &RepoFacts<'_>,
    provers: &Provers,
    arg_types: &ArgTypes,
    rejected: &HashSet<(Rel, u32)>,
) -> bool {
    let (Some(module), Some(owner)) = module_and_owner(diag, facts) else {
        return false;
    };
    if diag.rule == "reportUnnecessaryIsInstance" {
        if is_test_path(&diag.rel)
            || diag.message.contains("\"Never\"")
            || QUOTED_RE
                .captures_iter(&diag.message)
                .any(|c| VIRTUAL_ABCS.contains(&&c[1]))
        {
            return false;
        }
        let subject = node_at(module, diag, Kind::Call)
            .and_then(|at| module.call_at(at))
            .and_then(|c| c.arguments.args.first());
        if !subject.is_some_and(|a| operand_grounded(a, owner, module, facts, provers)) {
            return false;
        }
    } else if diag.rule == "reportUnnecessaryComparison"
        && let Some(Cn::Expr(Expr::Compare(cmp))) =
            node_of(module, node_at(module, diag, Kind::Compare))
    {
        if cmp
            .ops
            .iter()
            .any(|op| matches!(op, CmpOp::Eq | CmpOp::NotEq))
        {
            return false;
        }
        let operands: Vec<&Expr> = std::iter::once(&*cmp.left)
            .chain(cmp.comparators.iter())
            .collect();
        let identity_none = cmp
            .ops
            .iter()
            .all(|op| matches!(op, CmpOp::Is | CmpOp::IsNot))
            && operands.iter().any(|o| is_none(Some(o)));
        if identity_none {
            if !operands
                .iter()
                .all(|o| is_constant(o) || operand_grounded(o, owner, module, facts, provers))
            {
                return false;
            }
            let established = operands.iter().any(|o| match o {
                Expr::Name(n) => caller_established_none(owner, n.id.as_str(), arg_types),
                _ => false,
            });
            if established {
                return false;
            }
        }
    }
    let Some(fn_def) = func_def(module, owner) else {
        return false;
    };
    let args = fn_args(fn_def);
    let all_declared = without_receiver(&args)
        .iter()
        .copied()
        .chain(fn_def.parameters.vararg.as_deref())
        .chain(fn_def.parameters.kwarg.as_deref())
        .all(|a| {
            Cn::Param(a)
                .stamped()
                .is_some_and(|at| module.annotation(at).is_some())
        });
    all_declared
        && !fn_defaults(fn_def)
            .into_iter()
            .any(|(_, d)| rejected.contains(&(diag.rel.clone(), line_of(module, d))))
}
