//! The free lookups over a scope: class fields, where a name was bound or
//! drawn from, and the mutation-context test.

use super::*;

/// The node a node sits in, when it has one.
pub(super) fn parent_node<'t>(module: &Module<'t>, node: NodeIndex) -> Option<Cn<'t>> {
    module.parent_of(node).map(|p| module.nodes[p as usize])
}

/// A comprehension's `(target, iter)` pairs.
fn gens(generators: &[Comprehension]) -> Vec<(&Expr, &Expr)> {
    generators.iter().map(|g| (&g.target, &g.iter)).collect()
}

/// Field name -> the annotation the repo wrote for it, `None` when it wrote
/// none: what the class declares it holds, over the internal base chain.
/// Nearest class wins and an annotation anywhere beats none.
pub fn class_fields<'t>(facts: &RepoFacts<'t>, cls_q: &str) -> IndexMap<String, Option<&'t Expr>> {
    let mut out: IndexMap<String, Option<&'t Expr>> = IndexMap::new();
    for (_q, info) in class_walk(facts, cls_q, Step::Bases) {
        for (name, ann) in declared_by(facts, info) {
            if out.get(&name).copied().flatten().is_none() {
                out.insert(name, ann);
            }
        }
    }
    out
}

/// `(field, annotation)` one class declares: its body's own bindings first,
/// then every `self.x` its methods store. The method scopes are built here
/// rather than read from the `Provers` memo, which no `ClosedWorld` holds;
/// ceiling: one `writes` walk per method of the chain, paid per reflected
/// receiver.
pub(super) fn declared_by<'t>(
    facts: &RepoFacts<'t>,
    info: &ClassInfo,
) -> Vec<(String, Option<&'t Expr>)> {
    let Some(module) = facts.modules.get(&info.module) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Option<&'t Expr>)> = Vec::new();
    if let Cn::Stmt(Stmt::ClassDef(cls)) = module.nodes[info.node as usize] {
        for st in &cls.body {
            if let Stmt::AnnAssign(a) = st
                && let Expr::Name(n) = &*a.target
            {
                out.push((n.id.to_string(), Some(&*a.annotation)));
            }
        }
    }
    for name in module.nodes(&[Kind::Name], Some(&info.qname), false) {
        if let Cn::Expr(Expr::Name(n)) = module.nodes[name as usize]
            && n.ctx == ExprContext::Store
        {
            out.push((n.id.to_string(), None));
        }
    }
    for method_q in info.methods.values() {
        let Some(scope) = Scope::new(facts, method_q) else {
            continue;
        };
        for w in scope.writes(facts) {
            if w.kind != "attr" {
                continue;
            }
            let Cn::Expr(node) = module.nodes[w.node as usize] else {
                continue;
            };
            let Some(attr) = attr_on(node, &RECEIVERS) else {
                continue;
            };
            let owner = module
                .parent_of(w.node)
                .map(|p| module.nodes[p as usize])
                .and_then(|p| match p {
                    Cn::Stmt(Stmt::AnnAssign(a)) => Some(&*a.annotation),
                    _ => None,
                });
            out.push((attr.to_string(), owner));
        }
    }
    out
}

pub(super) fn parents(module: &Module<'_>, node: NodeIndex) -> Vec<NodeIndex> {
    let mut out = Vec::new();
    let mut cur = module.parent_of(node);
    while let Some(n) = cur {
        out.push(n);
        cur = module.parent_of(n);
    }
    out
}

/// The iterable the innermost `for`/comprehension around `node` binds `name`
/// from, at any scope; `None` when none does, or a def or lambda between them
/// takes `name` as a param (the climb needs no `Scope`).
pub fn bound_from<'t>(module: &Module<'t>, node: NodeIndex, name: &str) -> Option<&'t Expr> {
    for p in parents(module, node) {
        let cn = module.nodes[p as usize];
        if LEXICAL.contains(&cn.kind()) && all_arg_names(parameters_of(module, p)).contains(name) {
            return None;
        }
        let gens: Vec<(&Expr, &Expr)> = match cn {
            Cn::Stmt(Stmt::For(f)) => vec![(&f.target, &f.iter)],
            Cn::Expr(Expr::ListComp(c)) => gens(&c.generators),
            Cn::Expr(Expr::SetComp(c)) => gens(&c.generators),
            Cn::Expr(Expr::DictComp(c)) => gens(&c.generators),
            Cn::Expr(Expr::Generator(c)) => gens(&c.generators),
            Cn::CallGen(c, _) => gens(&c.generators),
            _ => Vec::new(),
        };
        for (target, iter) in gens {
            if matches!(target, Expr::Name(n) if n.id.as_str() == name) {
                return Some(iter);
            }
        }
    }
    None
}

/// The bound callee of the call an enclosing loop draws `name` from (`for f in
/// fields(self)`: `dataclasses.fields`).
pub fn drawn_from(module: &Module<'_>, node: NodeIndex, name: &str) -> Option<String> {
    match bound_from(module, node, name)? {
        Expr::Call(c) => module.dotted_name(&c.func),
        _ => None,
    }
}

/// Is `node` the object being mutated, judging by its parent chain: a mutator
/// call, a subscript or attribute store, an augmented assignment, at any
/// attribute depth (`CONFIG.items.append(1)` mutates `CONFIG`; a call result
/// on the way is fresh). The name-up mutation predicate, for refs at any
/// scope; a function body's own writes are `Scope`'s.
pub fn is_mutation_context(module: &Module<'_>, node: NodeIndex) -> bool {
    let Some(parent) = module.parent_of(node) else {
        return false;
    };
    match module.nodes[parent as usize] {
        Cn::Expr(Expr::Attribute(a)) => {
            if matches!(a.ctx, ExprContext::Store | ExprContext::Del) {
                return true;
            }
            let called = parent_node(module, parent)
                .is_some_and(|gp| {
                    matches!(gp, Cn::Expr(Expr::Call(c)) if Cn::Expr(&c.func).stamped() == Some(parent))
                });
            if called {
                return MUTATOR_METHODS.contains(a.attr.as_str());
            }
            is_mutation_context(module, parent)
        }
        Cn::Expr(Expr::Subscript(s)) => {
            Cn::Expr(&s.value).stamped() == Some(node)
                && matches!(s.ctx, ExprContext::Store | ExprContext::Del)
        }
        Cn::Stmt(Stmt::AugAssign(a)) => Cn::Expr(&a.target).stamped() == Some(node),
        _ => false,
    }
}

// --- the `scope` dump layer ------------------------------------------------

/// The function symbols in `facts.symbols` order.
pub fn functions<'a>(facts: &'a RepoFacts<'_>) -> Vec<&'a Qname> {
    facts
        .symbols
        .iter()
        .filter(|(_, s)| FUNCTION_KINDS.contains(&s.kind))
        .map(|(q, _)| q)
        .collect()
}
