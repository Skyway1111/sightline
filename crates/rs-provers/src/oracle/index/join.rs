//! The half of `rs/resolve.py:graph` that needs no toolchain: what the join
//! asks of facts (`_Symbols`, `_callee_name`, `_macro_call`), the fold of one
//! site's definitions into an edge, and the counts.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use sightline_core::findings::Qname;
use sightline_rs_facts::Node;
use sightline_rs_facts::model::RsFacts;

use crate::oracle::index::RsEdge;

/// `(rel, line, byte col, text)`: the key a site, a call and a macro call
/// share, byte columns on both sides (the LSIF reader's UTF-16 is gone).
pub type Key = (String, u32, u32, String);

/// One position to resolve; `at` is its byte offset in the file.
pub struct Row {
    pub key: Key,
    pub at: u32,
}

/// Where a resolved definition landed. An empty `ident` is a range with no
/// identifier to read (a definition whose range spans lines).
pub struct DefSite {
    pub rel: String,
    pub line: u32,
    pub ident: String,
}

/// One site's reading. `site` marks a key facts calls a call site, which
/// tells a call edge from a call written inside a macro.
pub struct Resolved {
    pub edge: RsEdge,
    pub site: bool,
}

/// What the join asks of facts: the symbol a definition names, the one a
/// site sits in.
pub struct Symbols {
    named: HashMap<(String, u32, String), Qname>,
    traits: HashMap<String, Vec<(u32, u32, Qname)>>,
    spans: HashMap<String, Vec<(u32, u32, Qname)>>,
    module: HashMap<String, Qname>,
}

impl Symbols {
    pub fn new(facts: &RsFacts<'_>) -> Symbols {
        let mut this = Symbols {
            named: HashMap::new(),
            traits: HashMap::new(),
            spans: HashMap::new(),
            module: facts
                .modules
                .values()
                .map(|m| (m.rel.to_string(), m.qname.clone()))
                .collect(),
        };
        for (qname, sym) in &facts.symbols {
            let rel = facts.rel_of(&sym.module).to_string();
            let span = (sym.lineno, sym.end_lineno, qname.clone());
            this.named
                .entry((rel.clone(), sym.lineno, sym.name.clone()))
                .or_insert_with(|| qname.clone());
            this.spans
                .entry(rel.clone())
                .or_default()
                .push(span.clone());
            if sym.kind == "trait" {
                this.traits.entry(rel).or_default().push(span);
            }
        }
        for found in this.spans.values_mut() {
            found.sort();
        }
        this
    }

    /// The symbol a definition range names, and whether it is open. A
    /// definition naming no symbol inside a trait's span is that trait's
    /// declaration; one naming none is no edge.
    fn symbol_at(&self, site: &DefSite) -> Option<(Qname, bool)> {
        let key = (site.rel.clone(), site.line, site.ident.clone());
        if let Some(qname) = self.named.get(&key) {
            return Some((qname.clone(), false));
        }
        self.traits
            .get(&site.rel)
            .into_iter()
            .flatten()
            .rev()
            .find(|(start, end, _)| *start <= site.line && site.line <= *end)
            .map(|(_, _, trait_q)| (trait_q.clone(), true))
    }

    /// The innermost symbol whose span holds the line; the module at top level.
    fn owner(&self, rel: &str, line: u32) -> String {
        let found = self.spans.get(rel).map(Vec::as_slice).unwrap_or_default();
        let cut = found.partition_point(|(start, _, _)| *start <= line);
        match found[..cut].iter().rev().find(|(_, end, _)| *end >= line) {
            Some((_, _, qname)) => qname.to_string(),
            None => self
                .module
                .get(rel)
                .map(Qname::to_string)
                .unwrap_or_default(),
        }
    }
}

/// The two lookups an edge's `caller` and `call` come from, keyed by the
/// callee identifier's own column so a value reference on the line stays
/// a reference.
pub struct Sites {
    calls: HashMap<Key, String>,
    macro_calls: HashSet<Key>,
}

impl Sites {
    pub fn new(facts: &RsFacts<'_>) -> Sites {
        let mut calls: HashMap<Key, String> = HashMap::new();
        for site in &facts.call_sites {
            let module = &facts.modules[&*site.module];
            if let Some(name) = callee_name(site.node) {
                calls
                    .entry(key_of(&module.rel, name, &module.text(name)))
                    .or_insert_with(|| site.enclosing.to_string());
            }
        }
        // the same key for a call no call site covers (`_macro_call`)
        let mut macro_calls: HashSet<Key> = HashSet::new();
        for found in &facts.refs {
            let module = &facts.modules[&*found.module];
            if macro_call(&module.lines, found.node) {
                let node = found.node;
                macro_calls.insert(key_of(&module.rel, node, &module.text(node)));
            }
        }
        Sites { calls, macro_calls }
    }
}

