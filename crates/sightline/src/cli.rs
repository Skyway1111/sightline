//! The command line: verbs, flags, help strings and defaults, declared
//! with clap derive.

use clap::{Args, Parser, Subcommand};

use sightline_core::rule::RuleSet;

#[derive(Parser)]
#[command(
    name = "sightline",
    version = crate::version::long(),
    about = "A linter for code an agent wrote: ranked findings for a Python or Rust repository",
    after_help = "Docs and source: https://github.com/Skyway1111/sightline\n\
                  `sightline explain` prints the rule roster; `sightline explain <id>` prints one rule."
)]
pub struct Cli {
    /// Worker threads (default: every core)
    #[arg(long, global = true, value_name = "N")]
    pub threads: Option<usize>,
    /// No oracle progress lines on stderr, for a hook or a CI job
    #[arg(long, global = true)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args)]
pub struct Repo {
    /// The repository to read
    pub root: String,
    /// Read the config from this file instead of the tree, for a checkout you cannot write to
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

const RULES_HELP: &str = "Run only these rules: ids or slugs, comma-separated";

#[derive(Subcommand)]
pub enum Command {
    /// Report every finding in a repository, worst first
    Audit {
        #[command(flatten)]
        repo: Repo,
        /// Write the report as one JSON document
        #[arg(long)]
        json: bool,
        /// Write SARIF 2.1.0, which GitHub code scanning accepts
        #[arg(long, conflicts_with = "json")]
        sarif: bool,
        /// Ignore the baseline and report every finding, not only new ones
        #[arg(long)]
        all: bool,
        /// Report only findings under these paths (facts stay repo-wide)
        #[arg(long, num_args = 1.., value_name = "PATH")]
        paths: Option<Vec<String>>,
        #[arg(long, help = RULES_HELP, value_parser = rule_ids)]
        rules: Option<RuleSet>,
        /// Write this audit's per-pass walls to a JSON file
        #[arg(long, value_name = "JSON")]
        profile: Option<String>,
    },
    /// Block on the files a change touched, or on the whole tree with --full
    Gate {
        #[command(flatten)]
        repo: Repo,
        /// Files to gate (default: the git working-tree diff against HEAD)
        #[arg(long, num_args = 0..)]
        files: Option<Vec<String>>,
        /// Also gate files changed in commits since the merge base with REF
        #[arg(long, value_name = "REF")]
        since: Option<String>,
        /// Run the whole audit pipeline, for CI
        #[arg(long)]
        full: bool,
    },
    /// Write the baseline that RATCHET rules block against
    Baseline {
        #[command(flatten)]
        repo: Repo,
        /// Drop baseline keys no finding claims any more
        #[arg(long)]
        prune: bool,
    },
    /// Print a unified diff of verified fixes; never writes to the tree
    Fix {
        #[command(flatten)]
        repo: Repo,
        /// Write the diff to this file instead of stdout
        #[arg(long)]
        out: Option<String>,
        #[arg(long, help = RULES_HELP, value_parser = rule_ids)]
        rules: Option<RuleSet>,
    },
    /// Print what the provers hold about one symbol or module
    Facts {
        #[command(flatten)]
        repo: Repo,
        /// Dotted symbol or module name, as the report spells it
        qname: String,
    },
    /// Print one rule's meaning, goal and measured precision; with no rule, the roster
    Explain {
        /// Rule id or slug
        rule: Option<String>,
        /// Every rule's record and measured rows as one JSON document
        #[arg(long, conflicts_with = "rule")]
        json: bool,
    },
    /// Dump what a layer of the pipeline holds
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
    /// Write one JSON document per layer of the pipeline
    Dump {
        /// The repository to read
        root: String,
        /// `all`, one layer, or a comma-separated list (a list comes off one
        /// build of the tree)
        #[arg(long, value_name = "L")]
        layer: String,
        /// Read the config from this file instead of the tree
        #[arg(long)]
        config: Option<String>,
        /// One layer's file, or the directory a layer list fills
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
