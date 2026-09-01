//! The verb set of `cli.py`: the same verbs, flags, help strings and
//! defaults, declared with clap derive.

use clap::{Args, Parser, Subcommand};

use sightline_core::rule::RuleSet;

#[derive(Parser)]
#[command(name = "sightline", version = crate::version::long(), about = "Rank findings for a repo")]
pub struct Cli {
    /// worker threads (default: every core)
    #[arg(long, global = true, value_name = "N")]
    pub threads: Option<usize>,
    /// no oracle progress lines on stderr (a hook, a CI job)
    #[arg(long, global = true)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args)]
pub struct Repo {
    pub root: String,
    /// config file (read-only checkouts)
    #[arg(long)]
    pub config: Option<String>,
}

/// `cli._rule_ids`: `--rules 32,dead-symbols`, ids or slugs, an unknown mark
/// rejected. The two rules crates are the id and slug tables.
pub fn rule_ids(spec: &str) -> Result<RuleSet, String> {
    let records = sightline_py_rules::RULES
        .iter()
        .map(|r| &r.record)
        .chain(sightline_rs_rules::RULES.iter().map(|r| &r.record));
    let mut ids = RuleSet::new();
    for mark in spec.split(',') {
        let mark = mark.trim();
        let hit = records
            .clone()
            .find(|r| r.slug == mark || r.id == mark)
            .ok_or_else(|| format!("unknown rule {}", sightline_core::pytext::repr_str(mark)))?;
        ids.insert(hit.id.to_string());
    }
    Ok(ids)
}

const RULES_HELP: &str = "run only these rules: ids or slugs, comma-separated";

#[derive(Subcommand)]
pub enum Command {
    /// rank findings for a repo
    Audit {
        #[command(flatten)]
        repo: Repo,
        #[arg(long)]
        json: bool,
        /// SARIF 2.1.0 (GitHub upload)
        #[arg(long, conflicts_with = "json")]
        sarif: bool,
        /// ignore the baseline: every finding, not only new ones
        #[arg(long)]
        all: bool,
        /// report only findings under these paths (facts stay repo-wide)
        #[arg(long, num_args = 1.., value_name = "PATH")]
        paths: Option<Vec<String>>,
        #[arg(long, help = RULES_HELP, value_parser = rule_ids)]
        rules: Option<RuleSet>,
        /// write the per-pass walls of this audit here (`xtask profile`)
        #[arg(long, value_name = "JSON")]
        profile: Option<String>,
    },
    /// blocking check: changed files (fast) or --full
    Gate {
        #[command(flatten)]
        repo: Repo,
        /// files to gate; default: git working-tree diff vs HEAD
        #[arg(long, num_args = 0..)]
        files: Option<Vec<String>>,
        /// also gate files changed in commits since the merge base with REF
        #[arg(long, value_name = "REF")]
        since: Option<String>,
        /// whole audit pipeline (CI)
        #[arg(long)]
        full: bool,
    },
    /// write or prune the ratchet baseline
    Baseline {
        #[command(flatten)]
        repo: Repo,
        #[arg(long)]
        prune: bool,
    },
    /// unified diff of verified mechanical fixes (never touches the tree)
    Fix {
        #[command(flatten)]
        repo: Repo,
        /// write the diff here instead of stdout
        #[arg(long)]
        out: Option<String>,
        #[arg(long, help = RULES_HELP, value_parser = rule_ids)]
        rules: Option<RuleSet>,
    },
    /// what the provers hold about one symbol or module
    Facts {
        #[command(flatten)]
        repo: Repo,
        /// dotted symbol or module qname
        qname: String,
    },
    /// a rule's meaning and goal; without one, the roster of every rule
    Explain { rule: Option<String> },
    /// what a layer of the pipeline holds
    #[command(subcommand)]
    Debug(Debug),
}

impl Command {
    /// What the run was doing, as a panic report names it.
    pub fn verb(&self) -> &'static str {
        match self {
            Command::Audit { .. } => "audit",
            Command::Gate { .. } => "gate",
            Command::Baseline { .. } => "baseline",
            Command::Fix { .. } => "fix",
            Command::Facts { .. } => "facts",
            Command::Explain { .. } => "explain",
            Command::Debug(_) => "debug dump",
        }
    }
}

#[derive(Subcommand)]
pub enum Debug {
    /// one JSON document per layer of the pipeline
    Dump {
        root: String,
        /// `all`, one layer, or a comma-separated list; a list comes off one
        /// build of the tree
        #[arg(long, value_name = "L")]
        layer: String,
        /// config file (read-only checkouts)
        #[arg(long)]
        config: Option<String>,
        /// one layer's file, or the directory a layer list fills
        #[arg(short = 'o', long, value_name = "FILE")]
        out: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_ids_takes_an_id_or_a_slug_and_rejects_an_unknown_mark() {
        assert_eq!(
            rule_ids("32, dead-symbols"),
            Ok(["32".to_string()].into_iter().collect())
        );
        assert_eq!(
            rule_ids("section-comments,32"),
            Ok(["18".to_string(), "32".to_string()].into_iter().collect())
        );
        // a Rust reading shares its sibling's slug, so one mark names both
        assert_eq!(
            rule_ids("structural-clones"),
            Ok(["11".to_string()].into_iter().collect())
        );
        assert_eq!(rule_ids("nope"), Err("unknown rule 'nope'".to_string()));
    }

    #[test]
    fn the_declared_surface_is_the_one_cli_py_parses() {
        use clap::CommandFactory;
        let command = Cli::command();
        command.clone().debug_assert();

        let mut verbs: Vec<&str> = command.get_subcommands().map(|c| c.get_name()).collect();
        verbs.sort_unstable();
        assert_eq!(
            verbs,
            [
                "audit", "baseline", "debug", "explain", "facts", "fix", "gate"
            ]
        );

        // the two flags every verb takes; `--quiet` reaches the oracle
        // printers wherever a verb runs them
        let mut global: Vec<&str> = command
            .get_arguments()
            .filter(|a| a.is_global_set())
            .map(clap::Arg::get_id)
            .map(clap::Id::as_str)
            .collect();
        global.sort_unstable();
        assert_eq!(global, ["quiet", "threads"]);
    }

    /// `explain` answers with the roster where no id is named.
    #[test]
    fn explain_takes_an_id_or_nothing() {
        use clap::CommandFactory;
        let explain = Cli::command();
        let explain = explain
            .get_subcommands()
            .find(|c| c.get_name() == "explain")
            .expect("the explain verb");
        assert!(!explain.get_positionals().any(clap::Arg::is_required_set));
    }
}
