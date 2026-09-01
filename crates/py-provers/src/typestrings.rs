//! Port of `provers/typestrings.py` (codemap 3.3): type-string algebra over
//! oracle answers, its own home so argtypes and the oracle never entangle
//! through it.

use std::sync::LazyLock;

use regex::Regex;
use sightline_core::pytext;

/// Split on `sep` occurrences at bracket depth 0, outside quotes.
pub fn split_top<'a>(s: &'a str, sep: &str) -> Vec<&'a str> {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let step = sep.chars().count();
    let at = |i: usize| chars.get(i).map_or(s.len(), |(b, _)| *b);
    let mut parts: Vec<&str> = Vec::new();
    let mut depth: i32 = 0;
    let (mut start, mut i) = (0usize, 0usize);
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i].1;
        match quote {
            Some(q) => {
                if c == '\\' {
                    i += 1;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '\'' || c == '"' => quote = Some(c),
            None => {
                depth += i32::from("[(".contains(c)) - i32::from("])".contains(c));
                if depth == 0 && s[at(i)..].starts_with(sep) {
                    parts.push(&s[start..at(i)]);
                    i += step;
                    start = at(i);
                    continue;
                }
            }
        }
        i += 1;
    }
    parts.push(&s[start..]);
    parts
}

/// Top-level ` | ` split, bracket-aware.
pub fn split_union(type_str: &str) -> Vec<&str> {
    split_top(type_str, " | ")
        .into_iter()
        .map(pytext::strip)
        .filter(|p| !p.is_empty())
        .collect()
}

static LITERAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Literal\[(.*)\]$").expect("a literal pattern"));
static INT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-?\d+$").expect("a literal pattern"));

/// `Literal['a', 3, True]` -> the base types of its values.
pub fn deliteral(member: &str) -> Vec<String> {
    let Some(caps) = LITERAL_RE.captures(member) else {
        return vec![member.to_string()];
    };
    let mut raws: Vec<&str> = split_top(&caps[1], ",")
        .into_iter()
        .map(pytext::strip)
        .collect();
    if raws.last() == Some(&"") {
        raws.pop();
    }
    raws.into_iter()
        .map(|raw| {
            if raw == "True" || raw == "False" {
                "bool"
            } else if raw.starts_with('\'') || raw.starts_with('"') {
                "str"
            } else if raw.starts_with("b'") || raw.starts_with("b\"") {
                "bytes"
            } else if INT_RE.is_match(raw) {
                "int"
            } else {
                member // enum literals and the like: keep as written
            }
            .to_string()
        })
        .collect()
}

pub fn generic_base(member: &str) -> &str {
    member.split_once('[').map_or(member, |(head, _)| head)
}

/// Members with literals lowered to base types; `None` when any is
/// `Any`/`Unknown` - absence of type information is not evidence (#5, #33).
pub fn union_members(type_str: &str) -> Option<Vec<String>> {
    let members: Vec<String> = split_union(type_str)
        .into_iter()
        .flat_map(deliteral)
        .collect();
    let opaque = members
        .iter()
        .any(|m| matches!(generic_base(m), "Any" | "Unknown"));
    (!opaque).then_some(members)
}

/// The union's spelling: sorted, a member another subsumes dropped - the
/// `tuple[()]` a `()` default contributes beside a `tuple[X, ...]`.
pub fn join<S: AsRef<str>>(members: &[S]) -> Vec<String> {
    let mut out: Vec<String> = members.iter().map(|m| m.as_ref().to_string()).collect();
    out.sort();
    out.dedup();
    if out
        .iter()
        .any(|m| m.starts_with("tuple[") && m.ends_with(", ...]"))
    {
        out.retain(|m| m != "tuple[()]");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `tests/provers/test_typestrings.py:test_union_members_and_join`.
    #[test]
    fn union_members_and_join() {
        assert_eq!(
            union_members("Literal['a', 1] | None"),
            Some(vec!["str".into(), "int".into(), "None".into()])
        );
        // a nested Any is the member's
        assert_eq!(
            union_members("list[Any]"),
            Some(vec!["list[Any]".to_string()])
        );
        for opaque in ["Any", "Unknown | int", "Literal['a'] | Unknown"] {
            assert_eq!(union_members(opaque), None, "{opaque}");
        }
        // a `()` default's `tuple[()]` beside a `tuple[X, ...]` is subsumed
        assert_eq!(
            join(&["tuple[()]", "tuple[Cand, ...]", "None"]),
            ["None", "tuple[Cand, ...]"]
        );
        assert_eq!(
            join(&["tuple[()]", "list[int]"]),
            ["list[int]", "tuple[()]"]
        );
        assert_eq!(join(&["int", "int", "str"]), ["int", "str"]);
    }

    /// `tests/provers/test_typestrings.py:test_split_top_skips_quoted_separators`.
    #[test]
    fn split_top_skips_quoted_separators() {
        assert_eq!(
            split_top("Literal[\"a, b\"], int", ","),
            ["Literal[\"a, b\"]", " int"]
        );
        assert_eq!(
            split_union("Literal[\"a | b\"] | None"),
            ["Literal[\"a | b\"]", "None"]
        );
        assert_eq!(deliteral("Literal[\"a, b\"]"), ["str"]);
        assert_eq!(deliteral("Literal[\"a\", 1]"), ["str", "int"]);
        assert_eq!(deliteral("Literal['it\\'s', 2]"), ["str", "int"]);
    }
}
