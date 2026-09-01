//! Every row of `data/astutil.json` is CPython's answer for one `astutil.py`
//! helper on one source. The generator is
//! `../sightline-phase2/scratch/facts-ast/gen_astutil.py`.

use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_python_parser::parse_module;
use serde_json::{Value, json};
use sightline_core::pytext::repr_float;
use sightline_py_facts::astutil::{
    self, CHAIN, all_arg_names, attr_on, chain_root, fn_defaults, is_mutable_init, literal_affixes,
    mentions, name_tokens, subnodes,
};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::literal::{Literal, literal};
use sightline_py_facts::unparse;

const DATA: &str = include_str!("../data/astutil.json");

fn sorted_strings<'a>(items: impl IntoIterator<Item = &'a str>) -> Value {
    let mut out: Vec<&str> = items.into_iter().collect();
    out.sort_unstable();
    json!(out)
}

fn encode(value: &Literal) -> Value {
    match value {
        Literal::Computed => json!({"kind": "computed"}),
        Literal::Bool(b) => json!({"kind": "bool", "value": b}),
        Literal::Int(i) => json!({"kind": "int", "value": i.to_string()}),
        Literal::Float(f) => json!({"kind": "float", "value": repr_float(*f)}),
        Literal::Str(s) => json!({"kind": "str", "value": s}),
        Literal::Bytes(b) => json!({"kind": "bytes", "value": b}),
        Literal::None => json!({"kind": "none"}),
        Literal::Tuple(v) => json!({"kind": "tuple", "value": members(v)}),
        Literal::List(v) => json!({"kind": "list", "value": members(v)}),
        Literal::Set(v) => {
            // A Python set has no order; the generator sorts its members the
            // same way.
            let mut items = members(v);
            items.sort_by_key(|m| serde_json::to_string(m).expect("a json value"));
            json!({"kind": "set", "value": items})
        }
        Literal::Dict(v) => json!({
            "kind": "dict",
            "value": v.iter().map(|(k, x)| json!([encode(k), encode(x)])).collect::<Vec<_>>(),
        }),
    }
}

fn members(values: &[Literal]) -> Vec<Value> {
    values.iter().map(encode).collect()
}

/// The `Cn` the row's source is: one expression statement, or one def.
enum Subject<'a> {
    Expression(&'a Expr),
    Def(&'a StmtFunctionDef),
}

impl<'a> Subject<'a> {
    fn of(suite: &'a [Stmt], source: &str) -> Self {
        match suite {
            [Stmt::Expr(e)] => Subject::Expression(&e.value),
            [Stmt::FunctionDef(f)] => Subject::Def(f),
            _ => panic!("{source:?} is neither one expression nor one def"),
        }
    }

    fn expression(&self, source: &str) -> &'a Expr {
        match self {
            Subject::Expression(e) => e,
            Subject::Def(_) => panic!("{source:?} is a def, not an expression"),
        }
    }

    fn definition(&self, source: &str) -> &'a StmtFunctionDef {
        match self {
            Subject::Def(f) => f,
            Subject::Expression(_) => panic!("{source:?} is an expression, not a def"),
        }
    }

    fn node(&self, suite: &'a [Stmt]) -> Cn<'a> {
        match self {
            Subject::Expression(e) => Cn::Expr(e),
            Subject::Def(_) => Cn::Stmt(&suite[0]),
        }
    }
}

