//! `RsProvers`: every question a Rust rule asks of a body or a module,
//! walked once and memoized (port of `rs/provers.py`). The mining, the
//! comment predicates and the complexity scorer are the language-neutral
//! cores; this crate only builds their inputs out of tree-sitter nodes.
//!
//! The one importer of `oracle`: nothing else in the Rust stack runs a
//! toolchain, and what it answered hangs off `RsProvers.rust`.
//!
//! `RsProvers<'t>` borrows the facts and the answers it reads, so the stack
//! hands one out per pass (`RsStack::provers`) rather than owning it beside
//! the arena: a memo holding a `Node<'t>` cannot sit in a covariant
//! `self_cell` dependent.

mod bodies;
pub mod catalog;
mod clones;
pub mod closed_world;
mod docs;
pub mod dump;
pub mod layers;
pub mod oracle;
pub mod spelled;
pub mod splice;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Value, json};

use sightline_core::clones::{MIN_BLOCK_STMTS, MIN_CLONE_NODES, Seq, digest, repeats};
use sightline_core::findings::Qname;
use sightline_core::pytext;
use sightline_core::text::{is_phase_label, lookup};
use sightline_rs_facts::build::parse;
use sightline_rs_facts::model::{RsComment, RsFacts, RsImpl, RsModule, RsSymbol, is_fn_kind, text};
use sightline_rs_facts::nodes::{
    ALL, ATTRS, COMMENTS, DURATION_SCALE, GUARDS, IDENTS, LITERALS, NESTED_FN, allow_names,
    ancestors, arg_nodes, call_target, children, closure_params, descend, forwards_only, has,
    is_fn, named_children, number, own_sequences, statements, type_params,
};
use sightline_rs_facts::{COMMENT_PREFIX, Node, TREE_SITTER, TREE_SITTER_RUST};

use crate::closed_world::ClosedWorld;
use crate::oracle::RsAnswers;
use crate::spelled::Uses;

pub use bodies::{RsBody, RsCall, RsClosure};
pub use clones::{RsCloneGroup, RsSeq};
pub use docs::{MIN_CODE_LINES, RsAllow, RsCommentBlock, parses_as_code};

/// Kinds that declare a type.
pub const TYPE_KINDS: &str = "struct enum type";
/// A type, borrowed.
const DEREF: &str = "reference_type pointer_type";

/// `impl <what a generated impl names> for`, inside a macro's token tree.
static IMPL_HEAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\bimpl\b([^;{}]*?)\bfor\b").expect("the impl-head pattern compiles")
});
static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("the word pattern compiles"));

/// Every memo one pass shares. The stack builds one per pass, so no cell
/// needs a `warm`: no initializer here runs a rayon job.
pub struct RsProvers<'t> {
    facts: &'t RsFacts<'t>,
    /// the toolchain's answers
    pub rust: &'t RsAnswers,
    bodies: Vec<OnceLock<RsBody<'t>>>,
    no_body: OnceLock<RsBody<'t>>,
    allows: OnceLock<IndexMap<Qname, Vec<RsAllow>>>,
    /// module qname -> covered line -> index into that module's comments
    doc_lines: OnceLock<HashMap<Qname, HashMap<u32, usize>>>,
    module_docs: OnceLock<IndexMap<Qname, Vec<String>>>,
    comment_blocks: OnceLock<IndexMap<Qname, Vec<RsCommentBlock>>>,
    trait_impls: OnceLock<IndexMap<String, Vec<String>>>,
    local_types: OnceLock<HashSet<String>>,
    orphan_traits: OnceLock<HashSet<String>>,
    blanket_traits: OnceLock<HashSet<String>>,
    macro_traits: OnceLock<HashSet<String>>,
    function_digests: OnceLock<IndexMap<Qname, String>>,
    clone_sequences: OnceLock<Vec<RsSeq<'t>>>,
    block_clones: OnceLock<Vec<RsCloneGroup<'t>>>,
    instantiations: OnceLock<IndexMap<Qname, Uses>>,
    unindexed_names: OnceLock<BTreeSet<String>>,
    closed_world: OnceLock<ClosedWorld<'t>>,
    /// the two node-keyed caches, by `Node::id` (R3)
    shapes: Mutex<HashMap<usize, Arc<str>>>,
    sizes: Mutex<HashMap<usize, usize>>,
}

