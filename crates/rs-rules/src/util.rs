//! `rs/rules/util.py`: the one `site` helper every Rust rule calls.

use sightline_core::findings::Site;
use sightline_rs_facts::Node;
use sightline_rs_facts::model::{RsFacts, RsSymbol};

/// A finding's site at a node, owned by a symbol. tree-sitter's
/// `start_point` row is 0-based and its column is a BYTE column.
pub fn site(facts: &RsFacts<'_>, sym: &RsSymbol<'_>, node: Node<'_>) -> Site {
    Site {
        rel: facts.modules[&sym.module].rel.clone(),
        line: node.start_position().row as u32 + 1,
        col: node.start_position().column as u32,
        symbol: sym.qname.clone(),
    }
}
