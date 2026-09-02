//! The arenas and their indexes. No opinions, no oracle.
//!
//! Facts are a `Tree` of parsed modules owned by the stack and a `RepoFacts`
//! borrowing it. Every node of a module has a dense
//! `NodeIndex` in traversal order; `parent`, `enclosing` and `spans` are
//! `Vec`s on that index, and no reader recomputes a position.

use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf;
use indexmap::IndexMap;
use ruff_python_ast::ModModule;
use ruff_python_parser::Parsed;
use sightline_core::config::Config;
use sightline_core::findings::{Qname, Rel};
use sightline_core::pytext;

use crate::lines::Lines;
use crate::module::{Comment, Module};

/// A node's place in its module's traversal order.
pub type NodeIndex = u32;
pub type ModuleId = u32;
pub type SymbolId = u32;
pub type RefId = u32;
pub type CallSiteId = u32;
/// A scope's place in `Module::nodes_by_scope`; 0 is the module's own.
pub type ScopeId = u32;

/// CPython's `(lineno, col_offset, end_lineno, end_col_offset)`: 1-based
/// line, UTF-8 byte column (R1). A cell is `None` where the class has no
/// such field, which only `TypeIgnore` reaches.
pub type Span = [Option<u32>; 4];

/// `Symbol.kind` values a def backs.
pub const FUNCTION_KINDS: [&str; 2] = ["function", "method"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    /// the name is the function of a Call
    Callee,
    /// any other read (argument, assignment right side, decorator)
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
    /// plain-receiver CHA: a method-name match, never typed evidence
    ByName,
    /// CHA found more than one override candidate
    Ambiguous,
    /// resolves outside the repo (import or builtin)
    External,
    /// taints whole-program analysis
    Unresolved,
}

impl Resolution {
    // sightline-ok: 11 - an enum's match table is its own name
    pub fn value(self) -> &'static str {
        match self {
            Resolution::Resolved => "resolved",
            Resolution::ByName => "by-name",
            Resolution::Ambiguous => "ambiguous",
            Resolution::External => "external",
            Resolution::Unresolved => "unresolved",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub qname: Qname,
    pub module: Qname,
    pub name: Box<str>,
    /// "function" | "class" | "method" | "variable"
    pub kind: &'static str,
    pub node: NodeIndex,
    pub lineno: u32,
    /// last line of the definition, 0 where the node has none
    pub end_lineno: u32,
    pub is_public: bool,
    /// enclosing class or function qname, `None` at module level
    pub parent: Option<Qname>,
}

#[derive(Debug, Clone)]
pub struct Ref {
    pub module: Qname,
    pub node: NodeIndex,
    pub target: Qname,
    pub kind: RefKind,
}

#[derive(Debug, Clone)]
pub struct CallSite {
    pub module: Qname,
    pub node: NodeIndex,
    /// qname of the enclosing symbol (the module qname at top level)
    pub enclosing: Qname,
    pub resolution: Resolution,
    pub target: Option<Qname>,
    /// CHA candidates when ambiguous, the oracle's external homes when external
    pub candidates: Vec<Qname>,
    pub lineno: u32,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub qname: Qname,
    pub module: Qname,
    pub node: NodeIndex,
    /// internal base qnames
    pub bases: Vec<Qname>,
    /// non-repo base qnames through the module's bindings, the expression
    /// text where the root is unbound
    pub external_bases: Vec<String>,
    /// name -> method qname
    pub methods: IndexMap<Box<str>, Qname>,
    pub subclasses: Vec<Qname>,
}

pub struct RepoFacts<'t> {
    pub root: Utf8PathBuf,
    pub config: Config,
    pub modules: IndexMap<Qname, Module<'t>>,
    pub symbols: IndexMap<Qname, Symbol>,
    pub classes: IndexMap<Qname, ClassInfo>,
    pub refs: Vec<Ref>,
    pub call_sites: Vec<CallSite>,
    pub errors: Vec<String>,
    /// rel -> lines
    pub doc_files: IndexMap<Rel, Vec<String>>,
    /// every file the walk kept
    pub all_files: Vec<Rel>,
    /// pyproject entry points, as the `pkg.mod:obj` strings they name
    pub entry_points: Vec<String>,
    /// module qnames a distribution in the tree packages or its docs publish;
    /// empty means an application, whose every caller is in this tree
    pub published: HashSet<Qname>,
    /// the paths an import resolves against: root, `src/`, workspace members
    pub import_roots: Vec<Utf8PathBuf>,
    /// path segments of the repo's declared type-check scope
    pub typed_scope: Vec<String>,
    pub refs_to: HashMap<Qname, Vec<RefId>>,
    pub symbols_by_module: HashMap<Qname, Vec<SymbolId>>,
    pub call_index: HashMap<(ModuleId, NodeIndex), CallSiteId>,
    /// method name -> the repo qnames spelling it (CHA), read by pass B
    pub method_index: HashMap<Box<str>, Vec<Qname>>,
    /// a production `__getattr__`, so any attribute name on a plain receiver
    /// may reach a repo body
    pub proxied: bool,
    /// R20's `cc_prior` memo: cognitive complexity per function symbol
    pub cc: HashMap<Qname, u32>,
    module_by_rel: HashMap<Rel, Qname>,
    classes_by_name: HashMap<Box<str>, Vec<Qname>>,
}

