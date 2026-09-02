//! Pass B of `build_facts`: one walk per module for its comments, refs
//! and call sites, resolved against the table pass A built.

use super::*;

const NAMES: &str = "identifier type_identifier";
const PATHS: &str = "scoped_identifier scoped_type_identifier";

const STORE_FIELDS: [(&str, &str); 4] = [
    ("assignment_expression", "left"),
    ("compound_assignment_expr", "left"),
    ("let_declaration", "pattern"),
    ("for_expression", "pattern"),
];

pub(super) struct WalkOut<'t> {
    pub(super) refs: Vec<RsRef<'t>>,
    pub(super) call_sites: Vec<RsCallSite<'t>>,
    pub(super) comments: Vec<RsComment>,
}

/// One walk: comments, name-level refs and call sites. A path is one ref,
/// and the scope is the innermost symbol whose node holds the site.
pub(super) fn walk_module<'t>(facts: &RsFacts<'t>, module: &RsModule<'t>) -> WalkOut<'t> {
    let src = module.bytes;
    let owners: HashMap<usize, Qname> = facts
        .symbols_of(&module.qname)
        .map(|s| (s.node.id(), s.qname.clone()))
        .collect();
    let mut out = WalkOut {
        refs: Vec::new(),
        call_sites: Vec::new(),
        comments: Vec::new(),
    };
    let mut stack: Vec<(Node<'t>, Qname, RefKind)> =
        vec![(module.root, module.qname.clone(), RefKind::Load)];
    while let Some((node, scope, ctx)) = stack.pop() {
        let kind = node.kind();
        let scope = owners.get(&node.id()).cloned().unwrap_or(scope);
        if has(COMMENTS, kind) {
            out.comments.push(comment_of(node, src));
            continue;
        }
        if kind == "use_declaration" {
            continue; // a binding, not a read of what it names
        }
        if has(NAMES, kind) || has(PATHS, kind) {
            add_ref(&mut out, facts, module, Some(node), ctx);
            continue;
        }
        // the macro's own name is its ref, not a second read of it
        let mut own_name: Option<usize> = None;
        if kind == "call_expression" {
            add_call(&mut out, facts, module, node, &scope);
        } else if kind == "macro_invocation" {
            let macro_node = node.child_by_field_name("macro");
            add_ref(&mut out, facts, module, macro_node, RefKind::Callee);
            own_name = macro_node.map(|m| m.id());
        }
        for (i, child) in children(node).into_iter().enumerate().rev() {
            if child.is_named() && Some(child.id()) != own_name {
                let field = node.field_name_for_child(i as u32);
                stack.push((child, scope.clone(), context_of(node, field, ctx)));
            }
        }
    }
    out.comments.sort_by_key(|c| c.line);
    out
}

fn context_of(node: Node<'_>, field: Option<&str>, ctx: RefKind) -> RefKind {
    let kind = node.kind();
    if let Some(field) = field
        && STORE_FIELDS.contains(&(kind, field))
    {
        return RefKind::Store;
    }
    if kind == "call_expression" {
        return if field == Some("function") {
            RefKind::Callee
        } else {
            RefKind::Load
        };
    }
    if kind == "generic_function" {
        ctx
    } else {
        RefKind::Load
    }
}

/// `///` documents the item below, `//!` the module; everything else is a
/// comment. A doc's text is what is left once the three-character marker
/// goes, so a leading slash or star is content.
fn comment_of(node: Node<'_>, src: &[u8]) -> RsComment {
    let kinds: Vec<&str> = children(node).iter().map(|c| c.kind()).collect();
    let doc = if kinds.contains(&"inner_doc_comment_marker") {
        "module-doc"
    } else if kinds.contains(&"outer_doc_comment_marker") {
        "doc"
    } else {
        ""
    };
    let raw = text(node, src);
    let body = pytext::rstrip_chars(&raw, "\r\n");
    let body = if doc.is_empty() {
        body.to_string()
    } else {
        let trimmed = if body.starts_with("/*") {
            body.strip_suffix("*/").unwrap_or(body)
        } else {
            body
        };
        pytext::strip(trimmed.get(3..).unwrap_or("")).to_string()
    };
    RsComment {
        line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        text: body,
        kind: if doc.is_empty() { "comment" } else { doc },
    }
}

fn add_ref<'t>(
    out: &mut WalkOut<'t>,
    facts: &RsFacts<'t>,
    module: &RsModule<'t>,
    node: Option<Node<'t>>,
    kind: RefKind,
) {
    let Some(node) = node else {
        return;
    };
    out.refs.push(RsRef {
        module: module.qname.clone(),
        node,
        target: target_of(facts, &text(node, module.bytes), module),
        kind,
        lineno: node.start_position().row as u32 + 1,
    });
}

