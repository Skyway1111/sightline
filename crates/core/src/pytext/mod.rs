//! CPython string semantics: the one home for every `str` operation the port
//! copies from Python (R6, R8, R11, R18). A caller reaches for a spelling
//! here instead of Rust's own, because Rust's differs: `char::is_whitespace`
//! misses `\x1c`-`\x1f`, `str::lines` breaks only at `\n`, `{}` on an `f64`
//! prints `1e16` where Python prints `1e+16`.
//!
//! `tests/pytext_differential.rs` checks every function here against rows
//! CPython 3.14 wrote (`tests/data/pytext.json`).

mod chars;
mod glob;
mod repr;
mod text;

pub use chars::{is_digit, is_identifier, is_upper_first};
pub use glob::{escape_re, fnmatch, fnmatchcase};
pub use repr::{format_g, repr_bytes, repr_float, repr_int, repr_str, repr_str_list};
pub use text::*;
