//! One module per verb of `cli.py`, and the two helpers they share.

pub mod audit;
pub mod baseline;
pub mod explain;
pub mod facts;
pub mod fix;
pub mod gate;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};

use sightline_core::config::{Config, load_config};
use sightline_core::walk::normpath;

use crate::cli;
use crate::pipeline::resolve;

/// `cli._root_config`: the resolved root and the config that root runs.
pub fn root_config(repo: &cli::Repo) -> Result<(Utf8PathBuf, Config)> {
    let root = resolve(&repo.root)?;
    let config = load_config(&root, repo.config.as_deref().map(Utf8Path::new));
    Ok((root, config))
}

/// `cli._rel_prefixes`: `--paths` as posix rel prefixes, `.` the empty
/// prefix (the whole tree); an absolute path must sit under the root.
pub fn rel_prefixes(root: &Utf8Path, paths: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for p in paths {
        let spelled = p.replace('\\', "/");
        let rel = if Utf8Path::new(&spelled).is_absolute() {
            // `Path.resolve()` answers for a path that does not exist too
            let real = resolve(p).unwrap_or_else(|_| Utf8PathBuf::from(&spelled));
            let under = real
                .strip_prefix(root)
                .map_err(|_| format!("--paths: {p} is outside {root}"))?;
            normpath(&under.as_str().replace('\\', "/"))
        } else {
            normpath(&spelled)
        };
        out.push(if rel == "." { String::new() } else { rel });
    }
    Ok(out)
}

/// Does this finding's path sit under one of the `--paths` prefixes? An
/// empty prefix is the whole tree.
pub fn under(rel: &str, paths: &[String]) -> bool {
    paths
        .iter()
        .any(|p| p.is_empty() || rel == p || rel.starts_with(&format!("{p}/")))
}