impl<'t> RsProvers<'t> {
    pub fn new(facts: &'t RsFacts<'t>, rust: &'t RsAnswers) -> RsProvers<'t> {
        RsProvers {
            facts,
            rust,
            bodies: (0..facts.symbols.len()).map(|_| OnceLock::new()).collect(),
            no_body: OnceLock::new(),
            allows: OnceLock::new(),
            doc_lines: OnceLock::new(),
            module_docs: OnceLock::new(),
            comment_blocks: OnceLock::new(),
            trait_impls: OnceLock::new(),
            local_types: OnceLock::new(),
            orphan_traits: OnceLock::new(),
            blanket_traits: OnceLock::new(),
            macro_traits: OnceLock::new(),
            function_digests: OnceLock::new(),
            clone_sequences: OnceLock::new(),
            block_clones: OnceLock::new(),
            instantiations: OnceLock::new(),
            unindexed_names: OnceLock::new(),
            closed_world: OnceLock::new(),
            shapes: Mutex::default(),
            sizes: Mutex::default(),
        }
    }

    pub fn facts(&self) -> &'t RsFacts<'t> {
        self.facts
    }

    /// Every memo whose initializer runs rayon jobs, forced on the calling
    /// thread before the rules go parallel (`run_rules`): the blocks, and the
    /// #34 reading of each run long enough for a rule to ask.
    pub fn warm(&self) {
        use rayon::prelude::*;
        // fn bodies under rayon: the cells are per-symbol `OnceLock`s, and
        // filling them inside #20's own pass ran one body at a time
        let fns: Vec<&Qname> = self
            .facts
            .symbols
            .iter()
            .filter(|(_, s)| is_fn_kind(s.kind))
            .map(|(q, _)| q)
            .collect();
        fns.par_iter().for_each(|q| {
            self.body(q);
        });
        let long: Vec<&docs::RsCommentBlock> = self
            .comment_blocks()
            .values()
            .flatten()
            .filter(|b| b.lines.len() >= docs::MIN_CODE_LINES)
            .collect();
        long.par_iter().for_each(|b| {
            b.code();
        });
    }

    pub fn provenance(&self, facts: &RsFacts<'_>) -> Value {
        json!({"rs": {
            "tree_sitter": TREE_SITTER,
            "tree_sitter_rust": TREE_SITTER_RUST,
            "parse_errors": facts.errors.len(),
            "oracle": self.rust.block,
        }})
    }

    /// trait name -> the type qnames the repo implements it for (#37's
    /// single-impl arm).
    pub fn trait_impls(&self) -> &IndexMap<String, Vec<String>> {
        self.trait_impls.get_or_init(|| {
            let mut out: IndexMap<String, Vec<String>> = IndexMap::new();
            for i in &self.facts.impls {
                if let Some(name) = &i.trait_name {
                    out.entry(name.clone())
                        .or_default()
                        .push(i.type_qname.clone());
                }
            }
            out
        })
    }

    /// Every type name the repo declares: what tells an `impl` on the repo's
    /// own type from one on a type it does not own.
    pub fn local_types(&self) -> &HashSet<String> {
        self.local_types.get_or_init(|| {
            self.facts
                .symbols
                .values()
                .filter(|s| has(TYPE_KINDS, s.kind))
                .map(|s| s.name.clone())
                .collect()
        })
    }

    /// Traits whose every `impl` targets a type from outside the repo. Rust's
    /// orphan rule leaves a trait as the one way to hang a method there, so a
    /// single impl is the shape the language forces (#37).
    pub fn orphan_traits(&self) -> &HashSet<String> {
        self.orphan_traits.get_or_init(|| {
            let mut found: IndexMap<&str, bool> = IndexMap::new();
            for i in &self.facts.impls {
                if let Some(name) = &i.trait_name {
                    let foreign = self.foreign(i);
                    let all = found.entry(name.as_str()).or_insert(true);
                    *all = *all && foreign;
                }
            }
            found
                .into_iter()
                .filter(|(_, all)| *all)
                .map(|(name, _)| name.to_string())
                .collect()
        })
    }

    /// Does an `impl` block target a type this repo does not declare? Its own
    /// type parameter is not one: `impl<T> Trait for T` reaches every type
    /// there is, the opposite of an abstraction with one user.
    fn foreign(&self, i: &RsImpl<'_>) -> bool {
        let src = self.facts.modules[&i.module].bytes;
        !self.local_types().contains(&i.type_name)
            && !type_params(i.node, src).contains(&i.type_name)
    }

    /// Traits some `impl` hangs on a family of types rather than on one, so
    /// no count of impl blocks says how many implementors they have (#37).
    pub fn blanket_traits(&self) -> &HashSet<String> {
        self.blanket_traits.get_or_init(|| {
            self.facts
                .impls
                .iter()
                .filter(|i| i.trait_name.is_some() && self.blanket(i))
                .filter_map(|i| i.trait_name.clone())
                .collect()
        })
    }

    /// Does the `impl` hang the trait on a type it parameterizes (`for
    /// Box<T>`, log4rs's `for DeserializeEraser<T>`)? Then it has one
    /// implementor per argument its users pick and the repo counts none of
    /// them. A target that is the parameter itself (`for T`, `for &T`) is an
    /// extension trait: one body, the one implementation #37 reads.
    fn blanket(&self, i: &RsImpl<'_>) -> bool {
        let src = self.facts.modules[&i.module].bytes;
        let mut target = i.node.child_by_field_name("type");
        while let Some(t) = target
            && has(DEREF, t.kind())
        {
            target = t.child_by_field_name("type");
        }
        // a bare name: the parameter itself, or one type
        let Some(target) = target.filter(|t| t.kind() != "type_identifier") else {
            return false;
        };
        let params = type_params(i.node, src);
        descend(target, IDENTS)
            .into_iter()
            .any(|n| params.contains(&text(n, src).into_owned()))
    }

    /// Names an `impl ... for` spells inside a `macro_rules!` body. The item
    /// walk never enters a macro, so no in-repo impl count answers for a
    /// trait one of them implements (#37).
    pub fn macro_traits(&self) -> &HashSet<String> {
        self.macro_traits.get_or_init(|| {
            let mut out: HashSet<String> = HashSet::new();
            for sym in self.facts.symbols.values() {
                if sym.kind != "macro" {
                    continue;
                }
                let body = text(sym.node, self.facts.modules[&sym.module].bytes);
                for head in IMPL_HEAD.captures_iter(&body) {
                    let named = head.get(1).expect("the pattern has one group").as_str();
                    out.extend(WORD.find_iter(named).map(|m| m.as_str().to_string()));
                }
            }
            out
        })
    }

    // --- the folds the source spells ----------------------------------------

    /// generic item qname -> what the repo's prod uses of it name (#37).
    pub fn instantiations(&self) -> &IndexMap<Qname, Uses> {
        self.instantiations
            .get_or_init(|| spelled::instantiations(self.facts))
    }

    /// Every identifier a macro body or an attribute's strings spell (#48).
    pub fn unindexed_names(&self) -> &BTreeSet<String> {
        self.unindexed_names
            .get_or_init(|| spelled::unindexed_names(self.facts))
    }

    /// The per-pass memo the graph rules and `describe` share.
    pub fn closed_world(&self) -> &ClosedWorld<'t> {
        self.closed_world
            .get_or_init(|| ClosedWorld::new(self.facts, self.rust))
    }
}

/// A memo whose value is cloned out. The lock is never held while the value
/// is built, so a build that re-enters this memo (`shape`, `size`,
/// `ClosedWorld::verdict`) reads its own children first.
pub(crate) fn cached<K, V>(cell: &Mutex<HashMap<K, V>>, key: K, build: impl FnOnce() -> V) -> V
where
    K: Eq + std::hash::Hash,
    V: Clone,
{
    if let Some(hit) = cell.lock().expect("no memo panicked").get(&key).cloned() {
        return hit;
    }
    let out = build();
    cell.lock()
        .expect("no memo panicked")
        .insert(key, out.clone());
    out
}
