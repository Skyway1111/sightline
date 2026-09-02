//! The Rust index in process: `RsEdge` and `RsGraph`, the resolved
//! references by either end, and `graph`, which loads every project root,
//! resolves every token of every module file and joins the definitions to
//! facts symbols.

mod join;
mod load;

use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;
use std::time::Instant;

use hir::Semantics;
use ide_db::RootDatabase;
use indexmap::IndexMap;
use rayon::prelude::*;
use sightline_rs_facts::model::RsFacts;

use crate::oracle::RsOracle;
use join::{Resolved, Sites, Symbols};

/// The index joined to facts, one graph for every project root, the roots
/// loaded in sequence. A root whose load fails answers nothing and is down
/// for every later pass (`RsOracle::fail`). The roots are disjoint, so a
/// module file resolves under exactly one of them, the one its rel sits in.
pub fn graph(oracle: &RsOracle, facts: &RsFacts<'_>) -> RsGraph {
    let known = Symbols::new(facts);
    let sites = Sites::new(facts);
    let rels: Vec<String> = facts.modules.values().map(|m| m.rel.to_string()).collect();
    let mut found: Vec<Resolved> = Vec::new();
    let (mut inside, mut outside) = (BTreeSet::new(), BTreeSet::new());
    for project in oracle.roots.iter().filter(|p| !oracle.is_down(p)) {
        let started = Instant::now();
        let loaded = match load::load(oracle, project, &rels) {
            Ok(loaded) => loaded,
            Err(err) => {
                let why = err.to_string();
                let first = why.lines().next().unwrap_or_default();
                oracle.fail(Some(project), &format!("workspace load: {first}"));
                continue;
            }
        };
        let home = match oracle.home(project) {
            found if found.is_empty() => String::new(),
            found => format!("{found}/"),
        };
        let mine: Vec<&String> = rels.iter().filter(|rel| rel.starts_with(&home)).collect();
        let files = &loaded.files;
        found.extend(resolve_files(&loaded.db, files, &mine, &known, &sites));
        inside.extend(files.documents_in.iter().cloned());
        outside.extend(files.documents_out.iter().cloned());
        oracle.event("index", started);
        if !loaded.close() {
            eprintln!("sightline: rs index left its temp directory behind");
        }
    }
    let sites = facts.call_sites.len();
    let counts = join::counts(&found, sites, (inside.len(), outside.len()));
    let mut edges: Vec<RsEdge> = found.into_iter().map(|it| it.edge).collect();
    edges.sort_by(|a, b| {
        (&a.rel, a.line, &a.callee, &a.caller, a.call)
            .cmp(&(&b.rel, b.line, &b.callee, &b.caller, b.call))
    });
    RsGraph::new(edges, counts)
}

/// Every indexed token of one root's files resolved and joined, one
/// `db.clone()` snapshot per worker of the bin's global rayon pool. The seed
/// is cloned outside the parallel iterator: a `&RootDatabase` would ask the
/// database to be `Sync`. Files merge in `mine` order, so the bytes do not
/// move with the thread count.
fn resolve_files(
    db: &RootDatabase,
    files: &load::Files,
    mine: &[&String],
    known: &Symbols,
    sites: &Sites,
) -> Vec<Resolved> {
    let seed = db.clone();
    let per_file: Vec<Vec<Resolved>> = mine
        .par_iter()
        .map_with(seed, |db, rel| {
            hir::attach_db(db, || {
                let sema = Semantics::new(&*db);
                files
                    .token_rows(&sema, rel)
                    .iter()
                    .filter_map(|row| join::edge_of(known, sites, row, &files.defs_at(&sema, row)))
                    .collect()
            })
        })
        .collect();
    per_file.into_iter().flatten().collect()
}

/// One reference rust-analyzer resolved to a symbol this repo declares.
/// `open` marks a callee that is a trait's declaration rather than an impl:
/// the body that runs is not the one it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsEdge {
    /// the symbol the site sits in; the module qname at top level
    pub caller: String,
    pub callee: String,
    pub rel: String,
    pub line: u32,
    /// the site is a call, not a plain reference
    pub call: bool,
    pub open: bool,
}

#[derive(Default)]
pub struct RsGraph {
    pub edges: Vec<RsEdge>,
    pub counts: IndexMap<String, u64>,
    ends: OnceLock<Ends>,
}

#[derive(Default)]
struct Ends {
    into: HashMap<String, Vec<usize>>,
    out: HashMap<String, Vec<usize>>,
}

impl RsGraph {
    pub fn new(edges: Vec<RsEdge>, counts: IndexMap<String, u64>) -> RsGraph {
        RsGraph {
            edges,
            counts,
            ends: OnceLock::new(),
        }
    }

    fn ends(&self) -> &Ends {
        self.ends.get_or_init(|| {
            let mut ends = Ends::default();
            for (i, edge) in self.edges.iter().enumerate() {
                ends.into.entry(edge.callee.clone()).or_default().push(i);
                ends.out.entry(edge.caller.clone()).or_default().push(i);
            }
            ends
        })
    }

    /// Every resolved reference to the symbol, calls and plain refs.
    pub fn edges_to(&self, qname: &str) -> Vec<&RsEdge> {
        self.pick(self.ends().into.get(qname))
    }

    pub fn edges_from(&self, qname: &str) -> Vec<&RsEdge> {
        self.pick(self.ends().out.get(qname))
    }

    // sightline-ok: 11 - one filter over each end of the graph
    pub fn calls_to(&self, qname: &str) -> Vec<&RsEdge> {
        self.edges_to(qname)
            .into_iter()
            .filter(|e| e.call)
            .collect()
    }

    // sightline-ok: 11 - one filter over each end of the graph
    pub fn calls_from(&self, qname: &str) -> Vec<&RsEdge> {
        self.edges_from(qname)
            .into_iter()
            .filter(|e| e.call)
            .collect()
    }

    fn pick(&self, rows: Option<&Vec<usize>>) -> Vec<&RsEdge> {
        rows.map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|i| &self.edges[*i])
            .collect()
    }
}
