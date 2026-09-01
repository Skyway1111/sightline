//! The `debug dump` layers the Rust provers answer: `rs-bodies`, `rs-graph`,
//! `rs-world` and `rs-clones`. `dump.rs` writes them; `None` is a layer no
//! stack of this language answers.

use serde_json::Value;
use sightline_rs_facts::model::RsFacts;

use crate::{RsProvers, dump};

pub fn layer(name: &str, facts: &RsFacts<'_>, provers: &RsProvers<'_>) -> Option<Value> {
    match name {
        "rs-bodies" => Some(dump::rs_bodies(facts, provers)),
        "rs-graph" => Some(dump::rs_graph(provers)),
        "rs-world" => Some(dump::rs_world(facts, provers)),
        "rs-clones" => Some(dump::rs_clones(facts, provers)),
        _ => None,
    }
}
