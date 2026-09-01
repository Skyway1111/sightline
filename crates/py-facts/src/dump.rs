//! The `debug dump` layers this stack answers: `listing`, `facts` and
//! `traversal`.

use std::collections::BTreeSet;

use ruff_python_ast::{Parameter, Stmt};
use serde_json::{Map, Value, json};

use sightline_core::pyjson::object;

use crate::cn::Cn;
use crate::model::{FUNCTION_KINDS, NodeIndex, RepoFacts, Span};
use crate::module::Module;
use crate::qnames::under;
use crate::unparse;

/// The walk every language shares, and the repo-wide inputs the Python
/// build reads off it.
pub fn listing(facts: &RepoFacts<'_>) -> Value {
    let mut published: Vec<&str> = facts.published.iter().map(|q| &**q).collect();
    published.sort_unstable();
    json!({
        "files": facts.all_files.iter().map(|r| &**r).collect::<Vec<_>>(),
        "import_roots": facts.import_roots.iter().map(|p| under(&facts.root, p)).collect::<Vec<_>>(),
        "entry_points": facts.entry_points,
        "typed_scope": facts.typed_scope,
        "published": published,
    })
}

pub fn facts(facts: &RepoFacts<'_>) -> Value {
    let modules: Vec<Value> = facts
        .modules
        .values()
        .map(|m| {
            json!({
                "qname": &*m.qname,
                "rel": &*m.rel,
                "lines": m.lines.len(),
                "lossy": m.lossy,
                "all_names": m.all_names.as_ref().map(|names| {
                    names.iter().map(|n| &**n).collect::<Vec<_>>()
                }),
                "dynamic_all": m.dynamic_all,
                "bindings": object(
                    m.bindings.iter().map(|(k, v)| (k.to_string(), Value::from(&**v))),
                ),
            })
        })
        .collect();
    let symbols: Vec<Value> = facts
        .symbols
        .values()
        .map(|s| {
            json!({
                "qname": &*s.qname,
                "module": &*s.module,
                "name": &*s.name,
                "kind": s.kind,
                "lineno": s.lineno,
                "end_lineno": s.end_lineno,
                "is_public": s.is_public,
                "parent": s.parent.as_deref(),
            })
        })
        .collect();
    let classes: Vec<Value> = facts
        .classes
        .values()
        .map(|c| {
            json!({
                "qname": &*c.qname,
                "bases": c.bases.iter().map(|b| &**b).collect::<Vec<_>>(),
                "external_bases": c.external_bases,
                "methods": object(
                    c.methods.iter().map(|(k, v)| (k.to_string(), Value::from(&**v))),
                ),
                "subclasses": c.subclasses.iter().map(|s| &**s).collect::<Vec<_>>(),
            })
        })
        .collect();
    let refs: Vec<Value> = facts
        .refs
        .iter()
        .map(|r| {
            let span = span_of(&facts.modules[&r.module], r.node);
            json!({
                "module": &*r.module,
                "target": &*r.target,
                "kind": r.kind.value(),
                "line": span[0],
                "col": span[1],
            })
        })
        .collect();
    let call_sites: Vec<Value> = facts
        .call_sites
        .iter()
        .map(|c| {
            let span = span_of(&facts.modules[&c.module], c.node);
            json!({
                "module": &*c.module,
                "line": span[0],
                "col": span[1],
                "end_line": span[2],
                "end_col": span[3],
                "enclosing": &*c.enclosing,
                "resolution": c.resolution.value(),
                "target": c.target.as_deref(),
                "candidates": c.candidates.iter().map(|q| &**q).collect::<Vec<_>>(),
            })
        })
        .collect();
    let signatures = object(
        facts
            .symbols
            .iter()
            .filter(|(_, s)| FUNCTION_KINDS.contains(&s.kind))
            .map(|(q, s)| {
                let m = &facts.modules[&s.module];
                (
                    q.to_string(),
                    json!({
                        "params": params(m, s.node),
                        "returns": m.returns(s.node).map(unparse::expr),
                    }),
                )
            }),
    );
    json!({
        "modules": modules,
        "symbols": symbols,
        "classes": classes,
        "refs": refs,
        "call_sites": call_sites,
        "signatures": signatures,
    })
}

fn span_of(module: &Module<'_>, node: NodeIndex) -> Span {
    module.span(node).unwrap_or_default()
}

/// Every parameter with the annotation facts lifted onto it (R15), in
/// signature order.
fn params(module: &Module<'_>, def: NodeIndex) -> Vec<Value> {
    let Cn::Stmt(Stmt::FunctionDef(node)) = module.nodes[def as usize] else {
        return Vec::new();
    };
    let p = &node.parameters;
    let slots: Vec<&Parameter> = p
        .posonlyargs
        .iter()
        .chain(p.args.iter())
        .map(|a| &a.parameter)
        .chain(p.vararg.as_deref())
        .chain(p.kwonlyargs.iter().map(|a| &a.parameter))
        .chain(p.kwarg.as_deref())
        .collect();
    slots
        .into_iter()
        .map(|slot| {
            let annotation = Cn::Param(slot)
                .stamped()
                .and_then(|i| module.annotation(i))
                .map(unparse::expr);
            json!([slot.name.as_str(), annotation])
        })
        .collect()
}

/// `nodes_by_scope` per module: scopes in first-visit order, per scope the
/// positioned nodes of each class in traversal order. `fields` is CPython's
/// child order per class seen.
pub fn traversal(facts: &RepoFacts<'_>) -> Value {
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    let mut nodes = 0usize;
    let mut modules: Vec<Value> = Vec::with_capacity(facts.modules.len());
    for m in facts.modules.values() {
        let mut scopes: Vec<Value> = Vec::with_capacity(m.nodes_by_scope.len());
        for (scope, buckets) in &m.nodes_by_scope {
            let mut kinds = Map::new();
            for (kind, list) in buckets {
                if !kind.positioned() || list.is_empty() {
                    continue;
                }
                let spans: Vec<Value> = list
                    .iter()
                    .map(|i| Value::from(m.spans[*i as usize].unwrap_or_default().to_vec()))
                    .collect();
                nodes += spans.len();
                seen.insert(kind.name());
                kinds.insert(kind.name().to_string(), Value::Array(spans));
            }
            scopes.push(json!({"scope": &**scope, "kinds": kinds}));
        }
        modules.push(json!({"rel": &*m.rel, "qname": &*m.qname, "scopes": scopes}));
    }
    let fields = object(seen.iter().map(|name| {
        let kind = crate::kinds::Kind::from_name(name).expect("a class the traversal emitted");
        (name.to_string(), Value::from(kind.fields().to_vec()))
    }));
    json!({"fields": fields, "modules": modules, "nodes": nodes})
}
