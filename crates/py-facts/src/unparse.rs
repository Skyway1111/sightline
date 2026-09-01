//! R10's wrapper: the only place a gap between `ruff_python_codegen`'s
//! `Mode::AstUnparse` and CPython's `ast.unparse` is patched.
//!
//! Each patch names the `_ast_unparse.py` rule it reproduces (CPython 3.14).
//! A patch rewrites the cloned AST, never the emitted message: the site that
//! needs the CPython spelling gets a node whose verbatim rendering is it.

use ruff_python_ast::str_prefix::{ByteStringPrefix, StringLiteralPrefix};
use ruff_python_ast::visitor::transformer::{
    Transformer, walk_comprehension, walk_expr, walk_stmt,
};
use ruff_python_ast::{
    AtomicNodeIndex, BytesLiteral, BytesLiteralFlags, BytesLiteralValue, Comprehension, Expr,
    ExprBytesLiteral, ExprContext, ExprList, ExprName, ExprStringLiteral, Stmt, StringLiteral,
    StringLiteralFlags, StringLiteralValue, name::Name,
};
use ruff_python_codegen::{Generator, Indentation, Mode};
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;

fn emitter<'a>(indent: &'a Indentation) -> Generator<'a> {
    // `LineEnding::default()` is CrLf on Windows; ast.unparse always emits Lf.
    Generator::new(indent, LineEnding::Lf).with_mode(Mode::AstUnparse)
}

fn raw_expr(node: &Expr) -> String {
    emitter(&Indentation::default()).expr(node)
}

fn raw_stmt(node: &Stmt) -> String {
    emitter(&Indentation::default()).stmt(node)
}

/// A node that unparses to `text` and nothing else.
fn verbatim(text: String) -> Expr {
    Expr::Name(ExprName {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        id: Name::new(text),
        ctx: ExprContext::Load,
    })
}

/// The nodes rendered as the elements of a one-element list, brackets
/// stripped. `Generator::expr` starts at `precedence::MIN`, where nothing
/// groups; a list element is rendered at `precedence::COMMA`, whose grouping
/// is what CPython's default `_Precedence.TEST` produces (tuple, yield,
/// yield-from and walrus group; lambda, conditional and boolean do not).
fn at_comma(elts: &[Expr]) -> String {
    let list = Expr::List(ExprList {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        elts: elts.to_vec(),
        ctx: ExprContext::Load,
    });
    let rendered = raw_expr(&list);
    rendered[1..rendered.len() - 1].to_string()
}

/// `_ast_unparse.py:items_view`: comma-joined, with a trailing comma when the
/// sequence is one element.
fn items_view(elts: &[Expr]) -> String {
    let inner = at_comma(elts);
    if elts.len() == 1 {
        format!("{inner},")
    } else {
        inner
    }
}

/// P3. `visit_Constant` writes `repr(value)`: the source's triple quotes, raw
/// prefix and implicit concatenation are all gone, and only a `u` prefix
/// survives. The generator re-uses the literal's own flags and parts.
fn respell_string(node: &ExprStringLiteral) -> Expr {
    let prefix = if node.value.is_unicode() {
        StringLiteralPrefix::Unicode
    } else {
        StringLiteralPrefix::Empty
    };
    Expr::StringLiteral(ExprStringLiteral {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        value: StringLiteralValue::single(StringLiteral {
            range: TextRange::default(),
            node_index: AtomicNodeIndex::NONE,
            value: node.value.to_str().into(),
            flags: StringLiteralFlags::empty().with_prefix(prefix),
        }),
    })
}

fn respell_bytes(node: &ExprBytesLiteral) -> Expr {
    Expr::BytesLiteral(ExprBytesLiteral {
        node_index: AtomicNodeIndex::NONE,
        range: TextRange::default(),
        value: BytesLiteralValue::single(BytesLiteral {
            range: TextRange::default(),
            node_index: AtomicNodeIndex::NONE,
            value: node.value.bytes().collect::<Vec<u8>>().into(),
            flags: BytesLiteralFlags::empty().with_prefix(ByteStringPrefix::Regular),
        }),
    })
}

struct Patch;

