//! Name-level liveness (#32,
//! vulture semantics) and the reflection vocabulary that defeats it - #24
//! reads the same dynamic-name calls.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use ruff_python_ast::{Expr, ExprContext, Stmt, StmtClassDef};
use serde_json::{Map, Value, json};

use sightline_core::findings::Qname;
use sightline_core::pytext;
use sightline_py_facts::astutil::{literal_affixes, walk};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{
    NodeIndex, RefKind, RepoFacts, Resolution, Step, class_walk, is_test_path,
};
use sightline_py_facts::module::Module;
use sightline_py_facts::unparse;

use crate::Provers;

/// The reflection functions: a name they build is never grep-visible (#24)
/// and is live wherever it resolves (#32).
pub const ATTR_FUNCS: [&str; 4] = ["getattr", "setattr", "delattr", "hasattr"];

static IDENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("a literal pattern"));

const REFLECTORS: [&str; 3] = [
    "dataclasses.asdict",
    "dataclasses.astuple",
    "dataclasses.fields",
];

/// The node classes a quoted annotation can hang off.
const ANNOTATED: [Kind; 4] = [
    Kind::Arg,
    Kind::AnnAssign,
    Kind::FunctionDef,
    Kind::AsyncFunctionDef,
];

/// A module-level literal container a prod string table can live in.
const TABLES: [Kind; 4] = [Kind::Tuple, Kind::List, Kind::Set, Kind::Dict];

/// The declared type of a node a quoted annotation can hang off, R15's
/// lifted annotations included.
fn annotation_of<'m>(module: &'m Module<'_>, node: NodeIndex) -> Option<&'m Expr> {
    match module.nodes[node as usize] {
        Cn::Param(_) => module.annotation(node),
        Cn::Stmt(Stmt::AnnAssign(a)) => Some(&a.annotation),
        Cn::Stmt(Stmt::FunctionDef(_)) => module.returns(node),
        _ => None,
    }
}

/// The text a `Constant` holds when that is a string, as `is_const_str` reads
/// it: a plain literal, or one folded run of an f-string's chunks.
fn const_str<'a>(node: Cn<'a>) -> Option<&'a str> {
    match node {
        Cn::Expr(Expr::StringLiteral(s)) => Some(s.value.to_str()),
        Cn::FConst { owner, .. } => Some(owner.map_or("", |l| &l.value)),
        _ => None,
    }
}

/// (identifier, the annotated node) per name spelled inside a quoted
/// annotation: a forward reference the AST never Loads.
fn quoted_annotation_names(module: &Module<'_>) -> Vec<(Box<str>, NodeIndex)> {
    let mut out = Vec::new();
    for node in module.nodes(&ANNOTATED, None, false) {
        let Some(ann) = annotation_of(module, node) else {
            continue;
        };
        for reached in walk(Cn::Expr(ann)) {
            let Some(text) = const_str(reached) else {
                continue;
            };
            out.extend(
                IDENT_RE
                    .find_iter(text)
                    .map(|m| (Box::from(m.as_str()), node)),
            );
        }
    }
    out
}

/// Names this module reads (#32's import arm), quoted forward-ref
/// annotations included - they use imports the AST never Loads. `skip` reads
/// a line span as already deleted: the answer to "what would this module stop
/// loading without it".
pub fn module_loads(module: &Module<'_>, skip: (u32, u32)) -> HashSet<Box<str>> {
    let cut = |line: u32| skip.0 <= line && line <= skip.1;
    let mut loads: HashSet<Box<str>> = HashSet::new();
    for node in module.nodes(&[Kind::Name], None, false) {
        if let Cn::Expr(Expr::Name(n)) = module.nodes[node as usize]
            && n.ctx == ExprContext::Load
            && !cut(module.line_of(node))
        {
            loads.insert(n.id.as_str().into());
        }
    }
    for (name, node) in quoted_annotation_names(module) {
        if !cut(module.line_of(node)) {
            loads.insert(name);
        }
    }
    loads
}

/// Names reached with no reference the index resolves. `strings` and `attrs`
/// are invisible to a checker too, so no world vetoes taking them; `kwargs`
/// names a record field spelled only where it is constructed; `tables` and
/// `test_attrs` silence #32/#48.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Unseen {
    /// a call is passed the name as text
    pub strings: HashSet<Box<str>>,
    /// a keyword argument's name
    pub kwargs: HashSet<Box<str>>,
    /// reached as an attribute, receiver unresolved
    pub attrs: HashSet<Box<str>>,
    /// a prod module-level literal container spells it
    pub tables: HashSet<Box<str>>,
    /// a test's attribute load the index resolved to nothing
    pub test_attrs: HashSet<Box<str>>,
}

