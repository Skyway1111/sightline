//! Pass A of the build: the one traversal, the module-local reads
//! (`__all__`, bindings, comments) and the symbol, class and binding indexes.
//!
//! Everything here but the merge is per module, so `build.rs` runs it under
//! rayon and replays the merge in listing order.

use std::collections::HashMap;

use indexmap::IndexMap;
use ruff_python_ast::token::TokenKind;
use ruff_python_ast::{ExceptHandler, Expr, ModModule, Stmt, StmtClassDef};
use ruff_python_parser::Parsed;
use ruff_text_size::Ranged;

use sightline_core::findings::Qname;

use crate::cn::Cn;
use crate::kinds::Kind;
use crate::lines::Lines;
use crate::model::{ClassInfo, NodeIndex, RepoFacts, ScopeId, Span, Symbol, is_test_path};
use crate::module::{Module, Source};
use crate::order::children;
use crate::qnames::resolve_dotted_expr;

// --- the scope marks --------------------------------------------------------

/// The qname each def or class statement opens, keyed by the statement's
/// address. Defs under control flow keep the enclosing scope's qname;
/// `@x.setter` and `@x.deleter` become `C.x.setter`; a lambda and a
/// comprehension are not scopes.
pub type Marks = HashMap<usize, Box<str>>;

pub fn marks(module: &ModModule, qname: &str) -> Marks {
    let mut out = Marks::default();
    mark_stmts(&module.body, qname, false, &mut out);
    out
}

fn mark_stmts(stmts: &[Stmt], parent_q: &str, in_class: bool, out: &mut Marks) {
    for st in stmts {
        match def_parts(st) {
            Some((name, body, is_class)) => {
                let mut q = format!("{parent_q}.{name}");
                if in_class && let Some(acc) = property_accessor(st, name) {
                    q.push('.');
                    q.push_str(acc);
                }
                out.insert(stmt_key(st), q.as_str().into());
                mark_stmts(body, &q, is_class, out);
            }
            None => {
                for block in blocks(st) {
                    mark_stmts(block, parent_q, in_class, out);
                }
            }
        }
    }
}

fn stmt_key(st: &Stmt) -> usize {
    st as *const Stmt as usize
}

/// `(name, body, is_class)` for a def or a class statement.
fn def_parts(st: &Stmt) -> Option<(&str, &[Stmt], bool)> {
    match st {
        Stmt::FunctionDef(n) => Some((n.name.as_str(), &n.body, false)),
        Stmt::ClassDef(n) => Some((n.name.as_str(), &n.body, true)),
        _ => None,
    }
}

/// `setter` or `deleter` for a def decorated `@<its own name>.setter`.
fn property_accessor(st: &Stmt, name: &str) -> Option<&'static str> {
    let decorators = match st {
        Stmt::FunctionDef(n) => &n.decorator_list,
        Stmt::ClassDef(n) => &n.decorator_list,
        _ => return None,
    };
    for d in decorators {
        let Expr::Attribute(a) = &d.expression else {
            continue;
        };
        let Expr::Name(base) = a.value.as_ref() else {
            continue;
        };
        if base.id.as_str() != name {
            continue;
        }
        match a.attr.as_str() {
            "setter" => return Some("setter"),
            "deleter" => return Some("deleter"),
            _ => {}
        }
    }
    None
}

/// `_blocks`: the statement lists nested directly under a compound
/// statement, in CPython's field order (`body`, `orelse`, `finalbody`, then
/// the handlers, then the match cases).
fn blocks(st: &Stmt) -> Vec<&[Stmt]> {
    match st {
        Stmt::If(n) => {
            let mut out: Vec<&[Stmt]> = vec![&n.body];
            for clause in &n.elif_else_clauses {
                out.push(&clause.body);
            }
            out
        }
        Stmt::While(n) => vec![&n.body, &n.orelse],
        Stmt::For(n) => vec![&n.body, &n.orelse],
        Stmt::With(n) => vec![&n.body],
        Stmt::Try(n) => {
            let mut out: Vec<&[Stmt]> = vec![&n.body, &n.orelse, &n.finalbody];
            out.extend(n.handlers.iter().map(|h| {
                let ExceptHandler::ExceptHandler(h) = h;
                &*h.body
            }));
            out
        }
        Stmt::Match(n) => n.cases.iter().map(|case| case.body.as_slice()).collect(),
        _ => Vec::new(),
    }
}

