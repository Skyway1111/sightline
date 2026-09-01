//! Family C, context economics: #24, #26, #27, #29, #36, #38, #59 - what a
//! reader must ingest to use the code, and what the code makes them ingest
//! for nothing. Import topology lives in `imports.rs`, dead weight in
//! `dead.rs`.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use ruff_python_ast::{Expr, ExprCall, Stmt, StmtFunctionDef};

use sightline_core::catalog::SPENDS;
use sightline_core::clones::digest_n;
use sightline_core::findings::{Evidence, Finding, Qname, Sink, Site};
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::astutil::{
    RECEIVERS, fn_args, fn_body, literal_affixes, mentions, subnodes, walk,
};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{FUNCTION_KINDS, NodeIndex, RepoFacts, Symbol, is_test_path};
use sightline_py_facts::module::Module;
use sightline_py_provers::Provers;
use sightline_py_provers::callgraph::callee_of;
use sightline_py_provers::catalog::classes_of;
use sightline_py_provers::comments::{docstring, documents_module};
use sightline_py_provers::imports::{readers, shared_homes, under_main_guard};
use sightline_py_provers::liveness::ATTR_FUNCS;
use sightline_py_provers::scope::is_mutation_context;
use sightline_py_provers::spend::{handed_through, own_params, runs_under, spend_of};
use sightline_py_provers::typestrings::split_union;

use crate::framework::is_registered;
use crate::model::Rule;
use crate::util::{
    IGNORE_PRAGMA_RE, fn_of, is_nested, iter_prod_functions, node_site, nontrivial_literal,
};

/// `isinstance(node, ast.Constant)`: ruff spells each literal class apart.
fn is_constant(value: &Expr) -> bool {
    Cn::Expr(value).kind() == Kind::Constant
}

/// The statement index of a node CPython's traversal stamped.
fn at(node: Cn<'_>) -> NodeIndex {
    node.stamped().expect("a body node has its index")
}

// --- #24 dynamic identifier construction -------------------------------------

const NAMESPACE_FUNCS: [&str; 3] = ["globals", "locals", "vars"];

/// The def a node sits in, `None` at module scope: the nearest enclosing
/// `FunctionDef`/`AsyncFunctionDef` up the parent chain.
fn enclosing_def(module: &Module<'_>, node: NodeIndex) -> Option<NodeIndex> {
    let mut cur = module.parent_of(node);
    while let Some(up) = cur {
        if matches!(
            module.nodes[up as usize].kind(),
            Kind::FunctionDef | Kind::AsyncFunctionDef
        ) {
            return Some(up);
        }
        cur = module.parent_of(up);
    }
    None
}

/// Is the name assembled out of parts in this function (`f'_{k}_path'`,
/// `'_' + k`, `'_%s' % k`, `'_{}'.format(k)`), at the call or one binding hop
/// above it? A name that arrives whole is written somewhere a reader can grep.
fn built(module: &Module<'_>, node: NodeIndex, arg: Option<&Expr>) -> bool {
    let Some(arg) = arg else {
        return false;
    };
    if literal_affixes(arg).is_some() {
        return true;
    }
    let Expr::Name(name) = arg else {
        return false;
    };
    let Some(def) = enclosing_def(module, node) else {
        return false;
    };
    subnodes(module.nodes[def as usize], |k| k == Kind::Assign)
        .into_iter()
        .any(|n| match n {
            Cn::Stmt(Stmt::Assign(st)) => {
                st.targets.len() == 1
                    && matches!(&st.targets[0], Expr::Name(t) if t.id == name.id)
                    && literal_affixes(&st.value).is_some()
            }
            _ => false,
        })
}

/// A repo module as the receiver: a name resolved against it is a lookup grep
/// cannot follow whichever function spelled the name.
fn module_namespace(facts: &RepoFacts<'_>, module: &Module<'_>, obj: &Expr) -> bool {
    match obj {
        Expr::Name(n) => module
            .bindings
            .get(n.id.as_str())
            .is_some_and(|q| facts.modules.contains_key(q)),
        _ => false,
    }
}

/// A tuple, list or set of constants: a table written where a reader greps.
fn literal_table(value: &Expr) -> bool {
    let elts = match value {
        Expr::Tuple(t) => &t.elts,
        Expr::List(l) => &l.elts,
        Expr::Set(s) => &s.elts,
        _ => return false,
    };
    elts.iter().all(is_constant)
}