impl Unseen {
    /// Any of the three - a fold takes the name out of the file.
    pub fn reached(&self, name: &str) -> bool {
        self.strings.contains(name) || self.kwargs.contains(name) || self.attrs.contains(name)
    }

    /// A prod string table (`KEYS = ("field",)`, a coverage map's dotted
    /// path) or a test reaching the name off a fixture-loaded module.
    pub fn named(&self, name: &str) -> bool {
        self.tables.contains(name) || self.test_attrs.contains(name)
    }
}

/// The identifiers a string constant spells (`"pkg.mod._f"`), else none.
fn spelled(node: Option<Cn<'_>>) -> Vec<Box<str>> {
    let Some(text) = node.and_then(const_str) else {
        return Vec::new();
    };
    let parts: Vec<&str> = text.split('.').collect();
    if parts.is_empty() || !parts.iter().all(|p| pytext::is_identifier(p)) {
        return Vec::new();
    }
    parts.into_iter().map(Box::from).collect()
}

/// What vulture-grade liveness (#32/#48) and a hoist (#35) must not take on
/// the index's word alone: `patch("pkg.mod._helper")`, `Row(damageshare=x)`,
/// `bench._helper` on a fixture-loaded module.
pub fn unseen_names(facts: &RepoFacts<'_>) -> Unseen {
    let mut out = Unseen::default();
    let resolved: HashSet<(&Qname, NodeIndex)> =
        facts.refs.iter().map(|r| (&r.module, r.node)).collect();
    for module in facts.modules.values() {
        let test = is_test_path(&module.rel);
        for node in module.nodes(&[Kind::Attribute], None, false) {
            let Cn::Expr(Expr::Attribute(a)) = module.nodes[node as usize] else {
                continue;
            };
            out.attrs.insert(a.attr.as_str().into());
            if test && a.ctx == ExprContext::Load && !resolved.contains(&(&module.qname, node)) {
                out.test_attrs.insert(a.attr.as_str().into());
            }
        }
        for node in module.nodes(&[Kind::Call], None, false) {
            let Cn::Expr(Expr::Call(call)) = module.nodes[node as usize] else {
                continue;
            };
            out.kwargs.extend(
                call.arguments
                    .keywords
                    .iter()
                    .filter_map(|kw| kw.arg.as_ref().map(|a| Box::from(a.as_str()))),
            );
            let args = call.arguments.args.iter();
            let values = call.arguments.keywords.iter().map(|kw| &kw.value);
            for arg in args.chain(values) {
                out.strings.extend(spelled(Some(Cn::Expr(arg))));
            }
        }
        if test {
            continue;
        }
        for node in module.nodes(&TABLES, Some(&module.qname), false) {
            let elements: Vec<Option<Cn<'_>>> = match module.nodes[node as usize] {
                Cn::Expr(Expr::Dict(d)) => d
                    .items
                    .iter()
                    .map(|i| i.key.as_ref().map(Cn::Expr))
                    .chain(d.items.iter().map(|i| Some(Cn::Expr(&i.value))))
                    .collect(),
                Cn::Expr(Expr::Tuple(t)) => t.elts.iter().map(|e| Some(Cn::Expr(e))).collect(),
                Cn::Expr(Expr::List(l)) => l.elts.iter().map(|e| Some(Cn::Expr(e))).collect(),
                Cn::Expr(Expr::Set(s)) => s.elts.iter().map(|e| Some(Cn::Expr(e))).collect(),
                _ => continue,
            };
            for element in elements {
                out.tables.extend(spelled(element));
            }
        }
    }
    out
}

fn reflects(module: &Module<'_>, call: &ruff_python_ast::ExprCall) -> bool {
    if matches!(&*call.func, Expr::Name(n) if n.id.as_str() == "vars") {
        return true;
    }
    module
        .dotted_name(&call.func)
        .is_some_and(|d| REFLECTORS.contains(&d.as_str()))
}

