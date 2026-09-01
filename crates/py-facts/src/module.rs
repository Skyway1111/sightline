//! One parsed module and its arenas (`facts/model.py:Module`): the source,
//! the tree, the dense node index with its side tables, the bindings.

use crate::cn::Cn;
use crate::kinds::Kind;
use crate::model::{ModuleId, NodeIndex, ScopeId, Span};
use camino::Utf8PathBuf;
use indexmap::IndexMap;
use ruff_python_ast::{Expr, ModModule};
use ruff_python_parser::Parsed;
use sightline_core::findings::{Qname, Rel};
use std::collections::{HashMap, HashSet};

/// One source file as read and parsed, the arena facts borrow.
pub struct Source {
    pub rel: Rel,
    pub path: Utf8PathBuf,
    pub qname: Qname,
    pub text: String,
    pub parsed: Parsed<ModModule>,
    /// non-UTF-8 bytes read as U+FFFD, so byte columns here are no one else's
    pub lossy: bool,
}

/// What the stack owns; `RepoFacts` borrows it.
pub struct Tree {
    pub modules: Vec<Source>,
}

/// A comment token: 1-based line, code-point column, the text as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub line: u32,
    pub col: u32,
    pub text: Box<str>,
}

pub struct Module<'t> {
    /// this module's place in `RepoFacts::modules`, which `Scope::module`
    /// reads back with `get_index`; `build::index_tree` stamps it from the
    /// insert, so the two never part
    pub id: ModuleId,
    pub qname: Qname,
    pub rel: Rel,
    pub path: Utf8PathBuf,
    pub source: &'t str,
    /// R2: split on `\n` alone, one trailing empty element dropped
    pub lines: Vec<&'t str>,
    pub parsed: &'t Parsed<ModModule>,
    /// one entry per CPython node, in traversal order
    pub nodes: Vec<Cn<'t>>,
    pub spans: Vec<Option<Span>>,
    pub parent: Vec<Option<NodeIndex>>,
    /// the scope each node sits in, `None` at module scope
    pub enclosing: Vec<Option<ScopeId>>,
    /// scopes in first-visit order, kinds within a scope in first-visit
    /// order (R5)
    pub nodes_by_scope: IndexMap<Qname, Vec<(Kind, Vec<NodeIndex>)>>,
    /// local name -> qname
    pub bindings: IndexMap<Box<str>, Qname>,
    /// the literal `__all__`, where the module spells one
    pub all_names: Option<Vec<Box<str>>>,
    pub dynamic_all: bool,
    /// R15: the annotation a `# type:` comment spells, by parameter or def
    pub type_annotations: HashMap<NodeIndex, Expr>,
    pub comments: Vec<Comment>,
    /// lines whose comment owns the line
    pub standalone_comments: HashSet<u32>,
    pub lossy: bool,
    pub(crate) scope_keys: Vec<Qname>,
}

impl<'t> Module<'t> {
    /// `nodes_by_scope` keys sorted, which the `nested` run bisects.
    pub(crate) fn sort_scope_keys(&mut self) {
        let mut keys: Vec<Qname> = self.nodes_by_scope.keys().cloned().collect();
        keys.sort();
        self.scope_keys = keys;
    }

    pub fn rel_import_base(&self, level: u32, module: Option<&str>) -> String {
        if level == 0 {
            return module.unwrap_or("").to_string();
        }
        let mut parts: Vec<&str> = self.qname.split('.').collect();
        if !self.rel.ends_with("__init__.py") {
            parts.pop();
        }
        // Python slices `parts[: len - level + 1]`, and a negative bound
        // there counts from the end rather than clamping at zero.
        let n = parts.len() as i64 - level as i64 + 1;
        let keep = if n < 0 {
            (parts.len() as i64 + n).max(0) as usize
        } else {
            (n as usize).min(parts.len())
        };
        let base = parts[..keep].join(".");
        [base.as_str(), module.unwrap_or("")]
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(".")
    }

