//! Every integration test of this crate in one binary. A target per file
//! relinks the whole dependency graph, and the check lane paid that link
//! once per file. One module per test file, named after what it tests.

mod comments;
mod context;
mod dead;
mod describe;
mod emit;
mod fixes;
mod helpers;
mod identity;
mod idioms;
mod imports;
mod neutrality;
mod oracle_errors;
mod oracle_rules_trust;
mod perf;
mod records;
mod registry;
mod render;
mod returns;
mod surface;
mod tests_quality;
mod trust;
