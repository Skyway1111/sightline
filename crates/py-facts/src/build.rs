//! `facts/build.py:build_facts`: read the listing, parse under rayon, then
//! the passes in module order.
//!
//! `only` restricts the build to the given rel paths (the gate's
//! single-file facts). Module qnames come from `qnames::module_qname_map`
//! either way, so a single-file build and a full build agree.

use std::collections::HashMap;

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::{IndexMap, IndexSet};
use rayon::prelude::*;
use ruff_python_ast::PySourceType;
use ruff_python_parser::Parsed;
use sightline_core::config::Config;
use sightline_core::findings::{Qname, Rel};
use sightline_core::lang::Listing;
use sightline_core::pytext;
use sightline_core::walk::read_text;

use crate::cn::Cn;
use crate::index::{self, ClassOp, SymbolOp};
use crate::lines::Lines;
use crate::model::{RepoFacts, Symbol, source_lines};
use crate::module::{Module, Source, Tree};
use crate::{complexity, inputs, qnames, resolve, typecomments};

mod built {
    // the one self-referential struct of the Python stack (decision 7)
    #![allow(unsafe_code)]

    use super::{RepoFacts, Tree};

    self_cell::self_cell!(
        /// The parsed tree and the facts borrowing it, owned together.
        pub struct PyBuilt {
            owner: Tree,
            #[covariant]
            dependent: RepoFacts,
        }
    );
}

pub use built::PyBuilt;

/// The reference parses with CPython 3.14 (decision 11), and `Parsed`
/// counts version-related syntax as an error: at ruff's default `PY310` a
/// `type X = ...` statement makes its module a parse error.
fn options() -> ruff_python_parser::ParseOptions {
    ruff_python_parser::ParseOptions::from(PySourceType::Python)
        .with_target_version(ruff_python_ast::PythonVersion::PY314)
}

/// The file's own bytes as lines that keep their terminators, without the
/// newline translation `read_text` applies. `emit`'s diff has to match what
/// sits on disk, so `git apply` finds the context lines it names. Invalid
/// UTF-8 is replaced, as the reference reads it.
pub fn raw_lines(path: &Utf8Path) -> Vec<String> {
    let bytes = std::fs::read(path).unwrap_or_default();
    String::from_utf8_lossy(&bytes)
        .split_inclusive('\n')
        .map(str::to_string)
        .collect()
}

/// Does this text parse as a module? `emit` asks it of the patch it wrote:
/// two deletions can meet and leave a block with no body.
pub fn parses(text: &str) -> bool {
    !ruff_python_parser::parse_unchecked(text, options()).has_syntax_errors()
}

/// R14: the first syntax error as facts spell it.
fn parse_error(rel: &str, parsed: &Parsed<ruff_python_ast::ModModule>, lines: &Lines) -> String {
    match parsed.errors().first() {
        Some(e) => format!(
            "{rel}: {} (line {})",
            e.error,
            lines.pos(e.location.start().to_u32()).0
        ),
        None => format!("{rel}: syntax error (line 1)"),
    }
}

/// What the walk and the repo-wide inputs give the build before a module is
/// indexed.
struct Prep {
    root: Utf8PathBuf,
    config: Config,
    all_files: Vec<Rel>,
    doc_files: IndexMap<Rel, Vec<String>>,
    entry_points: Vec<String>,
    typed_scope: Vec<String>,
    import_roots: Vec<Utf8PathBuf>,
    errors: Vec<String>,
}