/// Name arguments of getattr/hasattr-style calls in the module, read through
/// a local its own scope binds exactly once: `m = '_{0}_{1}'.format(domain,
/// rule)` a statement above `getattr(self, m)` builds the name just as
/// spelling it inline does.
fn dispatch_name_args<'m>(facts: &RepoFacts<'_>, module: &'m Module<'_>) -> Vec<&'m Expr> {
    let mut built: HashMap<(Qname, &str), Option<&Expr>> = HashMap::new();
    for node in module.nodes(&[Kind::Assign], None, false) {
        let Cn::Stmt(Stmt::Assign(asn)) = module.nodes[node as usize] else {
            continue;
        };
        let [Expr::Name(target)] = &asn.targets[..] else {
            continue;
        };
        let key = (facts.enclosing(module, node), target.id.as_str());
        // bound twice: unknowable
        let value = if built.contains_key(&key) {
            None
        } else {
            Some(&*asn.value)
        };
        built.insert(key, value);
    }
    let mut out: Vec<&Expr> = Vec::new();
    for node in module.nodes(&[Kind::Call], None, false) {
        let Cn::Expr(Expr::Call(call)) = module.nodes[node as usize] else {
            continue;
        };
        let named = matches!(&*call.func, Expr::Name(n) if ATTR_FUNCS.contains(&n.id.as_str()));
        if !named || call.arguments.args.len() < 2 {
            continue;
        }
        let arg = &call.arguments.args[1];
        let resolved = match arg {
            Expr::Name(n) => built
                .get(&(facts.enclosing(module, node), n.id.as_str()))
                .copied()
                .flatten(),
            _ => None,
        };
        out.push(resolved.unwrap_or(arg));
    }
    out
}

