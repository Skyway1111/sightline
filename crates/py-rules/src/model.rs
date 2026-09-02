//! The fn half of a rule: what one is here, and the two catalog types
//! #12 and #41 match through. The record half is
//! `core::rule::RuleRecord`, which every language-blind reader holds.

use ruff_python_ast::{Stmt, StmtFunctionDef};

use sightline_core::findings::Sink;
use sightline_core::rule::RuleRecord;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{NodeIndex, RepoFacts, Symbol};
use sightline_py_facts::module::Module;
use sightline_py_provers::Provers;

/// A rule fn: facts and provers in, findings out in yield order.
pub type RuleFn = fn(&RepoFacts<'_>, &Provers, &mut Sink);

/// One rule: the record that describes it beside the fn that reads it.
/// `pub const RULE_N: Rule` sits next to `fn rule_n`, and `RULES` lists it
/// (`deny(dead_code)` fails the build for one the list forgot).
pub struct Rule {
    pub record: RuleRecord,
    pub run: RuleFn,
}

/// What a catalog matcher may ask about the function it is matching.
pub struct MatchCtx<'a, 't> {
    pub facts: &'a RepoFacts<'t>,
    pub module: &'a Module<'t>,
    pub sym: &'a Symbol,
    /// hot-path loop amplification (#41 perf-catalog; 0 elsewhere)
    pub amp: u32,
}

impl<'t> MatchCtx<'_, 't> {
    /// The function's def node. `fn` is a keyword here, so the accessor is
    /// `func`.
    pub fn func(&self) -> &'t StmtFunctionDef {
        match self.module.nodes[self.sym.node as usize] {
            Cn::Stmt(Stmt::FunctionDef(f)) => f,
            _ => panic!("a function symbol's node is a def"),
        }
    }

    /// The function's own nodes of these kinds, nested defs included.
    pub fn nodes(&self, kinds: &[Kind]) -> Vec<NodeIndex> {
        self.module.nodes(kinds, Some(&self.sym.qname), true)
    }
}

/// One catalog entry: a matcher yielding every node it fires on, and what
/// to do instead. #12's idioms and #41's perf shapes are one table type,
/// and each entry earns its place by a committed proof
/// (`xtask catalog`, `xtask perf-catalog`).
pub struct Shape {
    pub matcher: fn(&StmtFunctionDef, &MatchCtx<'_, '_>) -> Vec<NodeIndex>,
    pub suggestion: &'static str,
    /// node kind the index must hold before matching
    pub trigger: Option<Kind>,
}
