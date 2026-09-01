//! Port of `provers/spend.py` (codemap 3.3): what a call spends - the cost a
//! caller cannot see from the signature, the catalog's world classes minus
//! `logs` (logging and printing are io, and cost a caller nothing). Read by
//! #59.

use std::collections::BTreeSet;

use ruff_python_ast::{Expr, ExprCall, Stmt, StmtFunctionDef};

use sightline_core::catalog::SPENDS;
use sightline_py_facts::astutil::{CHAIN, RECEIVERS, chain_root, fn_args};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::module::Module;
use sightline_py_facts::order;

/// Is the receiver an object the caller passed in (`source.read_text()` on
/// `source: Path`)? Then the cost is in the signature, not hidden by it.
fn handed_in(func: &Expr, params: &BTreeSet<String>) -> bool {
    let Expr::Attribute(a) = func else {
        return false;
    };
    chain_root(&a.value, &CHAIN).is_some_and(|root| params.contains(root))
}

/// The nodes a body runs: everything under it except a decorated nested def -
/// a handler a factory registers spends when the handler runs, not when the
/// factory does. An undecorated nested def is the body's own. The stack pops
/// what it pushed last, as the Python generator does, so the first spend a
/// caller sees is the same one.
pub fn runs_under<'t>(node: Cn<'t>) -> Vec<Cn<'t>> {
    let mut stack = vec![node];
    let mut out = Vec::new();
    let mut kids: Vec<Cn<'t>> = Vec::new();
    while let Some(n) = stack.pop() {
        out.push(n);
        kids.clear();
        order::children(n, &mut kids);
        for child in &kids {
            let registered = matches!(
                child,
                Cn::Stmt(Stmt::FunctionDef(f)) if !f.decorator_list.is_empty()
            );
            if !registered {
                stack.push(*child);
            }
        }
    }
    out
}

/// The parameters a def was given, receivers aside.
pub fn own_params(node: Cn<'_>) -> BTreeSet<String> {
    let Cn::Stmt(Stmt::FunctionDef(f)) = node else {
        return BTreeSet::new();
    };
    fn_args(f)
        .into_iter()
        .map(|a| a.name.to_string())
        .filter(|a| !RECEIVERS.contains(&a.as_str()))
        .collect()
}

/// The callee's parameters this call fills from `params` - the caller's own. A
/// helper's spend on one of those is the caller's caller's cost; on anything
/// else it is the caller's own.
pub fn handed_through(
    call: &ExprCall,
    def: &StmtFunctionDef,
    params: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut bound: Vec<(String, &Expr)> = fn_args(def)
        .into_iter()
        .map(|a| a.name.to_string())
        .zip(call.arguments.args.iter())
        .collect();
    for kw in &call.arguments.keywords {
        if let Some(name) = &kw.arg {
            let name = name.to_string();
            match bound.iter_mut().find(|(p, _)| *p == name) {
                Some(slot) => slot.1 = &kw.value,
                None => bound.push((name, &kw.value)),
            }
        }
    }
    bound
        .into_iter()
        .filter(|(_, v)| chain_root(v, &CHAIN).is_some_and(|root| params.contains(root)))
        .map(|(p, _)| p)
        .collect()
}

/// What this body spends that its signature does not show: the first catalog
/// call it makes on something other than what it was given (its own
/// parameters, or `given` - what a caller reading through it was given
/// itself), or a loop with no bound. `None` when it only computes: line count
/// is not a cost.
pub fn spend_of(
    module: &Module<'_>,
    node: Cn<'_>,
    given: Option<&BTreeSet<String>>,
) -> Option<String> {
    let owned;
    let seen = match given {
        Some(given) => given,
        None => {
            owned = own_params(node);
            &owned
        }
    };
    for n in runs_under(node) {
        match n {
            Cn::Stmt(Stmt::While(w)) => {
                if matches!(&*w.test, Expr::BooleanLiteral(b) if b.value) {
                    return Some("while True".to_string());
                }
            }
            Cn::Expr(Expr::Call(call)) if !handed_in(&call.func, seen) => {
                let dotted = module.dotted_name(&call.func);
                let name = match &*call.func {
                    Expr::Attribute(a) => Some(a.attr.as_str()),
                    Expr::Name(x) => Some(x.id.as_str()),
                    _ => None,
                };
                if !crate::catalog::classes_of(dotted.as_deref(), name).is_disjoint(&SPENDS) {
                    return dotted.or_else(|| name.map(str::to_string));
                }
            }
            _ => {}
        }
    }
    None
}
