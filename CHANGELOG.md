# Changelog

The user-visible changes per release. The release workflow reads the matching
section into the GitHub release notes.

## 0.3.0 - 2026-09-01

- The crate is `sightline-lint` on crates.io, and the `ty-unnecessary` fork it
  builds on is published beside it as `sightline-ruff-*` and `sightline-ty-*`.
  Release archives and installers are named `sightline-lint-*`.
- The report ranks on a lower bound of each rule's judged precision, the
  posterior mean less one standard deviation, so a rule judged on five sites
  ranks under one judged on two hundred at the same fraction. `explain`,
  `docs/rules.md`, the JSON and the SARIF print the 95% interval beside every
  fraction.
- The text report groups files and symbols in rank order instead of by
  finding count, drops the module's own prefix from every message, and names
  the line of every clone copy. `audit --top N` prints the N strongest.
- `#1 weak-boundary-types` reports one finding per signature, listing its
  weak slots, instead of one per parameter.
- Family letters are gone: a rule belongs to trust, surface, context, perf,
  tests or checker, and `explain` prints its posture and scope in words. The
  roster has a scope column and a legend.
- The baseline is `.sightline-baseline`, one line per key, safe under
  `merge=union`, and holds the shape of each symbol's body beside its name:
  a renamed or moved symbol keeps its allowance, so the fast gate and `--full`
  agree after a rename. The 0.2 JSON file is read and replaced.
- The fast gate's header names the repo-scope rules it did not run.
- `sightline-ok` on a `def`, `class` or `fn` line covers the definition;
  `sightline-ok-file` covers the file; `[[tool.sightline.overrides]]` switches
  rules off under paths; `complexity-threshold` moves #23's bar.
- The Python environment is found through `VIRTUAL_ENV`, `CONDA_PREFIX`,
  `UV_PROJECT_ENVIRONMENT`, `.venv`, `venv`, `env` and poetry's cache, and the
  header names the one the checker read.
- `[tool.poetry]` counts as packaging metadata, so a poetry library is
  published and #60, #56 and the closed world read it as one.
- `#33 return-honesty` reads production code only; `#37
  speculative-generality` counts no test double as the one
  implementation and leaves a published abstraction alone.
- An audit of a Rust tree removes the build directories no audit touched in
  30 days, and a check that fails for want of fetched dependencies says to run
  `cargo fetch`. The cargo version pin and its header note are gone.
- A `.py` file beside a `Cargo.toml` with no Python manifest is a stray
  script the header names, not a second tree to audit.
- A pre-commit hook definition, a GitHub composite action, and a
  checksum-verified install without a script.
- `docs/rules.md` lists every rule with what it checks, its goal, posture and
  measured precision. `cargo xtask rules-doc` renders it from `sightline
  explain --json`, a new flag, and `cargo xtask check` fails when it drifts.
- `sightline explain` prints each measured sample's population in plain words,
  and the rule goals spell out their citations.
- `--help` copy is sentence-cased and ends with the repository link.
- Comments and docs cite neither the rewrite's plan or the Python tool it
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
