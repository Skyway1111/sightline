//! Framework-fixed guard (port of `rules/framework.py`): is a signature or
//! member owned by a dispatch contract, or relocatable? One home, read by
//! #6, #10, #32, #40, #48, #55.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};

use sightline_py_facts::astutil::{fn_args, fn_params};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{
    FUNCTION_KINDS, RefKind, RepoFacts, Step, Symbol, class_walk, has_framework_base,
    has_framework_base_transitive, is_test_path,
};
use sightline_py_facts::qnames::resolve_dotted_expr;

/// Every class in a chain touching an external base: descendants answer to
/// its contract, mixins provide what it dispatches on.
pub fn framework_coupled(facts: &RepoFacts<'_>) -> HashSet<String> {
    let mut out: HashSet<String> = HashSet::new();
    for cls_q in facts.classes.keys() {
        let chain = class_walk(facts, cls_q, Step::Bases);
        if chain.iter().any(|(_, info)| has_framework_base(info)) {
            out.extend(chain.iter().map(|(q, _)| q.to_string()));
        }
    }
    // a class nested in a coupled body (django Meta) is declarative too,
    // variables included
    let nested: Vec<String> = facts
        .classes
        .keys()
        .filter(|q| {
            facts.symbols.get(&***q).is_some_and(|s| {
                s.parent
                    .as_deref()
                    .is_some_and(|parent| out.contains(parent))
            })
        })
        .map(|q| q.to_string())
        .collect();
    out.extend(nested);
    out
}

/// A `metaclass=` in the chain consumes the class body by code: its
/// variables are alive by construction (enum-choices members).
pub fn metaclassed(facts: &RepoFacts<'_>, cls_q: &str) -> bool {
    class_walk(facts, cls_q, Step::Bases)
        .iter()
        .any(|(_, info)| match facts.modules.get(&info.module) {
            Some(module) => match module.nodes[info.node as usize] {
                Cn::Stmt(Stmt::ClassDef(c)) => c.arguments.as_ref().is_some_and(|a| {
                    a.keywords
                        .iter()
                        .any(|kw| kw.arg.as_ref().is_some_and(|n| n.as_str() == "metaclass"))
                }),
                _ => false,
            },
            None => false,
        })
}

/// Method names any internal base defines: overriding one is dispatch-bound.
pub fn inherited_method_names(facts: &RepoFacts<'_>, cls_q: &str) -> HashSet<String> {
    class_walk(facts, cls_q, Step::Bases)
        .iter()
        .filter(|(q, _)| &**q != cls_q)
        .flat_map(|(_, info)| info.methods.keys().map(|n| n.to_string()))
        .collect()
}

/// The method's signature is owned by a dispatch contract: its class has a
/// framework base, or an internal base defines the same method name
/// (`logging.Formatter.format` cannot be narrowed).
pub fn is_override_fixed(facts: &RepoFacts<'_>, sym: &Symbol) -> bool {
    let Some(parent) = sym.parent.as_deref() else {
        return false;
    };
    if sym.kind != "method" || !facts.classes.contains_key(parent) {
        return false;
    }
    has_framework_base_transitive(facts, parent)
        || inherited_method_names(facts, parent).contains(&*sym.name)
}

/// `(name, param names)` two or more prod modules spell identically at
/// module level: a signature no single def chose, so its slots answer to
/// whatever loads them. #55's width and #32's unread params both defer.
pub fn plugin_signatures(facts: &RepoFacts<'_>) -> HashSet<(String, Vec<String>)> {
    let mut seen: HashMap<(String, Vec<String>), HashSet<&str>> = HashMap::new();
    for sym in facts.symbols.values() {
        let Some(module) = facts.modules.get(&sym.module) else {
            continue;
        };
        if sym.kind != "function" || sym.parent.is_some() || is_test_path(&module.rel) {
            continue;
        }
        let params = fn_params(crate::util::fn_of(module, sym))
            .into_iter()
            .map(str::to_string)
            .collect();
        seen.entry((sym.name.to_string(), params))
            .or_default()
            .insert(&sym.module);
    }
    seen.into_iter()
        .filter(|(_, mods)| mods.len() >= 2)
        .map(|(key, _)| key)
        .collect()
}

/// pass / raise / bare constants only: a template whose signature is the
/// contract (#32, #48).
pub fn is_stub(stmts: &[Stmt]) -> bool {
    stmts.iter().all(|st| {
        matches!(st, Stmt::Pass(_) | Stmt::Raise(_))
            || matches!(st, Stmt::Expr(e) if is_constant(&e.value))
    })
}

