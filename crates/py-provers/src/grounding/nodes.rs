//! The reads grounding's predicates make of the tree: the node a diagnostic
//! points at, the symbols and defs around it, and the literals and names a
//! declaration spells.

use super::*;

pub(super) fn is_none(node: Option<&Expr>) -> bool {
    matches!(node, Some(Expr::NoneLiteral(_)))
}

pub(super) fn is_constant(node: &Expr) -> bool {
    Cn::Expr(node).kind() == Kind::Constant
}

/// A literal `None` default, plain or through `dataclasses.field`.
pub(super) fn defaults_none(value: Option<&Expr>) -> bool {
    match value {
        Some(Expr::Call(call)) => call.arguments.keywords.iter().any(|kw| {
            kw.arg.as_ref().is_some_and(|a| a.as_str() == "default") && is_none(Some(&kw.value))
        }),
        other => is_none(other),
    }
}

/// Class-level fields with a literal `None` default (dataclass or plain):
/// `self.field is None` guards on them rest on the same lie a `None`-defaulted
/// param does.
pub(super) fn none_defaulted_fields(cls: &StmtClassDef) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for st in &cls.body {
        let (value, targets): (Option<&Expr>, Vec<&Expr>) = match st {
            Stmt::AnnAssign(a) => (a.value.as_deref(), vec![&a.target]),
            Stmt::Assign(a) => (Some(&a.value), a.targets.iter().collect()),
            _ => continue,
        };
        if !defaults_none(value) {
            continue;
        }
        out.extend(targets.into_iter().filter_map(name_id));
    }
    out
}

/// Internal class qname an annotation expr names, else `None`.
pub(super) fn annotation_class(
    ann: Option<&Expr>,
    module: &Module<'_>,
    facts: &RepoFacts<'_>,
) -> Option<Qname> {
    match ann {
        Some(e @ (Expr::Name(_) | Expr::Attribute(_))) => {
            let q = resolve_dotted_expr(e, module, facts)?;
            facts.classes.contains_key(&q).then_some(q)
        }
        _ => None,
    }
}

/// The name an expression spells, when it spells one.
pub(super) fn name_id(node: &Expr) -> Option<String> {
    match node {
        Expr::Name(n) => Some(n.id.to_string()),
        _ => None,
    }
}

/// The node an index points at, where the caller has one.
pub(super) fn node_of<'t>(module: &Module<'t>, at: Option<NodeIndex>) -> Option<Cn<'t>> {
    at.map(|at| module.nodes[at as usize])
}

/// The diag's module and innermost enclosing function symbol (or `None`).
pub(super) fn module_and_owner<'a, 't>(
    diag: &OracleDiag,
    facts: &'a RepoFacts<'t>,
) -> (Option<&'a Module<'t>>, Option<&'a Symbol>) {
    let Some(module) = facts.module_by_rel(&diag.rel) else {
        return (None, None);
    };
    let mut owner: Option<&Symbol> = None;
    for at in facts
        .symbols_by_module
        .get(&module.qname)
        .map_or(&[][..], |v| v)
    {
        let Some((_, sym)) = facts.symbols.get_index(*at as usize) else {
            continue;
        };
        let end = if sym.end_lineno == 0 {
            sym.lineno
        } else {
            sym.end_lineno
        };
        if !FUNCTION_KINDS.contains(&sym.kind) || sym.lineno > diag.line || diag.line > end {
            continue;
        }
        // Python's `max` keeps the first of equal keys (R4)
        if owner.is_none_or(|best| sym.lineno > best.lineno) {
            owner = Some(sym);
        }
    }
    (Some(module), owner)
}

pub(super) fn node_at(module: &Module<'_>, diag: &OracleDiag, kind: Kind) -> Option<NodeIndex> {
    module.nodes(&[kind], None, false).into_iter().find(|at| {
        module
            .span(*at)
            .is_some_and(|s| s[0] == Some(diag.line) && s[1] == Some(diag.col))
    })
}

/// The `If` statement's body, or its CPython `orelse`, as node indices.
pub(super) fn if_branch(
    module: &Module<'_>,
    at: NodeIndex,
    orelse: bool,
) -> Option<Vec<NodeIndex>> {
    let (body, rest) = match module.nodes[at as usize] {
        Cn::Stmt(Stmt::If(n)) => (
            n.body.iter().collect::<Vec<_>>(),
            n.elif_else_clauses.as_slice(),
        ),
        Cn::Elif(clauses) => (clauses[0].body.iter().collect(), &clauses[1..]),
        _ => return None,
    };
    if !orelse {
        return Some(
            body.into_iter()
                .filter_map(|s| Cn::Stmt(s).stamped())
                .collect(),
        );
    }
    Some(match rest.first() {
        None => Vec::new(),
        // an `elif` is one CPython `If` spanning the rest of the chain
        Some(next) if next.test.is_some() => Cn::Elif(rest).stamped().into_iter().collect(),
        Some(next) => next
            .body
            .iter()
            .filter_map(|s| Cn::Stmt(s).stamped())
            .collect(),
    })
}

/// The operands a #2 diagnostic judges: an isinstance call's subject, a
/// comparison's sides.
pub(super) fn tested<'t>(module: &Module<'t>, diag: &OracleDiag) -> Vec<&'t Expr> {
    if diag.rule == "reportUnnecessaryIsInstance" {
        return node_at(module, diag, Kind::Call)
            .and_then(|at| module.call_at(at))
            .map_or(Vec::new(), |c| c.arguments.args.iter().take(1).collect());
    }
    match node_of(module, node_at(module, diag, Kind::Compare)) {
        Some(Cn::Expr(Expr::Compare(cmp))) => std::iter::once(&*cmp.left)
            .chain(cmp.comparators.iter())
            .collect(),
        _ => Vec::new(),
    }
}

/// `class_fields` over an optional class: no class declares nothing.
pub(super) fn fields_of<'t>(
    facts: &RepoFacts<'t>,
    cls_q: Option<&str>,
) -> IndexMap<String, Option<&'t Expr>> {
    class_fields(facts, cls_q.unwrap_or(""))
}

pub(super) fn owning_class<'t>(facts: &RepoFacts<'t>, owner: &Symbol) -> Option<&'t StmtClassDef> {
    let info = facts.classes.get(owner.parent.as_ref()?)?;
    match facts.modules.get(&info.module)?.nodes[info.node as usize] {
        Cn::Stmt(Stmt::ClassDef(cls)) => Some(cls),
        _ => None,
    }
}

pub(super) fn func_def<'t>(module: &Module<'t>, owner: &Symbol) -> Option<&'t StmtFunctionDef> {
    match module.nodes[owner.node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => Some(f),
        _ => None,
    }
}

pub(super) fn line_of(module: &Module<'_>, expr: &Expr) -> u32 {
    module.lines_of(Cn::Expr(expr)).0
}
