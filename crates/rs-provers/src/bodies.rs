//! `rs/provers.py`'s body queries: one symbol's calls, macros, closures,
//! `unsafe` blocks and `?`s, walked once, and the readings off one call.

use super::*;

/// A call or macro invocation as a body query answers it. `src` is the
/// module's bytes: a tree-sitter node does not know its file, so every
/// reader of the call's text holds them (`rs/model.py:text`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsCall<'t> {
    /// the last path segment, or the method name
    pub name: String,
    /// as spelled
    pub path: String,
    pub node: Node<'t>,
    pub line: u32,
    pub src: &'t [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsClosure<'t> {
    /// blind digest of the closure's body
    pub digest: String,
    /// its content, the closure's own parameters renamed by position
    pub key: String,
    /// nodes in it: the "worth a name" floor
    pub size: usize,
    /// only names and calls: nothing in it to drift
    pub forwards: bool,
    pub node: Node<'t>,
    pub line: u32,
    pub src: &'t [u8],
}

/// One symbol's body, walked once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RsBody<'t> {
    pub calls: Vec<RsCall<'t>>,
    pub macros: Vec<RsCall<'t>>,
    pub unsafe_blocks: Vec<Node<'t>>,
    pub closures: Vec<RsClosure<'t>>,
    pub allows: Vec<String>,
    /// `?` operators: how a fallible body reports (#42)
    pub tries: u32,
}

impl<'t> RsProvers<'t> {
    /// Calls and macro invocations by name, `unsafe` blocks, closures with
    /// their blind digests, and the `#[allow]`s written inside the body.
    pub fn body(&self, qname: &str) -> &RsBody<'t> {
        match self.facts.symbols.get_index_of(qname) {
            Some(i) => self.bodies[i].get_or_init(|| self.build_body(qname)),
            None => self.no_body.get_or_init(RsBody::default),
        }
    }

    fn build_body(&self, qname: &str) -> RsBody<'t> {
        let Some(sym) = self.facts.symbols.get(qname) else {
            return RsBody::default();
        };
        let Some(node) = sym.node.child_by_field_name("body") else {
            return RsBody::default();
        };
        let src = self.facts.modules[&sym.module].bytes;
        let mut out = RsBody::default();
        // a nested fn is its own body
        for cur in descend(node, NESTED_FN) {
            match cur.kind() {
                "call_expression" => out.calls.push(call_of(cur, "function", src)),
                "macro_invocation" => out.macros.push(call_of(cur, "macro", src)),
                "unsafe_block" => out.unsafe_blocks.push(cur),
                "closure_expression" => out.closures.push(self.closure(cur, src)),
                "try_expression" => out.tries += 1,
                kind if has(ATTRS, kind) => out.allows.extend(allow_names(cur, src)),
                _ => {}
            }
        }
        out
    }

    /// The return type a `fn` declares, as written; empty where it declares
    /// none (#42 reads a test's `Result`).
    pub fn returns(&self, qname: &str) -> String {
        let Some(sym) = self.facts.symbols.get(qname) else {
            return String::new();
        };
        match sym.node.child_by_field_name("return_type") {
            Some(node) => text(node, self.facts.modules[&sym.module].bytes).into_owned(),
            None => String::new(),
        }
    }

    /// Does the site sit under an `if`/`match`/loop of its own function? A
    /// `panic!` that does is a verdict; one that does not stops the body
    /// whatever the code did (#42).
    pub fn guarded(&self, call: &RsCall<'_>) -> bool {
        ancestors(call.node).iter().any(|n| has(GUARDS, n.kind()))
    }

    /// A macro invocation's arguments as written. tree-sitter leaves macro
    /// tokens unparsed, so the split at its top-level commas is the one
    /// reading of what `assert_eq!` compares (#44).
    pub fn macro_args(&self, call: &RsCall<'_>) -> Vec<String> {
        let Some(tree) = children(call.node)
            .into_iter()
            .find(|c| c.kind() == "token_tree")
        else {
            return Vec::new();
        };
        let inner = children(tree);
        let mut args: Vec<Vec<String>> = vec![Vec::new()];
        // the outer delimiters are not tokens
        for child in inner.iter().take(inner.len().saturating_sub(1)).skip(1) {
            if child.kind() == "," {
                args.push(Vec::new());
            } else {
                args.last_mut()
                    .expect("the first bucket is always there")
                    .push(text(*child, call.src).into_owned());
            }
        }
        args.into_iter()
            .filter(|a| !a.is_empty())
            .map(|a| a.join(" "))
            .collect()
    }

    /// The seconds a call's one argument spells as a literal `Duration`
    /// (`Duration::from_millis(50)` -> 0.05); `None` for anything else (#47).
    pub fn constant_duration(&self, call: &RsCall<'_>) -> Option<f64> {
        let args = arg_nodes(call.node);
        let [only] = args[..] else { return None };
        let made = arg_nodes(only);
        let [literal] = made[..] else { return None };
        let scale = lookup(&DURATION_SCALE, &call_of(only, "function", call.src).name)?;
        number(literal, call.src).map(|value| value * scale)
    }

    fn closure(&self, node: Node<'t>, src: &'t [u8]) -> RsClosure<'t> {
        let body = named_children(node).last().copied().unwrap_or(node);
        RsClosure {
            digest: digest(&self.shape(body, src)),
            key: digest(&self.content(body, &closure_params(node, src), src)),
            size: self.size(body),
            forwards: forwards_only(body),
            node,
            line: node.start_position().row as u32 + 1,
            src,
        }
    }
}

fn call_of<'t>(node: Node<'t>, field: &str, src: &'t [u8]) -> RsCall<'t> {
    let path = match call_target(node, field) {
        Some(target) => text(target, src).into_owned(),
        None => String::new(),
    };
    let name = path
        .replace('.', "::")
        .rsplit("::")
        .next()
        .unwrap_or_default()
        .to_string();
    RsCall {
        name,
        path,
        node,
        line: node.start_position().row as u32 + 1,
        src,
    }
}
