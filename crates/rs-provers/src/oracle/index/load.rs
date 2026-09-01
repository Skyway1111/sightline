//! The `ra_ap_*` side (`rs/oracle.py:_lsif`'s replacement, codemap section
//! 6): a project root loaded in process, the vfs walked into file maps, every
//! token that can name a definition, and each definition reduced to its name
//! range. The resolver is `IdentClass::classify_token`, what
//! `ide::static_index` runs and so what the reference's dump was built from;
//! the codemap recipe answered 65 fewer edges on salvo.

use std::collections::{BTreeSet, HashMap};

use camino::Utf8Path;
use hir::{HasSource, Semantics};
use ide_db::base_db::SourceDatabase;
use ide_db::defs::{Definition, IdentClass};
use ide_db::{FileId, FxHashMap, RootDatabase};
use load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use project_model::{CargoConfig, RustLibSource};
use syntax::ast::HasName;
use syntax::{AstNode, SyntaxToken, TextSize, ast};
use tempfile::TempDir;

use crate::oracle::RsOracle;
use crate::oracle::index::join::{DefSite, Row};

/// The file maps the join needs and the vfs `.rs` counts the header
/// reports. No database here: `RootDatabase` is not `Sync`, so a struct
/// holding one cannot reach a rayon worker.
#[derive(Default)]
pub struct Files {
    by_rel: HashMap<String, FileId>,
    by_file: HashMap<FileId, String>,
    /// vfs `.rs` paths under the audited root, and past it
    pub documents_in: BTreeSet<String>,
    pub documents_out: BTreeSet<String>,
}

/// One loaded root. `close` sweeps the temp directory, which empties only
/// once the database and the client have gone: the server holds the dlls it
/// copied there open.
pub struct Loaded {
    pub db: RootDatabase,
    pub files: Files,
    proc_macro: Option<proc_macro_api::ProcMacroClient>,
    tmp: TempDir,
}

impl Loaded {
    pub fn close(self) -> bool {
        drop(self.db);
        drop(self.proc_macro);
        self.tmp.close().is_ok()
    }
}

/// `rels` is every facts module rel of the audited root, so a sibling
/// crate's file is a document of this load and a cross-crate reference joins.
pub fn load(oracle: &RsOracle, project: &Utf8Path, rels: &[String]) -> anyhow::Result<Loaded> {
    let tmp = tempfile::Builder::new()
        .prefix("sightline-rs-index")
        .tempdir()?;
    let scoped = tmp.path().to_string_lossy().to_string();
    let mut extra_env: FxHashMap<String, Option<String>> = FxHashMap::default();
    for (key, value) in oracle.env(Some(project)) {
        extra_env.insert(key, Some(value));
    }
    for key in ["TMP", "TEMP"] {
        extra_env.insert(key.to_string(), Some(scoped.clone()));
    }
    let cargo_config = CargoConfig {
        all_targets: true,
        sysroot: Some(RustLibSource::Discover),
        set_test: true,
        extra_env,
        ..Default::default()
    };
    let load_config = LoadCargoConfig {
        load_out_dirs_from_check: true,
        with_proc_macro_server: ProcMacroServerChoice::Sysroot,
        prefill_caches: false,
        num_worker_threads: rayon::current_num_threads(),
        proc_macro_processes: 1,
    };
    let (db, vfs, proc_macro) =
        load_workspace_at(project.as_std_path(), &cargo_config, &load_config, &|_| ())?;

    let home = slashed(oracle.root.as_str()).to_lowercase() + "/";
    let wanted: BTreeSet<&str> = rels.iter().map(String::as_str).collect();
    let mut files = Files::default();
    for (file, path) in vfs.iter() {
        let Some(abs) = path.as_path() else { continue };
        let text = slashed(abs.as_str());
        if !text.ends_with(".rs") {
            continue;
        }
        let Some(cut) = text.to_lowercase().strip_prefix(&home).map(str::len) else {
            files.documents_out.insert(text);
            continue;
        };
        let rel = text[text.len() - cut..].to_string();
        files.documents_in.insert(text);
        // the `where` of the Python join: only the documents facts calls
        // modules, which is what the LSIF dump held
        if wanted.contains(rel.as_str()) {
            files.by_rel.insert(rel.clone(), file);
            files.by_file.insert(file, rel);
        }
    }
    Ok(Loaded {
        db,
        files,
        proc_macro,
        tmp,
    })
}

fn slashed(path: &str) -> String {
    path.replace('\\', "/")
}

impl Files {
    /// Every token of the file the reference indexed: the only enumeration
    /// reaching a path segment or a `use` tree leaf, neither a facts node.
    /// Lines break at `\n` alone, the split `facts.lines` is.
    pub fn token_rows(&self, sema: &Semantics<'_, RootDatabase>, rel: &str) -> Vec<Row> {
        let Some(&file) = self.by_rel.get(rel) else {
            return Vec::new();
        };
        let db = sema.db;
        let text = db.file_text(file).text(db).clone();
        let mut starts = vec![0usize];
        starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        sema.parse_guess_edition(file)
            .syntax()
            .descendants_with_tokens()
            .filter_map(|it| it.into_token())
            // the reference classified every non-trivia token of the file
            .filter(|it| !it.kind().is_trivia())
            .map(|token| {
                let offset = usize::from(token.text_range().start());
                let line = starts.partition_point(|&at| at <= offset);
                let col = (offset - starts[line - 1]) as u32;
                let key = (rel.to_string(), line as u32, col, token.text().to_string());
                Row {
                    key,
                    at: offset as u32,
                }
            })
            .collect()
    }

