//! `build_facts`: discover, parse and index a Cargo root. Pass A indexes
//! per-module items, `use` bindings and comments; pass B resolves
//! name-level refs and calls against the symbol table A built.

use std::cell::RefCell;
use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use rayon::prelude::*;
use sightline_core::config::Config;
use sightline_core::findings::{Qname, Rel};
use sightline_core::lang::Listing;
use sightline_core::pytext::{self, source_lines};
use sightline_core::text::lookup;
use sightline_core::walk;
use tree_sitter::{Node, Tree};

use crate::attrs::{attrs_of, cfgs, declared_cfgs, is_test_attr, named};
use crate::complexity::cognitive_complexity;
use crate::crates::{crate_roots, manifests as read_manifests, module_qname_map};
use crate::exports::{aliases, follow, published};
use crate::model::{
    RefKind, Resolution, RsCallSite, RsComment, RsFacts, RsImpl, RsModule, RsRef, RsSymbol,
    is_fn_kind, text,
};
use crate::nodes::{COMMENTS, children, has, named_children, nonempty};
use crate::{SUFFIX, in_prelude, is_test_path};

mod built {
    // the one self-referential struct of the Rust stack
    #![allow(unsafe_code)]

    use super::{RsFacts, RsTree};

    self_cell::self_cell!(
        /// The parsed trees and the facts borrowing them, owned together.
        pub struct RsBuilt {
            owner: RsTree,
            #[covariant]
            dependent: RsFacts,
        }
    );
}

mod items;
mod refs;

pub use built::RsBuilt;

use items::*;
use refs::*;

/// A parsed file: its bytes, the lossy decode where those are not UTF-8,
/// and its tree.
type Read = Option<(Vec<u8>, Option<String>, Tree)>;

/// A module's bare names, its `use` bindings and its re-exports.
type Scope = (
    IndexMap<String, Qname>,
    IndexMap<String, String>,
    Vec<(String, String)>,
);

/// `bytes` is what tree-sitter read, so every node offset indexes it;
/// `lossy` holds the decode only where those bytes are not UTF-8.
pub struct Source {
    pub rel: Rel,
    pub path: Utf8PathBuf,
    pub qname: Qname,
    pub bytes: Vec<u8>,
    pub lossy: Option<String>,
    pub tree: Tree,
}

impl Source {
    pub fn text(&self) -> &str {
        match &self.lossy {
            Some(text) => text,
            None => std::str::from_utf8(&self.bytes).expect("checked at read"),
        }
    }
}

pub struct RsTree {
    pub modules: Vec<Source>,
}

thread_local! {
    static PARSER: RefCell<tree_sitter::Parser> = RefCell::new(new_parser());
}

fn new_parser() -> tree_sitter::Parser {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("the Rust grammar loads");
    parser
}

/// The one Rust parse: facts' modules and the provers' comment reading.
pub fn parse(source: &[u8]) -> Option<Tree> {
    PARSER.with(|p| p.borrow_mut().parse(source, None))
}

/// Item node kind to `RsSymbol.kind`.
pub const ITEM_KINDS: [(&str, &str); 9] = [
    ("function_item", "function"),
    ("struct_item", "struct"),
    ("union_item", "struct"),
    ("enum_item", "enum"),
    ("trait_item", "trait"),
    ("type_item", "type"),
    ("const_item", "const"),
    ("static_item", "static"),
    ("macro_definition", "macro"),
];

/// A `.md` or `.rst` file as `doc_files` holds it.
fn read_doc(path: &Utf8Path) -> Vec<String> {
    let text = walk::read_text(path).map_or_else(String::new, |(text, _)| text);
    pytext::splitlines(&text)
        .iter()
        .map(|l| l.to_string())
        .collect()
}

/// The first ERROR or missing node's line: what the parse could not read.
fn error_line(root: Node<'_>) -> u32 {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            return node.start_position().row as u32 + 1;
        }
        if node.has_error() {
            stack.extend(children(node).into_iter().rev());
        }
    }
    1
}

/// What the walk gives the build before a module is indexed.
pub struct Prep {
    root: Utf8PathBuf,
    config: Config,
    all_files: Vec<Rel>,
    doc_files: IndexMap<Rel, Vec<String>>,
    crates: IndexMap<String, String>,
    module_qnames: Vec<String>,
    errors: Vec<String>,
    listing: Listing,
    /// every `Cargo.toml`, parsed once for `crate_roots` and `lib_crates`
    manifests: Vec<(String, toml::Table)>,
}

