//! `rs/facts.py` pass A: one module's items, the names it spells bare and
//! the `use` bindings behind them.

use super::*;

/// The bare type an `impl` block is for: through generics, references,
/// slices, arrays and paths to the name a method is keyed under.
fn type_name(node: Option<Node<'_>>, src: &[u8]) -> Option<String> {
    let mut node = node;
    while let Some(n) = node {
        if n.kind() == "type_identifier" || n.kind() == "primitive_type" {
            return Some(text(n, src).into_owned());
        }
        node = n
            .child_by_field_name("type")
            .or_else(|| n.child_by_field_name("element"))
            .or_else(|| n.child_by_field_name("name"));
    }
    None
}

/// Bare `pub`; `pub(crate)` and `pub(super)` are not.
fn is_public(node: Node<'_>, src: &[u8]) -> bool {
    children(node)
        .iter()
        .any(|c| c.kind() == "visibility_modifier" && text(*c, src) == "pub")
}

/// First spelling of a key owns it, so `Type::method` is one symbol however
/// many impl blocks define it; a trait impl only adds its trait.
pub(super) fn add_symbol<'t>(
    symbols: &mut IndexMap<Qname, RsSymbol<'t>>,
    sym: RsSymbol<'t>,
    trait_name: Option<String>,
) {
    let entry = symbols.entry(sym.qname.clone()).or_insert(sym);
    if let Some(name) = trait_name
        && !entry.traits.contains(&name)
    {
        entry.traits.push(name);
    }
}

/// Items of one scope: a `mod` body nests the scope, an `impl` body keys its
/// methods `Type::method`, a `fn` body may hold items of its own. An
/// enclosing `mod`'s cfgs come first in every item's `attrs`.
pub(super) fn index_items<'t>(
    pass: &mut ItemPass<'t>,
    module: &RsModule<'t>,
    block: Node<'t>,
    scope: &str,
    parent: Option<&Qname>,
    in_test: bool,
    inherited: &[String],
) {
    let src = module.bytes;
    for node in named_children(block) {
        let kind = node.kind();
        let mut attrs = inherited.to_vec();
        attrs.extend(attrs_of(node, src));
        let test = in_test || is_test_attr(&attrs);
        if kind == "mod_item" {
            let name = named(node, "name", src).filter(nonempty);
            if let Some(name) = &name
                && is_public(node, src)
                && !test
            {
                pass.pub_mods.insert(format!("{scope}::{name}"));
            }
            match node.child_by_field_name("body") {
                None => {
                    if attrs.iter().any(|a| a.starts_with("path")) {
                        let line = node.start_position().row + 1;
                        pass.notes
                            .push(format!("rs: #[path] not followed: {}:{line}", module.rel));
                    }
                }
                Some(body) => {
                    if let Some(name) = name {
                        index_items(
                            pass,
                            module,
                            body,
                            &format!("{scope}::{name}"),
                            parent,
                            test,
                            &cfgs(&attrs),
                        );
                    }
                }
            }
        } else if kind == "impl_item" {
            index_impl(pass, module, node, scope, test, inherited);
        } else if let Some(sym_kind) = lookup(&ITEM_KINDS, kind)
            && let Some(name) = named(node, "name", src).filter(nonempty)
        {
            let qname: Qname = format!("{scope}::{name}").as_str().into();
            let body = node.child_by_field_name("body");
            let mut is_test = test || is_test_attr(&attrs);
            pass.adds.push((
                RsSymbol {
                    qname: qname.clone(),
                    module: module.qname.clone(),
                    name,
                    kind: sym_kind,
                    node,
                    lineno: node.start_position().row as u32 + 1,
                    end_lineno: node.end_position().row as u32 + 1,
                    is_public: is_public(node, src),
                    parent: parent.cloned(),
                    attrs,
                    traits: Vec::new(),
                    is_test,
                },
                None,
            ));
            match pass.seen.get(&qname) {
                Some(first) => is_test = *first,
                None => {
                    pass.seen.insert(qname.clone(), is_test);
                }
            }
            if kind == "function_item"
                && let Some(body) = body
            {
                index_items(pass, module, body, &qname, Some(&qname), is_test, inherited);
            }
        }
    }
}

fn index_impl<'t>(
    pass: &mut ItemPass<'t>,
    module: &RsModule<'t>,
    node: Node<'t>,
    scope: &str,
    in_test: bool,
    inherited: &[String],
) {
    let src = module.bytes;
    let name = type_name(node.child_by_field_name("type"), src);
    let body = node.child_by_field_name("body");
    let (Some(name), Some(body)) = (name, body) else {
        return;
    };
    let trait_name = type_name(node.child_by_field_name("trait"), src);
    let type_qname = format!("{scope}::{name}");
    pass.impls.push(RsImpl {
        module: module.qname.clone(),
        trait_name: trait_name.clone(),
        type_name: name,
        type_qname: type_qname.clone(),
        lineno: node.start_position().row as u32 + 1,
        node,
    });
    for item in named_children(body) {
        if item.kind() != "function_item" {
            continue;
        }
        let Some(method) = named(item, "name", src).filter(nonempty) else {
            continue;
        };
        let mut attrs = inherited.to_vec();
        attrs.extend(attrs_of(item, src));
        let is_test = in_test || is_test_attr(&attrs);
        pass.adds.push((
            RsSymbol {
                qname: format!("{type_qname}::{method}").as_str().into(),
                module: module.qname.clone(),
                name: method,
                kind: "method",
                node: item,
                lineno: item.start_position().row as u32 + 1,
                end_lineno: item.end_position().row as u32 + 1,
                is_public: is_public(item, src),
                parent: Some(type_qname.as_str().into()),
                attrs,
                traits: Vec::new(),
                is_test,
            },
            trait_name.clone(),
        ));
    }
}

