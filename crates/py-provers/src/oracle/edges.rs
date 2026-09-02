//! The oracle's `call_edges`: every call expression in a project `.py` file
//! whose callee type denotes definitions, with its targets inside the root,
//! or its definitions' dotted homes when no body under the root runs. The
//! rows are sorted by the absolute path the checker knows the file by.

use ruff_db::files::File;
use ruff_db::parsed::parsed_module;
use ruff_db::source::{line_index, source_text};
use ruff_db::system::SystemPath;
use ruff_python_ast::visitor::source_order::{self, SourceOrderVisitor};
use ruff_python_ast::{self as ast};
use ruff_source_file::LineIndex;
use ruff_text_size::{Ranged, TextSize};
use ty_project::{Db as _, ProjectDatabase};
use ty_python_semantic::Db as _;
use ty_python_semantic::SemanticModel;
use ty_python_semantic::types::{CalleeDefinition, ReceiverClass, callee_definitions};

use crate::callgraph::CallEdge;

/// The sort key and sightline's site identity, on the absolute path.
type Key = (String, u32, u32, u32, u32);

pub fn call_edges(db: &ProjectDatabase, root: &SystemPath) -> Vec<CallEdge> {
    let mut edges: Vec<(Key, CallEdge)> = Vec::new();
    for file in &db.project().files(db) {
        let Some(path) = file.path(db).as_system_path() else {
            continue;
        };
        if path.extension() != Some("py") || !path.starts_with(root) {
            continue;
        }
        let Some(rel) = super::rel_of(db, root, file) else {
            continue;
        };
        let program_file = db.program_file(file);
        let parsed = parsed_module(db, program_file.python_file(db)).load(db);
        let model = SemanticModel::new(db, program_file);
        let source = source_text(db, file);
        let index = line_index(db, file);
        let mut collector = CallCollector::default();
        for stmt in &parsed.syntax().body {
            collector.visit_stmt(stmt);
        }
        for call in collector.calls {
            let Some(definitions) = callee_definitions(db, &model, call) else {
                continue;
            };
            let Some(verdict) = edge_verdict(db, root, &definitions) else {
                continue;
            };
            let (line, col) = byte_position(&index, &source, call.range().start());
            let (end_line, end_col) = byte_position(&index, &source, call.range().end());
            let (targets, external) = verdict;
            edges.push((
                (path.to_string(), line, col, end_line, end_col),
                CallEdge {
                    rel: rel.clone(),
                    line,
                    col,
                    end_line,
                    end_col,
                    targets,
                    external,
                },
            ));
        }
    }
    edges.sort_by(|a, b| a.0.cmp(&b.0));
    edges.into_iter().map(|(_, edge)| edge).collect()
}

/// A union that straddles the root is no verdict. The two sides are
/// exclusive: targets inside the root, or the dotted homes of definitions that
/// all lie outside it (and whose bound methods' receiver classes do too, since
/// a root class inheriting a library method may be called back through the
/// library's template hooks).
type Verdict = (
    Vec<(sightline_core::findings::Rel, u32)>,
    Vec<sightline_core::findings::Qname>,
);

fn edge_verdict(
    db: &ProjectDatabase,
    root: &SystemPath,
    definitions: &[CalleeDefinition],
) -> Option<Verdict> {
    // a vendored typeshed stub has no system path: outside the root
    let in_root = |file: File| -> bool {
        file.path(db)
            .as_system_path()
            .is_some_and(|path| path.starts_with(root))
    };
    let mut targets: Vec<(String, u32)> = Vec::new();
    let mut outside: Vec<String> = Vec::new();
    for definition in definitions {
        let target = definition.definition.file();
        if in_root(target) {
            let line = line_index(db, target).line_index(definition.definition.range().start());
            targets.push((
                target.path(db).as_system_path()?.to_string(),
                line.get() as u32,
            ));
            continue;
        }
        let receiver_outside = match definition.receiver {
            ReceiverClass::Unbound => true,
            ReceiverClass::Defined(class_file) => !in_root(class_file),
            ReceiverClass::Opaque => false,
        };
        if !receiver_outside {
            return None;
        }
        outside.push(definition.home.clone());
    }
    match (targets.is_empty(), outside.is_empty()) {
        (false, true) => {
            targets.sort();
            targets.dedup();
            // the target rows carry the absolute path the sort keyed on
            let rels = targets
                .into_iter()
                .map(|(path, line)| {
                    let rel = SystemPath::new(&path).strip_prefix(root).ok()?;
                    Some((rel.as_str().replace('\\', "/").into(), line))
                })
                .collect::<Option<Vec<_>>>()?;
            Some((rels, Vec::new()))
        }
        (true, false) => {
            outside.sort();
            outside.dedup();
            Some((Vec::new(), outside.into_iter().map(Into::into).collect()))
        }
        _ => None,
    }
}

/// `(1-based line, byte column within the line)` of an offset.
fn byte_position(index: &LineIndex, source: &str, offset: TextSize) -> (u32, u32) {
    let line = index.line_index(offset);
    let col = (offset - index.line_start(line, source)).to_u32();
    (line.get() as u32, col)
}

/// Every call expression in a module, nested scopes included, in source order.
#[derive(Default)]
struct CallCollector<'a> {
    calls: Vec<&'a ast::ExprCall>,
}

impl<'a> SourceOrderVisitor<'a> for CallCollector<'a> {
    fn visit_expr(&mut self, expr: &'a ast::Expr) {
        if let ast::Expr::Call(call) = expr {
            self.calls.push(call);
        }
        source_order::walk_expr(self, expr);
    }
}