/// A name or path through this module's bindings and items, then through the
/// aliases to the definition; the spelling where none of them knows it.
fn target_of(facts: &RsFacts<'_>, spelled: &str, module: &RsModule<'_>) -> String {
    let (head, _, rest) = pytext::partition(spelled, "::");
    let base = module
        .bindings
        .get(head)
        .map(String::as_str)
        .or_else(|| module.items.get(head).map(|q| &**q));
    let path = match base {
        Some(base) => join(base, rest),
        None if head == "crate" || head == "self" || head == "super" => {
            absolute(spelled, module, &module.items)
        }
        None => spelled.to_string(),
    };
    follow(&facts.aliases, &path)
}

/// EXTERNAL needs evidence that the name lives outside: the prelude, or a
/// `use` that says where it came from. A path alone is no evidence, and a
/// glob's name leaves UNRESOLVED: the walk never saw what it bound.
fn resolution_of(
    facts: &RsFacts<'_>,
    module: &RsModule<'_>,
    spelled: &str,
    target: &str,
) -> Resolution {
    if facts.symbols.contains_key(target) || facts.modules.contains_key(target) {
        return Resolution::Resolved;
    }
    if facts.crates.contains_key(pytext::partition(target, "::").0) {
        return Resolution::Unresolved; // names this repo, but no item answers
    }
    let head = pytext::partition(spelled, "::").0;
    if in_prelude(head) || module.bindings.contains_key(head) {
        Resolution::External
    } else {
        Resolution::Unresolved
    }
}

/// A path call resolves through the bindings; a method call on a plain
/// receiver is BY_NAME over every same-named method, and UNRESOLVED with
/// none, since a receiver type no walk knows is not evidence.
fn add_call<'t>(
    out: &mut WalkOut<'t>,
    facts: &RsFacts<'t>,
    module: &RsModule<'t>,
    node: Node<'t>,
    scope: &Qname,
) {
    let Some(mut fn_node) = node.child_by_field_name("function") else {
        return;
    };
    if fn_node.kind() == "generic_function" {
        fn_node = fn_node.child_by_field_name("function").unwrap_or(fn_node);
    }
    let src = module.bytes;
    let mut target: Option<String> = None;
    let how = if fn_node.kind() == "field_expression" {
        let name = named(fn_node, "field", src).unwrap_or_default();
        let empty: Vec<Qname> = Vec::new();
        let candidates = facts.methods_by_name.get(&name).unwrap_or(&empty);
        if candidates.len() == 1 {
            target = Some(candidates[0].to_string());
        }
        if candidates.is_empty() {
            Resolution::Unresolved
        } else {
            Resolution::ByName
        }
    } else if has(NAMES, fn_node.kind()) || has(PATHS, fn_node.kind()) {
        let spelled = text(fn_node, src).into_owned();
        let found = target_of(facts, &spelled, module);
        let how = resolution_of(facts, module, &spelled, &found);
        target = Some(found);
        how
    } else {
        Resolution::Unresolved
    };
    out.call_sites.push(RsCallSite {
        module: module.qname.clone(),
        node,
        enclosing: scope.clone(),
        resolution: how,
        target,
        lineno: node.start_position().row as u32 + 1,
    });
}
