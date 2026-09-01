//! Every integration test of this crate in one binary. A target per file
//! relinks the whole dependency graph, and the check lane paid that link
//! once per file. One module per test file, named after what it tests.

mod argtypes;
mod callgraph;
mod clones;
mod comments;
mod counterfactual;
mod escapes;
mod grounding;
mod hotness;
mod import_effects;
mod imports;
mod liveness;
mod oracle;
mod oracle_fixture;
mod records;
mod rettypes;
mod scope;
mod shipping;
mod spend;
mod wp;
