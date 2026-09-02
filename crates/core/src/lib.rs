//! The language-blind core: everything past the rules, and every prover
//! core both languages share. Config, the finding model, suppression,
//! rank and ratchet, the four renderers, precision data, the rule record
//! and registry, the `Stack` and `Repo` seam, the discovery walk, git,
//! patches and edits, worlds, clone mining, the complexity score, comment
//! predicates, the catalog vocabulary and Tarjan SCC.

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
