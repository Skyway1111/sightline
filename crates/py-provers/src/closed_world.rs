//! May we claim to know
//! every caller of a symbol? Fail-closed with a named escape reason; #5 runs
//! only on passes, and fixtures pin every named reason.
//!
//! A repo that publishes nothing (`facts.published` empty) is treated as an
//! application: public names are closed unless re-exported through `__all__`
//! or a package `__init__`. A published module's public names escape as
//! `published`.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use indexmap::IndexMap;
use ruff_python_ast::{Expr, ExprContext, Stmt};
use serde_json::{Value, json};

use sightline_core::findings::Qname;
use sightline_core::verdict::CwVerdict;
use sightline_py_facts::astutil::{RECEIVERS, attr_on, fn_args, literal_affixes, walk};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{
    FUNCTION_KINDS, NodeIndex, Ref, RefKind, RepoFacts, Step, Symbol, class_chain, class_walk,
    has_framework_base_transitive, is_test_path,
};
use sightline_py_facts::module::Module;
use sightline_py_facts::qnames::resolve_qname;

use crate::Provers;
use crate::annotations::resolve;
use crate::callgraph::CallGraph;
use crate::scope::{class_fields, drawn_from, functions};

/// Decorators that wrap a def transparently - signature kept, registered in no
/// registry - as qnames through the module's bindings (builtins bare); #32
/// reads the same set: a def one of these wraps is as dead as a bare one.
pub const SIGNATURE_KEEPERS: [&str; 14] = [
    "staticmethod",
    "classmethod",
    "property",
    "functools.lru_cache",
    "functools.cache",
    "functools.cached_property",
    "functools.wraps",
    "abc.abstractmethod",
    "contextlib.contextmanager",
    "contextlib.asynccontextmanager",
    "typing.overload",
    "typing.final",
    "typing_extensions.overload",
    "typing_extensions.final",
];
const ACCESSORS: [&str; 3] = ["setter", "getter", "deleter"];
const REFLECTORS: [&str; 4] = ["getattr", "setattr", "delattr", "hasattr"];
/// receiver unknown
const NAMERS: [&str; 2] = ["operator.methodcaller", "operator.attrgetter"];
/// a module by name
const IMPORTERS: [&str; 2] = ["importlib.import_module", "__import__"];

/// The receiver of a reflective read (R19: sentinels as variants).
#[derive(Clone, Copy)]
enum Receiver<'t> {
    /// the expression the read reflects on
    On(&'t Expr),
    /// `_NAMERS`: no receiver at all
    Unknown,
    /// `_IMPORT`: the name is a module's
    Import,
    /// `_MODULE`: `globals()[k]` / bare `vars()[k]`
    ModuleTable,
}

/// A decorator that wraps the def transparently: a `SIGNATURE_KEEPERS` name
/// resolved through the module's bindings (`@mylib.cache` is not functools'),
/// or a property accessor (`@x.setter`).
pub fn keeps_signature(dec: &Expr, module: &Module<'_>) -> bool {
    let target = match dec {
        Expr::Call(c) => &*c.func,
        other => other,
    };
    if let Expr::Attribute(a) = target
        && ACCESSORS.contains(&a.attr.as_str())
    {
        return true;
    }
    let q = module.dotted_name(target).unwrap_or_else(|| match target {
        Expr::Name(n) => n.id.to_string(),
        _ => String::new(),
    });
    SIGNATURE_KEEPERS.contains(&q.as_str())
}

