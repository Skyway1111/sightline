//! Family B (surface count). #11 clones, #14 data clump, #18 section comments, #20 repeated lambda, #21 distributed
//! invariant, #37 speculative generality, #48 fold candidate, #54 kind
//! switch, #55 positional width and #23 cognitive complexity.
//!
//! #12 is `idioms.rs`; the clone mining is `py_provers::clones`.
//!
//! file-length-ok: one file per rule family is this crate's shape, and a
//! RuleRecord lives beside the function it describes. Splitting the surface rules by
//! size would put a record and its rule in two places.

use std::collections::{BTreeSet, HashMap, HashSet};

use indexmap::IndexMap;
use ruff_python_ast::visitor::transformer::{Transformer, walk_expr};
use ruff_python_ast::{CmpOp, Expr, ExprCall, ExprContext, Parameter, Pattern, Stmt};

use sightline_core::clones::digest;
use sightline_core::findings::{Evidence, Finding, Qname, Rel, Sink, Site, SpanEdit};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope, owner_list};
use sightline_core::text::is_phase_label;
use sightline_py_facts::astutil::{
    CHAIN, RECEIVERS, attr_on, chain_root, document_order, fn_args, fn_body, fn_defaults,
    fn_params, fn_pos_args, line_span, subnodes,
};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::complexity::{cognitive_complexity, nesting_at};
use sightline_py_facts::kinds::{Kind, is_stmt};
use sightline_py_facts::model::{
    CallSite, ClassInfo, FUNCTION_KINDS, NodeIndex, RefKind, RepoFacts, Step, Symbol, class_walk,
    is_test_path,
};
use sightline_py_facts::module::Module;
use sightline_py_facts::unparse;
use sightline_py_provers::Provers;
use sightline_py_provers::argtypes::{Arg, arg_expr, prod_arg_sites};
use sightline_py_provers::callgraph::{CallGraph, callers_of};
use sightline_py_provers::clones::{CloneGroup, Member, Shapes, foreign_roots, mine};
use sightline_py_provers::comments::docstring;
use sightline_py_provers::counterfactual::Splice;
use sightline_py_provers::dump::{Dumps, Rename, constant, normalize};
use sightline_py_provers::imports::shared_homes;

use crate::framework::{is_override_fixed, is_registered, is_stub, plugin_signatures};
use crate::model::Rule;
use crate::util::{
    decorator_names, deletion, enclosing_at_line, fn_of, iter_functions, iter_prod_functions,
    node_site,
};

// --- #11 structural clones ---------------------------------------------------

