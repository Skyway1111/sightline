//! `facts/build.py` pass B: the per-module walk that records references and
//! resolves call sites.
//!
//! One `Resolver` per module, run under rayon; `build.rs` appends each
//! module's `Vec<Ref>` and `Vec<CallSite>` in `facts.modules` order, so one
//! thread and all cores answer the same lists (decision 8). A visited node's
//! `NodeIndex` is the one the traversal stamped on it (R3), so no reader
//! recomputes a position.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use ruff_python_ast::{
    Alias, Expr, ExprCall, ExprContext, ExprLambda, Stmt, StmtClassDef, StmtFunctionDef,
};
use sightline_core::findings::Qname;
use sightline_core::pytext::join_path;

use crate::astutil::{RECEIVERS, all_arg_names, fn_args};
use crate::cn::Cn;
use crate::index::imports;
use crate::kinds::Kind;
use crate::kinds::is_stmt;
use crate::model::{
    CallSite, NodeIndex, Ref, RefKind, RepoFacts, Resolution, Step, class_walk,
    has_framework_base_transitive,
};
use crate::module::Module;
use crate::order::children;
use crate::qnames::{import_alias, resolve_qname};

/// `dir(builtins)` on CPython 3.14, the reference interpreter, sorted as
/// `dir` sorts it. R16's `python_builtins` is the smaller set of names ruff
/// calls builtins; `builtins_hold_every_name_ruff_lists` prices the two.
pub const BUILTINS: &[&str] = &[
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BaseException",
    "BaseExceptionGroup",
    "BlockingIOError",
    "BrokenPipeError",
    "BufferError",
    "BytesWarning",
    "ChildProcessError",
    "ConnectionAbortedError",
    "ConnectionError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "DeprecationWarning",
    "EOFError",
    "Ellipsis",
    "EncodingWarning",
    "EnvironmentError",
    "Exception",
    "ExceptionGroup",
    "False",
    "FileExistsError",
    "FileNotFoundError",
    "FloatingPointError",
    "FutureWarning",
    "GeneratorExit",
    "IOError",
    "ImportError",
    "ImportWarning",
    "IndentationError",
    "IndexError",
    "InterruptedError",
    "IsADirectoryError",
    "KeyError",
    "KeyboardInterrupt",
    "LookupError",
    "MemoryError",
    "ModuleNotFoundError",
    "NameError",
    "None",
    "NotADirectoryError",
    "NotImplemented",
    "NotImplementedError",
    "OSError",
    "OverflowError",
    "PendingDeprecationWarning",
    "PermissionError",
    "ProcessLookupError",
    "PythonFinalizationError",
    "RecursionError",
    "ReferenceError",
    "ResourceWarning",
    "RuntimeError",
    "RuntimeWarning",
    "StopAsyncIteration",
    "StopIteration",
    "SyntaxError",
    "SyntaxWarning",
    "SystemError",
    "SystemExit",
    "TabError",
    "TimeoutError",
    "True",
    "TypeError",
    "UnboundLocalError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeError",
    "UnicodeTranslateError",
    "UnicodeWarning",
    "UserWarning",
    "ValueError",
    "Warning",
    // Windows only, and the pinned expectations were taken there
    "WindowsError",
    "ZeroDivisionError",
    "_IncompleteInputError",
    "__build_class__",
    "__debug__",
    "__doc__",
    "__import__",
    "__loader__",
    "__name__",
    "__package__",
    "__spec__",
    "abs",
    "aiter",
    "all",
    "anext",
    "any",
    "ascii",
    "bin",
    "bool",
    "breakpoint",
    "bytearray",
    "bytes",
    "callable",
    "chr",
    "classmethod",
    "compile",
    "complex",
    "copyright",
    "credits",
    "delattr",
    "dict",
    "dir",
    "divmod",
    "enumerate",
    "eval",
    "exec",
    "exit",
    "filter",
    "float",
    "format",
    "frozenset",
    "getattr",
    "globals",
    "hasattr",
    "hash",
    "help",
    "hex",
    "id",
    "input",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "license",
    "list",
    "locals",
    "map",
    "max",
    "memoryview",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "ord",
    "pow",
    "print",
    "property",
    "quit",
    "range",
    "repr",
    "reversed",
    "round",
    "set",
    "setattr",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "vars",
    "zip",
];

