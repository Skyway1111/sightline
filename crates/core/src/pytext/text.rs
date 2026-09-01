//! Whitespace, splitting and stripping as `str` does them.

/// Python `str.isspace` for one character: Unicode White_Space plus the four
/// C1 separators `\x1c`-`\x1f` that CPython's table holds and Rust's does not.
pub fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python `str.split()` with no separator: runs of whitespace separate, and
/// no empty field survives.
pub fn split(s: &str) -> Vec<&str> {
    s.split(is_py_space).filter(|p| !p.is_empty()).collect()
}

/// `sep.join([head, *parts])`: a dotted qname, or a `::` Rust path.
pub fn join_path(head: &str, parts: &[&str], sep: &str) -> String {
    let mut out = String::from(head);
    for part in parts {
        out.push_str(sep);
        out.push_str(part);
    }
    out
}

/// The characters CPython's line splitter breaks at.
fn is_linebreak(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\u{b}'
            | '\u{c}'
            | '\u{1c}'
            | '\u{1d}'
            | '\u{1e}'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// Python `str.splitlines()`: eleven break characters, `\r\n` as one break,
/// and no trailing empty field. `str::lines` breaks at `\n` alone, so a
/// module holding a `\f` reads one line short there (REF `CLAUDE.md`, the
/// `str.splitlines` trap).
/// Lines as CPython's tokenizer and AST count them: split on `\n` alone,
/// one trailing empty element dropped (R2). `splitlines` also breaks at
/// `\f`, `\v`, `\x1c`-`\x1e`, `\x85`, U+2028 and U+2029, and every line
/// index after one of those is off by one.
pub fn source_lines(source: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = source.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

pub fn splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < s.len() {
        let c = s[i..]
            .chars()
            .next()
            .expect("i sits on a character boundary");
        let width = c.len_utf8();
        if is_linebreak(c) {
            out.push(&s[start..i]);
            i += width;
            if c == '\r' && s[i..].starts_with('\n') {
                i += 1;
            }
            start = i;
        } else {
            i += width;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// Python `str.strip()`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

/// Python `str.lstrip()`.
pub fn lstrip(s: &str) -> &str {
    s.trim_start_matches(is_py_space)
}

/// Python `str.rstrip()`.
pub fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_py_space)
}

/// Python `str.strip(chars)`.
pub fn strip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_matches(|c| chars.contains(c))
}

/// Python `str.lstrip(chars)`.
pub fn lstrip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_start_matches(|c| chars.contains(c))
}

/// Python `str.rstrip(chars)`.
pub fn rstrip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_end_matches(|c| chars.contains(c))
}

/// Python `str.lower()`.
pub fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// Python `str.removeprefix(prefix)`.
pub fn removeprefix<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix).unwrap_or(s)
}

/// Python `str.partition(sep)`.
pub fn partition<'a>(s: &'a str, sep: &str) -> (&'a str, &'a str, &'a str) {
    match s.find(sep) {
        Some(at) => (&s[..at], &s[at..at + sep.len()], &s[at + sep.len()..]),
        None => (s, "", ""),
    }
}

/// Python `str.rpartition(sep)`: an absent separator puts the whole string
/// last, where `partition` puts it first.
pub fn rpartition<'a>(s: &'a str, sep: &str) -> (&'a str, &'a str, &'a str) {
    match s.rfind(sep) {
        Some(at) => (&s[..at], &s[at..at + sep.len()], &s[at + sep.len()..]),
        None => ("", "", s),
    }
}

/// Python `str.expandtabs(tabsize)`: the column resets at `\n` and `\r`.
pub fn expandtabs(s: &str, tabsize: usize) -> String {
    let mut out = String::with_capacity(s.len());
    let mut col = 0;
    for c in s.chars() {
        match c {
            '\t' => {
                let width = if tabsize == 0 {
                    0
                } else {
                    tabsize - col % tabsize
                };
                out.extend(std::iter::repeat_n(' ', width));
                col += width;
            }
            '\n' | '\r' => {
                out.push(c);
                col = 0;
            }
            _ => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// `inspect.cleandoc(doc)`, the reading `ast.get_docstring(clean=True)` gives
/// (R11).
pub fn cleandoc(doc: &str) -> String {
    let expanded = expandtabs(doc, 8);
    let mut lines: Vec<String> = expanded.split('\n').map(str::to_string).collect();
    let mut margin = usize::MAX;
    for line in lines.iter().skip(1) {
        let content = line.trim_start_matches(' ');
        if !content.is_empty() {
            margin = margin.min(line.chars().count() - content.chars().count());
        }
    }
    if let Some(first) = lines.first_mut() {
        *first = first.trim_start_matches(' ').to_string();
    }
    if margin < usize::MAX {
        for line in lines.iter_mut().skip(1) {
            *line = line.chars().skip(margin).collect();
        }
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    while lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }
    lines.join("\n")
}

/// `textwrap.dedent(text)`: the common leading run of spaces and tabs goes,
/// and a whitespace-only line becomes empty.
pub fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let blank = |l: &str| l.is_empty() || l.chars().all(is_py_space);
    let non_blank: Vec<&str> = lines.iter().copied().filter(|l| !blank(l)).collect();
    let mut margin = 0;
    if let (Some(lo), Some(hi)) = (non_blank.iter().min(), non_blank.iter().max()) {
        let mut hi = hi.chars();
        for (k, c) in lo.chars().enumerate() {
            margin = k;
            if hi.next() != Some(c) || (c != ' ' && c != '\t') {
                break;
            }
        }
    }
    lines
        .iter()
        .map(|l| {
            if blank(l) && !l.is_empty() {
                String::new()
            } else {
                l.chars().skip(margin).collect()
            }
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitlines_breaks_where_the_tokenizer_does_not() {
        assert_eq!(splitlines("a\x0cb\nc"), ["a", "b", "c"]);
        assert_eq!(splitlines("a\r\nb"), ["a", "b"]);
        assert_eq!(splitlines("a\n"), ["a"]);
        assert_eq!(splitlines(""), Vec::<&str>::new());
    }

    #[test]
    fn partition_and_rpartition_disagree_on_a_miss() {
        assert_eq!(partition("a.b", "."), ("a", ".", "b"));
        assert_eq!(partition("ab", "."), ("ab", "", ""));
        assert_eq!(rpartition("a.b.c", "."), ("a.b", ".", "c"));
        assert_eq!(rpartition("ab", "."), ("", "", "ab"));
    }
}
