//! Comment and docstring text as a rule may read it, the half that judges text
//! alone: whether a line labels a phase (#18), what raises a docstring
//! declares (#53), whether a comment run reads as documentation (#29), and the
//! wording a def uses to say its call must not raise (#42). The functions that
//! need an AST (`declares_no_raise`, `comment_blocks`, `documents_module`,
//! `parses_as_code`) stay in each language's provers.
//!
//! Every pattern here is its Python twin verbatim (R7).

use std::collections::{BTreeSet, HashMap};
use std::sync::{LazyLock, Mutex};

use regex::Regex;

use crate::pytext;

/// The value a table short enough to be a slice holds for `key` (R11).
pub fn lookup<V: Copy>(table: &[(&str, V)], key: &str) -> Option<V> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// The five candidates a typo most likely meant, best first
/// (`difflib.get_close_matches`, cutoff 0.4): what each language's `describe`
/// offers for a qname no index holds. The measure is the longest common
/// subsequence over characters rather than difflib's matching blocks.
pub fn nearest<'a, I: IntoIterator<Item = &'a str>>(word: &str, candidates: I) -> Vec<String> {
    let mut scored: Vec<(f64, &str)> = candidates
        .into_iter()
        .map(|c| (ratio(word, c), c))
        .filter(|(r, _)| *r >= 0.4)
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(5)
        .map(|(_, name)| name.to_string())
        .collect()
}

/// `2 * matched / (len(a) + len(b))`, matched being the longest common
/// subsequence.
fn ratio(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let total = a.len() + b.len();
    if total == 0 {
        return 1.0;
    }
    let mut row = vec![0usize; b.len() + 1];
    for x in &a {
        let mut prev = 0;
        for (j, y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if x == y { prev + 1 } else { above.max(row[j]) };
            prev = above;
        }
    }
    2.0 * row[b.len()] as f64 / total as f64
}

fn compiled(pattern: &str) -> Regex {
    Regex::new(pattern).expect("a literal pattern that compiles")
}

static RAISES_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"^([ \t]*)Raises[ \t]*:?[ \t]*$"));
static UNDERLINE_RE: LazyLock<Regex> = LazyLock::new(|| compiled(r"^[ \t]*-{3,}[ \t]*$"));
static SPHINX_RAISES_RE: LazyLock<Regex> = LazyLock::new(|| compiled(r":raises?[ \t]+([^:\n]+):"));
static TYPE_NAME_RE: LazyLock<Regex> = LazyLock::new(|| compiled(r"[A-Za-z_][\w.]*"));

/// The def says in its own words that what it calls must not raise.
pub static NO_RAISE_RE: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"(?i)\b(?:not|never|n't)\s+(?:be\s+|to\s+)?raise"));

/// A rule, a fence, an empty spacer comment.
pub const BAR_CHARS: &str = "-=*_~/# ";

/// A line the tools own: shebang, encoding, pragma.
pub static DIRECTIVE_RE: LazyLock<Regex> = LazyLock::new(|| {
    compiled(
        r"(?i)^(?:!|-\*-|noqa\b|coding[:=]|[\w.-]+:\s*(?:ignore|noqa|disable|enable|skip|off|on)\b)",
    )
});

pub static LICENSE_RE: LazyLock<Regex> =
    LazyLock::new(|| compiled(r"(?i)copyright|licen[sc]e|SPDX|warrant"));

static SECTION_RES: LazyLock<Mutex<HashMap<String, Regex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Does a comment line head a section of code, `# Step 2`, `# 3) load`, a
/// divider bar (#18)? `marker` is the language's comment opener. The match
/// runs under the cache lock: a cloned `Regex` starts with a cold scratch
/// cache, so handing clones out made every match pay a fresh allocation.
pub fn is_phase_label(text: &str, marker: &str) -> bool {
    let mut cache = SECTION_RES.lock().expect("no panic holds this lock");
    cache
        .entry(marker.to_string())
        .or_insert_with(|| {
            let m = pytext::escape_re(marker);
            compiled(&format!(
                r"(?i)^(?:{m})+\s*((step|phase|stage|part)\s*\d|\d+[.):]\s|[-=*{m}]{{3,}})"
            ))
        })
        .is_match(text)
}

