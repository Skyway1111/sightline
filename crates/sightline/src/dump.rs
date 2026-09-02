//! `debug dump`: one layer of the pipeline as JSON, for reading what a stage
//! holds without a debugger.
//!
//! The audited tree is the live root when it is clean and a detached worktree
//! at HEAD when it is dirty. The head names the live root either way, and its
//! `sha` says which tree the layer describes.

use std::collections::BTreeMap;
use std::io::Write;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde_json::{Map, Value};

use sightline_core::config::{Config, load_config};
use sightline_core::git::{Worktree, head_sha, working_tree_dirty};
use sightline_core::lang::{BuildMode, Stack};
use sightline_core::rank::rank;
use sightline_core::registry::Registry;
use sightline_core::render::{AuditResult, to_json, to_sarif, to_text};
use sightline_core::rule::RuleSet;
use sightline_core::{pyjson, walk};

use crate::pipeline::{self, Languages, resolve};

const DIRTY: &str = " (dirty live tree: audited a worktree at HEAD)";

/// The `traversal` layer alone is written without indent: it is millions of
/// spans.
fn render(doc: &Value, layer: &str) -> String {
    let text = if layer == "traversal" {
        pyjson::dumps_compact(doc)
    } else {
        pyjson::dumps(doc)
    };
    text + "\n"
}

pub fn run(
    registry: &Registry,
    langs: &Languages,
    root: &str,
    layer: &str,
    config_path: Option<&str>,
    out: Option<&str>,
) -> Result<u8> {
    let live = resolve(root)?;
    let dirty = working_tree_dirty(&live);
    let worktree = dirty
        .then(|| Worktree::add(&live).context("git worktree add failed"))
        .transpose()?;
    let audited = worktree.as_ref().map_or(live.clone(), |w| w.path.clone());
    let config_path = config_path.map(resolve).transpose()?;
    let mut config = load_config(&audited, config_path.as_deref());
    // a worktree holds no `.venv`, and without the target's interpreter the
    // checker leaves its third-party imports unresolved
    // (`fix_check.worktree_config`)
    if worktree.is_some() {
        let venv = live.join(".venv");
        if venv.is_dir() {
            config.python_env = Some(venv.as_str().replace('\\', "/"));
        }
    }

    let many = layer == "all" || layer.contains(',');
    let layers: Vec<&str> = match layer {
        "all" => ALL.to_vec(),
        list => list.split(',').collect(),
    };
    let sha = head_sha(&live) + if dirty { DIRTY } else { "" };
    let docs = layer_documents(registry, langs, &audited, &config, &layers)?;

    let mut texts: Vec<(&str, String)> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();
    for name in &layers {
        let Some(doc) = docs.built.get(*name) else {
            missing.push(name);
            continue;
        };
        let mut head = Map::new();
        head.insert("root".into(), Value::from(live.as_str()));
        head.insert("sha".into(), Value::from(sha.as_str()));
        head.insert("layer".into(), Value::from(*name));
        head.insert("notes".into(), Value::from(docs.notes.clone()));
        if let Value::Object(doc) = doc.clone() {
            head.extend(doc);
        }
        texts.push((name, render(&Value::Object(head), name)));
    }
    if !missing.is_empty() {
        eprintln!("no stack answers layer {}", missing.join(", "));
        if !many {
            return Ok(2);
        }
    }
    match out {
        // a layer list names a directory, one layer a file
        Some(path) if many => {
            let dir = Utf8Path::new(path);
            std::fs::create_dir_all(dir)?;
            for (name, text) in &texts {
                std::fs::write(dir.join(format!("{name}.json")), text.as_bytes())?;
            }
        }
        Some(path) => std::fs::write(path, texts[0].1.as_bytes())?,
        None => {
            for (_, text) in &texts {
                std::io::stdout().write_all(text.as_bytes())?;
            }
        }
    }
    Ok(0)
}

/// The layers the whole pipeline writes rather than one stack: they read
/// every stack's findings as one list.
const PIPELINE: &[&str] = &["audit", "text", "sarif", "fix"];

/// Every layer: what `--layer all` asks for, and the order one build answers
/// them in.
const ALL: &[&str] = &[
    "listing",
    "facts",
    "traversal",
    "scope",
    "graph",
    "world",
    "effects",
    "liveness",
    "imports",
    "hot",
    "records",
    "clones",
    "oracle",
    "verify",
    "neutral",
    "raw",
    "audit",
    "text",
    "sarif",
    "fix",
    "rs-facts",
    "rs-bodies",
    "rs-graph",
    "rs-world",
    "rs-clones",
];

/// What one build of the tree answers, and the notes every layer's head
/// holds: the stack layers written before `close`, the renders after it, one
/// header for all of them.
struct Documents {
    built: BTreeMap<String, Value>,
    notes: Vec<String>,
}

