//! #37, speculative generality: the single-impl trait and the monomorphic
//! parameter.

use std::collections::HashSet;

use sightline_core::findings::{Evidence, Finding, Qname, Sink};
use sightline_rs_facts::model::{RsFacts, RsSymbol};
use sightline_rs_facts::nodes::type_args;
use sightline_rs_provers::RsProvers;

use crate::util::site;

/// the sibling's prod-site floor: under it there is no "all"
const MONO_SITES: usize = 3;
/// the kind a `type Name<T> = ...` item takes
const ALIAS: &str = "type";

/// A `type` alias that hands the parameter straight to the type it names
/// (`type Result<T> = std::result::Result<T, Error>`, the shape of std's own
/// `io::Result`). The alias abstracts nothing of its own, so there is
/// nothing to collapse: naming the type only moves the spelling into every
/// signature that returns it.
fn passes_through(facts: &RsFacts<'_>, sym: &RsSymbol<'_>, param: &str) -> bool {
    let src = facts.modules[&sym.module].bytes;
    sym.kind == ALIAS
        && sym
            .node
            .child_by_field_name(ALIAS)
            .and_then(|aliased| type_args(aliased, src))
            .is_some_and(|args| args.iter().any(|a| a == param))
}

/// A trait the crate exports is implemented downstream, and one a
/// `macro_rules!` body implements expands where the item walk cannot follow,
/// so no in-repo count answers for either; a trait no impl names is a marker
/// or a bound, and stays silent the way a zero-impl Protocol does. One impl
/// on a foreign type is the orphan rule's only shape, not flexibility left
/// unexercised, and an impl on a type it parameterizes hangs the trait on a
/// family whose implementors nothing here counts.
///
/// The monomorphic arm reads the parameter the same way: every use spells
/// the one type argument, and a spelled `Foo<_>` is a type nothing here
/// knows, so it silences whatever the others agree on. An alias that hands
/// its parameter to the type it names abstracts nothing of its own.
pub(super) fn rule_37(facts: &RsFacts<'_>, provers: &RsProvers<'_>, out: &mut Sink) {
    let impls = provers.trait_impls();
    let uncounted: HashSet<&str> = provers
        .orphan_traits()
        .iter()
        .chain(provers.macro_traits())
        .chain(provers.blanket_traits())
        .map(String::as_str)
        .collect();
    for sym in facts.symbols.values() {
        if sym.kind != "trait" || sym.is_public || sym.is_test {
            continue;
        }
        let types = impls.get(&sym.name).map(Vec::as_slice).unwrap_or_default();
        if types.len() == 1 && !uncounted.contains(sym.name.as_str()) {
            out.push(Finding {
                rule: "37",
                site: site(facts, sym, sym.node),
                message: format!(
                    "trait {} has exactly one implementation ({}) - speculative abstraction",
                    sym.qname, types[0]
                ),
                cause: format!("single-impl:{}", sym.qname),
                evidence: Evidence::Idx {
                    detail: "trait".to_string(),
                },
                salience: 0.0,
                fix: None,
                lang: "rs",
            });
        }
    }
    let uses = provers.instantiations();
    let mut order: Vec<&Qname> = uses.keys().collect();
    order.sort();
    for qname in order {
        let item = &uses[qname];
        let sym = &facts.symbols[qname];
        if item.inferred || item.spelled.len() < MONO_SITES || facts.publishes(sym) {
            continue;
        }
        for (i, param) in item.params.iter().enumerate() {
            let named: HashSet<&str> = item.spelled.iter().map(|args| args[i].as_str()).collect();
            let [one] = named.into_iter().collect::<Vec<&str>>()[..] else {
                continue;
            };
            if passes_through(facts, sym, param) {
                continue;
            }
            out.push(Finding {
                rule: "37",
                site: site(facts, sym, sym.node),
                message: format!(
                    "type parameter `{param}` of {qname} is {one} at all {} instantiations the \
                     repo spells - name the type",
                    item.spelled.len()
                ),
                cause: format!("monomorphic:{qname}:{param}"),
                evidence: Evidence::Idx {
                    detail: "monomorphic".to_string(),
                },
                salience: item.spelled.len() as f64,
                fix: None,
                lang: "rs",
            });
        }
    }
}