/// The class body's annotated assignments to a plain name: a record's
/// declared fields, in order.
fn record_fields(cls: &StmtClassDef) -> Vec<(&str, &ruff_python_ast::StmtAnnAssign)> {
    cls.body
        .iter()
        .filter_map(|st| match st {
            Stmt::AnnAssign(a) => match &*a.target {
                Expr::Name(n) => Some((n.id.as_str(), a)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// The class-body fields a positional argument can bind, in order: not a
/// `ClassVar`, not `field(init=False)` / `field(kw_only=True)`, none past the
/// `KW_ONLY` marker.
fn positional_slots(cls: &StmtClassDef) -> Vec<&str> {
    let mut out = Vec::new();
    for (name, st) in record_fields(cls) {
        // `typing.ClassVar[str]`
        let text = unparse::expr(&st.annotation);
        let head = pytext::rpartition(pytext::partition(&text, "[").0, ".")
            .2
            .to_string();
        if head == "KW_ONLY" {
            break;
        }
        let mut init = None;
        let mut kw_only = None;
        if let Some(Expr::Call(c)) = st.value.as_deref() {
            for kw in c.arguments.keywords.iter() {
                let Some(arg) = &kw.arg else { continue };
                let flag = match &kw.value {
                    Expr::BooleanLiteral(b) => Some(Some(b.value)),
                    Expr::StringLiteral(_)
                    | Expr::BytesLiteral(_)
                    | Expr::NumberLiteral(_)
                    | Expr::NoneLiteral(_)
                    | Expr::EllipsisLiteral(_) => Some(None),
                    _ => None,
                };
                match (arg.as_str(), flag) {
                    ("init", Some(v)) => init = Some(v),
                    ("kw_only", Some(v)) => kw_only = Some(v),
                    _ => {}
                }
            }
        }
        if head != "ClassVar" && init != Some(Some(false)) && kw_only != Some(Some(true)) {
            out.push(name);
        }
    }
    out
}

/// (field name, constructing scope) for the class-body fields a construction
/// binds on a class with no own `__init__` (dataclass, NamedTuple, attrs):
/// `Row(1, 2)` reads its first two slots, `Row(b=2)` the field it names - a
/// field every site passes by keyword occurs in no other place, so #32's "its
/// name occurs in no other place" would be false.
fn construction_fields(facts: &RepoFacts<'_>) -> Vec<(Box<str>, Qname)> {
    let mut out = Vec::new();
    for site in &facts.call_sites {
        if site.resolution != Resolution::Resolved {
            continue;
        }
        let Some(info) = site.target.as_ref().and_then(|t| facts.classes.get(t)) else {
            continue;
        };
        if info.methods.contains_key("__init__") {
            continue;
        }
        let holder = &facts.modules[&info.module];
        let Cn::Stmt(Stmt::ClassDef(cls)) = holder.nodes[info.node as usize] else {
            continue;
        };
        let caller = &facts.modules[&site.module];
        let Cn::Expr(Expr::Call(call)) = caller.nodes[site.node as usize] else {
            continue;
        };
        let slots = positional_slots(cls);
        let bound = if call
            .arguments
            .args
            .iter()
            .any(|a| matches!(a, Expr::Starred(_)))
        {
            slots.len()
        } else {
            call.arguments.args.len()
        };
        let declared: HashSet<&str> = record_fields(cls).into_iter().map(|(n, _)| n).collect();
        let positional = slots.iter().take(bound).copied();
        let by_keyword = call
            .arguments
            .keywords
            .iter()
            .filter_map(|kw| kw.arg.as_ref().map(|a| a.as_str()))
            .filter(|a| declared.contains(a));
        for name in positional.chain(by_keyword) {
            out.push((Box::from(name), site.enclosing.clone()));
        }
    }
    out
}

/// Identifiers the repo's own prose spells (`doc_files`: .md/.rst). A tool an
/// index or a README names ships to the readers who run it, so "only tests
/// reach it" (#56) is no evidence nothing ships it.
pub fn documented_names(facts: &RepoFacts<'_>) -> HashSet<Box<str>> {
    facts
        .doc_files
        .values()
        .flatten()
        .flat_map(|line| IDENT_RE.find_iter(line).map(|m| Box::from(m.as_str())))
        .collect()
}

/// Per module, the names its `from M import *` readers load.
pub type Reexports = IndexMap<Qname, BTreeSet<Box<str>>>;

/// Module qname -> the names its star importers load. `from M import *`
/// republishes M's whole public surface: an alias a star importer reads is
/// M's re-export, not M's dead import.
pub fn star_reexports(facts: &RepoFacts<'_>) -> Reexports {
    let mut out: Reexports = IndexMap::new();
    for module in facts.modules.values() {
        for node in module.nodes(&[Kind::ImportFrom], None, false) {
            let Cn::Stmt(Stmt::ImportFrom(n)) = module.nodes[node as usize] else {
                continue;
            };
            if !n.names.iter().any(|a| a.name.as_str() == "*") {
                continue;
            }
            let base = module.rel_import_base(n.level, n.module.as_ref().map(|m| m.as_str()));
            out.entry(Qname::from(base.as_str()))
                .or_default()
                .extend(module_loads(module, (0, 0)));
        }
    }
    out
}

/// Live names by scope, and the live (prefix, suffix) patterns.
pub struct Live {
    pub live: IndexMap<Box<str>, BTreeSet<Qname>>,
    pub patterns: Vec<(String, String)>,
}

/// Vulture semantics: a symbol is dead only when its name never occurs
/// outside its own body (precision over recall). Live names, each with the
/// scopes it occurs in: attribute names, loaded names, `__all__` strings,
/// getattr-string args, the fields a construction binds by slot or keyword,
/// pyproject entry-point objects, their modules and an entry-point class's
/// public methods, inherited ones included (the plugin host's surface, scope
/// ""). Live (prefix, suffix) patterns: dispatch names built around a
/// variable, inline or through a local the scope binds once
/// (`getattr(x, f"validate_{k}")` makes every `validate_*` live); a pattern
/// with no constant text holds no evidence.
pub fn live_names(facts: &RepoFacts<'_>) -> Live {
    let mut live: IndexMap<Box<str>, BTreeSet<Qname>> = IndexMap::new();
    let mut patterns: Vec<(String, String)> = Vec::new();
    let root = Qname::from("");
    let mut add = |name: &str, scope: &Qname| {
        live.entry(Box::from(name))
            .or_default()
            .insert(scope.clone());
    };
    // an installed console script reaches `pkg.cli:main` over a seam no
    // reference in the tree crosses: the module and the object are roots
    for reference in &facts.entry_points {
        let target = pytext::partition(reference, "[").0.replace(':', ".");
        let target = pytext::strip(&target).to_string();
        for part in target.split('.').map(pytext::strip) {
            if pytext::is_identifier(part) {
                add(part, &root);
            }
        }
        for (_, info) in class_walk(facts, &target, Step::Bases) {
            for q in info.methods.values() {
                if let Some(sym) = facts.symbols.get(q)
                    && sym.is_public
                {
                    add(&sym.name, &root);
                }
            }
        }
    }
    for (name, scope) in construction_fields(facts) {
        add(&name, &scope);
    }
    for module in facts.modules.values() {
        for name in module.all_names.iter().flatten() {
            add(name, &module.qname);
        }
        for (scope, buckets) in &module.nodes_by_scope {
            for (kind, nodes) in buckets {
                match kind {
                    Kind::Attribute => {
                        for at in nodes {
                            if let Cn::Expr(Expr::Attribute(a)) = module.nodes[*at as usize] {
                                add(a.attr.as_str(), scope);
                            }
                        }
                    }
                    Kind::Name => {
                        for at in nodes {
                            if let Cn::Expr(Expr::Name(n)) = module.nodes[*at as usize]
                                && n.ctx == ExprContext::Load
                            {
                                add(n.id.as_str(), scope);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        // `-> "list[_T]"`
        for (name, node) in quoted_annotation_names(module) {
            add(&name, &facts.enclosing(module, node));
        }
        let calls = module.nodes(&[Kind::Call], None, false);
        let serializes = calls.iter().any(|at| {
            matches!(module.nodes[*at as usize], Cn::Expr(Expr::Call(c)) if reflects(module, c))
        });
        if serializes {
            // asdict/vars read every field by name: a serialized record's
            // fields are live
            for at in module.nodes(&[Kind::ClassDef], None, false) {
                if let Cn::Stmt(Stmt::ClassDef(cls)) = module.nodes[at as usize] {
                    for (name, _) in record_fields(cls) {
                        add(name, &module.qname);
                    }
                }
            }
        }
        for arg in dispatch_name_args(facts, module) {
            if let Expr::StringLiteral(s) = arg {
                add(s.value.to_str(), &module.qname);
            } else if let Some(pattern) = literal_affixes(arg) {
                patterns.push(pattern);
            }
        }
    }
    Live { live, patterns }
}

/// Every scope outside the symbol's own body where `name` occurs or `qname`
/// is referenced. A self-reference (recursion, a method calling itself on
/// `self`) is not a use; a store rebinds the name, only a load or a call uses
/// it.
fn referencing_scopes(facts: &RepoFacts<'_>, qname: &str, name: &str, live: &Live) -> Vec<Qname> {
    let nested = format!("{qname}.");
    let refs = facts
        .refs_to
        .get(qname)
        .into_iter()
        .flatten()
        .map(|r| &facts.refs[*r as usize])
        .filter(|r| r.kind != RefKind::Store)
        .map(|r| facts.enclosing(&facts.modules[&r.module], r.node));
    let named = live.live.get(name).into_iter().flatten().cloned();
    refs.chain(named)
        .filter(|scope| &**scope != qname && !scope.starts_with(&nested))
        .collect()
}

/// Does `name` occur, or is `qname` referenced, anywhere but inside the
/// symbol's own body? #32's claim: "occurs in no other place" when false.
pub fn referenced_outside(facts: &RepoFacts<'_>, qname: &str, name: &str, live: &Live) -> bool {
    !referencing_scopes(facts, qname, name, live).is_empty()
}

/// The test modules referencing the symbol when they are its only reach
/// (#56's claim: "reached only by tests"); empty where nothing or any prod
/// scope reaches it. A scope is judged by its module's path; an entry-point
/// root (scope "") is the installed distribution's, prod.
pub fn referenced_only_from_tests(
    facts: &RepoFacts<'_>,
    qname: &str,
    name: &str,
    live: &Live,
) -> BTreeSet<Qname> {
    let mut modules: BTreeSet<Qname> = BTreeSet::new();
    for scope in referencing_scopes(facts, qname, name, live) {
        let module = match facts.modules.contains_key(&scope) {
            true => Some(scope.clone()),
            false => facts.symbols.get(&scope).map(|s| s.module.clone()),
        };
        let is_test = module
            .as_ref()
            .and_then(|m| facts.modules.get(m))
            .is_some_and(|m| is_test_path(&m.rel));
        if !is_test {
            return BTreeSet::new();
        }
        modules.insert(module.expect("a test module was found"));
    }
    modules
}

/// `layer_liveness`.
pub fn dump(facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    let found = provers.live(facts);
    let unseen = provers.unseen(facts);
    let mut patterns: Vec<Vec<&str>> = found
        .patterns
        .iter()
        .map(|(prefix, suffix)| vec![prefix.as_str(), suffix.as_str()])
        .collect();
    patterns.sort();
    Some(json!({
        "live": Value::Object(
            found
                .live
                .iter()
                .map(|(name, scopes)| {
                    (name.to_string(), Value::from(scopes.iter().map(|s| &**s).collect::<Vec<_>>()))
                })
                .collect::<Map<String, Value>>(),
        ),
        "patterns": patterns,
        "unseen": json!({
            "strings": sorted(&unseen.strings),
            "kwargs": sorted(&unseen.kwargs),
            "attrs": sorted(&unseen.attrs),
            "tables": sorted(&unseen.tables),
            "test_attrs": sorted(&unseen.test_attrs),
        }),
        "reexports": Value::Object(
            provers
                .reexports(facts)
                .iter()
                .map(|(q, names)| {
                    (q.to_string(), Value::from(names.iter().map(|n| &**n).collect::<Vec<_>>()))
                })
                .collect::<Map<String, Value>>(),
        ),
    }))
}

fn sorted(names: &HashSet<Box<str>>) -> Vec<&str> {
    let mut out: Vec<&str> = names.iter().map(|n| &**n).collect();
    out.sort_unstable();
    out
}
