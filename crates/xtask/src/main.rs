//! The workspace's own tooling: the gate, the rulers and the receipts.
//!
//! Exit codes: 0 pass, 1 fail, 2 usage.

mod audit_bench;
mod banned;
mod bench_tables;
mod catalog;
mod check;
mod corpus;
mod fence;
mod fix_check;
mod gate_bench;
mod gauntlet;
mod install;
mod mt;
mod paths;
mod perf_catalog;
mod precision_sample;
mod profile;
mod retired;
mod surface;
mod text;
mod third_party;
mod worktree;

use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command>

  retired [--from <tsv>]
  banned <file>... | banned --tree
  check [--slow]
  install
  surface
  fence
  third-party [--check]
  catalog [--timeout N] [--self-test] [--python <exe>]
  perf-catalog [--self-test] [--python <exe>]
  precision-sample [--rules 35,36] [--arms cause,...] <audit.json> <root>...
  bench-tables [out-dir] [--doc PATH]
  corpus [out-dir] [--repeat-for-determinism] [--diff-against=DIR] [--lang=py|rs]
  fix-check [out-dir] [repo-name ...]
  gate-bench <repo-root> <full-audit-json> [config] [--suffix=.rs]
  audit-bench [repo-name ...] [--n N] [--reference PATH]
  profile [repo-name] [--json OUT] [--reference PATH]
  gauntlet count <repo-root> [--json]
  gauntlet sheet <audit.json> <out.tsv> [--carry <earlier.tsv>] [--rules 1,50]
  gauntlet tally <sheet.tsv>... [--bar 0.3] [--min-n 5]
  gauntlet clone [--lang py|rs|rs2a] [--held-out] [--ext <dir>]
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let code = match args.first().map(String::as_str) {
        Some("retired") => retired::main(&rest),
        Some("banned") => banned::main(&rest),
        Some("check") => check::main(&rest),
        Some("install") => install::main(&rest),
        Some("surface") => surface::main(&rest),
        Some("fence") => fence::main(&rest),
        Some("third-party") => third_party::main(&rest),
        Some("catalog") => catalog::main(&rest),
        Some("perf-catalog") => perf_catalog::main(&rest),
        Some("precision-sample") => precision_sample::main(&rest),
        Some("bench-tables") => bench_tables::main(&rest),
        Some("corpus") => corpus::main(&rest),
        Some("fix-check") => fix_check::main(&rest),
        Some("gate-bench") => gate_bench::main(&rest),
        Some("audit-bench") => audit_bench::main(&rest),
        Some("profile") => profile::main(&rest),
        Some("gauntlet") => gauntlet::main(&rest),
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match code {
        Ok(n) => ExitCode::from(n),
        Err(e) => {
            eprintln!("xtask: {e:#}");
            ExitCode::from(1)
        }
    }
}
