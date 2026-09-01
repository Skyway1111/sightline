//! `fnmatch` and `re.escape` as CPython reads a pattern.

use super::text::lower;

enum Tok {
    Star,
    Lit(Lit),
}

/// What one character is matched against: everything but a star, which
/// the matcher consumes itself.
enum Lit {
    Any,
    Char(char),
    Set { negated: bool, items: Vec<SetItem> },
}

enum SetItem {
    One(char),
    Range(char, char),
}

/// `fnmatch.translate`'s reading of one pattern: `*`, `?`, `[seq]`, `[!seq]`,
/// and an unclosed `[` as a literal.
fn compile(pat: &str) -> Vec<Tok> {
    let p: Vec<char> = pat.chars().collect();
    let n = p.len();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < n {
        let c = p[i];
        i += 1;
        match c {
            '*' => {
                if !matches!(out.last(), Some(Tok::Star)) {
                    out.push(Tok::Star);
                }
            }
            '?' => out.push(Tok::Lit(Lit::Any)),
            '[' => {
                let mut j = i;
                if j < n && p[j] == '!' {
                    j += 1;
                }
                if j < n && p[j] == ']' {
                    j += 1;
                }
                while j < n && p[j] != ']' {
                    j += 1;
                }
                if j >= n {
                    out.push(Tok::Lit(Lit::Char('[')));
                } else {
                    let stuff = &p[i..j];
                    i = j + 1;
                    let negated = stuff.first() == Some(&'!');
                    let body = if negated { &stuff[1..] } else { stuff };
                    out.push(Tok::Lit(Lit::Set {
                        negated,
                        items: set_items(body),
                    }));
                }
            }
            _ => out.push(Tok::Lit(Lit::Char(c))),
        }
    }
    out
}

fn set_items(body: &[char]) -> Vec<SetItem> {
    let mut items = Vec::new();
    let mut k = 0;
    while k < body.len() {
        if k + 2 < body.len() && body[k + 1] == '-' {
            items.push(SetItem::Range(body[k], body[k + 2]));
            k += 3;
        } else {
            items.push(SetItem::One(body[k]));
            k += 1;
        }
    }
    items
}

fn matches(lit: &Lit, c: char) -> bool {
    match lit {
        Lit::Any => true,
        Lit::Char(want) => *want == c,
        Lit::Set { negated, items } => {
            let hit = items.iter().any(|item| match item {
                SetItem::One(x) => *x == c,
                SetItem::Range(lo, hi) => *lo <= c && c <= *hi,
            });
            hit != *negated
        }
    }
}

/// Python `fnmatch.fnmatchcase(name, pat)`: no case folding, and `*` crosses
/// a path separator.
pub fn fnmatchcase(name: &str, pat: &str) -> bool {
    let toks = compile(pat);
    let text: Vec<char> = name.chars().collect();
    let (mut i, mut j) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while i < text.len() {
        match toks.get(j) {
            Some(Tok::Star) => {
                star = Some(j);
                mark = i;
                j += 1;
            }
            Some(Tok::Lit(lit)) if matches(lit, text[i]) => {
                i += 1;
                j += 1;
            }
            _ => match star {
                Some(back) => {
                    mark += 1;
                    i = mark;
                    j = back + 1;
                }
                None => return false,
            },
        }
    }
    toks[j..].iter().all(|t| matches!(t, Tok::Star))
}

/// Python `fnmatch.fnmatch(name, pat)`: `os.path.normcase` runs over both
/// first, which on Windows lowercases and turns `/` into `\`.
pub fn fnmatch(name: &str, pat: &str) -> bool {
    if cfg!(windows) {
        fnmatchcase(&normcase(name), &normcase(pat))
    } else {
        fnmatchcase(name, pat)
    }
}

fn normcase(s: &str) -> String {
    lower(&s.replace('/', "\\"))
}

/// `re.escape(text)`: the characters CPython's `_special_chars_map` names.
pub fn escape_re(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\u{b}\u{c}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
