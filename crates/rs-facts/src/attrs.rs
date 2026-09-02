//! What an item's `#[...]` run says: the attributes
//! above a node, the test readings among them, and the cfgs a `mod` or a
//! `mod x;` declaration hands what it holds. Facts asks these; no rule does.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use sightline_core::findings::Qname;
use sightline_core::pytext;
use tree_sitter::Node;

use crate::model::{RsFacts, text};
use crate::nodes::{COMMENTS, has, named_children, nonempty};

/// A `cfg` whose arguments hold the bare token `test`: `cfg(test)`,
/// `cfg(all(test, unix))`, `cfg(any(test, feature = "x"))` (R7).
static CFG_TEST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^cfg\([^"]*\btest\b"#).expect("a valid pattern"));

/// The `#[...]` attributes written above this item, inner text only. A
/// comment between an attribute and its item does not end the run.
pub fn attrs_of(node: Node<'_>, src: &[u8]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        if !(p.kind() == "attribute_item" || has(COMMENTS, p.kind())) {
            break;
        }
        if p.kind() == "attribute_item" {
            let raw = text(p, src);
            // `text(prev)[2:-1]`, empty where a broken parse left no `#[..]`
            let inner = raw.get(2..raw.len().saturating_sub(1)).unwrap_or("");
            out.push(pytext::strip(inner).to_string());
        }
        prev = p.prev_named_sibling();
    }
    out.reverse();
    out
}

pub fn is_test_attr<S: AsRef<str>>(attrs: &[S]) -> bool {
    attrs.iter().any(|a| {
        let a = a.as_ref();
        a == "test" || a.ends_with("::test") || CFG_TEST.is_match(a)
    })
}

pub fn named(node: Node<'_>, field: &str, src: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .map(|c| text(c, src).into_owned())
}

/// The `#[cfg]` and `#[cfg_attr]` attributes among these: what a `mod` hands
/// every item inside it, since cargo compiles the module or nothing.
pub fn cfgs<S: AsRef<str>>(attrs: &[S]) -> Vec<String> {
    attrs
        .iter()
        .map(AsRef::as_ref)
        .filter(|a| a.starts_with("cfg(") || a.starts_with("cfg_attr("))
        .map(str::to_string)
        .collect()
}

/// Module qname to the cfgs written on its `mod x;` declaration, so a file
/// module inherits them as an inline body does (a `#[cfg(test)] mod tests;`
/// makes `tests.rs` test code, a `#[cfg(feature)]` one an escape).
pub fn declared_cfgs(facts: &RsFacts<'_>) -> HashMap<Qname, Vec<String>> {
    let mut out: HashMap<Qname, Vec<String>> = HashMap::new();
    for module in facts.modules.values() {
        walk(module.root, &module.qname, &[], module.bytes, &mut out);
    }
    out
}

fn walk(
    block: Node<'_>,
    scope: &str,
    inherited: &[String],
    src: &[u8],
    out: &mut HashMap<Qname, Vec<String>>,
) {
    for node in named_children(block) {
        if node.kind() != "mod_item" {
            continue;
        }
        let Some(name) = named(node, "name", src).filter(nonempty) else {
            continue;
        };
        let mut carried = inherited.to_vec();
        carried.extend(cfgs(&attrs_of(node, src)));
        let inner = format!("{scope}::{name}");
        match node.child_by_field_name("body") {
            None => {
                if !carried.is_empty() {
                    out.insert(inner.as_str().into(), carried);
                }
            }
            Some(body) => walk(body, &inner, &carried, src, out),
        }
    }
}
