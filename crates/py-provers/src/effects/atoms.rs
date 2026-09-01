//! What one call site or one write reads as an effect atom, before the
//! fold over the call graph (`effects.rs`'s first half).

use super::*;

pub fn raised_name(module: &Module<'_>, node: &StmtRaise) -> Option<String> {
    let exc = node.exc.as_deref()?;
    let exc = match exc {
        Expr::Call(c) => &*c.func,
        other => other,
    };
    let dotted = module.dotted_name(exc).or_else(|| match exc {
        Expr::Name(n) if is_exception(n.id.as_str()) => Some(n.id.to_string()),
        _ => None,
    });
    Some(match dotted {
        None => UNNAMED.to_string(),
        Some(dotted) => sightline_core::pytext::rpartition(&dotted, ".")
            .2
            .to_string(),
    })
}

/// An external callee whose catalog class is world contact, spelled through
/// the module's bindings, by the name it calls on any receiver, or by a home
/// the oracle named (`conn.execute` is `sqlite3.Connection.execute`).
pub(super) fn is_io(module: &Module<'_>, call: &CallSite) -> bool {
    let Cn::Expr(Expr::Call(node)) = module.nodes[call.node as usize] else {
        return false;
    };
    let name = match &*node.func {
        Expr::Attribute(a) => Some(a.attr.as_str()),
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    };
    let own = module.dotted_name(&node.func);
    std::iter::once(own.as_deref())
        .chain(call.candidates.iter().map(|c| Some(&**c)))
        .any(|dotted| !classes_of(dotted, name).is_disjoint(&IO))
}

/// Does a write at `node` reach past the root object's own slots: a store or
/// mutator below one level (`p.items[k] = v`, `self.items.append`) or an
/// in-place operation on a slot (`self.items += [1]`)?
pub(super) fn through_field(module: &Module<'_>, node: NodeIndex) -> bool {
    let value = match module.nodes[node as usize] {
        Cn::Expr(Expr::Attribute(a)) => &*a.value,
        Cn::Expr(Expr::Subscript(s)) => &*s.value,
        _ => return false,
    };
    !matches!(value, Expr::Name(_))
        || matches!(
            module.parent_of(node).map(|p| module.nodes[p as usize]),
            Some(Cn::Stmt(Stmt::AugAssign(_)))
        )
}

/// The caller's atoms for a callee mutating the object `expr` holds: its own
/// receiver keeps `atom` (a field of it: `mutates-field`), a param or an alias
/// of one is `mutates-arg`/`slots-arg`, a module variable a `gw:`/`gs:`, a
/// local the body bound nothing, a display or a call result nothing; else
/// unknown.
fn owned(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    scope: &Scope,
    expr: Option<&Expr>,
    atom: &str,
    recv: bool,
) -> Option<BTreeSet<String>> {
    let expr = expr?;
    let Some(root) = chain_root(expr, &CHAIN) else {
        return Some(BTreeSet::new());
    };
    // a receiver passed over whole by name: the callee worked inside that
    // object's own state, so the caller's fact is that a passed object had its
    // slots written - not that this caller stored into a binding or reached
    // into a container (`CONFIG.reg.register(x)` reached: a `gw:`)
    let slots = recv && matches!(expr, Expr::Name(_));
    let params = scope.params(facts);
    let is_param = params.iter().any(|p| p == root);
    if RECEIVERS.contains(&root) && is_param {
        let atom = if matches!(expr, Expr::Name(_)) {
            atom
        } else {
            MUTATES_FIELD
        };
        return Some(BTreeSet::from([atom.to_string()]));
    }
    if is_param || scope.alias_tainted(facts).contains(root) {
        let atom = if slots { "slots-arg" } else { "mutates-arg" };
        return Some(BTreeSet::from([atom.to_string()]));
    }
    if scope.stored(facts).contains(root) {
        return Some(BTreeSet::new());
    }
    // `CONFIG.reg.register(x)` mutates CONFIG
    let bound = module.bindings.get(root)?;
    if !facts.symbols.contains_key(bound) {
        return None;
    }
    let kind = if slots { "gs" } else { "gw" };
    Some(BTreeSet::from([format!("{kind}:{bound}")]))
}

