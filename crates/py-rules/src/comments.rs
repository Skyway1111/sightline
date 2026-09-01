//! Family C, comment discipline (port of `rules/comments.py`, #39): the
//! restates arms (a comment against its code line, a one-line docstring
//! against its def's name, a dunder's docstring against its protocol's
//! vocabulary). #34 owns the commented-out-code class.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use ruff_python_ast::{Expr, Stmt};

use sightline_core::edits::{blank, char_slice};
use sightline_core::findings::{Evidence, Finding, Sink, Site, SpanEdit};
use sightline_core::pytext;
use sightline_core::rule::{Posture, RuleRecord, Scope};
use sightline_py_facts::astutil::name_tokens;
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{RepoFacts, Symbol};
use sightline_py_facts::module::{Comment, Module};
use sightline_py_provers::Provers;
use sightline_py_provers::comments::{body_of, docstring};
use sightline_py_provers::counterfactual::Splice;

use crate::model::Rule;
use crate::util::{decorator_lines, enclosing_at_line};

static LICENSE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)copyright|licen[cs]e|permission is hereby|redistribution")
        .expect("a pattern copied from `rules/comments.py`")
});
static EXEMPT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"sightline-(ok|fix)|noqa|\b(type|pyright|mypy|pylint|flake8):|^#!")
        .expect("a pattern copied from `rules/comments.py`")
});
/// A bare or labelled banner.
static DIVIDER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^#\s*[-=~*#]{3,}(\s|$)").expect("a pattern copied from `rules/comments.py`")
});
static WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z]+").expect("a pattern copied from `rules/comments.py`"));

const STOPWORDS: &str = "the a an to of in for is are it its this that and or with on at by as \
                         we into from then when each per not";
/// What a one-line docstring may spend beside its def's own name.
const FUNCTION_WORDS: &str = "the a an this given of for to and or return returns";
const FILLER: &str = "it its self object instance class method magic special dunder \
                      use used uses using as in on so we can be is are will";

/// A dunder's docstring restates when every word is one the protocol itself
/// spells, or a name already written at the def (its class and the bases that
/// class heads with), or filler. One word outside is prose about this
/// implementation, what the protocol cannot say, and is silent.
const PROTOCOL_WORDS: [(&str, &str); 16] = [
    (
        "__init__",
        "initialize initialise init construct constructor create \
         new instance set setup up state attribute field default",
    ),
    (
        "__enter__",
        "enter context manager with statement create open begin start acquire",
    ),
    (
        "__exit__",
        "exit context manager with statement close leave end finish",
    ),
    (
        "__str__",
        "str string printable print form format representation \
         human readable display text",
    ),
    (
        "__repr__",
        "repr representation string str printable print form format debug display text",
    ),
    ("__len__", "len length size count number item element"),
    (
        "__iter__",
        "iter iterate iteration iterator loop over item element yield",
    ),
    ("__next__", "next iterator iteration item element advance"),
    (
        "__eq__",
        "eq equal equality equals compare comparison same identical other",
    ),
    ("__hash__", "hash hashable key"),
    ("__bool__", "bool boolean truth truthy true false empty"),
    ("__call__", "call callable invoke run execute"),
    (
        "__contains__",
        "contain contains containment membership member test check item element",
    ),
    (
        "__getitem__",
        "get item index key subscript access lookup slice element",
    ),
    (
        "__setitem__",
        "set item index key subscript assign store element value",
    ),
    (
        "__delitem__",
        "delete del item index key subscript remove element",
    ),
];

fn words_of(text: &str) -> HashSet<String> {
    WORD_RE
        .find_iter(text)
        .map(|m| m.as_str())
        .filter(|w| w.len() > 1)
        .map(pytext::lower)
        .collect()
}

/// Python `any(ch.isdigit() for ch in text)` (R6: `isdigit` is `Nd` here).
fn has_digit(text: &str) -> bool {
    let mut buf = [0u8; 4];
    for c in text.chars() {
        if pytext::is_digit(c.encode_utf8(&mut buf)) {
            return true;
        }
    }
    false
}

