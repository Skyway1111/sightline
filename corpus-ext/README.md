# Measurement evidence

The judged samples behind every precision and recall number in
`data/precision.toml` and `benchmarks.md`. Nothing here is code, and nothing
here runs in an audit. The 34 MB is the cost of the numbers being checkable.

## What the files are

| Path | What it holds | Who reads it |
| --- | --- | --- |
| `manifest*.json` | One gauntlet round: the public repositories drawn, their pinned commits, the seed and the search queries. `manifest.json` through `manifest4.json` are the Python rounds, `manifest-rs*.json` the Rust ones | `cargo xtask gauntlet clone` re-clones a round at its pins |
| `pool*.json`, `sourcing_log*.json` | The candidate pool each round drew from, and why each candidate was taken or rejected | A reader checking the draw |
| `configs/` | The `[tool.sightline]` table each gauntlet repository was audited with | `cargo xtask gauntlet clone` |
| `audits/` | The `sightline audit --json` output the judges' sheets were drawn from, per repository and round | `cargo xtask gauntlet sheet`, which turns one into a judging sheet |
| `sheets/` | One row per finding, a hand verdict per row. `cargo xtask gauntlet tally` folds them into the per-rule and per-arm rates in `data/precision.toml` | `cargo xtask gauntlet tally` |
| `reports/` | The blind judges' write-ups of each repository, which the recall rows are matched against | A reader checking a recall row |
| `BRIEF.md` | The prompt each blind judge was given, verbatim. It names the Python tool the rewrite replaced; the protocol is unchanged | A reader checking the protocol |
| `decisions.tsv` | The append-only trail of every cut, restriction and retirement, with its evidence. `cargo xtask retired` extracts the retirement rows into `data/retired.toml`, which `sightline explain <retired id>` prints | `cargo xtask retired` |

## How to read a number

`sightline explain 1` prints `precision: 66/80 seed 202608284 - judged on 5
held-out Python repositories, round 4`. Round 4 is `manifest4.json`; the seed
is the one `cargo xtask precision-sample` drew the 80 rows with; the verdicts
are the `wave` sheets of those five repositories under `sheets/`.

Paths inside the committed reports and configs spell the clone root as
`<GAUNTLET_CORPUS_ROOT>`. Resolve it against your own `../gauntlet-corpus/`
clone; `cargo xtask gauntlet clone` puts the trees there.

The prose lint does not scan this directory: the sheets and reports are the
judges' words and the trail is history.
