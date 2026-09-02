//! What an `except` handler does with the failure - one classifier for #34's
//! prod swallows and family T's verdicts. The builtin exception table R16
//! names lives here too, since it answers both "is this name an exception"
//! and #33's base question.

use ruff_python_ast::{ExceptHandlerExceptHandler, Expr, ExprCall, Number, Stmt, StmtTry};
use sightline_py_facts::astutil::{mentions, walk};
use sightline_py_facts::cn::Cn;
use sightline_py_facts::kinds::Kind;

use crate::catalog::{COLLECTORS, classes_of};
use sightline_core::catalog::LOGS;

/// Calls that carry a test's verdict besides `assert` and `assert*` names
/// (`skip`/`xfail` decide nothing: a placeholder test skips).
const ORACLE_CALLS: [&str; 4] = ["raises", "warns", "deprecated_call", "fail"];

/// The name a call site spells (`f(x)` -> f, `a.b(x)` -> b).
pub fn call_name(call: &ExprCall) -> Option<&str> {
    match &*call.func {
        Expr::Name(n) => Some(n.id.as_str()),
        Expr::Attribute(a) => Some(a.attr.as_str()),
        _ => None,
    }
}

/// Do these nodes carry a verdict: an assert, a raise, an oracle call?
pub fn carries_verdict<'a>(nodes: impl IntoIterator<Item = Cn<'a>>) -> bool {
    nodes.into_iter().any(|n| match n {
        Cn::Stmt(Stmt::Assert(_) | Stmt::Raise(_)) => true,
        Cn::Expr(Expr::Call(c)) => {
            let name = call_name(c).unwrap_or("");
            name.starts_with("assert") || ORACLE_CALLS.contains(&name)
        }
        _ => false,
    })
}

pub fn block_has_verdict(stmts: &[Stmt]) -> bool {
    carries_verdict(stmts.iter().flat_map(|st| walk(Cn::Stmt(st))))
}

/// Every question a rule asks of one `except` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandlerOutcome {
    /// `except:` / Exception / BaseException, alone or in a tuple
    pub broad: bool,
    /// the bound error reaches a non-log call or a store, or the failure is recorded
    pub handles: bool,
    /// the bound error is stored or passed to a collection
    pub records: bool,
    /// a raise anywhere in the body
    pub reraises: bool,
    /// the whole body is `raise` / `raise e`
    pub bare_reraise: bool,
    /// the body asserts, raises or calls an oracle
    pub verdicts: bool,
    /// the body ends `return <default>`
    pub returns_default: bool,
}

pub fn handler_outcome(h: &ExceptHandlerExceptHandler) -> HandlerOutcome {
    let body = &h.body;
    let only = (body.len() == 1).then(|| &body[0]);
    let name = h.name.as_ref().map(|n| n.as_str());
    let broad = match h.type_.as_deref() {
        None => true,
        Some(Expr::Tuple(t)) => t.elts.iter().any(is_broad_name),
        Some(other) => is_broad_name(other),
    };
    let handles = (name.is_some_and(|n| {
        body.iter()
            .flat_map(|st| outside_logs(Cn::Stmt(st)))
            .any(|x| reaches(x, n))
    })) || body
        .iter()
        .flat_map(|st| walk(Cn::Stmt(st)))
        .any(records_failure);
    let bare_reraise = match only {
        Some(Stmt::Raise(r)) => match (&r.exc, &r.cause) {
            (None, _) => true,
            (Some(exc), cause) => {
                matches!(&**exc, Expr::Name(n) if name == Some(n.id.as_str())) && cause.is_none()
            }
        },
        _ => false,
    };
    HandlerOutcome {
        broad,
        handles,
        records: records(h),
        reraises: body
            .iter()
            .flat_map(|st| walk(Cn::Stmt(st)))
            .any(|n| matches!(n, Cn::Stmt(Stmt::Raise(_)))),
        bare_reraise,
        verdicts: block_has_verdict(body),
        returns_default: match body.last() {
            Some(Stmt::Return(r)) => is_default(r.value.as_deref()),
            _ => false,
        },
    }
}

fn is_broad_name(t: &Expr) -> bool {
    matches!(t, Expr::Name(n) if matches!(n.id.as_str(), "Exception" | "BaseException"))
}

/// Does the block leave its function or loop - a `return`, `break` or
/// `continue` of its own? A nested def's return is that def's.
pub fn exits(stmts: &[Stmt]) -> bool {
    let mut stack: Vec<Cn<'_>> = stmts.iter().map(Cn::Stmt).collect();
    let mut kids: Vec<Cn<'_>> = Vec::new();
    while let Some(n) = stack.pop() {
        if matches!(n.kind(), Kind::Return | Kind::Break | Kind::Continue) {
            return true;
        }
        if matches!(
            n.kind(),
            Kind::FunctionDef | Kind::AsyncFunctionDef | Kind::Lambda
        ) {
            continue;
        }
        kids.clear();
        sightline_py_facts::order::children(n, &mut kids);
        stack.extend(kids.iter().copied());
    }
    false
}

