//! Port of `provers/annotations.py` (codemap 3.3): reading an annotation, the
//! one home for what a repo's annotations say.
//!
//! Two depths, and the difference is a decision. What a type *admits* is read
//! through the repo's own aliases and string forms - `MaybeStr = str | None`
//! admits None wherever it is spelled, so #1's lying default, #3's guard and
//! #33's return contract all see the same annotation. How a boundary *reads*
//! is judged on the annotation as written: an alias is itself the name #1 and
//! #40 ask for, so `Rows = list` is a named boundary type, not a bare one.

use std::collections::BTreeSet;

use indexmap::IndexMap;
use ruff_python_ast::{Expr, Stmt};

use sightline_core::findings::Qname;
use sightline_core::pytext;
use sightline_py_facts::astutil::walk;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::model::RepoFacts;
use sightline_py_facts::order;

/// Module-local name -> qname.
pub type Bindings = IndexMap<Box<str>, Qname>;

/// An alias of an alias of an alias; a cycle stops here too.
const ALIAS_DEPTH: u32 = 3;
const NONE_ADMITTING: [&str; 3] = ["Optional", "Any", "object"];
const BARE_CONTAINERS: [&str; 10] = [
    "dict",
    "Dict",
    "list",
    "List",
    "set",
    "Set",
    "tuple",
    "Tuple",
    "frozenset",
    "FrozenSet",
];
/// Mappings whose value slot an open payload legitimately leaves `Any`.
const ANY_VALUED: [&str; 7] = [
    "dict",
    "Dict",
    "Mapping",
    "MutableMapping",
    "OrderedDict",
    "DefaultDict",
    "defaultdict",
];

/// Every type name the annotation spells, as written.
pub fn annotation_names(ann: &Expr) -> BTreeSet<String> {
    walk(Cn::Expr(ann))
        .filter_map(|n| match n {
            Cn::Expr(Expr::Name(x)) => Some(x.id.to_string()),
            Cn::Expr(Expr::Attribute(a)) => Some(a.attr.to_string()),
            _ => None,
        })
        .collect()
}

/// The annotation and every repo alias it reaches, each with the bindings of
/// the module that wrote it; string forms parsed in place. A generator in
/// Python, a visitor here: a parsed string form owns its tree, so no caller
/// gets to keep the borrow.
pub fn resolve<'f>(
    facts: &'f RepoFacts<'_>,
    bindings: &'f Bindings,
    ann: Option<&Expr>,
    visit: &mut dyn FnMut(&'f Bindings, &Expr),
) {
    resolve_at(facts, bindings, ann, 0, visit);
}

fn resolve_at<'f>(
    facts: &'f RepoFacts<'_>,
    bindings: &'f Bindings,
    ann: Option<&Expr>,
    depth: u32,
    visit: &mut dyn FnMut(&'f Bindings, &Expr),
) {
    let parsed;
    let ann = match ann {
        Some(Expr::StringLiteral(s)) => {
            match ruff_python_parser::parse_expression(s.value.to_str()) {
                Ok(p) => {
                    parsed = p;
                    parsed.expr()
                }
                Err(_) => return,
            }
        }
        Some(other) => other,
        None => return,
    };
    visit(bindings, ann);
    if depth >= ALIAS_DEPTH {
        return;
    }
    for node in walk(Cn::Expr(ann)) {
        if let Cn::Expr(Expr::Name(n)) = node
            && let Some((next, value)) = alias(facts, bindings, n.id.as_str())
        {
            resolve_at(facts, next, Some(value), depth + 1, visit);
        }
    }
}

/// `(bindings, value)` of the repo variable this name binds to -
/// `FnDef = ast.FunctionDef | ast.Lambda` - else `None`.
fn alias<'f>(
    facts: &'f RepoFacts<'_>,
    bindings: &Bindings,
    name: &str,
) -> Option<(&'f Bindings, &'f Expr)> {
    let sym = facts.symbols.get(bindings.get(name)?)?;
    if sym.kind != "variable" {
        return None;
    }
    let module = facts.modules.get(&sym.module)?;
    let value = match module.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::Assign(a)) => Some(&*a.value),
        Cn::Stmt(Stmt::AnnAssign(a)) => a.value.as_deref(),
        Cn::Stmt(Stmt::AugAssign(a)) => Some(&*a.value),
        _ => None,
    }?;
    Some((&module.bindings, value))
}

