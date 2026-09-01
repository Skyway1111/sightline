//! The tree-sitter node vocabulary and the pure node walks the Rust facts
//! and provers share (port of `rs/nodes.py`): kind sets, descent, statement
//! sequences, closure parameters, literal values. Nothing here reads a repo.

use std::collections::HashSet;

use indexmap::IndexMap;
use sightline_core::pytext;
use tree_sitter::Node;

use crate::model::{RsSymbol, is_fn_kind, text};

pub const BLOCKS: &str = "block match_block declaration_list";
pub const SCOPES: &str = "function_item closure_expression";
pub const NESTED_FN: &str = "function_item";
/// descend everything
pub const ALL: &str = "";
pub const ATTRS: &str = "attribute_item inner_attribute_item";
pub const COMMENTS: &str = "line_comment block_comment";
pub const STRINGS: &str = "string_literal raw_string_literal";
pub const LITERALS: &str = "string_literal raw_string_literal integer_literal float_literal \
    char_literal boolean_literal negative_literal";
/// a site under one of these decides; the rest of a body runs whatever
/// happened
pub const GUARDS: &str = "if_expression match_expression match_arm for_expression \
    while_expression loop_expression";
/// every way a name is spelled: a path is the same name with the module a
/// `use` would have taken off, so the blind digest reads both as noise
pub const IDENTS: &str = "identifier type_identifier field_identifier primitive_type \
    shorthand_field_identifier lifetime scoped_identifier scoped_type_identifier";
/// a body built of these alone spells names and the calls it feeds them to
pub const FORWARDS: &str = "identifier scoped_identifier field_identifier field_expression \
    call_expression arguments self";
/// a call handing out the right to write its receiver; a read (`.load`,
/// `.read`, `.get`) hands out none
pub const WRITE_METHODS: &str = "lock write set get_or_init store with_borrow_mut fetch_add \
    fetch_sub swap";
const ASSIGNS: &str = "assignment_expression compound_assignment_expr";
/// `Duration` constructors and the seconds one of their literals stands for
pub const DURATION_SCALE: [(&str, f64); 6] = [
    ("from_secs", 1.0),
    ("from_secs_f32", 1.0),
    ("from_secs_f64", 1.0),
    ("from_millis", 1e-3),
    ("from_micros", 1e-6),
    ("from_nanos", 1e-9),
];

/// `kind in SET`: the tables above are space-separated word lists, so a set
/// is one row of data (`rs/nodes.py`'s frozensets).
pub fn has(table: &str, kind: &str) -> bool {
    table.split(' ').any(|k| k == kind)
}

/// A field query answers an empty name where the node is missing.
pub fn nonempty<S: AsRef<str>>(name: &S) -> bool {
    !name.as_ref().is_empty()
}

/// A node's children, anonymous ones included, in document order.
// sightline-ok: 11 - tree-sitter's two cursor walks, named and all
pub fn children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

// sightline-ok: 11 - tree-sitter's two cursor walks, named and all
pub fn named_children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

pub fn is_fn(sym: &RsSymbol<'_>) -> bool {
    is_fn_kind(sym.kind)
}

pub use sightline_core::clones::digest;

/// A call's argument expressions.
pub fn arg_nodes<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    match node.child_by_field_name("arguments") {
        Some(args) => named_children(args),
        None => Vec::new(),
    }
}

/// The name node a call or a macro invocation names: `check::<Scheme>(..)`
/// hangs its turbofish off the call, so the name is the node under it.
pub fn call_target<'t>(node: Node<'t>, field_name: &str) -> Option<Node<'t>> {
    let target = node.child_by_field_name(field_name)?;
    if target.kind() == "generic_function" {
        return target.child_by_field_name("function");
    }
    Some(target)
}

/// A decimal literal's value, its `_` separators and its type suffix taken
/// off (`1_000u64` is 1000.0); `None` where the node spells no number.
pub fn number(node: Node<'_>, src: &[u8]) -> Option<f64> {
    let raw = text(node, src).replace('_', "");
    let rest = pytext::lstrip_chars(&raw, "0123456789.");
    let digits = &raw[..raw.len() - rest.len()];
    let bare = pytext::strip_chars(digits, ".");
    if bare.is_empty() || digits.matches('.').count() >= 2 {
        return None;
    }
    digits.parse::<f64>().ok()
}