/// Exception names a docstring's raises section declares, by their last dotted
/// part: a Google `Raises:` block (entries `X: why`, one indent below the
/// header), a NumPy `Raises` plus underline section (entries at the header's
/// indent), Sphinx `:raises X:` / `:raise X:` fields. Empty where no section
/// names a type.
pub fn declared_raises(doc: &str) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for caps in SPHINX_RAISES_RE.captures_iter(doc) {
        let field = caps.get(1).expect("the pattern has one group").as_str();
        names.extend(
            TYPE_NAME_RE
                .find_iter(field)
                .map(|m| m.as_str().to_string()),
        );
    }
    let lines = pytext::splitlines(doc);
    for i in 0..lines.len() {
        let Some(header) = RAISES_HEADER_RE.captures(lines[i]) else {
            continue;
        };
        let indent = header
            .get(1)
            .expect("the pattern has one group")
            .as_str()
            .chars()
            .count();
        let numpy = i + 1 < lines.len() && UNDERLINE_RE.is_match(lines[i + 1]);
        // Google: the first body line's indent, one level in
        let mut entry: i64 = if numpy { indent as i64 } else { -1 };
        for j in (if numpy { i + 2 } else { i + 1 })..lines.len() {
            let text = lines[j];
            if pytext::strip(text).is_empty() {
                continue;
            }
            let depth = (text.chars().count() - pytext::lstrip(text).chars().count()) as i64;
            if entry < 0 {
                if depth <= indent as i64 {
                    break;
                }
                entry = depth;
            }
            let next_header = numpy && j + 1 < lines.len() && UNDERLINE_RE.is_match(lines[j + 1]);
            if depth < entry || next_header {
                break;
            }
            if depth > entry {
                continue; // a description or continuation line
            }
            let head = if text.contains(':') {
                pytext::partition(text, ":").0
            } else {
                pytext::split(text)[0]
            };
            names.extend(TYPE_NAME_RE.find_iter(head).map(|m| m.as_str().to_string()));
        }
    }
    // exception types are CapWords; a prose head (`when x is bad: ...`) is not
    names
        .iter()
        .map(|n| pytext::rpartition(n, ".").2.to_string())
        .filter(|last| pytext::is_upper_first(last))
        .collect()
}

/// Does a run of comment lines document what it heads? What is left once a
/// shebang, an encoding line, a tool directive and rule bars are dropped is the
/// prose; a licence header states the terms, not what the code is. Every
/// comment opener is a bar character, so the reading needs no marker. The one
/// reading of a comment block as documentation.
pub fn reads_as_doc<S: AsRef<str>>(lines: &[S]) -> bool {
    let prose: Vec<&str> = lines
        .iter()
        .map(|raw| pytext::strip(pytext::lstrip_chars(raw.as_ref(), "#")))
        .filter(|text| {
            !text.is_empty()
                && text.chars().any(|c| !BAR_CHARS.contains(c))
                && !DIRECTIVE_RE.is_match(text)
        })
        .collect();
    !prose.is_empty() && !prose.iter().any(|line| LICENSE_RE.is_match(line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_phase_label_heads_a_section() {
        assert!(is_phase_label("# Step 2", "#"));
        assert!(is_phase_label("# 3) load", "#"));
        assert!(is_phase_label("### phase 1", "#"));
        assert!(is_phase_label("# ---- ", "#"));
        assert!(is_phase_label("#####", "#"));
        assert!(!is_phase_label("# a note about steps", "#"));
        assert!(!is_phase_label("Step 2", "#"));
        // the Rust marker builds its own pattern
        assert!(is_phase_label("// Step 2", "//"));
        assert!(is_phase_label("// ====", "//"));
        assert!(!is_phase_label("# Step 2", "//"));
    }

    #[test]
    fn a_google_block_names_its_types_one_indent_in() {
        let doc = "Do it.\n\nRaises:\n    ValueError: when x is bad\n        more prose\n    pkg.OwnError: why\n\nReturns:\n    nothing\n";
        assert_eq!(declared_raises(doc), set(&["OwnError", "ValueError"]));
    }

    #[test]
    fn a_numpy_section_reads_entries_at_the_header_indent() {
        let doc = "Do it.\n\nRaises\n------\nValueError\n    when x is bad\nKeyError\n    when y is missing\n\nNotes\n-----\nnothing\n";
        assert_eq!(declared_raises(doc), set(&["KeyError", "ValueError"]));
    }

    #[test]
    fn sphinx_fields_and_prose_heads() {
        assert_eq!(
            declared_raises(":raises ValueError: bad\n"),
            set(&["ValueError"])
        );
        assert_eq!(declared_raises(":raise OSError: bad\n"), set(&["OSError"]));
        // a prose head is not CapWords, so no type is declared
        assert_eq!(
            declared_raises("Raises:\n    when x is bad: why\n"),
            set(&[])
        );
        assert_eq!(declared_raises("no section here\n"), set(&[]));
    }

    #[test]
    fn no_raise_reads_the_wording_a_def_uses() {
        assert!(NO_RAISE_RE.is_match("must not raise"));
        assert!(NO_RAISE_RE.is_match("should never raise"));
        assert!(NO_RAISE_RE.is_match("must not be raised"));
        assert!(!NO_RAISE_RE.is_match("raises ValueError"));
    }

    #[test]
    fn prose_survives_bars_and_directives_but_a_licence_is_not_documentation() {
        assert!(reads_as_doc(&["# What this module is"]));
        assert!(reads_as_doc(&[
            "# ----------",
            "# What this module is",
            "# ------"
        ]));
        assert!(!reads_as_doc(&["# ----------"]));
        assert!(!reads_as_doc(&["#!/usr/bin/env python"]));
        assert!(!reads_as_doc(&["# -*- coding: utf-8 -*-"]));
        assert!(!reads_as_doc(&["# ruff: noqa"]));
        assert!(!reads_as_doc(&[
            "# Copyright 2026 someone",
            "# All rights reserved"
        ]));
        assert!(!reads_as_doc(&["# SPDX-License-Identifier: MIT"]));
        assert!(!reads_as_doc(Vec::<String>::new().as_slice()));
    }
}
