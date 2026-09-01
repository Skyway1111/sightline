//! Closed-world verdicts over the resolved edges (port of
//! `rs/closed_world.py`): may we claim to know every reference to a Rust
//! item? Fail-closed with a named escape reason, one fixture per reason, and
//! the graph rules run only on a pass.
//!
//! The Python sibling (`py-provers/src/closed_world.rs`) reads names and
//! reflection; here the index answers, so an escape is a place the index
//! cannot see: a downstream user (`published`), dispatch through a trait, a
//! linker name, a proc macro that may write the call, a `#[cfg]` arm this
//! build never compiled, or a crate the base check never compiled.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use sightline_core::findings::Qname;
use sightline_core::pytext;
use sightline_core::verdict::CwVerdict;
use sightline_rs_facts::crates::ident;
use sightline_rs_facts::exports::reachable;
use sightline_rs_facts::model::{RsFacts, RsSymbol, text};
use sightline_rs_facts::nodes::children;

use crate::cached;
use crate::oracle::RsAnswers;
use crate::oracle::index::RsGraph;

/// Attributes the compiler itself owns: everything else on an item is a proc
/// macro, which may write a reference no index of this source shows.
pub const COMPILER_ATTRS: &str = "allow warn deny forbid expect deprecated doc must_use inline \
    cold no_mangle export_name link_name link_section used repr non_exhaustive derive cfg \
    cfg_attr track_caller path macro_export macro_use automatically_derived unsafe no_std \
    no_main global_allocator panic_handler test bench ignore should_panic rustfmt";
/// The derives the compiler expands itself; any other names a proc macro.
pub const STD_DERIVES: &str = "Debug Clone Copy PartialEq Eq PartialOrd Ord Hash Default";
/// Markers that give an item a name the linker publishes.
pub const LINKED: [&str; 4] = ["no_mangle", "export_name", "link_name", "used"];

fn listed(table: &str, name: &str) -> bool {
    table.split(' ').any(|n| n == name)
}

pub struct ClosedWorld<'t> {
    facts: &'t RsFacts<'t>,
    graph: &'t RsGraph,
    /// the crates a world can speak for: a member whose base check errored is
    /// absent, and so is a crate the workspace never enumerated (salvo's
    /// `fuzz` declares a `[workspace]` of its own, so no check of the root
    /// compiles it and no deletion in it can ever be vetoed)
    checked: HashSet<String>,
    reachable: OnceLock<HashSet<Qname>>,
    verdicts: Mutex<HashMap<String, CwVerdict>>,
}

impl<'t> ClosedWorld<'t> {
    /// `answers` is `RsProvers.rust`: its graph and its unchecked members.
    pub fn new(facts: &'t RsFacts<'t>, answers: &'t RsAnswers) -> ClosedWorld<'t> {
        ClosedWorld {
            facts,
            graph: &answers.graph,
            checked: answers.checked.iter().map(|m| ident(&m.name)).collect(),
            reachable: OnceLock::new(),
            verdicts: Mutex::default(),
        }
    }

    /// Every bare-`pub` item a crate root reaches. rustc's `dead_code`
    /// reports private, `pub(crate)` and `pub`-in-private-module items and
    /// stays silent on exactly this set, in a bin crate and a lib alike, so
    /// this is what a dead-weight reading here may judge.
    pub fn reachable(&self) -> &HashSet<Qname> {
        self.reachable
            .get_or_init(|| reachable(self.facts, &self.facts.crates.keys().cloned().collect()))
    }

    pub fn verdict(&self, qname: &str) -> CwVerdict {
        cached(&self.verdicts, qname.to_string(), || self.compute(qname))
    }

    fn compute(&self, qname: &str) -> CwVerdict {
        let Some(sym) = self.facts.symbols.get(qname) else {
            return CwVerdict::escaped(["unknown-symbol"]);
        };
        let edges = self.graph.edges_to(qname);
        // every escape, in this order
        let mut reasons: Vec<&str> = Vec::new();

        // a downstream user reaches the item over a seam no audit of this
        // tree can enumerate
        if self.facts.publishes(sym) {
            reasons.push("published");
        }
        // the name is read as a value, not called: where it travels from
        // there the index does not say
        if edges.iter().any(|e| !e.call) {
            reasons.push("reference-escape");
        }
        // dispatch through a trait: the callers point at the declaration, so
        // the body that runs is not the one the edge names
        if edges.iter().any(|e| e.open) || !sym.traits.is_empty() {
            reasons.push("open-dispatch");
        }
        if self.linked(sym) {
            reasons.push("extern");
        }
        if proc_macro(&sym.attrs) {
            reasons.push("proc-macro");
        }
        // default features only: an arm this build never compiled has its
        // references in code cargo never read
        let gated_caller = edges.iter().any(|e| {
            self.facts
                .symbols
                .get(e.caller.as_str())
                .is_some_and(|caller| cfg_gated(caller))
        });
        if cfg_gated(sym) || gated_caller {
            reasons.push("cfg-gated");
        }
        if !self
            .checked
            .contains(&self.facts.modules[&sym.module].crate_name)
        {
            reasons.push("unchecked-crate");
        }

        if reasons.is_empty() {
            CwVerdict::passed()
        } else {
            CwVerdict::escaped(reasons)
        }
    }

    /// An item the linker names, so a caller outside this build reaches it: a
    /// `#[no_mangle]`-grade attribute (`#[unsafe(no_mangle)]` included) or an
    /// `extern` ABI on the `fn` itself.
    fn linked(&self, sym: &RsSymbol<'_>) -> bool {
        if sym
            .attrs
            .iter()
            .any(|a| LINKED.iter().any(|marker| a.contains(marker)))
        {
            return true;
        }
        let src = self.facts.modules[&sym.module].bytes;
        children(sym.node)
            .into_iter()
            .any(|c| c.kind() == "function_modifiers" && text(c, src).contains("extern"))
    }
}

/// A `#[cfg]` or `#[cfg_attr]` the item is not a test by: `is_test` already
/// holds every `cfg(test)` spelling (`rs-facts`).
fn cfg_gated(sym: &RsSymbol<'_>) -> bool {
    !sym.is_test
        && sym
            .attrs
            .iter()
            .any(|a| a.starts_with("cfg(") || a.starts_with("cfg_attr("))
}

/// An attribute or a derive the compiler does not own. Such a macro may write
/// the reference this index never sees (`#[tokio::main]`,
/// `#[derive(Parser)]`), so the world is open wherever one sits.
fn proc_macro(attrs: &[String]) -> bool {
    for attr in attrs {
        let head = attr.split('(').next().unwrap_or(attr);
        let name = pytext::strip(head.split('=').next().unwrap_or(head));
        if name != "derive" {
            if !listed(COMPILER_ATTRS, name) {
                return true;
            }
            continue;
        }
        // `attr[attr.find("(") + 1 : attr.rfind(")")]`, where a missing
        // parenthesis is Python's -1: `#[derive]` reads as `deriv`
        let open = attr.find('(').map_or(0, |i| i + 1);
        let close = attr.rfind(')').unwrap_or(attr.len().saturating_sub(1));
        if attr
            .get(open..close)
            .unwrap_or_default()
            .split(',')
            .map(pytext::strip)
            .filter(|d| !d.is_empty())
            .any(|d| !listed(STD_DERIVES, d.rsplit("::").next().unwrap_or(d)))
        {
            return true;
        }
    }
    false
}
