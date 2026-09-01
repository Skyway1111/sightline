//! Family C, dead weight: #32 (symbols, params, imports and `__all__` entries
//! nothing references), #60 (the same weight read off the call graph: defs no
//! call site resolves to), #56 (private symbols only tests reach) and #34
//! (commented-out code, no-op handlers, swallowed failures).
//!
//! file-length-ok: one file per rule family is this crate's shape, and a
//! RuleRecord lives beside the function it describes, the same reason
//! `surface.rs` and `tests_quality.rs` state.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Alias, ExceptHandler, Expr, ExprContext, Stmt, StmtTry};

use sightline_core::findings::{Evidence, Finding, Qname, Sink, Site};
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::astutil::{fn_args, fn_params};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{
    FUNCTION_KINDS, NodeIndex, RepoFacts, Step, Symbol, class_walk, is_test_path,
};
use sightline_py_facts::module::Module;
use sightline_py_provers::Provers;
use sightline_py_provers::annotations::none_inclusive;
use sightline_py_provers::callgraph::unspoken;
use sightline_py_provers::closed_world::keeps_signature;
use sightline_py_provers::comments::{comment_blocks, parses_as_code};
use sightline_py_provers::counterfactual::Splice;
use sightline_py_provers::handlers::{exits, handler_outcome, noop_try};
use sightline_py_provers::import_effects::binds_only;
use sightline_py_provers::imports::probes_availability;
use sightline_py_provers::liveness::{
    documented_names, module_loads, referenced_only_from_tests, referenced_outside,
};

use crate::framework::{
    framework_coupled, is_override_fixed, is_registered, is_stub, metaclassed, plugin_signatures,
};
use crate::model::Rule;
use crate::returns::return_contract_finding;
use crate::util::{deletion, enclosing_at_line, fn_of, node_site};

// --- #32 dead / unreferenced symbols ------------------------------------------

/// Does a decorator on this def or class change the signature a caller
/// reads? No other statement has any.
fn decorated_signature(module: &Module<'_>, node: NodeIndex) -> bool {
    let decorators = match module.nodes[node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => &f.decorator_list,
        Cn::Stmt(Stmt::ClassDef(c)) => &c.decorator_list,
        _ => return false,
    };
    !decorators
        .iter()
        .all(|d| keeps_signature(&d.expression, module))
}

/// The aliases an import statement binds; none for anything else.
fn aliases<'t>(module: &Module<'t>, node: NodeIndex) -> &'t [Alias] {
    match module.nodes[node as usize] {
        Cn::Stmt(Stmt::Import(n)) => &n.names,
        Cn::Stmt(Stmt::ImportFrom(n)) => &n.names,
        _ => &[],
    }
}

/// The name an alias binds locally, dotted head kept (`import a.b` binds `a`).
fn local_of(alias: &Alias) -> &str {
    let full = alias.asname.as_ref().unwrap_or(&alias.name);
    full.split('.').next().unwrap_or("")
}

/// A metaclass anywhere down the chain reads the body's names by prefix
/// (`dir(cls)`): a mixin's `_check_with_*` are the handlers.
fn metaclass_read(facts: &RepoFacts<'_>, cls_q: &str) -> bool {
    class_walk(facts, cls_q, Step::Subclasses)
        .iter()
        .any(|(q, _)| metaclassed(facts, q))
}

/// Registration and dispatch surfaces are alive by construction: defs a
/// decorator that may register wraps, override-fixed methods,
/// framework-coupled classes with their methods and class-body variables,
/// metaclass-consumed bodies, dunders, main.
fn exempt_symbol(
    facts: &RepoFacts<'_>,
    qname: &str,
    sym: &Symbol,
    coupled: &HashSet<String>,
) -> bool {
    if (sym.name.starts_with("__") && sym.name.ends_with("__")) || &*sym.name == "main" {
        return true;
    }
    let module = &facts.modules[&sym.module];
    if FUNCTION_KINDS.contains(&sym.kind)
        && (decorated_signature(module, sym.node) || is_override_fixed(facts, sym))
    {
        return true;
    }
    let owned_by_a_coupled_class =
        |parent: &str| coupled.contains(parent) || metaclass_read(facts, parent);
    match sym.kind {
        "class" => coupled.contains(qname),
        "method" => sym.parent.as_deref().is_some_and(owned_by_a_coupled_class),
        "variable" => sym.parent.as_deref().is_some_and(|parent| {
            facts.classes.contains_key(parent) && owned_by_a_coupled_class(parent)
        }),
        _ => false,
    }
}