/// Plural and -ing/-ed folded onto the base (`users`/`user`, `loading`/`load`,
/// `parsed`/`parse`, `entries`/`entry`); a trailing `e` and a doubled
/// consonant the suffix exposed go too (`fitting`/`fit`).
fn stem(word: &str) -> String {
    // every word here is `[A-Za-z]+`, so a byte length is a character count
    let plural = format!("{}y", word.strip_suffix("ies").unwrap_or(word));
    let mut w: &str = if word.ends_with("ies") && word.len() > 4 {
        &plural
    } else {
        ["ing", "ed", "es", "s"]
            .into_iter()
            .find(|suf| word.ends_with(suf) && word.len() - suf.len() >= 3)
            .map_or(word, |suf| &word[..word.len() - suf.len()])
    };
    w = w.strip_suffix('e').unwrap_or(w);
    let bytes = w.as_bytes();
    if bytes.len() >= 3 {
        let last = bytes[bytes.len() - 1];
        if last == bytes[bytes.len() - 2] && !b"aeiou".contains(&last) {
            w = &w[..w.len() - 1];
        }
    }
    w.to_string()
}

/// Judgeable for restatement: one line, and no digit (a digit is a fact no
/// name holds).
fn one_line_prose(doc: &str) -> bool {
    !pytext::strip(doc).contains('\n') && !has_digit(doc)
}

fn stems(words: impl IntoIterator<Item = String>) -> HashSet<String> {
    words.into_iter().map(|w| stem(&w)).collect()
}

/// A one-line docstring whose content words the def's own name already spells
/// (snake/camel split, stems folded) beside function words only:
/// `"""Get the user."""` on `get_user`.
fn restates_name(doc: &str, name: &str) -> bool {
    if !one_line_prose(doc) {
        return false;
    }
    let function_words: HashSet<&str> = FUNCTION_WORDS.split_whitespace().collect();
    let words = stems(
        words_of(doc)
            .into_iter()
            .filter(|w| !function_words.contains(w.as_str())),
    );
    let spelled = stems(name_tokens(name));
    !words.is_empty() && words.is_subset(&spelled)
}

/// Word tokens of the names already written where a method is defined, its
/// class and the bases that class heads with, read off the class header so
/// single-file and full-repo facts agree. `None`: not a method of a class.
fn site_names(facts: &RepoFacts<'_>, sym: &Symbol) -> Option<HashSet<String>> {
    let owner = facts.symbols.get(sym.parent.as_deref()?)?;
    if owner.kind != "class" {
        return None;
    }
    let module = facts.modules.get(&owner.module)?;
    let Cn::Stmt(Stmt::ClassDef(cls)) = module.nodes[owner.node as usize] else {
        return None;
    };
    let bases = cls.arguments.as_ref().map_or(&[][..], |a| &a.args);
    let heads =
        std::iter::once(owner.name.to_string()).chain(bases.iter().filter_map(|b| match b {
            Expr::Attribute(a) => Some(a.attr.to_string()),
            Expr::Name(n) => Some(n.id.to_string()),
            _ => None,
        }));
    Some(heads.flat_map(|head| name_tokens(&head)).collect())
}

/// `"""Method used for with statement"""` on `__enter__`: a dunder docstring
/// every word of which the protocol, the class's own name or filler already
/// spells.
fn restates_protocol(facts: &RepoFacts<'_>, sym: &Symbol, doc: &str) -> bool {
    let Some((_, protocol)) = PROTOCOL_WORDS.iter().find(|(name, _)| *name == &*sym.name) else {
        return false;
    };
    let Some(site) = site_names(facts, sym) else {
        return false;
    };
    if !one_line_prose(doc) {
        return false;
    }
    let allowed = stems(
        protocol
            .split_whitespace()
            .chain(FUNCTION_WORDS.split_whitespace())
            .chain(FILLER.split_whitespace())
            .map(str::to_string)
            .chain(site),
    );
    let words = stems(words_of(doc));
    !words.is_empty() && words.is_subset(&allowed)
}

