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
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace
cargo run -p sightline-lint -- --version
```

The tests that spawn the type checker or cargo are `#[ignore]`, so
`cargo test --workspace` is the unit half.

Four of the repository's own checks run anywhere as well:

```
cargo xtask banned --tree
cargo xtask surface
cargo xtask fence
cargo xtask rules-doc --check
```

`banned --tree` is a prose lint. It fails on an em dash, on a short list of
words this repository's comments and docs once reached for in place of a
mechanism or a number, and on citations of the plan and the Python tool the
rewrite replaced, which a reader here cannot open. The lists sit in
`crates/xtask/src/banned.rs` with the reason for each. It scans every text
file `git ls-files` names, code included, so stage a new file before you run
it. `surface` counts non-test lines under `crates/` against a bound, which
a commit that needs a higher one argues for. `fence` proves that no rules crate
depends on a parser or an oracle crate. `rules-doc --check` proves that
`docs/rules.md` matches the rule records the binary holds; a change to a rule's
record regenerates it with `cargo xtask rules-doc`.

Open a pull request with those eight green.

## The corpus trees

`cargo xtask check` ends with a stage that audits the clean Python
repository, one of six public repositories the gate reads from the corpus root:
`../sightline-corpus/`, beside this checkout, or the directory
`SIGHTLINE_CORPUS_ROOT` names. `crates/xtask/corpus.toml` names each one's
url, config, language, role and pin. Clone them there, checked out at their
pin, and give each Python one a `.venv` holding its dependencies, or the
oracle resolves that tree's imports against yours. A stage that cannot find
its tree prints the `git clone` that fixes it.

The stages before that one are the CI lanes plus a release build and
Sightline's audit of its own tree, so the commands above cover them without a
single corpus tree.

`cargo xtask check --slow` and the measurement commands read those trees
too: `corpus`,
`fix-check`, `audit-bench`, `profile`, `bench-tables`, `precision-sample` and
the `gauntlet` subcommands. A maintainer runs them before a merge.

## Report a false positive

A finding Sightline should not have made is the report worth sending. Each rule
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
4. Why the finding is wrong. Name what Sightline missed: the caller it did not
   resolve, the dynamic reference it cannot follow, the reason two blocks it
   read as one idea are two.
5. The config the run read, when the repository has one. `hot-roots` and
   `published` both change what the rules conclude.

To silence a finding in your own tree meanwhile, write `sightline-ok: <id or
slug>` above the line. [docs/reference.md](docs/reference.md) holds the syntax.

## Change a rule

A rule's precision and recall are both measured, on trees a contributor cannot
reach. A change to a rule's arms is measured on both before it lands: the false
positives it removes and the true positives it loses. Open an issue before you
write the code, so the measurement is agreed first.