/// Admits None (`| None`, Optional, Any, object)? Aliases and strings are read.
pub fn none_inclusive(facts: &RepoFacts<'_>, bindings: &Bindings, ann: &Expr) -> bool {
    let mut admits = false;
    resolve(facts, bindings, Some(ann), &mut |_, expr| {
        admits = admits || admits_none(expr);
    });
    admits
}

fn admits_none(expr: &Expr) -> bool {
    walk(Cn::Expr(expr)).any(|n| matches!(n, Cn::Expr(Expr::NoneLiteral(_))))
        || annotation_names(expr)
            .iter()
            .any(|n| NONE_ADMITTING.contains(&n.as_str()))
}

/// #1's boundary verdict, on the annotation as written: an unparameterized
/// container, or an `Any` the annotation did not already place.
pub fn weakness(ann: Option<&Expr>) -> Option<String> {
    let ann = ann?;
    let bare = match ann {
        Expr::Name(n) => Some(n.id.as_str()),
        Expr::Attribute(a) => Some(a.attr.as_str()), // typing.List spelling
        _ => None,
    }
    .filter(|n| BARE_CONTAINERS.contains(n));
    if let Some(name) = bare {
        return Some(format!("bare {}", pytext::lower(name)));
    }
    unplaced_any(ann).then(|| "contains Any".to_string())
}

fn head(node: &Expr) -> &str {
    match node {
        Expr::Name(n) => n.id.as_str(),
        Expr::Attribute(a) => a.attr.as_str(),
        _ => "",
    }
}

/// `Any` spelled where the annotation names no shape around it. A mapping's
/// value slot is not such a place: `dict[str, Any]` is what `json.loads`
/// returns and what an open vendor schema honestly is - the annotation named
/// the keys and left the values open on purpose. Nor is anywhere inside a
/// `Callable`: what crosses a callback is that callable's own contract, which
/// this signature does not get to narrow. `Any` anywhere else (bare, an
/// element type, a type argument) names nothing.
fn unplaced_any(ann: &Expr) -> bool {
    match ann {
        Expr::StringLiteral(s) => ruff_python_parser::parse_expression(s.value.to_str())
            .is_ok_and(|p| unplaced_any(p.expr())),
        Expr::Subscript(sub) => {
            let args: Vec<&Expr> = match &*sub.slice {
                Expr::Tuple(t) => t.elts.iter().collect(),
                other => vec![other],
            };
            let base = head(&sub.value);
            if base == "Callable" {
                return false;
            }
            if ANY_VALUED.contains(&base) && args.len() == 2 {
                // the key slot still counts
                return unplaced_any(args[0]);
            }
            unplaced_any(&sub.value) || args.into_iter().any(unplaced_any)
        }
        Expr::Name(_) | Expr::Attribute(_) => head(ann) == "Any",
        other => {
            let mut kids: Vec<Cn<'_>> = Vec::new();
            order::children(Cn::Expr(other), &mut kids);
            kids.into_iter().any(|c| match c {
                Cn::Expr(e) => unplaced_any(e),
                _ => false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use ruff_python_parser::parse_expression;

    use super::*;

    fn expr(source: &str) -> Expr {
        parse_expression(source)
            .expect("the fixture parses")
            .into_expr()
    }

    #[test]
    fn a_bare_container_and_an_unplaced_any_are_weak() {
        assert_eq!(weakness(Some(&expr("dict"))).as_deref(), Some("bare dict"));
        assert_eq!(
            weakness(Some(&expr("typing.List"))).as_deref(),
            Some("bare list")
        );
        assert_eq!(
            weakness(Some(&expr("list[Any]"))).as_deref(),
            Some("contains Any")
        );
        assert_eq!(weakness(Some(&expr("dict[str, Any]"))), None);
        assert_eq!(
            weakness(Some(&expr("dict[Any, str]"))).as_deref(),
            Some("contains Any")
        );
        assert_eq!(weakness(Some(&expr("Callable[..., Any]"))), None);
        assert_eq!(weakness(Some(&expr("list[int]"))), None);
        assert_eq!(weakness(None), None);
    }

    /// `None` is a `Constant`, never a `Name`: the set holds the names the
    /// annotation spells, and no literal.
    #[test]
    fn annotation_names_reads_every_spelled_name() {
        assert_eq!(
            annotation_names(&expr("dict[str, models.Node | None]")),
            BTreeSet::from([
                "Node".to_string(),
                "dict".to_string(),
                "models".to_string(),
                "str".to_string(),
            ])
        );
    }
}