pub fn is_builtin(name: &str) -> bool {
    BUILTINS.binary_search(&name).is_ok()
}

/// One module's pass-B output.
#[derive(Default)]
pub struct Resolved {
    pub refs: Vec<Ref>,
    pub call_sites: Vec<CallSite>,
}

/// A function's locals (name -> the target a function-level import bound it
/// to, else none), or a class body's qname.
enum Frame {
    Function(IndexMap<Box<str>, Option<Qname>>),
    Class(Qname),
}

/// A receiver chain `a.b.c`: its root `Name`, the attribute names in source
/// order, and the `Attribute` node each name closes.
struct Chain<'e> {
    base: &'e Expr,
    id: &'e str,
    parts: Vec<&'e str>,
    nodes: Vec<&'e Expr>,
}

fn chain(expr: &Expr) -> Option<Chain<'_>> {
    let mut parts = Vec::new();
    let mut nodes = Vec::new();
    let mut cur = expr;
    while let Expr::Attribute(a) = cur {
        parts.push(a.attr.as_str());
        nodes.push(cur);
        cur = &a.value;
    }
    let Expr::Name(name) = cur else {
        return None;
    };
    parts.reverse();
    nodes.reverse();
    Some(Chain {
        base: cur,
        id: name.id.as_str(),
        parts,
        nodes,
    })
}

/// The `alias` node of each name one import statement binds, `*` skipped, in
/// the order `index::imports` answers them.
fn aliases(st: &Stmt) -> Vec<&Alias> {
    match st {
        Stmt::Import(imp) => imp.names.iter().collect(),
        Stmt::ImportFrom(imp) => imp
            .names
            .iter()
            .filter(|a| a.name.as_str() != "*")
            .collect(),
        _ => Vec::new(),
    }
}

pub fn resolve<'t>(module: &Module<'t>, facts: &RepoFacts<'t>) -> Resolved {
    // scope qname -> the names declared directly in it, off the scope keys
    let mut kids: HashMap<Box<str>, Vec<Box<str>>> = HashMap::new();
    for scope in module.nodes_by_scope.keys() {
        let (parent, name) = match scope.rfind('.') {
            Some(i) => (&scope[..i], &scope[i + 1..]),
            None => ("", &**scope),
        };
        kids.entry(parent.into()).or_default().push(name.into());
    }
    let mut resolver = Resolver {
        module,
        facts,
        frames: Vec::new(),
        typed: Vec::new(),
        in_signature: false,
        children: kids,
        out: Resolved::default(),
    };
    for st in &module.parsed.syntax().body {
        resolver.visit(Cn::Stmt(st));
    }
    resolver.out
}

struct Resolver<'a, 't> {
    module: &'a Module<'t>,
    facts: &'a RepoFacts<'t>,
    frames: Vec<Frame>,
    /// beside each frame: local name -> the repo class its annotation names
    typed: Vec<HashMap<Box<str>, Qname>>,
    /// inside a def's defaults and annotations
    in_signature: bool,
    children: HashMap<Box<str>, Vec<Box<str>>>,
    out: Resolved,
}

