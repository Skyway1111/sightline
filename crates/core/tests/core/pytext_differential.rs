//! Every row of `data/pytext.json` is one CPython 3.14 answer, and this
//! file re-checks each one against `pyjson` and `pytext`.

use std::collections::HashMap;

use serde_json::{Value, json};
use sightline_core::{pyjson, pytext};

const DATA: &str = include_str!("../data/pytext.json");

#[test]
fn every_row_of_the_cpython_differential_passes() {
    let rows: Vec<Value> = serde_json::from_str(DATA).expect("the generator writes valid JSON");
    assert!(
        rows.len() >= 300,
        "the differential holds {} rows",
        rows.len()
    );
    // The generator ran on Windows, where CPython's `fnmatch` lowercases and
    // swaps the separator through `os.path.normcase`. On every other platform
    // `fnmatch` is `fnmatchcase`, so a `fnmatch` row wants the answer the
    // `fnmatchcase` row with the same arguments holds.
    let cased: HashMap<String, &Value> = rows
        .iter()
        .filter(|row| row["fn"] == "fnmatchcase")
        .map(|row| (row["args"].to_string(), &row["expected"]))
        .collect();
    let failures: Vec<String> = rows
        .iter()
        .filter_map(|row| {
            let name = row["fn"].as_str().expect("every row names a function");
            let args = row["args"]
                .as_array()
                .expect("every row holds its arguments");
            let want = if name == "fnmatch" && !cfg!(windows) {
                cased
                    .get(&row["args"].to_string())
                    .copied()
                    .expect("every fnmatch row has an fnmatchcase row beside it")
            } else {
                &row["expected"]
            };
            let got = answer(name, args);
            (got != *want).then(|| format!("{name}({args:?}) -> {got} want {want}"))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} rows differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn answer(name: &str, args: &[Value]) -> Value {
    let s = |i: usize| args[i].as_str().expect("a string argument");
    let n = |i: usize| args[i].as_i64().expect("an integer argument");
    let f = |i: usize| {
        s(i).parse::<f64>()
            .expect("a float spelled as a decimal string")
    };
    match name {
        "is_py_space" => json!(pytext::is_py_space(
            s(0).chars().next().expect("one character")
        )),
        "split" => json!(pytext::split(s(0))),
        "splitlines" => json!(pytext::splitlines(s(0))),
        "strip" => json!(pytext::strip(s(0))),
        "lstrip" => json!(pytext::lstrip(s(0))),
        "rstrip" => json!(pytext::rstrip(s(0))),
        "strip_chars" => json!(pytext::strip_chars(s(0), s(1))),
        "lstrip_chars" => json!(pytext::lstrip_chars(s(0), s(1))),
        "rstrip_chars" => json!(pytext::rstrip_chars(s(0), s(1))),
        "lower" => json!(pytext::lower(s(0))),
        "is_identifier" => json!(pytext::is_identifier(s(0))),
        "is_upper_first" => json!(pytext::is_upper_first(s(0))),
        "is_digit" => json!(pytext::is_digit(s(0))),
        "removeprefix" => json!(pytext::removeprefix(s(0), s(1))),
        "partition" => {
            let (a, b, c) = pytext::partition(s(0), s(1));
            json!([a, b, c])
        }
        "rpartition" => {
            let (a, b, c) = pytext::rpartition(s(0), s(1));
            json!([a, b, c])
        }
        "cleandoc" => json!(pytext::cleandoc(s(0))),
        "dedent" => json!(pytext::dedent(s(0))),
        "expandtabs" => json!(pytext::expandtabs(s(0), n(1) as usize)),
        "repr_float" => json!(pytext::repr_float(f(0))),
        "format_g" => json!(pytext::format_g(f(0))),
        "repr_int" => json!(pytext::repr_int(n(0))),
        "repr_str" => json!(pytext::repr_str(s(0))),
        "repr_bytes" => {
            let data: Vec<u8> = args[0]
                .as_array()
                .expect("a byte list")
                .iter()
                .map(|b| b.as_u64().expect("a byte") as u8)
                .collect();
            json!(pytext::repr_bytes(&data))
        }
        "repr_str_list" => {
            let items: Vec<&str> = args[0]
                .as_array()
                .expect("a string list")
                .iter()
                .map(|v| v.as_str().expect("a string"))
                .collect();
            json!(pytext::repr_str_list(&items))
        }
        "fnmatch" => json!(pytext::fnmatch(s(0), s(1))),
        "fnmatchcase" => json!(pytext::fnmatchcase(s(0), s(1))),
        "dumps" => json!(pyjson::dumps(&args[0])),
        other => panic!("the differential names an unported function: {other}"),
    }
}