// --- the one traversal ------------------------------------------------------

pub struct Traversal<'t> {
    pub nodes: Vec<Cn<'t>>,
    pub spans: Vec<Option<Span>>,
    pub parent: Vec<Option<NodeIndex>>,
    pub enclosing: Vec<Option<ScopeId>>,
    pub nodes_by_scope: IndexMap<Qname, Vec<(Kind, Vec<NodeIndex>)>>,
}

/// The parent map, the enclosing map and the per-scope node index, in one
/// explicit-stack pre-order walk. Def and class nodes arrive pre-marked with
/// their qname.
pub fn traverse<'t>(
    module: &'t ModModule,
    qname: &Qname,
    source: &str,
    lines: &Lines,
    marks: &Marks,
    type_ignores: &[u32],
) -> Traversal<'t> {
    let mut ix = Traversal {
        nodes: Vec::with_capacity(4096),
        spans: Vec::with_capacity(4096),
        parent: Vec::with_capacity(4096),
        enclosing: Vec::with_capacity(4096),
        nodes_by_scope: IndexMap::new(),
    };
    ix.nodes_by_scope.insert(qname.clone(), Vec::new());
    // `Module.type_ignores` follows `Module.body`, so they sit under the
    // module on the stack and pop after every statement subtree.
    let mut stack: Vec<(Cn<'t>, ScopeId, NodeIndex)> = type_ignores
        .iter()
        .rev()
        .map(|line| (Cn::TypeIgnore(*line), 0, 0))
        .collect();
    stack.push((Cn::Module(module), 0, u32::MAX));
    let mut scratch: Vec<Cn<'t>> = Vec::with_capacity(64);

    while let Some((node, inherited, parent)) = stack.pop() {
        let mut scope = inherited;
        if let Some(key) = node.def_key()
            && let Some(q) = marks.get(&key)
        {
            let qname: Qname = (**q).into();
            scope = match ix.nodes_by_scope.get_index_of(&qname) {
                Some(id) => id as ScopeId,
                None => ix.nodes_by_scope.insert_full(qname, Vec::new()).0 as ScopeId,
            };
        }
        let index = ix.nodes.len() as NodeIndex;
        ix.nodes.push(node);
        ix.spans.push(span(node, source, lines));
        ix.parent.push((parent != u32::MAX).then_some(parent));
        ix.enclosing.push((scope != 0).then_some(scope));
        node.set_index(index);

        let kind = node.kind();
        let bucket = &mut ix.nodes_by_scope[scope as usize];
        match bucket.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, list)) => list.push(index),
            None => bucket.push((kind, vec![index])),
        }

        scratch.clear();
        children(node, &mut scratch);
        stack.extend(scratch.iter().rev().map(|c| (*c, scope, index)));
    }
    ix
}

/// The CPython position tuple of one node.
fn span(node: Cn<'_>, source: &str, lines: &Lines) -> Option<Span> {
    if let Cn::TypeIgnore(line) = node {
        return Some([Some(line), None, None, None]);
    }
    let range = node.range(source)?;
    let (line, col) = lines.pos(range.start().to_u32());
    let (end_line, end_col) = lines.pos(range.end().to_u32());
    Some([Some(line), Some(col), Some(end_line), Some(end_col)])
}

