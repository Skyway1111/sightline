# Contributing

## Set up the checkout

On Windows, allow long paths before you clone. One file in the type-checker
fork has a path longer than 260 characters, and the checkout stops without it:

```
git config --global core.longpaths true
```

`rust-toolchain.toml` pins Rust 1.97.1, and rustup picks that toolchain up
inside the checkout. Every file in the workspace is written with LF.

## What you can run

CI runs four lanes. Each one runs on any machine:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo install --path crates/sightline --locked
```

The tests that spawn the type checker or cargo are `#[ignore]`, so
`cargo test --workspace` is the unit half.

Three of the repository's own checks run anywhere as well:

```
cargo xtask banned --tree
cargo xtask surface
cargo xtask fence
```

`banned --tree` scans the tree for a word list and the em dash. It reads `git
ls-files`, so a file you have not staged is invisible to it. Run `git add`
first. `surface` counts non-test lines under `crates/` against a bound, which
a commit that needs a higher one argues for. `fence` proves that no rules crate
depends on a parser or an oracle crate.

Open a pull request with those seven green.

## The corpus trees

`cargo xtask check` ends with a stage that audits the Python clean pole, one
of six public repositories the gate expects beside the checkout.
`crates/xtask/corpus.toml` names each one's url, config, language, role and
pin. Clone them as siblings of this directory, checked out at their pin, and
give each Python one a `.venv` holding its dependencies, or the oracle
resolves that tree's imports against yours. A stage that cannot find its tree
prints the `git clone` that fixes it.

The stages before that one are the CI lanes plus a release build and
sightline's audit of its own tree, so the commands above cover them without a
single corpus tree.

`cargo xtask check --slow` and the rulers read those trees too: `corpus`,
`fix-check`, `audit-bench`, `profile`, `bench-tables`, `precision-sample` and
the `gauntlet` subcommands. A maintainer runs them before a merge.

## Report a false positive

A finding sightline should not have made is the report worth sending. Each rule
has a measured precision, in `benchmarks.md` and `data/precision.toml`, and a
false positive is evidence against that number.

Run `sightline explain <id>` first. The rule's record says what the rule checks
and what it was measured at. A finding that sits inside the rule's stated arms
and is still wrong is the one to report.

Open an issue with the false positive template, which asks for five things:

1. The provenance header, which is the first line `sightline audit ROOT`
   prints. It names the version and what the run saw.
2. The finding, from `sightline audit ROOT --json`. One entry of `findings` is
   enough. It holds the rule id, the file, the line, the message and the tier.
3. The code the finding points at, or a public repository that shows it.
4. Why the finding is wrong. Name what sightline missed: the caller it did not
   resolve, the dynamic reference it cannot follow, the reason two blocks it
   read as one idea are two.
5. The config the run read, when the repository has one. `hot-roots` and
   `published` both change what the rules conclude.

To silence a finding in your own tree meanwhile, write `sightline-ok: <id or
slug>` above the line. [docs/reference.md](docs/reference.md) holds the syntax.

## Change a rule

A rule's precision and recall are both measured, on trees a contributor cannot
reach. A change to a rule's arms is priced on both sides before it lands. Open
an issue before you write the code, so the measurement is agreed first.