impl<'t> RepoFacts<'t> {
    pub fn new(root: Utf8PathBuf, config: Config) -> RepoFacts<'t> {
        RepoFacts {
            root,
            config,
            modules: IndexMap::new(),
            symbols: IndexMap::new(),
            classes: IndexMap::new(),
            refs: Vec::new(),
            call_sites: Vec::new(),
            errors: Vec::new(),
            doc_files: IndexMap::new(),
            all_files: Vec::new(),
            entry_points: Vec::new(),
            published: HashSet::new(),
            import_roots: Vec::new(),
            typed_scope: Vec::new(),
            refs_to: HashMap::new(),
            symbols_by_module: HashMap::new(),
            call_index: HashMap::new(),
            method_index: HashMap::new(),
            proxied: false,
            cc: HashMap::new(),
            module_by_rel: HashMap::new(),
            classes_by_name: HashMap::new(),
        }
    }

    /// The indexes every reader shares, built once when the passes are done.
    pub(crate) fn close_indexes(&mut self) {
        self.module_by_rel = self
            .modules
            .values()
            .map(|m| (m.rel.clone(), m.qname.clone()))
            .collect();
        let mut by_name: HashMap<Box<str>, Vec<Qname>> = HashMap::new();
        for q in self.classes.keys() {
            let bare = q.rsplit('.').next().unwrap_or(q);
            by_name.entry(bare.into()).or_default().push(q.clone());
        }
        self.classes_by_name = by_name;
    }

    pub fn module_by_rel(&self, rel: &str) -> Option<&Module<'t>> {
        self.modules.get(self.module_by_rel.get(rel)?)
    }

    /// Bare name -> the repo classes spelled so. An oracle display name is
    /// bare, so two homes make it no one's.
    pub fn classes_by_name(&self, name: &str) -> &[Qname] {
        self.classes_by_name.get(name).map_or(&[], |v| v)
    }

    /// The path of the file a symbol, ref or call site lives in.
    pub fn rel_of(&self, module: &str) -> Option<&Rel> {
        self.modules.get(module).map(|m| &m.rel)
    }

    pub fn enclosing(&self, module: &Module<'t>, node: NodeIndex) -> Qname {
        match module.enclosing[node as usize] {
            Some(scope) => scope_name(module, scope),
            None => module.qname.clone(),
        }
    }

    /// The def or class `node` sits in; `None` at module scope. A module
    /// qname can also name a symbol, so the module is never looked up as one.
    pub fn enclosing_symbol(&self, module: &Module<'t>, node: NodeIndex) -> Option<&Symbol> {
        let scope = module.enclosing[node as usize]?;
        self.symbols.get(&scope_name(module, scope))
    }

    /// A downstream user can reach `sym`: a public name of a published
    /// module, nested in classes only.
    pub fn publishes(&self, sym: &Symbol) -> bool {
        if !self.published.contains(&sym.module) || !sym.is_public {
            return false;
        }
        let mut parent = sym.parent.as_ref().and_then(|p| self.symbols.get(p));
        while let Some(p) = parent {
            if p.kind != "class" {
                return false;
            }
            parent = p.parent.as_ref().and_then(|q| self.symbols.get(q));
        }
        true
    }

    /// module qname -> (symbol, inbound cross-module refs) for each of its
    /// referenced symbols. #27 prices a module by them, `fan_in` sums them.
    pub fn inbound_refs(&self) -> IndexMap<Qname, Vec<(Qname, u32)>> {
        let mut out: IndexMap<Qname, Vec<(Qname, u32)>> = IndexMap::new();
        for (target, refs) in &self.refs_to {
            let Some(sym) = self.symbols.get(target) else {
                continue;
            };
            let n = refs
                .iter()
                .filter(|r| self.refs[**r as usize].module != sym.module)
                .count() as u32;
            if n > 0 {
                out.entry(sym.module.clone())
                    .or_default()
                    .push((target.clone(), n));
            }
        }
        out
    }

    /// Inbound cross-module refs per module, summed.
    pub fn fan_in(&self) -> HashMap<Qname, u32> {
        self.inbound_refs()
            .into_iter()
            .map(|(q, rows)| (q, rows.iter().map(|(_, n)| n).sum()))
            .collect()
    }
}

/// The qname of a scope by its place in `nodes_by_scope`.
fn scope_name(module: &Module<'_>, scope: ScopeId) -> Qname {
    module
        .nodes_by_scope
        .get_index(scope as usize)
        .map_or_else(|| module.qname.clone(), |(q, _)| q.clone())
}

