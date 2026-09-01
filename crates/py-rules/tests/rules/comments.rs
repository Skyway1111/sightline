//! #39 comment discipline: the restatement and divider arms
//! (`tests/rules/test_comments.py`), and the splice `fix` rides
//! (`tests/test_fixes.py`, the #39 pair).

use sightline_core::edits::apply_edits;
use sightline_core::findings::Finding;
use sightline_py_provers::Provers;
use sightline_py_rules::comments::comment_splice;
use sightline_testkit::{build, run_rule};

fn causes(findings: &[Finding], holding: &str) -> Vec<String> {
    findings
        .iter()
        .filter(|f| f.cause.contains(holding))
        .map(|f| f.cause.clone())
        .collect()
}

#[test]
fn restating_comments_fire() {
    let found = run_rule(
        "39",
        &[(
            "m.py",
            concat!(
                "def handle(user):\n",
                "    # save the user\n",
                "    save(user)\n",
                "    return self.name  # return the name\n",
                "def save(user):\n",
                "    return user\n",
            ),
        )],
    );
    assert_eq!(
        causes(&found, "restates"),
        ["comment-restates:m:2", "comment-restates:m:4"]
    );
}

#[test]
fn informative_and_exempt_comments_stay_silent() {
    let found = run_rule(
        "39",
        &[(
            "m.py",
            concat!(
                "def handle(user, lock):\n",
                "    # guard against reentrancy during rollback\n",
                "    save(user)\n",
                "    x = user.load()  # type: ignore\n",
                "    return x  # sightline-ok: 12\n",
                "def save(user):\n",
                "    return user\n",
            ),
        )],
    );
    assert!(found.is_empty(), "{:?}", causes(&found, ""));
}

/// A wrapped continuation line of a multi-line why-comment must not be
/// compared against the next code line. A single-line standalone restatement
/// still fires.
#[test]
fn a_block_continuation_is_not_judged_as_restatement() {
    let found = run_rule(
        "39",
        &[(
            "m.py",
            concat!(
                "def handle(user):\n",
                "    # we must persist before notifying because the queue\n",
                "    # consumer reads the saved user row\n",
                "    save(user)\n",
                "def again(user):\n",
                "    # save the user\n",
                "    save(user)\n",
                "def save(user):\n",
                "    return user\n",
            ),
        )],
    );
    assert_eq!(causes(&found, "restates"), ["comment-restates:m:6"]);
}

/// A comment over a def or class heads the block it opens (a section banner in
/// a long class), and a comment over an assert names the case its literals
/// encode; the annotating twin still fires.
#[test]
fn a_label_over_a_def_or_an_assert_is_not_restatement() {
    let found = run_rule(
        "39",
        &[(
            "m.py",
            concat!(
                "class Store:\n",
                "    # document processing\n",
                "    def _init_processing(self, document):\n",
                "        return document\n",
                "def test_extract(x):\n",
                "    # extract is none\n",
                "    assert extract('') is None\n",
                "    # extract the value\n",
                "    value = extract(x)\n",
                "    return value\n",
                "def extract(x):\n",
                "    return x or None\n",
            ),
        )],
    );
    assert_eq!(causes(&found, "restates"), ["comment-restates:m:8"]);
}

/// Stems fold: `Loads the given row` on `load_rows`, `Stores users` on
/// `UserStore`; a content word the name lacks (`Row`), a second line or a
/// digit keeps the docstring informative.
#[test]
fn a_one_line_docstring_restating_the_name_fires() {
    let found = run_rule(
        "39",
        &[(
            "m.py",
            concat!(
                "def get_user(uid):\n    \"\"\"Get the user.\"\"\"\n    return uid\n",
                "def load_rows(path):\n    \"\"\"Loads the given row.\"\"\"\n    return path\n",
                "class UserStore:\n    \"\"\"Stores users.\"\"\"\n",
                "def parse_line(s):\n    \"\"\"Parse a line into a Row.\"\"\"\n    return s\n",
                "def save(x):\n    \"\"\"Save.\n\n    Persists x to disk.\n    \"\"\"\n    return x\n",
                "def get_id(x):\n    \"\"\"Get id 2.\"\"\"\n    return x\n",
                "def go(x):\n    \"\"\"Returns.\"\"\"\n    return x\n",
            ),
        )],
    );
    let docs: Vec<&Finding> = found
        .iter()
        .filter(|f| f.cause.starts_with("docstring-restates:"))
        .collect();
    assert_eq!(
        docs.iter().map(|f| f.cause.clone()).collect::<Vec<_>>(),
        [
            "docstring-restates:m.get_user",
            "docstring-restates:m.load_rows",
            "docstring-restates:m.UserStore",
        ]
    );
    assert_eq!(
        (docs[0].site.line, &*docs[0].site.symbol),
        (2, "m.get_user")
    );
    assert_eq!(
        docs[0].message,
        "docstring 'Get the user.' restates the name get_user"
    );
}

