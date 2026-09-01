//! The CPython class vocabulary a bucket key can hold. `positioned` is false
//! for the classes CPython gives no `lineno`, which the dump therefore drops.

macro_rules! kinds {
    ($($v:ident => $n:expr, $pos:expr;)*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum Kind { $($v),* }

        /// CPython's `_fields` per class: the `traversal` layer's `fields` table.
static FIELDS: &[(Kind, &[&str])] = &[
    (Kind::Module, &["body", "type_ignores"]),
    (Kind::Arguments, &["posonlyargs", "args", "vararg", "kwonlyargs", "kw_defaults", "kwarg", "defaults"]),
    (Kind::Comprehension, &["target", "iter", "ifs", "is_async"]),
    (Kind::WithItem, &["context_expr", "optional_vars"]),
    (Kind::MatchCase, &["pattern", "guard", "body"]),
    (Kind::FunctionDef, &["name", "args", "body", "decorator_list", "returns", "type_comment", "type_params"]),
    (Kind::AsyncFunctionDef, &["name", "args", "body", "decorator_list", "returns", "type_comment", "type_params"]),
    (Kind::ClassDef, &["name", "bases", "keywords", "body", "decorator_list", "type_params"]),
    (Kind::Return, &["value"]),
    (Kind::Delete, &["targets"]),
    (Kind::Assign, &["targets", "value", "type_comment"]),
    (Kind::TypeAlias, &["name", "type_params", "value"]),
    (Kind::AugAssign, &["target", "op", "value"]),
    (Kind::AnnAssign, &["target", "annotation", "value", "simple"]),
    (Kind::For, &["target", "iter", "body", "orelse", "type_comment"]),
    (Kind::AsyncFor, &["target", "iter", "body", "orelse", "type_comment"]),
    (Kind::While, &["test", "body", "orelse"]),
    (Kind::If, &["test", "body", "orelse"]),
    (Kind::With, &["items", "body", "type_comment"]),
    (Kind::AsyncWith, &["items", "body", "type_comment"]),
    (Kind::Match, &["subject", "cases"]),
    (Kind::Raise, &["exc", "cause"]),
    (Kind::Try, &["body", "handlers", "orelse", "finalbody"]),
    (Kind::TryStar, &["body", "handlers", "orelse", "finalbody"]),
    (Kind::Assert, &["test", "msg"]),
    (Kind::Import, &["names"]),
    (Kind::ImportFrom, &["module", "names", "level"]),
    (Kind::Global, &["names"]),
    (Kind::Nonlocal, &["names"]),
    (Kind::Expr, &["value"]),
    (Kind::Pass, &[]),
    (Kind::Break, &[]),
    (Kind::Continue, &[]),
    (Kind::BoolOp, &["op", "values"]),
    (Kind::NamedExpr, &["target", "value"]),
    (Kind::BinOp, &["left", "op", "right"]),
    (Kind::UnaryOp, &["op", "operand"]),
    (Kind::Lambda, &["args", "body"]),
    (Kind::IfExp, &["test", "body", "orelse"]),
    (Kind::Dict, &["keys", "values"]),
    (Kind::Set, &["elts"]),
    (Kind::ListComp, &["elt", "generators"]),
    (Kind::SetComp, &["elt", "generators"]),
    (Kind::DictComp, &["key", "value", "generators"]),
    (Kind::GeneratorExp, &["elt", "generators"]),
    (Kind::Await, &["value"]),
    (Kind::Yield, &["value"]),
    (Kind::YieldFrom, &["value"]),
    (Kind::Compare, &["left", "ops", "comparators"]),
    (Kind::Call, &["func", "args", "keywords"]),
    (Kind::FormattedValue, &["value", "conversion", "format_spec"]),
    (Kind::JoinedStr, &["values"]),
    (Kind::Interpolation, &["value", "str", "conversion", "format_spec"]),
    (Kind::TemplateStr, &["values"]),
    (Kind::Constant, &["value", "kind"]),
    (Kind::Attribute, &["value", "attr", "ctx"]),
    (Kind::Subscript, &["value", "slice", "ctx"]),
    (Kind::Starred, &["value", "ctx"]),
    (Kind::Name, &["id", "ctx"]),
    (Kind::List, &["elts", "ctx"]),
    (Kind::Tuple, &["elts", "ctx"]),
    (Kind::Slice, &["lower", "upper", "step"]),
    (Kind::ExceptHandler, &["type", "name", "body"]),
    (Kind::MatchValue, &["value"]),
    (Kind::MatchSingleton, &["value"]),
    (Kind::MatchSequence, &["patterns"]),
    (Kind::MatchMapping, &["keys", "patterns", "rest"]),
    (Kind::MatchClass, &["cls", "patterns", "kwd_attrs", "kwd_patterns"]),
    (Kind::MatchStar, &["name"]),
    (Kind::MatchAs, &["pattern", "name"]),
    (Kind::MatchOr, &["patterns"]),
    (Kind::TypeVar, &["name", "bound", "default_value"]),
    (Kind::ParamSpec, &["name", "default_value"]),
    (Kind::TypeVarTuple, &["name", "default_value"]),
    (Kind::Alias, &["name", "asname"]),
    (Kind::Arg, &["arg", "annotation", "type_comment"]),
    (Kind::Keyword, &["arg", "value"]),
    (Kind::TypeIgnore, &["lineno", "tag"]),
];

impl Kind {
            pub fn name(self) -> &'static str {
                match self { $(Kind::$v => $n),* }
            }
            pub fn positioned(self) -> bool {
                match self { $(Kind::$v => $pos),* }
            }
            pub fn from_name(s: &str) -> Option<Kind> {
                match s { $($n => Some(Kind::$v),)* _ => None }
            }
        }
    };
}