/// `(node, receiver, name)` per reflective read in the module: a `REFLECTORS`
/// call, a `NAMERS` call, an `IMPORTERS` call and a `globals()[k]` /
/// `vars(x)[k]` load.
fn reflections<'t>(module: &Module<'t>) -> Vec<(NodeIndex, Receiver<'t>, &'t Expr)> {
    let mut out: Vec<(NodeIndex, Receiver<'t>, &'t Expr)> = Vec::new();
    for node in module.nodes(&[Kind::Call], None, false) {
        let Cn::Expr(Expr::Call(call)) = module.nodes[node as usize] else {
            continue;
        };
        let q = module
            .dotted_name(&call.func)
            .unwrap_or_else(|| match &*call.func {
                Expr::Name(n) => n.id.to_string(),
                _ => String::new(),
            });
        let args = &call.arguments.args;
        if REFLECTORS.contains(&q.as_str()) && args.len() >= 2 {
            out.push((node, Receiver::On(&args[0]), &args[1]));
        } else if NAMERS.contains(&q.as_str()) && !args.is_empty() {
            out.push((node, Receiver::Unknown, &args[0]));
        } else if IMPORTERS.contains(&q.as_str()) && !args.is_empty() {
            out.push((node, Receiver::Import, &args[0]));
        }
    }
    for node in module.nodes(&[Kind::Subscript], None, false) {
        let Cn::Expr(Expr::Subscript(sub)) = module.nodes[node as usize] else {
            continue;
        };
        if sub.ctx != ExprContext::Load {
            continue;
        }
        let Expr::Call(table) = &*sub.value else {
            continue;
        };
        let named =
            matches!(&*table.func, Expr::Name(n) if matches!(n.id.as_str(), "globals" | "vars"));
        if !named || !table.arguments.keywords.is_empty() {
            continue;
        }
        let receiver = match table.arguments.args.first() {
            Some(arg) => Receiver::On(arg),
            None => Receiver::ModuleTable,
        };
        out.push((node, receiver, &sub.slice));
    }
    out
}

/// Where reflection reaches: names read as strings, `(prefix, suffix)`
/// patterns a built name is read around (`f"on_{k}"`), modules whose globals
/// are subscripted opaquely or whose qname a built import name matches
/// (`import_module(f"plugins.{k}")`), and the class chains of receivers
/// reflected on opaquely - `self`, or a param or `self` field declared as a
/// repo class. An undeclared receiver reaches nothing: reading it as "every
/// function" silenced #5/#37 (and the since-retired #4) entirely at zero false
/// positives removed.
#[derive(Debug, Default)]
pub struct Reach {
    pub names: BTreeSet<String>,
    pub patterns: BTreeSet<(String, String)>,
    pub modules: BTreeSet<Qname>,
    pub classes: BTreeSet<Qname>,
}

impl Reach {
    pub fn reaches(&self, sym: &Symbol) -> bool {
        self.names.contains(&*sym.name)
            || self
                .patterns
                .iter()
                .any(|(p, s)| sym.name.starts_with(p.as_str()) && sym.name.ends_with(s.as_str()))
            || (sym.parent.is_none() && self.modules.contains(&sym.module))
            || sym
                .parent
                .as_ref()
                .is_some_and(|p| self.classes.contains(p))
    }
}

static NOT_A_FUNCTION: LazyLock<CwVerdict> =
    LazyLock::new(|| CwVerdict::escaped(["not-a-function"]));

pub struct ClosedWorld {
    pub reach: Reach,
    verdicts: IndexMap<Qname, CwVerdict>,
}

impl ClosedWorld {
    pub fn build(facts: &RepoFacts<'_>, calls: &CallGraph) -> ClosedWorld {
        let mut world = ClosedWorld {
            reach: compute_reach(facts),
            verdicts: IndexMap::new(),
        };
        let verdicts: IndexMap<Qname, CwVerdict> = functions(facts)
            .into_iter()
            .map(|q| (q.clone(), world.compute(facts, calls, q)))
            .collect();
        world.verdicts = verdicts;
        world
    }

    /// The Python memo answers any qname; anything but a function symbol
    /// escapes as `not-a-function`.
    pub fn verdict(&self, qname: &str) -> &CwVerdict {
        self.verdicts.get(qname).unwrap_or(&NOT_A_FUNCTION)
    }