/// Names the module binds at module scope (stores, imports, defs, under
/// `if`/`try` included); `None` where a star import makes the set unknowable.
fn module_bound(facts: &RepoFacts<'_>, module: &Module<'_>) -> Option<HashSet<String>> {
    let mut bound: HashSet<String> = module
        .nodes(&[Kind::Name], Some(&module.qname), false)
        .into_iter()
        .filter_map(|n| match module.nodes[n as usize] {
            Cn::Expr(Expr::Name(x)) if x.ctx == ExprContext::Store => Some(x.id.to_string()),
            _ => None,
        })
        .collect();
    bound.extend(
        facts
            .symbols_by_module
            .get(&module.qname)
            .into_iter()
            .flatten()
            .filter_map(|id| facts.symbols.get_index(*id as usize))
            .filter(|(_, s)| s.parent.is_none())
            .map(|(_, s)| s.name.to_string()),
    );
    for node in module.nodes(
        &[Kind::Import, Kind::ImportFrom],
        Some(&module.qname),
        false,
    ) {
        for alias in aliases(module, node) {
            if alias.name.as_str() == "*" {
                return None;
            }
            bound.insert(local_of(alias).to_string());
        }
    }
    Some(bound)
}

const SETTINGS_KEYS: [&str; 4] = ["extensions", "master_doc", "project", "html_theme"];

/// A module a tool executes for its namespace, spelling that tool's own keys:
/// Sphinx reads a docs tree's `conf` module globals by name, so every
/// top-level binding there is the file's whole interface.
fn settings_module(facts: &RepoFacts<'_>, module: &Module<'_>) -> bool {
    module.rel.rsplit('/').next() == Some("conf.py")
        && module_bound(facts, module)
            .is_some_and(|bound| SETTINGS_KEYS.iter().any(|k| bound.contains(*k)))
}

/// `(qname, symbol)` name-level liveness judges: outside tests, conftest and
/// settings modules, outside the registration and dispatch exemptions.
fn judged<'a>(facts: &'a RepoFacts<'_>, coupled: &HashSet<String>) -> Vec<(&'a Qname, &'a Symbol)> {
    let unjudged: HashSet<&Qname> = facts
        .modules
        .values()
        .filter(|m| is_test_path(&m.rel) || settings_module(facts, m))
        .map(|m| &m.qname)
        .collect();
    facts
        .symbols
        .iter()
        .filter(|(qname, sym)| {
            !unjudged.contains(&sym.module) && !exempt_symbol(facts, qname, sym, coupled)
        })
        .collect()
}

/// A dispatch pattern built around a variable reaches the name.
fn reflected(name: &str, patterns: &[(String, String)]) -> bool {
    patterns
        .iter()
        .any(|(pre, suf)| name.starts_with(pre.as_str()) && name.ends_with(suf.as_str()))
}

