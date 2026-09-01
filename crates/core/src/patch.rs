//! Patch composition and the unified diff (port of `emit.py`'s diff half).
//! Language-blind: it reads a `Fix`'s spans and two line lists, never source.
//! Building the patched lines is each language's emitter (phase 5).

use std::collections::HashSet;

use similar::{Algorithm, DiffTag, capture_diff_slices, group_diff_ops};

use crate::edits::takes_line;
use crate::findings::{Finding, Rel};

/// One patch per site: a fix whose edit sits on a line another fix deletes,
/// or whose whole deletion sits inside another's, loses its patch and keeps
/// its finding. `apply_edits` reads spans as disjoint, and the larger
/// deletion has taken the site whole (#10's widening of a def #32 deletes,
/// #5's lift or #33's annotation on a def #48 folds, a #35 hoist out of a
/// def #32 deletes, whose import transport would otherwise land orphaned at
/// the top of the file).
pub fn compose(findings: Vec<Finding>) -> Vec<Finding> {
    let taken: Vec<HashSet<(Rel, u32)>> = findings
        .iter()
        .map(|f| match &f.fix {
            Some(fix) => fix
                .edits
                .iter()
                .filter(|e| takes_line(e))
                .map(|e| (fix.rel.clone(), e.line))
                .collect(),
            None => HashSet::new(),
        })
        .collect();
    let gone: HashSet<&(Rel, u32)> = taken.iter().flatten().collect();

    findings
        .into_iter()
        .enumerate()
        .map(|(i, mut f)| {
            let Some(fix) = &f.fix else { return f };
            let lines: HashSet<(Rel, u32)> = fix
                .edits
                .iter()
                .map(|e| (fix.rel.clone(), e.line))
                .collect();
            let inside = fix
                .edits
                .iter()
                .any(|e| !takes_line(e) && gone.contains(&(fix.rel.clone(), e.line)));
            let swallowed = taken
                .iter()
                .enumerate()
                .any(|(j, t)| j != i && lines.len() < t.len() && lines.is_subset(t));
            if inside || swallowed {
                f.fix = None;
            }
            f
        })
        .collect()
}

/// The `# sightline-fix:` lines naming what a patch discharges, sorted.
/// `git apply` ignores text before the first diff header.
pub fn headers(findings: &[Finding]) -> Vec<String> {
    let mut out: Vec<String> = findings
        .iter()
        .filter(|f| f.fix.is_some())
        .map(|f| format!("# sightline-fix: {} {}\n", f.rule, f.cause))
        .collect();
    out.sort();
    out
}

/// `difflib._format_range_unified`: an empty range begins at the line just
/// before it, and a one-line range prints no length.
fn range(start: usize, stop: usize) -> String {
    let length = stop - start;
    match length {
        1 => format!("{}", start + 1),
        0 => format!("{start},0"),
        _ => format!("{},{length}", start + 1),
    }
}

/// Unified diff of two line lists, each line holding its own terminator.
/// Three lines of context, `a/`/`b/` names, and an unterminated last line
/// marked as `git apply` and `diff` mark it.
pub fn unified_diff<S: AsRef<str>>(old: &[S], new: &[S], rel: &str) -> String {
    let old: Vec<&str> = old.iter().map(AsRef::as_ref).collect();
    let new: Vec<&str> = new.iter().map(AsRef::as_ref).collect();
    let groups = group_diff_ops(capture_diff_slices(Algorithm::Myers, &old, &new), 3);
    if groups.is_empty() {
        return String::new();
    }

    let mut out = format!("--- a/{rel}\n+++ b/{rel}\n");
    for group in &groups {
        let (first, last) = (group[0], group[group.len() - 1]);
        out.push_str(&format!(
            "@@ -{} +{} @@\n",
            range(first.old_range().start, last.old_range().end),
            range(first.new_range().start, last.new_range().end),
        ));
        for op in group {
            let tag = op.tag();
            if tag == DiffTag::Equal {
                for line in &old[op.old_range()] {
                    push(&mut out, ' ', line);
                }
                continue;
            }
            if matches!(tag, DiffTag::Delete | DiffTag::Replace) {
                for line in &old[op.old_range()] {
                    push(&mut out, '-', line);
                }
            }
            if matches!(tag, DiffTag::Insert | DiffTag::Replace) {
                for line in &new[op.new_range()] {
                    push(&mut out, '+', line);
                }
            }
        }
    }
    out
}

