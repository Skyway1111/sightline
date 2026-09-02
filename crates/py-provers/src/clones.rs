//! The mining half of clone detection: #11's function, block and expression
//! groups over the one blind normalization (`dump.rs`). The suffix-array
//! half is `core::clones`; the prover mines groups, the rule prices and
//! reports them.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

use indexmap::IndexMap;
use ruff_python_ast::{ElifElseClause, Expr, Stmt};
use serde_json::{Value, json};

use sightline_core::clones::{MIN_BLOCK_STMTS, MIN_CLONE_NODES, Seq, digest, repeats};
use sightline_core::findings::{Qname, Rel};
use sightline_py_facts::astutil::{CHAIN, chain_root, fn_body, walk};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::{Kind, is_def, is_expr};
use sightline_py_facts::model::{
    FUNCTION_KINDS, ModuleId, NodeIndex, RepoFacts, Symbol, is_test_path,
};
use sightline_py_facts::module::Module;
use sightline_py_facts::order;

use crate::Provers;
use crate::comments::body_of;
use crate::dump::{BLIND, Dumps, Sizes, normalize, size};
use crate::imports::{import_targets, internal_module};

/// An expression walk worth a name: 3 attributes or more, at 4 sites or more
/// in 2 modules or more.
const MIN_EXPR_ATTRS: usize = 3;
const MIN_EXPR_SITES: usize = 4;

/// One run's blind reading of the tree: the dump per statement or expression
/// and the node count per node, each computed once.
#[derive(Default)]
pub struct Shapes {
    dumps: RefCell<Dumps>,
    counts: RefCell<Sizes>,
}

impl Shapes {
    pub fn dump<'a>(&self, node: Cn<'a>, module: &'a Module<'a>) -> String {
        normalize(node, module, &BLIND, &mut self.dumps.borrow_mut(), None)
    }

    pub fn size(&self, node: Cn<'_>, module: &Module<'_>) -> usize {
        size(node, module, &mut self.counts.borrow_mut())
    }
}

/// One copy: its module, its owner, and the cloned nodes - the site first,
/// the whole block for a window, the one expression for an expression shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub module: Qname,
    pub symbol: Qname,
    pub nodes: Vec<NodeIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneGroup {
    pub key: String,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Clones {
    pub functions: Vec<CloneGroup>,
    pub blocks: Vec<CloneGroup>,
    pub exprs: Vec<CloneGroup>,
}

/// Every symbol a def backs, in facts order.
pub fn iter_functions<'f>(facts: &'f RepoFacts<'_>) -> Vec<&'f Symbol> {
    facts
        .symbols
        .values()
        .filter(|s| FUNCTION_KINDS.contains(&s.kind))
        .collect()
}

/// The one home for foreign roots, which #11 reads: names the module binds
/// to third-party or stdlib imports, plus self/cls. A
/// walk rooted there is that object's API path or an instance's own field,
/// not repo knowledge.
pub fn foreign_roots(facts: &RepoFacts<'_>, module: &Module<'_>) -> HashSet<Box<str>> {
    let mut names: HashSet<Box<str>> = ["self", "cls"].into_iter().map(Box::from).collect();
    let kinds = [Kind::Import, Kind::ImportFrom];
    for node in module.nodes(&kinds, Some(&module.qname), false) {
        let targets = import_targets(facts, module, node);
        if !targets.iter().all(|t| internal_module(facts, t).is_none()) {
            continue;
        }
        let aliases = match module.nodes[node as usize] {
            Cn::Stmt(Stmt::Import(n)) => &n.names,
            Cn::Stmt(Stmt::ImportFrom(n)) => &n.names,
            _ => continue,
        };
        for alias in aliases {
            let bound = alias.asname.as_ref().unwrap_or(&alias.name);
            names.insert(bound.split('.').next().unwrap_or("").into());
        }
    }
    names
}

/// The three clone populations over `functions`, each group >= 2 members and
/// maximal: whole bodies, own-scope maximal statement repeats, and repeated
/// attribute-walk expressions in prod bodies. Test members count toward a
/// group - a group is dropped only when every member is a test.
pub fn mine(
    facts: &RepoFacts<'_>,
    functions: &[&Symbol],
    foreign: &HashMap<Qname, HashSet<Box<str>>>,
) -> Clones {
    let shapes = Shapes::default();
    let rows = sequences(facts, functions, &shapes);
    Clones {
        functions: function_groups(facts, functions, &shapes),
        blocks: block_groups(&rows),
        exprs: expr_groups(facts, functions, foreign, &shapes),
    }
}

/// The body a function symbol owns, its docstring dropped.
fn own_body<'t>(module: &Module<'t>, sym: &Symbol) -> Option<&'t [Stmt]> {
    body_of(module, sym.node).map(fn_body)
}

