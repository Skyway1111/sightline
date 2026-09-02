//! The oracle's type queries: a span's type read where the caller's byte
//! columns land, and a module member's type read off an appended
//! `reveal_type` the query removes again.

use super::*;

impl Oracle {
    /// One `span_type` per query, in query order: `None` for a miss
    /// (no expression at exactly that range, no inferred type). Chunked across
    /// rayon workers over `db.clone()`.
    pub fn span_types(&self, queries: &[TypeQuery]) -> Vec<Option<String>> {
        if queries.is_empty() {
            return Vec::new();
        }
        self.base(); // the base pass runs first, and never under an override
        let root = self.sys_root.clone();
        self.pass("types", |db| {
            let min_len = minimum_parallel_job_len(queries.len(), 64);
            queries
                .par_iter()
                .with_min_len(min_len)
                .map_with(db.clone(), |db, q| {
                    salsa::attach_allow_change(db, || {
                        let file = resolve(db, &root, &q.rel)?;
                        span_type(db, file, q)
                    })
                })
                .collect()
        })
        .unwrap_or_else(|| vec![None; queries.len()])
    }

    /// The type module-level code sees under `dotted` (`f`, `Cls.m`) in the
    /// module at `rel`: the answer to an appended `reveal_type(<expr>)`,
    /// displayed and normalized as a span query is.
    pub fn module_member_type(&self, rel: &Rel, dotted: &str) -> Option<String> {
        let one = [dotted.to_string()];
        self.module_member_types(rel, &one).pop().flatten()
    }

    /// Every candidate of one file on one override: `reveal_type(<dotted>)`
    /// appended per entry, read back in order, the override restored.
    pub fn module_member_types(&self, rel: &Rel, dotted: &[String]) -> Vec<Option<String>> {
        if dotted.is_empty() {
            return Vec::new();
        }
        self.base();
        let root = self.sys_root.clone();
        self.pass("types", |db| {
            appended(db, &root, rel, dotted, |db, file, line| {
                appended_type(db, file, line, |db, ty, model| {
                    normalize_type_display(&revealed_display(db, ty, &model.program_environment()))
                })
            })
        })
        .unwrap_or_else(|| vec![None; dotted.len()])
    }

    /// Does the module bound to `local` in the module at `rel` hold a class
    /// named `name`? The counterfactual's `stdlib_home` asks.
    pub fn member_is_class(&self, rel: &Rel, local: &str, name: &str) -> bool {
        let one = [format!("{local}.{name}")];
        self.base();
        let root = self.sys_root.clone();
        self.pass("types", |db| {
            appended(db, &root, rel, &one, |db, file, line| {
                appended_type(db, file, line, |_, ty, _| ty.is_class_literal())
            })
        })
        .and_then(|mut answers| answers.pop().flatten())
        .unwrap_or(false)
    }
}

/// `reveal_type(<expr>)` appended per entry at module scope, read back in
/// order, the override restored. The reveal artifacts never reach
/// `diagnostics`: the base pass ran before this one and is memoized.
fn appended<R>(
    db: &mut ProjectDatabase,
    root: &SystemPath,
    rel: &str,
    exprs: &[String],
    read: impl Fn(&ProjectDatabase, File, usize) -> Option<R>,
) -> Vec<Option<R>> {
    let Some(file) = resolve(db, root, rel) else {
        return exprs.iter().map(|_| None).collect();
    };
    let prior = file.source_text_override(db).clone();
    let base = source_text(db, file);
    let mut content = base.as_str().to_string();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    let mut line = content.matches('\n').count();
    let mut lines = Vec::with_capacity(exprs.len());
    for expr in exprs {
        content.push_str("reveal_type(");
        content.push_str(expr);
        content.push_str(")\n");
        line += 1;
        lines.push(line);
    }
    let overridden = base.with_text(content, &SourceMap::default());
    file.set_source_text_override(db).to(Some(overridden));
    let answers = lines.iter().map(|line| read(db, file, *line)).collect();
    file.set_source_text_override(db).to(prior);
    answers
}

/// What `read` makes of the type of the expression the appended
/// `reveal_type(<expr>)` statement on `line` (1-based, module level) wraps.
fn appended_type<R>(
    db: &ProjectDatabase,
    file: File,
    line: usize,
    read: impl FnOnce(&ProjectDatabase, Type<'_>, &SemanticModel<'_>) -> R,
) -> Option<R> {
    let program_file = db.program_file(file);
    let parsed = parsed_module(db, program_file.python_file(db)).load(db);
    let source = source_text(db, file);
    let index = line_index(db, file);
    let line_start = index.line_start(OneIndexed::new(line)?, &source);
    let Stmt::Expr(stmt) = parsed
        .syntax()
        .body
        .iter()
        .find(|stmt| stmt.range().start() == line_start)?
    else {
        return None;
    };
    let Expr::Call(call) = &*stmt.value else {
        return None;
    };
    let model = SemanticModel::new(db, program_file);
    let ty = call.arguments.args.first()?.inferred_type(&model)?;
    Some(read(db, ty, &model))
}

/// Native `(file, span) -> type`: the expression whose range is exactly the
/// requested span, displayed and normalized. `None` is an honest miss (no node
/// at exactly that range, a line past the file, or no inferred type), never a
/// nearest-node guess and never a panic: a caller whose byte columns disagree
/// with this file (a module decoded lossily) asks past it.
fn span_type(db: &ProjectDatabase, file: File, query: &TypeQuery) -> Option<String> {
    let program_file = db.program_file(file);
    let parsed = parsed_module(db, program_file.python_file(db)).load(db);
    let source = source_text(db, file);
    let index = line_index(db, file);
    if query.line as usize > index.line_count() {
        return None; // `line_start` indexes the line-start table: past it, a panic
    }
    let line_start = index.line_start(OneIndexed::new(query.line as usize)?, &source);
    let start = line_start + TextSize::from(query.col_start);
    let end = line_start + TextSize::from(query.col_end);
    if end > TextSize::of(source.as_str()) || start > end {
        return None;
    }
    let range = TextRange::new(start, end);
    // the innermost *expression* at exactly the requested range (the leaf may
    // be a sub-expression token node, a StringLiteral under ExprStringLiteral)
    let covering = covering_node(parsed.syntax().into(), range)
        .find_first(AnyNodeRef::is_expression)
        .ok()?;
    let node = covering.node();
    if node.range() != range {
        return None;
    }
    let expr_ref = node.as_expr_ref()?;
    let model = SemanticModel::new(db, program_file);
    let ty = expr_ref.inferred_type(&model)?;
    Some(normalize_type_display(&revealed_display(
        db,
        ty,
        &model.program_environment(),
    )))
}