    fn compute(&self, facts: &RepoFacts<'_>, calls: &CallGraph, qname: &str) -> CwVerdict {
        let Some(sym) = facts.symbols.get(qname) else {
            return NOT_A_FUNCTION.clone();
        };
        if !FUNCTION_KINDS.contains(&sym.kind) {
            return NOT_A_FUNCTION.clone();
        }
        let parent = sym.parent.as_ref().and_then(|p| facts.symbols.get(p));
        if parent.is_some_and(|p| p.kind != "class") {
            return CwVerdict::escaped(["nested"]);
        }
        let Some(module) = facts.modules.get(&sym.module) else {
            return NOT_A_FUNCTION.clone();
        };
        let refs: Vec<&Ref> = facts
            .refs_to
            .get(qname)
            .map_or(&[][..], |v| v)
            .iter()
            .map(|i| &facts.refs[*i as usize])
            .collect();
        // every escape, in this order; consumers read the set
        let mut reasons: Vec<&str> = Vec::new();
        if facts.publishes(sym) {
            reasons.push("published");
        }
        if reexported(facts, module, sym, &refs) {
            reasons.push("re-export");
        }
        if refs
            .iter()
            .any(|r| !is_alias(facts, r) && r.kind != RefKind::Callee)
        {
            reasons.push("reference-escape");
        }
        if self.reach.reaches(sym) {
            reasons.push("dynamic-access");
        }
        if !keeps_every_signature(module, sym) {
            reasons.push("unknown-decorator");
        }
        if splat_forwarded(facts, calls, qname) {
            reasons.push("kwargs-forward");
        }

        // methods: a library base dispatches to its hooks from code no repo
        // file shows; overrides elsewhere in the internal hierarchy (up, down,
        // or across a shared base) reopen dispatch
        if sym.kind == "method"
            && let Some(owner) = sym.parent.as_ref()
        {
            if has_framework_base_transitive(facts, owner) {
                reasons.push("framework-base");
            }
            let overridden = class_chain(facts, owner, |i| {
                i.bases.iter().chain(i.subclasses.iter()).cloned()
            })
            .into_iter()
            .any(|(q, info)| q != *owner && info.methods.contains_key(&*sym.name));
            if overridden {
                reasons.push("method-override");
            }
        }

        if reasons.is_empty() {
            CwVerdict::passed()
        } else {
            CwVerdict::escaped(reasons)
        }
    }
}

/// An `import` binding of the symbol (the ref sits on an alias node).
fn is_alias(facts: &RepoFacts<'_>, r: &Ref) -> bool {
    facts
        .modules
        .get(&r.module)
        .is_some_and(|m| matches!(m.nodes[r.node as usize], Cn::Alias(_)))
}

/// The re-export surface: `__all__` membership, a dynamic `__all__`, or an
/// import landing in a package `__init__`.
fn reexported(facts: &RepoFacts<'_>, module: &Module<'_>, sym: &Symbol, refs: &[&Ref]) -> bool {
    let all_named = module
        .all_names
        .as_ref()
        .is_some_and(|names| names.iter().any(|n| **n == *sym.name));
    let init_import = refs.iter().any(|r| {
        is_alias(facts, r)
            && facts
                .rel_of(&r.module)
                .is_some_and(|rel| rel.ends_with("__init__.py"))
    });
    module.dynamic_all || all_named || init_import
}

/// Every decorator is on the allow-list: none changes the signature.
fn keeps_every_signature(module: &Module<'_>, sym: &Symbol) -> bool {
    match module.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => f
            .decorator_list
            .iter()
            .all(|d| keeps_signature(&d.expression, module)),
        _ => true,
    }
}

/// A caller forwards `*args` or `**kwargs` in: argument provenance is opaque.
fn splat_forwarded(facts: &RepoFacts<'_>, calls: &CallGraph, qname: &str) -> bool {
    calls.callers(qname).any(|c| {
        let Some(caller) = facts.modules.get(&c.module) else {
            return false;
        };
        match caller.nodes[c.node as usize] {
            Cn::Expr(Expr::Call(call)) => {
                call.arguments.keywords.iter().any(|kw| kw.arg.is_none())
                    || call
                        .arguments
                        .args
                        .iter()
                        .any(|a| matches!(a, Expr::Starred(_)))
            }
            _ => false,
        }
    })
}

fn compute_reach(facts: &RepoFacts<'_>) -> Reach {
    let mut reach = Reach::default();
    for module in facts.modules.values() {
        let test_site = is_test_path(&module.rel);
        for (node, receiver, name) in reflections(module) {
            let affixes = literal_affixes(name);
            if let Receiver::Import = receiver {
                // a constant name is an import edge, not a reach; a test's
                // dynamic import is the test's subject, not the program's
                // dispatch (M: one test emptied #37 and the since-retired #4
                // at 0 FP / 8 TP)
                if !test_site && let Some((prefix, suffix)) = affixes {
                    reach.modules.extend(
                        facts
                            .modules
                            .keys()
                            .filter(|q| q.starts_with(&prefix) && q.ends_with(&suffix))
                            .cloned(),
                    );
                }
            } else if let Expr::StringLiteral(s) = name {
                reach.names.insert(s.value.to_str().to_string());
            } else if let Some(affixes) = affixes {
                reach.patterns.insert(affixes);
            } else if let Receiver::ModuleTable = receiver {
                reach.modules.insert(module.qname.clone());
            } else if is_field_name(module, node, name) {
                // `f.name` off a `fields(...)` loop: a field, never a method
            } else {
                for cls_q in reflected_classes(facts, module, node, receiver) {
                    for (q, _) in class_walk(facts, &cls_q, Step::Bases) {
                        reach.classes.insert(q);
                    }
                    for (q, _) in class_walk(facts, &cls_q, Step::Subclasses) {
                        reach.classes.insert(q);
                    }
                }
            }
        }
    }
    reach
}

