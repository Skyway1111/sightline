//! The oracle-revealed return types of return-unannotated internal functions
//! (#36/#40), one `Oracle::module_member_type` per candidate module-level
//! code can name and never rebinds.

use indexmap::IndexMap;
use serde_json::{Value, json};

use sightline_core::findings::{Qname, Rel};
use sightline_core::pytext;
use sightline_py_facts::model::{FUNCTION_KINDS, RefKind, RepoFacts};

use crate::oracle::Oracle;
use crate::typestrings::split_top;

/// The return part of an oracle callable string: the text after the last
/// top-level ` -> `, minus the group parens pyright puts around union
/// returns (`-> (Any | None)`). `None` when the string is not arrow-shaped
/// (property objects, overloads, decorator wrappers).
pub fn return_of(callable_type: &str) -> Option<String> {
    let parts = split_top(callable_type, " -> ");
    if parts.len() < 2 || !callable_type.starts_with('(') {
        return None;
    }
    let mut ret = pytext::strip(parts[parts.len() - 1]);
    while ret.starts_with('(') && closes_at_end(ret) {
        let mut chars = ret.chars();
        chars.next();
        chars.next_back();
        ret = pytext::strip(chars.as_str());
    }
    (!ret.is_empty()).then(|| ret.to_string())
}

/// Does the paren opening `s` close at its last character?
fn closes_at_end(s: &str) -> bool {
    let last = s.chars().count().saturating_sub(1);
    let mut depth: i32 = 0;
    for (i, ch) in s.chars().enumerate() {
        depth += i32::from("[(".contains(ch)) - i32::from("])".contains(ch));
        if depth == 0 {
            return i == last;
        }
    }
    false
}

/// How module-level code names the symbol (`f`, `Cls.m`), `None` where it
/// cannot.
pub fn module_scope_name(
    facts: &RepoFacts<'_>,
    name: &str,
    parent: Option<&Qname>,
) -> Option<String> {
    let Some(parent_q) = parent else {
        return Some(name.to_string());
    };
    let parent = facts.symbols.get(parent_q)?;
    (parent.kind == "class" && parent.parent.is_none()).then(|| format!("{}.{name}", parent.name))
}

/// Without an oracle: no candidates, no queries, every return unknown.
#[derive(Debug, Default)]
pub struct RetTypes {
    types: IndexMap<Qname, Option<String>>,
}

impl RetTypes {
    pub fn new(facts: &RepoFacts<'_>, oracle: Option<&Oracle>) -> RetTypes {
        let Some(oracle) = oracle else {
            return RetTypes::default();
        };
        // one source override per file, its candidates appended together
        let cands = candidates(facts);
        let mut by_rel: IndexMap<Rel, Vec<usize>> = IndexMap::new();
        for (at, (_, rel, _)) in cands.iter().enumerate() {
            by_rel.entry(rel.clone()).or_default().push(at);
        }
        let mut answers: Vec<Option<String>> = vec![None; cands.len()];
        for (rel, slots) in &by_rel {
            let dotted: Vec<String> = slots.iter().map(|at| cands[*at].2.clone()).collect();
            for (at, answer) in slots.iter().zip(oracle.module_member_types(rel, &dotted)) {
                answers[*at] = answer.and_then(|t| return_of(&t));
            }
        }
        RetTypes {
            types: cands
                .into_iter()
                .map(|(qname, _, _)| qname)
                .zip(answers)
                .collect(),
        }
    }

    /// The queried qnames, in query order (sorted symbols).
    pub fn candidates(&self) -> Vec<&Qname> {
        self.types.keys().collect()
    }

    /// The oracle-inferred return type, `None` when unqueryable or unanswered.
    pub fn return_type(&self, qname: &str) -> Option<&str> {
        self.types.get(qname).and_then(|t| t.as_deref())
    }

    /// `_oracle_answers`' `ret_types`: `{qname: type | null}` per candidate.
    pub fn dump_map(&self) -> Value {
        json!(
            self.types
                .iter()
                .map(|(q, t)| (q.to_string(), t.clone()))
                .collect::<IndexMap<String, Option<String>>>()
        )
    }
}

/// `_queries`: `(qname, rel, module-scope spelling)` for every
/// return-unannotated function module-level code can name and never rebinds,
/// in qname order.
fn candidates(facts: &RepoFacts<'_>) -> Vec<(Qname, Rel, String)> {
    let mut qnames: Vec<&Qname> = facts.symbols.keys().collect();
    qnames.sort();
    let mut out = Vec::new();
    for qname in qnames {
        let sym = &facts.symbols[qname];
        let Some(module) = facts.modules.get(&sym.module) else {
            continue;
        };
        if !FUNCTION_KINDS.contains(&sym.kind) || module.returns(sym.node).is_some() {
            continue;
        }
        let Some(name) = module_scope_name(facts, &sym.name, sym.parent.as_ref()) else {
            continue;
        };
        let rebound = facts.refs_to.get(qname).is_some_and(|refs| {
            refs.iter()
                .any(|r| facts.refs[*r as usize].kind == RefKind::Store)
        });
        if rebound {
            continue;
        }
        out.push((qname.clone(), module.rel.clone(), name));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_of_reads_the_arrow_shape() {
        let table = [
            ("(x: int) -> int", Some("int")),
            // pyright parenthesizes union returns: the parens are display,
            // not a member
            ("() -> (Any | None)", Some("Any | None")),
            (
                "(ctx: SlotCtx) -> (dict[str, Any] | None)",
                Some("dict[str, Any] | None"),
            ),
            // a nested callable return keeps its own parens; a grouped one
            // loses only the group
            ("(x) -> (int) -> str", Some("str")),
            ("(x) -> ((int) -> str)", Some("(int) -> str")),
            (
                "(x) -> ((int) -> (str | None))",
                Some("(int) -> (str | None)"),
            ),
            // not arrow-shaped: property objects, overloads
            ("property", None),
            ("Overload[(x: int) -> int, (x: str) -> str]", None),
        ];
        for (callable_type, expected) in table {
            assert_eq!(
                return_of(callable_type).as_deref(),
                expected,
                "{callable_type}"
            );
        }
    }

    /// `_queries`: return-unannotated functions module-level code can name
    /// and never rebinds, in qname order. The enumeration reads no checker,
    /// as Python's does not.
    #[test]
    fn candidates_are_the_names_module_level_code_can_reveal() {
        let (_dir, built) = crate::argtypes::mini_repo(&[(
            "m.py",
            b"def f():\n    return 1\n\
              def g() -> int:\n    return 2\n\
              def h():\n    return 3\n\
              h = h\n\
              class C:\n    def m(self):\n        return 4\n\
              def outer():\n    def inner():\n        return 5\n    return inner\n",
        )]);
        let facts = built.borrow_dependent();

        let named: Vec<(String, String)> = candidates(facts)
            .into_iter()
            .map(|(q, _rel, name)| (q.to_string(), name))
            .collect();

        assert_eq!(
            named,
            [
                ("m.C.m".to_string(), "C.m".to_string()),
                ("m.f".to_string(), "f".to_string()),
                ("m.outer".to_string(), "outer".to_string()),
            ]
        );
    }
}