fn key_of(rel: &str, node: Node<'_>, text: &str) -> Key {
    let (line, col) = (
        node.start_position().row as u32 + 1,
        node.start_position().column as u32,
    );
    (rel.to_string(), line, col, text.to_string())
}

/// One site folded into an edge, as `graph` folds one reference: a single
/// distinct callee or nothing. A callee both named and trait-open reads as
/// not open (`decisions.tsv`); no site of the three trees ties.
pub fn edge_of(known: &Symbols, sites: &Sites, row: &Row, defs: &[DefSite]) -> Option<Resolved> {
    let found: HashSet<(Qname, bool)> = defs.iter().filter_map(|d| known.symbol_at(d)).collect();
    let callees: HashSet<&Qname> = found.iter().map(|(callee, _)| callee).collect();
    if callees.len() != 1 {
        return None;
    }
    let (callee, open) = found.iter().min()?;
    let (rel, line, _col, _text) = &row.key;
    let call = sites.calls.get(&row.key);
    let in_macro = call.is_none() && sites.macro_calls.contains(&row.key);
    Some(Resolved {
        site: call.is_some(),
        edge: RsEdge {
            caller: call.cloned().unwrap_or_else(|| known.owner(rel, *line)),
            callee: callee.to_string(),
            rel: rel.clone(),
            line: *line,
            call: call.is_some() || in_macro,
            open: *open,
        },
    })
}

/// `resolve.graph`'s counts, `documents_*` the vfs `.rs` counts inside and
/// outside the audited root.
pub fn counts(
    found: &[Resolved],
    call_sites: usize,
    docs: (usize, usize),
) -> IndexMap<String, u64> {
    let n = |f: fn(&Resolved) -> bool| found.iter().filter(|r| f(r)).count() as u64;
    let (resolved, opened) = (n(|r| r.site && !r.edge.open), n(|r| r.site && r.edge.open));
    let macros = n(|r| !r.site && r.edge.call);
    let sites = call_sites as u64;
    let unresolved = sites.saturating_sub(resolved + opened);
    let cross = n(|r| crate_of(&r.edge.caller) != crate_of(&r.edge.callee));
    [
        ("documents_in", docs.0 as u64),
        ("documents_out", docs.1 as u64),
        ("call_sites", sites),
        ("call_edges", resolved),
        ("open_edges", opened),
        ("macro_edges", macros),
        ("refs", found.len() as u64 - resolved - opened - macros),
        ("unresolved_call_sites", unresolved),
        ("cross_crate_edges", cross),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value))
    .collect()
}

fn crate_of(qname: &str) -> &str {
    qname.split("::").next().unwrap_or(qname)
}

/// The identifier a call site spells its callee by: the method of a field
/// call, the last segment of a path, the turbofish's own function.
fn callee_name<'t>(node: Node<'t>) -> Option<Node<'t>> {
    let mut fun = node.child_by_field_name("function")?;
    if fun.kind() == "generic_function" {
        fun = fun.child_by_field_name("function").unwrap_or(fun);
    }
    match fun.kind() {
        "field_expression" => fun.child_by_field_name("field"),
        "scoped_identifier" => fun.child_by_field_name("name"),
        "identifier" => Some(fun),
        _ => None,
    }
}

/// A call written among a macro invocation's tokens, which tree-sitter
/// leaves unparsed (a `macro_rules!` body's are a pattern, not code): the
/// source says it, by the argument list or turbofish opening right after.
fn macro_call(lines: &[&str], node: Node<'_>) -> bool {
    let mut up = node.parent();
    if up.map(|p| p.kind()) != Some("token_tree") {
        return false;
    }
    while let Some(found) = up.filter(|p| p.kind() == "token_tree") {
        up = found.parent();
    }
    let end = node.end_position();
    let tail = lines
        .get(end.row)
        .and_then(|line| line.as_bytes().get(end.column..))
        .unwrap_or_default();
    up.is_some_and(|p| p.kind() == "macro_invocation")
        && (tail.starts_with(b"(") || tail.starts_with(b"::<"))
}