pub const RULE_11: Rule = Rule {
    record: RuleRecord {
        id: "11",
        slug: "structural-clones",
        family: "surface",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "AST-normalized T2 clone groups (whole function, statement block, \
                  repeated attribute-walk expression), ratcheted; first copies exempt",
        goal: "One home per fact: every extra copy is a place the next fix \
               forgets (GitClear 8x duplication; Van Eerd's migration grace).",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_11,
};

/// The variable annotations written inside a copy. Blind normalization reads
/// two bodies declaring different types as one shape, and the declared type
/// is the fact the copy exists for.
fn declared_types(module: &Module<'_>, nodes: &[NodeIndex]) -> Vec<String> {
    let mut out = Vec::new();
    for at in nodes {
        for reached in subnodes(module.nodes[*at as usize], |k| k == Kind::AnnAssign) {
            if let Cn::Stmt(Stmt::AnnAssign(a)) = reached {
                out.push(unparse::expr(&a.annotation));
            }
        }
    }
    out
}

/// Is the group one fact with one home? Every copy that would carry a finding
/// declares the same types, and they could all move into one home. Test
/// members count toward the group and answer neither question.
fn one_fact(facts: &RepoFacts<'_>, members: &[Member], homes: &IndexMap<Qname, String>) -> bool {
    let mut types: HashSet<Vec<String>> = HashSet::new();
    let mut where_they_live: HashSet<&str> = HashSet::new();
    for member in members {
        let module = &facts.modules[&member.module];
        if is_test_path(&module.rel) {
            continue;
        }
        types.insert(declared_types(module, &member.nodes));
        where_they_live.insert(homes.get(&module.qname).map_or("", String::as_str));
    }
    types.len() <= 1 && where_they_live.len() <= 1
}

/// The spans a reader is asked to move, per function group: the reported
/// copies alone, which is what holds the blame bill to the files #11 reports
/// in.
fn priced_spans(
    facts: &RepoFacts<'_>,
    groups: &[&CloneGroup],
) -> Vec<Vec<sightline_core::git::Span>> {
    groups
        .iter()
        .map(|group| {
            group
                .members
                .iter()
                .filter_map(|member| {
                    let module = &facts.modules[&member.module];
                    if is_test_path(&module.rel) {
                        return None;
                    }
                    let sym = facts.symbols.get(&*member.symbol)?;
                    let (line, end) =
                        line_span((module.line_of(sym.node), module.end_line_of(sym.node)));
                    Some((module.rel.to_string(), line, end))
                })
                .collect()
        })
        .collect()
}

fn clone_finding(
    facts: &RepoFacts<'_>,
    module: &Module<'_>,
    node: NodeIndex,
    key: &str,
    kind: &str,
    message: String,
    salience: f64,
) -> Finding {
    Finding {
        rule: "11",
        site: node_site(facts, module, node),
        message,
        cause: format!("{kind}:{key}"),
        evidence: Evidence::Idx {
            detail: key.to_string(),
        },
        salience,
        fix: None,
        lang: "py",
    }
}

/// Whole-function T2 clones, statement-block clones at sub-function
/// granularity and expression clones. Test members count toward a group but
/// never carry a finding, nor price its migration grace. A group is one fact
/// only where every copy declares the same types and shares one home, and a
/// window of one repeated statement shape is a table, not a fact.
/// The qnames a clone group's message lists, in one order on every platform:
/// a group's members reach a rule in discovery order, which the walk spells
/// differently on Windows and on Unix.
/// Each copy as `qname L<line>`, so a reader opens the other copies without
/// a search.
fn owners(facts: &RepoFacts<'_>, members: &[Member]) -> Vec<String> {
    let mut out: Vec<String> = members
        .iter()
        .map(|m| {
            format!(
                "{} L{}",
                m.symbol,
                facts.modules[&m.module].line_of(m.nodes[0])
            )
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn rule_11(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let functions = sightline_py_provers::clones::iter_functions(facts);
    let mut foreign: HashMap<Qname, HashSet<Box<str>>> = HashMap::new();
    for sym in &functions {
        if !foreign.contains_key(&sym.module) {
            let module = &facts.modules[&sym.module];
            foreign.insert(sym.module.clone(), foreign_roots(facts, module));
        }
    }
    let mined = mine(facts, &functions, &foreign);
    let homes = shared_homes(facts);
    let shapes = Shapes::default();

    let fn_groups: Vec<&CloneGroup> = mined
        .functions
        .iter()
        .filter(|g| one_fact(facts, &g.members, &homes))
        .collect();
    // migration grace: rank by count x age of the youngest copy a reader is
    // asked to move
    let git = provers.git_ages.as_ref().filter(|g| g.available());
    let ages: Vec<Option<i64>> = match git {
        Some(git) => git.youngest_ages_days(&priced_spans(facts, &fn_groups)),
        None => vec![None; fn_groups.len()],
    };
    for (group, age) in fn_groups.iter().zip(ages) {
        let count = group.members.len();
        let salience = match age {
            None => count as f64,
            Some(age) => (count as i64 * age) as f64,
        };
        let owners = owners(facts, &group.members);
        let listed = owner_list(&owners);
        for member in &group.members {
            let module = &facts.modules[&member.module];
            if !is_test_path(&module.rel) {
                out.push(clone_finding(
                    facts,
                    module,
                    member.nodes[0],
                    &group.key,
                    "clone",
                    format!("structural clone x{count}: {listed}"),
                    salience,
                ));
            }
        }
    }
    for group in &mined.blocks {
        // a window of one repeated statement shape is a table's form, not a
        // fact: every occurrence keys alike once the literals are erased
        let first = &group.members[0];
        let first_module = &facts.modules[&first.module];
        let one_shape: HashSet<String> = first
            .nodes
            .iter()
            .map(|at| shapes.dump(first_module.nodes[*at as usize], first_module))
            .collect();
        if !one_fact(facts, &group.members, &homes) || one_shape.len() == 1 {
            continue;
        }
        let count = group.members.len();
        let stmts = first.nodes.len();
        let owners = owners(facts, &group.members);
        let listed = owner_list(&owners);
        for member in &group.members {
            let module = &facts.modules[&member.module];
            if !is_test_path(&module.rel) {
                out.push(clone_finding(
                    facts,
                    module,
                    member.nodes[0],
                    &group.key,
                    "clone-block",
                    format!("structural block clone x{count} ({stmts} stmts): {listed}"),
                    (count * stmts) as f64,
                ));
            }
        }
    }
    for group in &mined.exprs {
        if !one_fact(facts, &group.members, &homes) {
            continue;
        }
        let count = group.members.len();
        let anchor = &group.members[0];
        let module = &facts.modules[&anchor.module];
        let owners = owners(facts, &group.members);
        out.push(clone_finding(
            facts,
            module,
            anchor.nodes[0],
            &group.key,
            "expr-clone",
            format!("expression clone x{count}: {}", owner_list(&owners)),
            count as f64,
        ));
    }
}

// --- #14 data clump ----------------------------------------------------------

/// signatures per group signature: above it, the param is ambient
const AMBIENT: usize = 8;

pub const RULE_14: Rule = Rule {
    record: RuleRecord {
        id: "14",
        slug: "data-clump",
        family: "surface",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "same >=3-param group across >=3 typed signatures",
        goal: "A parameter group that travels together is a concept without a \
               name (Fowler; Smith's fourth).",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_14,
};

/// function qname -> the module and node its site sits at
type Owners = IndexMap<Qname, (Qname, NodeIndex)>;

/// One group: the functions sharing keys, the union of the keys, and the
/// document-first site.
struct SharedGroup {
    qnames: Vec<Qname>,
    names: BTreeSet<String>,
    module: Qname,
    node: NodeIndex,
}

/// Per set of >= `least` functions sharing keys (#14's param triples, #54's
/// literal pairs): the qnames, the union of the keys they share and the
/// document-first site. The widest group owns each anchor, else sub-groups
/// re-count one site per size.
fn shared_groups(
    facts: &RepoFacts<'_>,
    hits: &IndexMap<BTreeSet<String>, Owners>,
    least: usize,
) -> Vec<SharedGroup> {
    /// the first key's owners, and the union of every key that set shares
    type Shared<'a> = (&'a Owners, BTreeSet<String>);

    let mut by_group: IndexMap<Vec<Qname>, Shared<'_>> = IndexMap::new();
    for (key, owners) in hits {
        let mut qnames: Vec<Qname> = owners.keys().cloned().collect();
        qnames.sort();
        if qnames.len() < least {
            continue;
        }
        let held = by_group.entry(qnames).or_insert((owners, BTreeSet::new()));
        held.1.extend(key.iter().cloned());
    }
    let mut ordered: Vec<(&Vec<Qname>, &Shared<'_>)> = by_group.iter().collect();
    ordered.sort_by(|a, b| a.0.cmp(b.0));

    type Rank = (usize, usize, String);
    let mut best: IndexMap<(Rel, u32), (Rank, SharedGroup)> = IndexMap::new();
    for (qnames, (owners, names)) in ordered {
        let Some((module_q, node)) = owners.values().min_by_key(|(module_q, node)| {
            let module = &facts.modules[module_q];
            (module.rel.clone(), module.line_of(*node))
        }) else {
            continue;
        };
        let module = &facts.modules[module_q];
        let anchor = (module.rel.clone(), module.line_of(*node));
        let rank: Rank = (
            qnames.len(),
            names.len(),
            names.iter().cloned().collect::<Vec<String>>().join(","),
        );
        let group = SharedGroup {
            qnames: qnames.clone(),
            names: names.clone(),
            module: module_q.clone(),
            node: *node,
        };
        match best.get(&anchor) {
            Some((held, _)) if rank <= *held => {}
            _ => {
                best.insert(anchor, (rank, group));
            }
        }
    }
    let mut anchors: Vec<(Rel, u32)> = best.keys().cloned().collect();
    anchors.sort();
    anchors
        .into_iter()
        .filter_map(|anchor| best.shift_remove(&anchor).map(|(_, group)| group))
        .collect()
}

/// The class an annotation names, a subscript's base included.
fn annotated_class(module: &Module<'_>, annotation: Option<&Expr>) -> Option<String> {
    let base = match annotation? {
        Expr::Subscript(s) => &*s.value,
        other => other,
    };
    module.dotted_name(base)
}

/// Every prod call site passes the param straight through under its own name:
/// the object rides the chain unchanged (#14's threaded context).
fn passed_through(facts: &RepoFacts<'_>, calls: &CallGraph, sym: &Symbol, param: &str) -> bool {
    let sites = prod_arg_sites(facts, calls, sym, param);
    !sites.is_empty()
        && sites.iter().all(|(module, arg)| match arg {
            Arg::Expr(at) => {
                matches!(module.nodes[*at as usize], Cn::Expr(Expr::Name(n)) if n.id == param)
            }
            _ => false,
        })
}

/// A slot a framework binds, not a caller: `Header(...)` as its default.
fn injected(facts: &RepoFacts<'_>, module: &Module<'_>, default: Option<&Expr>) -> bool {
    match default {
        Some(Expr::Call(c)) => module
            .dotted_name(&c.func)
            .is_none_or(|name| !facts.symbols.contains_key(&*name)),
        _ => false,
    }
}

/// Every 3-combination of a sorted list, in Python's lexicographic order.
fn triples_of(names: &[String]) -> Vec<BTreeSet<String>> {
    let mut out = Vec::new();
    for i in 0..names.len() {
        for j in i + 1..names.len() {
            for k in j + 1..names.len() {
                out.push(BTreeSet::from([
                    names[i].clone(),
                    names[j].clone(),
                    names[k].clone(),
                ]));
            }
        }
    }
    out
}

/// Combination mining capped at 15 params; triples shared by one function set
/// merge into one finding. Threaded context is already a named concept and no
/// clump member. The finding asks for a type, so every signature in the group
/// must have declared one for every member. A parameter riding `AMBIENT` times
/// more signatures than the group has travels with nothing.
fn rule_14(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let calls = provers.calls(facts);
    let mut triples: IndexMap<BTreeSet<String>, Owners> = IndexMap::new();
    let mut declared: HashMap<Qname, BTreeSet<String>> = HashMap::new();
    let mut holders: HashMap<String, usize> = HashMap::new();
    for (module, sym) in iter_prod_functions(facts) {
        if is_override_fixed(facts, sym) || is_registered(facts, sym, None) {
            continue;
        }
        let fn_def = fn_of(module, sym);
        let annotations: IndexMap<&str, Option<&Expr>> = fn_args(fn_def)
            .iter()
            .map(|a| {
                let declared = Cn::Param(a).stamped().and_then(|at| module.annotation(at));
                (a.name.as_str(), declared)
            })
            .collect();
        let defaults: HashMap<&str, &Expr> = fn_defaults(fn_def)
            .into_iter()
            .map(|(a, d)| (a.name.as_str(), d))
            .collect();
        let names: Vec<String> = fn_params(fn_def)
            .into_iter()
            .filter(|p| {
                let annotation = annotations.get(p).copied().flatten();
                !RECEIVERS.contains(p)
                    && !(annotated_class(module, annotation)
                        .is_some_and(|q| facts.classes.contains_key(&*q))
                        && passed_through(facts, calls, sym, p))
                    && !injected(facts, module, defaults.get(p).copied())
            })
            .map(str::to_string)
            .collect();
        declared.insert(
            sym.qname.clone(),
            names
                .iter()
                .filter(|p| annotations.get(&***p).copied().flatten().is_some())
                .cloned()
                .collect(),
        );
        if !(3..=15).contains(&names.len()) {
            continue;
        }
        for p in &names {
            *holders.entry(p.clone()).or_insert(0) += 1;
        }
        let mut sorted = names.clone();
        sorted.sort();
        for combo in triples_of(&sorted) {
            triples
                .entry(combo)
                .or_default()
                .insert(sym.qname.clone(), (sym.module.clone(), sym.node));
        }
    }
    for group in shared_groups(facts, &triples, 3) {
        // the group is one signature's untyped params
        if group.qnames.iter().any(|q| {
            declared
                .get(q)
                .is_none_or(|declared| !group.names.is_subset(declared))
        }) {
            continue;
        }
        // an ambient parameter drags arbitrary neighbours in
        if group
            .names
            .iter()
            .any(|p| holders.get(p).copied().unwrap_or(0) > AMBIENT * group.qnames.len())
        {
            continue;
        }
        let listed: Vec<&str> = group.names.iter().map(String::as_str).collect();
        let module = &facts.modules[&group.module];
        out.push(Finding {
            rule: "14",
            site: node_site(facts, module, group.node),
            message: format!(
                "params ({}) recur together in {} signatures - wants a type",
                listed.join(", "),
                group.qnames.len()
            ),
            cause: format!("clump:{}", listed.join(",")),
            evidence: Evidence::idx(),
            salience: group.qnames.len() as f64,
            fix: None,
            lang: "py",
        });
    }
}

// --- #18 section comments ----------------------------------------------------

pub const RULE_18: Rule = Rule {
    record: RuleRecord {
        id: "18",
        slug: "section-comments",
        family: "surface",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: ">=2 labeled phases narrated inside one function",
        goal: "A numbered phase comment is a function boundary spelled in prose \
               (Smith; Van Eerd V5).",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_18,
};

/// Does the value of an `Expr`, `Return`, `Assign` or `AnnAssign` read as
/// a call, `await` stripped?
fn is_call_value(stmt: &Stmt) -> bool {
    let mut value = match stmt {
        Stmt::Expr(e) => Some(&*e.value),
        Stmt::Return(r) => r.value.as_deref(),
        Stmt::Assign(a) => Some(&*a.value),
        Stmt::AnnAssign(a) => a.value.as_deref(),
        _ => None,
    };
    while let Some(Expr::Await(a)) = value {
        value = Some(&a.value);
    }
    matches!(value, Some(Expr::Call(_)))
}

/// The labels that head a section of code. A label owns the statements
/// between it and the next label: none of its own is no phase, and neither is
/// the single call that already bears the phase's name.
fn phase_lines(module: &Module<'_>, node: NodeIndex, lines: &[u32]) -> Vec<u32> {
    let mut stmts: Vec<(u32, Cn<'_>)> = subnodes(module.nodes[node as usize], is_stmt)
        .into_iter()
        .map(|n| (n.stamped().map_or(0, |at| module.line_of(at)), n))
        .collect();
    stmts.sort_by_key(|(line, _)| *line);
    let mut out = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let next = lines.get(at + 1).copied().unwrap_or(u32::MAX);
        let head = stmts.iter().find(|(head, _)| head > line);
        match head {
            None => continue,
            Some((head_line, _)) if *head_line > next => continue,
            Some((_, Cn::Stmt(st))) if is_call_value(st) => continue,
            Some(_) => out.push(*line),
        }
    }
    out
}

/// A phase is a section of code: `phase_lines` keeps the labels that head
/// statements of their own, the anchor stays the first label.
fn rule_18(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for module in facts.modules.values() {
        let mut per_symbol: IndexMap<Qname, Vec<u32>> = IndexMap::new();
        for comment in &module.comments {
            if !is_phase_label(&comment.text, "#") {
                continue;
            }
            let owner = enclosing_at_line(facts, module, comment.line);
            if facts
                .symbols
                .get(owner.as_str())
                .is_some_and(|sym| FUNCTION_KINDS.contains(&sym.kind))
            {
                per_symbol
                    .entry(Qname::from(owner))
                    .or_default()
                    .push(comment.line);
            }
        }
        let mut owners: Vec<&Qname> = per_symbol.keys().collect();
        owners.sort();
        for owner in owners {
            let lines = &per_symbol[owner];
            let mut sorted = lines.clone();
            sorted.sort_unstable();
            let sym = &facts.symbols[&**owner];
            let phases = phase_lines(module, sym.node, &sorted);
            if phases.len() < 2 {
                continue;
            }
            out.push(Finding {
                rule: "18",
                site: Site {
                    rel: module.rel.clone(),
                    line: lines[0],
                    col: 0,
                    symbol: owner.clone(),
                },
                message: format!(
                    "{owner} narrates {} labeled phases - each is a function \
                     boundary spelled in prose",
                    phases.len()
                ),
                cause: format!("sections:{owner}"),
                evidence: Evidence::ast(),
                salience: phases.len() as f64,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #20 repeated nontrivial lambda ------------------------------------------

/// the family's bar for "a pattern": #21, #54 and #11 count 3
const LAMBDA_COPIES: usize = 3;

pub const RULE_20: Rule = Rule {
    record: RuleRecord {
        id: "20",
        slug: "repeated-lambda",
        family: "surface",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "same nontrivial lambda body >=3 times in a module",
        goal: "Interface symmetry (Sean Parent): a predicate written three \
               times drifts; name it once.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_20,
};

/// A second copy is a coincidence a reader still holds in one glance; the
/// third is the pattern.
fn rule_20(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    let shapes = Shapes::default();
    for module in facts.modules.values() {
        let mut by_key: IndexMap<String, Vec<NodeIndex>> = IndexMap::new();
        for at in module.nodes(&[Kind::Lambda], None, false) {
            let Cn::Expr(Expr::Lambda(lambda)) = module.nodes[at as usize] else {
                continue;
            };
            if shapes.size(Cn::Expr(&lambda.body), module) < 5 {
                continue;
            }
            let mut params: HashMap<&str, String> = HashMap::new();
            if let Some(declared) = lambda.parameters.as_deref() {
                let slots = declared
                    .posonlyargs
                    .iter()
                    .chain(declared.args.iter())
                    .chain(declared.kwonlyargs.iter());
                for (i, a) in slots.enumerate() {
                    params.insert(a.parameter.name.as_str(), format!("p{i}"));
                }
            }
            // the body as a standalone expression, params renamed by position
            let rename = |name: &str| params.get(name).cloned();
            let mut memo = Dumps::new();
            let body = normalize(
                Cn::Expr(&lambda.body),
                module,
                &Rename::By(&rename),
                &mut memo,
                Some(false),
            );
            by_key
                .entry(format!("Expression(body={body})"))
                .or_default()
                .push(at);
        }
        let mut keys: Vec<&String> = by_key.keys().collect();
        keys.sort();
        for key in keys {
            let lams = &by_key[key];
            if lams.len() < LAMBDA_COPIES {
                continue;
            }
            let first = *lams
                .iter()
                .min_by_key(|at| {
                    let span = module.span(**at).unwrap_or_default();
                    (span[0].unwrap_or(0), span[1].unwrap_or(0))
                })
                .expect("a group holds three lambdas");
            let Cn::Expr(node) = module.nodes[first as usize] else {
                continue;
            };
            out.push(Finding {
                rule: "20",
                site: node_site(facts, module, first),
                message: format!(
                    "lambda `{}` appears {}x in {} - name it once",
                    unparse::expr(node),
                    lams.len(),
                    module.qname
                ),
                cause: format!("lambda:{}:{}", module.qname, &digest(key)[..8]),
                evidence: Evidence::ast(),
                salience: lams.len() as f64,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #21 distributed ad-hoc invariant ----------------------------------------

const INVARIANT_OPS: [Kind; 4] = [Kind::Subscript, Kind::Call, Kind::BinOp, Kind::Compare];
const INVARIANT_KINDS: [Kind; 5] = [
    Kind::Subscript,
    Kind::Call,
    Kind::BinOp,
    Kind::Compare,
    Kind::BoolOp,
];
const MIN_INVARIANT_NODES: usize = 5;

pub const RULE_21: Rule = Rule {
    record: RuleRecord {
        id: "21",
        slug: "distributed-invariant",
        family: "surface",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "same self-rooted decision in >=3 methods of one class",
        goal: "Encapsulate the invariant (Smith's CaseInsensitiveMap): a rule \
               enforced at every call site belongs in the type.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_21,
};

/// self/cls pinned, every other identifier blinded: the chain is the fact.
fn invariant_rename(name: &str) -> Option<String> {
    Some(if RECEIVERS.contains(&name) {
        "self".to_string()
    } else {
        "_n_".to_string()
    })
}

/// Maximal candidate expressions involving self.<attr> that decide something,
/// normalized: a shape inside a reported shape is that shape's.
fn self_exprs(
    facts: &RepoFacts<'_>,
    provers: &Provers,
    module: &Module<'_>,
    sym: &Symbol,
    shapes: &Shapes,
    memo: &mut Dumps,
) -> Vec<(NodeIndex, String)> {
    let Some(scope) = provers.scope_of(facts, &sym.qname) else {
        return Vec::new();
    };
    let receivers: Vec<NodeIndex> = module
        .nodes(&[Kind::Attribute], Some(&sym.qname), true)
        .into_iter()
        .filter(|at| match module.nodes[*at as usize] {
            Cn::Expr(e) => attr_on(e, &RECEIVERS).is_some(),
            _ => false,
        })
        .collect();
    // a candidate wraps a self attribute and is (or wraps) an operator
    let wrapping = scope.ancestor_ids(facts, receivers, false);
    let operators = scope.ancestor_ids(
        facts,
        module.nodes(&INVARIANT_OPS, Some(&sym.qname), true),
        true,
    );
    let candidates: HashSet<NodeIndex> = wrapping.intersection(&operators).copied().collect();

    let mut ordered = module.nodes(&INVARIANT_KINDS, Some(&sym.qname), true);
    document_order(&mut ordered, |at| {
        let span = module.span(*at).unwrap_or_default();
        (
            span[0].unwrap_or(0),
            span[1].unwrap_or(0),
            span[2].unwrap_or(0),
            span[3].unwrap_or(0),
        )
    });
    let mut taken: HashSet<NodeIndex> = HashSet::new();
    let mut out = Vec::new();
    for at in ordered {
        let node = module.nodes[at as usize];
        if !candidates.contains(&at)
            || shapes.size(node, module) < MIN_INVARIANT_NODES
            || scope
                .ancestor_ids(facts, [at], false)
                .iter()
                .any(|up| taken.contains(up))
        {
            continue;
        }
        taken.insert(at);
        if matches!(node.kind(), Kind::Compare | Kind::BoolOp) {
            let key = normalize(node, module, &Rename::By(&invariant_rename), memo, None);
            out.push((at, key));
        }
    }
    out
}

/// A comparison or boolean over self's state recurring across a class's
/// methods is a rule the type should carry. A repeated read of the class's own
/// field and a repeated call to its own helper are not.
fn rule_21(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let shapes = Shapes::default();
    let mut memo = Dumps::new();
    for (cls_q, info) in &facts.classes {
        // repeated test asserts are not invariants
        if facts
            .rel_of(&info.module)
            .is_some_and(|rel| is_test_path(rel))
        {
            continue;
        }
        let module = &facts.modules[&info.module];
        let mut by_expr: IndexMap<String, IndexMap<Qname, NodeIndex>> = IndexMap::new();
        for m_q in info.methods.values() {
            let Some(m_sym) = facts.symbols.get(&**m_q) else {
                continue;
            };
            for (node, key) in self_exprs(facts, provers, module, m_sym, &shapes, &mut memo) {
                by_expr.entry(key).or_default().insert(m_q.clone(), node);
            }
        }
        let mut keys: Vec<&String> = by_expr.keys().collect();
        keys.sort();
        for key in keys {
            let methods = &by_expr[key];
            if methods.len() < 3 {
                continue;
            }
            let sample = methods
                .iter()
                .min_by_key(|(m_q, _)| *m_q)
                .map(|(_, node)| *node)
                .expect("a group holds three methods");
            let Cn::Expr(node) = module.nodes[sample as usize] else {
                continue;
            };
            out.push(Finding {
                rule: "21",
                site: node_site(facts, module, info.node),
                message: format!(
                    "`{}` recurs in {} methods of {cls_q} - encapsulate the invariant",
                    unparse::expr(node),
                    methods.len()
                ),
                cause: format!("invariant:{cls_q}:{}", &digest(key)[..8]),
                evidence: Evidence::ast(),
                salience: methods.len() as f64,
                fix: None,
                lang: "py",
            });
        }
    }
}

// --- #37 speculative generality ----------------------------------------------
// Inverse of #14: flexibility no prod caller exercises. Prod sites judge - a
// test caller is never evidence, but it is a veto.

pub const RULE_37: Rule = Rule {
    record: RuleRecord {
        id: "37",
        slug: "speculative-generality",
        family: "surface",
        engine_class: "WP+IDX",
        posture: Posture::Ratchet,
        meaning: "monomorphic params, unused defaults, single-impl abstractions",
        goal: "Flexibility no one exercises is debt (inverse of #14): a knob \
               every prod caller sets the same way, a default no one \
               overrides, an interface with one implementation.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_37,
};

/// The `ExprCall` a call site sits on.
fn call_of<'t>(module: &Module<'t>, node: NodeIndex) -> Option<&'t ExprCall> {
    module.call_at(node)
}

/// ("mono", literal) when every prod site passes the same literal; ("default",
/// "") when every prod site omits a defaulted param; `None` otherwise.
fn param_verdict(
    facts: &RepoFacts<'_>,
    seen: &[(Qname, Arg)],
    defaulted: bool,
) -> Option<(&'static str, String)> {
    if seen.is_empty() || seen.iter().any(|(_, arg)| *arg == Arg::Unknown) {
        return None;
    }
    if seen.iter().all(|(_, arg)| *arg == Arg::Omitted) {
        return defaulted.then(|| ("default", String::new()));
    }
    let keys: HashSet<Option<String>> = seen
        .iter()
        .map(|(module_q, arg)| match arg {
            Arg::Expr(at) => {
                let module = &facts.modules[module_q];
                match module.nodes[*at as usize] {
                    Cn::Expr(e) if Cn::Expr(e).kind() == Kind::Constant => {
                        Some(constant(e, module))
                    }
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    match keys.into_iter().collect::<Vec<Option<String>>>().as_slice() {
        [Some(key)] => Some(("mono", key.clone())),
        _ => None,
    }
}

/// What one call site binds to the slot, with the module the argument lives in.
fn bound_at(facts: &RepoFacts<'_>, site: &CallSite, idx: usize, param: &str) -> (Qname, Arg) {
    let arg = facts
        .modules
        .get(&site.module)
        .and_then(|module| call_of(module, site.node))
        .map_or(Arg::Unknown, |call| arg_expr(call, idx, param));
    (site.module.clone(), arg)
}

/// (kind, param, literal) per speculative param of one function. A test that
/// passes the param vetoes the unused default.
fn judge_params<'p>(
    facts: &RepoFacts<'_>,
    fn_def: &'p ruff_python_ast::StmtFunctionDef,
    prod: &[&CallSite],
    tests: &[&CallSite],
) -> Vec<(&'static str, &'p Parameter, String)> {
    let positional = fn_pos_args(fn_def);
    let defaulted: HashSet<&str> = fn_defaults(fn_def)
        .into_iter()
        .map(|(a, _)| a.name.as_str())
        .collect();
    let slots: Vec<(usize, &Parameter)> = positional
        .iter()
        .enumerate()
        .map(|(i, a)| (i, *a))
        .chain(
            fn_def
                .parameters
                .kwonlyargs
                .iter()
                .map(|a| (usize::MAX, &a.parameter)),
        )
        .collect();
    let mut out = Vec::new();
    for (idx, a) in slots {
        let name = a.name.as_str();
        let seen: Vec<(Qname, Arg)> = prod.iter().map(|c| bound_at(facts, c, idx, name)).collect();
        let Some((kind, lit)) = param_verdict(facts, &seen, defaulted.contains(name)) else {
            continue;
        };
        if kind == "default"
            && tests
                .iter()
                .any(|c| bound_at(facts, c, idx, name).1 != Arg::Omitted)
        {
            continue;
        }
        out.push((kind, a, lit));
    }
    out
}

/// The names this call is given under their own names.
fn verbatim(call: &ExprCall) -> HashSet<&str> {
    let positional = call.arguments.args.iter().filter_map(|a| match a {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    });
    let keywords = call.arguments.keywords.iter().filter_map(|kw| {
        let name = kw.arg.as_ref()?;
        match &kw.value {
            Expr::Name(n) if n.id.as_str() == name.as_str() => Some(name.as_str()),
            _ => None,
        }
    });
    positional.chain(keywords).collect()
}

/// A drop-in shim: the whole parameter list passed verbatim to one
/// third-party call whose value the def returns. The slots are the wrapped
/// API's, and pruning one breaks the substitution the shim exists for.
fn mirrors_foreign(facts: &RepoFacts<'_>, module: &Module<'_>, node: NodeIndex) -> bool {
    let Cn::Stmt(Stmt::FunctionDef(fn_def)) = module.nodes[node as usize] else {
        return false;
    };
    let params: HashSet<&str> = fn_params(fn_def)
        .into_iter()
        .filter(|p| !RECEIVERS.contains(p))
        .collect();
    let foreign = foreign_roots(facts, module);
    subnodes(module.nodes[node as usize], |k| k == Kind::Return)
        .into_iter()
        .filter_map(|reached| match reached {
            Cn::Stmt(Stmt::Return(r)) => match r.value.as_deref() {
                Some(Expr::Call(c)) => Some(c),
                _ => None,
            },
            _ => None,
        })
        .any(|call| {
            chain_root(&call.func, &CHAIN).is_some_and(|root| foreign.contains(root))
                && params.is_subset(&verbatim(call))
        })
}

/// Monomorphic params, never-overridden defaults (closed world, >=3 prod
/// sites), single-implementation Protocols and ABCs. A drop-in shim has no
/// knobs of its own.
fn rule_37(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let cw = provers.closed_world(facts);
    let calls = provers.calls(facts);
    for (module, sym) in iter_prod_functions(facts) {
        if is_override_fixed(facts, sym) || !cw.verdict(&sym.qname).passed {
            continue;
        }
        let callers = callers_of(&sym.qname, facts, calls);
        let n = callers.prod.len();
        let fn_def = fn_of(module, sym);
        if n < 3 || mirrors_foreign(facts, module, sym.node) {
            continue;
        }
        for (kind, a, lit) in judge_params(facts, fn_def, &callers.prod, &callers.test) {
            let tail = if kind == "mono" {
                format!(
                    "receives the same literal {lit} at all {n} prod call sites \
                     - inline the value or default it"
                )
            } else {
                format!(
                    "is never overridden across {n} prod call sites - the \
                     flexibility is speculative"
                )
            };
            let cause = if kind == "mono" {
                "monomorphic"
            } else {
                "unused-default"
            };
            let at = Cn::Param(a).stamped().unwrap_or(sym.node);
            out.push(Finding {
                rule: "37",
                site: node_site(facts, module, at),
                message: format!("param '{}' of {} {tail}", a.name, sym.qname),
                cause: format!("{cause}:{}:{}", sym.qname, a.name),
                evidence: Evidence::Wp {
                    premises: vec!["closed-world:pass".to_string(), format!("prod-sites:{n}")],
                },
                salience: n as f64,
                fix: None,
                lang: "py",
            });
        }
    }
    single_impl_abstractions(facts, out);
}

/// The abstraction each class declares itself to be, computed once: Python
/// asks it of every class inside the loop over every class.
fn abstraction_kinds(facts: &RepoFacts<'_>) -> IndexMap<Qname, Option<&'static str>> {
    facts
        .classes
        .iter()
        .map(|(q, info)| (q.clone(), abstraction_kind(facts, info)))
        .collect()
}

fn abstraction_kind(facts: &RepoFacts<'_>, info: &ClassInfo) -> Option<&'static str> {
    for base in &info.external_bases {
        let tail = base.rsplit('.').next().unwrap_or("");
        match tail.split('[').next().unwrap_or("") {
            "Protocol" => return Some("Protocol"),
            "ABC" => return Some("ABC"),
            _ => {}
        }
    }
    let module = facts.modules.get(&info.module)?;
    let metaclassed = match module.nodes[info.node as usize] {
        Cn::Stmt(Stmt::ClassDef(c)) => c.arguments.as_ref().is_some_and(|a| {
            a.keywords.iter().any(|kw| {
                kw.arg.as_ref().is_some_and(|n| n.as_str() == "metaclass")
                    && unparse::expr(&kw.value).contains("ABCMeta")
            })
        }),
        _ => false,
    };
    let abstract_method = info.methods.values().any(|q| {
        facts.symbols.get(&**q).is_some_and(|m| {
            facts.modules.get(&m.module).is_some_and(|module| {
                FUNCTION_KINDS.contains(&m.kind)
                    && decorator_names(fn_of(module, m)).contains("abstractmethod")
            })
        })
    });
    (metaclassed || abstract_method).then_some("ABC")
}

/// A Protocol is implemented by shape: any class with its methods, inherited
/// ones included. An abstraction is never an implementation, and a test
/// double is the second one. One implemented by a test double alone types a
/// foreign object the tests stand in for, and a published abstraction is a
/// contract downstream implements: neither is speculative.
fn single_impl_abstractions(facts: &RepoFacts<'_>, out: &mut Sink) {
    let kinds = abstraction_kinds(facts);
    let in_tests = |module: &Qname| facts.rel_of(module).is_some_and(|rel| is_test_path(rel));
    let mut classes: Vec<&Qname> = facts.classes.keys().collect();
    classes.sort();
    for cls_q in classes {
        let info = &facts.classes[cls_q];
        if in_tests(&info.module) || facts.symbols.get(cls_q).is_some_and(|s| facts.publishes(s)) {
            continue;
        }
        let Some(kind) = kinds.get(cls_q).copied().flatten() else {
            continue;
        };
        let nominal: HashSet<Qname> = class_walk(facts, cls_q, Step::Subclasses)
            .into_iter()
            .map(|(q, _)| q)
            .collect();
        let mut impls: Vec<&Qname> = facts
            .classes
            .keys()
            .filter(|q| {
                *q != cls_q
                    && kinds.get(*q).copied().flatten().is_none()
                    && (nominal.contains(*q)
                        || (kind == "Protocol"
                            && !info.methods.is_empty()
                            && has_every_method(facts, q, info)))
            })
            .collect();
        impls.sort();
        if impls.len() != 1 || in_tests(&facts.classes[impls[0]].module) {
            continue;
        }
        let module = &facts.modules[&info.module];
        out.push(Finding {
            rule: "37",
            site: node_site(facts, module, info.node),
            message: format!(
                "{kind} {cls_q} has exactly one implementation ({}) - speculative abstraction",
                impls[0]
            ),
            cause: format!("single-impl:{cls_q}"),
            evidence: Evidence::Idx {
                detail: kind.to_string(),
            },
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

/// Every method name the protocol declares, spelled somewhere on the
/// candidate's own internal base chain, itself included.
fn has_every_method(facts: &RepoFacts<'_>, candidate: &str, protocol: &ClassInfo) -> bool {
    let chain = class_walk(facts, candidate, Step::Bases);
    let spelled: HashSet<&str> = chain
        .iter()
        .flat_map(|(_, base)| base.methods.keys().map(|n| &**n))
        .collect();
    protocol.methods.keys().all(|n| spelled.contains(&**n))
}

// --- #48 fold candidate ------------------------------------------------------

/// the one-line bound, less a `a = 1; b = 2` run on one line
const FOLD_MAX_STMTS: usize = 4;
/// constants in one display: the name is what the table means
const VOCABULARY: usize = 3;
const EXPR_SCOPES: [Kind; 5] = [
    Kind::Lambda,
    Kind::ListComp,
    Kind::SetComp,
    Kind::DictComp,
    Kind::GeneratorExp,
];
const LANDINGS: [Kind; 4] = [Kind::Assign, Kind::AnnAssign, Kind::Return, Kind::Expr];
const ATOMS: [Kind; 9] = [
    Kind::Name,
    Kind::Constant,
    Kind::Call,
    Kind::Attribute,
    Kind::Subscript,
    Kind::List,
    Kind::Dict,
    Kind::Set,
    Kind::Tuple,
];

pub const RULE_48: Rule = Rule {
    record: RuleRecord {
        id: "48",
        slug: "fold-candidate",
        family: "surface",
        engine_class: "WP+IDX",
        posture: Posture::Ratchet,
        meaning: "private def with one prod call site and no other reference, \
                  body on one line: fold it into its caller",
        goal: "A name is a promise of reuse (John Ousterhout's shallow \
               module): a helper one reader calls once costs a hop and a \
               signature for nothing.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_48,
};

/// The body spells a literal table: the name is the table's meaning and the
/// call site would read the members instead.
fn names_a_vocabulary(module: &Module<'_>, sym: &Symbol) -> bool {
    module
        .nodes(
            &[Kind::Tuple, Kind::List, Kind::Set],
            Some(&sym.qname),
            false,
        )
        .into_iter()
        .any(|at| {
            let elts: &[Expr] = match module.nodes[at as usize] {
                Cn::Expr(Expr::Tuple(t)) => &t.elts,
                Cn::Expr(Expr::List(l)) => &l.elts,
                Cn::Expr(Expr::Set(s)) => &s.elts,
                _ => &[],
            };
            elts.iter()
                .filter(|e| Cn::Expr(e).kind() == Kind::Constant)
                .count()
                >= VOCABULARY
        })
}

/// Prose written about this name: a docstring, or a comment inside the def's
/// span. The fold deletes it.
fn carries_prose(module: &Module<'_>, sym: &Symbol) -> bool {
    let documented = match module.nodes[sym.node as usize] {
        Cn::Stmt(Stmt::FunctionDef(f)) => docstring(&f.body).is_some_and(|d| !d.is_empty()),
        _ => false,
    };
    documented
        || module
            .comments
            .iter()
            .any(|c| sym.lineno <= c.line && c.line <= sym.end_lineno)
}

/// Inside a lambda or comprehension, where statements cannot land.
fn in_expression_scope(module: &Module<'_>, node: NodeIndex) -> bool {
    let mut cur = module.parent_of(node);
    while let Some(at) = cur {
        let kind = module.nodes[at as usize].kind();
        if sightline_py_facts::kinds::is_def(kind) {
            return false;
        }
        if EXPR_SCOPES.contains(&kind) {
            return true;
        }
        cur = module.parent_of(at);
    }
    false
}

/// Where the helper's statements would go: a single-return body substitutes
/// anywhere; a `return` anywhere but last never lands; a generator's only as a
/// `for` iterable or under `yield from`; any other body only where the call is
/// the whole value of an assignment, return or expression statement.
fn statements_land(facts: &RepoFacts<'_>, sym: &Symbol, site: &CallSite) -> bool {
    let own = &facts.modules[&sym.module];
    let Cn::Stmt(Stmt::FunctionDef(def)) = own.nodes[sym.node as usize] else {
        return false;
    };
    let body = fn_body(&def.body);
    if body.len() == 1 && matches!(body[0], Stmt::Return(_)) {
        return true;
    }
    let last = body.last().and_then(|st| Cn::Stmt(st).stamped());
    if own
        .nodes(&[Kind::Return], Some(&sym.qname), false)
        .into_iter()
        .any(|at| Some(at) != last)
    {
        return false;
    }
    let caller = &facts.modules[&site.module];
    let holder = caller
        .parent_of(site.node)
        .map(|at| caller.nodes[at as usize]);
    if !own
        .nodes(&[Kind::Yield, Kind::YieldFrom], Some(&sym.qname), false)
        .is_empty()
    {
        return match holder {
            Some(Cn::Expr(Expr::YieldFrom(_))) => true,
            Some(Cn::Stmt(Stmt::For(f))) => Cn::Expr(&f.iter).stamped() == Some(site.node),
            _ => false,
        };
    }
    match holder {
        Some(Cn::Stmt(st)) if LANDINGS.contains(&Cn::Stmt(st).kind()) => {
            holder_value(st).and_then(|v| Cn::Expr(v).stamped()) == Some(site.node)
        }
        _ => false,
    }
}

/// The `.value` of an `Assign`, `AnnAssign`, `Return` or `Expr`.
fn holder_value(st: &Stmt) -> Option<&Expr> {
    match st {
        Stmt::Assign(a) => Some(&a.value),
        Stmt::AnnAssign(a) => a.value.as_deref(),
        Stmt::Return(r) => r.value.as_deref(),
        Stmt::Expr(e) => Some(&e.value),
        _ => None,
    }
}

/// The one prod call site of a def whose only reference in the repo is that
/// call. `None` when the def is decorated, override-fixed or referenced by
/// value, when the caller is itself or a test, when the call sits inside a
/// lambda or comprehension or where the body's statements cannot land, or when
/// the fold, priced at the call site's nesting depth, would put the caller past
/// #23's threshold.
fn fold_site<'a>(
    facts: &RepoFacts<'_>,
    calls: &'a CallGraph,
    sym: &Symbol,
) -> Option<&'a CallSite> {
    let refs = facts.refs_to.get(&sym.qname).map_or(&[][..], |v| v);
    let sites = calls.calls_to.get(&sym.qname).map_or(&[][..], |v| v);
    let own = &facts.modules[&sym.module];
    if refs.len() != 1
        || facts.refs[refs[0] as usize].kind != RefKind::Callee
        || sites.len() != 1
        || !decorator_names(fn_of(own, sym)).is_empty()
        || is_override_fixed(facts, sym)
    {
        return None;
    }
    let site = &calls.sites[sites[0] as usize];
    let module = &facts.modules[&site.module];
    if site.enclosing == sym.qname
        || is_test_path(&module.rel)
        || in_expression_scope(module, site.node)
        || !statements_land(facts, sym, site)
    {
        return None;
    }
    let helper_cc = facts.cc.get(&sym.qname).copied().unwrap_or(0);
    if helper_cc > 0
        && let Some(caller) = facts.symbols.get(&*site.enclosing)
        && FUNCTION_KINDS.contains(&caller.kind)
    {
        let parent = |n: Cn<'_>| {
            n.stamped()
                .and_then(|at| module.parent_of(at))
                .map(|up| module.nodes[up as usize])
        };
        let depth = nesting_at(module.nodes[site.node as usize], &parent);
        let Cn::Stmt(Stmt::FunctionDef(def)) = own.nodes[sym.node as usize] else {
            return None;
        };
        let caller_cc = facts.cc.get(&caller.qname).copied().unwrap_or(0);
        if caller_cc + cognitive_complexity(def, depth) >= facts.config.complexity_threshold {
            return None;
        }
    }
    Some(site)
}

/// A private def with one prod call site and no other reference, its whole
/// body on one line: the fold is a substitution, so the name adds a hop and a
/// signature for a single reader and nothing else. A body naming a literal
/// table is that table's documentation, and prose about the name is what the
/// call site has no room to hold.
fn rule_48(facts: &RepoFacts<'_>, provers: &Provers, out: &mut Sink) {
    let calls = provers.calls(facts);
    let unseen = provers.unseen(facts);
    for (module, sym) in iter_prod_functions(facts) {
        let Cn::Stmt(Stmt::FunctionDef(def)) = module.nodes[sym.node as usize] else {
            continue;
        };
        let stmts = fn_body(&def.body);
        if !sym.name.starts_with('_')
            || sym.name.ends_with("__")
            || is_stub(stmts)
            || stmts.len() > FOLD_MAX_STMTS
        {
            continue;
        }
        let ends = stmts.last().and_then(|st| Cn::Stmt(st).stamped());
        let starts = stmts.first().and_then(|st| Cn::Stmt(st).stamped());
        let one_line = match (ends, starts) {
            (Some(ends), Some(starts)) => module.end_line_of(ends) == module.line_of(starts),
            _ => false,
        };
        if !one_line
            || names_a_vocabulary(module, sym)
            || unseen.named(&sym.name)
            || carries_prose(module, sym)
        {
            continue;
        }
        let Some(site) = fold_site(facts, calls, sym) else {
            continue;
        };
        out.push(Finding {
            rule: "48",
            site: node_site(facts, module, sym.node),
            message: format!(
                "{} (one line) is called once, from {}: fold it into the caller",
                sym.qname, site.enclosing
            ),
            cause: format!("fold:{}", sym.qname),
            evidence: Evidence::Wp {
                premises: vec![
                    "prod-callers:1".to_string(),
                    format!("caller:{}", site.enclosing),
                ],
            },
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

/// Param names in the body's expression replaced by the call's arguments. A
/// replacement is not visited again, so no substitution feeds another.
struct Bind<'a> {
    bound: &'a IndexMap<String, Expr>,
}

impl Transformer for Bind<'_> {
    fn visit_expr(&self, expr: &mut Expr) {
        if let Expr::Name(n) = &*expr
            && n.ctx == ExprContext::Load
            && let Some(replacement) = self.bound.get(n.id.as_str())
        {
            // the replacement stands where the name stood: ruff's generator
            // prints a call's arguments in range order, and one that kept the
            // call site's own range would print out of place. The nested
            // ranges stay as they are, since `iter_source_order` reads the top
            // level alone, and `substitutable` admits names, literals and
            // attribute chains only.
            let range = n.range;
            *expr = replacement.clone();
            match expr {
                Expr::Name(n) => n.range = range,
                Expr::Attribute(a) => a.range = range,
                Expr::NumberLiteral(n) => n.range = range,
                Expr::StringLiteral(s) => s.range = range,
                Expr::BytesLiteral(b) => b.range = range,
                Expr::BooleanLiteral(b) => b.range = range,
                Expr::NoneLiteral(n) => n.range = range,
                Expr::EllipsisLiteral(e) => e.range = range,
                _ => {}
            }
            return;
        }
        walk_expr(self, expr);
    }
}

/// A name, a literal or an attribute chain off one: the fold moves the
/// argument, and anything that runs can be skipped by a short circuit,
/// doubled, or reordered.
fn substitutable(e: &Expr) -> bool {
    let mut cur = e;
    while let Expr::Attribute(a) = cur {
        cur = &a.value;
    }
    matches!(Cn::Expr(cur).kind(), Kind::Name | Kind::Constant)
}

/// Every param bound to the argument the one call site passes, defaults for
/// the rest. `None` where the binding is not by name alone or where any bound
/// expression is not substitutable.
fn bound_args(
    def: &ruff_python_ast::StmtFunctionDef,
    call: &ExprCall,
) -> Option<IndexMap<String, Expr>> {
    let params = &def.parameters;
    let names: Vec<&str> = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .map(|a| a.parameter.name.as_str())
        .collect();
    let args = &call.arguments.args;
    if params.vararg.is_some()
        || params.kwarg.is_some()
        || !params.kwonlyargs.is_empty()
        || args.len() > names.len()
        || args.iter().any(|a| matches!(a, Expr::Starred(_)))
        || call
            .arguments
            .keywords
            .iter()
            .any(|k| k.arg.as_ref().is_none_or(|n| !names.contains(&n.as_str())))
    {
        return None;
    }
    let defaults: Vec<&Expr> = fn_defaults(def).into_iter().map(|(_, d)| d).collect();
    let mut bound: IndexMap<String, Expr> = IndexMap::new();
    for (name, default) in names[names.len() - defaults.len()..].iter().zip(&defaults) {
        bound.insert((*name).to_string(), (*default).clone());
    }
    for (name, arg) in names.iter().zip(args) {
        bound.insert((*name).to_string(), arg.clone());
    }
    for kw in &call.arguments.keywords {
        if let Some(name) = &kw.arg {
            bound.insert(name.to_string(), kw.value.clone());
        }
    }
    (bound.len() == names.len() && bound.values().all(substitutable)).then_some(bound)
}

/// #48's fold as a patch, single-return bodies only: the call site takes the
/// returned expression with each param bound to its argument, and the helper's
/// lines go with it. Module-level def, one call site on one line in the
/// helper's own module, no name a string or an attribute reaches.
pub fn fold_splice(cause: &str, facts: &RepoFacts<'_>, provers: &Provers) -> Option<Splice> {
    let qname = cause.strip_prefix("fold:").unwrap_or(cause);
    let sym = facts.symbols.get(qname)?;
    let sites = provers
        .calls(facts)
        .calls_to
        .get(&sym.qname)
        .map_or(&[][..], |v| v);
    if sym.parent.is_some() || sites.len() != 1 || provers.unseen(facts).reached(&sym.name) {
        return None;
    }
    let site = &provers.calls(facts).sites[sites[0] as usize];
    let module = facts.modules.get(&sym.module)?;
    let Cn::Stmt(Stmt::FunctionDef(def)) = module.nodes[sym.node as usize] else {
        return None;
    };
    let body = fn_body(&def.body);
    let returned = match body {
        [Stmt::Return(r)] => r.value.as_deref(),
        _ => None,
    }?;
    if site.module != sym.module {
        return None;
    }
    let span = module.span(site.node)?;
    let (line, col, end_line, end_col) = (span[0]?, span[1]?, span[2], span[3]?);
    if end_line != Some(line) {
        return None;
    }
    let call = call_of(module, site.node)?;
    let bound = bound_args(def, call)?;
    let gone = deletion(module, sym.node);
    if gone.is_empty() {
        return None;
    }
    let mut root = returned.clone();
    Bind { bound: &bound }.visit_expr(&mut root);
    let mut text = unparse::expr(&root);
    // an atom substitutes bare, and so does the whole value of a statement;
    // anywhere else the surrounding expression's precedence is unknown
    let holder = module
        .parent_of(site.node)
        .map(|at| module.nodes[at as usize]);
    let lands = match holder {
        Some(Cn::Stmt(st)) if LANDINGS.contains(&Cn::Stmt(st).kind()) => {
            holder_value(st).and_then(|v| Cn::Expr(v).stamped()) == Some(site.node)
        }
        _ => false,
    };
    if !ATOMS.contains(&Cn::Expr(&root).kind()) && !lands {
        text = format!("({text})");
    }
    let mut edits = vec![SpanEdit {
        line,
        col_start: col,
        col_end: end_col,
        text,
    }];
    edits.extend(gone);
    Some(Splice {
        id: cause.to_string(),
        owner: module.qname.to_string(),
        edits,
        spelling: String::new(),
        imports: Vec::new(),
        param: String::new(),
    })
}

// --- #54 kind switch ---------------------------------------------------------

pub const RULE_54: Rule = Rule {
    record: RuleRecord {
        id: "54",
        slug: "kind-switch",
        family: "surface",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: "same identifier-spelled tag set switched on in >=3 prod functions",
        goal: "Replace conditional with polymorphism (Fowler): a kind tag \
               tested in three places is a type whose dispatch is spread by \
               hand.",
        lang: "py",
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_54,
};

/// `x`, `x.kind`, `self.a.b` spelled; `None` off any other root.
fn switch_subject(node: &Expr) -> Option<String> {
    let mut root = node;
    while let Expr::Attribute(a) = root {
        root = &a.value;
    }
    matches!(root, Expr::Name(_)).then(|| unparse::expr(node))
}

/// The string literals, where every element is one.
fn strings(elts: &[&Expr]) -> Option<BTreeSet<String>> {
    let vals: Vec<String> = elts
        .iter()
        .filter_map(|e| match e {
            Expr::StringLiteral(s) => Some(s.value.to_str().to_string()),
            _ => None,
        })
        .collect();
    (!vals.is_empty() && vals.len() == elts.len()).then(|| vals.into_iter().collect())
}

/// (subject, literals) of `x == "a"`, `"a" == x` or `x in ("a", "b")`.
fn switch_case(test: &Expr) -> Option<(String, BTreeSet<String>)> {
    let Expr::Compare(c) = test else {
        return None;
    };
    if c.ops.len() != 1 || c.comparators.len() != 1 {
        return None;
    }
    let (left, lits) = match c.ops[0] {
        CmpOp::Eq => {
            let (left, right) = if matches!(Cn::Expr(&c.left).kind(), Kind::Constant) {
                (&c.comparators[0], &*c.left)
            } else {
                (&*c.left, &c.comparators[0])
            };
            (left, strings(&[right]))
        }
        CmpOp::In => {
            let elts: &[Expr] = match &c.comparators[0] {
                Expr::Tuple(t) => &t.elts,
                Expr::List(l) => &l.elts,
                Expr::Set(s) => &s.elts,
                _ => return None,
            };
            (&*c.left, strings(&elts.iter().collect::<Vec<&Expr>>()))
        }
        _ => return None,
    };
    let subject = switch_subject(left)?;
    let lits = lits?;
    (!subject.is_empty()).then_some((subject, lits))
}

/// Per subject, its first switch and every identifier-spelled string the
/// function's `if` tests (`or` flattened) and `match` cases compare it against.
fn switches(module: &Module<'_>, sym: &Symbol) -> IndexMap<String, (NodeIndex, BTreeSet<String>)> {
    let mut out: IndexMap<String, (NodeIndex, BTreeSet<String>)> = IndexMap::new();
    let mut ordered = module.nodes(&[Kind::If, Kind::Match], Some(&sym.qname), false);
    document_order(&mut ordered, |at| {
        let span = module.span(*at).unwrap_or_default();
        (
            span[0].unwrap_or(0),
            span[1].unwrap_or(0),
            span[2].unwrap_or(0),
            span[3].unwrap_or(0),
        )
    });
    for at in ordered {
        let cases: Vec<(String, BTreeSet<String>)> = match module.nodes[at as usize] {
            Cn::Stmt(Stmt::Match(m)) => {
                let patterns: Vec<&Pattern> = m
                    .cases
                    .iter()
                    .flat_map(|c| match &c.pattern {
                        Pattern::MatchOr(or) => or.patterns.iter().collect::<Vec<&Pattern>>(),
                        other => vec![other],
                    })
                    .collect();
                let values: Vec<&Expr> = patterns
                    .iter()
                    .filter_map(|p| match p {
                        Pattern::MatchValue(v) => Some(&*v.value),
                        _ => None,
                    })
                    .collect();
                match (switch_subject(&m.subject), strings(&values)) {
                    (Some(subject), Some(lits)) if !subject.is_empty() => {
                        vec![(subject, lits)]
                    }
                    _ => Vec::new(),
                }
            }
            node => {
                let Some(test) = if_test(node) else { continue };
                let tests: Vec<&Expr> = match test {
                    Expr::BoolOp(b) if b.op == ruff_python_ast::BoolOp::Or => {
                        b.values.iter().collect()
                    }
                    other => vec![other],
                };
                tests.into_iter().filter_map(switch_case).collect()
            }
        };
        for (subject, lits) in cases {
            // a tag is spelled the way a member of the enum it wants would be
            let held = out.entry(subject).or_insert((at, BTreeSet::new()));
            held.1
                .extend(lits.into_iter().filter(|s| pytext::is_identifier(s)));
        }
    }
    out
}

/// The test of an `If` statement or of the `elif` clause CPython reads as one.
fn if_test(node: Cn<'_>) -> Option<&Expr> {
    match node {
        Cn::Stmt(Stmt::If(n)) => Some(&n.test),
        Cn::Elif(rest) => rest[0].test.as_ref(),
        _ => None,
    }
}

/// Per prod function and subject, the strings it is switched over across
/// `if`/`elif` tests and `match` cases; a switch is >=2 of them. Literal pairs
/// shared by >=3 functions group as #14's triples do.
fn rule_54(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    let mut pairs: IndexMap<BTreeSet<String>, Owners> = IndexMap::new();
    for (module, sym) in iter_prod_functions(facts) {
        for (node, lits) in switches(module, sym).values() {
            let sorted: Vec<&String> = lits.iter().collect();
            for i in 0..sorted.len() {
                for j in i + 1..sorted.len() {
                    let combo = BTreeSet::from([sorted[i].clone(), sorted[j].clone()]);
                    pairs
                        .entry(combo)
                        .or_default()
                        .entry(sym.qname.clone())
                        .or_insert((sym.module.clone(), *node));
                }
            }
        }
    }
    for group in shared_groups(facts, &pairs, 3) {
        let module = &facts.modules[&group.module];
        let spelled: Vec<String> = group.names.iter().map(|s| pytext::repr_str(s)).collect();
        let owners: Vec<&str> = group.qnames.iter().map(|q| &**q).collect();
        out.push(Finding {
            rule: "54",
            site: node_site(facts, module, group.node),
            message: format!(
                "{{{}}} switched over in {} functions ({}) - dispatch wants one home",
                spelled.join(", "),
                group.qnames.len(),
                owner_list(&owners)
            ),
            cause: format!(
                "kind-switch:{}",
                group
                    .names
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<&str>>()
                    .join(",")
            ),
            evidence: Evidence::idx(),
            salience: group.qnames.len() as f64,
            fix: None,
            lang: "py",
        });
    }
}

// --- #55 positional width ----------------------------------------------------

const WIDTH: usize = 5;

pub const RULE_55: Rule = Rule {
    record: RuleRecord {
        id: "55",
        slug: "positional-width",
        family: "surface",
        engine_class: "IDX",
        posture: Posture::Ratchet,
        meaning: ">=5 positional params (receiver out) and no `*` marker",
        goal: "Past a handful, positional slots are ordered by memory \
               (Martin's polyadic limit): a `*` marker or a record names each \
               one at the call site.",
        lang: "py",
        // reads the class base chain (override-fixed) - a rule on repo-wide
        // state may not claim file scope
        scope: Scope::Repo,
        complement: "",
    },
    run: rule_55,
};

/// Positional-only and positional-or-keyword params, the receiver dropped, on
/// a prod def with neither `*` nor `*args`; an override's signature is the
/// base's, a test's params are fixtures, and a module-level signature another
/// prod module's def spells identically is a plugin contract.
fn rule_55(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    let plugins = plugin_signatures(facts);
    for (module, sym) in iter_prod_functions(facts) {
        let fn_def = fn_of(module, sym);
        let width = fn_pos_args(fn_def).len();
        let signature = (
            sym.name.to_string(),
            fn_params(fn_def)
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<String>>(),
        );
        if width < WIDTH
            || fn_def.parameters.vararg.is_some()
            || !fn_def.parameters.kwonlyargs.is_empty()
            || is_override_fixed(facts, sym)
            || (sym.parent.is_none() && plugins.contains(&signature))
        {
            continue;
        }
        out.push(Finding {
            rule: "55",
            site: node_site(facts, module, sym.node),
            message: format!(
                "{} takes {width} positional params with no `*` - callers order \
                 them by memory",
                sym.qname
            ),
            cause: format!("positional-width:{}", sym.qname),
            evidence: Evidence::idx(),
            salience: width as f64,
            fix: None,
            lang: "py",
        });
    }
}

// --- #23 cognitive-complexity emitter ----------------------------------------
// The ranking prior alone never surfaced judged instances: it stays, and a
// REPORT finding also fires past SonarSource's threshold.

pub const RULE_23: Rule = Rule {
    record: RuleRecord {
        id: "23",
        slug: "cognitive-complexity",
        family: "surface",
        engine_class: "AST",
        posture: Posture::Report,
        meaning: "cognitive complexity >= 15; also the ranking prior",
        goal: "Complexity predicts comprehension time (meta-analysis); \
               REPORT only: a gate here would push authors to extract \
               helpers to dodge it.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_23,
};

fn rule_23(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for (module, sym) in iter_functions(facts) {
        let cc = facts.cc.get(&sym.qname).copied().unwrap_or(0);
        let threshold = facts.config.complexity_threshold;
        if cc < threshold {
            continue;
        }
        out.push(Finding {
            rule: "23",
            site: node_site(facts, module, sym.node),
            message: format!(
                "{} has cognitive complexity {cc} (threshold {threshold})",
                sym.qname
            ),
            cause: format!("cognitive-complexity:{}", sym.qname),
            evidence: Evidence::ast(),
            salience: f64::from(cc),
            fix: None,
            lang: "py",
        });
    }
}
