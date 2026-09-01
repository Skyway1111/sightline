//! sightline: audit / gate / baseline / fix / facts / explain / debug.

mod cli;
mod dump;
mod pipeline;
mod verbs;
mod version;

use anyhow::Result;
use clap::Parser;

// The Windows system heap serializes under rayon's small-allocation load;
// with it, parallel rule walls sum like sequential ones. mimalloc holds the
// per-thread fast path the workload needs.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use cli::{Cli, Command, Debug};
use pipeline::Languages;

/// sightline panicked: the run produced no report and the tree is unjudged.
const EXIT_BUG: u8 = 3;

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    sightline_core::progress::set_quiet(cli.quiet);
    // `db.check()` recurses as deep as the deepest type: the shim gives every
    // worker 16 MB. `--threads` picks the pool's size; without it rayon's own
    // default, every core, stands.
    let mut pool = rayon::ThreadPoolBuilder::new().stack_size(16 * 1024 * 1024);
    if let Some(threads) = cli.threads {
        pool = pool.num_threads(threads);
    }
    pool.build_global().ok();
    // A panic reaches a user as a raw runtime message and exit 101, which no
    // verb documents. The default hook still prints it, once per panicking
    // thread; the catch names the run, points at the tracker and leaves with
    // the documented code (`docs/reference.md`).
    let verb = cli.command.verb();
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(cli)));
    match unwound {
        Ok(Ok(code)) => code.into(),
        Ok(Err(err)) => {
            eprintln!("sightline: {err:#}");
            2.into()
        }
        Err(_) => {
            eprintln!(
                "sightline {}: `{verb}` hit a bug and stopped. Report the panic \
                 above, and the repository it ran on, at {}/issues",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_REPOSITORY"),
            );
            EXIT_BUG.into()
        }
    }
}

fn run(cli: Cli) -> Result<u8> {
    let registry = pipeline::registry()?;
    let langs = Languages::new(&registry);
    match &cli.command {
        Command::Debug(Debug::Dump {
            root,
            layer,
            config,
            out,
        }) => dump::run(
            &registry,
            &langs,
            root,
            layer,
            config.as_deref(),
            out.as_deref(),
        ),
        Command::Explain { rule } => verbs::explain::run(&registry, rule.as_deref()),
        Command::Audit {
            repo,
            json,
            sarif,
            all,
            paths,
            rules,
            profile,
        } => {
            let (root, config) = verbs::root_config(repo)?;
            let opts = verbs::audit::Options {
                json: *json,
                sarif: *sarif,
                all: *all,
                paths: paths.as_deref().unwrap_or_default(),
                rules: rules.as_ref(),
                profile: profile.as_deref(),
            };
            verbs::audit::run(&root, &config, &registry, &langs, &opts)
        }
        Command::Gate {
            repo,
            files,
            since,
            full,
        } => {
            let (root, config) = verbs::root_config(repo)?;
            verbs::gate::run(
                &root,
                &config,
                &registry,
                &langs,
                files.as_deref(),
                since.as_deref(),
                *full,
            )
        }
        Command::Baseline { repo, prune } => {
            let (root, config) = verbs::root_config(repo)?;
            verbs::baseline::run(&root, &config, &registry, &langs, *prune)
        }
        Command::Fix { repo, out, rules } => {
            let (root, config) = verbs::root_config(repo)?;
            verbs::fix::run(
                &root,
                &config,
                &registry,
                &langs,
                out.as_deref(),
                rules.as_ref(),
            )
        }
        Command::Facts { repo, qname } => {
            let (root, config) = verbs::root_config(repo)?;
            verbs::facts::run(&root, &config, &registry, &langs, qname)
        }
    }
}