fn function_groups(
    facts: &RepoFacts<'_>,
    functions: &[&Symbol],
    shapes: &Shapes,
) -> Vec<CloneGroup> {
    let mut groups: IndexMap<String, Vec<Member>> = IndexMap::new();
    for sym in functions {
        let module = &facts.modules[&sym.module];
        let Some(body) = own_body(module, sym) else {
            continue;
        };
        // the floor counts the body as a module: + the Module node
        let nodes: usize = body.iter().map(|s| shapes.size(Cn::Stmt(s), module)).sum();
        if body.is_empty() || nodes + 1 < MIN_CLONE_NODES {
            continue;
        }
        let text: Vec<String> = body
            .iter()
            .map(|s| shapes.dump(Cn::Stmt(s), module))
            .collect();
        groups
            .entry(digest(&text.join("\n")))
            .or_default()
            .push(Member {
                module: sym.module.clone(),
                symbol: sym.qname.clone(),
                nodes: vec![sym.node],
            });
    }
    // a first copy is never a finding
    let mut out: Vec<CloneGroup> = groups
        .into_iter()
        .filter(|(_, members)| members.len() >= 2)
        .map(|(key, members)| CloneGroup { key, members })
        .collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

/// A statement list of one scope. `Elif` is the `orelse=[If]` CPython nests
/// an `elif` in: one statement, so the block floor always drops it, and its
/// own blocks are queued a level below its parent's, as CPython has them.
#[derive(Clone, Copy)]
enum Block<'a> {
    Stmts(&'a [Stmt]),
    Elif(&'a [ElifElseClause]),
}

fn orelse(rest: &[ElifElseClause]) -> Option<Block<'_>> {
    match rest.first() {
        None => None,
        Some(next) if next.test.is_some() => Some(Block::Elif(rest)),
        Some(next) if next.body.is_empty() => None,
        Some(next) => Some(Block::Stmts(&next.body)),
    }
}

/// The blocks one statement holds, in `("body", "orelse", "finalbody")` then
/// handlers then cases order, empty lists dropped.
fn stmt_blocks<'a>(st: &'a Stmt, out: &mut Vec<Block<'a>>) {
    let mut body = |b: &'a [Stmt]| {
        if !b.is_empty() {
            out.push(Block::Stmts(b));
        }
    };
    match st {
        Stmt::If(n) => {
            body(&n.body);
            if let Some(block) = orelse(&n.elif_else_clauses) {
                out.push(block);
            }
        }
        Stmt::For(n) => {
            body(&n.body);
            body(&n.orelse);
        }
        Stmt::While(n) => {
            body(&n.body);
            body(&n.orelse);
        }
        Stmt::With(n) => body(&n.body),
        Stmt::Try(n) => {
            body(&n.body);
            body(&n.orelse);
            body(&n.finalbody);
            for h in &n.handlers {
                let ruff_python_ast::ExceptHandler::ExceptHandler(h) = h;
                body(&h.body);
            }
        }
        Stmt::Match(n) => {
            for case in &n.cases {
                body(&case.body);
            }
        }
        _ => {}
    }
}