/// What the callee's receiver atoms mean to this site's caller: a class call's
/// object is fresh, but the constructor's field writes reach what it was
/// given; a method call's receiver is the expression before the dot, or the
/// first argument of an unbound `Cls.m(obj)` / plain `f(obj)`.
pub(super) fn translate(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    scope: &Scope,
    call: &ruff_python_ast::ExprCall,
    is_class: bool,
) -> Xlate {
    if is_class {
        let args: Vec<&Expr> = call
            .arguments
            .args
            .iter()
            .map(|a| match a {
                Expr::Starred(s) => &*s.value,
                other => other,
            })
            .chain(call.arguments.keywords.iter().map(|kw| &kw.value))
            .collect();
        let mut fields = Some(BTreeSet::new());
        for arg in args {
            let held = owned(facts, module, scope, Some(arg), MUTATES_FIELD, false);
            fields = match (fields, held) {
                (Some(mut have), Some(more)) => {
                    have.extend(more);
                    Some(have)
                }
                _ => None,
            };
        }
        return Xlate {
            mutates_self: Some(BTreeSet::new()),
            mutates_field: fields,
        };
    }
    let mut receiver: Option<&Expr> = match &*call.func {
        Expr::Attribute(a) => Some(&a.value),
        _ => None,
    };
    let on_class = receiver
        .and_then(|r| module.dotted_name(r))
        .and_then(|q| facts.symbols.get(q.as_str()).map(is_class_symbol))
        .unwrap_or(false);
    if receiver.is_none() || on_class {
        receiver = call.arguments.args.first();
    }
    Xlate {
        mutates_self: owned(facts, module, scope, receiver, MUTATES_SELF, true),
        mutates_field: owned(facts, module, scope, receiver, MUTATES_FIELD, true),
    }
}

/// Name -> the first line a branch of `owner` tests it; a local bound from an
/// expression reading such a name counts as that name (`cached = C.get(k)`
/// then `if cached: return` tests C). A guard testing one of the names its own
/// branch assigns covers the whole fill: `if _ENGINE is None: _ENGINE = ...;
/// _FACTORY = ...` is one memo, not a memo beside a rebind.
pub(super) fn branch_tested(module: &Module<'_>, owner: &str) -> HashMap<String, u32> {
    let mut tested: HashMap<String, u32> = HashMap::new();
    let assigns = module.nodes(&[Kind::Assign], Some(owner), true);
    let targets = |st: NodeIndex| -> Vec<&str> {
        match module.nodes[st as usize] {
            Cn::Stmt(Stmt::Assign(a)) => a
                .targets
                .iter()
                .filter_map(|t| match t {
                    Expr::Name(n) => Some(n.id.as_str()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    };
    let see = |tested: &mut HashMap<String, u32>, expr: &Expr, line: u32| {
        for n in walk(Cn::Expr(expr)) {
            if let Cn::Expr(Expr::Name(x)) = n {
                let at = tested.entry(x.id.to_string()).or_insert(line);
                *at = (*at).min(line);
            }
        }
    };
    for node in module.nodes(&[Kind::If], Some(owner), true) {
        let test = match module.nodes[node as usize] {
            Cn::Stmt(Stmt::If(n)) => Some(&*n.test),
            Cn::Elif(rest) => rest[0].test.as_ref(),
            _ => None,
        };
        let Some(test) = test else { continue };
        let line = module.line_of(node);
        see(&mut tested, test, line);
        let (lo, hi) = line_span((line, module.end_line_of(node)));
        let filled: BTreeSet<&str> = assigns
            .iter()
            .filter(|st| {
                let at = module.line_of(**st);
                lo <= at && at <= hi
            })
            .flat_map(|st| targets(*st))
            .collect();
        let named: BTreeSet<&str> = walk(Cn::Expr(test))
            .filter_map(|n| match n {
                Cn::Expr(Expr::Name(x)) => Some(x.id.as_str()),
                _ => None,
            })
            .collect();
        if filled.intersection(&named).next().is_some() {
            for name in filled {
                let at = tested.entry(name.to_string()).or_insert(line);
                *at = (*at).min(line);
            }
        }
    }
    for st in &assigns {
        let lines: Vec<u32> = targets(*st)
            .into_iter()
            .filter_map(|t| tested.get(t).copied())
            .collect();
        if let Some(first) = lines.into_iter().min()
            && let Cn::Stmt(Stmt::Assign(a)) = module.nodes[*st as usize]
        {
            see(&mut tested, &a.value, first);
        }
    }
    tested
}

/// A module variable holding an object with slots to write: a container
/// display or constructor, or a repo-class instance (`CONFIG = Config()`); a
/// function's result or an external class's is no one's to read as mutable.
pub(super) fn mutable_global(facts: &RepoFacts<'_>, sym: &Symbol) -> bool {
    let Some(module) = facts.modules.get(&sym.module) else {
        return false;
    };
    let value = match module.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::Assign(a)) => Some(&*a.value),
        Cn::Stmt(Stmt::AnnAssign(a)) => a.value.as_deref(),
        Cn::Stmt(Stmt::AugAssign(a)) => Some(&*a.value),
        _ => None,
    };
    if is_mutable_init(value) {
        return true;
    }
    let Some(Expr::Call(call)) = value else {
        return false;
    };
    let Some(dotted) = module.dotted_name(&call.func) else {
        return false;
    };
    let (kind, q) = resolve_qname(&dotted, facts, 0);
    kind == "symbol" && facts.symbols.get(&q).is_some_and(is_class_symbol)
}

pub(super) type Direct = (
    IndexMap<Qname, BTreeSet<String>>,
    HashSet<Qname>,
    BTreeMap<String, BTreeSet<String>>,
    HashMap<(String, String), Xlate>,
);

pub(super) fn is_class_symbol(sym: &Symbol) -> bool {
    sym.kind == "class"
}
