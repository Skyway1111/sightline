//! Where the target's Python environment is: the interpreter the oracle
//! resolves imports against. Without one the checker reads the audit
//! machine's packages, which is the silent downgrade the header names.

use camino::{Utf8Path, Utf8PathBuf};
use sha2::{Digest, Sha256};

/// Where `detect` looks, in order, spelled for the header note.
pub const CANDIDATES: &str = "python-env, VIRTUAL_ENV, CONDA_PREFIX, UV_PROJECT_ENVIRONMENT, \
                              .venv, venv, env, poetry's cache";

/// The interpreter under an environment directory.
const EXE: &str = if cfg!(windows) {
    "Scripts/python.exe"
} else {
    "bin/python"
};

/// The target repo's interpreter. The first environment that holds one
/// wins: the config's `python-env` (under the root, or absolute), the one
/// `VIRTUAL_ENV`, `CONDA_PREFIX` or `UV_PROJECT_ENVIRONMENT` names, `.venv`,
/// `venv` or `env` under the root, then the poetry cache's environment for
/// a root holding `poetry.lock`. `var` reads the environment, so a test can
/// hand one in.
pub fn detect(
    root: &Utf8Path,
    configured: Option<&str>,
    var: impl Fn(&str) -> Option<String>,
) -> Option<Utf8PathBuf> {
    let mut dirs: Vec<Utf8PathBuf> = configured.map(|c| root.join(c)).into_iter().collect();
    for name in ["VIRTUAL_ENV", "CONDA_PREFIX", "UV_PROJECT_ENVIRONMENT"] {
        dirs.extend(var(name).map(|v| root.join(v)));
    }
    dirs.extend([".venv", "venv", "env"].map(|d| root.join(d)));
    dirs.extend(poetry_env(root, &var));
    dirs.into_iter()
        .map(|dir| dir.join(EXE))
        .find(|p| p.is_file())
}

/// The environment poetry made for this root, in its cache: named
/// `<name>-<hash>-py<version>`, the hash being the first eight url-safe
/// base64 characters of the sha256 of the root's path. The newest Python
/// wins where several sit there.
fn poetry_env(root: &Utf8Path, var: &impl Fn(&str) -> Option<String>) -> Option<Utf8PathBuf> {
    if !root.join("poetry.lock").is_file() {
        return None;
    }
    let cache = match var("POETRY_CACHE_DIR") {
        Some(dir) => Utf8PathBuf::from(dir),
        None if cfg!(windows) => Utf8PathBuf::from(var("LOCALAPPDATA")?).join("pypoetry/Cache"),
        None if cfg!(target_os = "macos") => {
            Utf8PathBuf::from(var("HOME")?).join("Library/Caches/pypoetry")
        }
        None => match var("XDG_CACHE_HOME") {
            Some(xdg) => Utf8PathBuf::from(xdg).join("pypoetry"),
            None => Utf8PathBuf::from(var("HOME")?).join(".cache/pypoetry"),
        },
    };
    let prefix = poetry_prefix(root, &project_name(root)?);
    let mut found: Vec<Utf8PathBuf> = std::fs::read_dir(cache.join("virtualenvs"))
        .ok()?
        .flatten()
        .filter_map(|e| Utf8PathBuf::from_path_buf(e.path()).ok())
        .filter(|p| p.file_name().is_some_and(|n| n.starts_with(&prefix)))
        .collect();
    found.sort();
    found.pop()
}

/// `<name>-<hash>-py`, as poetry's `generate_env_name` spells it: the name
/// lowercased with its shell characters blanked and cut at 42, the path
/// hashed as `os.path.normcase(os.path.realpath(cwd))` (lowercase with
/// backslashes on Windows, itself elsewhere).
fn poetry_prefix(root: &Utf8Path, name: &str) -> String {
    let sanitized: String = name
        .to_lowercase()
        .chars()
        .map(|c| match c {
            ' ' | '$' | '`' | '!' | '*' | '@' | '"' | '\\' | '\r' | '\n' | '\t' => '_',
            other => other,
        })
        .take(42)
        .collect();
    let spelled = if cfg!(windows) {
        root.as_str().replace('/', "\\").to_lowercase()
    } else {
        root.as_str().to_string()
    };
    let digest = Sha256::digest(spelled.as_bytes());
    format!("{sanitized}-{}-py", base64_urlsafe(&digest[..6]))
}

