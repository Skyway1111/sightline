//! `rs/provers.py`'s clone queries: the per-function digest, the mined
//! sequences and block repeats, and the two node-keyed caches under them.

use super::*;

/// A mined statement sequence plus the statements this front digested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsSeq<'t> {
    pub seq: Seq,
    pub module: Qname,
    pub owner: Qname,
    pub stmts: Vec<Node<'t>>,
}

/// One mined repeat: the digest keying it, the window's per-statement
/// shapes, and the owner and statements of each occurrence.
pub struct RsCloneGroup<'t> {
    pub key: String,
    pub shapes: Vec<String>,
    pub members: Vec<(&'t RsSymbol<'t>, Vec<Node<'t>>)>,
}

impl<'t> RsProvers<'t> {
    /// Cognitive complexity of the symbol's body, the same number the rank
    /// prior reads (#23).
    pub fn complexity(&self, qname: &str) -> u32 {
        self.facts.cc_prior(qname)
    }

    /// symbol qname -> the blind digest of its whole body, for bodies past
    /// the mining's node floor (#11's function arm).
    pub fn function_digests(&self) -> &IndexMap<Qname, String> {
        self.function_digests.get_or_init(|| {
            let mut out: IndexMap<Qname, String> = IndexMap::new();
            for (qname, sym) in &self.facts.symbols {
                let body = if is_fn(sym) {
                    sym.node.child_by_field_name("body")
                } else {
                    None
                };
                let Some(body) = body else { continue };
                let src = self.facts.modules[&sym.module].bytes;
                let stmts = statements(body);
                if stmts.iter().map(|s| self.size(*s)).sum::<usize>() + 1 < MIN_CLONE_NODES {
                    continue;
                }
                let shapes: Vec<Arc<str>> = stmts.iter().map(|s| self.shape(*s, src)).collect();
                let joined: Vec<&str> = shapes.iter().map(|s| &**s).collect();
                out.insert(qname.clone(), digest(&joined.join("\n")));
            }
            out
        })
    }

    /// Every `fn` body and nested block as a digest sequence, for the neutral
    /// repeat mining (`core::clones::repeats`).
    pub fn clone_sequences(&self) -> &[RsSeq<'t>] {
        self.clone_sequences.get_or_init(|| {
            let mut out: Vec<RsSeq<'t>> = Vec::new();
            for (qname, sym) in &self.facts.symbols {
                if !is_fn(sym) {
                    continue;
                }
                let module = &self.facts.modules[&sym.module];
                for (stmts, top) in own_sequences(sym.node) {
                    if stmts.len() < MIN_BLOCK_STMTS {
                        continue;
                    }
                    out.push(RsSeq {
                        seq: Seq {
                            digests: stmts
                                .iter()
                                .map(|s| digest(&self.shape(*s, module.bytes)))
                                .collect(),
                            sizes: stmts.iter().map(|s| self.size(*s)).collect(),
                            order: module.rel.to_string(),
                            top,
                            prod: !sym.is_test,
                        },
                        module: sym.module.clone(),
                        owner: qname.clone(),
                        stmts,
                    });
                }
            }
            out
        })
    }

    /// The neutral repeat mining back on the statements it ran over (#11's
    /// block arm); the function arm owns the whole-body duplicates.
    pub fn block_clones(&self) -> &[RsCloneGroup<'t>] {
        self.block_clones.get_or_init(|| {
            let rows = self.clone_sequences();
            let seqs: Vec<Seq> = rows.iter().map(|r| r.seq.clone()).collect();
            repeats(&seqs)
                .into_iter()
                .map(|rep| {
                    let (first, start) = rep.runs[0];
                    RsCloneGroup {
                        key: rep.key,
                        shapes: seqs[first].digests[start..start + rep.length].to_vec(),
                        members: rep
                            .runs
                            .iter()
                            .map(|&(s, i)| {
                                (
                                    &self.facts.symbols[&rows[s].owner],
                                    rows[s].stmts[i..i + rep.length].to_vec(),
                                )
                            })
                            .collect(),
                    }
                })
                .collect()
        })
    }

    /// The blind digest text: structure and the tokens that decide something
    /// (`+` is not `-`, `<` is not `>`), every name and literal noise,
    /// comments no part of the code. Memoized so a node nested d deep is
    /// serialized once.
    pub fn shape(&self, node: Node<'_>, src: &[u8]) -> Arc<str> {
        cached(&self.shapes, node.id(), || {
            let kind = node.kind();
            if has(IDENTS, kind) {
                return "n".into();
            }
            if has(LITERALS, kind) {
                return "c".into();
            }
            let parts: Vec<String> = children(node)
                .into_iter()
                .filter(|c| !has(COMMENTS, c.kind()))
                .map(|c| {
                    if c.is_named() {
                        self.shape(c, src).to_string()
                    } else {
                        text(c, src).into_owned()
                    }
                })
                .collect();
            format!("{kind}({})", parts.join(",")).into()
        })
    }

    /// The digest text where the names are the fact: every token as written,
    /// `params` renamed. The blind shape reads two comparators that sort on
    /// different fields as one closure, so #20 keys on this.
    pub(super) fn content(
        &self,
        node: Node<'_>,
        params: &IndexMap<String, String>,
        src: &[u8],
    ) -> String {
        if node.named_child_count() == 0 {
            let spelled = text(node, src).into_owned();
            return params.get(&spelled).cloned().unwrap_or(spelled);
        }
        let parts: Vec<String> = children(node)
            .into_iter()
            .filter(|c| !has(COMMENTS, c.kind()))
            .map(|c| {
                if c.is_named() {
                    self.content(c, params, src)
                } else {
                    text(c, src).into_owned()
                }
            })
            .collect();
        format!("{}({})", node.kind(), parts.join(","))
    }

    /// The nodes in a subtree, the "worth a name" floor's unit.
    pub fn size(&self, node: Node<'_>) -> usize {
        cached(&self.sizes, node.id(), || {
            1 + named_children(node)
                .into_iter()
                .map(|c| self.size(c))
                .sum::<usize>()
        })
    }
}
