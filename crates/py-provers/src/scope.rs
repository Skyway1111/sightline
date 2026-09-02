//! One `Scope` per function - what its body declares, rebinds, aliases,
//! writes through and demands of each param. Built from facts' node index and
//! parent map, no second traversal, and memoized per symbol in `Provers`
//! (R20), so a body is never walked twice. Every scope question is a query
//! here.

use std::collections::{BTreeSet, HashSet};
use std::sync::OnceLock;

use indexmap::IndexMap;
use rayon::prelude::*;
use ruff_python_ast::{
    CmpOp, Comprehension, Expr, ExprContext, Parameters, Pattern, Stmt, StmtFunctionDef,
};
use serde_json::{Value, json};

use sightline_core::findings::Qname;
use sightline_py_facts::astutil::{
    RECEIVERS, all_arg_names, attr_on, chain_root, fn_args, fn_body, fn_params, is_mutable_init,
    line_span, walk,
};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{
    ClassInfo, FUNCTION_KINDS, ModuleId, NodeIndex, RepoFacts, Resolution, Step, class_walk,
};
use sightline_py_facts::module::Module;

use crate::Provers;
use crate::catalog::MUTATOR_METHODS;

/// `_LEXICAL`: a def or a lambda is its own scope.
const LEXICAL: [Kind; 3] = [Kind::FunctionDef, Kind::AsyncFunctionDef, Kind::Lambda];
/// `_DEFS`: the statements that bind their own name.
const DEFS: [Kind; 3] = [Kind::FunctionDef, Kind::AsyncFunctionDef, Kind::ClassDef];
const LOOPS: [Kind; 3] = [Kind::For, Kind::AsyncFor, Kind::While];
/// `_CHAIN`: a call result on the way is no one's.
const CHAIN: [Kind; 2] = [Kind::Attribute, Kind::Subscript];
/// Rebinds a local name.
const BINDS: [&str; 4] = ["name", "del", "except", "import"];
/// Writes through a reference.
const THROUGH: [&str; 3] = ["attr", "subscript", "call"];
const OUTER: [&str; 2] = ["global", "nonlocal"];

/// Every node kind that binds a name or writes through a reference, in a
/// pinned order (R5).
const WRITE_NODES: [Kind; 20] = [
    Kind::Assign,
    Kind::AnnAssign,
    Kind::AugAssign,
    Kind::Delete,
    Kind::NamedExpr,
    Kind::For,
    Kind::AsyncFor,
    Kind::Comprehension,
    Kind::With,
    Kind::AsyncWith,
    Kind::ExceptHandler,
    Kind::Import,
    Kind::ImportFrom,
    Kind::Global,
    Kind::Nonlocal,
    Kind::Call,
    Kind::Match,
    Kind::FunctionDef,
    Kind::AsyncFunctionDef,
    Kind::ClassDef,
];

mod footprints;
mod layer;
mod lookup;
mod writes;

pub use footprints::Footprint;
pub use layer::dump;
pub use lookup::{bound_from, class_fields, drawn_from, functions, is_mutation_context};
pub use writes::{Guard, Write};

use lookup::parent_node;
use writes::*;

/// A function's own view of itself. Products are lazy: a consumer asking only
/// for mutated params never pays for the alias fixpoint. Nothing here depends
/// on node order - the index groups by kind, not by document.
pub struct Scope {
    pub module: ModuleId,
    pub qname: Qname,
    /// the `def` node
    pub func: NodeIndex,
    params: OnceLock<Vec<String>>,
    signature: OnceLock<HashSet<NodeIndex>>,
    in_lambda: OnceLock<HashSet<NodeIndex>>,
    writes: OnceLock<Vec<Write>>,
    declared: OnceLock<BTreeSet<String>>,
    loops: OnceLock<Vec<(u32, u32)>>,
    rebindings: OnceLock<Vec<u32>>,
    stored: OnceLock<BTreeSet<String>>,
    outer_names: OnceLock<BTreeSet<String>>,
    alias_tainted: OnceLock<BTreeSet<String>>,
    guards: OnceLock<Vec<Guard>>,
    names: OnceLock<Vec<NodeIndex>>,
    footprints: OnceLock<IndexMap<String, Footprint>>,
    mutated_params: OnceLock<BTreeSet<String>>,
    mutates_alias: OnceLock<bool>,
}