/// CPython's `TYPE_IGNORE` token rule (`Parser/tokenizer.c`): `#`, spaces or
/// tabs, `type`, `:`, spaces or tabs, then `ignore` not followed by an
/// alphanumeric. One `Module.type_ignores` entry per hit, with its line.
pub fn type_ignores(parsed: &Parsed<ModModule>, source: &str, lines: &Lines) -> Vec<u32> {
    let mut out = Vec::new();
    for token in parsed.tokens().iter() {
        if token.kind() != TokenKind::Comment {
            continue;
        }
        let Some(tail) = crate::typecomments::strip_prefix(&source[token.range()]) else {
            continue;
        };
        if crate::typecomments::is_ignore(tail) {
            out.push(lines.pos(token.range().start().to_u32()).0);
        }
    }
    out
}

// --- module-local reads -----------------------------------------------------

/// `_extract_all`: the literal `__all__`, or the mark that the module builds
/// one at run time.
pub fn extract_all(stmts: &[Stmt], all_names: &mut Option<Vec<Box<str>>>, dynamic: &mut bool) {
    for st in stmts {
        match st {
            Stmt::Assign(a) if is_all_target(a.targets.as_slice()) => match a.value.as_ref() {
                Expr::List(l) => literal_all(&l.elts, all_names, dynamic),
                Expr::Tuple(t) => literal_all(&t.elts, all_names, dynamic),
                _ => *dynamic = true,
            },
            Stmt::AugAssign(a) if is_all_name(&a.target) => *dynamic = true,
            Stmt::Expr(e) => {
                if let Expr::Call(call) = e.value.as_ref()
                    && let Expr::Attribute(attr) = call.func.as_ref()
                    && is_all_name(&attr.value)
                {
                    *dynamic = true;
                }
            }
            // a try-import or TYPE_CHECKING block holds `__all__` too
            Stmt::If(_) | Stmt::Try(_) => {
                for block in all_blocks(st) {
                    extract_all(block, all_names, dynamic);
                }
            }
            _ => {}
        }
    }
}

/// The blocks `_extract_all` descends: CPython's `body`, `orelse`, the
/// handler bodies, then `finalbody`.
fn all_blocks(st: &Stmt) -> Vec<&[Stmt]> {
    match st {
        Stmt::If(n) => {
            let mut out: Vec<&[Stmt]> = vec![&n.body];
            for clause in &n.elif_else_clauses {
                out.push(&clause.body);
            }
            out
        }
        Stmt::Try(n) => {
            let mut out: Vec<&[Stmt]> = vec![&n.body, &n.orelse];
            out.extend(n.handlers.iter().map(|h| {
                let ExceptHandler::ExceptHandler(h) = h;
                &*h.body
            }));
            out.push(&n.finalbody);
            out
        }
        _ => Vec::new(),
    }
}

fn is_all_name(expr: &Expr) -> bool {
    matches!(expr, Expr::Name(n) if n.id.as_str() == "__all__")
}

fn is_all_target(targets: &[Expr]) -> bool {
    targets.len() == 1 && is_all_name(&targets[0])
}

fn literal_all(elts: &[Expr], all_names: &mut Option<Vec<Box<str>>>, dynamic: &mut bool) {
    let mut names = Vec::with_capacity(elts.len());
    for e in elts {
        match e {
            Expr::StringLiteral(s) => names.push(s.value.to_str().into()),
            _ => {
                *dynamic = true;
                return;
            }
        }
    }
    *all_names = Some(names);
}