/// A closure's own parameter names, by position: `|a, b|` and `|x, y|`
/// write one shape (#20's key).
pub fn closure_params(node: Node<'_>, src: &[u8]) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    let Some(params) = node.child_by_field_name("parameters") else {
        return out;
    };
    for (i, p) in descend(params, ALL)
        .into_iter()
        .filter(|n| n.kind() == "identifier")
        .enumerate()
    {
        out.insert(text(p, src).into_owned(), format!("p{i}"));
    }
    out
}

/// The type parameters an item declares: `impl<T> Trait for T` targets a
/// name of its own, not a type anyone wrote.
pub fn type_params(node: Node<'_>, src: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let Some(params) = node.child_by_field_name("type_parameters") else {
        return out;
    };
    for child in named_children(params) {
        let head = if child.kind() == "type_identifier" {
            Some(child)
        } else {
            named_children(child)
                .into_iter()
                .find(|c| c.kind() == "type_identifier")
        };
        if let Some(head) = head {
            out.push(text(head, src).into_owned());
        }
    }
    out
}

/// The type arguments a `Foo<Bar>` or `f::<Bar>()` names, lifetimes left
/// out; `None` where the node names none (#37's monomorphic arm).
pub fn type_args(node: Node<'_>, src: &[u8]) -> Option<Vec<String>> {
    let args = node.child_by_field_name("type_arguments")?;
    Some(
        named_children(args)
            .into_iter()
            .filter(|c| c.kind() != "lifetime")
            .map(|c| text(c, src).into_owned())
            .collect(),
    )
}

/// Every type parameter the items enclosing this node declare: a `Foo<T>`
/// written under `impl<T>` or in a generic signature names a parameter,
/// which is the declaration side and no instantiation at all.
pub fn params_in_scope(node: Node<'_>, src: &[u8]) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut cur = Some(node);
    while let Some(n) = cur {
        out.extend(type_params(n, src));
        cur = n.parent();
    }
    out
}

/// Is a `///` run written above this item? It sits above the item's
/// attributes, which is where an attributed `fn`'s doc is written (#59).
pub fn item_doc(node: Node<'_>) -> bool {
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        if !(has(ATTRS, p.kind()) || has(COMMENTS, p.kind())) {
            return false;
        }
        if children(p)
            .iter()
            .any(|c| c.kind() == "outer_doc_comment_marker")
        {
            return true;
        }
        prev = p.prev_named_sibling();
    }
    false
}

/// Every `match` of this body whose arms each return what they matched. A
/// verbatim re-wrap only compiles where both sides are the one type, so such
/// a `match` is the identity however its scrutinee is spelled.
pub fn identity_matches<'t>(func: Node<'t>, src: &[u8]) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    for node in descend(func, NESTED_FN) {
        if node.kind() != "match_expression" {
            continue;
        }
        let arms: Vec<Node<'t>> = match node.child_by_field_name("body") {
            Some(block) => named_children(block)
                .into_iter()
                .filter(|c| c.kind() == "match_arm")
                .collect(),
            None => Vec::new(),
        };
        if !arms.is_empty() && arms.iter().all(|a| returns_its_pattern(*a, src)) {
            out.push(node);
        }
    }
    out
}

/// `Err(e) => Err(e)`. An arm's guard is part of the pattern node, so a
/// guarded arm never spells its pattern back.
fn returns_its_pattern(arm: Node<'_>, src: &[u8]) -> bool {
    let pattern = arm.child_by_field_name("pattern");
    let mut value = arm.child_by_field_name("value");
    if let Some(v) = value
        && v.kind() == "block"
    {
        let inner = statements(v);
        value = if inner.len() == 1 {
            Some(inner[0])
        } else {
            None
        };
    }
    let (Some(pattern), Some(value)) = (pattern, value) else {
        return false;
    };
    let spelled = flat(value, src);
    flat(pattern, src) == pytext::strip(pytext::removeprefix(&spelled, "return "))
}

fn flat(node: Node<'_>, src: &[u8]) -> String {
    pytext::split(&text(node, src)).join(" ")
}

/// Is a closure body only names and the calls it feeds them to, with no
/// literal, operator, cast, index or branch? Then nothing in it decides
/// anything, so a second copy names no new fact (#20).
pub fn forwards_only(body: Node<'_>) -> bool {
    if body.kind() == "block" {
        let inner = statements(body);
        return inner.len() == 1 && forwards_only(inner[0]);
    }
    has(FORWARDS, body.kind()) && descend(body, ALL).iter().all(|n| has(FORWARDS, n.kind()))
}