/// The serde_json map this test compares through is key-sorted, which is what
/// the generator's `json.dumps(sort_keys=True)` writes.
#[test]
fn the_json_map_sorts_its_keys() {
    let text = serde_json::to_string(&json!({"value": 1, "kind": "int"})).expect("a json value");
    assert_eq!(text, r#"{"kind":"int","value":1}"#);
}

#[test]
fn every_helper_answers_what_cpython_answered() {
    let doc: Value = serde_json::from_str(DATA).expect("the fixture parses");
    let rows = doc["rows"].as_array().expect("a row list");
    assert!(rows.len() >= 200, "{} rows is under the bar", rows.len());
    let mut checked = 0;
    for row in rows {
        let name = row["fn"].as_str().expect("a helper name");
        let source = row["source"].as_str().expect("a source");
        let want = &row["expected"];
        if name == "name_tokens" {
            let tokens = name_tokens(source);
            let got = sorted_strings(tokens.iter().map(String::as_str));
            assert_eq!(&got, want, "name_tokens on {source:?}");
            checked += 1;
            continue;
        }
        let parsed = parse_module(source).expect("the source parses");
        let suite = parsed.suite();
        let subject = Subject::of(suite, source);
        let got = match name {
            "literal_affixes" => match literal_affixes(subject.expression(source)) {
                Some((prefix, suffix)) => json!([prefix, suffix]),
                None => Value::Null,
            },
            "chain_root" => json!(chain_root(subject.expression(source), &CHAIN)),
            "is_mutable_init" => json!(is_mutable_init(Some(subject.expression(source)))),
            "literal" => encode(&literal(subject.expression(source))),
            "attr_on" => {
                let names: Vec<&str> = row["arg"]
                    .as_array()
                    .expect("a name list")
                    .iter()
                    .map(|n| n.as_str().expect("a name"))
                    .collect();
                json!(attr_on(subject.expression(source), &names))
            }
            "subnodes" => json!(
                subnodes(subject.node(suite), |_| true)
                    .iter()
                    .map(|n| n.kind().name())
                    .collect::<Vec<_>>()
            ),
            "fn_defaults" => json!(
                fn_defaults(subject.definition(source))
                    .iter()
                    .map(|(a, d)| json!([a.name.as_str(), unparse::expr(d)]))
                    .collect::<Vec<_>>()
            ),
            "all_arg_names" => {
                let params = &subject.definition(source).parameters;
                sorted_strings(all_arg_names(Some(params)))
            }
            "mentions" => json!(mentions(
                subject.node(suite),
                row["arg"].as_str().expect("a name")
            )),
            other => panic!("no helper named {other}"),
        };
        assert_eq!(&got, want, "{name} on {source:?}");
        checked += 1;
    }
    assert_eq!(checked, rows.len());
}

/// R12 stops where `ast.literal_eval` keeps going: an `Ellipsis`, a complex
/// number and an integer past `u64::MAX` are `Computed`, so the fixture holds
/// no row for them.
#[test]
fn r12_leaves_three_values_computed() {
    for source in [
        "...",
        "1j",
        "99999999999999999999999",
        "-99999999999999999999999",
    ] {
        let parsed = parse_module(source).expect("the source parses");
        let [Stmt::Expr(e)] = &parsed.suite()[..] else {
            panic!("{source:?} is not one expression")
        };
        assert_eq!(literal(&e.value), Literal::Computed, "{source}");
    }
}

/// `fn_body` drops a docstring and nothing else (R11).
#[test]
fn fn_body_drops_only_a_string_docstring() {
    for (source, kept) in [
        ("def f():\n    'doc'\n    return 1\n", 1),
        ("def f():\n    b'doc'\n    return 1\n", 2),
        ("def f():\n    f'doc'\n    return 1\n", 2),
        ("def f():\n    return 1\n", 1),
        ("def f():\n    'a' 'b'\n", 0),
    ] {
        let parsed = parse_module(source).expect("the source parses");
        let [Stmt::FunctionDef(f)] = &parsed.suite()[..] else {
            panic!("{source:?} is not one def")
        };
        assert_eq!(astutil::fn_body(&f.body).len(), kept, "{source}");
    }
}

/// `document_order` is stable and puts an ancestor ahead of its descendants.
#[test]
fn document_order_puts_ancestors_first() {
    let mut rows = [(2, 0, 2, 9), (1, 0, 3, 0), (1, 0, 1, 4), (1, 4, 1, 8)];
    astutil::document_order(&mut rows, |r| *r);
    assert_eq!(
        rows,
        [(1, 0, 3, 0), (1, 0, 1, 4), (1, 4, 1, 8), (2, 0, 2, 9)]
    );
}

/// `line_span` of a synthesized node, which has no end line.
#[test]
fn line_span_ends_a_synthesized_node_where_it_starts() {
    assert_eq!(astutil::line_span((7, 0)), (7, 7));
    assert_eq!(astutil::line_span((7, 9)), (7, 9));
}