/// (statement list, is-the-top-level-body) for fn's own scope: the body and
/// every nested compound block, breadth first; nested defs are their own
/// functions.
fn own_sequences(body: &[Stmt]) -> Vec<(Block<'_>, bool)> {
    let mut queue: VecDeque<(Block<'_>, bool)> = VecDeque::from([(Block::Stmts(body), true)]);
    let mut out = Vec::new();
    let mut kids: Vec<Block<'_>> = Vec::new();
    while let Some((seq, top)) = queue.pop_front() {
        out.push((seq, top));
        kids.clear();
        match seq {
            Block::Stmts(stmts) => {
                for st in stmts {
                    if is_def(Cn::Stmt(st).kind()) {
                        continue;
                    }
                    stmt_blocks(st, &mut kids);
                }
            }
            Block::Elif(rest) => {
                let clause = &rest[0];
                if !clause.body.is_empty() {
                    kids.push(Block::Stmts(&clause.body));
                }
                if let Some(block) = orelse(&rest[1..]) {
                    kids.push(block);
                }
            }
        }
        queue.extend(kids.iter().map(|b| (*b, false)));
    }
    out
}

/// One mined sequence and the statements this front digested into it.
struct Row {
    seq: Seq,
    module: Qname,
    symbol: Qname,
    stmts: Vec<NodeIndex>,
}

fn sequences(facts: &RepoFacts<'_>, functions: &[&Symbol], shapes: &Shapes) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for sym in functions {
        let module = &facts.modules[&sym.module];
        let Some(body) = own_body(module, sym) else {
            continue;
        };
        let prod = !is_test_path(&module.rel);
        for (block, top) in own_sequences(body) {
            let Block::Stmts(stmts) = block else { continue };
            if stmts.len() < MIN_BLOCK_STMTS {
                continue;
            }
            rows.push(Row {
                seq: Seq {
                    digests: stmts
                        .iter()
                        .map(|s| digest(&shapes.dump(Cn::Stmt(s), module)))
                        .collect(),
                    sizes: stmts
                        .iter()
                        .map(|s| shapes.size(Cn::Stmt(s), module))
                        .collect(),
                    order: module.rel.to_string(),
                    top,
                    prod,
                },
                module: sym.module.clone(),
                symbol: sym.qname.clone(),
                stmts: stmts
                    .iter()
                    .map(|s| Cn::Stmt(s).stamped().unwrap_or_default())
                    .collect(),
            });
        }
    }
    rows
}

/// The mined repeats back on the statements they ran over.
fn block_groups(rows: &[Row]) -> Vec<CloneGroup> {
    let seqs: Vec<Seq> = rows.iter().map(|r| r.seq.clone()).collect();
    repeats(&seqs)
        .into_iter()
        .map(|rep| CloneGroup {
            key: rep.key,
            members: rep
                .runs
                .iter()
                .map(|&(s, i)| Member {
                    module: rows[s].module.clone(),
                    symbol: rows[s].symbol.clone(),
                    nodes: rows[s].stmts[i..i + rep.length].to_vec(),
                })
                .collect(),
        })
        .collect()
}

/// (expression, data attributes within) for every expression in fn's body;
/// nested defs and classes are their own symbols, decorators and the
/// signature are not body. A method call's name is an operation, not a step
/// of the walk.
fn body_exprs<'a>(body: &'a [Stmt]) -> Vec<(Cn<'a>, usize)> {
    let mut out = Vec::new();
    for st in body {
        attrs_below(Cn::Stmt(st), false, &mut out);
    }
    out
}

fn attrs_below<'a>(node: Cn<'a>, called: bool, out: &mut Vec<(Cn<'a>, usize)>) -> usize {
    let kind = node.kind();
    if is_def(kind) {
        return 0;
    }
    let mut attrs = usize::from(kind == Kind::Attribute && !called);
    let mut kids = Vec::new();
    order::children(node, &mut kids);
    for (at, child) in kids.iter().enumerate() {
        attrs += attrs_below(*child, kind == Kind::Call && at == 0, out);
    }
    // an await is its operand, not a second site
    if is_expr(kind) && kind != Kind::Await {
        out.push((node, attrs));
    }
    attrs
}

/// One expression site, with what the group ordering and the layer read off
/// it.
struct Site<'a> {
    node: Cn<'a>,
    at: (ModuleId, NodeIndex),
    rel: Rel,
    line: u32,
    col: u32,
    member: Member,
}

