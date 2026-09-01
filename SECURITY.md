# Security policy

## Report a vulnerability

Report privately through GitHub. Open the repository's Security tab, choose
Report a vulnerability, and write it up there. That opens a private advisory
only the maintainers can read. Do not open a public issue for a vulnerability.

Include the version `sightline --version` prints, your platform, and the
smallest repository or input that shows the problem.

Expect a first reply within seven days. A fix ships in the next release, and
the advisory is published with it.

A finding sightline gets wrong is not a vulnerability. Report it as a false
positive, which [CONTRIBUTING.md](CONTRIBUTING.md) describes.

## Supported versions

The latest release. Nothing is backported.

## An audit runs the audited tree's code

Whenever the audited repository holds Rust, `sightline audit`, `sightline gate
--full`, `sightline fix`, `sightline facts` and `sightline debug dump` run code
from it. Two paths do that:

- The oracle runs `cargo metadata --no-deps` and `cargo check --workspace
  --all-targets --keep-going` in every project root of the tree. Cargo runs
  each crate's `build.rs`, and the build script of every dependency it has to
  build.
- The index loads the workspace through rust-analyzer's loader, with
  `load_out_dirs_from_check` on and a proc-macro server from the sysroot.
  Expanding a macro runs the proc-macro crate's own code.

That code runs as you, with your privileges, in your session. The exposure is
the same as `cargo build`, `cargo check`, or opening the tree in an editor
backed by rust-analyzer. Audit a Rust repository only when you would build it.

Nothing sandboxes the build, and sightline does not try to. Audit a tree you do
not trust inside a container or a virtual machine.

Two limits hold whatever the tree does:

- Both cargo passes run with `CARGO_NET_OFFLINE=true`, so cargo fetches
  nothing. A dependency missing from the local registry cache leaves the check
  failing and the oracle silent, and the provenance header says so.
- Cargo builds into a directory outside every tree sightline reads, so a build
  never writes into the audited repository.
  [docs/reference.md](docs/reference.md) names that directory.

`sightline gate` without `--full` builds no oracle and runs no cargo. Its
header says `fast gate: oracle and repo-scope rules not run`.

An audit of a Python tree runs no code from the tree. The type checker reads
source and stubs, and imports nothing. Beyond cargo, the only program sightline
runs is `git`, in the audited root, to read the diff and the history.

## In CI

`sightline gate . --full` on a `pull_request` event runs the fork's build
scripts on the runner. Any CI job that builds a pull request does the same, and
the `pull_request` event already denies that job your secrets and a write
token.

Do not move the job to `pull_request_target`. That event hands the job the base
repository's secrets and a write token, so a workflow that checks out the pull
request's head and audits it there gives a fork's build script both.
