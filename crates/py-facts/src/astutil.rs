//! Pure-AST predicates shared by rules and provers (no facts, no opinions
//! about findings). The port of `astutil.py` over `ruff_python_ast` and
//! `order::Cn`; every walk here is `order::children`, CPython's `_fields`
//! order (R5).

use std::collections::{HashSet, VecDeque};
use std::sync::LazyLock;

use regex::Regex;
use ruff_python_ast::{
    Expr, ExprFString, FStringPart, InterpolatedStringElement, Operator, Parameter,
    ParameterWithDefault, Parameters, Stmt, StmtFunctionDef,
};

use crate::cn::Cn;
use crate::kinds::Kind;
use crate::order::{self};

/// The receiver is the method's own object, never an argument a caller chose.
pub const RECEIVERS: [&str; 2] = ["self", "cls"];

/// `(first, last)` line of a span-table row; a synthesized node, which has no
/// end line, ends where it starts.
pub fn line_span((line, end_line): (u32, u32)) -> (u32, u32) {
    (line, if end_line == 0 { line } else { end_line })
}

static TOKEN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Z]?[a-z0-9]+|[A-Z]+").expect("a literal pattern"));

/// Lower-cased word tokens of an identifier or code line (snake + camel).
pub fn name_tokens(name: &str) -> HashSet<String> {
    TOKEN_RE
        .find_iter(name)
        .map(|m| m.as_str().to_lowercase())
        .collect()
}

/// A bare call as a statement (`f()`), the shape a result is dropped in.
pub fn is_call_stmt(st: &Stmt) -> bool {
    matches!(st, Stmt::Expr(e) if matches!(&*e.value, Expr::Call(_)))
}

pub fn is_const_str(node: Option<&Expr>) -> bool {
    matches!(node, Some(Expr::StringLiteral(_)))
}

// A name built around a variable: the constant text at either end is the
// pattern's evidence (`f"on_{e}"`, `"on_" + e`, `"on_%s" % e`,
// `"on_{}".format(e)`).
static HOLE_PERCENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"%(?:\(\w+\))?[-#0 +]*(?:\*|\d+)?(?:\.(?:\*|\d+))?[hlL]?[diouxXeEfFgGcrsa%]")
        .expect("a literal pattern")
});
static HOLE_FORMAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{[^{}]*\}").expect("a literal pattern"));

/// CPython's `JoinedStr.values`: `Some(text)` is a `Constant`, folded over the
/// run of adjacent literal chunks the way `order::children` folds it; `None`
/// is a `FormattedValue`.
fn fstring_values(node: &ExprFString) -> Vec<Option<String>> {
    let mut out: Vec<Option<String>> = Vec::new();
    let mut open: Option<String> = None;
    fn chunk(text: &str, open: &mut Option<String>) {
        match open {
            Some(run) => run.push_str(text),
            slot => *slot = Some(text.to_string()),
        }
    }
    for part in node.value.iter() {
        match part {
            FStringPart::Literal(s) => chunk(&s.value, &mut open),
            FStringPart::FString(f) => {
                for element in f.elements.iter() {
                    match element {
                        InterpolatedStringElement::Literal(l) => chunk(&l.value, &mut open),
                        InterpolatedStringElement::Interpolation(i) => {
                            if let Some(debug) = &i.debug_text {
                                chunk(debug.as_str(), &mut open);
                            }
                            out.extend(open.take().map(Some));
                            out.push(None);
                        }
                    }
                }
            }
        }
    }
    out.extend(open.take().map(Some));
    out
}

/// `(prefix, suffix)` a built dispatch name is wrapped in; `None` without
/// constant text at either end. #32 keeps the matching names live and the
/// closed world reads them as reflected: one reading of a built name.
pub fn literal_affixes(arg: &Expr) -> Option<(String, String)> {
    let values = match arg {
        Expr::FString(f) => fstring_values(f),
        _ => Vec::new(),
    };
    let (prefix, suffix) = match arg {
        // An f-string with no parts (`f""`) falls through, as CPython's empty
        // `JoinedStr.values` does.
        Expr::FString(_) if !values.is_empty() => {
            let head = values[0].clone().unwrap_or_default();
            let tail = match values.len() {
                1 => String::new(),
                n => values[n - 1].clone().unwrap_or_default(),
            };
            (head, tail)
        }
        Expr::BinOp(add) if add.op == Operator::Add => {
            let mut left: &Expr = arg;
            while let Expr::BinOp(b) = left {
                if b.op != Operator::Add {
                    break;
                }
                left = &b.left;
            }
            let mut right: &Expr = arg;
            while let Expr::BinOp(b) = right {
                if b.op != Operator::Add {
                    break;
                }
                right = &b.right;
            }
            (const_str(left), const_str(right))
        }
        _ => {
            let (template, percent) = match arg {
                Expr::BinOp(b) if b.op == Operator::Mod => (Some(&*b.left), true),
                Expr::Call(c) => match &*c.func {
                    Expr::Attribute(a) if a.attr.as_str() == "format" => (Some(&*a.value), false),
                    _ => (None, false),
                },
                _ => (None, false),
            };
            let Some(Expr::StringLiteral(s)) = template else {
                return None;
            };
            let hole = if percent { &HOLE_PERCENT } else { &HOLE_FORMAT };
            let parts: Vec<&str> = hole.split(s.value.to_str()).collect();
            let tail = match parts.len() {
                1 => String::new(),
                n => parts[n - 1].to_string(),
            };
            (parts[0].to_string(), tail)
        }
    };
    (!prefix.is_empty() || !suffix.is_empty()).then_some((prefix, suffix))
}