kinds! {
    Module => "Module", false;
    Arguments => "arguments", false;
    Comprehension => "comprehension", false;
    WithItem => "withitem", false;
    MatchCase => "match_case", false;

    FunctionDef => "FunctionDef", true;
    AsyncFunctionDef => "AsyncFunctionDef", true;
    ClassDef => "ClassDef", true;
    Return => "Return", true;
    Delete => "Delete", true;
    Assign => "Assign", true;
    TypeAlias => "TypeAlias", true;
    AugAssign => "AugAssign", true;
    AnnAssign => "AnnAssign", true;
    For => "For", true;
    AsyncFor => "AsyncFor", true;
    While => "While", true;
    If => "If", true;
    With => "With", true;
    AsyncWith => "AsyncWith", true;
    Match => "Match", true;
    Raise => "Raise", true;
    Try => "Try", true;
    TryStar => "TryStar", true;
    Assert => "Assert", true;
    Import => "Import", true;
    ImportFrom => "ImportFrom", true;
    Global => "Global", true;
    Nonlocal => "Nonlocal", true;
    Expr => "Expr", true;
    Pass => "Pass", true;
    Break => "Break", true;
    Continue => "Continue", true;

    BoolOp => "BoolOp", true;
    NamedExpr => "NamedExpr", true;
    BinOp => "BinOp", true;
    UnaryOp => "UnaryOp", true;
    Lambda => "Lambda", true;
    IfExp => "IfExp", true;
    Dict => "Dict", true;
    Set => "Set", true;
    ListComp => "ListComp", true;
    SetComp => "SetComp", true;
    DictComp => "DictComp", true;
    GeneratorExp => "GeneratorExp", true;
    Await => "Await", true;
    Yield => "Yield", true;
    YieldFrom => "YieldFrom", true;
    Compare => "Compare", true;
    Call => "Call", true;
    FormattedValue => "FormattedValue", true;
    JoinedStr => "JoinedStr", true;
    Interpolation => "Interpolation", true;
    TemplateStr => "TemplateStr", true;
    Constant => "Constant", true;
    Attribute => "Attribute", true;
    Subscript => "Subscript", true;
    Starred => "Starred", true;
    Name => "Name", true;
    List => "List", true;
    Tuple => "Tuple", true;
    Slice => "Slice", true;

    ExceptHandler => "ExceptHandler", true;

    MatchValue => "MatchValue", true;
    MatchSingleton => "MatchSingleton", true;
    MatchSequence => "MatchSequence", true;
    MatchMapping => "MatchMapping", true;
    MatchClass => "MatchClass", true;
    MatchStar => "MatchStar", true;
    MatchAs => "MatchAs", true;
    MatchOr => "MatchOr", true;

    TypeVar => "TypeVar", true;
    ParamSpec => "ParamSpec", true;
    TypeVarTuple => "TypeVarTuple", true;

    Alias => "alias", true;
    Arg => "arg", true;
    Keyword => "keyword", true;

    TypeIgnore => "TypeIgnore", true;
}

impl Kind {
    /// CPython's `_fields` for this class, which the `traversal` layer
    /// prints so the child order in `order.rs` is checked, not trusted.
    pub fn fields(self) -> &'static [&'static str] {
        FIELDS
            .iter()
            .find(|(k, _)| *k == self)
            .map_or(&[], |(_, f)| f)
    }
}

// --- the CPython base classes (`ast.stmt`, `ast.expr`, `DEF_NODES`) ----------

/// `astutil.DEF_NODES`.
pub fn is_def(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::FunctionDef | Kind::AsyncFunctionDef | Kind::ClassDef
    )
}

/// The CPython classes under `ast.stmt`.
pub fn is_stmt(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::FunctionDef
            | Kind::AsyncFunctionDef
            | Kind::ClassDef
            | Kind::Return
            | Kind::Delete
            | Kind::Assign
            | Kind::TypeAlias
            | Kind::AugAssign
            | Kind::AnnAssign
            | Kind::For
            | Kind::AsyncFor
            | Kind::While
            | Kind::If
            | Kind::With
            | Kind::AsyncWith
            | Kind::Match
            | Kind::Raise
            | Kind::Try
            | Kind::TryStar
            | Kind::Assert
            | Kind::Import
            | Kind::ImportFrom
            | Kind::Global
            | Kind::Nonlocal
            | Kind::Expr
            | Kind::Pass
            | Kind::Break
            | Kind::Continue
    )
}

/// The CPython classes under `ast.expr`.
pub fn is_expr(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::BoolOp
            | Kind::NamedExpr
            | Kind::BinOp
            | Kind::UnaryOp
            | Kind::Lambda
            | Kind::IfExp
            | Kind::Dict
            | Kind::Set
            | Kind::ListComp
            | Kind::SetComp
            | Kind::DictComp
            | Kind::GeneratorExp
            | Kind::Await
            | Kind::Yield
            | Kind::YieldFrom
            | Kind::Compare
            | Kind::Call
            | Kind::FormattedValue
            | Kind::JoinedStr
            | Kind::Interpolation
            | Kind::TemplateStr
            | Kind::Constant
            | Kind::Attribute
            | Kind::Subscript
            | Kind::Starred
            | Kind::Name
            | Kind::List
            | Kind::Tuple
            | Kind::Slice
    )
}
