//! Comment queries: a module's `#[allow]`s, the doc lines above an item,
//! its header, and what a comment run reads as.

use super::*;

/// One `#[allow(...)]`: a place a module silences the compiler, and the
/// lint names it silences there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsAllow {
    pub names: Vec<String>,
    pub line: u32,
}

/// A run of adjacent non-doc comment lines, and the two readings of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsCommentBlock {
    pub start: u32,
    pub lines: Vec<String>,
    /// parses as at least one item or statement, no ERROR (#34)
    code: std::sync::OnceLock<bool>,
    /// its head is a phase label (#18)
    pub label: bool,
}

/// A run shorter than this is never reported as commented-out code, so its
/// reading is left to whoever asks (`rs-rules` #34 reads the same floor).
pub const MIN_CODE_LINES: usize = 3;

impl RsCommentBlock {
    /// The #34 reading, taken on demand: the two parses it costs are the
    /// whole comment pass, and #34 asks only of a run long enough to report.
    pub fn code(&self) -> bool {
        *self.code.get_or_init(|| parses_as_code(&self.lines))
    }
}

impl<'t> RsProvers<'t> {
    /// module qname -> every `#[allow(...)]` written in that file, with the
    /// lints it silences (`facts` prints their count).
    pub fn allows(&self) -> &IndexMap<Qname, Vec<RsAllow>> {
        self.allows.get_or_init(|| {
            self.facts
                .modules
                .iter()
                .map(|(qname, m)| {
                    let rows = descend(m.root, ALL)
                        .into_iter()
                        .filter(|n| has(ATTRS, n.kind()))
                        .filter_map(|n| {
                            let names = allow_names(n, m.bytes);
                            (!names.is_empty()).then(|| RsAllow {
                                names,
                                line: n.start_position().row as u32 + 1,
                            })
                        })
                        .collect();
                    (qname.clone(), rows)
                })
                .collect()
        })
    }

    /// module qname -> every line a `///` or `/** */` doc holds, mapped to
    /// that comment's index. A line comment node spans into the row after
    /// it, so the rows are counted off the stripped body: the item on that
    /// next row owns no doc, which is what tells a `fn`'s own doc from the
    /// one a `const` in its body would otherwise claim.
    fn doc_lines(&self) -> &HashMap<Qname, HashMap<u32, usize>> {
        self.doc_lines.get_or_init(|| {
            self.facts
                .modules
                .iter()
                .map(|(qname, m)| {
                    let mut order: Vec<usize> = (0..m.comments.len()).collect();
                    order.sort_by_key(|i| m.comments[*i].line);
                    let mut covers: HashMap<u32, usize> = HashMap::new();
                    for i in order {
                        let found = &m.comments[i];
                        if found.kind != "doc" {
                            continue;
                        }
                        let rows = pytext::splitlines(&found.text).len().max(1) as u32;
                        for line in found.line..found.line + rows {
                            covers.insert(line, i);
                        }
                    }
                    (qname.clone(), covers)
                })
                .collect()
        })
    }

    /// The doc run written directly above the item starting at this line, in
    /// order; an attribute line between the run and the item does not end it
    /// (#39's copied-doc arm, #53's `# Errors` section).
    pub fn doc_above(&self, module: &RsModule<'_>, lineno: u32) -> Vec<String> {
        let covers = &self.doc_lines()[&module.qname];
        let mut out: Vec<String> = Vec::new();
        let mut line = lineno.saturating_sub(1);
        while line >= 1 {
            if let Some(i) = covers.get(&line) {
                let found = &module.comments[*i];
                out.push(found.text.clone());
                line = found.line.saturating_sub(1);
            } else if pytext::lstrip(module.lines[line as usize - 1]).starts_with("#[") {
                line -= 1;
            } else {
                break;
            }
        }
        out.reverse();
        out
    }

    /// module qname -> what the file says about itself: its `//!` lines, else
    /// the `#![doc = ...]` attribute that is a header by another spelling -
    /// `include_str!` of a README is the doc rustdoc shows (#29).
    /// `#![doc(html_logo_url = ...)]` configures rustdoc and says nothing.
    pub fn module_docs(&self) -> &IndexMap<Qname, Vec<String>> {
        self.module_docs.get_or_init(|| {
            self.facts
                .modules
                .iter()
                .map(|(qname, m)| {
                    if !m.doc.is_empty() {
                        return (qname.clone(), m.doc.clone());
                    }
                    let rows = children(m.root)
                        .into_iter()
                        .filter(|n| n.kind() == "inner_attribute_item")
                        .filter_map(|n| {
                            let raw = text(n, m.bytes);
                            let body = pytext::removeprefix(&raw, "#![");
                            let attr = pytext::strip(body.strip_suffix(']').unwrap_or(body));
                            let named = pytext::lstrip(attr.get(3..).unwrap_or(""));
                            (attr.starts_with("doc") && named.starts_with('='))
                                .then(|| attr.to_string())
                        })
                        .collect();
                    (qname.clone(), rows)
                })
                .collect()
        })
    }

    /// module qname -> its runs of adjacent non-doc comment lines, each with
    /// the one reading of "this run is code".
    /// Every run in every module, read in parallel: `blocks` parses each run
    /// twice, which is the whole comment pass on a Rust tree. Warmed by
    /// `run_rules` on the calling thread, never first touched from a worker.
    pub fn comment_blocks(&self) -> &IndexMap<Qname, Vec<RsCommentBlock>> {
        self.comment_blocks.get_or_init(|| {
            use rayon::prelude::*;
            let modules: Vec<_> = self.facts.modules.iter().collect();
            modules
                .par_iter()
                .map(|(qname, m)| ((*qname).clone(), blocks(&m.comments)))
                .collect::<Vec<_>>()
                .into_iter()
                .collect()
        })
    }
}

fn blocks(comments: &[RsComment]) -> Vec<RsCommentBlock> {
    let mut runs: Vec<Vec<&RsComment>> = Vec::new();
    for c in comments.iter().filter(|c| c.kind == "comment") {
        match runs.last_mut() {
            Some(run) if run.last().expect("a run is never empty").end_line + 1 == c.line => {
                run.push(c);
            }
            _ => runs.push(vec![c]),
        }
    }
    runs.into_iter()
        .map(|run| {
            let lines: Vec<String> = run.iter().map(|c| c.text.clone()).collect();
            RsCommentBlock {
                start: run[0].line,
                code: std::sync::OnceLock::new(),
                label: is_phase_label(&run[0].text, COMMENT_PREFIX),
                lines,
            }
        })
        .collect()
}

/// Does a run of comment lines read as Rust - at least one item or
/// statement, no ERROR node (#34)? A bare statement needs a `fn` around it,
/// so the run is tried both ways. The one reading; a rule never parses.
pub fn parses_as_code<S: AsRef<str>>(lines: &[S]) -> bool {
    let stripped: Vec<&str> = lines
        .iter()
        .map(|x| pytext::removeprefix(pytext::lstrip_chars(pytext::strip(x.as_ref()), "/"), " "))
        .collect();
    let body = stripped.join("\n");
    for wrapped in [false, true] {
        let source = if wrapped {
            format!("fn _f() {{\n{body}\n}}")
        } else {
            body.clone()
        };
        let Some(tree) = parse(source.as_bytes()) else {
            continue;
        };
        let root = tree.root_node();
        if root.has_error() || root.named_child_count() == 0 {
            continue;
        }
        let inner = if wrapped {
            named_children(root)[0].child_by_field_name("body")
        } else {
            Some(root)
        };
        if inner.is_some_and(|n| !statements(n).is_empty()) {
            return true;
        }
    }
    false
}
