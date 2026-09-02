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

/// One `[[tool.sightline.overrides]]` row: rules off under these paths,
/// matched as `excludes` entries are.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Override {
    pub paths: Vec<String>,
    pub rules_off: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub excludes: Vec<String>,
    pub rules_off: BTreeSet<String>,
    pub overrides: Vec<Override>,
    pub oracle: bool,
    pub python_env: Option<String>,
    /// #41's seeds
    pub hot_roots: Vec<String>,
    /// overrides the packaging read (`facts.published`): `Some(false)` for an
    /// app that packages a `src/` anyway, `Some(true)` for a library whose
    /// metadata says nothing. `None` leaves the read alone.
    pub published: Option<bool>,
    /// #23's bar, `complexity-threshold`
    pub complexity_threshold: u32,
}

/// The table as written; `rules-off` entries may be integers in TOML.
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
struct Table {
    excludes: Vec<String>,
    rules_off: Vec<toml::Value>,
    overrides: Vec<OverrideTable>,
    oracle: bool,
    python_env: Option<String>,
    hot_roots: Vec<toml::Value>,
    published: Option<bool>,
    complexity_threshold: Option<u32>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case", default)]
struct OverrideTable {
    paths: Vec<String>,
    rules_off: Vec<toml::Value>,
}

/// A TOML value as `str(value)` spells it: the string itself for a string.
#[must_use]
pub fn spelled(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl Config {
    #[must_use]
    pub fn new() -> Self {
        Self {
            oracle: true,
            complexity_threshold: crate::complexity::CC_THRESHOLD,
            ..Default::default()
        }
    }

    pub fn from_table(table: &toml::Table) -> Self {
        let t: Table = table.clone().try_into().unwrap_or_default();
        let oracle = table
            .get("oracle")
            .is_none_or(|v| v.as_bool().unwrap_or(true));
        Self {
            excludes: t.excludes,
            rules_off: t.rules_off.iter().map(spelled).collect(),
            overrides: t
                .overrides
                .iter()
                .map(|o| Override {
                    paths: o.paths.clone(),
                    rules_off: o.rules_off.iter().map(spelled).collect(),
                })
                .collect(),
            oracle,
            python_env: t.python_env,
            hot_roots: t.hot_roots.iter().map(spelled).collect(),
            published: t.published,
            complexity_threshold: t
                .complexity_threshold
                .unwrap_or(crate::complexity::CC_THRESHOLD),
        }
    }
}

/// `[tool.sightline]` from `pyproject.toml`, else from `sightline.toml`: a
/// Cargo root has no pyproject to hang the table off. A missing file is the
/// default config; an unreadable table too.
#[must_use]
pub fn load_config(root: &Utf8Path, config_path: Option<&Utf8Path>) -> Config {
    let mut source: Utf8PathBuf =
        config_path.map_or_else(|| root.join("pyproject.toml"), Utf8Path::to_path_buf);
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
    doc.get("tool")
        .and_then(|t| t.get("sightline"))
        .and_then(|s| s.as_table())
        .map_or_else(Config::new, Config::from_table)
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
complexity-threshold = 20

[[tool.sightline.overrides]]
paths = ["tests", "scripts/*.py"]
rules-off = [33, "speculative-generality"]
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
        assert_eq!(c.complexity_threshold, 20);
        assert_eq!(
            c.overrides,
            vec![Override {
                paths: vec!["tests".to_string(), "scripts/*.py".to_string()],
                rules_off: BTreeSet::from(["33".to_string(), "speculative-generality".to_string()]),
            }]
        );
        // the threshold's default is #23's documented bar
        assert_eq!(Config::new().complexity_threshold, 15);
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