/// `only` restricts the build to the given rel paths (single-file facts for
/// the gate): qnames come from the path layout either way, so a single-file
/// build's symbols, refs and comments equal the full build's. Call
/// resolution, the aliases and `published` read the repo-wide table.
pub fn build_facts(
    root: &Utf8Path,
    config: &Config,
    listing: &Listing,
    only: Option<&IndexSet<Rel>>,
) -> RsBuilt {
    let (tree, prep) = parse_tree(root, config, listing, only);
    RsBuilt::new(tree, |tree| index_tree(tree, prep))
}

/// The read and the parse, before any index: the seam a stack that owns
/// facts and provers in one cell builds through.
pub fn parse_tree(
    root: &Utf8Path,
    config: &Config,
    listing: &Listing,
    only: Option<&IndexSet<Rel>>,
) -> (RsTree, Prep) {
    let manifests = read_manifests(listing);
    let crates = crate_roots(root, &manifests);
    let qname_map = module_qname_map(&crates, listing);
    let mut prep = Prep {
        root: root.to_path_buf(),
        config: config.clone(),
        all_files: Vec::new(),
        doc_files: IndexMap::new(),
        module_qnames: qname_map.values().cloned().collect(),
        crates,
        errors: Vec::new(),
        listing: listing.clone(),
        manifests,
    };

    let mut candidates: Vec<(&Utf8PathBuf, &String)> = Vec::new();
    for (path, rel) in listing {
        if only.is_some_and(|set| !set.contains(rel.as_str())) {
            continue;
        }
        prep.all_files.push(rel.as_str().into());
        if rel.ends_with(".md") || rel.ends_with(".rst") {
            prep.doc_files.insert(rel.as_str().into(), read_doc(path));
        } else if rel.ends_with(SUFFIX) {
            candidates.push((path, rel));
        }
    }

    let read: Vec<Read> = candidates
        .par_iter()
        .map(|(path, _)| {
            let bytes = std::fs::read(path.as_std_path()).ok()?;
            let tree = parse(&bytes)?;
            let lossy = match std::str::from_utf8(&bytes) {
                Ok(_) => None,
                Err(_) => Some(String::from_utf8_lossy(&bytes).into_owned()),
            };
            Some((bytes, lossy, tree))
        })
        .collect();

    let mut modules: Vec<Source> = Vec::with_capacity(candidates.len());
    for ((path, rel), slot) in candidates.into_iter().zip(read) {
        let Some((bytes, lossy, tree)) = slot else {
            continue;
        };
        if tree.root_node().has_error() {
            let line = error_line(tree.root_node());
            prep.errors
                .push(format!("{rel}: parse error (line {line})"));
        }
        modules.push(Source {
            rel: rel.as_str().into(),
            path: path.to_path_buf(),
            qname: qname_map[rel].as_str().into(),
            bytes,
            lossy,
            tree,
        });
    }
    (RsTree { modules }, prep)
}

/// What one module's item pass produced: each symbol with the trait whose
/// impl block defined it, if any. `seen` is the merge `_add` would have done
/// already, so a qname spelled twice in one module keeps the first symbol
/// and a `fn` body indexes under that symbol's test reading.
#[derive(Default)]
struct ItemPass<'t> {
    adds: Vec<(RsSymbol<'t>, Option<String>)>,
    seen: HashMap<Qname, bool>,
    impls: Vec<RsImpl<'t>>,
    notes: Vec<String>,
    pub_mods: IndexSet<String>,
}