impl Scope {
    /// `None` when the qname is not a function symbol.
    pub fn new(facts: &RepoFacts<'_>, qname: &str) -> Option<Scope> {
        let sym = facts.symbols.get(qname)?;
        if !FUNCTION_KINDS.contains(&sym.kind) {
            return None;
        }
        Some(Scope {
            module: facts.modules.get(&sym.module)?.id,
            qname: qname.into(),
            func: sym.node,
            params: OnceLock::new(),
            signature: OnceLock::new(),
            in_lambda: OnceLock::new(),
            writes: OnceLock::new(),
            declared: OnceLock::new(),
            loops: OnceLock::new(),
            rebindings: OnceLock::new(),
            stored: OnceLock::new(),
            outer_names: OnceLock::new(),
            alias_tainted: OnceLock::new(),
            guards: OnceLock::new(),
            names: OnceLock::new(),
            footprints: OnceLock::new(),
            mutated_params: OnceLock::new(),
            mutates_alias: OnceLock::new(),
        })
    }

    pub fn module<'a, 't>(&self, facts: &'a RepoFacts<'t>) -> &'a Module<'t> {
        facts
            .modules
            .get_index(self.module as usize)
            .map(|(_, m)| m)
            .expect("the scope's module is facts'")
    }

    /// The `def` this scope is.
    pub fn func_def<'t>(&self, facts: &RepoFacts<'t>) -> &'t StmtFunctionDef {
        match self.module(facts).nodes[self.func as usize] {
            Cn::Stmt(Stmt::FunctionDef(f)) => f,
            _ => panic!("a function symbol's node is a def"),
        }
    }

    /// Positional and kw-only params, self/cls included.
    pub fn params(&self, facts: &RepoFacts<'_>) -> &[String] {
        self.params.get_or_init(|| {
            fn_params(self.func_def(facts))
                .into_iter()
                .map(str::to_string)
                .collect()
        })
    }

    /// Node indices outside the body: decorators, defaults, annotations.
    fn signature(&self, facts: &RepoFacts<'_>) -> &HashSet<NodeIndex> {
        self.signature.get_or_init(|| {
            let f = self.func_def(facts);
            let mut parts: Vec<Cn<'_>> = vec![Cn::Params(&f.parameters)];
            parts.extend(f.returns.as_deref().map(Cn::Expr));
            parts.extend(f.decorator_list.iter().map(|d| Cn::Expr(&d.expression)));
            if let Some(tp) = f.type_params.as_deref() {
                parts.extend(tp.type_params.iter().map(Cn::TypeParam));
            }
            parts
                .into_iter()
                .flat_map(walk)
                .filter_map(|n| n.stamped())
                .collect()
        })
    }

    /// Node indices under an own-scope lambda: a lambda is its own scope.
    fn in_lambda(&self, facts: &RepoFacts<'_>) -> &HashSet<NodeIndex> {
        self.in_lambda.get_or_init(|| {
            let module = self.module(facts);
            module
                .nodes(&[Kind::Lambda], Some(&self.qname), false)
                .into_iter()
                .flat_map(|lam| {
                    walk(module.nodes[lam as usize])
                        .filter_map(|n| n.stamped())
                        .filter(move |i| *i != lam)
                })
                .collect()
        })
    }

    /// Every binding and reference-write under the body, own scope and nested
    /// defs alike (`Write.own` marks the former).
    pub fn writes(&self, facts: &RepoFacts<'_>) -> &[Write] {
        self.writes.get_or_init(|| {
            let module = self.module(facts);
            let signature = self.signature(facts);
            let in_lambda = self.in_lambda(facts);
            let mut out: Vec<Write> = Vec::new();
            for node in module.nodes(&WRITE_NODES, Some(&self.qname), true) {
                if node == self.func || signature.contains(&node) {
                    continue;
                }
                // a def binds its name in the scope holding it, not in its own
                let holder = if DEFS.contains(&module.nodes[node as usize].kind()) {
                    module.parent_of(node)
                } else {
                    Some(node)
                };
                let own = holder.is_some_and(|h| facts.enclosing(module, h) == self.qname)
                    && !in_lambda.contains(&node);
                writes_of(module, node, own, &mut out);
            }
            out
        })
    }

    /// Params plus AnnAssign'd locals: names whose type the repo wrote. A
    /// plain local (`s = self.scale`) launders an inferred type and is not.
    pub fn declared(&self, facts: &RepoFacts<'_>) -> &BTreeSet<String> {
        self.declared.get_or_init(|| {
            let f = self.func_def(facts);
            let mut out: BTreeSet<String> = all_arg_names(Some(&f.parameters))
                .into_iter()
                .map(str::to_string)
                .collect();
            for w in self.writes(facts) {
                if w.own && w.decl && w.kind == "name" {
                    out.extend(w.root.clone());
                }
            }
            out
        })
    }

    /// Line spans of the own scope's loops.
    pub fn loops(&self, facts: &RepoFacts<'_>) -> &[(u32, u32)] {
        self.loops.get_or_init(|| {
            let module = self.module(facts);
            module
                .nodes(&LOOPS, Some(&self.qname), false)
                .into_iter()
                .map(|n| line_span((module.line_of(n), module.end_line_of(n))))
                .collect()
        })
    }

    /// The node's parents, innermost first, stopping below the def itself: the
    /// one parent-map climb every other query is phrased in.
    fn ancestry(&self, module: &Module<'_>, node: NodeIndex) -> Vec<NodeIndex> {
        let mut out = Vec::new();
        let mut cur = module.parent_of(node);
        while let Some(n) = cur {
            if n == self.func {
                break;
            }
            out.push(n);
            cur = module.parent_of(n);
        }
        out
    }

    /// The innermost loop `node` sits in, nested defs traversed.
    pub fn enclosing_loop(&self, facts: &RepoFacts<'_>, node: NodeIndex) -> Option<NodeIndex> {
        let module = self.module(facts);
        self.ancestry(module, node)
            .into_iter()
            .find(|p| LOOPS.contains(&module.nodes[*p as usize].kind()))
    }

    /// The writes the body performs inside one statement's line span.
    pub fn writes_in(&self, facts: &RepoFacts<'_>, node: NodeIndex) -> Vec<&Write> {
        let module = self.module(facts);
        let (lo, hi) = line_span((module.line_of(node), module.end_line_of(node)));
        self.writes(facts)
            .iter()
            .filter(|w| {
                let line = module.line_of(w.node);
                lo <= line && line <= hi
            })
            .collect()
    }

    /// Own-scope writes that rebind a local name (a declaration among them:
    /// `decl` tells).
    pub fn rebindings(&self, facts: &RepoFacts<'_>) -> Vec<&Write> {
        let writes = self.writes(facts);
        self.rebindings
            .get_or_init(|| {
                writes
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| w.own && BINDS.contains(&w.kind))
                    .map(|(i, _)| i as u32)
                    .collect()
            })
            .iter()
            .map(|i| &writes[*i as usize])
            .collect()
    }

    /// Names rebound on a path to `line`: a binding preceding it, or one
    /// inside a loop spanning it. A declared type covers the entry value only
    /// (`while isinstance(node, B): node = node.parent`), so an AnnAssign
    /// counts only when `declared`: `x: dict = json.loads(x)` still leaves no
    /// caller's value for a guard to judge.
    pub fn rebound_before(
        &self,
        facts: &RepoFacts<'_>,
        line: u32,
        declared: bool,
    ) -> BTreeSet<String> {
        let module = self.module(facts);
        let spanning: Vec<(u32, u32)> = self
            .loops(facts)
            .iter()
            .copied()
            .filter(|(a, b)| *a <= line && line <= *b)
            .collect();
        self.rebindings(facts)
            .into_iter()
            .filter(|w| declared || !w.decl)
            .filter(|w| {
                let at = module.line_of(w.node);
                at < line || spanning.iter().any(|(a, b)| *a <= at && at <= *b)
            })
            .filter_map(|w| w.root.clone())
            .collect()
    }

    /// Names the body stores to (not a del, an import, or a write through).
    pub fn stored(&self, facts: &RepoFacts<'_>) -> &BTreeSet<String> {
        self.stored.get_or_init(|| {
            self.writes(facts)
                .iter()
                .filter(|w| w.kind == "name")
                .filter_map(|w| w.root.clone())
                .collect()
        })
    }

    /// Names a `global`/`nonlocal` claims: storing to one leaves the scope.
    pub fn outer_names(&self, facts: &RepoFacts<'_>) -> &BTreeSet<String> {
        self.outer_names.get_or_init(|| {
            self.writes(facts)
                .iter()
                .filter(|w| OUTER.contains(&w.kind))
                .filter_map(|w| w.root.clone())
                .collect()
        })
    }

    /// Locally-stored names that may alias structures rooted outside the
    /// function: mutating them mutates shared state. A root is shared when it
    /// is a param, a declared `global`/`nonlocal`, a non-local name, or itself
    /// tainted; taint propagates local-to-local to a fixpoint. Call results
    /// and displays stay fresh - ownership is unknowable at the AST level.
    pub fn alias_tainted(&self, facts: &RepoFacts<'_>) -> &BTreeSet<String> {
        self.alias_tainted.get_or_init(|| {
            let params: BTreeSet<String> = self.params(facts).iter().cloned().collect();
            let unshared: BTreeSet<String> = self
                .stored(facts)
                .difference(&params)
                .cloned()
                .collect::<BTreeSet<String>>()
                .difference(self.outer_names(facts))
                .cloned()
                .collect();
            let bound: Vec<(&String, &BTreeSet<String>)> = self
                .writes(facts)
                .iter()
                .filter_map(|w| Some((w.root.as_ref()?, w.aliases.as_ref()?)))
                .collect();
            let mut tainted: BTreeSet<String> = BTreeSet::new();
            let mut changed = true;
            while changed {
                changed = false;
                for (name, roots) in &bound {
                    // `roots <= unshared - tainted`: every root still free
                    let free = roots
                        .iter()
                        .all(|r| unshared.contains(r) && !tainted.contains(r));
                    if !tainted.contains(*name) && !free {
                        tainted.insert((*name).clone());
                        changed = true;
                    }
                }
            }
            tainted
        })
    }

    /// The body statements, docstring dropped.
    pub fn body<'t>(&self, facts: &RepoFacts<'t>) -> &'t [Stmt] {
        fn_body(&self.func_def(facts).body)
    }

    /// Indices of every ancestor below the def of the given nodes (the nodes
    /// themselves too when `own`); a climb stops at the first index collected -
    /// its own ancestors are already in.
    pub fn ancestor_ids(
        &self,
        facts: &RepoFacts<'_>,
        nodes: impl IntoIterator<Item = NodeIndex>,
        own: bool,
    ) -> HashSet<NodeIndex> {
        let module = self.module(facts);
        let mut out: HashSet<NodeIndex> = HashSet::new();
        for node in nodes {
            let climb = self.ancestry(module, node);
            let walk = if own {
                std::iter::once(node).chain(climb).collect::<Vec<_>>()
            } else {
                climb
            };
            for cur in walk {
                if !out.insert(cur) {
                    break;
                }
            }
        }
        out
    }

    /// Param checks appearing as If/Assert tests (guards, not predicates), in
    /// document order, on the entry value only: a check after the param is
    /// rebound (`x = json.loads(x)`) judges a value no caller sent - #5's
    /// contradiction reads this.
    pub fn guards(&self, facts: &RepoFacts<'_>) -> &[Guard] {
        self.guards.get_or_init(|| {
            let module = self.module(facts);
            let params: BTreeSet<&str> = self.params(facts).iter().map(String::as_str).collect();
            let mut tests: Vec<(NodeIndex, &Expr)> = module
                .nodes(&[Kind::If, Kind::Assert], Some(&self.qname), true)
                .into_iter()
                .filter_map(|n| {
                    let test = test_of(module, n)?;
                    Some((Cn::Expr(test).stamped()?, test))
                })
                .collect();
            tests.sort_by_key(|(i, _)| {
                (
                    module.line_of(*i),
                    module.span(*i).and_then(|s| s[1]).unwrap_or(0),
                )
            });
            let mut out: Vec<Guard> = Vec::new();
            for expr in tests.into_iter().flat_map(|(_, t)| walk(Cn::Expr(t))) {
                let Cn::Expr(expr) = expr else { continue };
                if let Some(guard) = isinstance_guard(module, expr, &params) {
                    out.push(guard);
                } else if let Some(guard) = none_guard(expr, &params) {
                    out.push(guard);
                }
            }
            out.retain(|g| {
                !self
                    .rebound_before(facts, module.line_of(g.node), true)
                    .contains(&g.param)
            });
            out
        })
    }

    /// Every name the body mentions, nested defs included; the signature
    /// (decorators, defaults, annotations) is not the body.
    pub fn names(&self, facts: &RepoFacts<'_>) -> &[NodeIndex] {
        self.names.get_or_init(|| {
            let signature = self.signature(facts);
            self.module(facts)
                .nodes(&[Kind::Name], Some(&self.qname), true)
                .into_iter()
                .filter(|n| !signature.contains(n))
                .collect()
        })
    }

    /// Mentions of `name` in the body; a rebinding nested def is skipped.
    pub fn uses_of(&self, facts: &RepoFacts<'_>, name: &str) -> Vec<NodeIndex> {
        let module = self.module(facts);
        self.names(facts)
            .iter()
            .copied()
            .filter(|n| name_id(module, *n) == Some(name))
            .filter(|n| !self.shadowed(module, *n, name))
            .collect()
    }
}