/// `(qname, info)` for the class and every internal class `step` reaches.
/// `step` yields qnames rather than a slice, so a caller can walk both
/// directions at once (`closed_world`'s method-override arm).
pub fn class_chain<'a, 'f, I: IntoIterator<Item = Qname>>(
    facts: &'a RepoFacts<'f>,
    cls_q: &str,
    step: impl Fn(&'a ClassInfo) -> I,
) -> Vec<(Qname, &'a ClassInfo)> {
    let mut frontier: Vec<Qname> = vec![cls_q.into()];
    let mut seen: HashSet<Qname> = HashSet::new();
    let mut out = Vec::new();
    while let Some(q) = frontier.pop() {
        if seen.contains(&q) {
            continue;
        }
        let Some(info) = facts.classes.get(&q) else {
            continue;
        };
        seen.insert(q.clone());
        out.push((q, info));
        frontier.extend(step(info));
    }
    out
}

/// Which way `class_walk` steps through the class table.
#[derive(Debug, Clone, Copy)]
pub enum Step {
    Bases,
    Subclasses,
}

/// `base_chain` / `subclass_chain`: `class_chain` one way.
pub fn class_walk<'a, 'f>(
    facts: &'a RepoFacts<'f>,
    cls_q: &str,
    step: Step,
) -> Vec<(Qname, &'a ClassInfo)> {
    class_chain(facts, cls_q, |i| {
        match step {
            Step::Bases => &i.bases,
            Step::Subclasses => &i.subclasses,
        }
        .iter()
        .cloned()
    })
}

pub use sightline_core::pytext::source_lines;

/// The one reading of "what is a test file": a `tests`, `test` or `testing`
/// dir on the path, a `test_*.py` or `*_test.py` name, a conftest.
pub fn is_test_path(rel: &str) -> bool {
    let mut parts: Vec<&str> = rel.split('/').collect();
    let name = parts.pop().unwrap_or("");
    parts
        .iter()
        .any(|d| matches!(*d, "tests" | "test" | "testing"))
        || name.starts_with("test_")
        || name.ends_with("_test.py")
        || name == "conftest.py"
}

/// An external base past `object`, typing markers included: a contract the
/// repo never sees.
pub fn has_framework_base(info: &ClassInfo) -> bool {
    info.external_bases
        .iter()
        .any(|b| b != "object" && b != "builtins.object")
}

/// `has_framework_base` up the internal base chain.
pub fn has_framework_base_transitive(facts: &RepoFacts<'_>, cls_q: &str) -> bool {
    class_walk(facts, cls_q, Step::Bases)
        .iter()
        .any(|(_, info)| has_framework_base(info))
}

/// The comment tokens of a parsed module, with the code-point column
/// `tokenize` reports and the text as written.
pub(crate) fn comments(parsed: &Parsed<ModModule>, source: &str, lines: &Lines) -> Vec<Comment> {
    use ruff_python_ast::token::TokenKind;
    use ruff_text_size::Ranged;

    parsed
        .tokens()
        .iter()
        .filter(|t| t.kind() == TokenKind::Comment)
        .map(|t| {
            let start = t.range().start().to_u32();
            let (line, byte_col) = lines.pos(start);
            let line_start = (start - byte_col) as usize;
            Comment {
                line,
                col: source[line_start..start as usize].chars().count() as u32,
                text: source[t.range()].into(),
            }
        })
        .collect()
}

/// Lines whose comment owns the line. #34 reads runs of them, #39 judges
/// them against the next code line rather than their own.
pub(crate) fn standalone_comments(comments: &[Comment], lines: &[&str]) -> HashSet<u32> {
    comments
        .iter()
        .filter(|c| {
            lines
                .get(c.line as usize - 1)
                .is_some_and(|l| pytext::lstrip(l).starts_with('#'))
        })
        .map(|c| c.line)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_count_newlines_alone_like_the_ast() {
        // `str.splitlines` breaks at \x0c and U+2028; the AST counts \n alone
        assert_eq!(
            source_lines("a\x0cb\nc\u{2028}d\n"),
            ["a\x0cb", "c\u{2028}d"]
        );
        assert_eq!(source_lines(""), Vec::<&str>::new());
        assert_eq!(source_lines("a"), ["a"]);
        assert_eq!(source_lines("a\n\n"), ["a", ""]);
    }

    #[test]
    fn test_paths_are_dirs_names_and_conftest() {
        assert!(is_test_path("tests/x.py"));
        assert!(is_test_path("a/testing/x.py"));
        assert!(is_test_path("src/test_x.py"));
        assert!(is_test_path("src/x_test.py"));
        assert!(is_test_path("conftest.py"));
        assert!(!is_test_path("src/x.py"));
        // a file named `tests.py` is not a directory named `tests`
        assert!(!is_test_path("tests.py"));
    }
}