/// Repeated attribute walks in prod bodies, past `MIN_EXPR_ATTRS` and
/// `MIN_EXPR_SITES`, maximal shapes only (a sub-expression of a reported
/// shape is that shape's). Foreign-rooted walks are out, and so is `f(...)`
/// with a plain-name callee: that is the function's argument list, not a
/// walk.
fn expr_groups(
    facts: &RepoFacts<'_>,
    functions: &[&Symbol],
    foreign: &HashMap<Qname, HashSet<Box<str>>>,
    shapes: &Shapes,
) -> Vec<CloneGroup> {
    let mut groups: IndexMap<String, Vec<Site<'_>>> = IndexMap::new();
    for sym in functions {
        let module = &facts.modules[&sym.module];
        if is_test_path(&module.rel) {
            continue;
        }
        let Some(body) = own_body(module, sym) else {
            continue;
        };
        let rooted_off = foreign.get(&module.qname);
        for (node, attrs) in body_exprs(body) {
            if attrs < MIN_EXPR_ATTRS || !repo_knowledge(node, rooted_off) {
                continue;
            }
            let Some(index) = node.stamped() else {
                continue;
            };
            let Some(span) = module.span(index) else {
                continue;
            };
            groups
                .entry(digest(&shapes.dump(node, module)))
                .or_default()
                .push(Site {
                    node,
                    at: (module.id, index),
                    rel: module.rel.clone(),
                    line: span[0].unwrap_or_default(),
                    col: span[1].unwrap_or_default(),
                    member: Member {
                        module: sym.module.clone(),
                        symbol: sym.qname.clone(),
                        nodes: vec![index],
                    },
                });
        }
    }
    for sites in groups.values_mut() {
        sites.sort_by(|a, b| (&a.rel, a.line, a.col).cmp(&(&b.rel, b.line, b.col)));
    }
    // largest shapes first: they own their parts
    let mut order: Vec<(usize, &String)> = groups
        .iter()
        .map(|(key, sites)| {
            let module = &facts.modules[&sites[0].member.module];
            (shapes.size(sites[0].node, module), key)
        })
        .collect();
    order.sort_by(|a, b| (std::cmp::Reverse(a.0), a.1).cmp(&(std::cmp::Reverse(b.0), b.1)));

    let mut taken: HashSet<(ModuleId, NodeIndex)> = HashSet::new();
    let mut out: Vec<CloneGroup> = Vec::new();
    for (_, key) in order {
        let sites: Vec<&Site<'_>> = groups[key]
            .iter()
            .filter(|s| !taken.contains(&s.at))
            .collect();
        let homes: HashSet<&Rel> = sites.iter().map(|s| &s.rel).collect();
        if sites.len() < MIN_EXPR_SITES || homes.len() < 2 {
            continue;
        }
        for site in &sites {
            let module = &facts.modules[&site.member.module];
            for reached in walk(site.node) {
                if let Some(index) = reached.stamped() {
                    taken.insert((module.id, index));
                }
            }
        }
        out.push(CloneGroup {
            key: key.clone(),
            members: sites.iter().map(|s| s.member.clone()).collect(),
        });
    }
    out
}

fn repo_knowledge(node: Cn<'_>, foreign: Option<&HashSet<Box<str>>>) -> bool {
    let Cn::Expr(expr) = node else {
        return true;
    };
    if let Some(root) = chain_root(expr, &CHAIN)
        && foreign.is_some_and(|names| names.contains(root))
    {
        return false;
    }
    // `f(...)` with a plain-name callee is the function's argument list, not
    // a walk: the name already exists
    !matches!(expr, Expr::Call(c) if matches!(&*c.func, Expr::Name(_)))
}

/// `layer_clones`.
pub fn dump(facts: &RepoFacts<'_>, _provers: &Provers) -> Option<Value> {
    let functions = iter_functions(facts);
    let mut foreign: HashMap<Qname, HashSet<Box<str>>> = HashMap::new();
    for sym in &functions {
        if !foreign.contains_key(&sym.module) {
            let module = &facts.modules[&sym.module];
            foreign.insert(sym.module.clone(), foreign_roots(facts, module));
        }
    }
    let mined = mine(facts, &functions, &foreign);
    Some(json!({
        "functions": sites(facts, &mined.functions),
        "blocks": sites(facts, &mined.blocks),
        "exprs": sites(facts, &mined.exprs),
    }))
}

/// A group as the harness keys it: sorted member sites, digest omitted.
fn sites(facts: &RepoFacts<'_>, groups: &[CloneGroup]) -> Value {
    let mut rows: Vec<Vec<(Rel, Qname, u32, u32)>> = groups
        .iter()
        .map(|group| {
            let mut members: Vec<(Rel, Qname, u32, u32)> = group
                .members
                .iter()
                .map(|m| {
                    let module = &facts.modules[&m.module];
                    let first = m.nodes.iter().map(|n| module.line_of(*n)).min();
                    let last = m.nodes.iter().map(|n| module.end_line_of(*n)).max();
                    (
                        module.rel.clone(),
                        m.symbol.clone(),
                        first.unwrap_or_default(),
                        last.unwrap_or_default(),
                    )
                })
                .collect();
            members.sort();
            members
        })
        .collect();
    rows.sort();
    Value::from(
        rows.into_iter()
            .map(|members| {
                members
                    .into_iter()
                    .map(|(rel, qname, first, last)| json!([&*rel, &*qname, first, last]))
                    .collect::<Vec<Value>>()
            })
            .collect::<Vec<Vec<Value>>>(),
    )
}
