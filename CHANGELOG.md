# Changelog

The user-visible changes per release. The release workflow reads the matching
section into the GitHub release notes.

## Unreleased

- `docs/rules.md` lists every rule with what it checks, its goal, posture and
  measured precision. `cargo xtask rules-doc` renders it from `sightline
  explain --json`, a new flag, and `cargo xtask check` fails when it drifts.
- `sightline explain` prints each measured sample's population in plain words,
  and the rule goals spell out their citations.
- `--help` copy is sentence-cased and ends with the repository link.
- Comments and docs no longer cite the rewrite's plan or the Python tool it
  replaced; the prose lint fails on such a citation.

## 0.2.0 - 2026-08-31

The first public release.

- `sightline explain` with no argument prints the whole rule roster: id, slug,
  language, family, posture, tier and the judged precision where a round
  measured one. `explain <slug>` resolves like `explain <id>`.
- `--quiet` silences the oracle pass lines on stderr, for hooks and CI.
- SARIF output names the repository on the driver and puts each rule's
  meaning, goal and measured precision in `help.markdown`, which GitHub code
  scanning renders on the alert.
- Every archive ships `THIRD-PARTY.md`, the license notices of the crates the
  binary links, and `cargo xtask check` fails when it drifts from the graph.
- A bug exits 3 with one line naming the verb and where to report it, and the
  panic stays visible above it. Fixed: the audit panic on a tree where two
  files claim one module qname, confirmed clean on the nine public
  repositories that hit it.
- `#44 tautological-assertion` leaves `assert False` unreachability markers
  and a repository's own assert helpers alone, and moves to REPORT at its
  measured precision. `#31 boundary-contracts` is cut and its id retired at a
  measured 8 of 110: import-linter reports the real eight itself. No rule now
  blocks without a baseline; `gate` blocks on what is new against the one you
  wrote.
- The Python corpus is public: `powertools-lambda-python` and `sqlglot` join
  `merged-calculator`, and every corpus row in `crates/xtask/corpus.toml`
  names the url it clones from. `benchmarks.md` is re-measured over the six.
- One-line installers for macOS, Linux and Windows.

## 0.1.0 - 2026-08-31

Released privately.