/// Single handler whose whole body is `raise` / `raise e`: removing the try
/// changes nothing (multi-handler re-raises are intentional filters).
pub fn noop_try(tr: &StmtTry) -> bool {
    if tr.handlers.len() != 1 || !tr.finalbody.is_empty() {
        return false;
    }
    let ruff_python_ast::ExceptHandler::ExceptHandler(h) = &tr.handlers[0];
    handler_outcome(h).bare_reraise
}

/// The handler keeps the exception for a later verdict: stores it or hands it
/// to a collection (`errors.append(exc)`). Named, not folded: every field of
/// the outcome is one question with one name.
fn records(h: &ExceptHandlerExceptHandler) -> bool {
    let Some(name) = h.name.as_ref().map(|n| n.as_str()) else {
        return false;
    };
    h.body
        .iter()
        .flat_map(|st| walk(Cn::Stmt(st)))
        .filter(|n| match n {
            Cn::Stmt(Stmt::Assign(_) | Stmt::AnnAssign(_)) => true,
            Cn::Expr(Expr::Call(c)) => matches!(
                &*c.func,
                Expr::Attribute(a) if COLLECTORS.contains(a.attr.as_str())
            ),
            _ => false,
        })
        .any(|n| mentions(n, name))
}

/// The handler leaves a mark the caller can read without the error: a store
/// through an attribute or subscript, or a collector call
/// (`failed.append(path)`).
fn records_failure(n: Cn<'_>) -> bool {
    let stored = |t: &Expr| matches!(t, Expr::Attribute(_) | Expr::Subscript(_));
    match n {
        Cn::Stmt(Stmt::Assign(a)) => a.targets.iter().any(stored),
        Cn::Stmt(Stmt::AugAssign(a)) => stored(&a.target),
        Cn::Stmt(Stmt::AnnAssign(a)) => stored(&a.target),
        Cn::Expr(Expr::Call(c)) => matches!(
            &*c.func,
            Expr::Attribute(a) if COLLECTORS.contains(a.attr.as_str())
        ),
        _ => false,
    }
}