/// Discover, parse and index a repo.
pub fn build_facts(
    root: &Utf8Path,
    config: &Config,
    listing: &Listing,
    only: Option<&IndexSet<Rel>>,
) -> PyBuilt {
    let mut prep = Prep {
        root: root.to_path_buf(),
        config: config.clone(),
        all_files: Vec::new(),
        doc_files: IndexMap::new(),
        entry_points: inputs::entry_points(root),
        typed_scope: inputs::typed_scope(root),
        import_roots: qnames::import_roots(root, listing),
        errors: Vec::new(),
    };
    let qname_map = qnames::module_qname_map(root, listing);

    let mut candidates: Vec<(&Utf8PathBuf, &String)> = Vec::new();
    for (path, rel) in listing {
        if only.is_some_and(|set| !set.contains(rel.as_str())) {
            continue;
        }
        prep.all_files.push(rel.as_str().into());
        if rel.ends_with(".md") || rel.ends_with(".rst") {
            let text = read_text(path).map(|(t, _)| t).unwrap_or_default();
            prep.doc_files.insert(
                rel.as_str().into(),
                pytext::splitlines(&text)
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            );
            continue;
        }
        if rel.ends_with(".py") {
            candidates.push((path, rel));
        }
    }

    type Read = Option<(String, bool, Parsed<ruff_python_ast::ModModule>)>;
    let read: Vec<Read> = candidates
        .par_iter()
        .map(|(path, _)| {
            let (text, lossy) = read_text(path)?;
            let parsed = ruff_python_parser::parse_unchecked(&text, options())
                .try_into_module()
                .expect("a module parse answers a module");
            Some((text, lossy, parsed))
        })
        .collect();

    let mut modules: Vec<Source> = Vec::with_capacity(candidates.len());
    for ((path, rel), slot) in candidates.into_iter().zip(read) {
        let Some((text, lossy, parsed)) = slot else {
            continue;
        };
        if parsed.has_syntax_errors() {
            prep.errors
                .push(parse_error(rel, &parsed, &Lines::new(&text)));
            continue;
        }
        modules.push(Source {
            rel: rel.as_str().into(),
            path: path.to_path_buf(),
            qname: qname_map[rel].as_str().into(),
            text,
            parsed,
            lossy,
        });
    }

    let published = inputs::published(
        root,
        config,
        listing,
        modules.iter().map(|s| (&s.qname, &*s.rel)),
    );
    PyBuilt::new(Tree { modules }, |tree| index_tree(tree, prep, published))
}

fn index_tree<'t>(
    tree: &'t Tree,
    prep: Prep,
    published: std::collections::HashSet<Qname>,
) -> RepoFacts<'t> {
    let mut facts = RepoFacts::new(prep.root, prep.config);
    facts.all_files = prep.all_files;
    facts.doc_files = prep.doc_files;
    facts.entry_points = prep.entry_points;
    facts.typed_scope = prep.typed_scope;
    facts.import_roots = prep.import_roots;
    facts.errors = prep.errors;
    facts.published = published;

    let lines: Vec<Lines> = tree
        .modules
        .par_iter()
        .map(|s| Lines::new(&s.text))
        .collect();
    let passes: Vec<index::PassA<'t>> = tree
        .modules
        .par_iter()
        .zip(&lines)
        .map(|(source, lines)| index::pass_a(source, lines))
        .collect();

    for ((source, pass), line_index) in tree.modules.iter().zip(passes).zip(&lines) {
        let mut module = Module {
            // stamped from the insert below
            id: 0,
            qname: source.qname.clone(),
            rel: source.rel.clone(),
            path: source.path.clone(),
            source: &source.text,
            lines: source_lines(&source.text),
            parsed: &source.parsed,
            nodes: pass.traversal.nodes,
            spans: pass.traversal.spans,
            parent: pass.traversal.parent,
            enclosing: pass.traversal.enclosing,
            nodes_by_scope: pass.traversal.nodes_by_scope,
            bindings: IndexMap::new(),
            all_names: None,
            dynamic_all: false,
            type_annotations: Default::default(),
            standalone_comments: crate::model::standalone_comments(
                &pass.comments,
                &source_lines(&source.text),
            ),
            comments: pass.comments,
            lossy: source.lossy,
            scope_keys: Default::default(),
        };
        let body = &source.parsed.syntax().body;
        let mut all_names = None;
        let mut dynamic = false;
        index::extract_all(body, &mut all_names, &mut dynamic);
        module.all_names = all_names;
        module.dynamic_all = dynamic;
        index::collect_bindings(&mut module, body);
        module.type_annotations = typecomments::annotations(&module, line_index);
        module.sort_scope_keys();
        let at = facts.modules.insert_full(source.qname.clone(), module).0;
        facts.modules[at].id = at as u32;
    }

    let scoped: Vec<index::ScopeIndex> = facts
        .modules
        .values()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|m| index::index_scope(m))
        .collect();
    for scope in scoped {
        for op in scope.symbols {
            match op {
                SymbolOp::Set(sym) => {
                    facts.symbols.insert(sym.qname.clone(), sym);
                }
                SymbolOp::Default(sym) => {
                    facts.symbols.entry(sym.qname.clone()).or_insert(sym);
                }
            }
        }
        for op in scope.classes {
            match op {
                ClassOp::Define(info) => {
                    facts.classes.insert(info.qname.clone(), info);
                }
                ClassOp::Method {
                    class_q,
                    name,
                    method_q,
                } => {
                    if let Some(info) = facts.classes.get_mut(&class_q) {
                        info.methods.insert(name, method_q);
                    }
                }
            }
        }
    }

    index::link_subclasses(&mut facts);
    index::method_index(&mut facts);

    // pass B per module under rayon, appended in `facts.modules` order
    let resolved: Vec<resolve::Resolved> = facts
        .modules
        .values()
        .collect::<Vec<_>>()
        .par_iter()
        .map(|m| resolve::resolve(m, &facts))
        .collect();
    for module in resolved {
        facts.refs.extend(module.refs);
        facts.call_sites.extend(module.call_sites);
    }

    for (i, call) in facts.call_sites.iter().enumerate() {
        let module = facts.modules[&call.module].id;
        facts.call_index.insert((module, call.node), i as u32);
    }
    for (i, r) in facts.refs.iter().enumerate() {
        facts
            .refs_to
            .entry(r.target.clone())
            .or_default()
            .push(i as u32);
    }
    for (i, sym) in facts.symbols.values().enumerate() {
        facts
            .symbols_by_module
            .entry(sym.module.clone())
            .or_default()
            .push(i as u32);
    }
    facts.cc = complexity_prior(&facts);
    facts.close_indexes();
    facts
}