    /// Nodes of exactly these kinds, document order. `scope` names one
    /// symbol's own scope (a nested def is its own scope; a lambda and a
    /// comprehension are not); `nested` adds the descendant scopes (R5).
    pub fn nodes(&self, kinds: &[Kind], scope: Option<&str>, nested: bool) -> Vec<NodeIndex> {
        let buckets: Vec<&Vec<(Kind, Vec<NodeIndex>)>> = match scope {
            None => self.nodes_by_scope.values().collect(),
            // the scope and its descendants are a run of sorted keys, since
            // "/" follows "." in ASCII
            Some(scope) if nested => {
                let lo = self.scope_keys.partition_point(|k| &**k < scope);
                let bound = format!("{scope}/");
                let hi = self.scope_keys.partition_point(|k| &**k < bound.as_str());
                let dotted = format!("{scope}.");
                self.scope_keys[lo..hi]
                    .iter()
                    .filter(|k| &***k == scope || k.starts_with(&dotted))
                    .filter_map(|k| self.nodes_by_scope.get(k))
                    .collect()
            }
            Some(scope) => self.nodes_by_scope.get(scope).into_iter().collect(),
        };
        let mut out = Vec::new();
        for bucket in buckets {
            for kind in kinds {
                if let Some((_, list)) = bucket.iter().find(|(k, _)| k == kind) {
                    out.extend(list.iter().copied());
                }
            }
        }
        out
    }

    /// R15: a parameter's declared type, its own annotation first and the
    /// one a `# type:` comment spells second. The one reader of a
    /// parameter's type; nothing touches `Parameter.annotation` directly.
    pub fn annotation(&self, param: NodeIndex) -> Option<&Expr> {
        if let Cn::Param(p) = self.nodes[param as usize]
            && let Some(own) = p.annotation.as_deref()
        {
            return Some(own);
        }
        self.type_annotations.get(&param)
    }

    /// R15: a def's declared return type, its own first.
    pub fn returns(&self, func: NodeIndex) -> Option<&Expr> {
        if let Cn::Stmt(ruff_python_ast::Stmt::FunctionDef(f)) = self.nodes[func as usize]
            && let Some(own) = f.returns.as_deref()
        {
            return Some(own);
        }
        self.type_annotations.get(&func)
    }

    /// The node holding `node`; `None` at the tree root.
    pub fn parent_of(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.parent[node as usize]
    }

    pub fn span(&self, node: NodeIndex) -> Option<Span> {
        self.spans[node as usize]
    }

    /// The call at this index, `None` for any other node. The one reader of
    /// a call node off an index, so no rule spells the match itself.
    pub fn call_at(&self, node: NodeIndex) -> Option<&'t ruff_python_ast::ExprCall> {
        match self.nodes[node as usize] {
            Cn::Expr(Expr::Call(c)) => Some(c),
            _ => None,
        }
    }

    /// The (first, last) line of a node the index stamped, `(0, 0)` for one
    /// it did not.
    pub fn lines_of(&self, node: Cn<'_>) -> (u32, u32) {
        node.stamped()
            .map_or((0, 0), |at| (self.line_of(at), self.end_line_of(at)))
    }

    /// 1-based line, 0 where the node has none.
    pub fn line_of(&self, node: NodeIndex) -> u32 {
        self.span_part(node, 0)
    }

    /// Last line of the node, 0 where it has none.
    pub fn end_line_of(&self, node: NodeIndex) -> u32 {
        self.span_part(node, 2)
    }

    fn span_part(&self, node: NodeIndex, part: usize) -> u32 {
        self.spans[node as usize].and_then(|s| s[part]).unwrap_or(0)
    }

    /// `a.b.c` through this module's import bindings; `None` when the root
    /// is unbound, so a local object never matches a catalog.
    pub fn dotted_name(&self, expr: &Expr) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        let mut cur = expr;
        while let Expr::Attribute(a) = cur {
            parts.push(a.attr.as_str());
            cur = &a.value;
        }
        let Expr::Name(name) = cur else {
            return None;
        };
        let base = self.bindings.get(name.id.as_str())?;
        parts.reverse();
        Some(
            std::iter::once(&**base)
                .chain(parts)
                .collect::<Vec<_>>()
                .join("."),
        )
    }

    /// `emit.header_end`: 1-based line and code-point column of the `:` that
    /// closes a def header. It is the first colon token past the parameter
    /// list and the return annotation, since every earlier one sits inside
    /// their brackets. `None` for a def the token stream does not reach.
    pub fn header_end(&self, func: &ruff_python_ast::StmtFunctionDef) -> Option<(u32, u32)> {
        use ruff_python_ast::token::TokenKind;
        use ruff_text_size::Ranged;

        let after = func
            .returns
            .as_ref()
            .map_or_else(|| func.parameters.range().end(), |r| r.range().end());
        let colon = self
            .parsed
            .tokens()
            .iter()
            .find(|t| t.kind() == TokenKind::Colon && t.range().start() >= after)?
            .range()
            .start()
            .to_usize();
        let head = &self.source[..colon];
        let line_start = head.rfind('\n').map_or(0, |at| at + 1);
        Some((
            head.matches('\n').count() as u32 + 1,
            head[line_start..].chars().count() as u32,
        ))
    }
}