/// Descendants not inside a log call (`log.error(str(e))` handles nothing):
/// `print(...)`, `log.<level>(...)`, `traceback.print_exc()`.
fn outside_logs(node: Cn<'_>) -> Vec<Cn<'_>> {
    let mut stack = vec![node];
    let mut out = Vec::new();
    let mut kids: Vec<Cn<'_>> = Vec::new();
    while let Some(n) = stack.pop() {
        if let Cn::Expr(Expr::Call(c)) = n
            && classes_of(None, call_name(c)).contains(LOGS)
        {
            continue;
        }
        out.push(n);
        kids.clear();
        sightline_py_facts::order::children(n, &mut kids);
        stack.extend(kids.iter().copied());
    }
    out
}

/// `name` is stored from, or passed to a call.
fn reaches(n: Cn<'_>, name: &str) -> bool {
    match n {
        Cn::Stmt(Stmt::Assign(a)) => mentions(Cn::Expr(&a.value), name),
        Cn::Stmt(Stmt::AnnAssign(a)) => a
            .value
            .as_deref()
            .is_some_and(|v| mentions(Cn::Expr(v), name)),
        Cn::Expr(Expr::Call(c)) => c
            .arguments
            .args
            .iter()
            .chain(c.arguments.keywords.iter().map(|k| &k.value))
            .any(|a| mentions(Cn::Expr(a), name)),
        _ => false,
    }
}

/// None, zero or an empty literal. A bool is an answer (the EAFP probe
/// `try: ...; return True / except: return False`) and a non-empty string is
/// a failure message.
fn is_default(value: Option<&Expr>) -> bool {
    let Some(value) = value else {
        return true;
    };
    match value {
        Expr::NoneLiteral(_) => true,
        Expr::BooleanLiteral(_) => false,
        // Python compares by `==`, so `0.0` and `0j` equal `0` too
        Expr::NumberLiteral(n) => match &n.value {
            Number::Int(i) => i.as_u64() == Some(0),
            Number::Float(f) => *f == 0.0,
            Number::Complex { real, imag } => *real == 0.0 && *imag == 0.0,
        },
        Expr::StringLiteral(s) => s.value.is_empty(),
        Expr::BytesLiteral(b) => b.value.is_empty(),
        Expr::Dict(d) => d.items.is_empty(),
        Expr::List(l) => l.elts.is_empty(),
        Expr::Tuple(t) => t.elts.is_empty(),
        _ => false,
    }
}

// --- the builtin exception table (R16) --------------------------------------

/// Every name `vars(builtins)` binds to a `BaseException` subclass in CPython
/// 3.14, with its immediate base. Keyed by `__name__`, so the `OSError`
/// aliases (`IOError`, `EnvironmentError`, `WindowsError`) are not names of
/// their own.
/// `ruff_python_stdlib::builtins::is_exception(_, 14)` is not this set: it
/// adds `ImportCycleError` and drops `_IncompleteInputError`.
const EXCEPTIONS: [(&str, &str); 69] = [
    ("ArithmeticError", "Exception"),
    ("AssertionError", "Exception"),
    ("AttributeError", "Exception"),
    ("BaseException", "object"),
    ("BaseExceptionGroup", "BaseException"),
    ("BlockingIOError", "OSError"),
    ("BrokenPipeError", "ConnectionError"),
    ("BufferError", "Exception"),
    ("BytesWarning", "Warning"),
    ("ChildProcessError", "OSError"),
    ("ConnectionAbortedError", "ConnectionError"),
    ("ConnectionError", "OSError"),
    ("ConnectionRefusedError", "ConnectionError"),
    ("ConnectionResetError", "ConnectionError"),
    ("DeprecationWarning", "Warning"),
    ("EOFError", "Exception"),
    ("EncodingWarning", "Warning"),
    ("Exception", "BaseException"),
    ("ExceptionGroup", "BaseExceptionGroup"),
    ("FileExistsError", "OSError"),
    ("FileNotFoundError", "OSError"),
    ("FloatingPointError", "ArithmeticError"),
    ("FutureWarning", "Warning"),
    ("GeneratorExit", "BaseException"),
    ("ImportError", "Exception"),
    ("ImportWarning", "Warning"),
    ("IndentationError", "SyntaxError"),
    ("IndexError", "LookupError"),
    ("InterruptedError", "OSError"),
    ("IsADirectoryError", "OSError"),
    ("KeyError", "LookupError"),
    ("KeyboardInterrupt", "BaseException"),
    ("LookupError", "Exception"),
    ("MemoryError", "Exception"),
    ("ModuleNotFoundError", "ImportError"),
    ("NameError", "Exception"),
    ("NotADirectoryError", "OSError"),
    ("NotImplementedError", "RuntimeError"),
    ("OSError", "Exception"),
    ("OverflowError", "ArithmeticError"),
    ("PendingDeprecationWarning", "Warning"),
    ("PermissionError", "OSError"),
    ("ProcessLookupError", "OSError"),
    ("PythonFinalizationError", "RuntimeError"),
    ("RecursionError", "RuntimeError"),
    ("ReferenceError", "Exception"),
    ("ResourceWarning", "Warning"),
    ("RuntimeError", "Exception"),
    ("RuntimeWarning", "Warning"),
    ("StopAsyncIteration", "Exception"),
    ("StopIteration", "Exception"),
    ("SyntaxError", "Exception"),
    ("SyntaxWarning", "Warning"),
    ("SystemError", "Exception"),
    ("SystemExit", "BaseException"),
    ("TabError", "IndentationError"),
    ("TimeoutError", "OSError"),
    ("TypeError", "Exception"),
    ("UnboundLocalError", "NameError"),
    ("UnicodeDecodeError", "UnicodeError"),
    ("UnicodeEncodeError", "UnicodeError"),
    ("UnicodeError", "ValueError"),
    ("UnicodeTranslateError", "UnicodeError"),
    ("UnicodeWarning", "Warning"),
    ("UserWarning", "Warning"),
    ("ValueError", "Exception"),
    ("Warning", "Exception"),
    ("ZeroDivisionError", "ArithmeticError"),
    ("_IncompleteInputError", "SyntaxError"),
];

/// `name in BUILTIN_EXCEPTIONS`.
pub fn is_exception(name: &str) -> bool {
    EXCEPTIONS.iter().any(|(n, _)| *n == name)
}

/// `issubclass(BUILTIN_EXCEPTIONS[name], BUILTIN_EXCEPTIONS[base])` over the
/// table's single-inheritance chain; false for a name the table does not hold.
pub fn exception_is(name: &str, base: &str) -> bool {
    if !is_exception(base) {
        return false;
    }
    let mut cur = name;
    while let Some((_, up)) = EXCEPTIONS.iter().find(|(n, _)| *n == cur) {
        if cur == base {
            return true;
        }
        cur = up;
    }
    false
}

#[cfg(test)]
mod tests {
    use ruff_python_parser::parse_module;

    use super::*;

    fn one_try(source: &str) -> StmtTry {
        let parsed = parse_module(source).expect("the fixture parses");
        match &parsed.suite()[0] {
            Stmt::Try(t) => t.clone(),
            other => panic!("{other:?} is not a try"),
        }
    }

    fn outcome(source: &str) -> HandlerOutcome {
        let tr = one_try(source);
        let ruff_python_ast::ExceptHandler::ExceptHandler(h) = &tr.handlers[0];
        handler_outcome(h)
    }

    #[test]
    fn a_log_call_does_not_handle_the_error() {
        let logged = outcome("try:\n    f()\nexcept Exception as e:\n    log.error(str(e))\n");
        assert!(logged.broad && !logged.handles && !logged.records);
        let used = outcome("try:\n    f()\nexcept Exception as e:\n    report(e)\n");
        assert!(used.handles);
        let kept = outcome("try:\n    f()\nexcept ValueError as e:\n    errs.append(e)\n");
        assert!(!kept.broad && kept.records && kept.handles);
    }

    #[test]
    fn a_bare_reraise_is_the_whole_body() {
        assert!(outcome("try:\n    f()\nexcept E:\n    raise\n").bare_reraise);
        assert!(outcome("try:\n    f()\nexcept E as e:\n    raise e\n").bare_reraise);
        assert!(!outcome("try:\n    f()\nexcept E as e:\n    raise e from x\n").bare_reraise);
        assert!(!outcome("try:\n    f()\nexcept E:\n    log()\n    raise\n").bare_reraise);
        assert!(noop_try(&one_try("try:\n    f()\nexcept E:\n    raise\n")));
        assert!(!noop_try(&one_try(
            "try:\n    f()\nexcept E:\n    raise\nfinally:\n    g()\n"
        )));
    }

    #[test]
    fn a_default_return_is_none_zero_or_an_empty_literal() {
        for tail in [
            "return",
            "return None",
            "return 0",
            "return ''",
            "return []",
        ] {
            assert!(
                outcome(&format!("try:\n    f()\nexcept E:\n    {tail}\n")).returns_default,
                "{tail}"
            );
        }
        for tail in ["return True", "return 'boom'", "return [1]", "pass"] {
            assert!(
                !outcome(&format!("try:\n    f()\nexcept E:\n    {tail}\n")).returns_default,
                "{tail}"
            );
        }
    }

    #[test]
    fn a_nested_defs_return_is_not_the_blocks_own() {
        let parsed = parse_module("def outer():\n    def inner():\n        return 1\n    x = 1\n")
            .expect("the fixture parses");
        let Stmt::FunctionDef(f) = &parsed.suite()[0] else {
            panic!("not a def")
        };
        assert!(!exits(&f.body));
        let parsed = parse_module("def outer():\n    for x in y:\n        break\n")
            .expect("the fixture parses");
        let Stmt::FunctionDef(f) = &parsed.suite()[0] else {
            panic!("not a def")
        };
        assert!(exits(&f.body));
    }

    #[test]
    fn a_verdict_is_an_assert_a_raise_or_an_oracle_call() {
        let parsed = parse_module("assert x\n").expect("the fixture parses");
        assert!(block_has_verdict(parsed.suite()));
        let parsed = parse_module("pytest.raises(E)\n").expect("the fixture parses");
        assert!(block_has_verdict(parsed.suite()));
        let parsed = parse_module("self.assertEqual(a, b)\n").expect("the fixture parses");
        assert!(block_has_verdict(parsed.suite()));
        let parsed = parse_module("pytest.skip('no')\n").expect("the fixture parses");
        assert!(!block_has_verdict(parsed.suite()));
    }

    /// R16: the table is `vars(builtins)` keyed by `__name__`, so the
    /// `OSError` aliases are no names of their own, and ruff's own list
    /// differs by two rows.
    #[test]
    fn the_builtin_exception_table_is_cpythons_own() {
        use ruff_python_stdlib::builtins::is_exception as ruff_is_exception;

        assert!(is_exception("ValueError") && is_exception("_IncompleteInputError"));
        assert!(!is_exception("IOError") && !is_exception("WindowsError"));
        assert!(!is_exception("ImportCycleError") && ruff_is_exception("ImportCycleError", 14));
        assert!(!ruff_is_exception("_IncompleteInputError", 14));
        for (name, _) in EXCEPTIONS {
            assert!(
                ruff_is_exception(name, 14) || name == "_IncompleteInputError",
                "{name}"
            );
        }
        assert!(exception_is("ModuleNotFoundError", "ImportError"));
        assert!(exception_is("TabError", "Exception"));
        assert!(!exception_is("ImportError", "ModuleNotFoundError"));
        assert!(!exception_is("ValueError", "Nope"));
    }
}
