//! Character classes: what CPython calls printable, a decimal digit,
//! an identifier.

use std::sync::LazyLock;

use regex::Regex;

/// Every general category CPython calls printable, so a character outside it
/// is escaped by `repr_str`. The categories `Cc`, `Cf`, `Co`, `Cn`, `Zl`,
/// `Zp` and `Zs` are the complement; `Cs` cannot reach a Rust `char`.
static PRINTABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{L}\p{M}\p{N}\p{P}\p{S}]$").expect("a literal pattern"));

static DECIMAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\p{Nd}+$").expect("a literal pattern"));

pub(super) fn is_printable(c: char) -> bool {
    if c.is_ascii() {
        return (' '..='~').contains(&c);
    }
    PRINTABLE.is_match(c.encode_utf8(&mut [0; 4]))
}

/// Python `str.isidentifier()`.
pub fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => {
            (first == '_' || unicode_ident::is_xid_start(first))
                && chars.all(unicode_ident::is_xid_continue)
        }
        None => false,
    }
}

/// Python `s[:1].isupper()`: the first character is cased upper.
pub fn is_upper_first(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}

/// Python `str.isdigit()` narrowed to `Nd` (R6): the two call sites read git's
/// own output, which is ASCII.
pub fn is_digit(s: &str) -> bool {
    !s.is_empty() && DECIMAL.is_match(s)
}
