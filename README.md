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
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Skyway1111/sightline/releases/latest/download/sightline-installer.sh | sh
```

Windows:

```
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Skyway1111/sightline/releases/latest/download/sightline-installer.ps1 | iex"
```

Or take the archive yourself from the
[releases page](https://github.com/Skyway1111/sightline/releases). Windows
x64, Linux x64 and macOS arm64 are built for every release; each holds the
binary, LICENSE, THIRD-PARTY.md and this file.

Check the install:

```
sightline --version
```

It prints the crate version, the pinned rev of the type-checker fork and the
`ra_ap_*` version the Rust index was compiled against:

```
sightline 0.2.0 (ty-unnecessary 284831cb43bb167d149b23f0e49bcae015c4d183, ra_ap 0.0.328)
```

Sightline is not on crates.io. `cargo install sightline` installs an unrelated
crate that holds the name, and the type-checker fork here is a git dependency,
which crates.io does not accept.

## Build from source

The workspace pins Rust 1.97.1 in `rust-toolchain.toml`. rustup reads that file
from your current directory, not from the source that cargo fetches for you, so
name the toolchain on the command line.

On Windows, allow long paths first. One file in the type-checker fork has a
path longer than 260 characters, and the git checkout stops without it:

```
git config --global core.longpaths true
```

Then:

```
rustup toolchain install 1.97.1
cargo +1.97.1 install --git https://github.com/Skyway1111/sightline sightline --locked
```

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
file and then by symbol, worst first:

```
sightline 0.2.0 | modules 39 | findings 92 (proved 0 / indexed 53 / heuristic 39) | suppressed 0 | baselined 0
  languages: rs

src/document/parsing/equation.rs  621 lines, fan-in 4 | 22 findings: #11 x19, #23 x3
  doxx::document::parsing::equation::extract_inline_equation_positions  L31-163 (133 lines)
    31:0    heuristic #23  ... has cognitive complexity 19 (threshold 15)
```

The report goes to stdout and the per-pass timings go to stderr, so a redirect
keeps them apart. A first audit of a Rust tree pays for a full `cargo check` of
it. Later audits reuse the build directory that
[docs/reference.md](docs/reference.md) locates.

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
run blocks on every finding already in the tree:

```
sightline baseline .
git add .sightline-baseline.json
```

Then take one of the two jobs below, or both. The first blocks a pull request.
The second uploads the findings to code scanning, which annotates the diff and
never blocks.

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
          curl -sSfLO https://github.com/Skyway1111/sightline/releases/latest/download/sightline-x86_64-unknown-linux-gnu.tar.xz
          tar -xf sightline-x86_64-unknown-linux-gnu.tar.xz
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
          curl -sSfLO https://github.com/Skyway1111/sightline/releases/latest/download/sightline-x86_64-unknown-linux-gnu.tar.xz
          tar -xf sightline-x86_64-unknown-linux-gnu.tar.xz
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

## Configure

Sightline reads `[tool.sightline]` from `pyproject.toml`, and from
`sightline.toml` when there is no `pyproject.toml`. Pass `--config PATH` for a
checkout you cannot write to. The keys are listed in
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