fn push(out: &mut String, sign: char, line: &str) {
    out.push(sign);
    out.push_str(line);
    if !line.ends_with('\n') {
        out.push_str("\n\\ No newline at end of file\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::edits::blank;
    use crate::findings::tests::{ast, finding};
    use crate::findings::{Finding, Fix, Site, SpanEdit};

    fn with_fix(rel: &str, edits: Vec<SpanEdit>, imports: &[&str]) -> Finding {
        Finding {
            site: Site {
                rel: rel.into(),
                line: 1,
                col: 0,
                symbol: "m.f".into(),
            },
            fix: Some(Fix {
                rel: rel.into(),
                edits,
                imports: imports.iter().map(|s| s.to_string()).collect(),
            }),
            ..finding("5", ast())
        }
    }

    #[test]
    fn a_deletion_inside_another_loses_its_patch_and_its_imports() {
        // `test_emit.py`
        let lines = ["def dead():", "    import os", "    return os"];
        let dead = with_fix("m.py", blank(&lines, 1, 3), &[]);
        let hoist = with_fix("m.py", blank(&lines, 2, 2), &["import os"]);
        let composed = compose(vec![hoist, dead.clone()]);
        assert!(composed[0].fix.is_none());
        assert_eq!(composed[1].fix, dead.fix);
        // equal deletions are neither inside the other: both keep their patch
        let twins = compose(vec![dead.clone(), dead]);
        assert!(twins.iter().all(|f| f.fix.is_some()));
    }

    #[test]
    fn an_edit_on_a_deleted_line_loses_its_patch() {
        let lines = ["def dead():", "    return 1"];
        let dead = with_fix("m.py", blank(&lines, 1, 2), &[]);
        let inside = with_fix(
            "m.py",
            vec![SpanEdit {
                line: 2,
                col_start: 11,
                col_end: 12,
                text: "2".into(),
            }],
            &[],
        );
        let composed = compose(vec![inside, dead]);
        assert!(composed[0].fix.is_none());
        assert!(composed[1].fix.is_some());
        // another file's deletion on the same line number takes nothing
        let elsewhere = with_fix("other.py", blank(&lines, 1, 2), &[]);
        let far = with_fix(
            "m.py",
            vec![SpanEdit {
                line: 2,
                col_start: 11,
                col_end: 12,
                text: "2".into(),
            }],
            &[],
        );
        assert!(compose(vec![far, elsewhere])[0].fix.is_some());
    }

    #[test]
    fn the_headers_name_every_patched_finding_sorted() {
        let mut later = with_fix("m.py", blank(&["x"], 1, 1), &[]);
        later.rule = "32";
        later.cause = "dead-symbol:m._x".into();
        let mut plain = later.clone();
        plain.fix = None;
        plain.cause = "unfixable".into();
        assert_eq!(
            headers(&[later, plain, with_fix("m.py", blank(&["x"], 1, 1), &[])]),
            [
                "# sightline-fix: 32 dead-symbol:m._x\n",
                "# sightline-fix: 5 c\n",
            ]
        );
    }

    #[test]
    fn an_edit_inside_one_line_diffs_as_one_hunk() {
        let old = ["def f(xs):\n", "    return list(xs)\n"];
        let new = ["def f(xs: list[int]):\n", "    return list(xs)\n"];
        assert_eq!(
            unified_diff(&old, &new, "m.py"),
            "--- a/m.py\n\
             +++ b/m.py\n\
             @@ -1,2 +1,2 @@\n\
             -def f(xs):\n\
             +def f(xs: list[int]):\n\
             \x20    return list(xs)\n"
        );
    }

    #[test]
    fn an_unterminated_last_line_is_marked_on_the_side_that_has_it() {
        let old = ["a\n", "b"];
        let new = ["a\n", "b\n"];
        let diff = unified_diff(&old, &new, "m.py");
        assert!(diff.contains("-b\n\\ No newline at end of file\n"));
        assert!(diff.ends_with("+b\n"));
    }

    #[test]
    fn an_insertion_at_the_top_begins_the_range_before_it() {
        let old = ["a\n"];
        let new = ["import os\n", "a\n"];
        assert_eq!(
            unified_diff(&old, &new, "m.py"),
            "--- a/m.py\n+++ b/m.py\n@@ -1 +1,2 @@\n+import os\n a\n"
        );
    }

    #[test]
    fn two_equal_line_lists_diff_to_nothing() {
        let same = ["a\n", "b\n"];
        assert_eq!(unified_diff(&same, &same, "m.py"), "");
    }

    #[test]
    fn a_whole_file_deletion_prints_a_zero_length_new_range() {
        let old = ["a\n"];
        let new: [&str; 0] = [];
        assert_eq!(
            unified_diff(&old, &new, "m.py"),
            "--- a/m.py\n+++ b/m.py\n@@ -1 +0,0 @@\n-a\n"
        );
    }
}