/// A literal here, or a loop variable over a literal table.
fn greppable(module: &Module<'_>, node: NodeIndex, arg: &Expr) -> bool {
    if is_constant(arg) {
        return true;
    }
    let Expr::Name(name) = arg else {
        return false;
    };
    let scope = enclosing_def(module, node).unwrap_or(0);
    let tables: BTreeSet<&str> = subnodes(module.nodes[0], |k| k == Kind::Assign)
        .into_iter()
        .filter_map(|n| match n {
            Cn::Stmt(Stmt::Assign(st)) if st.targets.len() == 1 => match &st.targets[0] {
                Expr::Name(t) if literal_table(&st.value) => Some(t.id.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    subnodes(module.nodes[scope as usize], |k| {
        matches!(k, Kind::For | Kind::Comprehension)
    })
    .into_iter()
    .any(|n| {
        let (target, iter) = match n {
            Cn::Stmt(Stmt::For(f)) => (&*f.target, &*f.iter),
            Cn::Comp(c) => (&c.target, &c.iter),
            _ => return false,
        };
        matches!(target, Expr::Name(t) if t.id == name.id)
            && (literal_table(iter)
                || matches!(iter, Expr::Name(i) if tables.contains(i.id.as_str())))
    })
}

pub const RULE_24: Rule = Rule {
    record: RuleRecord {
        id: "24",
        slug: "dynamic-identifiers",
        family: "C",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "getattr/import_module/globals[] with names assembled at runtime",
        goal: "Greppability: a name assembled at runtime can't be found by \
               search, and it blinds every whole-program guarantee (#5, #6).",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_24,
};

/// Sites that resolve an identifier at runtime: a name built here applied to
/// an object (getattr/setattr on one the function was not given) or to a
/// dynamic import, and any name that is not greppable resolved against a repo
/// namespace (a repo module's getattr, a namespace-func subscript).
fn rule_24(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for module in facts.modules.values() {
        for node in module.nodes(&[Kind::Call, Kind::Subscript], None, false) {
            let form: Option<&str> = match module.nodes[node as usize] {
                Cn::Expr(Expr::Subscript(sub)) => {
                    let func = match &*sub.value {
                        Expr::Call(c) => Some(&*c.func),
                        _ => None,
                    };
                    match func {
                        Some(Expr::Name(f))
                            if NAMESPACE_FUNCS.contains(&f.id.as_str())
                                && !greppable(module, node, &sub.slice) =>
                        {
                            Some(f.id.as_str())
                        }
                        _ => None,
                    }
                }
                Cn::Expr(Expr::Call(call)) => call_form(facts, module, node, call),
                _ => None,
            };
            let Some(form) = form else { continue };
            out.push(Finding {
                rule: "24",
                site: node_site(facts, module, node),
                message: format!(
                    "dynamic identifier construction via {form} - unfindable by \
                     search, blinds whole-program analysis"
                ),
                cause: format!("dynamic-id:{form}:{}", module.line_of(node)),
                evidence: Evidence::Ast {
                    detail: form.to_string(),
                },
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}

/// #24's two call arms: a namespace function on a built or ungreppable name,
/// and a dynamic import of a built one.
fn call_form<'t>(
    facts: &RepoFacts<'_>,
    module: &Module<'t>,
    node: NodeIndex,
    call: &'t ExprCall,
) -> Option<&'t str> {
    let args = &call.arguments.args;
    match &*call.func {
        Expr::Name(f) if ATTR_FUNCS.contains(&f.id.as_str()) && args.len() >= 2 => {
            let (obj, name_arg) = (&args[0], &args[1]);
            let placed = module_namespace(facts, module, obj) && !greppable(module, node, name_arg);
            // a receiver's own members are enumerable in one class
            let assembled = built(module, node, Some(name_arg))
                && !matches!(obj, Expr::Name(o) if RECEIVERS.contains(&o.id.as_str()));
            (placed || assembled).then_some(f.id.as_str())
        }
        Expr::Name(f) if f.id.as_str() == "__import__" && built(module, node, args.first()) => {
            Some(f.id.as_str())
        }
        Expr::Attribute(a)
            if a.attr.as_str() == "import_module" && built(module, node, args.first()) =>
        {
            Some("import_module")
        }
        _ => None,
    }
}

// --- #26 declaration literalness ---------------------------------------------

static CONST_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Z][A-Z0-9_]{2,}$").expect("a valid pattern"));

const OPAQUE_NODES: [Kind; 6] = [
    Kind::ListComp,
    Kind::SetComp,
    Kind::DictComp,
    Kind::GeneratorExp,
    Kind::Starred,
    Kind::Lambda,
];
const NON_SCALAR: [Kind; 6] = [
    Kind::List,
    Kind::Tuple,
    Kind::Set,
    Kind::Dict,
    Kind::Call,
    Kind::Subscript,
];

/// Assembled from parts elsewhere: a comprehension or splat anywhere, or a
/// BinOp over more than constants and names. A nested constructor call whose
/// leaves are literals reads top-down: a declaration, not computed.
fn is_computed_declaration(value: &Expr) -> bool {
    walk(Cn::Expr(value)).any(|n| {
        OPAQUE_NODES.contains(&n.kind())
            || (n.kind() == Kind::BinOp && !subnodes(n, |k| NON_SCALAR.contains(&k)).is_empty())
    })
}

fn is_empty_container_init(value: &Expr) -> bool {
    match value {
        Expr::List(l) => l.elts.is_empty(),
        Expr::Tuple(t) => t.elts.is_empty(),
        Expr::Set(s) => s.elts.is_empty(),
        Expr::Dict(d) => d.items.is_empty(),
        Expr::Call(c) => {
            matches!(&*c.func, Expr::Name(n) if ["list", "dict", "set"].contains(&n.id.as_str()))
                && c.arguments.args.is_empty()
                && c.arguments.keywords.is_empty()
        }
        _ => false,
    }
}

/// A transition or alias package: nothing but imports and an `__all__` the
/// sibling it re-exports owns. The star import is the declaration here.
fn reexport_shim(module: &Module<'_>) -> bool {
    let Cn::Module(m) = module.nodes[0] else {
        return false;
    };
    let body = fn_body(&m.body);
    !body.is_empty()
        && body.iter().all(|st| {
            matches!(st, Stmt::Import(_) | Stmt::ImportFrom(_))
                || (matches!(st, Stmt::Assign(_)) && mentions(Cn::Stmt(st), "__all__"))
        })
}

/// An empty-init constant appended to by later module-level code is the real
/// assemble-by-code. Module scope only: a registry filled inside a def is #9's.
fn mutated_at_module_level(module: &Module<'_>, init: NodeIndex, name: &str) -> bool {
    let line = module.line_of(init);
    let end = match module.end_line_of(init) {
        0 => line,
        e => e,
    };
    module
        .nodes(&[Kind::Name], Some(&module.qname), false)
        .into_iter()
        .filter(|n| {
            let at = module.line_of(*n);
            !(line <= at && at <= end)
        })
        .any(|n| {
            matches!(module.nodes[n as usize], Cn::Expr(Expr::Name(x)) if x.id.as_str() == name)
                && is_mutation_context(module, n)
        })
}

pub const RULE_26: Rule = Rule {
    record: RuleRecord {
        id: "26",
        slug: "declaration-literalness",
        family: "C",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "dynamic __all__, star imports, computed constant declarations",
        goal: "Declarations should be literal: a reader (or grep) must read the \
               list, not execute the code that builds it.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_26,
};

/// Module-level declarations a reader must execute: a computed `__all__`, a
/// star import, and a CONST assembled by code or filled after an empty init.
fn rule_26(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for module in facts.modules.values() {
        if reexport_shim(module) {
            continue;
        }
        let Cn::Module(m) = module.nodes[0] else {
            continue;
        };
        if module.dynamic_all {
            let st = m
                .body
                .iter()
                .find(|st| mentions(Cn::Stmt(st), "__all__"))
                .expect("a dynamic __all__ is spelled in the body");
            out.push(Finding {
                rule: "26",
                site: node_site(facts, module, at(Cn::Stmt(st))),
                message: format!("__all__ of {} is assembled by code", module.qname),
                cause: format!("dynamic-all:{}", module.qname),
                evidence: Evidence::ast(),
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
        for st in &m.body {
            let node = at(Cn::Stmt(st));
            if let Stmt::ImportFrom(imp) = st {
                if imp.names.iter().any(|a| a.name.as_str() == "*") {
                    out.push(Finding {
                        rule: "26",
                        site: node_site(facts, module, node),
                        message: format!("star import in {} hides the declaration", module.qname),
                        cause: format!(
                            "star-import:{}:{}",
                            module.qname,
                            imp.module.as_ref().map_or("", |m| m.as_str())
                        ),
                        evidence: Evidence::ast(),
                        salience: 0.0,
                        fix: None,
                        lang: "py",
                    });
                }
                continue;
            }
            let Stmt::Assign(assign) = st else { continue };
            let (Some(Expr::Name(target)), 1) = (assign.targets.first(), assign.targets.len())
            else {
                continue;
            };
            // fixture loads are test data; a path anchored at __file__ has no
            // literal form and names no members
            if !CONST_NAME_RE.is_match(target.id.as_str())
                || is_test_path(&module.rel)
                || mentions(Cn::Expr(&assign.value), "__file__")
            {
                continue;
            }
            let computed = is_computed_declaration(&assign.value)
                || (is_empty_container_init(&assign.value)
                    && mutated_at_module_level(module, node, target.id.as_str()));
            if !computed {
                continue;
            }
            out.push(Finding {
                rule: "26",
                site: node_site(facts, module, node),
                message: format!(
                    "{}.{} is assembled by code - a reader must execute it to \
                     know the members",
                    module.qname, target.id
                ),
                cause: format!("computed-declaration:{}.{}", module.qname, target.id),
                evidence: Evidence::ast(),
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #27 module purchase price -----------------------------------------------

/// module size past which a hot symbol is expensive
const PRICE_LINES: usize = 500;
const PRICE_MIN_FANIN: u32 = 3;
/// hot symbols named in the message
const PRICE_NAMED: usize = 3;
/// distinct internal modules a module loads at any scope
const FAN_OUT: usize = 10;
/// a module one parent loads is that parent's own part
const FAN_OUT_READERS: usize = 2;

/// Is every hot symbol one top-level class or a member of it: then the module
/// already is the smallest unit of its concept.
fn one_concept(facts: &RepoFacts<'_>, qname: &str, hot: &[(Qname, u32)]) -> bool {
    let prefix = format!("{qname}.");
    let owners: BTreeSet<&str> = hot
        .iter()
        .map(|(t, _)| {
            sightline_core::pytext::partition(sightline_core::pytext::removeprefix(t, &prefix), ".")
                .0
        })
        .collect();
    if owners.len() != 1 {
        return false;
    }
    let owner = owners.iter().next().expect("one owner");
    facts
        .symbols
        .get(&*format!("{qname}.{owner}"))
        .is_some_and(|cls| cls.kind == "class")
}

pub const RULE_27: Rule = Rule {
    record: RuleRecord {
        id: "27",
        slug: "purchase-price",
        family: "C",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "modules big enough that their hot symbols tax every reader; modules \
                  with a reader that import ten or more internal modules",
        goal: "Context economics: every fact lives in a container an agent must \
               ingest whole; hot symbols in huge files tax every task, and a \
               module wired to ten others makes a reader load ten files to read one.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_27,
};

/// One finding per module: the price is the module's, not each symbol's. The
/// fan-out arm counts the modules a reader must load - a TYPE_CHECKING-only
/// edge is the checker's, never a load - and needs several readers.
fn rule_27(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let graph = provers.import_graph(facts);
    let read_by = readers(facts, graph);
    for (qname, module) in &facts.modules {
        let typed = graph.typed.get(qname);
        let n = graph.full[qname]
            .iter()
            .filter(|d| !typed.is_some_and(|t| t.contains(*d)))
            .count();
        if n < FAN_OUT
            || read_by.get(qname).map_or(0, |r| r.len()) < FAN_OUT_READERS
            || module.rel.ends_with("__init__.py")
        {
            continue;
        }
        out.push(Finding {
            rule: "27",
            site: Site {
                rel: module.rel.clone(),
                line: 1,
                col: 0,
                symbol: qname.clone(),
            },
            message: format!(
                "{qname} imports {n} internal modules - a reader loads {n} files \
                 to read one"
            ),
            cause: format!("fan-out:{qname}"),
            evidence: Evidence::idx(),
            salience: n as f64,
            fix: None,
            lang: "py",
        });
    }
    let inbound_refs = facts.inbound_refs();
    let mut homes: Vec<(&Qname, &Vec<(Qname, u32)>)> = inbound_refs.iter().collect();
    homes.sort_by(|a, b| a.0.cmp(b.0));
    for (qname, inbound) in homes {
        let module = &facts.modules[qname];
        let price = module.lines.len();
        let mut hot: Vec<(Qname, u32)> = inbound
            .iter()
            .filter(|(_, n)| *n >= PRICE_MIN_FANIN)
            .cloned()
            .collect();
        hot.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        if price < PRICE_LINES || hot.is_empty() || one_concept(facts, qname, &hot) {
            continue;
        }
        let refs_in: u32 = inbound.iter().map(|(_, n)| n).sum();
        let prefix = format!("{qname}.");
        let named = hot
            .iter()
            .take(PRICE_NAMED)
            .map(|(t, n)| format!("{} ({n})", sightline_core::pytext::removeprefix(t, &prefix)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push(Finding {
            rule: "27",
            site: Site {
                rel: module.rel.clone(),
                line: 1,
                col: 0,
                symbol: qname.clone(),
            },
            message: format!(
                "{qname} is {price} lines holding {} hot symbols, led by {named} \
                 - every reader pays the whole file",
                hot.len()
            ),
            cause: format!("price:{qname}"),
            evidence: Evidence::idx(),
            salience: (price as f64) * f64::from(refs_in),
            fix: None,
            lang: "py",
        });
    }
}

// --- #29 top-loading ----------------------------------------------------------

/// small files are never punished: a def-count trigger slides into
/// doc-presence scoring on tiny modules
const TOPLOAD_MIN_LINES: usize = 150;

pub const RULE_29: Rule = Rule {
    record: RuleRecord {
        id: "29",
        slug: "top-loading",
        family: "C",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "undocumented big modules",
        goal: "Top-load the map: the first screen should say what a module is \
               (#59 asks the same of an entry point's cost).",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_29,
};

/// A module past the line bar whose first screen says nothing about it: no
/// docstring, and no leading comment block that reads as one.
fn rule_29(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    // test modules are not entry points a reader budgets for
    for module in facts.modules.values() {
        let Cn::Module(m) = module.nodes[0] else {
            continue;
        };
        if is_test_path(&module.rel)
            || module.lines.len() < TOPLOAD_MIN_LINES
            || docstring(&m.body).is_some()
            || documents_module(module)
        {
            continue;
        }
        let top_defs = m
            .body
            .iter()
            .filter(|st| matches!(st, Stmt::FunctionDef(_) | Stmt::ClassDef(_)))
            .count();
        out.push(Finding {
            rule: "29",
            site: Site {
                rel: module.rel.clone(),
                line: 1,
                col: 0,
                symbol: module.qname.clone(),
            },
            message: format!(
                "{} ({} lines, {top_defs} top-level defs) has no top-loading docstring",
                module.qname,
                module.lines.len()
            ),
            cause: format!("top-loading:{}", module.qname),
            evidence: Evidence::ast(),
            salience: module.lines.len() as f64,
            fix: None,
            lang: "py",
        });
    }
}

// --- #59 entry-point cost docs ------------------------------------------------

const HEAVY_SPAN: u32 = 30;

/// The functions this file defines that the body calls by name or on
/// `self`/`cls`, each with the parameters the call fills from the entry
/// point's own.
fn same_file_callees<'a>(
    facts: &'a RepoFacts<'_>,
    module: &Module<'_>,
    sym: &Symbol,
) -> Vec<(&'a Symbol, BTreeSet<String>)> {
    let params = own_params(module.nodes[sym.node as usize]);
    let mut out = Vec::new();
    for n in runs_under(module.nodes[sym.node as usize]) {
        let Cn::Expr(Expr::Call(call)) = n else {
            continue;
        };
        let (home, name) = match &*call.func {
            Expr::Attribute(a) => {
                let home = match &*a.value {
                    Expr::Name(recv) if RECEIVERS.contains(&recv.id.as_str()) => {
                        sym.parent.as_deref()
                    }
                    _ => None,
                };
                (home, a.attr.as_str())
            }
            Expr::Name(f) => (Some(&*module.qname), f.id.as_str()),
            _ => continue,
        };
        let Some(home) = home else { continue };
        let Some(callee) = facts.symbols.get(&*format!("{home}.{name}")) else {
            continue;
        };
        if FUNCTION_KINDS.contains(&callee.kind)
            && callee.module == module.qname
            && callee.qname != sym.qname
        {
            let given = handed_through(call, fn_of(module, callee), &params);
            out.push((callee, given));
        }
    }
    out
}

/// What this entry point spends: in its own body; in a helper the file defines
/// and it calls; else through one repo call of its own.
fn spend(
    facts: &RepoFacts<'_>,
    provers: &Provers,
    module: &Module<'_>,
    sym: &Symbol,
) -> Option<String> {
    if let Some(spent) = spend_of(module, module.nodes[sym.node as usize], None) {
        return Some(spent);
    }
    for (callee, given) in same_file_callees(facts, module, sym) {
        let spent = spend_of(module, module.nodes[callee.node as usize], Some(&given))
            .or_else(|| spends_past_file(facts, provers, callee));
        if let Some(spent) = spent {
            return Some(format!("{} -> {spent}", callee.name));
        }
    }
    spends_past_file(facts, provers, sym)
}

/// What this entry point spends through a repo callee the file does not
/// define: the first such body's spend, one hop over the call graph.
fn spends_past_file(facts: &RepoFacts<'_>, provers: &Provers, sym: &Symbol) -> Option<String> {
    let calls = provers.calls(facts);
    let module = &facts.modules[&sym.module];
    for n in runs_under(module.nodes[sym.node as usize]) {
        let Cn::Expr(Expr::Call(_)) = n else { continue };
        let site = n
            .stamped()
            .and_then(|node| calls.by_node(facts, module.id, node));
        let callee = site
            .and_then(|s| callee_of(facts, s))
            .and_then(|q| facts.symbols.get(&*q));
        let Some(callee) = callee else { continue };
        if callee.qname == sym.qname {
            continue;
        }
        let home = &facts.modules[&callee.module];
        if let Some(spent) = spend_of(home, home.nodes[callee.node as usize], None) {
            return Some(format!("{} -> {spent}", callee.qname));
        }
    }
    None
}

/// Is this def what the module runs under its `__name__` guard, and nothing
/// else calls it? Then it is `main` by another name. One hop further out is
/// the same shape, but no further.
fn script_entry(facts: &RepoFacts<'_>, module: &Module<'_>, sym: &Symbol, hops: u32) -> bool {
    let refs = facts.refs_to.get(&sym.qname).map_or(&[][..], |v| v);
    !refs.is_empty()
        && refs.iter().all(|id| {
            let r = &facts.refs[*id as usize];
            r.module == module.qname
                && (under_main_guard(module, r.node)
                    || (hops > 0
                        && facts
                            .enclosing_symbol(module, r.node)
                            .is_some_and(|caller| {
                                caller.qname != sym.qname
                                    && script_entry(facts, module, caller, hops - 1)
                            })))
        })
}

/// Does the signature already say what the body spends: a return annotation
/// the catalog classes as a spend, or a parameter named for polling?
fn signature_declares(module: &Module<'_>, sym: &Symbol, fn_def: &StmtFunctionDef) -> bool {
    let name = match module.returns(sym.node) {
        Some(Expr::Name(n)) => Some(n.id.to_string()),
        Some(Expr::Attribute(a)) => Some(a.attr.to_string()),
        _ => None,
    };
    !classes_of(None, name.as_deref()).is_disjoint(&SPENDS)
        || fn_args(fn_def).iter().any(|a| a.name.contains("poll"))
}

pub const RULE_59: Rule = Rule {
    record: RuleRecord {
        id: "59",
        slug: "cost-docstring",
        family: "C",
        engine_class: "AST+IDX",
        posture: Posture::Ratchet,
        meaning: "heavy entry points that spend off the machine without saying so",
        goal: "An entry point's first screen should say what a call costs where the \
               caller cannot walk it back: another machine, another process, state \
               everyone shares, data deleted (sanctioned presence exception).",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_59,
};

/// A public prod def past the heavy span that spends and documents nothing -
/// in a docstring or in its signature.
fn rule_59(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_prod_functions(facts) {
        let fn_def = fn_of(module, sym);
        let line = module.line_of(sym.node);
        let span = match module.end_line_of(sym.node) {
            0 => line,
            end => end,
        } - line;
        let Cn::Module(m) = module.nodes[0] else {
            continue;
        };
        // a script's entry point is documented by the module's first screen:
        // a docstring, or for `main` the header comment block
        let module_documents = if &*sym.name == "main" {
            docstring(&m.body).is_some() || documents_module(module)
        } else {
            script_entry(facts, module, sym, 1) && docstring(&m.body).is_some()
        };
        // a same-module table's fn is documented at the table
        let judged = sym.is_public
            && !is_nested(facts, sym)
            && !is_registered(facts, sym, Some(&sym.module))
            && span >= HEAVY_SPAN
            && docstring(&fn_def.body).is_none()
            && !module_documents
            && !signature_declares(module, sym, fn_def);
        if !judged {
            continue;
        }
        let Some(spent) = spend(facts, provers, module, sym) else {
            continue;
        };
        out.push(Finding {
            rule: "59",
            site: node_site(facts, module, sym.node),
            message: format!(
                "heavy entry point {} ({span} lines) spends ({spent}) and declares \
                 no cost in a docstring",
                sym.qname
            ),
            cause: format!("cost-docstring:{}", sym.qname),
            evidence: Evidence::ast(),
            salience: f64::from(span),
            fix: None,
            lang: "py",
        });
    }
}

// --- #38 value duplication ----------------------------------------------------

/// `(statement, (type, repr))` per module-level literal declaration.
fn module_declarations(module: &Module<'_>) -> Vec<(NodeIndex, (&'static str, String))> {
    let Cn::Module(m) = module.nodes[0] else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for st in &m.body {
        let (target, value) = match st {
            Stmt::Assign(a) if a.targets.len() == 1 => (Some(&a.targets[0]), Some(&*a.value)),
            Stmt::AnnAssign(a) => (Some(&*a.target), a.value.as_deref()),
            _ => (None, None),
        };
        if !matches!(target, Some(Expr::Name(_))) {
            continue;
        }
        if let Some(key) = nontrivial_literal(value) {
            out.push((at(Cn::Stmt(st)), key));
        }
    }
    out
}

pub const RULE_38: Rule = Rule {
    record: RuleRecord {
        id: "38",
        slug: "value-duplication",
        family: "C",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "same string literal declared at module level in >=3 modules of one \
                  shipping bundle",
        goal: "One fact, one home: value copies drift independently, and the next \
               fix updates some of them.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_38,
};

/// What groups #38's sites: the shipping home beside the literal's identity.
/// The group order is this key's sort order.
type ValueKey = (String, (&'static str, String));

/// Same nontrivial literal declared at module level in >=3 prod modules
/// sharing a home (#11's reading): a bundle that ships on its own holds its
/// own copy because it has to, and every site of one group is one cause.
fn rule_38(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    let homes = shared_homes(facts);
    let mut by_value: IndexMap<ValueKey, Vec<(&Module<'_>, NodeIndex)>> = IndexMap::new();
    for module in facts.modules.values() {
        // expected-value re-declarations are test data
        if is_test_path(&module.rel) {
            continue;
        }
        let home = homes[&module.qname].clone();
        for (node, key) in module_declarations(module) {
            by_value
                .entry((home.clone(), key))
                .or_default()
                .push((module, node));
        }
    }
    let mut groups: Vec<_> = by_value.into_iter().collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    for ((_, key), hits) in groups {
        let mut modules: Vec<&Qname> = hits.iter().map(|(m, _)| &m.qname).collect();
        modules.sort();
        modules.dedup();
        if modules.len() < 3 {
            continue;
        }
        let digest = digest_n(&format!("{}|{}", key.0, key.1), 8);
        let shown = modules
            .iter()
            .take(4)
            .map(|q| &***q)
            .collect::<Vec<_>>()
            .join(", ")
            + if modules.len() > 4 { " ..." } else { "" };
        let shortened: String = key.1.chars().take(40).collect();
        for (module, node) in &hits {
            out.push(Finding {
                rule: "38",
                site: node_site(facts, module, *node),
                message: format!(
                    "literal {shortened} declared in {} modules ({shown}) - one \
                     fact, one home",
                    modules.len()
                ),
                cause: format!("value-dup:{digest}"),
                evidence: Evidence::Idx {
                    detail: shortened.clone(),
                },
                salience: modules.len() as f64,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #36 type-lie density -----------------------------------------------------
// Pragmas count wherever prod code lives (config excludes don't launder them).
// A `cast()` is a typed claim the checker keeps checking against, so it never
// counts toward the density.

/// lines per pragma: sparser than this is targeted, not dense
const LIE_SPACING: usize = 20;

fn caller_is_typed(facts: &RepoFacts<'_>, enclosing: &str) -> bool {
    let Some(owner) = facts.symbols.get(enclosing) else {
        return false;
    };
    if !FUNCTION_KINDS.contains(&owner.kind) {
        return false;
    }
    let module = &facts.modules[&owner.module];
    let fn_def = fn_of(module, owner);
    module.returns(owner.node).is_some()
        || fn_args(fn_def).iter().any(|a| {
            Cn::Param(a)
                .stamped()
                .is_some_and(|p| module.annotation(p).is_some())
        })
}

pub const RULE_36: Rule = Rule {
    record: RuleRecord {
        id: "36",
        slug: "type-lie-density",
        family: "C",
        engine_class: "AST+ORACLE",
        posture: Posture::Ratchet,
        meaning: "per-module ignore-pragma density; Any-laundering helpers",
        goal: "The oracle's guarantees stop at every silencing pragma: dense \
               ignores and Any-laundering helpers blind #2/#5 and every reader \
               who trusts the types.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_36,
};

/// Per-module ignore-pragma density in prod code; Any-laundering helpers whose
/// oracle-revealed `Any` return feeds typed callers.
fn rule_36(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let mut modules: Vec<&Module<'_>> = facts.modules.values().collect();
    modules.sort_by(|a, b| a.qname.cmp(&b.qname));
    for module in modules {
        // a fixture's ignores blind no prover
        if is_test_path(&module.rel) {
            continue;
        }
        let pragmas: Vec<u32> = module
            .comments
            .iter()
            .filter(|c| IGNORE_PRAGMA_RE.is_match(&c.text))
            .map(|c| c.line)
            .collect();
        if pragmas.len() < 3 || pragmas.len() * LIE_SPACING < module.lines.len() {
            continue;
        }
        out.push(Finding {
            rule: "36",
            site: Site {
                rel: module.rel.clone(),
                line: *pragmas.iter().min().expect("three pragmas at least"),
                col: 0,
                symbol: module.qname.clone(),
            },
            message: format!(
                "{} silences the type checker {}x (ignore pragmas)",
                module.qname,
                pragmas.len()
            ),
            cause: format!("type-lies:{}", module.qname),
            evidence: Evidence::ast(),
            salience: pragmas.len() as f64,
            fix: None,
            lang: "py",
        });
    }
    let ret_types = provers.ret_types(facts);
    // a top-level `Any` launders at every call site; Any nested in a generic
    // (dict[str, Any]) is intrinsic to the data
    for qname in ret_types.candidates() {
        let Some(ret) = ret_types.return_type(qname) else {
            continue;
        };
        if !split_union(ret).contains(&"Any") {
            continue;
        }
        let sym = &facts.symbols[&**qname];
        let mut typed_callers: Vec<&Qname> = provers
            .calls(facts)
            .callers(qname)
            .filter(|c| caller_is_typed(facts, &c.enclosing))
            .map(|c| &c.enclosing)
            .collect();
        typed_callers.sort();
        typed_callers.dedup();
        if typed_callers.is_empty() {
            continue;
        }
        let shown = typed_callers
            .iter()
            .take(3)
            .map(|q| &***q)
            .collect::<Vec<_>>()
            .join(", ");
        out.push(Finding {
            rule: "36",
            site: node_site(facts, &facts.modules[&sym.module], sym.node),
            message: format!(
                "{qname} has no return annotation and returns `{ret}` into typed \
                 callers ({shown}) - an Any-laundering helper"
            ),
            cause: format!("any-laundering:{qname}"),
            evidence: Evidence::Oracle {
                rule: "unknown-return".to_string(),
                grounded: false,
                message: ret.to_string(),
            },
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}