/// `#[allow(dead_code, unused)]` to the names it silences.
pub fn allow_names(node: Node<'_>, src: &[u8]) -> Vec<String> {
    let raw = text(node, src);
    let attr = pytext::strip(pytext::strip_chars(pytext::strip_chars(&raw, "#!"), "[]"));
    if !attr.starts_with("allow(") || !attr.ends_with(')') {
        return Vec::new();
    }
    attr[6..attr.len() - 1]
        .split(',')
        .map(pytext::strip)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// The names this body writes: the root of an assignment target, and the
/// receiver of a call that hands out the right to write it (#9). A nested
/// `fn` writes on its own account.
pub fn written_names(func: Node<'_>, src: &[u8]) -> HashSet<String> {
    let mut out = HashSet::new();
    for node in descend(func, NESTED_FN) {
        if has(ASSIGNS, node.kind()) {
            out.extend(root_name(node.child_by_field_name("left"), src));
        } else if node.kind() == "call_expression" {
            let Some(target) = node.child_by_field_name("function") else {
                continue;
            };
            if target.kind() != "field_expression" {
                continue;
            }
            if let Some(field) = target.child_by_field_name("field")
                && has(WRITE_METHODS, &text(field, src))
            {
                out.extend(root_name(target.child_by_field_name("value"), src));
            }
        }
    }
    out
}

/// The module-level name an expression spells: an identifier, or a path's
/// last segment. A field or an index reaches into something else.
fn root_name(node: Option<Node<'_>>, src: &[u8]) -> Option<String> {
    let node = node?;
    match node.kind() {
        "identifier" => Some(text(node, src).into_owned()),
        "scoped_identifier" => Some(pytext::rpartition(&text(node, src), "::").2.to_string()),
        _ => None,
    }
}

/// Every node enclosing this one inside its `fn`, innermost first: what a
/// site sits under is what decides how it reads.
pub fn ancestors<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut cur = node.parent();
    while let Some(n) = cur {
        if n.kind() == "function_item" {
            break;
        }
        out.push(n);
        cur = n.parent();
    }
    out
}

/// Every named descendant of `root`, document order, not entering a node of
/// a `stop` kind (but yielding it).
pub fn descend<'t>(root: Node<'t>, stop: &str) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let is_root = node.id() == root.id();
        if !is_root {
            out.push(node);
        }
        if is_root || !has(stop, node.kind()) {
            let kids = named_children(node);
            stack.extend(kids.into_iter().rev());
        }
    }
    out
}

pub fn statements<'t>(block: Node<'t>) -> Vec<Node<'t>> {
    named_children(block)
        .into_iter()
        .filter(|c| !has(COMMENTS, c.kind()))
        .collect()
}

/// (statements, is-the-whole-body) for the `fn`'s own scope: its body and
/// every nested block. A nested `fn` is its own sequence, not part of this
/// one.
pub fn own_sequences<'t>(func: Node<'t>) -> Vec<(Vec<Node<'t>>, bool)> {
    let mut out = Vec::new();
    let mut queue: std::collections::VecDeque<(Node<'t>, bool)> =
        match func.child_by_field_name("body") {
            Some(body) => std::collections::VecDeque::from([(body, true)]),
            None => std::collections::VecDeque::new(),
        };
    while let Some((block, top)) = queue.pop_front() {
        let stmts = statements(block);
        for stmt in &stmts {
            queue.extend(inner_blocks(*stmt).into_iter().map(|b| (b, false)));
        }
        out.push((stmts, top));
    }
    out
}

/// The blocks one level in from this statement: descend until a block or a
/// scope of its own. A statement that is itself a scope, a nested `fn`
/// item, owns its body, so this scope never counts those statements too.
pub fn inner_blocks<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack: Vec<Node<'t>> = if has(SCOPES, node.kind()) {
        Vec::new()
    } else {
        named_children(node).into_iter().rev().collect()
    };
    while let Some(cur) = stack.pop() {
        if has(BLOCKS, cur.kind()) {
            out.push(cur);
        } else if !has(SCOPES, cur.kind()) {
            stack.extend(named_children(cur).into_iter().rev());
        }
    }
    out
}
