//! Span edits. Columns are the units
//! the reporting site used, which for every edit site is a code point
//! count (R17), so the slicing is by code point too.

use crate::findings::SpanEdit;

/// Python's `s[start:end]` over code points: both ends clamp, and a start
/// past the end is the empty string.
pub fn char_slice(s: &str, start: usize, end: usize) -> &str {
    let at = |n: usize| s.char_indices().nth(n).map_or(s.len(), |(i, _)| i);
    let (a, z) = (at(start), at(end));
    if a >= z { "" } else { &s[a..z] }
}

/// In place, right to left and bottom up so every span stays valid: the one
/// encoding both the emitter and the counterfactual worlds apply.
pub fn apply_edits(lines: &mut [String], edits: &[SpanEdit]) {
    let mut order: Vec<&SpanEdit> = edits.iter().collect();
    order.sort_by_key(|e| std::cmp::Reverse((e.line, e.col_start)));
    for e in order {
        let ln = &lines[e.line as usize - 1];
        let head = char_slice(ln, 0, e.col_start as usize);
        let tail = char_slice(ln, e.col_end as usize, usize::MAX);
        lines[e.line as usize - 1] = format!("{head}{}{tail}", e.text);
    }
}

/// `blank`'s own edit: it deletes the line instead of editing inside it.
pub fn takes_line(e: &SpanEdit) -> bool {
    e.col_start == 0 && e.text.is_empty()
}

/// Delete lines `[first, last]` by emptying them: a world's diagnostic diff
/// is line-keyed, so a deletion may never shift a line. `patch` drops an
/// emptied line from the diff, where applied text does move.
pub fn blank<S: AsRef<str>>(lines: &[S], first: u32, last: u32) -> Vec<SpanEdit> {
    (first..=last)
        .map(|ln| SpanEdit {
            line: ln,
            col_start: 0,
            col_end: lines[ln as usize - 1].as_ref().chars().count() as u32,
            text: String::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(line: u32, col_start: u32, col_end: u32, text: &str) -> SpanEdit {
        SpanEdit {
            line,
            col_start,
            col_end,
            text: text.into(),
        }
    }

    #[test]
    fn char_slice_counts_code_points_and_clamps() {
        assert_eq!(char_slice("abc", 0, 2), "ab");
        assert_eq!(char_slice("abc", 1, 99), "bc");
        assert_eq!(char_slice("abc", 2, 1), "");
        assert_eq!(char_slice("abc", 9, 99), "");
        // three code points, seven bytes: a byte slice would split the emoji
        assert_eq!(char_slice("a\u{e9}\u{1f600}b", 1, 3), "\u{e9}\u{1f600}");
    }

    #[test]
    fn edits_apply_bottom_up_and_right_to_left() {
        let mut lines = vec!["abcdef".to_string(), "ghijkl".to_string()];
        apply_edits(
            &mut lines,
            &[edit(1, 1, 2, "X"), edit(1, 4, 5, "Y"), edit(2, 0, 3, "")],
        );
        assert_eq!(lines, ["aXcdYf", "jkl"]);
    }

    #[test]
    fn an_edit_inside_a_multibyte_line_keeps_its_neighbours() {
        let mut lines = vec!["x = \"\u{e9}\u{1f600}\"  # note".to_string()];
        apply_edits(&mut lines, &[edit(1, 8, 16, "")]);
        assert_eq!(lines, ["x = \"\u{e9}\u{1f600}\""]);
    }

    #[test]
    fn blank_empties_every_line_of_the_span() {
        let lines = [
            "one".to_string(),
            "\u{e9}\u{1f600}".to_string(),
            "three".to_string(),
        ];
        let edits = blank(&lines, 2, 3);
        assert_eq!(edits, [edit(2, 0, 2, ""), edit(3, 0, 5, "")]);
        assert!(edits.iter().all(takes_line));
        assert!(!takes_line(&edit(1, 0, 3, "x")));
        assert!(!takes_line(&edit(1, 2, 3, "")));
    }
}