fn is_field_name(module: &Module<'_>, node: NodeIndex, name: &Expr) -> bool {
    let Expr::Attribute(a) = name else {
        return false;
    };
    let Expr::Name(base) = &*a.value else {
        return false;
    };
    a.attr.as_str() == "name"
        && drawn_from(module, node, base.id.as_str()).as_deref() == Some("dataclasses.fields")
}

/// The classes an opaque read at `node` reflects on through its receiver: the
/// owner class for `self`/`cls`, else what a param or a `self` field is
/// declared as (`declared_classes`).
fn reflected_classes(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    node: NodeIndex,
    receiver: Receiver<'_>,
) -> Vec<Qname> {
    let Some(owner) = facts.symbols.get(&facts.enclosing(module, node)) else {
        return Vec::new();
    };
    if !FUNCTION_KINDS.contains(&owner.kind) {
        return Vec::new();
    }
    let cls_q = owner
        .parent
        .as_ref()
        .filter(|p| facts.classes.contains_key(*p));
    let Receiver::On(receiver) = receiver else {
        return Vec::new();
    };
    match receiver {
        Expr::Name(n) if RECEIVERS.contains(&n.id.as_str()) => cls_q.cloned().into_iter().collect(),
        Expr::Name(n) => {
            let ann = match module.nodes[owner.node as usize] {
                Cn::Stmt(Stmt::FunctionDef(f)) => fn_args(f)
                    .into_iter()
                    .find(|a| a.name.as_str() == n.id.as_str())
                    .and_then(|a| Cn::Param(a).stamped())
                    .and_then(|i| module.annotation(i)),
                _ => None,
            };
            declared_classes(facts, module, ann)
        }
        _ => {
            let Some(cls_q) = cls_q else {
                return Vec::new();
            };
            let Some(attr) = attr_on(receiver, &RECEIVERS) else {
                return Vec::new();
            };
            let fields = class_fields(facts, cls_q);
            declared_classes(facts, module, fields.get(attr).copied().flatten())
        }
    }
}

/// Repo classes an annotation names by a bare name, through the module's
/// bindings, string forms and repo aliases (`Node | None`, `"Node"`,
/// `NodeLike = Node | None`); a `models.Node` chain is not read.
fn declared_classes(facts: &RepoFacts<'_>, module: &Module<'_>, ann: Option<&Expr>) -> Vec<Qname> {
    let mut out: Vec<Qname> = Vec::new();
    resolve(facts, &module.bindings, ann, &mut |bindings, expr| {
        for n in walk(Cn::Expr(expr)) {
            if let Cn::Expr(Expr::Name(name)) = n
                && let Some(bound) = bindings.get(name.id.as_str())
            {
                let (_kind, q) = resolve_qname(bound, facts, 0);
                if facts.classes.contains_key(&q) {
                    out.push(q);
                }
            }
        }
    });
    out
}

// --- the `world` dump layer ------------------------------------------------

/// One symbol's verdict row.
fn verdict_row(world: &ClosedWorld, qname: &str) -> Value {
    let v = world.verdict(qname);
    let mut reasons: Vec<&str> = v.reasons.iter().map(String::as_str).collect();
    reasons.sort();
    json!({ "passed": v.passed, "reason": v.reason, "reasons": reasons })
}

/// `layer_world`.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let world = provers.closed_world(facts);
    let rows: serde_json::Map<String, Value> = functions(facts)
        .into_iter()
        .map(|q| (q.to_string(), verdict_row(world, q)))
        .collect();
    Some(json!({ "functions": rows }))
}
