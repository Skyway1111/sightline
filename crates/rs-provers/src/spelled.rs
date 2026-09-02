//! What the source spells where no index answers for it: the type arguments
//! every use of a generic item names (#37's monomorphic arm), and the names
//! a macro body or an attribute's strings mention (#48). Both are folds over
//! facts the rules reach through
//! `RsProvers`, and neither holds an opinion.

use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;

use sightline_core::findings::Qname;
use sightline_core::pytext;
use sightline_rs_facts::model::{RsFacts, RsRef, RsSymbol, is_fn_kind, text};
use sightline_rs_facts::nodes::{
    ALL, ATTRS, STRINGS, descend, has, params_in_scope, type_args, type_params,
};

use crate::TYPE_KINDS;

const SPELLS: &str = "generic_type generic_function";
/// `Foo<_>`: the argument is written and still unknown.
const INFERRED: &str = "_";
/// Names the implementor, so it is a parameter of its own.
const SELF: &str = "Self";

static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("the word pattern compiles"));

/// One generic item's uses outside itself: the parameters it declares, the
/// arguments each spelled use names, and whether a use wrote `Foo<_>` - an
/// argument the source spells and still leaves unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Uses {
    pub params: Vec<String>,
    pub spelled: Vec<Vec<String>>,
    pub inferred: bool,
}

/// generic item qname -> what the repo's prod uses of it name. A use inside
/// the item's own impl blocks says nothing about how the repo instantiates
/// it; a use whose arguments name a type parameter in scope is the
/// declaration side; a use that spells no argument (`Foo::new()`, `let x:
/// Foo`) wrote no type here and neither counts nor silences.
pub fn instantiations(facts: &RsFacts<'_>) -> IndexMap<Qname, Uses> {
    let mut own: IndexMap<Qname, Vec<Qname>> = IndexMap::new();
    for (qname, sym) in &facts.symbols {
        if let Some(parent) = &sym.parent {
            own.entry(parent.clone()).or_default().push(qname.clone());
        }
    }
    let mut out: IndexMap<Qname, Uses> = IndexMap::new();
    for (qname, sym) in &facts.symbols {
        let generic = is_fn_kind(sym.kind) || has(TYPE_KINDS, sym.kind);
        let src = facts.modules[&sym.module].bytes;
        let params = if generic {
            type_params(sym.node, src)
        } else {
            Vec::new()
        };
        if params.is_empty() || sym.is_test {
            continue;
        }
        let associated = own.get(qname).map(Vec::as_slice).unwrap_or_default();
        let mut spelled: Vec<Vec<String>> = Vec::new();
        let mut inferred = false;
        for r in uses(facts, sym, associated) {
            let args = if *r.target == **qname {
                args_of(facts, r)
            } else {
                None
            };
            let Some(args) = args else { continue };
            if args.iter().any(|a| a == INFERRED) || args.len() != params.len() {
                inferred = true;
            } else if !declares(facts, &args, r) {
                spelled.push(args);
            }
        }
        out.insert(
            qname.clone(),
            Uses {
                params,
                spelled,
                inferred,
            },
        );
    }
    out
}

/// Every prod reference naming the item or one of its associated items, the
/// declaration's own name and the item's own body left out.
fn uses<'f, 't>(
    facts: &'f RsFacts<'t>,
    sym: &RsSymbol<'t>,
    associated: &[Qname],
) -> Vec<&'f RsRef<'t>> {
    let name = sym.node.child_by_field_name("name");
    let mut inside: Vec<(&Qname, u32, u32)> = vec![(&sym.module, sym.lineno, sym.end_lineno)];
    for q in associated {
        let other = &facts.symbols[q];
        inside.push((&other.module, other.lineno, other.end_lineno));
    }
    let mut out: Vec<&'f RsRef<'t>> = Vec::new();
    for target in std::iter::once(&sym.qname).chain(associated) {
        out.extend(facts.refs_of(target).filter(|r| {
            !name.is_some_and(|n| r.node.id() == n.id())
                && !facts.is_test(facts.rel_of(&r.module))
                && !inside.iter().any(|(home, start, end)| {
                    r.module == **home && *start <= r.lineno && r.lineno <= *end
                })
        }));
    }
    out
}

fn args_of(facts: &RsFacts<'_>, r: &RsRef<'_>) -> Option<Vec<String>> {
    let parent = r.node.parent()?;
    if !has(SPELLS, parent.kind()) {
        return None;
    }
    type_args(parent, facts.modules[&r.module].bytes)
}

/// Does the use name a parameter rather than a type? `impl<T> Foo<T>` and
/// `fn g<T>(x: Foo<T>)` write the declaration side, and what `Foo` is there
/// is decided wherever `g` is instantiated; `Self` and its associated types
/// (itertools' `VecIntoIter<Self::Item>`) name one type per implementor.
fn declares(facts: &RsFacts<'_>, args: &[String], r: &RsRef<'_>) -> bool {
    let mut scope: HashSet<String> = params_in_scope(r.node, facts.modules[&r.module].bytes);
    scope.insert(SELF.to_string());
    args.iter()
        .flat_map(|arg| WORD.find_iter(arg))
        .any(|word| scope.contains(word.as_str()))
}

/// Every identifier a `macro_rules!` body spells, and every one written
/// inside an attribute's strings (`#[serde(default = "default_preset")]`, a
/// derive's own call to a name the source never spells as a call). Neither
/// is a reference any index reaches, so no reading may call a name's edges
/// all the readers it has (#48).
pub fn unindexed_names(facts: &RsFacts<'_>) -> BTreeSet<String> {
    macro_bodies(facts)
        .into_iter()
        .chain(attr_strings(facts))
        .flat_map(|source| {
            WORD.find_iter(&source)
                .map(|m| m.as_str().to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn macro_bodies(facts: &RsFacts<'_>) -> Vec<String> {
    facts
        .symbols
        .values()
        .filter(|s| s.kind == "macro")
        .map(|s| text(s.node, facts.modules[&s.module].bytes).into_owned())
        .collect()
}

/// The string literals of every attribute but `doc`, whose prose names
/// nothing the compiler calls.
fn attr_strings(facts: &RsFacts<'_>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in facts.modules.values() {
        for node in descend(m.root, ALL) {
            if !has(ATTRS, node.kind())
                || pytext::lstrip_chars(&text(node, m.bytes), "#![").starts_with("doc")
            {
                continue;
            }
            for inner in descend(node, ALL) {
                if has(STRINGS, inner.kind()) {
                    out.push(text(inner, m.bytes).into_owned());
                }
            }
        }
    }
    out
}
