//! `scope.py`'s footprint half: what a body demands of each param, and
//! the alias and mutation questions that read the footprints.

use super::*;

/// What a body demands of one param - #5's body-usage widening, #10's
/// protocol check.
#[derive(Debug, Clone, Default)]
pub struct Footprint {
    /// `p.x` reads
    pub attrs: BTreeSet<String>,
    /// `p.m(...)` methods
    pub called: BTreeSet<String>,
    /// `p[...]` read
    pub subscripted: bool,
    /// `p[...] =` / `del p[...]`
    pub sub_stored: bool,
    /// `for _ in p` / comprehension iter
    pub iterated: bool,
    /// `len(p)`
    pub sized: bool,
    /// `x in p`
    pub contained: bool,
    /// mutator method / subscript store / augassign
    pub mutated: bool,
    /// resolved (callee qname, parameter position)
    pub forwarded: Vec<(Qname, usize)>,
    /// any raw use we do not model (return, compare, ...)
    pub other: bool,
}

impl Scope {
    /// How each param is used, self/cls included: one pass over the loads the
    /// scope reaches, so a nested def rebinding a param hides it.
    pub fn footprints(&self, facts: &RepoFacts<'_>) -> &IndexMap<String, Footprint> {
        self.footprints.get_or_init(|| {
            let module = self.module(facts);
            let mut out: IndexMap<String, Footprint> = self
                .params(facts)
                .iter()
                .map(|p| (p.clone(), Footprint::default()))
                .collect();
            for node in self.names(facts) {
                let Some(id) = name_id(module, *node) else {
                    continue;
                };
                if !out.contains_key(id) || self.shadowed(module, *node, id) {
                    continue;
                }
                let loaded = matches!(
                    module.nodes[*node as usize],
                    Cn::Expr(Expr::Name(n)) if n.ctx == ExprContext::Load
                );
                if (loaded || in_place(module, module.parent_of(*node)))
                    && let Some(fp) = out.get_mut(id)
                {
                    self.classify(facts, module, *node, fp);
                }
            }
            out
        })
    }

    /// Params whose object the body mutates (effects' `mutates-arg`/`-self`).
    pub fn mutated_params(&self, facts: &RepoFacts<'_>) -> &BTreeSet<String> {
        self.mutated_params.get_or_init(|| {
            self.footprints(facts)
                .iter()
                .filter(|(_, fp)| fp.mutated)
                .map(|(p, _)| p.clone())
                .collect()
        })
    }

    /// A write through a local aliasing shared state (`ys = xs;
    /// ys.append(1)`), or in place on it (`ys += [1]`).
    pub fn mutates_alias(&self, facts: &RepoFacts<'_>) -> bool {
        *self.mutates_alias.get_or_init(|| {
            let module = self.module(facts);
            let tainted = self.alias_tainted(facts);
            self.writes(facts).iter().any(|w| {
                w.root.as_ref().is_some_and(|r| tainted.contains(r))
                    && (THROUGH.contains(&w.kind) || in_place(module, module.parent_of(w.node)))
            })
        })
    }

    /// Does a nested def or lambda between `node` and the body rebind it?
    pub(super) fn shadowed(&self, module: &Module<'_>, node: NodeIndex, name: &str) -> bool {
        self.ancestry(module, node).into_iter().any(|p| {
            LEXICAL.contains(&module.nodes[p as usize].kind())
                && all_arg_names(parameters_of(module, p)).contains(name)
        })
    }