/// The comment tokens #39 judges: tool directives and license headers out.
/// Contiguous standalone comments around a license marker above the module's
/// first statement, its docstring aside, are mandated boilerplate, and a
/// `license` in a body governs nothing.
fn governed_comments<'m>(module: &'m Module<'_>) -> Vec<&'m Comment> {
    let standalone = &module.standalone_comments;
    let header_end = module
        .parsed
        .syntax()
        .body
        .iter()
        .find(|st| !matches!(st, Stmt::Expr(e) if Cn::Expr(&e.value).kind() == Kind::Constant))
        .and_then(|st| Cn::Stmt(st).stamped())
        .map_or(module.lines.len() as u32 + 1, |at| module.line_of(at));
    let mut licensed: HashSet<u32> = HashSet::new();
    for t in &module.comments {
        if t.line < header_end && standalone.contains(&t.line) && LICENSE_RE.is_match(&t.text) {
            for step in [1i64, -1] {
                let mut line = t.line as i64;
                while line > 0 && standalone.contains(&(line as u32)) {
                    licensed.insert(line as u32);
                    line += step;
                }
            }
        }
    }
    module
        .comments
        .iter()
        .filter(|t| !EXEMPT_RE.is_match(&t.text) && !licensed.contains(&t.line))
        .collect()
}

/// Not judged for restatement: a standalone comment next to another (the tail
/// of a wrapped why-comment) or any comment beside a `# ---` divider (a
/// section banner is navigation, not restatement).
fn beside_comment(module: &Module<'_>, line: u32, own: bool) -> bool {
    let at = line as usize;
    let before = module.lines.get(at.saturating_sub(2)..at - 1);
    let after = module.lines.get(at..at + 1);
    before.into_iter().chain(after).flatten().any(|s| {
        pytext::lstrip(s).starts_with('#') && (own || DIVIDER_RE.is_match(pytext::strip(s)))
    })
}

/// Every content word of the comment already appears in the code line. A
/// comment with digits stays informative (derivations like `449.1 x 0.45 =
/// 202.1` share only their alpha words with the assert).
fn restates(comment: &str, code: &str) -> bool {
    if has_digit(comment) {
        return false;
    }
    let stopwords: HashSet<&str> = STOPWORDS.split_whitespace().collect();
    let words: HashSet<String> = words_of(comment)
        .into_iter()
        .filter(|w| !stopwords.contains(w.as_str()))
        .collect();
    words.len() >= 2 && words.is_subset(&name_tokens(code))
}

/// Lines a comment above them labels rather than annotates: a `def` / `class`
/// header (decorators included) takes the banner of the block it opens, and an
/// `assert` takes the name of the case its literals encode. Neither is a
/// statement whose words the comment could be restating.
fn labelled_lines(module: &Module<'_>) -> HashSet<u32> {
    let mut lines: HashSet<u32> = module
        .nodes(&[Kind::Assert], None, false)
        .into_iter()
        .map(|at| module.line_of(at))
        .collect();
    for at in module.nodes(
        &[Kind::FunctionDef, Kind::AsyncFunctionDef, Kind::ClassDef],
        None,
        false,
    ) {
        lines.insert(module.line_of(at));
        lines.extend(decorator_lines(module, at));
    }
    lines
}

