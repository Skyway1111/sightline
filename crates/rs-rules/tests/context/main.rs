//! The context, dead and trust rule tests in one binary. A target per file
//! relinks the whole dependency graph, so one module per source file sits
//! under this one.

mod context;
mod dead;
mod trust;