/// R20's memo: the ranking prior for every function symbol, computed once at
/// build. Phase 3's `Provers` reads it rather than the AST.
fn complexity_prior(facts: &RepoFacts<'_>) -> HashMap<Qname, u32> {
    let defs: Vec<(&Qname, &Symbol)> = facts
        .symbols
        .iter()
        .filter(|(_, s)| crate::model::FUNCTION_KINDS.contains(&s.kind))
        .collect();
    defs.par_iter()
        .map(|(qname, sym)| {
            let cc = match facts.modules[&sym.module].nodes[sym.node as usize] {
                Cn::Stmt(ruff_python_ast::Stmt::FunctionDef(def)) => {
                    complexity::cognitive_complexity(def, 0)
                }
                _ => 0,
            };
            ((*qname).clone(), cc)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sightline_core::walk;

    /// A module at the root folds to the name a package of that name already
    /// holds, and the fallback folded to it again: the second insert replaced
    /// the first in place, leaving the map a module short and every id past
    /// the gap one above its place. `Scope::module` reads an id as a place.
    #[test]
    fn a_module_id_is_its_place_in_the_map() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(tmp.path()).unwrap();
        for (rel, text) in [
            ("pkg/__init__.py", ""),
            ("pkg/mod.py", "def g(a):\n    return a\n"),
            ("pkg.py", "def f(a, b):\n    return a\n"),
        ] {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
        }
        let config = Config::new();
        let built = build_facts(root, &config, &walk::discover(root, &config), None);
        let facts = built.borrow_dependent();
        let held: Vec<(&str, &str)> = facts.modules.iter().map(|(q, m)| (&**q, &*m.rel)).collect();
        assert_eq!(
            held,
            [
                ("pkg", "pkg/__init__.py"),
                ("pkg.mod", "pkg/mod.py"),
                ("pkg.py", "pkg.py"),
            ]
        );
        for (at, m) in facts.modules.values().enumerate() {
            assert_eq!(m.id as usize, at, "{} took id {}", m.rel, m.id);
        }
    }
}
