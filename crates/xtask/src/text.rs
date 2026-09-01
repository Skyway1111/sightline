//! Spelling helpers two subcommands each need, so neither writes its own.

/// Which format a string is being spelled for. JSON escapes every character
/// above ASCII as UTF-16 units and spells its hex in lower case; a TOML
/// basic string keeps them and spells its hex in upper case.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Json,
    Toml,
}

/// The escapes both formats share, then a control character as `\uXXXX`.
/// The quotes are the caller's: `gauntlet`'s writer is mid-document and
/// `retired`'s wraps one value.
pub fn escape_into(out: &mut String, s: &str, style: Style) {
    let json = style == Style::Json;
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 && json => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c if !json || c.is_ascii() => out.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
}

/// A catalog pair's verdict by (planted, proven). A planted pair always
/// fails the run and one that proves is a machinery gap. `denied` is the
/// checker's word for a pair it could not prove: `refuted` for the idiom
/// catalog, `refused` for the perf one.
pub fn label(planted: bool, proven: bool, denied: &str) -> String {
    match (planted, proven) {
        (false, true) => "proven".to_string(),
        (false, false) => denied.to_uppercase(),
        (true, true) => "SELF-TEST MISS".to_string(),
        (true, false) => format!("{denied} (expected)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both callers' bytes, unchanged: JSON spells a non-ASCII char as
    /// UTF-16 units in lower case, TOML keeps it and upper-cases its
    /// control escapes.
    #[test]
    fn each_style_spells_what_its_format_needs() {
        let mut json = String::new();
        escape_into(&mut json, "a\"\\\n\u{1}\u{20ac}", Style::Json);
        assert_eq!(json, "a\\\"\\\\\\n\\u0001\\u20ac");
        let mut toml = String::new();
        escape_into(&mut toml, "a\"\\\n\u{1}\u{20ac}", Style::Toml);
        assert_eq!(toml, "a\\\"\\\\\\n\\u0001\u{20ac}");
        // a character outside the basic plane becomes a surrogate pair
        let mut wide = String::new();
        escape_into(&mut wide, "\u{1f600}", Style::Json);
        assert_eq!(wide, "\\ud83d\\ude00");
    }

    #[test]
    fn the_label_reads_the_checkers_own_word() {
        assert_eq!(label(false, true, "refuted"), "proven");
        assert_eq!(label(false, false, "refuted"), "REFUTED");
        assert_eq!(label(false, false, "refused"), "REFUSED");
        assert_eq!(label(true, true, "refused"), "SELF-TEST MISS");
        assert_eq!(label(true, false, "refuted"), "refuted (expected)");
    }
}