/// `(alias index, local name, dotted target)` per name one import binds;
/// `*` is skipped.
pub fn imports(module: &Module<'_>, st: &Stmt) -> Vec<(String, String)> {
    match st {
        Stmt::Import(imp) => imp
            .names
            .iter()
            .map(|a| {
                let head = a.name.split('.').next().unwrap_or("").to_string();
                match &a.asname {
                    Some(alias) => (alias.to_string(), a.name.to_string()),
                    None => (head.clone(), head),
                }
            })
            .collect(),
        Stmt::ImportFrom(imp) => {
            let base = module.rel_import_base(imp.level, imp.module.as_deref());
            imp.names
                .iter()
                .filter(|a| a.name.as_str() != "*")
                .map(|a| {
                    let local = a.asname.as_ref().unwrap_or(&a.name).to_string();
                    let target = if base.is_empty() {
                        a.name.to_string()
                    } else {
                        format!("{base}.{}", a.name)
                    };
                    (local, target)
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// `_collect_bindings`: module-scope bindings, the blocks of a module-level
/// `if`, `try` or `with` included. A def under `if sys.platform == "win32":`
/// is the module's symbol, so a load of it must resolve.
pub fn collect_bindings(module: &mut Module<'_>, stmts: &[Stmt]) {
    let qname = module.qname.clone();
    for st in stmts {
        match st {
            Stmt::Import(_) | Stmt::ImportFrom(_) => {
                for (local, target) in imports(module, st) {
                    module.bindings.insert(local.into(), target.into());
                }
            }
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {
                let (name, _, _) = def_parts(st).expect("a def or class");
                module
                    .bindings
                    .insert(name.into(), format!("{qname}.{name}").into());
            }
            Stmt::Assign(_) | Stmt::AnnAssign(_) => {
                let targets: Vec<&Expr> = match st {
                    Stmt::Assign(a) => a.targets.iter().collect(),
                    Stmt::AnnAssign(a) => vec![&a.target],
                    _ => unreachable!(),
                };
                for t in targets {
                    // only stored names bind: `os.environ[k] = v` reads `os`
                    for name in stored_names(t) {
                        module
                            .bindings
                            .insert(name.as_str().into(), format!("{qname}.{name}").into());
                    }
                }
            }
            // a TYPE_CHECKING body binds nothing at run time
            _ if !type_checking_guard(st) => {
                for block in blocks(st) {
                    collect_bindings(module, block);
                }
            }
            _ => {}
        }
    }
}

/// The `Name` nodes of a target expression whose context is `Store`, in
/// `ast.walk` order (breadth first, the root included).
fn stored_names(target: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    let mut queue: Vec<Cn<'_>> = vec![Cn::Expr(target)];
    let mut scratch: Vec<Cn<'_>> = Vec::new();
    let mut at = 0;
    while at < queue.len() {
        let node = queue[at];
        at += 1;
        if let Cn::Expr(Expr::Name(n)) = node
            && n.ctx.is_store()
        {
            out.push(n.id.to_string());
        }
        scratch.clear();
        children(node, &mut scratch);
        queue.extend(scratch.iter().copied());
    }
    out
}

/// `if TYPE_CHECKING:` or `if typing.TYPE_CHECKING:`. An import under it is
/// a checker's binding, never the interpreter's.
pub fn type_checking_guard(st: &Stmt) -> bool {
    let Stmt::If(n) = st else {
        return false;
    };
    match n.test.as_ref() {
        Expr::Name(name) => name.id.as_str() == "TYPE_CHECKING",
        Expr::Attribute(a) => a.attr.as_str() == "TYPE_CHECKING",
        _ => false,
    }
}

// --- the symbol and class index ---------------------------------------------

/// One write into the repo-wide symbol table, replayed in module order.
pub enum SymbolOp {
    /// `facts.symbols[q] = sym`
    Set(Symbol),
    /// `facts.symbols.setdefault(q, sym)`
    Default(Symbol),
}

/// One write into the repo-wide class table, replayed in module order.
pub enum ClassOp {
    Define(ClassInfo),
    Method {
        class_q: Qname,
        name: Box<str>,
        method_q: Qname,
    },
}

pub struct ScopeIndex {
    pub symbols: Vec<SymbolOp>,
    pub classes: Vec<ClassOp>,
}

/// `_index_scope`: the symbols a module declares, with the class table and
/// the method slots. Positions come from the module's span table, so a
/// decorated def is numbered at its keyword line (R1).
pub fn index_scope(module: &Module<'_>) -> ScopeIndex {
    let mut out = ScopeIndex {
        symbols: Vec::new(),
        classes: Vec::new(),
    };
    let body = &module.parsed.syntax().body;
    let qname = module.qname.clone();
    walk_scope(module, body, &qname, false, true, &mut out);
    out
}

fn walk_scope(
    module: &Module<'_>,
    stmts: &[Stmt],
    parent_q: &Qname,
    in_class: bool,
    top: bool,
    out: &mut ScopeIndex,
) {
    let parent: Option<Qname> = (!top).then(|| parent_q.clone());
    for st in stmts {
        match def_parts(st) {
            Some((name, body, is_class)) => {
                let mut q = format!("{parent_q}.{name}");
                let accessor = if in_class {
                    property_accessor(st, name)
                } else {
                    None
                };
                // `@x.setter` is a second body under the getter's name: its
                // own symbol, so no reader folds two bodies into one function
                if let Some(acc) = accessor {
                    q.push('.');
                    q.push_str(acc);
                }
                let q: Qname = q.as_str().into();
                let kind = if is_class {
                    "class"
                } else if in_class {
                    "method"
                } else {
                    "function"
                };
                let node = node_of(st);
                out.symbols.push(SymbolOp::Set(symbol(
                    module,
                    q.clone(),
                    name,
                    kind,
                    node,
                    top,
                    parent.clone(),
                )));
                if is_class {
                    out.classes.push(ClassOp::Define(ClassInfo {
                        qname: q.clone(),
                        module: module.qname.clone(),
                        node,
                        bases: Vec::new(),
                        external_bases: Vec::new(),
                        methods: IndexMap::new(),
                        subclasses: Vec::new(),
                    }));
                } else if in_class && accessor.is_none() {
                    out.classes.push(ClassOp::Method {
                        class_q: parent_q.clone(),
                        name: name.into(),
                        method_q: q.clone(),
                    });
                }
                walk_scope(module, body, &q, is_class, false, out);
            }
            None => match st {
                Stmt::Assign(_) | Stmt::AnnAssign(_) if top || in_class => {
                    let targets: Vec<&Expr> = match st {
                        Stmt::Assign(a) => a.targets.iter().collect(),
                        Stmt::AnnAssign(a) => vec![&a.target],
                        _ => unreachable!(),
                    };
                    let node = node_of(st);
                    for t in targets {
                        for name in walk_names(t) {
                            let q: Qname = format!("{parent_q}.{name}").as_str().into();
                            out.symbols.push(SymbolOp::Default(symbol(
                                module,
                                q,
                                &name,
                                "variable",
                                node,
                                top,
                                parent.clone(),
                            )));
                        }
                    }
                }
                // a def nested under control flow keeps its scope-level qname
                _ => {
                    for block in blocks(st) {
                        walk_scope(module, block, parent_q, in_class, top, out);
                    }
                }
            },
        }
    }
}

/// Every `Name` of a target expression, whatever its context, in `ast.walk`
/// order: the symbol index binds a name it stores or annotates alike.
fn walk_names(target: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    let mut queue: Vec<Cn<'_>> = vec![Cn::Expr(target)];
    let mut scratch: Vec<Cn<'_>> = Vec::new();
    let mut at = 0;
    while at < queue.len() {
        let node = queue[at];
        at += 1;
        if let Cn::Expr(Expr::Name(n)) = node {
            out.push(n.id.to_string());
        }
        scratch.clear();
        children(node, &mut scratch);
        queue.extend(scratch.iter().copied());
    }
    out
}

fn symbol(
    module: &Module<'_>,
    qname: Qname,
    name: &str,
    kind: &'static str,
    node: NodeIndex,
    top: bool,
    parent: Option<Qname>,
) -> Symbol {
    let is_public = !name.starts_with('_')
        && match (top, &module.all_names) {
            (true, Some(all)) => all.iter().any(|n| &**n == name),
            _ => true,
        };
    Symbol {
        qname,
        module: module.qname.clone(),
        name: name.into(),
        kind,
        node,
        lineno: module.line_of(node),
        end_lineno: module.end_line_of(node),
        is_public,
        parent,
    }
}

/// The traversal index the statement was stamped with.
fn node_of(st: &Stmt) -> NodeIndex {
    use ruff_python_ast::HasNodeIndex;
    st.node_index().load().as_u32().unwrap_or(0)
}

// --- the cross-module indexes -----------------------------------------------

/// `_link_subclasses`: a class base that names a repo class links both ways;
/// anything else is an external base, spelled through the module's bindings
/// or, where the root is unbound, unparsed.
pub fn link_subclasses(facts: &mut RepoFacts<'_>) {
    enum Link {
        Internal(Qname, Qname),
        External(Qname, String),
    }
    let mut links: Vec<Link> = Vec::new();
    for info in facts.classes.values() {
        let Some(module) = facts.modules.get(&info.module) else {
            continue;
        };
        let Cn::Stmt(Stmt::ClassDef(node)) = module.nodes[info.node as usize] else {
            continue;
        };
        for base_expr in class_bases(node) {
            let head = match base_expr {
                Expr::Subscript(s) => s.value.as_ref(),
                other => other,
            };
            match resolve_dotted_expr(head, module, facts) {
                Some(target) if facts.classes.contains_key(&target) => {
                    links.push(Link::Internal(info.qname.clone(), target));
                }
                _ => links.push(Link::External(
                    info.qname.clone(),
                    module
                        .dotted_name(head)
                        .unwrap_or_else(|| crate::unparse::expr(base_expr)),
                )),
            }
        }
    }
    for link in links {
        match link {
            Link::Internal(child, base) => {
                if let Some(info) = facts.classes.get_mut(&child) {
                    info.bases.push(base.clone());
                }
                if let Some(info) = facts.classes.get_mut(&base) {
                    info.subclasses.push(child);
                }
            }
            Link::External(child, text) => {
                if let Some(info) = facts.classes.get_mut(&child) {
                    info.external_bases.push(text);
                }
            }
        }
    }
}

/// The positional bases of a class, which is what CPython's `ClassDef.bases`
/// holds; ruff keeps them beside the keywords in one `Arguments`.
fn class_bases(node: &StmtClassDef) -> impl Iterator<Item = &Expr> {
    node.arguments.iter().flat_map(|a| a.args.iter())
}

/// The CHA method table and the proxy mark, both read by pass B.
pub fn method_index(facts: &mut RepoFacts<'_>) {
    let mut index: HashMap<Box<str>, Vec<Qname>> = HashMap::new();
    for cls in facts.classes.values() {
        for (name, q) in &cls.methods {
            index.entry(name.clone()).or_default().push(q.clone());
        }
    }
    // a production `__getattr__` may route any attribute name to a repo
    // body; a test's fake proxy never runs in the program judged
    facts.proxied = facts.classes.values().any(|cls| {
        cls.methods.contains_key("__getattr__")
            && facts
                .modules
                .get(&cls.module)
                .is_some_and(|m| !is_test_path(&m.rel))
    });
    facts.method_index = index;
}

/// Everything pass A computes for one module, before the repo-wide merge.
pub struct PassA<'t> {
    pub traversal: Traversal<'t>,
    pub comments: Vec<crate::module::Comment>,
}

/// The per-module half of pass A: the marks, the traversal, the spans and
/// the comment tokens. Runs under rayon (`build.rs`).
pub fn pass_a<'t>(source: &'t Source, lines: &Lines) -> PassA<'t> {
    let module = source.parsed.syntax();
    let marks = marks(module, &source.qname);
    let ignores = type_ignores(&source.parsed, &source.text, lines);
    PassA {
        traversal: traverse(module, &source.qname, &source.text, lines, &marks, &ignores),
        comments: crate::model::comments(&source.parsed, &source.text, lines),
    }
}
