//! The Rust stack's facts over tree-sitter, provers apart: the vocabulary
//! every module of it shares, the parsed modules, the crate layout a path
//! spells, and the cross-module indexes.
//!
//! `Node` is re-exported so `rs-rules` reads tree-sitter through this
//! crate and never lists the parser in its own manifest.

pub mod attrs;
pub mod build;
pub mod complexity;
pub mod crates;
pub mod dump;
pub mod exports;
pub mod model;
pub mod nodes;

pub use tree_sitter::Node;

pub const SUFFIX: &str = ".rs";
pub const MANIFEST: &str = "Cargo.toml";
pub const COMMENT_PREFIX: &str = "//";

/// The parser versions the provenance header prints. Cargo hands a crate no
/// version of its dependencies, so these sit beside the manifest's pin and a
/// test reads both.
pub const TREE_SITTER: &str = "0.26.13";
pub const TREE_SITTER_RUST: &str = "0.24.2";

/// A test path: cargo's own three roots for code that is not the crate.
pub const TEST_DIRS: &str = "tests benches examples";

/// Rust's prelude and the standard-library roots: names every crate holds
/// without a `use`, so a call on one resolves outside the repo. The analog
/// of Python's builtins, and why `Ok(v)` is EXTERNAL and not UNRESOLVED.
pub const PRELUDE: &str = "std core alloc Ok Err Some None Option Result String Vec Box Default \
    From Into TryFrom TryInto Iterator IntoIterator Clone Copy Drop ToString ToOwned Send Sync \
    Sized drop format panic";

/// Does the prelude name this path head?
pub fn in_prelude(name: &str) -> bool {
    PRELUDE.split(' ').any(|n| n == name)
}

/// The one path reading of "what is a test file"; the two item-level
/// readings (`#[cfg(test)]`, `#[test]`) are `RsSymbol.is_test`.
pub fn is_test_path(rel: &str) -> bool {
    let mut parts = rel.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_some() && TEST_DIRS.split(' ').any(|d| d == part) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The printed versions and the manifest's pin are one reading.
    #[test]
    fn the_parser_versions_match_the_manifest_pin() {
        let manifest = include_str!("../Cargo.toml");
        assert!(manifest.contains(r#"tree-sitter = "0.26""#));
        assert!(manifest.contains(r#"tree-sitter-rust = "=0.24.2""#));
        assert!(TREE_SITTER.starts_with("0.26."));
        assert_eq!(TREE_SITTER_RUST, "0.24.2");
    }

    #[test]
    fn the_prelude_holds_the_thirty_names_a_crate_spells_bare() {
        assert_eq!(PRELUDE.split(' ').count(), 30);
        assert!(in_prelude("String") && in_prelude("panic"));
        assert!(!in_prelude("Strin") && !in_prelude(""));
    }

    #[test]
    fn a_test_path_is_a_directory_segment_never_the_file_name() {
        assert!(is_test_path("tests/it.rs"));
        assert!(is_test_path("crates/a/benches/b.rs"));
        assert!(is_test_path("examples/e.rs"));
        assert!(!is_test_path("src/tests.rs"));
        assert!(!is_test_path("src/lib.rs"));
    }
}