/// The name `[project]` or `[tool.poetry]` gives the distribution.
fn project_name(root: &Utf8Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("pyproject.toml")).ok()?;
    let doc: toml::Table = text.parse().ok()?;
    let poetry = doc.get("tool").and_then(|t| t.get("poetry"));
    let table = doc.get("project").or(poetry)?;
    Some(table.get("name")?.as_str()?.to_string())
}

/// The url-safe alphabet over whole 3-byte groups: eight characters for six
/// bytes, which is all the poetry name reads.
fn base64_urlsafe(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    bytes
        .chunks_exact(3)
        .flat_map(|c| {
            let n = (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]);
            (0..4).map(move |i| ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn write(root: &Utf8Path, files: &[(&str, &str)]) {
        for (rel, text) in files {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
    }

    fn tmp() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap().to_path_buf();
        (dir, root)
    }

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn the_config_then_the_variables_then_the_root_dirs() {
        let (_dir, root) = tmp();
        assert_eq!(detect(&root, None, none), None);
        write(&root, &[(&format!("venv/{EXE}"), "")]);
        assert_eq!(detect(&root, None, none), Some(root.join("venv").join(EXE)));
        write(&root, &[(&format!(".venv/{EXE}"), "")]);
        assert_eq!(
            detect(&root, None, none),
            Some(root.join(".venv").join(EXE))
        );
        // a variable outranks the root's own dirs, an absolute one included
        write(&root, &[(&format!("elsewhere/{EXE}"), "")]);
        let elsewhere = root.join("elsewhere");
        let vars = HashMap::from([("CONDA_PREFIX".to_string(), elsewhere.to_string())]);
        assert_eq!(
            detect(&root, None, |k| vars.get(k).cloned()),
            Some(elsewhere.join(EXE))
        );
        // the config outranks everything, and one that holds no interpreter
        // falls through
        write(&root, &[(&format!("customenv/{EXE}"), "")]);
        assert_eq!(
            detect(&root, Some("customenv"), |k| vars.get(k).cloned()),
            Some(root.join("customenv").join(EXE))
        );
        assert_eq!(
            detect(&root, Some("missing"), none),
            Some(root.join(".venv").join(EXE))
        );
    }

    #[test]
    fn the_poetry_env_is_found_by_its_name_and_hash() {
        let (_dir, root) = tmp();
        let project = root.join("proj");
        write(
            &project,
            &[
                ("pyproject.toml", "[tool.poetry]\nname = \"My Pkg\"\n"),
                ("poetry.lock", ""),
            ],
        );
        let prefix = poetry_prefix(&project, "My Pkg");
        assert!(
            prefix.starts_with("my_pkg-") && prefix.ends_with("-py"),
            "{prefix}"
        );
        assert_eq!(prefix.len(), "my_pkg-".len() + 8 + "-py".len());
        let cache = root.join("cache");
        for version in ["3.11", "3.12"] {
            write(
                &cache,
                &[(&format!("virtualenvs/{prefix}{version}/{EXE}"), "")],
            );
        }
        write(
            &cache,
            &[(&format!("virtualenvs/other-abcdefgh-py3.12/{EXE}"), "")],
        );
        let vars = HashMap::from([("POETRY_CACHE_DIR".to_string(), cache.to_string())]);
        assert_eq!(
            detect(&project, None, |k| vars.get(k).cloned()),
            Some(cache.join(format!("virtualenvs/{prefix}3.12")).join(EXE))
        );
        // no lock file: not a poetry root
        std::fs::remove_file(project.join("poetry.lock")).unwrap();
        assert_eq!(detect(&project, None, |k| vars.get(k).cloned()), None);
    }

    #[test]
    fn the_hash_is_poetrys() {
        // `base64.urlsafe_b64encode(hashlib.sha256(b"abc").digest())[:8]`
        let digest = Sha256::digest(b"abc");
        assert_eq!(base64_urlsafe(&digest[..6]), "ungWv48B");
    }
}
