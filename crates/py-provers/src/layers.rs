//! The provers' `debug dump` layers: one document per prover, written by the
//! module that owns it. `None` is a layer this stack does not answer.

use serde_json::Value;

use sightline_py_facts::model::RepoFacts;

use crate::{
    Provers, callgraph, clones, closed_world, counterfactual, effects, hotness, imports, liveness,
    oracle, records, scope,
};

pub fn layer(name: &str, facts: &RepoFacts<'_>, provers: &Provers) -> Option<Value> {
    match name {
        "scope" => scope::dump(facts, provers),
        "graph" => callgraph::dump(facts, provers),
        "world" => closed_world::dump(facts, provers),
        "effects" => effects::dump(facts, provers),
        "liveness" => liveness::dump(facts, provers),
        "imports" => imports::dump(facts, provers),
        "hot" => hotness::dump(facts, provers),
        "records" => records::dump(facts, provers),
        "clones" => clones::dump(facts, provers),
        "oracle" => oracle::dump(facts, provers),
        "verify" => counterfactual::dump(facts, provers),
        _ => None,
    }
}