/// The g3 pair, from asciimatics: the protocol's own vocabulary plus the names
/// at the def (`parser`, the `Exception` the class heads with) against prose
/// about this implementation.
#[test]
fn a_dunder_docstring_spelling_only_its_protocol_fires() {
    let src = [
        "class ManagedScreen:",
        "    def __enter__(self):",
        "        \"\"\"Method used for with statement\"\"\"",
        "        return self",
        "    def __exit__(self, etype, value, tb):",
        "        \"\"\"Clear up the resources for this context.\"\"\"",
        "        return None",
        "class ResizeScreenError(Exception):",
        "    def __str__(self):",
        "        \"\"\"Printable form of the exception.\"\"\"",
        "        return self._message",
        "class Parser:",
        "    def __init__(self):",
        "        \"\"\"Initialize the parser.\"\"\"",
        "        self._state = None",
        "class ColouredText:",
        "    def __repr__(self):",
        "        \"\"\"Return the processed text.\"\"\"",
        "        return self._text",
        "class DropScreen:",
        "    def __init__(self, screen):",
        "        \"\"\"See ParticleEffect for details of the parameters.\"\"\"",
        "        self._screen = screen",
        "",
    ]
    .join("\n");
    let found = run_rule("39", &[("m.py", &src)]);
    let dunders: Vec<&Finding> = found
        .iter()
        .filter(|f| f.cause.starts_with("dunder-restates:"))
        .collect();
    assert_eq!(
        dunders.iter().map(|f| f.cause.clone()).collect::<Vec<_>>(),
        [
            "dunder-restates:m.ManagedScreen.__enter__",
            "dunder-restates:m.ResizeScreenError.__str__",
            "dunder-restates:m.Parser.__init__",
        ]
    );
    assert_eq!(
        dunders[0].message,
        "docstring 'Method used for with statement' restates what __enter__ means"
    );
}

/// Only the protocols the table spells are judged, and only on a method: a
/// module-level `__call__` has no class name at its site.
#[test]
fn a_dunder_outside_the_table_and_a_module_level_one_stay_silent() {
    let found = run_rule(
        "39",
        &[(
            "m.py",
            concat!(
                "class S:\n",
                "    def __get__(self, obj, objtype):\n",
                "        \"\"\"Class decorator method.\"\"\"\n",
                "        return obj\n",
                "def __call__(name):\n",
                "        \"\"\"Method used to call it.\"\"\"\n",
                "        return name\n",
            ),
        )],
    );
    assert!(
        !found
            .iter()
            .any(|f| f.cause.starts_with("dunder-restates:"))
    );
}

const COMMENTS: &str = concat!(
    "def f(rows: int) -> int:\n",
    "    # rows total\n",
    "    rows_total = rows + 1\n",
    "    return rows_total  # return rows total\n",
);

/// The comment fix takes the line it owns and only the text it shares
/// (`tests/test_fixes.py`, the #39 pair, read off the splice the emitter
/// attaches).
#[test]
fn the_comment_splice_takes_the_line_it_owns_and_only_the_shared_text() {
    let (_dir, stack) = build(&[("m.py", COMMENTS)]);
    let bare = Provers::bare(stack.facts());
    let patched = |cause: &str| -> Vec<String> {
        let splice = comment_splice(cause, stack.facts(), &bare).expect("#39 builds a splice");
        let mut lines: Vec<String> = COMMENTS.lines().map(str::to_string).collect();
        apply_edits(&mut lines, &splice.edits);
        lines
    };
    assert_eq!(patched("comment-restates:m:2")[1], "");
    assert_eq!(patched("comment-restates:m:4")[3], "    return rows_total");
}