/// The names this module can spell without a path: its own items (a nested
/// one only where the top level leaves the name free, so a `#[cfg(test)]`
/// module calls the module's functions bare) and its child modules, which
/// come from the layout so that `only=` sees the same scope.
pub(super) fn collect_scope(
    facts: &RsFacts<'_>,
    module: &RsModule<'_>,
    module_qnames: &[String],
) -> Scope {
    let top = module.qname.matches("::").count() + 1;
    let mut items: IndexMap<String, Qname> = IndexMap::new();
    for sym in facts.symbols_of(&module.qname) {
        if sym.kind == "method" {
            continue; // never spelled bare: a method call is BY_NAME
        }
        if sym.qname.matches("::").count() == top || !items.contains_key(&sym.name) {
            items.insert(sym.name.clone(), sym.qname.clone());
        }
    }
    let prefix = format!("{}::", module.qname);
    for qname in module_qnames {
        if let Some(tail) = qname.strip_prefix(&prefix)
            && !tail.contains("::")
        {
            items
                .entry(tail.to_string())
                .or_insert_with(|| qname.as_str().into());
        }
    }
    let (bindings, reexports) = collect_bindings(module, &items);
    (items, bindings, reexports)
}

/// `use` declarations, local name to the path they name, `crate`/`self`/
/// `super` resolved against this module; a `pub use` re-exports as well.
fn collect_bindings(
    module: &RsModule<'_>,
    items: &IndexMap<String, Qname>,
) -> (IndexMap<String, String>, Vec<(String, String)>) {
    let src = module.bytes;
    let mut bindings: IndexMap<String, String> = IndexMap::new();
    let mut reexports: Vec<(String, String)> = Vec::new();
    let mut stack = vec![module.root];
    while let Some(node) = stack.pop() {
        // a `use` inside an inline `mod` binds in this file too
        if node.kind() != "use_declaration" {
            stack.extend(named_children(node).into_iter().rev());
            continue;
        }
        let Some(argument) = node.child_by_field_name("argument") else {
            continue;
        };
        let exported = is_public(node, src);
        let mut entries = Vec::new();
        use_entries(argument, "", src, &mut entries);
        for (local, path) in entries {
            let target = absolute(&path, module, items);
            if exported {
                reexports.push((local.clone(), target.clone()));
            }
            if local != "*" {
                bindings.insert(local, target);
            }
        }
    }
    (bindings, reexports)
}

fn use_entries(node: Node<'_>, prefix: &str, src: &[u8], out: &mut Vec<(String, String)>) {
    match node.kind() {
        "scoped_use_list" => {
            let base = join(prefix, &named(node, "path", src).unwrap_or_default());
            if let Some(listed) = node.child_by_field_name("list") {
                for item in named_children(listed) {
                    use_entries(item, &base, src, out);
                }
            }
        }
        "use_list" => {
            for item in named_children(node) {
                use_entries(item, prefix, src, out);
            }
        }
        "use_as_clause" => {
            let alias = named(node, "alias", src).filter(nonempty);
            let path = join(prefix, &named(node, "path", src).unwrap_or_default());
            if let Some(alias) = alias {
                out.push((alias, path));
            }
        }
        "use_wildcard" => {
            let spelled = text(node, src);
            let cut = spelled.strip_suffix('*').unwrap_or(&spelled);
            out.push((
                "*".to_string(),
                join(prefix, pytext::rstrip_chars(cut, ":")),
            ));
        }
        _ => {
            let path = join(prefix, &text(node, src));
            let last = pytext::rpartition(&path, "::").2.to_string();
            out.push((last, path));
        }
    }
}

pub(super) fn join(prefix: &str, rest: &str) -> String {
    if !prefix.is_empty() && !rest.is_empty() {
        return format!("{prefix}::{rest}");
    }
    if prefix.is_empty() {
        rest.to_string()
    } else {
        prefix.to_string()
    }
}

/// `crate::`, `self::` and `super::` against this module's own qname; a head
/// this module's scope already names against that item (Rust 2018's uniform
/// path, `use dns::ToIpAddrs`); anything else is rooted at a crate name.
pub(super) fn absolute(
    path: &str,
    module: &RsModule<'_>,
    items: &IndexMap<String, Qname>,
) -> String {
    let (head, _, rest) = pytext::partition(path, "::");
    if head == "crate" {
        return join(&module.crate_name, rest);
    }
    if head == "self" {
        return join(&module.qname, rest);
    }
    if head == "super" {
        let mut base: &str = &module.qname;
        let mut head = head;
        let mut rest = rest;
        while head == "super" {
            // `base.rsplit("::", 1)[0]`: a base without `::` stops shrinking
            let (before, sep, _) = pytext::rpartition(base, "::");
            base = if sep.is_empty() { base } else { before };
            let (h, _, r) = pytext::partition(rest, "::");
            head = h;
            rest = r;
        }
        return join(base, &join(head, rest));
    }
    match items.get(head) {
        Some(base) => join(base, rest),
        None => path.to_string(),
    }
}