/// Plain functions only (method params answer to dispatch contracts, a
/// table-registered def's to its consumer, and a signature a sibling module
/// spells identically is a plugin ABI); stub templates carry params as
/// contract.
fn dead_params(
    facts: &RepoFacts<'_>,
    qname: &str,
    sym: &Symbol,
    plugins: &HashSet<(String, Vec<String>)>,
    out: &mut Sink,
) {
    if sym.kind != "function" {
        return;
    }
    let module = &facts.modules[&sym.module];
    let fn_def = fn_of(module, sym);
    let plugin_slot = sym.parent.is_none()
        && plugins.contains(&(
            sym.name.to_string(),
            fn_params(fn_def).into_iter().map(str::to_string).collect(),
        ));
    if is_registered(facts, sym, None) || is_stub(&fn_def.body) || plugin_slot {
        return;
    }
    let body_loads: HashSet<&str> = module
        .nodes(&[Kind::Name], Some(qname), true)
        .into_iter()
        .filter_map(|n| match module.nodes[n as usize] {
            Cn::Expr(Expr::Name(x)) if x.ctx == ExprContext::Load => Some(x.id.as_str()),
            _ => None,
        })
        .collect();
    for arg in fn_args(fn_def) {
        if arg.name.starts_with('_') || body_loads.contains(arg.name.as_str()) {
            continue;
        }
        let node = Cn::Param(arg).stamped().expect("a parameter has its index");
        out.push(Finding {
            rule: "32",
            site: node_site(facts, module, node),
            message: format!("param '{}' of {qname} is never read", arg.name),
            cause: format!("dead-param:{qname}:{}", arg.name),
            evidence: Evidence::idx(),
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

/// `(import statement, local name)` per alias `loads` never reads and no other
/// module reaches through: star is unknowable, `import x as x` is the
/// re-export idiom, and an availability probe is its own reader.
fn unused_aliases(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    loads: &HashSet<Box<str>>,
    reexported: Option<&std::collections::BTreeSet<Box<str>>>,
) -> Vec<(NodeIndex, String)> {
    let mut out = Vec::new();
    for node in module.nodes(&[Kind::Import, Kind::ImportFrom], None, false) {
        if let Cn::Stmt(Stmt::ImportFrom(n)) = module.nodes[node as usize]
            && n.module
                .as_ref()
                .is_some_and(|m| m.as_str() == "__future__")
        {
            continue;
        }
        if probes_availability(module, node) {
            continue;
        }
        for alias in aliases(module, node) {
            let local = local_of(alias);
            let reached = alias.name.as_str() == "*"
                || alias.asname.as_ref().map(|a| a.as_str()) == Some(alias.name.as_str())
                || loads.contains(local)
                || reexported.is_some_and(|r| r.contains(local))
                || module
                    .all_names
                    .as_ref()
                    .is_some_and(|all| all.iter().any(|n| &**n == local))
                || facts
                    .refs_to
                    .contains_key(&*format!("{}.{local}", module.qname));
            if !reached {
                out.push((node, local.to_string()));
            }
        }
    }
    out
}

/// #32's dead symbol as a patch: its own lines, plus every module-level import
/// statement the module stops loading once they are gone and that only binds
/// names - an import whose target runs something stays, because taking it
/// takes that effect and no world diffs one. A name a string or a keyword
/// argument reaches is reported and never patched.
pub fn dead_symbol_splice(cause: &str, facts: &RepoFacts<'_>, provers: &Provers) -> Option<Splice> {
    let sym = facts.symbols.get(cause.strip_prefix("dead-symbol:")?)?;
    let unseen = provers.unseen(facts);
    if unseen.strings.contains(&*sym.name) || unseen.kwargs.contains(&*sym.name) {
        return None;
    }
    let module = &facts.modules[&sym.module];
    let mut edits = deletion(module, sym.node);
    // `a, b = f()`: one dead name never takes the whole statement
    let one_name = match module.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::Assign(a)) => a.targets.len() == 1 && matches!(a.targets[0], Expr::Name(_)),
        Cn::Stmt(Stmt::AnnAssign(a)) => matches!(&*a.target, Expr::Name(_)),
        Cn::Stmt(Stmt::AugAssign(a)) => matches!(&*a.target, Expr::Name(_)),
        Cn::Stmt(Stmt::For(f)) => matches!(&*f.target, Expr::Name(_)),
        _ => false,
    };
    if edits.is_empty() || (sym.kind == "variable" && !one_name) {
        return None;
    }
    let skip = (edits[0].line, edits[edits.len() - 1].line);
    // `try: X = a` / `except ImportError: X = b`: one symbol, two module-scope
    // bindings. Deleting the recorded node leaves the other, the re-audit
    // still reports the symbol, and the patch claims what it did not do. A
    // name stored again at module scope outside the deleted span gets no
    // splice; the finding stands unfixed.
    let rebound = module
        .nodes(&[Kind::Name], None, false)
        .into_iter()
        .any(|at| {
            let Cn::Expr(Expr::Name(n)) = module.nodes[at as usize] else {
                return false;
            };
            let line = module.line_of(at);
            if n.ctx != ExprContext::Store
                || n.id.as_str() != &*sym.name
                || skip.0 <= line && line <= skip.1
            {
                return false;
            }
            let home = enclosing_at_line(facts, module, line);
            home.as_str() == &*module.qname || home.as_str() == &*sym.qname
        });
    if rebound {
        return None;
    }
    let loads = module_loads(module, skip);
    let reexports = provers.reexports(facts);
    let orphaned = unused_aliases(facts, module, &loads, reexports.get(&module.qname));
    let unused: HashSet<&str> = orphaned.iter().map(|(_, local)| local.as_str()).collect();
    let Cn::Module(m) = module.nodes[0] else {
        return None;
    };
    let top: HashSet<NodeIndex> = m
        .body
        .iter()
        .filter_map(|st| Cn::Stmt(st).stamped())
        .collect();
    let effects = provers.import_effects(facts);
    let mut once: Vec<NodeIndex> = Vec::new();
    for (node, _) in &orphaned {
        if top.contains(node) && !once.contains(node) {
            once.push(*node);
        }
    }
    for imp in once {
        if aliases(module, imp)
            .iter()
            .all(|a| unused.contains(local_of(a)))
            && binds_only(facts, module, imp, effects)
        {
            edits.extend(deletion(module, imp));
        }
    }
    Some(Splice {
        id: cause.to_string(),
        owner: module.qname.to_string(),
        edits,
        spelling: String::new(),
        imports: Vec::new(),
        param: String::new(),
    })
}

pub const RULE_32: Rule = Rule {
    record: RuleRecord {
        id: "32",
        slug: "dead-symbols",
        family: "C",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "unreferenced symbols/params/imports by name-level liveness; a \
                  self-reference is not a use, a pure wrapper decorator no registration",
        goal: "Dead code taxes every reader: an unreferenced symbol is context an \
               agent ingests for nothing.",
        lang: "py",
        scope: Scope::Repo,
        complement: "ruff F822 covers __all__ names the module never binds",
    },
    run: rule_32,
};

/// Dead symbols, dead params, dead imports - name-level liveness with
/// registration and dispatch exemptions.
fn rule_32(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let live = provers.live(facts);
    let unseen = provers.unseen(facts);
    let plugins = plugin_signatures(facts);
    let reexports = provers.reexports(facts);
    for (qname, sym) in judged(facts, &framework_coupled(facts)) {
        // a distribution's public name is called downstream, where no reference
        // set here reaches; `_` is the throwaway, nothing to delete
        let reached = referenced_outside(facts, qname, &sym.name, live)
            || reflected(&sym.name, &live.patterns)
            || unseen.named(&sym.name)
            || facts.publishes(sym)
            || &*sym.name == "_";
        if !reached {
            out.push(Finding {
                rule: "32",
                site: node_site(facts, &facts.modules[&sym.module], sym.node),
                message: format!(
                    "{} {qname} is never referenced - its name appears in no other \
                     place in the repo",
                    sym.kind
                ),
                cause: format!("dead-symbol:{qname}"),
                evidence: Evidence::idx(),
                salience: if sym.name.starts_with('_') { 2.0 } else { 1.0 },
                fix: None,
                lang: "py",
            });
        }
        dead_params(facts, qname, sym, &plugins, out);
    }
    for module in facts.modules.values() {
        // tests collect by name and inject fixtures; __init__ imports are the
        // re-export surface
        if is_test_path(&module.rel) || module.rel.ends_with("__init__.py") {
            continue;
        }
        let loads = module_loads(module, (0, 0));
        for (node, local) in unused_aliases(facts, module, &loads, reexports.get(&module.qname)) {
            out.push(Finding {
                rule: "32",
                site: node_site(facts, module, node),
                message: format!("import '{local}' in {} is never used", module.qname),
                cause: format!("dead-import:{}:{local}", module.qname),
                evidence: Evidence::idx(),
                salience: 0.0,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #60 dead by call graph ---------------------------------------------------

pub const RULE_60: Rule = Rule {
    record: RuleRecord {
        id: "60",
        slug: "dead-by-graph",
        family: "C",
        engine_class: "WP",
        posture: Posture::Report,
        meaning: "a prod def whose name still occurs but which no call site in the \
                  upgraded call graph resolves to, over a passed closed world",
        goal: "#32 can only claim a name that occurs in no other place. A def the \
               graph shows no one runs is dead weight too - the reader ingests a \
               body the program never enters - but gating it would price the \
               graph's blind spots instead of the code: a caller the oracle \
               cannot resolve reads here as no caller, so this reports and never \
               blocks.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_60,
};

/// Defs no call site in the program resolves to, over #32's judged set and
/// exemptions: its name-level arm's complement, a passed closed world, and no
/// name the graph cannot speak for.
fn rule_60(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let calls = provers.calls(facts);
    let world = provers.closed_world(facts);
    let live = provers.live(facts);
    let unseen = provers.unseen(facts);
    let graph = unspoken(facts, calls);
    // a by-name site of a name only one def holds is that def's caller, not a
    // blind spot; a method a Protocol declared in the tree names is run through
    // the protocol-typed variable, which no edge shows
    let mut homes: HashMap<&str, u32> = HashMap::new();
    for sym in facts.symbols.values() {
        if FUNCTION_KINDS.contains(&sym.kind) {
            *homes.entry(&sym.name).or_default() += 1;
        }
    }
    let protocol_methods: HashSet<&str> = facts
        .classes
        .values()
        .filter(|info| {
            info.external_bases
                .iter()
                .any(|b| b.rsplit('.').next() == Some("Protocol"))
        })
        .flat_map(|info| info.methods.keys().map(|n| &**n))
        .collect();
    for (qname, sym) in judged(facts, &framework_coupled(facts)) {
        let unnamed = graph.unnamed.get(&*sym.name).copied().unwrap_or(0);
        // a distribution's public def is its surface, published module or not:
        // a config file may name it by path; its private defs stay judged
        if !FUNCTION_KINDS.contains(&sym.kind)
            || graph.guessed.contains(qname)
            || graph.shadowed.contains(qname)
            || (unnamed > 0 && homes.get(&*sym.name) == Some(&1))
            || (sym.kind == "method" && protocol_methods.contains(&*sym.name))
            || (!facts.published.is_empty() && sym.is_public)
        {
            continue;
        }
        // an installed console script (liveness' scope "") reaches its object
        // over a seam no call site in the tree crosses
        if calls.calls_to.get(qname).is_some_and(|v| !v.is_empty())
            || live.live.get(&*sym.name).is_some_and(|s| s.contains(""))
        {
            continue;
        }
        // a name occurring in no other place is #32's: never both
        if graph.valued.contains(&*sym.name)
            || !world.verdict(qname).passed
            || !(referenced_outside(facts, qname, &sym.name, live)
                || reflected(&sym.name, &live.patterns)
                || unseen.named(&sym.name))
        {
            continue;
        }
        out.push(Finding {
            rule: "60",
            site: node_site(facts, &facts.modules[&sym.module], sym.node),
            message: format!(
                "no resolved caller in the whole program runs {} {qname} (the name \
                 occurs: {} references, {unnamed} unresolved/by-name sites)",
                sym.kind,
                facts.refs_to.get(qname).map_or(0, Vec::len)
            ),
            cause: format!("dead-by-graph:{qname}"),
            evidence: Evidence::Wp {
                premises: vec![
                    "closed-world:pass".to_string(),
                    "resolved-callers:0".to_string(),
                    "ambiguous-candidates:0".to_string(),
                ],
            },
            salience: if sym.name.starts_with('_') { 2.0 } else { 1.0 },
            fix: None,
            lang: "py",
        });
    }
}

// --- #56 test-only symbols ----------------------------------------------------

/// The class is reached from a prod module: its test-only method is a live
/// type's helper, not a feature nothing ships.
fn prod_uses(facts: &RepoFacts<'_>, cls_q: Option<&str>) -> bool {
    let Some(cls_q) = cls_q else { return false };
    facts
        .refs_to
        .get(cls_q)
        .map_or(&[][..], |v| v)
        .iter()
        .any(|id| {
            let r = &facts.refs[*id as usize];
            !facts.rel_of(&r.module).is_some_and(|rel| is_test_path(rel))
        })
}

pub const RULE_56: Rule = Rule {
    record: RuleRecord {
        id: "56",
        slug: "test-only-symbol",
        family: "C",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "unpublished prod symbols every reference to which sits under a test \
                  path; #32's registration/dispatch/entry-point exemptions apply",
        goal: "A symbol only its tests reach is not #32's dead code - its name \
               occurs somewhere, so the deletion emitters' claim fails - but it is \
               a feature nothing ships, kept alive by tests proving nothing the \
               product does: delete both, the symbol and its tests.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_56,
};

/// Unpublished prod symbols reached only by tests, over the liveness index #32
/// reads with the same roots. A published module's public name is the
/// distribution's surface: its callers live outside the tree.
fn rule_56(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let live = provers.live(facts);
    let unseen = provers.unseen(facts);
    // the repo's own prose is a publication too: a hand-run tool an index or a
    // module README names ships to the readers who run it
    let documented = documented_names(facts);
    // a method name bound on two classes: name-level liveness cannot tell
    // whose `x.reset()` a test calls, so "only tests" is unmeasurable there
    let mut homes: HashMap<&str, u32> = HashMap::new();
    for sym in facts.symbols.values() {
        if sym.kind == "method" {
            *homes.entry(&sym.name).or_default() += 1;
        }
    }
    for (qname, sym) in judged(facts, &framework_coupled(facts)) {
        // a constant only tests read is a declared convention, not a feature,
        // so #56 never judges a variable; a method on a class prod uses is
        // "move the helper into the tests", not "delete both"
        let unjudged = sym.kind == "variable"
            || (sym.kind == "method"
                && (homes.get(&*sym.name).copied().unwrap_or(0) > 1
                    || prod_uses(facts, sym.parent.as_deref())));
        if facts.publishes(sym)
            || documented.contains(&*sym.name)
            || unseen.kwargs.contains(&*sym.name)
            || unseen.strings.contains(&*sym.name)
            || unjudged
        {
            continue;
        }
        let tests = referenced_only_from_tests(facts, qname, &sym.name, live);
        if tests.is_empty() || reflected(&sym.name, &live.patterns) {
            continue;
        }
        out.push(Finding {
            rule: "56",
            site: node_site(facts, &facts.modules[&sym.module], sym.node),
            message: format!(
                "{} {qname} is referenced only by tests ({}) - delete both",
                sym.kind,
                tests.iter().map(|q| &**q).collect::<Vec<_>>().join(", ")
            ),
            cause: format!("test-only:{qname}"),
            evidence: Evidence::idx(),
            salience: if sym.name.starts_with('_') { 2.0 } else { 1.0 },
            fix: None,
            lang: "py",
        });
    }
}

// --- #34 commented-out / no-op code --------------------------------------------

const SWALLOW_MESSAGE: &str = "broad except swallows the error and returns a default - \
                               callers cannot tell failure from a result";

/// The enclosing function's return contract owns a default-return handler:
/// #33 already reports it, or its annotation declares None and the handler
/// returns None (the Optional idiom, as #33 reads it).
fn contract_owns(facts: &RepoFacts<'_>, module: &Module<'_>, handler: NodeIndex) -> bool {
    let Some(sym) = facts.enclosing_symbol(module, handler) else {
        return false;
    };
    if !FUNCTION_KINDS.contains(&sym.kind) {
        return false;
    }
    let Cn::Handler(h) = module.nodes[handler as usize] else {
        return false;
    };
    if return_contract_finding(facts, module, sym).is_some() {
        return true;
    }
    let returns_none = matches!(h.body.last(), Some(Stmt::Return(r))
        if r.value.is_none() || matches!(r.value.as_deref(), Some(Expr::NoneLiteral(_))));
    module
        .returns(sym.node)
        .is_some_and(|ret| none_inclusive(facts, &module.bindings, ret))
        && returns_none
}

/// Each broad handler with no raise that neither handles the bound error nor
/// does more than return a default, on a try whose own body has no exit: the
/// handler's `return` short-circuits a function the success path runs on past.
fn swallowed(facts: &RepoFacts<'_>, module: &Module<'_>, tr: &StmtTry) -> Vec<NodeIndex> {
    if exits(&tr.body) {
        return Vec::new();
    }
    tr.handlers
        .iter()
        .filter_map(|h| {
            let ExceptHandler::ExceptHandler(h) = h;
            let out = handler_outcome(h);
            let node = Cn::Handler(h).stamped()?;
            (out.broad
                && !out.reraises
                && !out.handles
                && out.returns_default
                && !contract_owns(facts, module, node))
            .then_some(node)
        })
        .collect()
}

pub const RULE_34: Rule = Rule {
    record: RuleRecord {
        id: "34",
        slug: "noop-code",
        family: "C",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "commented-out code; no-op re-raise handlers; a broad except \
                  returning a default past the success path",
        goal: "Delete dead weight: git remembers old code, and a handler that only \
               re-raises is noise wearing an error strategy's clothes.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_34,
};

/// Dead weight a reader pays for: commented-out code blocks, no-op try/except
/// re-raises, broad handlers returning a default the caller cannot tell from a
/// result.
fn rule_34(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for module in facts.modules.values() {
        for (start, lines) in comment_blocks(module) {
            if lines.len() < 3 || !parses_as_code(&lines) {
                continue;
            }
            out.push(Finding {
                rule: "34",
                site: Site {
                    rel: module.rel.clone(),
                    line: start,
                    col: 0,
                    symbol: module.qname.clone(),
                },
                message: format!(
                    "{} commented-out code lines - delete them; git remembers",
                    lines.len()
                ),
                cause: format!("commented-code:{}:{start}", module.qname),
                evidence: Evidence::ast(),
                salience: lines.len() as f64,
                fix: None,
                lang: "py",
            });
        }
        for node in module.nodes(&[Kind::Try], None, false) {
            let Cn::Stmt(Stmt::Try(tr)) = module.nodes[node as usize] else {
                continue;
            };
            if noop_try(tr) {
                out.push(Finding {
                    rule: "34",
                    site: node_site(facts, module, node),
                    message: "try/except that only re-raises - the handler is a no-op".to_string(),
                    cause: format!("noop-try:{}:{}", module.qname, module.line_of(node)),
                    evidence: Evidence::ast(),
                    salience: 0.0,
                    fix: None,
                    lang: "py",
                });
            }
            // a swallow in a test is not the prod defect this prices
            if is_test_path(&module.rel) {
                continue;
            }
            for handler in swallowed(facts, module, tr) {
                out.push(Finding {
                    rule: "34",
                    site: node_site(facts, module, handler),
                    message: SWALLOW_MESSAGE.to_string(),
                    cause: format!(
                        "swallowed-default-return:{}:{}",
                        module.qname,
                        module.line_of(handler)
                    ),
                    evidence: Evidence::ast(),
                    salience: 0.0,
                    fix: None,
                    lang: "py",
                });
            }
        }
    }
}
