//! The overlays one world holds: each edited file's whole text, and the file
//! cut the checker's pass over that world must reach.

use super::*;

/// The file with the splices applied; imports ride an existing line.
pub(super) fn world_content(source: &str, body: &[Stmt], props: &[&Proposal]) -> String {
    let mut lines: Vec<String> = source_lines(source)
        .iter()
        .map(|l| (*l).to_string())
        .collect();
    let edits: Vec<SpanEdit> = props.iter().flat_map(|p| p.edits.iter().cloned()).collect();
    apply_edits(&mut lines, &edits);
    let imports = merge_imports(props.iter().flat_map(|p| p.imports.iter().cloned()));
    if !imports.is_empty() {
        ride_import_line(&mut lines, &imports.join("; "), source, body);
    }
    lines.join("\n") + "\n"
}

/// Line count preserved (the shim diffs by line): ahead of the first
/// non-`__future__` top-of-file import, else a blank line before the first
/// statement, else EOF (unresolved below 3.14: an honest veto).
fn ride_import_line(lines: &mut Vec<String>, stmt: &str, source: &str, body: &[Stmt]) {
    let index = Lines::new(source);
    // ruff's node range starts at the first decorator, which is what
    // `min(node.lineno, *decorators)` reads in CPython (R1)
    let line_of = |st: &Stmt| index.pos(st.range().start().to_u32()).0 as usize;
    let past = fn_body(body);
    let mut stop = lines.len() + 1;
    for st in past {
        let home = match st {
            Stmt::ImportFrom(from) => from
                .module
                .as_ref()
                .map(ruff_python_ast::Identifier::as_str),
            Stmt::Import(_) => None,
            _ => {
                stop = line_of(st);
                break;
            }
        };
        if home != Some("__future__") {
            let at = line_of(st) - 1;
            lines[at] = format!("{stmt}; {}", lines[at]);
            return;
        }
    }
    let start = if past.len() < body.len() {
        index.pos(body[0].range().end().to_u32()).0 as usize
    } else {
        0
    };
    for line in lines.iter_mut().take(stop.saturating_sub(1)).skip(start) {
        if line.chars().all(pytext::is_py_space) {
            *line = stmt.to_string();
            return;
        }
    }
    lines.push(stmt.to_string());
}

/// The files one world's check must reach: the union of the group's watched
/// sets plus each overlay's own file, in first-seen order (codemap 5).
/// `None` where a member watches every file, which asks for a whole check.
pub(super) fn files_of(group: &[&Proposal]) -> Option<IndexSet<Rel>> {
    let mut out: IndexSet<Rel> = IndexSet::new();
    for p in group {
        let mut watched: Vec<&String> = p.watched.as_ref()?.iter().collect();
        watched.sort();
        out.extend(watched.into_iter().map(|f| Rel::from(f.as_str())));
        out.insert(p.rel.clone());
    }
    Some(out)
}

pub(super) fn union_cuts(cuts: Vec<Option<IndexSet<Rel>>>) -> Option<IndexSet<Rel>> {
    let mut out: IndexSet<Rel> = IndexSet::new();
    for cut in cuts {
        out.extend(cut?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    use ruff_python_parser::parse_module;

    use crate::counterfactual::fixtures::{edit, proposal};

    fn content(source: &str, props: &[&Proposal]) -> String {
        let parsed = parse_module(source).expect("a parsed fixture");
        world_content(source, &parsed.syntax().body, props)
    }

    #[test]
    fn an_import_rides_the_first_non_future_import_line() {
        let source = "from __future__ import annotations\nimport os\n\n\ndef f():\n    return os\n";
        let p = Proposal {
            imports: vec!["from typing import Optional".to_string()],
            ..proposal("p", "m.py", (5, 6), vec![edit(6, 11, 13, "os.sep")])
        };
        assert_eq!(
            content(source, &[&p]).lines().collect::<Vec<_>>(),
            [
                "from __future__ import annotations",
                "from typing import Optional; import os",
                "",
                "",
                "def f():",
                "    return os.sep",
            ]
        );
    }

    #[test]
    fn an_import_takes_a_blank_line_when_the_file_imports_nothing() {
        // the docstring's own line is never taken: the search starts past it
        let source = "\"\"\"Doc.\"\"\"\n\n@deco\ndef f():\n    return 1\n";
        let p = Proposal {
            imports: vec!["import os".to_string()],
            ..proposal("p", "m.py", (4, 5), Vec::new())
        };
        assert_eq!(
            content(source, &[&p]).lines().collect::<Vec<_>>(),
            [
                "\"\"\"Doc.\"\"\"",
                "import os",
                "@deco",
                "def f():",
                "    return 1",
            ]
        );
    }

    #[test]
    fn an_import_with_no_room_lands_at_eof() {
        // no import line and no blank line before the first statement: the
        // statement rides at EOF, where the name goes unresolved and the
        // world vetoes honestly
        let source = "def f():\n    return 1\n";
        let p = Proposal {
            imports: vec!["import os".to_string()],
            ..proposal("p", "m.py", (1, 2), Vec::new())
        };
        assert_eq!(
            content(source, &[&p]).lines().collect::<Vec<_>>(),
            ["def f():", "    return 1", "import os"]
        );
    }

    #[test]
    fn a_world_splices_every_proposal_of_its_file_and_ends_in_one_newline() {
        let source = "def f(a, b):\n    return a\n";
        let first = proposal("a", "m.py", (1, 2), vec![edit(1, 7, 7, ": int")]);
        let second = proposal("b", "m.py", (1, 2), vec![edit(1, 10, 10, ": str")]);
        assert_eq!(
            content(source, &[&first, &second]),
            "def f(a: int, b: str):\n    return a\n"
        );
    }

    #[test]
    fn a_splice_past_a_form_feed_lands_on_the_ast_line() {
        // `str.splitlines` breaks at the form feed and the AST does not, so
        // the edit's line 4 is `def f`, not the line before it
        let source = "def a():\n    return 1\n\x0c\ndef f(x):\n    return x\n";
        let p = proposal("p", "m.py", (4, 5), vec![edit(4, 7, 7, ": int")]);
        assert!(content(source, &[&p]).contains("def f(x: int):"));
    }

    #[test]
    fn the_file_cut_is_the_watched_union_plus_every_overlay() {
        let a = Proposal {
            watched: Some(HashSet::from(["c.py".to_string(), "b.py".to_string()])),
            ..proposal("a", "m.py", (1, 2), Vec::new())
        };
        let b = Proposal {
            watched: Some(HashSet::from(["c.py".to_string()])),
            ..proposal("b", "n.py", (1, 2), Vec::new())
        };
        let cut = files_of(&[&a, &b]).expect("both sets are closed");
        assert_eq!(
            cut.iter().map(|r| &**r).collect::<Vec<_>>(),
            ["b.py", "c.py", "m.py", "n.py"]
        );
        // one open set opens the whole cut
        let open = Proposal {
            watched: None,
            ..b.clone()
        };
        assert!(files_of(&[&a, &open]).is_none());
        assert!(union_cuts(vec![files_of(&[&a]), None]).is_none());
    }
}