fn const_str(node: &Expr) -> String {
    match node {
        Expr::StringLiteral(s) => s.value.to_str().to_string(),
        _ => String::new(),
    }
}

/// Body without a leading docstring (R11: a leading `Expr` whose value is a
/// string literal, not bytes, not an f-string).
pub fn fn_body(body: &[Stmt]) -> &[Stmt] {
    match body.first() {
        Some(Stmt::Expr(e)) if matches!(&*e.value, Expr::StringLiteral(_)) => &body[1..],
        _ => body,
    }
}

fn pos_params(params: &Parameters) -> Vec<&ParameterWithDefault> {
    params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .collect()
}

/// Declared args (positional + kw-only), self/cls included.
pub fn fn_args(fn_def: &StmtFunctionDef) -> Vec<&Parameter> {
    let p = &fn_def.parameters;
    p.posonlyargs
        .iter()
        .chain(p.args.iter())
        .chain(p.kwonlyargs.iter())
        .map(|a| &a.parameter)
        .collect()
}

pub fn without_receiver<'a, 'p>(args: &'a [&'p Parameter]) -> &'a [&'p Parameter] {
    match args.first() {
        Some(first) if RECEIVERS.contains(&first.name.as_str()) => &args[1..],
        _ => args,
    }
}

/// Positional args with a leading self/cls dropped.
pub fn fn_pos_args(fn_def: &StmtFunctionDef) -> Vec<&Parameter> {
    let pos: Vec<&Parameter> = pos_params(&fn_def.parameters)
        .into_iter()
        .map(|a| &a.parameter)
        .collect();
    without_receiver(&pos).to_vec()
}

/// `(arg, default)` pairs: positional tail plus defaulted kw-onlys. Defaults
/// align against the full positional list, receiver included.
pub fn fn_defaults(fn_def: &StmtFunctionDef) -> Vec<(&Parameter, &Expr)> {
    let p = &fn_def.parameters;
    pos_params(p)
        .into_iter()
        .chain(p.kwonlyargs.iter())
        .filter_map(|a| a.default.as_ref().map(|d| (&a.parameter, &**d)))
        .collect()
}

/// Every name the signature binds, `*args`/`**kwargs` included. A bare
/// `lambda: 0` has no `Parameters` node in ruff, so the caller passes `None`.
pub fn all_arg_names(params: Option<&Parameters>) -> HashSet<&str> {
    let Some(p) = params else {
        return HashSet::new();
    };
    p.posonlyargs
        .iter()
        .chain(p.args.iter())
        .chain(p.kwonlyargs.iter())
        .map(|a| &a.parameter)
        .chain(p.vararg.as_deref())
        .chain(p.kwarg.as_deref())
        .map(|a| a.name.as_str())
        .collect()
}

pub fn fn_params(fn_def: &StmtFunctionDef) -> Vec<&str> {
    fn_args(fn_def).iter().map(|a| a.name.as_str()).collect()
}

/// The chain kinds `chain_root` walks through by default.
pub const CHAIN: [Kind; 3] = [Kind::Attribute, Kind::Subscript, Kind::Call];

/// Root `Name` id of a receiver chain (`a.b[0].c(x)` -> `a`), else `None`;
/// `await` is transparent.
pub fn chain_root<'a>(node: &'a Expr, kinds: &[Kind]) -> Option<&'a str> {
    let mut node = node;
    loop {
        node = match node {
            Expr::Attribute(a) if kinds.contains(&Kind::Attribute) => &a.value,
            Expr::Subscript(s) if kinds.contains(&Kind::Subscript) => &s.value,
            Expr::Call(c) if kinds.contains(&Kind::Call) => &c.func,
            Expr::Await(a) => &a.value,
            Expr::Name(n) => return Some(n.id.as_str()),
            _ => return None,
        };
    }
}

/// The attr of `name.attr` for one of `names` (`self.seen` -> `"seen"`), else
/// `None`: a one-hop reference off a bare name.
pub fn attr_on<'a>(node: &'a Expr, names: &[&str]) -> Option<&'a str> {
    let Expr::Attribute(a) = node else {
        return None;
    };
    let Expr::Name(base) = &*a.value else {
        return None;
    };
    names.contains(&base.id.as_str()).then(|| a.attr.as_str())
}

/// `ast.walk`: the node and its descendants, breadth first with the root
/// first, each node's children in CPython's `_fields` order.
pub struct Walk<'a> {
    queue: VecDeque<Cn<'a>>,
    kids: Vec<Cn<'a>>,
}