/// CPython's `Constant`: a number, a string, bytes, a bool, `None` or
/// `Ellipsis`. Ruff spells each as its own expression.
fn is_constant(value: &Expr) -> bool {
    matches!(
        value,
        Expr::NumberLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::EllipsisLiteral(_)
    )
}

/// Referenced by value (a table `fn=`, a callback, a returned closure; an
/// import is not a use), or decorated by a repo def that keeps it
/// (`@rule(...)`, `@register`): the consumer fixes the signature. `within`
/// limits both to one module, for file-scoped rules.
pub fn is_registered(facts: &RepoFacts<'_>, sym: &Symbol, within: Option<&str>) -> bool {
    let by_value = facts
        .refs_to
        .get(&sym.qname)
        .map_or(&[][..], |v| v)
        .iter()
        .filter_map(|at| facts.refs.get(*at as usize))
        .any(|r| {
            r.kind == RefKind::Load
                && !matches!(
                    facts
                        .modules
                        .get(&r.module)
                        .map(|m| m.nodes[r.node as usize]),
                    Some(Cn::Alias(_))
                )
                && within.is_none_or(|w| &*r.module == w)
        });
    by_value
        || decorator_defs(facts, sym)
            .into_iter()
            .any(|d| within.is_none_or(|w| &*d.module == w) && keeps_the_def(facts, d))
}

/// The repo defs the symbol's decorators resolve to through its module's
/// bindings (`@rule(...)`'s head is `rule`).
fn decorator_defs<'a>(facts: &'a RepoFacts<'_>, sym: &Symbol) -> Vec<&'a Symbol> {
    let Some(module) = facts.modules.get(&sym.module) else {
        return Vec::new();
    };
    let decorators = match module.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => &f.decorator_list,
        Cn::Stmt(Stmt::ClassDef(c)) => &c.decorator_list,
        _ => return Vec::new(),
    };
    decorators
        .iter()
        .map(|d| match &d.expression {
            Expr::Call(c) => &*c.func,
            other => other,
        })
        .filter_map(|head| resolve_dotted_expr(head, module, facts))
        .filter_map(|q| facts.symbols.get(&*q))
        .filter(|target| FUNCTION_KINDS.contains(&target.kind))
        .collect()
}

/// The def keeps the function it decorates: it returns its first parameter,
/// directly (`register(fn)`) or through the nested def it returns
/// (`rule(...)`'s closure), or calls it with spelled-out arguments
/// (`f(buffer, args, options)` in the wrapper it returns), which fixes the
/// arity just as hard. A splat forward (`f(*a, **kw)`) is transparent and a
/// keeper (`lru_cache`) fixes no signature.
fn keeps_the_def(facts: &RepoFacts<'_>, sym: &Symbol) -> bool {
    let Some(module) = facts.modules.get(&sym.module) else {
        return false;
    };
    let fn_def = crate::util::fn_of(module, sym);
    let args = fn_args(fn_def);
    let first = args.first().map(|a| a.name.as_str());
    if let Some(first) = first
        && module
            .nodes(&[Kind::Call], Some(&sym.qname), true)
            .into_iter()
            .filter_map(|at| match module.nodes[at as usize] {
                Cn::Expr(Expr::Call(c)) => Some(c),
                _ => None,
            })
            .any(|c| {
                matches!(&*c.func, Expr::Name(n) if n.id.as_str() == first)
                    && !c
                        .arguments
                        .args
                        .iter()
                        .any(|a| matches!(a, Expr::Starred(_)))
                    && c.arguments.keywords.iter().all(|kw| kw.arg.is_some())
            })
    {
        return true;
    }
    for at in module.nodes(&[Kind::Return], Some(&sym.qname), false) {
        let Cn::Stmt(Stmt::Return(ret)) = module.nodes[at as usize] else {
            continue;
        };
        let Some(Expr::Name(name)) = ret.value.as_deref() else {
            continue;
        };
        if first == Some(name.id.as_str()) {
            return true;
        }
        let inner = facts.symbols.get(&*format!("{}.{}", sym.qname, name.id));
        if inner.is_some_and(|i| FUNCTION_KINDS.contains(&i.kind) && keeps_the_def(facts, i)) {
            return true;
        }
    }
    false
}