fn restates_arm(facts: &RepoFacts<'_>, module: &Module<'_>, governed: &[&Comment], out: &mut Sink) {
    let labelled = labelled_lines(module);
    for t in governed {
        let own = module.standalone_comments.contains(&t.line);
        if DIVIDER_RE.is_match(&t.text) || beside_comment(module, t.line, own) {
            continue;
        }
        // the code a comment annotates: its own line, or the next code line
        let below = module.lines[t.line as usize..]
            .iter()
            .enumerate()
            .find(|(_, s)| !pytext::strip(s).is_empty() && !pytext::lstrip(s).starts_with('#'))
            .map(|(n, _)| t.line + 1 + n as u32);
        let code: Option<&str> = if !own {
            Some(char_slice(
                module.lines[t.line as usize - 1],
                0,
                t.col as usize,
            ))
        } else {
            below
                .filter(|n| !labelled.contains(n))
                .map(|n| module.lines[n as usize - 1])
        };
        if !code.is_some_and(|code| restates(&t.text, code)) {
            continue;
        }
        out.push(Finding {
            rule: "39",
            site: Site {
                rel: module.rel.clone(),
                line: t.line,
                col: t.col,
                symbol: enclosing_at_line(facts, module, t.line).into(),
            },
            message: format!(
                "comment {} restates the code it annotates",
                pytext::repr_str(pytext::strip(pytext::lstrip_chars(&t.text, "# ")))
            ),
            cause: format!("comment-restates:{}:{}", module.qname, t.line),
            evidence: Evidence::ast(),
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

pub const RULE_39: Rule = Rule {
    record: RuleRecord {
        id: "39",
        slug: "comment-discipline",
        family: "C",
        engine_class: "AST",
        posture: Posture::Ratchet,
        meaning: "comments restating code; a one-line docstring restating the def's \
                  name or a dunder's protocol",
        goal: "Comments carry only what the code cannot: a restatement is \
               context every reader pays for.",
        lang: "py",
        scope: Scope::File,
        complement: "",
    },
    run: rule_39,
};

/// Comments that restate their code, docstrings that restate their def's name
/// or their dunder's protocol.
fn rule_39(facts: &RepoFacts<'_>, _provers: &Provers, out: &mut Sink) {
    for module in facts.modules.values() {
        restates_arm(facts, module, &governed_comments(module), out);
    }
    for sym in facts.symbols.values() {
        if sym.kind == "variable" {
            continue;
        }
        let Some(module) = facts.modules.get(&sym.module) else {
            continue;
        };
        let doc = body_of(module, sym.node)
            .and_then(docstring)
            .unwrap_or_default();
        if doc.is_empty() {
            continue;
        }
        let (what, cause) = if restates_name(&doc, &sym.name) {
            (
                format!("restates the name {}", sym.name),
                "docstring-restates",
            )
        } else if restates_protocol(facts, sym, &doc) {
            (
                format!("restates what {} means", sym.name),
                "dunder-restates",
            )
        } else {
            continue;
        };
        let Some(head) = body_of(module, sym.node)
            .and_then(|body| body.first())
            .and_then(|st| Cn::Stmt(st).stamped())
        else {
            continue;
        };
        let span = module.span(head);
        out.push(Finding {
            rule: "39",
            site: Site {
                rel: module.rel.clone(),
                line: span.and_then(|s| s[0]).unwrap_or(1),
                col: span.and_then(|s| s[1]).unwrap_or(0),
                symbol: sym.qname.clone(),
            },
            message: format!("docstring {} {what}", pytext::repr_str(pytext::strip(&doc))),
            cause: format!("{cause}:{}", sym.qname),
            evidence: Evidence::ast(),
            salience: 0.0,
            fix: None,
            lang: "py",
        });
    }
}

/// #39's restating comment as a patch: the whole line where the comment owns
/// it, the comment's own text where it shares one with code; a docstring arm
/// names no comment line and gets none.
pub fn comment_splice(cause: &str, facts: &RepoFacts<'_>, _provers: &Provers) -> Option<Splice> {
    let parts: Vec<&str> = cause.split(':').collect();
    if parts.len() != 3 || parts[0] != "comment-restates" {
        return None;
    }
    let module = facts.modules.get(parts[1])?;
    let line: u32 = parts[2].parse().ok()?;
    let tok = module.comments.iter().find(|t| t.line == line)?;
    let text = module.lines.get(line as usize - 1)?;
    // R17: the columns are code-point indexes into the line, as the Python
    // site's `tokenize` column and `len(str)` are
    let edits = if module.standalone_comments.contains(&line) {
        blank(&module.lines, line, line)
    } else {
        vec![SpanEdit {
            line,
            col_start: pytext::rstrip(char_slice(text, 0, tok.col as usize))
                .chars()
                .count() as u32,
            col_end: text.chars().count() as u32,
            text: String::new(),
        }]
    };
    Some(Splice {
        id: cause.to_string(),
        owner: module.qname.to_string(),
        edits,
        spelling: String::new(),
        imports: Vec::new(),
        param: String::new(),
    })
}
