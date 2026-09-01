//! Port of `provers/counterfactual.py` and `provers/__init__.py:_placed`
//! (phase 4, unit `py-counterfactual`): the counterfactual arbiter. A
//! proposal is one exact splice, judged by the oracle's worlds: a new error
//! in the proposal's watched files is a `Veto`, a `reportUnnecessary*` newly
//! firing in the callee body is the `Receipt`, else verified non-breaking.
//! The split of a merged world's implicated set is `core::worlds::vetoed`.
//!
//! Splices land in line, so line numbers are preserved and the diff is
//! sound; a needed import rides an existing top-of-file line for the same
//! reason (`ride_import_line`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use regex::{Captures, Regex};
use ruff_python_ast::Stmt;
use ruff_python_stdlib::sys::is_known_standard_library;
use ruff_text_size::Ranged;
use serde_json::{Value, json};

use sightline_core::edits::{apply_edits, char_slice};
use sightline_core::findings::{Evidence, Fix, Rel, SpanEdit};
use sightline_core::pytext;
use sightline_core::worlds::{Spliced, World, errored, vetoed};
use sightline_py_facts::astutil::{fn_body, line_span};
use sightline_py_facts::lines::Lines;
use sightline_py_facts::model::{RepoFacts, source_lines};
use sightline_py_facts::module::Module;
use sightline_py_facts::qnames::resolve_qname;

use crate::Provers;
use crate::callgraph::{CallGraph, callers_of};
use crate::oracle::{Oracle, OracleDiag, UNNECESSARY_RULES};

mod layer;
mod overlay;
mod spelling;
mod verify;

pub use layer::dump;
pub use spelling::{from_home, merge_imports, respell, spell};
pub use verify::verify;

/// A rule's proposal in the rule's own terms: the exact edits inside the
/// symbol it names (a module qname when the edit sits outside any symbol).
/// `spelling` is the type text whose names the file must import; `imports`
/// are statements the splice brings itself (#35's hoisted import); `param`
/// is the parameter the edit annotates (#5's lift, #10's widening).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Splice {
    pub id: String,
    pub owner: String,
    pub edits: Vec<SpanEdit>,
    pub spelling: String,
    pub imports: Vec<String>,
    pub param: String,
}

/// A `Splice` placed in the tree: the file, the callee body span a Receipt
/// may fire in (`(0, 0)` for a module owner), every caller file the veto
/// watches (`None` where the edit's dependents cannot be enumerated: every
/// file is watched), the import statements the splice needs and the
/// parameter it annotates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub id: String,
    pub owner: String,
    pub rel: Rel,
    pub edits: Vec<SpanEdit>,
    pub span: (u32, u32),
    pub watched: Option<std::collections::HashSet<String>>,
    pub imports: Vec<String>,
    pub param: String,
}

impl Proposal {
    /// The emitter's payload: exactly the splice the world verified.
    pub fn fix(&self) -> Fix {
        Fix {
            rel: self.rel.clone(),
            edits: self.edits.clone(),
            imports: self.imports.clone(),
        }
    }
}

impl Spliced for Proposal {
    fn id(&self) -> &str {
        &self.id
    }

    fn rel(&self) -> &str {
        &self.rel
    }

    fn span(&self) -> (u32, u32) {
        self.span
    }

    fn watched(&self) -> Option<&HashSet<String>> {
        self.watched.as_ref()
    }
}

/// What a world said of one proposal (`Veto | Receipt | None` in Python).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// the splice raised a new error in a watched file
    Veto,
    /// the `reportUnnecessary*` message that newly fired in the body
    Receipt(String),
    Clean,
}

/// `Provers._placed`: a splice placed in the tree, `None` where the module
/// or the spelling cannot be placed (an unsafe spelling: unbound everywhere,
/// or bound to something else). A module owner (an edit outside any symbol,
/// every deletion the fix verb makes) has no span and no caller list:
/// nothing may break anywhere. A lossy module is out, its byte columns
/// being no one else's.
pub fn placed(
    facts: &RepoFacts<'_>,
    calls: &CallGraph,
    oracle: &Oracle,
    splice: &Splice,
) -> Option<Proposal> {
    let symbol = facts.symbols.get(splice.owner.as_str());
    let module = match symbol {
        Some(sym) => facts.modules.get(&*sym.module)?,
        None => facts.modules.get(splice.owner.as_str())?,
    };
    if module.lossy {
        return None;
    }
    let (respelled, imports) = spell(&splice.spelling, module, facts, Some(oracle))?;
    let watched = symbol.map(|_| {
        let callers = callers_of(&splice.owner, facts, calls);
        callers
            .prod
            .iter()
            .chain(callers.test.iter())
            .filter_map(|call| facts.rel_of(&call.module))
            .map(|rel| rel.to_string())
            .collect()
    });
    Some(Proposal {
        id: splice.id.clone(),
        owner: splice.owner.clone(),
        rel: module.rel.clone(),
        edits: splice
            .edits
            .iter()
            .map(|e| SpanEdit {
                text: respell(&e.text, &respelled),
                ..e.clone()
            })
            .collect(),
        span: symbol.map_or((0, 0), |s| line_span((s.lineno, s.end_lineno))),
        watched,
        imports: imports
            .into_iter()
            .chain(splice.imports.iter().cloned())
            .collect(),
        param: splice.param.clone(),
    })
}

/// What the world proved: a receipt when a check in the callee body went
/// redundant under the splice, else that no watched file errored.
pub fn evidence_of(outcome: &Outcome) -> Evidence {
    match outcome {
        Outcome::Receipt(diag) => Evidence::Counterfactual {
            receipt: diag.clone(),
        },
        _ => Evidence::Wp {
            premises: vec!["counterfactual:clean".to_string()],
        },
    }
}

/// The two builders the children's tests share.
#[cfg(test)]
mod fixtures {
    use super::*;

    pub(super) fn edit(line: u32, col_start: u32, col_end: u32, text: &str) -> SpanEdit {
        SpanEdit {
            line,
            col_start,
            col_end,
            text: text.to_string(),
        }
    }

    pub(super) fn proposal(
        id: &str,
        rel: &str,
        span: (u32, u32),
        edits: Vec<SpanEdit>,
    ) -> Proposal {
        Proposal {
            id: id.to_string(),
            owner: id.to_string(),
            rel: Rel::from(rel),
            edits,
            span,
            watched: Some(HashSet::new()),
            imports: Vec::new(),
            param: String::new(),
        }
    }
}
