//! #21, an invariant a panic arm enforces (`rs/rules/surface.py` L139-274).

use std::collections::HashMap;

use sightline_core::findings::{Evidence, Finding, Qname, Sink};
use sightline_core::pytext;
use sightline_rs_facts::Node;
use sightline_rs_facts::model::{RsFacts, RsSymbol, text};
use sightline_rs_facts::nodes::{ALL, descend, has, is_fn, named_children, statements};
use sightline_rs_provers::RsProvers;

use crate::util::site;

/// `todo!` and `unimplemented!` mark work not done: the fix there is to write
/// the arm, not to narrow the type (turmoil's five SOCK_SEQPACKET stubs).
const PANICS: &str = "unreachable panic";
/// a path plus fields
const HEADED: &str = "tuple_struct_pattern struct_pattern";
const PATHS: &str = "identifier scoped_identifier type_identifier scoped_type_identifier";
/// the enum is not spelled there: read the variant alone
const IMPLEMENTOR: &str = "Self";
/// such an enum makes a fallback arm the caller's duty
const OPEN: &str = "non_exhaustive";

/// Variant name -> the qnames of the repo enums declaring it, read off the
/// enum items once. A `#[non_exhaustive]` enum stays out: a downstream
/// `match` on one must write a fallback arm, so the panic there is the
/// language's shape and not a choice the author made.
fn variants(facts: &RsFacts<'_>) -> HashMap<String, Vec<Qname>> {
    let mut out: HashMap<String, Vec<Qname>> = HashMap::new();
    for (qname, sym) in &facts.symbols {
        let body = match sym.kind {
            "enum" => sym.node.child_by_field_name("body"),
            _ => None,
        };
        let Some(body) = body.filter(|_| !sym.attrs.iter().any(|a| a == OPEN)) else {
            continue;
        };
        let src = facts.modules[&sym.module].bytes;
        for variant in named_children(body) {
            let name = variant.child_by_field_name("name");
            if let (Some(name), "enum_variant") = (name, variant.kind()) {
                out.entry(text(name, src).into_owned())
                    .or_default()
                    .push(qname.clone());
            }
        }
    }
    for owners in out.values_mut() {
        owners.sort();
    }
    out
}

/// The repo enum and variant a pattern's path names: its last two segments
/// where it spells the enum, else the one repo enum owning the bare variant
/// name. A path with no crate prefix resolves inside its own crate, so where
/// several crates declare the name the arm's crate decides. `None` where no
/// repo enum owns the name (a foreign one, or a binding whose name reads as
/// a path) or where the crate leaves it open.
fn enum_of<'a>(
    variants: &'a HashMap<String, Vec<Qname>>,
    path: &'a str,
    krate: &str,
) -> Option<(&'a str, &'a str)> {
    let (spelled, _, variant) = pytext::rpartition(path, "::");
    let mut owners: Vec<&str> = match variants.get(variant) {
        Some(qs) => qs.iter().map(|q| &**q).collect(),
        None => Vec::new(),
    };
    let head = pytext::rpartition(spelled, "::").2;
    if !head.is_empty() && head != IMPLEMENTOR {
        owners.retain(|q| pytext::rpartition(q, "::").2 == head);
    }
    if owners.len() > 1 {
        owners.retain(|q| q.split("::").next().unwrap_or_default() == krate);
    }
    match owners[..] {
        [one] => Some((one, variant)),
        _ => None,
    }
}

/// Is the arm's whole body one panicking macro call? Braces around that one
/// call say the same thing.
fn panic_body(arm: Node<'_>, src: &[u8]) -> bool {
    let mut value = arm.child_by_field_name("value");
    if let Some(block) = value.filter(|v| v.kind() == "block") {
        let inner = statements(block);
        value = match inner[..] {
            [only] => Some(only),
            _ => None,
        };
    }
    if let Some(stmt) = value.filter(|v| v.kind() == "expression_statement") {
        value = match named_children(stmt)[..] {
            [only] => Some(only),
            _ => None,
        };
    }
    let Some(call) = value.filter(|v| v.kind() == "macro_invocation") else {
        return false;
    };
    call.child_by_field_name("macro")
        .is_some_and(|m| has(PANICS, &text(m, src)))
}

/// The path an arm's pattern names, else `None`. A wildcard names no variant
/// (and often stands for a foreign enum's open set), a guard makes the panic
/// the condition's business rather than the variant's, and an `A | B` arm
/// names a set that no one narrowing of the scrutinee removes.
fn pattern_path(arm: Node<'_>, src: &[u8]) -> Option<String> {
    let pattern = arm.child_by_field_name("pattern")?;
    if pattern.child_by_field_name("condition").is_some() {
        return None;
    }
    let mut node = named_children(pattern).first().copied();
    if let Some(headed) = node.filter(|n| has(HEADED, n.kind())) {
        node = headed.child_by_field_name("type");
    }
    node.filter(|n| has(PATHS, n.kind()))
        .map(|n| text(n, src).into_owned())
}

/// The Rust reading of the sibling's shape: the invariant lives in a `match`
/// arm instead of in the type, and every other reader of that enum has to
/// hold it too. A wildcard arm is not this one, an arm over a foreign or
/// `#[non_exhaustive]` enum names a set the repo does not close, and test
/// code panics for a living. Salience is how many panic arms the enum draws
/// repo-wide, so the enum four arms exclude a variant of ranks above the one
/// a single arm does.
pub(super) fn rule_21(facts: &RsFacts<'_>, _provers: &RsProvers<'_>, out: &mut Sink) {
    let variants = variants(facts);
    let mut order: Vec<&Qname> = facts.symbols.keys().collect();
    order.sort();
    let mut found: Vec<(&RsSymbol<'_>, Node<'_>, String, String)> = Vec::new();
    for qname in order {
        let sym = &facts.symbols[qname];
        let body = if is_fn(sym) {
            sym.node.child_by_field_name("body")
        } else {
            None
        };
        let Some(body) = body.filter(|_| !sym.is_test) else {
            continue;
        };
        let src = facts.modules[&sym.module].bytes;
        let krate = qname.split("::").next().unwrap_or_default();
        for arm in descend(body, ALL) {
            if arm.kind() != "match_arm" || !panic_body(arm, src) {
                continue;
            }
            let Some(path) = pattern_path(arm, src) else {
                continue;
            };
            if let Some((owner, variant)) = enum_of(&variants, &path, krate) {
                found.push((sym, arm, owner.to_string(), variant.to_string()));
            }
        }
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for (_, _, owner, _) in &found {
        *counts.entry(owner.as_str()).or_default() += 1;
    }
    for (sym, arm, owner, variant) in &found {
        out.push(Finding {
            rule: "21",
            site: site(facts, sym, *arm),
            message: format!(
                "{} panics on {owner}::{variant} instead of holding the invariant in the type \
                 - narrow the scrutinee (a second enum without the variant, or a `TryFrom`)",
                sym.qname
            ),
            cause: format!("panic-arm:{owner}:{variant}"),
            evidence: Evidence::ast(),
            salience: counts[owner.as_str()] as f64,
            fix: None,
            lang: "rs",
        });
    }
}
