//! `RsFacts` data model over tree-sitter nodes (port of `rs/model.py`'s
//! data half): indexes only, no opinions and no oracle. The Rust cognitive
//! complexity classification is `complexity.rs`.

use std::borrow::{Borrow, Cow};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::ops::Index;

use camino::Utf8PathBuf;
use indexmap::{IndexMap, IndexSet};
use sightline_core::config::Config;
use sightline_core::findings::{Qname, Rel};
use tree_sitter::Node;

use crate::is_test_path;

/// Kinds a `fn` backs.
pub const FUNCTION_KINDS: [&str; 2] = ["function", "method"];

pub fn is_fn_kind(kind: &str) -> bool {
    FUNCTION_KINDS.contains(&kind)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    /// the name is the function of a call or a macro
    Callee,
    Load,
    Store,
}

impl RefKind {
    // sightline-ok: 11 - an enum's match table is its own name
    pub fn value(self) -> &'static str {
        match self {
            RefKind::Callee => "callee",
            RefKind::Load => "load",
            RefKind::Store => "store",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolution {
    Resolved,
    /// a method name match on a plain receiver: no types, so every
    /// same-named method in the repo is a candidate
    ByName,
    /// resolves outside the repo
    External,
    Unresolved,
}

impl Resolution {
    // sightline-ok: 11 - an enum's match table is its own name
    pub fn value(self) -> &'static str {
        match self {
            Resolution::Resolved => "resolved",
            Resolution::ByName => "by-name",
            Resolution::External => "external",
            Resolution::Unresolved => "unresolved",
        }
    }
}

/// A node's source. Rust is UTF-8, but a stray byte must degrade rather
/// than kill the run (R22).
pub fn text<'a>(node: Node<'_>, src: &'a [u8]) -> Cow<'a, str> {
    match node.utf8_text(src) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => String::from_utf8_lossy(&src[node.byte_range()])
            .into_owned()
            .into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsComment {
    /// 1-based start
    pub line: u32,
    pub end_line: u32,
    /// the source line(s), marker included
    pub text: String,
    /// "comment" | "doc" (`///`) | "module-doc" (`//!`)
    pub kind: &'static str,
}

pub struct RsModule<'t> {
    pub qname: Qname,
    pub rel: Rel,
    pub path: Utf8PathBuf,
    /// the lossy decode of the whole file; `bytes` is what the parse read
    pub source: &'t str,
    pub bytes: &'t [u8],
    pub lines: Vec<&'t str>,
    /// the qname's first `::` segment
    pub crate_name: String,
    pub root: Node<'t>,
    /// local name -> path
    pub bindings: IndexMap<String, String>,
    /// bare name -> qname
    pub items: IndexMap<String, Qname>,
    /// what this module re-exports, which `bindings` does not tell apart:
    /// the `pub use` entries as (local name, path), a glob's local name
    /// being `*`
    pub reexports: Vec<(String, String)>,
    /// `pub mod` scope qnames
    pub pub_mods: IndexSet<String>,
    pub comments: Vec<RsComment>,
    /// the `//!` lines, markers stripped
    pub doc: Vec<String>,
}

impl<'t> RsModule<'t> {
    pub fn text(&self, node: Node<'_>) -> Cow<'t, str> {
        text(node, self.bytes)
    }
}

pub struct RsSymbol<'t> {
    pub qname: Qname,
    pub module: Qname,
    pub name: String,
    /// function|method|struct|enum|trait|type|const|static|macro
    pub kind: &'static str,
    pub node: Node<'t>,
    pub lineno: u32,
    pub end_lineno: u32,
    /// bare `pub`; `pub(crate)` is not
    pub is_public: bool,
    /// enclosing symbol qname, `None` at module scope
    pub parent: Option<Qname>,
    /// the item's own `#[...]`, as written
    pub attrs: Vec<String>,
    /// traits whose impl block defines this method
    pub traits: Vec<String>,
    /// the path, `#[cfg(test)]` or `#[test]`
    pub is_test: bool,
}

pub struct RsRef<'t> {
    pub module: Qname,
    pub node: Node<'t>,
    /// the resolved path, or the name as spelled
    pub target: String,
    pub kind: RefKind,
    pub lineno: u32,
}