impl<'t> Resolver<'_, 't> {
    fn idx(&self, node: Cn<'t>) -> NodeIndex {
        node.stamped()
            .expect("the traversal stamped every node this walk reaches")
    }

    // --- scope helpers

    /// `(a local of an enclosing function frame, the import target a
    /// function-level import bound it to)`.
    fn lookup_local(&self, name: &str) -> (bool, Option<Qname>) {
        if matches!(self.frames.last(), Some(Frame::Class(_))) {
            return (false, None); // class body names resolve via bindings only
        }
        for frame in self.frames.iter().rev() {
            if let Frame::Function(locals) = frame
                && let Some(target) = locals.get(name)
            {
                return (true, target.clone());
            }
        }
        (false, None)
    }

    /// The dotted target a name denotes here: a function-level import's, else
    /// the module binding when the name is not a plain local.
    fn binding(&self, name: &str) -> Option<Qname> {
        let (local, target) = self.lookup_local(name);
        if local {
            target
        } else {
            self.module.bindings.get(name).cloned()
        }
    }

    /// The class whose body, or whose method (one def deep), we are in.
    fn enclosing_class(&self) -> Option<Qname> {
        let mut depth = 0;
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Class(q) => return (depth <= 1).then(|| q.clone()),
                Frame::Function(_) => depth += 1,
            }
        }
        None
    }

    fn scope_nodes(&self, kinds: &[Kind], scope: &str) -> Vec<NodeIndex> {
        self.module.nodes(kinds, Some(scope), false)
    }

    /// Params plus the names bound in the def's own scope (off the index),
    /// global and nonlocal declarations removed. A nested def or class binds
    /// its name to the nested symbol, as a function-level import binds its
    /// alias to the target: loads of it resolve there, so `refs_to` has a key
    /// for a closure.
    fn locals(&self, def: &StmtFunctionDef, scope: &str) -> IndexMap<Box<str>, Option<Qname>> {
        let mut names: IndexMap<Box<str>, Option<Qname>> = IndexMap::new();
        for i in self.scope_nodes(&[Kind::Name], scope) {
            if let Cn::Expr(Expr::Name(n)) = self.module.nodes[i as usize]
                && matches!(n.ctx, ExprContext::Store | ExprContext::Del)
            {
                names.insert(n.id.as_str().into(), None);
            }
        }
        for i in self.scope_nodes(&[Kind::ExceptHandler], scope) {
            if let Cn::Handler(h) = self.module.nodes[i as usize]
                && let Some(name) = &h.name
            {
                names.insert(name.as_str().into(), None);
            }
        }
        for name in self.children.get(scope).into_iter().flatten() {
            let target: Qname = format!("{scope}.{name}").into();
            names.insert(name.clone(), Some(target));
        }
        for i in self.scope_nodes(&[Kind::Import, Kind::ImportFrom], scope) {
            let Cn::Stmt(st) = self.module.nodes[i as usize] else {
                continue;
            };
            for (local, target) in imports(self.module, st) {
                names.insert(local.into(), Some(target.into()));
            }
        }
        for i in self.scope_nodes(&[Kind::Global, Kind::Nonlocal], scope) {
            let declared: &[ruff_python_ast::Identifier] = match self.module.nodes[i as usize] {
                Cn::Stmt(Stmt::Global(g)) => &g.names,
                Cn::Stmt(Stmt::Nonlocal(n)) => &n.names,
                _ => continue,
            };
            for name in declared {
                names.shift_remove(name.as_str());
            }
        }
        // `{**dict.fromkeys(all_arg_names(fn)), **names}`: the arg keys come
        // first, and `names` overrides a value
        let mut out: IndexMap<Box<str>, Option<Qname>> = IndexMap::new();
        for arg in all_arg_names(Some(&def.parameters)) {
            out.insert(arg.into(), None);
        }
        for (name, target) in names {
            out.insert(name, target);
        }
        out
    }

    /// Local name -> the repo class its param or `AnnAssign` annotation names.
    fn typed(&self, def: &StmtFunctionDef, scope: &str) -> HashMap<Box<str>, Qname> {
        let mut out = HashMap::new();
        for param in fn_args(def) {
            let ann = Cn::Param(param)
                .stamped()
                .and_then(|i| self.module.annotation(i));
            if let Some(q) = self.annotated_class(ann) {
                out.insert(param.name.as_str().into(), q);
            }
        }
        for i in self.scope_nodes(&[Kind::AnnAssign], scope) {
            if let Cn::Stmt(Stmt::AnnAssign(st)) = self.module.nodes[i as usize]
                && let Expr::Name(target) = &*st.target
                && let Some(q) = self.annotated_class(Some(&st.annotation))
            {
                out.insert(target.id.as_str().into(), q);
            }
        }
        out
    }

    fn annotated_class(&self, ann: Option<&Expr>) -> Option<Qname> {
        let dotted = self.module.dotted_name(ann?)?;
        let (kind, q) = resolve_qname(&dotted, self.facts, 0);
        (kind == "symbol" && self.facts.classes.contains_key(&q)).then_some(q)
    }

    // --- recording

    fn add_ref(&mut self, node: Cn<'t>, target: Qname, kind: RefKind) {
        let node = self.idx(node);
        self.out.refs.push(Ref {
            module: self.module.qname.clone(),
            node,
            target,
            kind,
        });
    }

    fn add_call(
        &mut self,
        node: Cn<'t>,
        resolution: Resolution,
        target: Option<Qname>,
        candidates: Vec<Qname>,
    ) {
        if self.in_signature {
            return;
        }
        let node = self.idx(node);
        self.out.call_sites.push(CallSite {
            module: self.module.qname.clone(),
            node,
            enclosing: self.facts.enclosing(self.module, node),
            resolution,
            target,
            candidates,
            lineno: self.module.line_of(node),
        });
    }

    // --- name resolution

    /// `resolve_qname` for a dotted spelling, the re-export hop it takes
    /// recorded as a LOAD of the alias (`from M import name`, `M.name`).
    fn resolve_hop(&mut self, q: &str, node: Cn<'t>) -> (&'static str, Qname) {
        if let Some(alias) = import_alias(q, self.facts) {
            self.add_ref(node, alias, RefKind::Load);
        }
        resolve_qname(q, self.facts, 0)
    }

    /// `(kind, qname)`: local, symbol, module, external or unresolved.
    fn resolve_name(&self, name: &str) -> (&'static str, Option<Qname>) {
        if let Some(bound) = self.binding(name) {
            let (kind, q) = resolve_qname(&bound, self.facts, 0);
            return (kind, matches!(kind, "symbol" | "module").then_some(q));
        }
        if self.lookup_local(name).0 {
            return ("local", None);
        }
        let kind = if is_builtin(name) {
            "external"
        } else {
            "unresolved"
        };
        (kind, None)
    }

    /// A ref on the longest chain prefix that is an internal symbol
    /// (`state.cache` within `state.cache.update(...)`).
    fn ref_chain_prefix(&mut self, base_q: &str, parts: &[&str], nodes: &[&'t Expr]) {
        for j in (1..parts.len()).rev() {
            let (kind, resolved) =
                resolve_qname(&join_path(base_q, &parts[..j], "."), self.facts, 0);
            if kind == "symbol" {
                self.add_ref(Cn::Expr(nodes[j - 1]), resolved, RefKind::Load);
                return;
            }
        }
    }

    // --- call resolution

    fn resolve_call(&mut self, node: Cn<'t>, call: &'t ExprCall) {
        let func = &*call.func;
        if let Expr::Name(name) = func {
            match self.resolve_name(name.id.as_str()) {
                ("symbol", Some(q)) => {
                    self.add_call(node, Resolution::Resolved, Some(q.clone()), Vec::new());
                    self.add_ref(Cn::Expr(func), q, RefKind::Callee);
                }
                (kind, _) => {
                    let hit = if kind == "external" {
                        Resolution::External
                    } else {
                        Resolution::Unresolved
                    };
                    self.add_call(node, hit, None, Vec::new());
                }
            }
            return;
        }
        let Some(ch) = chain(func) else {
            self.add_call(node, Resolution::Unresolved, None, Vec::new());
            return;
        };
        let attr = *ch.parts.last().expect("a chain names one attribute");
        if RECEIVERS.contains(&ch.id)
            && ch.parts.len() == 1
            && let Some(cls_q) = self.enclosing_class()
        {
            self.method_call(node, call, attr, Some(&cls_q));
            return;
        }
        let (kind, q) = self.resolve_name(ch.id);
        match (kind, q) {
            ("symbol" | "module", Some(q)) => {
                let spelled = join_path(&q, &ch.parts, ".");
                match self.resolve_hop(&spelled, Cn::Expr(func)) {
                    ("symbol", fq) => {
                        self.add_call(node, Resolution::Resolved, Some(fq.clone()), Vec::new());
                        self.add_ref(Cn::Expr(func), fq, RefKind::Callee);
                    }
                    ("external", _) => {
                        self.add_call(node, Resolution::External, None, Vec::new());
                    }
                    _ => {
                        self.ref_chain_prefix(&q, &ch.parts, &ch.nodes);
                        self.add_call(node, Resolution::Unresolved, None, Vec::new());
                    }
                }
            }
            ("external", _) => self.add_call(node, Resolution::External, None, Vec::new()),
            // a plain receiver: class-hierarchy analysis by method name
            _ if ch.parts.len() == 1 && self.facts.method_index.contains_key(attr) => {
                self.method_call(node, call, attr, None);
            }
            ("local", _) if ch.parts.len() == 1 && self.no_repo_body(ch.id) => {
                self.add_call(node, Resolution::External, None, Vec::new());
            }
            _ => self.add_call(node, Resolution::Unresolved, None, Vec::new()),
        }
    }

    /// A local's method no repo class defines runs no repo body, unless a
    /// production `__getattr__` proxy exists (the name reaches it) or the
    /// local is annotated with a class on a library base (a template method
    /// there calls the repo's hooks: `Enc(JSONEncoder)`, `e.encode(o)`).
    fn no_repo_body(&self, local: &str) -> bool {
        if self.facts.proxied {
            return false;
        }
        for (frame, typed) in self.frames.iter().rev().zip(self.typed.iter().rev()) {
            if let Frame::Function(locals) = frame
                && locals.contains_key(local)
            {
                return match typed.get(local) {
                    None => true,
                    Some(cls_q) => !has_framework_base_transitive(self.facts, cls_q),
                };
            }
        }
        true
    }

    /// Candidates over `cls_q`'s hierarchy (a self or cls receiver), else
    /// repo-wide by method name.
    fn method_candidates(&self, attr: &str, cls_q: Option<&str>) -> Vec<Qname> {
        let found: Vec<Qname> = match cls_q {
            None => {
                let mut all = self
                    .facts
                    .method_index
                    .get(attr)
                    .cloned()
                    .unwrap_or_default();
                all.sort();
                all
            }
            Some(cls_q) => {
                let mut walked: Vec<Qname> = class_walk(self.facts, cls_q, Step::Bases)
                    .into_iter()
                    .chain(class_walk(self.facts, cls_q, Step::Subclasses))
                    .map(|(q, _)| q)
                    .collect();
                walked.sort();
                walked.dedup();
                walked
                    .iter()
                    .filter_map(|q| self.facts.classes[q].methods.get(attr).cloned())
                    .collect()
            }
        };
        let mut seen: HashSet<Qname> = HashSet::new();
        found
            .into_iter()
            .filter(|q| seen.insert(q.clone()))
            .collect()
    }

    /// One candidate resolves (BY_NAME for a plain receiver: a guess the call
    /// graph re-judges), more than one is ambiguous.
    fn method_call(&mut self, node: Cn<'t>, call: &'t ExprCall, attr: &str, cls_q: Option<&str>) {
        let candidates = self.method_candidates(attr, cls_q);
        match candidates.len() {
            1 => {
                let hit = if cls_q.is_some() {
                    Resolution::Resolved
                } else {
                    Resolution::ByName
                };
                let target = candidates[0].clone();
                self.add_call(node, hit, Some(target.clone()), Vec::new());
                self.add_ref(Cn::Expr(&call.func), target, RefKind::Callee);
            }
            0 => self.add_call(node, Resolution::Unresolved, None, Vec::new()),
            _ => self.add_call(node, Resolution::Ambiguous, None, candidates),
        }
    }

    // --- walk

    fn visit(&mut self, node: Cn<'t>) {
        match node {
            Cn::Stmt(Stmt::FunctionDef(def)) => self.visit_def(node, def),
            Cn::Stmt(Stmt::ClassDef(cls)) => self.visit_class(node, cls),
            // imports are references too, at any scope
            Cn::Stmt(st @ Stmt::ImportFrom(_)) => self.visit_import_from(st),
            Cn::Expr(Expr::Lambda(lambda)) => self.visit_lambda(lambda),
            Cn::Expr(Expr::Call(call)) => self.visit_call(node, call),
            Cn::Expr(expr @ Expr::Name(name)) => self.visit_name(node, expr, name.ctx),
            Cn::Expr(expr @ Expr::Attribute(_)) => self.visit_attribute(node, expr),
            _ => self.generic(node),
        }
    }

    fn generic(&mut self, node: Cn<'t>) {
        let mut kids = Vec::new();
        children(node, &mut kids);
        for child in kids {
            self.visit(child);
        }
    }

    fn enter(&mut self, frame: Frame, typed: HashMap<Box<str>, Qname>, body: &'t [Stmt]) {
        self.frames.push(frame);
        self.typed.push(typed);
        for st in body {
            self.visit(Cn::Stmt(st));
        }
        self.frames.pop();
        self.typed.pop();
    }

    /// Defaults and annotations evaluate in the enclosing scope, once at
    /// definition: their names are refs, their calls no call site of the def
    /// (a `Path(...)` default is not an effect of every call). The flag nests,
    /// so a lambda default does not reopen the def's call sites.
    fn visit_def(&mut self, node: Cn<'t>, def: &'t StmtFunctionDef) {
        for dec in &def.decorator_list {
            self.visit(Cn::Expr(&dec.expression));
        }
        let outer = std::mem::replace(&mut self.in_signature, true);
        self.visit(Cn::Params(&def.parameters));
        if let Some(returns) = &def.returns {
            self.visit(Cn::Expr(returns));
        }
        self.in_signature = outer;
        let scope = self.facts.enclosing(self.module, self.idx(node));
        let frame = Frame::Function(self.locals(def, &scope));
        let typed = self.typed(def, &scope);
        self.enter(frame, typed, &def.body);
    }

    fn visit_class(&mut self, node: Cn<'t>, cls: &'t StmtClassDef) {
        for dec in &cls.decorator_list {
            self.visit(Cn::Expr(&dec.expression));
        }
        for arguments in cls.arguments.iter() {
            for base in &arguments.args {
                self.visit(Cn::Expr(base));
            }
            for kw in &arguments.keywords {
                self.visit(Cn::Expr(&kw.value));
            }
        }
        let qname = self.facts.enclosing(self.module, self.idx(node));
        self.enter(Frame::Class(qname), HashMap::new(), &cls.body);
    }

    fn visit_import_from(&mut self, st: &'t Stmt) {
        for ((_, target), alias) in imports(self.module, st).into_iter().zip(aliases(st)) {
            let (kind, resolved) = self.resolve_hop(&target, Cn::Alias(alias));
            if matches!(kind, "symbol" | "module") {
                self.add_ref(Cn::Alias(alias), resolved, RefKind::Load);
            }
        }
    }

    /// A lambda's params, plus any walrus store in its body, are its frame.
    fn visit_lambda(&mut self, lambda: &'t ExprLambda) {
        let outer = std::mem::replace(&mut self.in_signature, true);
        if let Some(params) = &lambda.parameters {
            self.visit(Cn::Params(params));
        }
        self.in_signature = outer;
        let mut frame: IndexMap<Box<str>, Option<Qname>> = IndexMap::new();
        for arg in all_arg_names(lambda.parameters.as_deref()) {
            frame.insert(arg.into(), None);
        }
        for node in crate::astutil::walk(Cn::Expr(&lambda.body)) {
            if let Cn::Expr(Expr::Name(n)) = node
                && n.ctx == ExprContext::Store
            {
                frame.insert(n.id.as_str().into(), None);
            }
        }
        self.frames.push(Frame::Function(frame));
        self.typed.push(HashMap::new());
        self.visit(Cn::Expr(&lambda.body));
        self.frames.pop();
        self.typed.pop();
    }

    fn visit_call(&mut self, node: Cn<'t>, call: &'t ExprCall) {
        self.resolve_call(node, call);
        for arg in &call.arguments.args {
            self.visit(Cn::Expr(arg));
        }
        for kw in &call.arguments.keywords {
            self.visit(Cn::Expr(&kw.value));
        }
        // the callee's own ref is recorded; visit its base
        let func = &*call.func;
        if !matches!(func, Expr::Name(_)) {
            match chain(func) {
                None => self.visit(Cn::Expr(func)),
                Some(ch) => self.visit(Cn::Expr(ch.base)),
            }
        }
    }

    fn visit_name(&mut self, node: Cn<'t>, expr: &'t Expr, ctx: ExprContext) {
        let Expr::Name(name) = expr else { return };
        if ctx == ExprContext::Load {
            if let ("symbol" | "module", Some(q)) = self.resolve_name(name.id.as_str()) {
                self.add_ref(node, q, RefKind::Load);
            }
            return;
        }
        if matches!(self.frames.last(), Some(Frame::Class(_))) {
            return;
        }
        // a store or del rebinds the frame's own name: the module's (at module
        // scope or under `global`; an import alias rebound is the module's
        // binding, never the origin's) or a nested def's. `X += 1` and `del X`
        // read the name too.
        let (local, target) = self.lookup_local(name.id.as_str());
        let bound = if local {
            target
        } else {
            Some(Qname::from(format!(
                "{}.{}",
                self.module.qname,
                name.id.as_str()
            )))
        };
        let Some(bound) = bound else { return };
        let Some(sym) = self.facts.symbols.get(&bound) else {
            return;
        };
        if local && sym.parent.is_none() {
            return;
        }
        let def_node = sym.node;
        let mut stmt = Some(self.idx(node));
        while let Some(i) = stmt {
            if is_stmt(self.module.nodes[i as usize].kind()) {
                break;
            }
            stmt = self.module.parent_of(i);
        }
        if stmt == Some(def_node) {
            return; // the def itself
        }
        self.add_ref(node, bound.clone(), RefKind::Store);
        if let Some(i) = stmt
            && matches!(
                self.module.nodes[i as usize].kind(),
                Kind::AugAssign | Kind::Delete
            )
        {
            self.add_ref(node, bound, RefKind::Load);
        }
    }

    fn visit_attribute(&mut self, node: Cn<'t>, expr: &'t Expr) {
        let Expr::Attribute(attribute) = expr else {
            return;
        };
        let Some(ch) = chain(expr) else {
            self.visit(Cn::Expr(&attribute.value));
            return;
        };
        match self.binding(ch.id) {
            Some(bound) => {
                let spelled = join_path(&bound, &ch.parts, ".");
                match self.resolve_hop(&spelled, node) {
                    ("symbol", fq) => {
                        // `mod.NAME = x` is a store on the target, not a read
                        // (rule #9: cross-module reassignment)
                        let store = matches!(attribute.ctx, ExprContext::Store | ExprContext::Del);
                        let kind = if store { RefKind::Store } else { RefKind::Load };
                        self.add_ref(node, fq, kind);
                    }
                    _ => self.ref_chain_prefix(&bound, &ch.parts, &ch.nodes),
                }
            }
            // `self.method` passed by value (a callback) is a reference the
            // call graph never sees
            None if RECEIVERS.contains(&ch.id)
                && ch.parts.len() == 1
                && attribute.ctx == ExprContext::Load =>
            {
                let cls_q = self.enclosing_class();
                let candidates = self.method_candidates(ch.parts[0], cls_q.as_deref());
                if candidates.len() == 1 && cls_q.is_some() {
                    self.add_ref(node, candidates[0].clone(), RefKind::Load);
                }
            }
            None => {}
        }
        self.visit(Cn::Expr(ch.base));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_sorted_so_the_membership_test_can_bisect() {
        let mut sorted = BUILTINS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, BUILTINS);
        assert_eq!(BUILTINS.len(), 160);
        assert!(is_builtin("print") && is_builtin("__build_class__"));
        assert!(!is_builtin("nonesuch"));
    }

    /// R16 reads ruff's list; `dir(builtins)` is wider, and the resolver
    /// answers `dir`. The names beyond ruff's are listed here, so a ruff bump
    /// that moves them fails this test rather than a corpus audit.
    #[test]
    fn builtins_hold_every_name_ruff_lists() {
        let ruff: HashSet<&str> =
            ruff_python_stdlib::builtins::python_builtins(14, false).collect();
        let ours: HashSet<&str> = BUILTINS.iter().copied().collect();
        let missing: Vec<&&str> = ruff.difference(&ours).collect();
        assert!(missing.is_empty(), "ruff names {missing:?}, dir does not");
        let mut extra: Vec<&str> = ours.difference(&ruff).copied().collect();
        extra.sort_unstable();
        // `WindowsError` is an alias only this platform binds, and ruff's
        // table has yet to learn 3.14's `_IncompleteInputError`
        assert_eq!(extra, ["WindowsError", "_IncompleteInputError"]);
    }
}