    fn classify(
        &self,
        facts: &RepoFacts<'_>,
        module: &Module<'_>,
        node: NodeIndex,
        fp: &mut Footprint,
    ) {
        let Some(parent) = module.parent_of(node) else {
            fp.other = true;
            return;
        };
        let is_node = |e: &Expr| Cn::Expr(e).stamped() == Some(node);
        match module.nodes[parent as usize] {
            Cn::Expr(Expr::Attribute(a)) if is_node(&a.value) => {
                let called = parent_node(module, parent)
                    .is_some_and(|gp| {
                        matches!(gp, Cn::Expr(Expr::Call(c)) if Cn::Expr(&c.func).stamped() == Some(parent))
                    });
                if called {
                    fp.called.insert(a.attr.to_string());
                    if MUTATOR_METHODS.contains(a.attr.as_str()) {
                        fp.mutated = true;
                    }
                } else if a.ctx == ExprContext::Load {
                    fp.attrs.insert(a.attr.to_string());
                } else {
                    fp.mutated = true;
                    fp.other = true;
                }
            }
            Cn::Expr(Expr::Subscript(s)) if is_node(&s.value) => {
                if s.ctx == ExprContext::Load {
                    fp.subscripted = true;
                } else {
                    fp.sub_stored = true;
                    fp.mutated = true;
                }
            }
            Cn::Stmt(Stmt::For(f)) if is_node(&f.iter) => fp.iterated = true,
            Cn::Comp(c) if is_node(&c.iter) => fp.iterated = true,
            Cn::Expr(Expr::Call(c))
                if matches!(&*c.func, Expr::Name(n) if n.id.as_str() == "len")
                    && c.arguments.args.iter().any(is_node) =>
            {
                fp.sized = true;
            }
            Cn::Expr(Expr::Compare(c))
                if c.comparators.iter().any(is_node)
                    && c.ops
                        .iter()
                        .any(|op| matches!(op, CmpOp::In | CmpOp::NotIn)) =>
            {
                fp.contained = true;
            }
            Cn::Stmt(Stmt::AugAssign(a)) if is_node(&a.target) => {
                fp.mutated = true;
                fp.other = true;
            }
            Cn::Expr(Expr::Call(c)) if c.arguments.args.iter().any(is_node) => {
                let at = c
                    .arguments
                    .args
                    .iter()
                    .position(is_node)
                    .expect("the guard found it");
                self.forward(facts, module, parent, Key::Pos(at), fp);
            }
            Cn::Keyword(k) if k.arg.is_some() && is_node(&k.value) => {
                let call = module.parent_of(parent);
                let name = k.arg.as_ref().expect("the guard checked it").to_string();
                match call {
                    Some(call) => self.forward(facts, module, call, Key::Kw(name), fp),
                    None => fp.other = true,
                }
            }
            _ => fp.other = true,
        }
    }

    /// The param passed to a resolved callee at a position, or by the keyword
    /// the callee's own signature places (receiver dropped, as the footprint's
    /// readers see it); anything else is a raw use.
    fn forward(
        &self,
        facts: &RepoFacts<'_>,
        module: &Module<'_>,
        call: NodeIndex,
        key: Key,
        fp: &mut Footprint,
    ) {
        let site = facts
            .call_index
            .get(&(module.id, call))
            .map(|i| &facts.call_sites[*i as usize]);
        let target = site
            .filter(|s| s.resolution == Resolution::Resolved)
            .and_then(|s| s.target.clone());
        let at = match (&key, &target) {
            (Key::Kw(name), Some(target)) => {
                let params: Vec<String> = facts
                    .symbols
                    .get(target)
                    .filter(|s| FUNCTION_KINDS.contains(&s.kind))
                    .and_then(|s| {
                        let owner = facts.modules.get(&s.module)?;
                        match owner.nodes[s.node as usize] {
                            Cn::Stmt(Stmt::FunctionDef(f)) => Some(f),
                            _ => None,
                        }
                    })
                    .map(|f| {
                        let args = fn_args(f);
                        sightline_py_facts::astutil::without_receiver(&args)
                            .iter()
                            .map(|a| a.name.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                params.iter().position(|p| p == name)
            }
            (Key::Kw(_), None) => None,
            (Key::Pos(at), _) => Some(*at),
        };
        match (target, at) {
            (Some(target), Some(at)) => fp.forwarded.push((target, at)),
            _ => fp.other = true,
        }
    }
}
