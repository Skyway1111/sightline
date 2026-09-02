//! `json.dumps(obj, indent=2, sort_keys=True)` byte for byte:
//! keys sorted by code point at every level, one item per line, non-ASCII as
//! `\uXXXX` with surrogate pairs, floats as CPython `repr`. `serde_json`'s own
//! pretty printer agrees on none of the four.

use serde_json::{Map, Value};

use crate::pytext::repr_float;

/// A JSON object off the rows a caller hands it; `dumps` sorts the keys.
pub fn object(rows: impl IntoIterator<Item = (String, Value)>) -> Value {
    Value::Object(rows.into_iter().collect::<Map<String, Value>>())
}

/// The document without its trailing newline: `render` adds that.
pub fn dumps(value: &Value) -> String {
    render(value, true)
}

/// `json.dumps(obj, indent=None, sort_keys=True)`, whose separators are
/// `", "` and `": "`. The `traversal` dump layer is written this way,
/// where millions of spans read no better one per line.
pub fn dumps_compact(value: &Value) -> String {
    render(value, false)
}

fn render(value: &Value, pretty: bool) -> String {
    let mut out = String::new();
    write(&mut out, value, 0, pretty);
    out
}

fn write(out: &mut String, value: &Value, depth: usize, pretty: bool) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => match n.as_f64() {
            Some(x) if n.is_f64() => out.push_str(&repr_float(x)),
            _ => out.push_str(&n.to_string()),
        },
        Value::String(s) => escape(out, s),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                separate(out, i, depth + 1, pretty);
                write(out, item, depth + 1, pretty);
            }
            close(out, depth, pretty);
            out.push(']');
        }
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                separate(out, i, depth + 1, pretty);
                escape(out, key);
                out.push_str(": ");
                write(out, &map[key.as_str()], depth + 1, pretty);
            }
            close(out, depth, pretty);
            out.push('}');
        }
    }
}

fn separate(out: &mut String, at: usize, depth: usize, pretty: bool) {
    match (pretty, at) {
        (true, 0) => out.push('\n'),
        (true, _) => out.push_str(",\n"),
        (false, 0) => {}
        (false, _) => out.push_str(", "),
    }
    if pretty {
        indent(out, depth);
    }
}

fn close(out: &mut String, depth: usize, pretty: bool) {
    if pretty {
        out.push('\n');
        indent(out, depth);
    }
}

fn indent(out: &mut String, depth: usize) {
    out.extend(std::iter::repeat_n(' ', depth * 2));
}

/// `json.encoder.py_encode_basestring_ascii`: `\` and `"` and everything
/// outside the printable ASCII run are escaped, the rest is copied.
fn escape(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ' '..='~' => out.push(c),
            _ if (c as u32) < 0x10000 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => {
                let n = c as u32 - 0x10000;
                let high = 0xd800 | ((n >> 10) & 0x3ff);
                let low = 0xdc00 | (n & 0x3ff);
                out.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
            }
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_containers_and_sorted_keys() {
        let value: Value = serde_json::from_str(r#"{"b": {}, "a": [], "c": [1]}"#).unwrap();
        assert_eq!(
            dumps(&value),
            "{\n  \"a\": [],\n  \"b\": {},\n  \"c\": [\n    1\n  ]\n}"
        );
    }
}