pub struct RsCallSite<'t> {
    pub module: Qname,
    pub node: Node<'t>,
    /// symbol qname, module qname at top level
    pub enclosing: Qname,
    pub resolution: Resolution,
    pub target: Option<String>,
    pub lineno: u32,
}

pub struct RsImpl<'t> {
    pub module: Qname,
    pub trait_name: Option<String>,
    pub type_name: String,
    pub type_qname: String,
    pub lineno: u32,
    pub node: Node<'t>,
}

#[derive(Default)]
pub struct RsFacts<'t> {
    pub root: Utf8PathBuf,
    pub config: Config,
    pub modules: IndexMap<Qname, RsModule<'t>>,
    pub symbols: IndexMap<Qname, RsSymbol<'t>>,
    pub refs: Vec<RsRef<'t>>,
    pub call_sites: Vec<RsCallSite<'t>>,
    pub impls: Vec<RsImpl<'t>>,
    pub errors: Vec<String>,
    /// what the header must say
    pub notes: Vec<String>,
    pub doc_files: IndexMap<Rel, Vec<String>>,
    pub all_files: Vec<Rel>,
    /// crate name -> dir rel
    pub crates: IndexMap<String, String>,
    /// symbol qnames a publishable lib crate's root reaches; empty is a
    /// tree whose every caller is in it (`exports.rs`)
    pub published: HashSet<Qname>,
    /// re-export alias qname -> the definition it names: what `refs_to` is
    /// keyed through, so a reference spelled through a `pub use` counts
    pub aliases: IndexMap<String, String>,
    /// target -> indices into `refs`
    pub refs_to: HashMap<String, Vec<u32>>,
    /// module qname -> indices into `symbols`
    pub symbols_by_module: HashMap<Qname, Vec<u32>>,
    pub methods_by_name: HashMap<String, Vec<Qname>>,
    /// the ranking prior, precomputed for every function symbol (R20)
    pub cc: HashMap<Qname, u32>,
    pub module_by_rel: HashMap<Rel, Qname>,
    /// inbound cross-module refs per module
    pub fan_in: HashMap<Qname, u32>,
}

/// The arena rows one bucket of an index names, in arena order: every index
/// of `RsFacts` holds positions, so a reader never clones a row.
fn indexed<'a, K: Borrow<str> + Eq + Hash, C: Index<usize, Output = T>, T: 'a>(
    index: &'a HashMap<K, Vec<u32>>,
    arena: &'a C,
    key: &str,
) -> impl Iterator<Item = &'a T> {
    let rows = index.get(key).map(Vec::as_slice).unwrap_or_default();
    rows.iter().map(move |i| &arena[*i as usize])
}

impl<'t> RsFacts<'t> {
    pub fn is_test(&self, rel: &str) -> bool {
        is_test_path(rel)
    }

    /// Cognitive complexity of the symbol's body, the ranking prior; 0 for
    /// anything a `fn` does not back.
    pub fn cc_prior(&self, qname: &str) -> u32 {
        self.cc.get(qname).copied().unwrap_or(0)
    }

    /// A downstream user can reach `sym`: the crate root reaches it through
    /// `pub mod` chains and `pub use` re-exports, and every step of the
    /// chain is bare `pub`.
    pub fn publishes(&self, sym: &RsSymbol<'t>) -> bool {
        self.published.contains(&sym.qname)
    }

    pub fn rel_of(&self, module: &str) -> &str {
        &self.modules[module].rel
    }

    /// The symbols of one module, in `symbols` order.
    pub fn symbols_of(&self, module: &str) -> impl Iterator<Item = &RsSymbol<'t>> {
        indexed(&self.symbols_by_module, &self.symbols, module)
    }

    /// The refs naming one target, in `refs` order.
    pub fn refs_of(&self, target: &str) -> impl Iterator<Item = &RsRef<'t>> {
        indexed(&self.refs_to, &self.refs, target)
    }
}