    /// Every definition the token names, dropped to the ones that land in a
    /// document of the audited root and are not the site itself.
    pub fn defs_at(&self, sema: &Semantics<'_, RootDatabase>, row: &Row) -> Vec<DefSite> {
        let Some(&file) = self.by_rel.get(&row.key.0) else {
            return Vec::new();
        };
        let offset = TextSize::new(row.at);
        let source = sema.parse_guess_edition(file);
        let Some(token) = source.syntax().token_at_offset(offset).right_biased() else {
            return Vec::new();
        };
        classify(sema, token)
            .into_iter()
            .filter_map(|def| self.def_site(sema, def, file, offset))
            .collect()
    }

    /// The rel of the file the definition was written in, its line, and the
    /// text its name range covers.
    fn def_site(
        &self,
        sema: &Semantics<'_, RootDatabase>,
        def: Definition,
        site_file: FileId,
        site_offset: TextSize,
    ) -> Option<DefSite> {
        let db = sema.db;
        let at = name_node(sema, def)?;
        let found = at.as_ref().original_file_range_rooted(db);
        let file = found.file_id.file_id(db);
        if file == site_file && found.range.start() == site_offset {
            return None; // a range whose own definition is itself: a declaration
        }
        let rel = self.by_file.get(&file)?;
        let text = db.file_text(file).text(db).clone();
        let start = usize::from(found.range.start());
        let cut = &text[start..usize::from(found.range.end())];
        Some(DefSite {
            rel: rel.clone(),
            line: text[..start].matches('\n').count() as u32 + 1,
            // a range over lines names no symbol either way: no facts
            // name holds a newline
            ident: cut.to_string(),
        })
    }
}

/// Every token descends, not only one inside a macro call's token tree: an
/// attribute proc macro leaves plain source whose semantics are in the
/// expansion.
fn classify(sema: &Semantics<'_, RootDatabase>, token: SyntaxToken) -> Vec<Definition> {
    for token in sema.descend_into_macros_exact(token) {
        if let Some(class) = IdentClass::classify_token(sema, &token) {
            let defs = class.definitions();
            if !defs.is_empty() {
                return defs.into_iter().map(|(def, _)| def).collect();
            }
        }
    }
    Vec::new()
}

/// The name range `NavigationTarget::focus_or_full_range` gave the
/// reference. The join is positional, so a kind facts never records joins
/// where its line and text meet a symbol's: a parameter shares its line with
/// its `fn` (33 turmoil edges) and a trait's `Self` names the trait (143).
fn name_node(
    sema: &Semantics<'_, RootDatabase>,
    def: Definition,
) -> Option<hir::InFile<syntax::SyntaxNode>> {
    let db = sema.db;
    macro_rules! named {
        ($it:expr, $get:ident) => {{
            let source = $it.source(db)?;
            let name = source.value.$get()?;
            Some(source.with_value(name.syntax().clone()))
        }};
    }
    match def {
        Definition::Function(it) => named!(it, name),
        Definition::Adt(it) => named!(it, name),
        Definition::Const(it) => named!(it, name),
        Definition::Static(it) => named!(it, name),
        Definition::TypeAlias(it) => named!(it, name),
        Definition::Trait(it) => named!(it, name),
        // `HasName` covers the `Either<ast::Macro, ast::Fn>` a macro's source
        // is, so `macro_rules! m` joins the facts `macro` symbol
        Definition::Macro(it) => named!(it, name),
        // `#[salvo(extract(..))]`: the helper names the derive that declares
        // it (32 salvo edges, `crates/core/src/serde/request.rs:632`)
        Definition::DeriveHelper(it) => named!(it.derive(), name),
        Definition::GenericParam(hir::GenericParam::LifetimeParam(it)) => named!(it, lifetime),
        Definition::GenericParam(hir::GenericParam::TypeParam(it)) => param(sema, it.merge()),
        Definition::GenericParam(hir::GenericParam::ConstParam(it)) => param(sema, it.merge()),
        Definition::Local(it) => Some(it.primary_source(db).name()?.map(|it| it.syntax().clone())),
        _ => None,
    }
}

/// A type or const parameter's name range, and a trait's own where the
/// parameter is its implicit `Self`: the source is an `Either` with no
/// `HasName` impl, and its first `Name` child is what that trait reads.
fn param(
    sema: &Semantics<'_, RootDatabase>,
    it: hir::TypeOrConstParam,
) -> Option<hir::InFile<syntax::SyntaxNode>> {
    let source = it.source(sema.db)?;
    let name = source.value.syntax().children().find_map(ast::Name::cast)?;
    Some(source.with_value(name.syntax().clone()))
}
