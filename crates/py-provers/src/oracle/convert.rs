//! The six lints the oracle turns on, a ty diagnostic as sightline reads
//! one, and the type displays the type-string algebra expects.

use ruff_db::diagnostic::{Diagnostic, DiagnosticId, Severity, UnifiedFile};
use ruff_db::source::{line_index, source_text};
use ruff_db::system::SystemPath;
use ty_project::ProjectDatabase;

use super::OracleDiag;

/// The six lints the oracle turns on (ty ships them at `Level::Ignore`) and
/// pyright's rule name for each. Every mapped rule is answered at *warning*
/// severity, which is what makes `possibly-unresolved-reference` reportable at
/// all: the counterfactual veto (#5/#10) fires on new *error*-severity
/// diagnostics, so a possibly-unbound read a splice reveals must never reach
/// it as one.
pub const ENABLED_RULES: [(&str, &str); 6] = [
    ("unnecessary-isinstance", "reportUnnecessaryIsInstance"),
    ("unnecessary-comparison", "reportUnnecessaryComparison"),
    ("unnecessary-contains", "reportUnnecessaryContains"),
    ("redundant-cast", "reportUnnecessaryCast"),
    ("unresolved-import", "reportMissingImports"),
    ("possibly-unresolved-reference", "reportPossiblyUnbound"),
];

/// pyright's name for a mapped rule; `None` for every other diagnostic.
fn pyright_rule(id: &DiagnosticId) -> Option<&'static str> {
    let DiagnosticId::Lint(name) = id else {
        return None;
    };
    ENABLED_RULES
        .iter()
        .find(|(rule, _)| *rule == name.as_str())
        .map(|(_, pyright)| *pyright)
}

/// A mapped rule at warning severity, any other error-severity diagnostic
/// under its own ty id, everything else dropped. Keying the passthrough by
/// ty's id is what arms the veto: the counterfactual identity is
/// `(rel, line, rule)`, and an empty rule there collapses every passthrough of
/// a file and line into one key.
pub fn convert(
    db: &ProjectDatabase,
    root: &SystemPath,
    diagnostic: &Diagnostic,
) -> Option<OracleDiag> {
    let id = diagnostic.id();
    if id == DiagnosticId::RevealedType {
        return None; // transport artifact (ours or a user-authored reveal_type)
    }
    let (rule, severity) = match pyright_rule(&id) {
        Some(rule) => (rule.to_string(), "warning"),
        None if diagnostic.severity() >= Severity::Error => (id.to_string(), "error"),
        None => return None,
    };
    let annotation = diagnostic.primary_annotation()?;
    let span = annotation.get_span();
    let &UnifiedFile::Ty(file) = span.file() else {
        return None;
    };
    let range = span.range()?;
    let rel = super::rel_of(db, root, file)?;
    let source = source_text(db, file);
    let index = line_index(db, file);
    let start = index.line_column(range.start(), &source);
    Some(OracleDiag {
        rel,
        line: start.line.to_zero_indexed() as u32 + 1,
        col: start.column.to_zero_indexed() as u32,
        rule,
        message: diagnostic.headline_message().replace('`', "\""),
        severity: severity.to_string(),
    })
}

/// Rewrite ty type displays that sightline's type-string algebra would misread
/// into pyright's forms: exact-form markers (`float*` -> `float`), module
/// literals (`<module 'x'>` -> `Module("x")`), and named function signatures
/// (`def f(x) -> R` -> `(x) -> R`).
pub fn normalize_type_display(text: &str) -> String {
    // Every rewrite leaves quoted string-literal contents (`Literal["a*b"]`)
    // alone. `<module 'x'>` runs on the whole string because the module form
    // holds its own single quotes, which the quote-aware pass below would
    // read as a string literal; the full `<module '` prefix is specific enough
    // not to occur inside a real literal. Its `Module("x")` output then reads
    // as a quoted span, protecting module names from the star and def surgery.
    let mut with_modules = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(idx) = rest.find("<module '") {
        with_modules.push_str(&rest[..idx]);
        let tail = &rest[idx + "<module '".len()..];
        if let Some(end) = tail.find("'>") {
            with_modules.push_str("Module(\"");
            with_modules.push_str(&tail[..end]);
            with_modules.push_str("\")");
            rest = &tail[end + 2..];
        } else {
            with_modules.push_str("<module '");
            rest = tail;
        }
    }
    with_modules.push_str(rest);

    rewrite_outside_quotes(&with_modules, |span, out| {
        let bytes = span.as_bytes();
        let mut skip_until = 0usize;
        for (i, ch) in span.char_indices() {
            if i < skip_until {
                continue;
            }
            if ch == '*' && i > 0 && bytes[i - 1].is_ascii_alphanumeric() {
                continue;
            }
            if span[i..].starts_with("def ")
                && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                && let Some(paren) = span[i..].find('(')
            {
                let name = &span[i + 4..i + paren];
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    skip_until = i + paren;
                    continue;
                }
            }
            out.push(ch);
        }
    })
}

/// Apply `rewrite` to the spans of `text` outside `"..."` and `'...'` string
/// literals (backslash escapes respected); a quoted span is copied through.
fn rewrite_outside_quotes(text: &str, rewrite: impl Fn(&str, &mut String)) -> String {
    let mut out = String::with_capacity(text.len());
    let mut span_start = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, ch) in text.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == q {
                    quote = None;
                    out.push_str(&text[span_start..i + ch.len_utf8()]);
                    span_start = i + ch.len_utf8();
                }
            }
            None => {
                if ch == '"' || ch == '\'' {
                    rewrite(&text[span_start..i], &mut out);
                    span_start = i;
                    quote = Some(ch);
                }
            }
        }
    }
    match quote {
        // an unterminated quote: the tail is literal
        Some(_) => out.push_str(&text[span_start..]),
        None => rewrite(&text[span_start..], &mut out),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::normalize_type_display;

    #[test]
    fn star_markers_stripped_outside_literals() {
        assert_eq!(normalize_type_display("float*"), "float");
        assert_eq!(
            normalize_type_display("int | float* | complex*"),
            "int | float | complex"
        );
    }

    #[test]
    fn literal_contents_untouched() {
        assert_eq!(
            normalize_type_display("Literal[\"a*b\"]"),
            "Literal[\"a*b\"]"
        );
        assert_eq!(
            normalize_type_display("Literal['def f(']"),
            "Literal['def f(']"
        );
        assert_eq!(
            normalize_type_display("Literal[\"x\"] | float*"),
            "Literal[\"x\"] | float"
        );
    }

    #[test]
    fn module_display_rewritten() {
        assert_eq!(
            normalize_type_display("<module 'torch'>"),
            "Module(\"torch\")"
        );
        // the rewritten module name is quote-protected from the later passes
        assert_eq!(normalize_type_display("<module 'a*b'>"), "Module(\"a*b\")");
    }

    #[test]
    fn def_signatures_lose_their_name() {
        assert_eq!(normalize_type_display("def call() -> int"), "() -> int");
        assert_eq!(
            normalize_type_display("def f(x, y) -> None"),
            "(x, y) -> None"
        );
        // not a definition display: untouched
        assert_eq!(
            normalize_type_display("undef (x) -> int"),
            "undef (x) -> int"
        );
    }
}