impl<'a> Iterator for Walk<'a> {
    type Item = Cn<'a>;

    fn next(&mut self) -> Option<Cn<'a>> {
        let node = self.queue.pop_front()?;
        self.kids.clear();
        order::children(node, &mut self.kids);
        self.queue.extend(self.kids.iter().copied());
        Some(node)
    }
}

pub fn walk(root: Cn<'_>) -> Walk<'_> {
    Walk {
        queue: VecDeque::from([root]),
        kids: Vec::new(),
    }
}

/// One expression or statement and its descendants, `keep` only.
pub fn subnodes<'a>(root: Cn<'a>, keep: impl Fn(Kind) -> bool) -> Vec<Cn<'a>> {
    walk(root).filter(|n| keep(n.kind())).collect()
}

/// The facts index is a bucket per kind per scope: one document order over
/// several kinds, ancestors ahead of their descendants. `key` is the caller's
/// `(line, col, end_line, end_col)`; the sort is stable.
pub fn document_order<T>(nodes: &mut [T], key: impl Fn(&T) -> (u32, u32, u32, u32)) {
    nodes.sort_by(|a, b| {
        let (a_line, a_col, a_end_line, a_end_col) = key(a);
        let (b_line, b_col, b_end_line, b_end_col) = key(b);
        (a_line, a_col)
            .cmp(&(b_line, b_col))
            .then((b_end_line, b_end_col).cmp(&(a_end_line, a_end_col)))
    });
}

const MUTABLE_CALLS: [&str; 7] = [
    "list",
    "dict",
    "set",
    "defaultdict",
    "deque",
    "Counter",
    "OrderedDict",
];

pub fn is_mutable_init(value: Option<&Expr>) -> bool {
    match value {
        Some(
            Expr::List(_)
            | Expr::Dict(_)
            | Expr::Set(_)
            | Expr::ListComp(_)
            | Expr::DictComp(_)
            | Expr::SetComp(_),
        ) => true,
        Some(Expr::Call(c)) => {
            matches!(&*c.func, Expr::Name(n) if MUTABLE_CALLS.contains(&n.id.as_str()))
        }
        _ => false,
    }
}

pub fn mentions(node: Cn<'_>, name: &str) -> bool {
    walk(node).any(|n| matches!(n, Cn::Expr(Expr::Name(x)) if x.id.as_str() == name))
}

#[cfg(test)]
mod tests {
    use ruff_python_parser::parse_module;

    use super::*;
    use crate::unparse;

    fn one_def(source: &str) -> StmtFunctionDef {
        let parsed = parse_module(source).expect("the fixture parses");
        match &parsed.suite()[0] {
            Stmt::FunctionDef(f) => f.clone(),
            other => panic!("{other:?} is not a def"),
        }
    }

    fn pairs(fn_def: &StmtFunctionDef) -> Vec<(String, String)> {
        fn_defaults(fn_def)
            .into_iter()
            .map(|(a, d)| (a.name.to_string(), unparse::expr(d)))
            .collect()
    }

    fn names(fn_def: &StmtFunctionDef) -> Vec<String> {
        let mut out: Vec<String> = all_arg_names(Some(&fn_def.parameters))
            .into_iter()
            .map(str::to_string)
            .collect();
        out.sort();
        out
    }

    #[test]
    fn fn_defaults_pairs_positional_tail_and_kwonly() {
        let fn_def = one_def("def outer(a=1, *, b=None): pass");
        assert_eq!(
            pairs(&fn_def),
            [
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "None".to_string()),
            ]
        );
    }

    #[test]
    fn all_arg_names_and_receiver_drop() {
        let fn_def = one_def("def f(a, /, b, *args, c, **kwargs): pass");
        assert_eq!(names(&fn_def), ["a", "args", "b", "c", "kwargs"]);

        let method = one_def("def f(self, value): pass");
        let params: Vec<&Parameter> = method
            .parameters
            .posonlyargs
            .iter()
            .chain(method.parameters.args.iter())
            .map(|a| &a.parameter)
            .collect();
        let kept: Vec<&str> = without_receiver(&params)
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        assert_eq!(kept, ["value"]);
    }

    /// A defaulted receiver must not shift default alignment onto later
    /// params: `fn_defaults` pairs before any receiver is dropped.
    #[test]
    fn defaults_align_before_the_receiver_is_dropped() {
        let method = one_def("def method(self=None, value=1): pass");
        assert_eq!(
            pairs(&method),
            [
                ("self".to_string(), "None".to_string()),
                ("value".to_string(), "1".to_string()),
            ]
        );
        let kept: Vec<&str> = fn_pos_args(&method)
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        assert_eq!(kept, ["value"]);
    }

    /// `*self` binds a name but is no receiver: nothing positional to drop.
    #[test]
    fn a_variadic_named_self_is_not_a_receiver() {
        let fn_def = one_def("def f(*self): pass");
        assert!(fn_pos_args(&fn_def).is_empty());
        assert_eq!(names(&fn_def), ["self"]);
    }
}
