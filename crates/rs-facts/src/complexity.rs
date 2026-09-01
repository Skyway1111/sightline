//! The Rust cognitive-complexity classification (`rs/model.py`, `_NESTERS`
//! down to `cognitive_complexity`): `if`/`match`/loops nest, a run of like
//! boolean operators is one flat increment, a direct recursive call another.
//! A nested `fn` scores on its own; a closure sinks its body without scoring.

use sightline_core::complexity::{Cc, score};
use tree_sitter::Node;

use crate::model::text;
use crate::nodes::{children, has};

const NESTERS: &str = "if_expression match_expression for_expression while_expression \
    loop_expression";
const INNER_FIELDS: &str = "consequence alternative body";
const BOOL_OPS: &str = "&& ||";

fn operator(node: Node<'_>, src: &[u8]) -> String {
    match node.child_by_field_name("operator") {
        Some(op) => text(op, src).into_owned(),
        None => String::new(),
    }
}

/// `name(...)`, `Self::name(...)` or `self.name(...)`: a direct recursive call.
fn recurses(node: Node<'_>, name: Option<&str>, src: &[u8]) -> bool {
    let Some(fn_node) = node.child_by_field_name("function") else {
        return false;
    };
    let spelled = text(fn_node, src).replace('.', "::");
    let last = spelled.rsplit("::").next().unwrap_or("").to_string();
    name.is_some_and(|n| last == n)
}

/// An `else_clause`: a lone `if` there is an `else if` (+1 flat, its own
/// visit); anything else is an `else` (+1, nested one deeper).
fn else_clause(node: Node<'_>, name: Option<&str>, src: &[u8]) -> Cc {
    let kids = crate::nodes::named_children(node);
    if kids.len() == 1 && kids[0].kind() == "if_expression" {
        return classify(kids[0], name, false, true, "", src);
    }
    Cc {
        flat: 1,
        nests: false,
        inner: false,
        kids: kids
            .into_iter()
            .map(|k| classify(k, name, true, false, "", src))
            .collect(),
    }
}

fn classify(
    node: Node<'_>,
    name: Option<&str>,
    inner: bool,
    is_elif: bool,
    op: &str,
    src: &[u8],
) -> Cc {
    let kind = node.kind();
    if kind == "function_item" {
        return Cc::default(); // its own finding, and not twice
    }
    let nester = has(NESTERS, kind);
    let mut flat = 0;
    let mut op = op.to_string();
    if nester {
        flat = u32::from(is_elif);
    } else if kind == "binary_expression" {
        let was = op;
        op = operator(node, src);
        flat = u32::from(has(BOOL_OPS, &op) && op != was);
    } else if kind == "call_expression" {
        flat = u32::from(recurses(node, name, src));
    }
    if kind != "binary_expression" {
        op.clear();
    }
    let sink = nester || kind == "closure_expression";
    let name = if kind == "closure_expression" {
        None
    } else {
        name
    };
    let mut kids: Vec<Cc> = Vec::new();
    for (i, child) in children(node).into_iter().enumerate() {
        if !child.is_named() {
            continue;
        }
        let field = node.field_name_for_child(i as u32);
        let found = if nester && field == Some("alternative") && child.kind() == "else_clause" {
            else_clause(child, name, src)
        } else {
            let inner = sink && field.is_some_and(|f| has(INNER_FIELDS, f));
            classify(child, name, inner, false, &op, src)
        };
        // a subtree scoring nothing is not in the tree: the score reads the
        // decisions, not the source
        if found.flat > 0 || found.nests || !found.kids.is_empty() {
            kids.push(found);
        }
    }
    Cc {
        flat,
        nests: nester && !is_elif,
        inner,
        kids,
    }
}

/// SonarSource cognitive complexity of a `fn` body; `nesting` prices the
/// body as if it sat that deep.
pub fn cognitive_complexity(node: Node<'_>, nesting: u32, src: &[u8]) -> u32 {
    let Some(body) = node.child_by_field_name("body") else {
        return 0;
    };
    let name = node
        .child_by_field_name("name")
        .map(|n| text(n, src).into_owned());
    let roots: Vec<Cc> = crate::nodes::named_children(body)
        .into_iter()
        .map(|k| classify(k, name.as_deref(), false, false, "", src))
        .collect();
    score(&roots, nesting)
}
