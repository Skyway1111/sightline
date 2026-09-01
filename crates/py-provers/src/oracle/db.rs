//! The checker database the oracle asks: the shim's construction, and the
//! two path maps every query goes through.

use super::*;

/// A checker absolute path as a posix rel under `root`, `None` outside it
/// (`Oracle._rel`).
pub(super) fn rel_of(db: &ProjectDatabase, root: &SystemPath, file: File) -> Option<Rel> {
    let path = file.path(db).as_system_path()?;
    Some(
        path.strip_prefix(root)
            .ok()?
            .as_str()
            .replace('\\', "/")
            .into(),
    )
}

pub(super) fn resolve(db: &ProjectDatabase, root: &SystemPath, rel: &str) -> Option<File> {
    system_path_to_file(db, SystemPathBuf::from(root.as_str()).join(rel)).ok()
}

/// The shim's construction, on the options `oracle.py:_config` writes.
pub(super) fn database(
    root: &SystemPath,
    excludes: &[String],
    import_roots: &[Utf8PathBuf],
    python_exe: Option<&Utf8Path>,
) -> anyhow::Result<ProjectDatabase> {
    let system = OsSystem::new(root);
    let mut metadata = ProjectMetadata::discover(root, &system)?;
    metadata.apply_configuration_files(&system)?;
    let extra = extra_paths(root, import_roots);
    let exclude: Vec<String> = SHADOW_EXCLUDES
        .iter()
        .map(|s| (*s).to_string())
        .chain(
            excludes
                .iter()
                .map(|e| format!("**/{}", e.trim_matches('/'))),
        )
        .collect();
    let options = Options {
        environment: Some(EnvironmentOptions {
            python: python_exe.map(|p| RelativePathBuf::cli(p.as_str())),
            // With an interpreter to ask, ty infers the version from it, as the
            // Python tool does. With none, it infers from the host, so the same
            // tree reads one way here and another on a machine whose `python`
            // is older. Fall back to the version the rest of the port assumes
            // (port rule R16: builtins and the stdlib list are 3.14).
            python_version: python_exe.is_none().then(|| {
                RangedValue::cli(
                    ty_project::metadata::python_version::SupportedPythonVersion::Py314,
                )
            }),
            extra_paths: (!extra.is_empty())
                .then(|| extra.iter().map(RelativePathBuf::cli).collect()),
            ..EnvironmentOptions::default()
        }),
        src: Some(SrcOptions {
            // pyright does not respect gitignore files; exclusion is config-driven only
            respect_ignore_files: Some(false),
            exclude: Some(RangedValue::cli(
                exclude.iter().map(RelativeGlobPattern::cli).collect(),
            )),
            ..SrcOptions::default()
        }),
        rules: Some(
            ENABLED_RULES
                .iter()
                .map(|(rule, _)| {
                    (
                        RangedValue::cli((*rule).to_string()),
                        RangedValue::cli(Level::Warn),
                    )
                })
                .collect(),
        ),
        ..Options::default()
    };
    metadata.apply_override_options(options);
    let mut db = ProjectDatabase::fallible(metadata, system)?;
    ruff_db::disable_lru(&mut db);
    Ok(db)
}

/// `_config`'s `extraPaths`: facts' import roots, or the root and its `src`
/// where the caller has no list; a path that is not a directory is dropped.
fn extra_paths(root: &SystemPath, import_roots: &[Utf8PathBuf]) -> Vec<String> {
    let fallback = [
        Utf8PathBuf::from(root.as_str()),
        Utf8PathBuf::from(root.as_str()).join("src"),
    ];
    let roots: &[Utf8PathBuf] = if import_roots.is_empty() {
        &fallback
    } else {
        import_roots
    };
    roots
        .iter()
        .filter(|p| p.is_dir())
        .map(|p| p.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use sightline_core::config::Config;
    use sightline_testkit::make_repo;

    #[test]
    fn a_workspace_members_src_joins_the_extra_paths() {
        // facts and the checker name one module the same way: without the
        // member's own `src` on the search path every import of it is
        // unresolved (the traps ledger's density jump)
        let dir = make_repo(&[
            ("member/pyproject.toml", "[project]\nname = \"member\"\n"),
            ("member/src/member/__init__.py", ""),
            ("app.py", "import member\n"),
        ]);
        let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
        let config = Config::new();
        let listing = sightline_core::walk::discover(root, &config);
        let built = sightline_py_facts::build::build_facts(root, &config, &listing, None);
        let sys_root = SystemPathBuf::from(root.as_str());
        assert_eq!(
            extra_paths(&sys_root, &built.borrow_dependent().import_roots),
            [
                root.join("member").join("src").as_str().to_string(),
                root.as_str().to_string(),
            ]
        );
        // a checker built without the list (a bare test fixture's) reads the root
        let member = SystemPathBuf::from(root.join("member").as_str());
        assert_eq!(
            extra_paths(&member, &[]),
            [
                root.join("member").as_str().to_string(),
                root.join("member").join("src").as_str().to_string(),
            ]
        );
    }
}