pub fn index_tree<'t>(tree: &'t RsTree, prep: Prep) -> RsFacts<'t> {
    let mut facts = RsFacts {
        root: prep.root.clone(),
        config: prep.config.clone(),
        ..Default::default()
    };
    facts.all_files = prep.all_files;
    facts.doc_files = prep.doc_files;
    facts.crates = prep.crates;
    facts.errors = prep.errors;

    let mut modules: IndexMap<Qname, RsModule<'t>> = IndexMap::with_capacity(tree.modules.len());
    for source in &tree.modules {
        let text = source.text();
        modules.insert(
            source.qname.clone(),
            RsModule {
                qname: source.qname.clone(),
                rel: source.rel.clone(),
                path: source.path.clone(),
                source: text,
                bytes: &source.bytes,
                lines: source_lines(text),
                crate_name: source.qname.split("::").next().unwrap_or("").to_string(),
                root: source.tree.root_node(),
                bindings: IndexMap::new(),
                items: IndexMap::new(),
                reexports: Vec::new(),
                pub_mods: IndexSet::new(),
                comments: Vec::new(),
                doc: Vec::new(),
            },
        );
    }

    // pass A: items, then the scope and the `use` bindings each module holds
    facts.modules = modules;
    let declared = declared_cfgs(&facts);
    // per module under rayon, merged below in `facts.modules` order, as
    // pass B is
    let passes: Vec<ItemPass<'t>> = facts
        .modules
        .values()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|module| {
            let mut pass = ItemPass::default();
            let inherited = declared.get(&module.qname).cloned().unwrap_or_default();
            index_items(
                &mut pass,
                module,
                module.root,
                &module.qname,
                None,
                is_test_path(&module.rel),
                &inherited,
            );
            pass
        })
        .collect();
    for (i, pass) in passes.into_iter().enumerate() {
        facts.notes.extend(pass.notes);
        facts.impls.extend(pass.impls);
        for (sym, trait_name) in pass.adds {
            add_symbol(&mut facts.symbols, sym, trait_name);
        }
        facts.modules[i].pub_mods = pass.pub_mods;
    }

    for (i, sym) in facts.symbols.values().enumerate() {
        facts
            .symbols_by_module
            .entry(sym.module.clone())
            .or_default()
            .push(i as u32);
    }
    let mut methods: IndexMap<String, Vec<Qname>> = IndexMap::new();
    for (q, sym) in &facts.symbols {
        if sym.kind == "method" {
            methods.entry(sym.name.clone()).or_default().push(q.clone());
        }
    }
    for (name, mut qs) in methods {
        qs.sort();
        facts.methods_by_name.insert(name, qs);
    }

    let scoped: Vec<Scope> = facts
        .modules
        .values()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|m| collect_scope(&facts, m, &prep.module_qnames))
        .collect();
    for (i, (items, bindings, reexports)) in scoped.into_iter().enumerate() {
        let module = &mut facts.modules[i];
        module.items = items;
        module.bindings = bindings;
        module.reexports = reexports;
    }
    // every module's re-exports, before pass B
    facts.aliases = aliases(&facts);

    // pass B per module under rayon, appended in `facts.modules` order
    let walked: Vec<WalkOut<'t>> = facts
        .modules
        .values()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|m| walk_module(&facts, m))
        .collect();
    for (i, out) in walked.into_iter().enumerate() {
        facts.refs.extend(out.refs);
        facts.call_sites.extend(out.call_sites);
        let module = &mut facts.modules[i];
        module.doc = out
            .comments
            .iter()
            .filter(|c| c.kind == "module-doc")
            .map(|c| c.text.clone())
            .collect();
        module.comments = out.comments;
    }
    for (i, r) in facts.refs.iter().enumerate() {
        facts
            .refs_to
            .entry(r.target.clone())
            .or_default()
            .push(i as u32);
    }
    facts.published = published(&facts, &prep.listing, &prep.manifests);
    facts.module_by_rel = facts
        .modules
        .values()
        .map(|m| (m.rel.clone(), m.qname.clone()))
        .collect();
    facts.fan_in = fan_in(&facts);
    facts.cc = complexity_prior(&facts);
    facts
}

/// Inbound cross-module refs per module (the render rollup's module line).
fn fan_in(facts: &RsFacts<'_>) -> HashMap<Qname, u32> {
    let mut out: HashMap<Qname, u32> = HashMap::new();
    for (target, rows) in &facts.refs_to {
        let Some(sym) = facts.symbols.get(target.as_str()) else {
            continue;
        };
        for i in rows {
            if facts.refs[*i as usize].module != sym.module {
                *out.entry(sym.module.clone()).or_default() += 1;
            }
        }
    }
    out
}

/// R20's memo: the ranking prior, computed once so facts stay immutable.
fn complexity_prior(facts: &RsFacts<'_>) -> HashMap<Qname, u32> {
    let defs: Vec<(&Qname, &RsSymbol<'_>)> = facts
        .symbols
        .iter()
        .filter(|(_, s)| is_fn_kind(s.kind))
        .collect();
    defs.par_iter()
        .map(|(qname, sym)| {
            let src = facts.modules[&sym.module].bytes;
            ((*qname).clone(), cognitive_complexity(sym.node, 0, src))
        })
        .collect()
}
