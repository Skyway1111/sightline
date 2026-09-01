//! Python facts (`docs/rewrite/codemap.md`, section 3.2 in the Python
//! tree): `facts/model.py`, `facts/build.py`, `astutil.py` and the Python
//! half of `complexity.py`, over `ruff_python_parser`'s tree. Facts are
//! arenas with indices (decision 7); the one traversal orders children as
//! CPython does (R5, `order.rs`). Each module is the port of the Python
//! source its header names; a stub module is one a phase-2 unit has not
//! filled yet.

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