impl Transformer for Patch {
    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr); // children first: a patch renders patched text
        match expr {
            // P1. `visit_Subscript`: a non-empty tuple slice loses its
            // parentheses. The generator prints `d[(str, int)]`.
            Expr::Subscript(sub) => {
                if let Expr::Tuple(tuple) = sub.slice.as_ref()
                    && !tuple.elts.is_empty()
                {
                    *sub.slice = verbatim(items_view(&tuple.elts));
                }
            }
            // P2. `visit_GeneratorExp` always parenthesizes, and `visit_Call`
            // has no sole-argument case: CPython prints `f((x for x in y))`.
            // The generator drops the inner pair.
            Expr::Call(call) => {
                if call.arguments.keywords.is_empty()
                    && call.arguments.args.len() == 1
                    && matches!(call.arguments.args[0], Expr::Generator(_))
                {
                    let inner = raw_expr(&call.arguments.args[0]);
                    call.arguments.args[0] = verbatim(inner);
                }
            }
            Expr::StringLiteral(s) => *expr = respell_string(s),
            Expr::BytesLiteral(b) => *expr = respell_bytes(b),
            _ => {}
        }
    }

    // P4. Every binding target is traversed at `_Precedence.TUPLE`, where a
    // tuple loses its parentheses: `for k, v in ys`, `a, b = z`. The
    // generator groups them (`COMPREHENSION_TARGET`, `COMMA`).
    fn visit_comprehension(&self, comprehension: &mut Comprehension) {
        walk_comprehension(self, comprehension);
        untuple(&mut comprehension.target);
    }

    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
        match stmt {
            // P5. An assignment's value is traversed at the unparser's default
            // `_Precedence.TEST`, above `YIELD`: `b = (yield)`. The generator's
            // `ASSIGN` and `AUG_ASSIGN` levels sit below it.
            Stmt::Assign(node) => {
                for target in &mut node.targets {
                    untuple(target);
                }
                *node.value = at_test(&node.value);
            }
            Stmt::AugAssign(node) => *node.value = at_test(&node.value),
            Stmt::AnnAssign(node) => {
                if let Some(value) = &mut node.value {
                    **value = at_test(value);
                }
            }
            Stmt::For(node) => untuple(&mut node.target),
            _ => {}
        }
    }
}

/// A target tuple as CPython writes it at `_Precedence.TUPLE`.
fn untuple(target: &mut Expr) {
    if let Expr::Tuple(tuple) = target
        && !tuple.elts.is_empty()
    {
        *target = verbatim(items_view(&tuple.elts));
    }
}

fn at_test(node: &Expr) -> Expr {
    verbatim(at_comma(std::slice::from_ref(node)))
}

pub fn expr(node: &Expr) -> String {
    let mut owned = node.clone();
    Patch.visit_expr(&mut owned);
    // P6. A root expression is traversed at the unparser's default
    // `_Precedence.TEST`; `Generator::expr` starts at `precedence::MIN`, where
    // a tuple, a yield and a walrus all lose their parentheses.
    at_comma(std::slice::from_ref(&owned))
}

pub fn stmt(node: &Stmt) -> String {
    let mut owned = node.clone();
    Patch.visit_stmt(&mut owned);
    raw_stmt(&owned)
}

#[cfg(test)]
mod tests {
    use ruff_python_parser::parse_module;

    fn one_expr(source: &str) -> String {
        let parsed = parse_module(source).expect("parses");
        let ruff_python_ast::Stmt::Expr(e) = &parsed.suite()[0] else {
            panic!("{source} is not one expression statement");
        };
        super::expr(&e.value)
    }

    fn one_stmt(source: &str) -> String {
        let parsed = parse_module(source).expect("parses");
        super::stmt(&parsed.suite()[0])
    }

    #[test]
    fn p1_a_tuple_slice_loses_its_parentheses() {
        assert_eq!(one_expr("dict[str, int]"), "dict[str, int]");
        assert_eq!(one_expr("d[x,]"), "d[x,]");
        assert_eq!(one_expr("d[()]"), "d[()]");
    }

    #[test]
    fn p2_a_sole_argument_generator_keeps_one_pair() {
        assert_eq!(one_expr("f(x for x in y)"), "f((x for x in y))");
        assert_eq!(one_expr("f((x for x in y), z)"), "f((x for x in y), z)");
    }

    #[test]
    fn p3_a_constant_is_respelled_as_repr() {
        assert_eq!(one_expr("'''d'''"), "'d'");
        assert_eq!(one_expr("rb'x'"), "b'x'");
        assert_eq!(one_expr("'a' 'b'"), "'ab'");
        assert_eq!(one_expr("u'a'"), "u'a'");
    }

    #[test]
    fn p4_a_target_tuple_is_bare() {
        assert_eq!(
            one_stmt("for k, v in ys:\n    pass\n"),
            "for k, v in ys:\n    pass"
        );
        assert_eq!(one_stmt("a, b = z\n"), "a, b = z");
        assert_eq!(one_expr("[x for k, v in ys]"), "[x for k, v in ys]");
    }

    #[test]
    fn p5_an_assigned_yield_is_parenthesized() {
        assert_eq!(one_stmt("b = yield\n"), "b = (yield)");
        assert_eq!(one_stmt("b += yield x\n"), "b += (yield x)");
        assert_eq!(one_stmt("b: int = yield\n"), "b: int = (yield)");
    }

    #[test]
    fn p6_a_root_expression_renders_at_test() {
        assert_eq!(one_expr("1, 2"), "(1, 2)");
        assert_eq!(one_expr("(yield)"), "(yield)");
        assert_eq!(one_expr("lambda: 0"), "lambda: 0");
        assert_eq!(one_expr("a if b else c"), "a if b else c");
    }

    /// `LineEnding::default()` is CrLf on Windows; every emitted line is Lf.
    #[test]
    fn a_multi_line_statement_ends_its_lines_with_lf() {
        let text = one_stmt("if a:\n    pass\n");
        assert_eq!(text, "if a:\n    pass");
        assert!(!text.contains('\r'));
    }
}
