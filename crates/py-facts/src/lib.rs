//! Python facts: `RepoFacts` built once over `ruff_python_parser`'s tree,
//! with the AST model, the pure-AST predicates and the Python half of the
//! complexity score. Facts are arenas with indices; the one traversal
//! orders children as CPython does (R5, `order.rs`).

pub mod astutil;
pub mod build;
pub mod cn;
pub mod complexity;
pub mod dump;
pub mod index;
pub mod inputs;
pub mod kinds;
pub mod lines;
pub mod literal;
pub mod model;
pub mod module;
pub mod order;
pub mod qnames;
pub mod resolve;
pub mod typecomments;
pub mod unparse;
