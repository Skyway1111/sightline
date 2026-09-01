//! The language-blind core (`docs/rewrite/codemap.md`, section 3.1 in the
//! Python tree): everything past the rules and every shared prover core.
//! Each module is the port of the Python source its header names; a stub
//! module is one a phase-1 unit has not filled yet.

pub mod catalog;
pub mod clones;
pub mod complexity;
pub mod config;
pub mod edits;
pub mod findings;
pub mod git;
pub mod graph;
pub mod lang;
pub mod patch;
pub mod precision;
pub mod progress;
pub mod pyjson;
pub mod pytext;
pub mod rank;
pub mod ratchet;
pub mod registry;
pub mod render;
pub mod rule;
pub mod suppress;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod text;
pub mod verdict;
pub mod walk;
pub mod worlds;
