//! Every integration test of this crate in one binary. A target per file
//! relinks the whole dependency graph, and the check lane paid that link
//! once per file. One module per test file, named after what it tests.

mod astutil_differential;
mod build;
mod cc_differential;
mod resolve;