fn layer_documents(
    registry: &Registry,
    langs: &Languages,
    root: &Utf8Path,
    config: &Config,
    layers: &[&str],
) -> Result<Documents> {
    let listing = walk::discover(root, config);
    let off: RuleSet = config.rules_off.clone();
    let mut stacks =
        pipeline::build_stacks(root, config, langs, &listing, None, &off, BuildMode::Full)?;
    // the layers are written while the checker still answers and the header
    // after `close`: `raw` runs the rules, and a crash under them belongs in
    // the notes the header prints
    let mut built = BTreeMap::new();
    let mut rendered: Vec<&str> = Vec::new();
    for layer in layers {
        if PIPELINE.contains(layer) {
            rendered.push(layer);
            continue;
        }
        if let Some(doc) = stacks.iter().find_map(|s| s.dump(layer)) {
            built.insert((*layer).to_string(), doc);
        }
    }
    if rendered.is_empty() {
        let mut notes: Vec<String> = Vec::new();
        for stack in &mut stacks {
            stack.close();
            notes.extend(stack.notes());
        }
        notes.sort();
        return Ok(Documents { built, notes });
    }
    let notes = renders(root, config, registry, stacks, &rendered, &mut built)?;
    Ok(Documents { built, notes })
}

/// `gate.collect` then the verbs the layers name: every stack's rules into
/// one list, suppressed as one, the emitter run per stack while the checker
/// still answers, then `close` and the renders off the header it leaves.
fn renders(
    root: &Utf8Path,
    config: &Config,
    registry: &Registry,
    stacks: Vec<Box<dyn Stack>>,
    layers: &[&str],
    docs: &mut BTreeMap<String, Value>,
) -> Result<Vec<String>> {
    let off: RuleSet = config.rules_off.clone();
    let mut collected = pipeline::collect_stacks(stacks, registry, &off, true);
    let (diff, _) = pipeline::fix_diff(&collected.repo, &collected.kept);
    pipeline::close(&mut collected.repo, &mut collected.walls);
    let repo = collected.repo;
    let (mut notes, provers) = pipeline::header(&repo);
    let result = AuditResult {
        findings: rank(collected.kept, &repo),
        suppressed: collected.suppressed.len(),
        absorbed: 0,
        notes: notes.clone(),
        facts: &repo,
        provers,
        rules_off: config.rules_off.iter().cloned().collect(),
        rules_only: Vec::new(),
        paths: Vec::new(),
    };
    for layer in layers {
        let doc = match *layer {
            "audit" => serde_json::json!({ "output": to_json(&result, registry) }),
            "text" => serde_json::json!({ "output": to_text(&result) }),
            "sarif" => serde_json::json!({ "output": to_sarif(&result, registry) }),
            _ => fix_document(root, &diff)?,
        };
        docs.insert((*layer).to_string(), doc);
    }
    notes.sort();
    Ok(notes)
}

/// The `fix` layer: the diff's header set and, per file it names, the text
/// `git apply` leaves in a copy of the tree.
fn fix_document(root: &Utf8Path, diff: &str) -> Result<Value> {
    let mut headers: Vec<&str> = diff
        .lines()
        .filter(|line| line.starts_with("# sightline-fix: "))
        .collect();
    headers.sort_unstable();
    let mut rels: Vec<&str> = diff
        .lines()
        .filter_map(|line| line.strip_prefix("+++ b/"))
        .map(str::trim_end)
        .collect();
    rels.sort_unstable();
    rels.dedup();

    let mut files = Map::new();
    if !rels.is_empty() {
        let dir = tempfile::Builder::new()
            .prefix("sightline-dump-fix-")
            .tempdir()?;
        let tree = Utf8PathBuf::from(dir.path().to_string_lossy().into_owned());
        for rel in &rels {
            let target = tree.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(root.join(rel), &target)?;
        }
        let patch = tree.join("layer.diff");
        std::fs::write(&patch, diff.as_bytes())?;
        let applied = std::process::Command::new("git")
            .args(["apply", patch.as_str()])
            .current_dir(&tree)
            .output()
            .context("running git apply")?;
        if !applied.status.success() {
            anyhow::bail!(
                "fix layer: git apply rejected the diff:\n{}",
                String::from_utf8_lossy(&applied.stderr)
            );
        }
        for rel in &rels {
            // the layer holds the patched text with LF endings on every
            // platform, as every writer in this workspace does
            let text = std::fs::read_to_string(tree.join(rel))?;
            let text = if text.contains('\r') {
                text.replace("\r\n", "\n").replace('\r', "\n")
            } else {
                text
            };
            files.insert((*rel).to_string(), Value::from(text));
        }
    }
    Ok(serde_json::json!({ "headers": headers, "files": files }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--layer all` names every layer once, the pipeline four among them.
    #[test]
    fn the_all_span_holds_every_layer_once() {
        let mut sorted = ALL.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 25);
        assert_eq!(ALL.len(), 25);
        for layer in PIPELINE {
            assert!(ALL.contains(layer), "{layer} is not in --layer all");
        }
    }
}
