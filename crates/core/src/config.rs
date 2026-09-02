//! `[tool.sightline]` from `pyproject.toml`, else from `sightline.toml`.

use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

/// Environment, vendored and generated directories the walk never enters;
/// dot-directories are skipped wholesale by the walker.
pub const DEFAULT_EXCLUDE_DIRS: &[&str] = &[
    "__pycache__",
    "venv",
    "node_modules",
    "site-packages",
    "build",
    "dist",
    "target",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub excludes: Vec<String>,
    pub rules_off: BTreeSet<String>,
    pub oracle: bool,
    pub python_env: Option<String>,
    /// family-P seeds
    pub hot_roots: Vec<String>,
    /// overrides the packaging read (`facts.published`): `Some(false)` for an
    /// app that packages a `src/` anyway, `Some(true)` for a library whose
    /// metadata says nothing. `None` leaves the read alone.
    pub published: Option<bool>,
}

/// The table as written; `rules-off` entries may be integers in TOML.
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
struct Table {
    excludes: Vec<String>,
    rules_off: Vec<toml::Value>,
    oracle: bool,
    python_env: Option<String>,
    hot_roots: Vec<toml::Value>,
    published: Option<bool>,
}

/// A TOML value as `str(value)` spells it: the string itself for a string.
pub fn spelled(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl Config {
    pub fn new() -> Config {
        Config {
            oracle: true,
            ..Default::default()
        }
    }

    pub fn from_table(table: &toml::Table) -> Config {
        let t: Table = table.clone().try_into().unwrap_or_default();
        let oracle = match table.get("oracle") {
            Some(v) => v.as_bool().unwrap_or(true),
            None => true,
        };
        Config {
            excludes: t.excludes,
            rules_off: t.rules_off.iter().map(spelled).collect(),
            oracle,
            python_env: t.python_env,
            hot_roots: t.hot_roots.iter().map(spelled).collect(),
            published: t.published,
        }
    }
}

/// `[tool.sightline]` from `pyproject.toml`, else from `sightline.toml`: a
/// Cargo root has no pyproject to hang the table off. A missing file is the
/// default config; an unreadable table too.
pub fn load_config(root: &Utf8Path, config_path: Option<&Utf8Path>) -> Config {
    let mut source: Utf8PathBuf = match config_path {
        Some(p) => p.to_path_buf(),
        None => root.join("pyproject.toml"),
    };
    if config_path.is_none() && !source.is_file() {
        source = root.join("sightline.toml");
    }
    if !source.is_file() {
        return Config::new();
    }
    let Ok(text) = std::fs::read_to_string(&source) else {
        return Config::new();
    };
    let Ok(doc) = text.parse::<toml::Table>() else {
        return Config::new();
    };
    match doc
        .get("tool")
        .and_then(|t| t.get("sightline"))
        .and_then(|s| s.as_table())
    {
        Some(table) => Config::from_table(table),
        None => Config::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_oracle_on() {
        assert!(Config::new().oracle);
        assert!(load_config(Utf8Path::new("Z:/no-such-dir"), None).oracle);
    }

    #[test]
    fn reads_the_table_with_numeric_rules_off() {
        let doc: toml::Table = r#"
[tool.sightline]
excludes = ["corpus-ext/reports"]
rules-off = [23, "41"]
oracle = false
python-env = ".venv"
hot-roots = ["pkg.main"]
published = false
"#
        .parse()
        .unwrap();
        let c = Config::from_table(doc["tool"]["sightline"].as_table().unwrap());
        assert_eq!(c.excludes, vec!["corpus-ext/reports"]);
        assert_eq!(
            c.rules_off,
            BTreeSet::from(["23".to_string(), "41".to_string()])
        );
        assert!(!c.oracle);
        assert_eq!(c.python_env.as_deref(), Some(".venv"));
        assert_eq!(c.hot_roots, vec!["pkg.main"]);
        assert_eq!(c.published, Some(false));
    }

    #[test]
    fn falls_back_to_sightline_toml() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        std::fs::write(
            root.join("sightline.toml"),
            "[tool.sightline]\nexcludes = [\"x\"]\n",
        )
        .unwrap();
        assert_eq!(load_config(root, None).excludes, vec!["x"]);
    }
}
