# Sightline

Sightline is a linter for code an agent wrote. The code compiles and the tests
pass; what it costs is a reader's attention. Sightline reads a repository once,
proves what it can with a type checker and a call graph, and prints findings
ordered by the measured chance each one is real.

It reads Python and Rust, and it ships as one binary with no runtime to install.

## Install

One line. The script downloads the archive for your platform from the latest
release, unpacks it, and puts `sightline` on your PATH.

macOS or Linux:

```
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Skyway1111/sightline/releases/latest/download/sightline-lint-installer.sh | sh
```

Windows:

```
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Skyway1111/sightline/releases/latest/download/sightline-lint-installer.ps1 | iex"
```

Where a script piped into a shell is not allowed, take the archive and its
checksum from the
[releases page](https://github.com/Skyway1111/sightline/releases) and verify
it before unpacking. Windows x64, Linux x64 and macOS arm64 are built for every
release; each archive holds the binary, LICENSE, THIRD-PARTY.md and this file:

```
curl -sSfLO https://github.com/Skyway1111/sightline/releases/latest/download/sightline-lint-x86_64-unknown-linux-gnu.tar.xz
curl -sSfLO https://github.com/Skyway1111/sightline/releases/latest/download/sightline-lint-x86_64-unknown-linux-gnu.tar.xz.sha256
sha256sum --check sightline-lint-x86_64-unknown-linux-gnu.tar.xz.sha256
tar -xf sightline-lint-x86_64-unknown-linux-gnu.tar.xz
```

`sha256.sum` on the same page lists every archive's digest in one file.

For a commit hook, the repository ships a
[pre-commit](https://pre-commit.com) definition that runs the fast gate over
the staged files with the `sightline` on your PATH:

```yaml
repos:
  - repo: https://github.com/Skyway1111/sightline
    rev: v0.3.0
    hooks:
      - id: sightline-gate
```

For GitHub Actions, `uses: Skyway1111/sightline@v0.3.0` installs a release on
a Linux runner and runs one verb; the CI section below shows it.

Check the install:

```
sightline --version
```

It prints the crate version, the version of the type-checker fork and the
`ra_ap_*` version the Rust index was compiled against:

```
sightline 0.3.0 (ty-unnecessary 0.1.0, ra_ap 0.0.328)
```

## Build from source

The crate on crates.io is `sightline-lint`; the binary it installs is
`sightline`. Another crate holds the bare name. The type-checker fork it
builds on is published beside it as `sightline-ruff-*` and `sightline-ty-*`,
each keeping its upstream library name.

The workspace pins Rust 1.97.1 in `rust-toolchain.toml`. rustup reads that file
from your current directory, not from the source that cargo fetches for you, so
name the toolchain on the command line:

```
rustup toolchain install 1.97.1
cargo +1.97.1 install sightline-lint --locked
```

From a checkout, `cargo +1.97.1 install --path crates/sightline --locked` builds
the same binary. On Windows, allow long paths before you clone; one fixture in
the fork has a path longer than 260 characters, and the checkout stops without
it: `git config --global core.longpaths true`.

The build compiles ty and rust-analyzer from source and takes a few minutes.

## Audit a repository

An audit of a Rust tree runs `cargo metadata` and `cargo check` over it, and
the index expands the tree's proc macros in a proc-macro server. Cargo runs
that tree's build scripts. Code from the audited repository therefore runs on
your machine with your privileges, the same way it does when you build the tree
or open it in an editor backed by rust-analyzer. Trust a repository before you
audit it. Both cargo passes run with `CARGO_NET_OFFLINE=true`, so neither
fetches anything. An audit of a Python tree runs no code from the tree.
[SECURITY.md](SECURITY.md) holds the detail.

Clone a repository and point `audit` at it:

```
git clone https://github.com/bgreenwell/doxx.git
sightline audit doxx
```

The first line is the provenance header, and the rest is the report, grouped by
file and then by symbol, in the order the findings rank. The file whose
strongest finding ranks first comes first:

```
sightline 0.3.0 | modules 39 | findings 92 (proved 0 / indexed 53 / heuristic 39) | suppressed 0 | baselined 0
  languages: rs

src/document/parsing/equation.rs  621 lines, fan-in 4 | 22 findings: #11 x19, #23 x3
  extract_inline_equation_positions  L31-163 (133 lines)
    31:0    heuristic #23  extract_inline_equation_positions has cognitive complexity 19 (threshold 15)
```

`--top 40` prints the forty strongest findings and counts the rest in the
header, which is the first screen to read on a large tree.

The report goes to stdout and the per-pass timings go to stderr, so a redirect
keeps them apart. A first audit of a Rust tree pays for a full `cargo check`
of it, at the memory and the time that check costs: the largest corpus tree,
salvo, takes 34 s and peaks at 3.1 GB. Later audits reuse the build directory
that [docs/reference.md](docs/reference.md) locates, and an audit removes the
build directories of roots no audit has touched in 30 days.

No finding makes `audit` exit non-zero. To read one finding's rule, pass its id
to `explain`, or browse every rule in [docs/rules.md](docs/rules.md):

```
sightline explain 23
```

## Get the report as JSON

```
sightline audit doxx --json
```

The document has two keys. `findings` is the ranked list. Each entry holds its
file, line, column, rule id, slug, message, tier, symbol span, judged
precision, and the fix when one is verified. `provenance` records what the run
saw: module count, the languages detected, parse errors, and a block per
language holding the oracle's own counters.

`--sarif` writes SARIF 2.1.0 instead, which GitHub code scanning accepts.

## Block a change

`gate` is the only verb a finding makes exit non-zero. Run it on the files a
commit touches:

```
cd doxx
sightline gate . --files src/equation.rs
```

With no `--files` it gates the working-tree diff against HEAD. `--full` runs the
whole audit pipeline for CI. A rule blocks by its posture: GATE blocks wherever
it runs, RATCHET blocks only what is new against the baseline that `sightline
baseline .` writes, and REPORT never blocks.

## Run it in CI

Write the baseline first and commit it. Without one, the first `gate --full`
run blocks on every finding already in the tree. The baseline is one line per
symbol, so a `merge=union` attribute lets git merge it without a conflict:

```
sightline baseline .
echo ".sightline-baseline merge=union" >> .gitattributes
git add .sightline-baseline .gitattributes
```

A symbol that is renamed or moved keeps its allowance: the baseline holds the
shape of each symbol's body beside its name, and a finding on a body the
baseline knows under another name is not new.

Then take one of the two jobs below, or both. The first blocks a pull request.
The second uploads the findings to code scanning, which annotates the diff and
never blocks. Each job spells the install out; `uses:
Skyway1111/sightline@v0.3.0` with `args: gate . --full` is the same install
and run as one step.

Both jobs build the checked-out tree, so a pull request from a fork runs that
fork's build scripts on your runner. That is true of any CI job that builds,
and [SECURITY.md](SECURITY.md) says what it means here.

Block a pull request:

```yaml
name: sightline
on: pull_request

jobs:
  gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7

      - name: Install sightline
        run: |
          cd "$RUNNER_TEMP"
          curl -sSfLO https://github.com/Skyway1111/sightline/releases/latest/download/sightline-lint-x86_64-unknown-linux-gnu.tar.xz
          tar -xf sightline-lint-x86_64-unknown-linux-gnu.tar.xz
          sudo install "$(find "$RUNNER_TEMP" -type f -name sightline)" /usr/local/bin/

      # The Rust oracle runs `cargo check --offline`. Without a fetch the
      # dependencies are missing, that check fails, and rules #32, #48, #56
      # and #59 go silent. Drop this step for a Python-only repository.
      - run: cargo fetch

      - run: sightline gate . --full
```

Upload the findings to code scanning:

```yaml
name: sightline scan
on:
  push:
    branches: [main]

jobs:
  scan:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      security-events: write
    steps:
      - uses: actions/checkout@v7

      - name: Install sightline
        run: |
          cd "$RUNNER_TEMP"
          curl -sSfLO https://github.com/Skyway1111/sightline/releases/latest/download/sightline-lint-x86_64-unknown-linux-gnu.tar.xz
          tar -xf sightline-lint-x86_64-unknown-linux-gnu.tar.xz
          sudo install "$(find "$RUNNER_TEMP" -type f -name sightline)" /usr/local/bin/

      - run: cargo fetch

      - run: sightline audit . --sarif > sightline.sarif

      - uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: sightline.sarif
```

## Read the fixes

`fix` prints a unified diff of the fixes it verified by re-checking the edited
world. It never writes to your tree:

```
sightline fix doxx --out fixes.diff
git -C doxx apply ../fixes.diff
```

Five rules propose a patch: #32 deletes a dead symbol, #33 writes the return
annotation the body proves, #35 hoists an import, #39 removes a comment that
restates its code, #48 folds a one-use helper. On the corpus that is about
two findings in a hundred. The rest of the report is for a reader.

## Configure

Sightline reads `[tool.sightline]` from `pyproject.toml`, and from
`sightline.toml` when there is no `pyproject.toml`. Pass `--config PATH` for a
checkout you cannot write to. `rules-off` switches a rule off for the tree,
an `overrides` table switches rules off under some paths, and
`complexity-threshold` moves #23's bar. In source, `sightline-ok: <id>` on a
line covers the line, on a `def` or a `fn` it covers the definition, and
`sightline-ok-file: <id>` covers the file. Every key and marker is in
[docs/reference.md](docs/reference.md).

## Read next

| Document | What it holds |
| --- | --- |
| [docs/rules.md](docs/rules.md) | Every rule: what it checks, the goal it approximates, its posture and its measured precision |
| [docs/reference.md](docs/reference.md) | Every verb, flag, config key, exit code, the suppression syntax, where Sightline writes, and the known limitations |
| [architecture.md](architecture.md) | How a finding is produced: the pipeline, the crate boundaries, the two oracles |
| [benchmarks.md](benchmarks.md) | Measured walls, fire rates, precision and recall, each with its reproduction command |
| [SECURITY.md](SECURITY.md) | What an audit executes, and how to report a vulnerability |
| [CONTRIBUTING.md](CONTRIBUTING.md) | What you can run outside the maintainer's machine, and what a useful false-positive report holds |

## License

MIT. See [LICENSE](LICENSE).
