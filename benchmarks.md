# Benchmarks

The canonical performance and quality numbers. Each entry is a measured value
with the command that reproduces it. History lives in git and in
`docs/review/decisions.tsv`. Per-rule precision and recall are encoded in
`data/precision.toml`; this file quotes the pooled reads and the method.

Four tables are generated. They sit between `<!-- generated: NAME -->` markers
and `cargo xtask bench-tables` rewrites them from `corpus/results/`. Never edit
inside the markers.

## Reference machine

- Windows 11, 32 GB RAM, AMD Ryzen 7 7800X3D
- Rust 1.97.1, the pin in `rust-toolchain.toml`
- Both oracles in process: ty from the fork rev in `Cargo.toml`, the Rust index
  from `ra_ap_* 0.0.328`; the corpus environment's cargo is the
  `sightline.toml` pin, 1.98.0
- Corpus pinned by commit (`crates/xtask/corpus.toml`, with the url each row
  clones from): powertools-lambda-python `a39e1018`, sqlglot `dcc36544`,
  merged-calculator `9eefca4b`, doxx `062819a1`, turmoil `684acc1a`, salvo
  `ebfefdcc`

## Release profile

`[profile.release]` sets `lto = "thin"`. Medians of 5 interleaved runs of
`sightline audit ROOT --json`, the two binaries alternating run by run so both
see the same machine load. 2026-08-30.

| Measure | `lto` off | `lto = "thin"` |
| --- | ---: | ---: |
| doxx audit | 15,377 ms | 15,259 ms |
| binary size | 94,516,082 B | 80,618,343 B |
| clean `cargo build --release` | 143 s | 157 s |

Thin LTO is faster on the audit and cuts 13.9 MB, for 14 s of clean build. The
audits produce identical bytes under both settings.

## Corpus audit walls

Full `sightline audit --json`, oracle on, cold process, alone on the machine.
`cargo xtask audit-bench` takes the median of 3 and records the peak working
set of the process tree. A budget is a ceiling a wall may not cross; a move
takes a receipt in the commit that moves it.

| repo | budget | measured | peak RSS |
| --- | ---: | ---: | ---: |
| powertools-lambda-python | 5 s | 4.2 s | 728 MB |
| sqlglot | 5 s | 4.4 s | 1,243 MB |
| merged-calculator | 19 s | 18.7 s | 1,940 MB |
| doxx | 16 s | 15.0 s | 2,032 MB |
| turmoil | 10 s | 9.2 s | 1,443 MB |
| salvo | 35 s | 18.2 s | 3,150 MB |

Wave-2 sweep, 2026-08-31, the binary at the corpus swap's close. The retired
Python tool's final walls, its budgets and the RSS comparison it was held to
live in git at the phase-9 close.

Two consecutive audits are identical byte for byte on every corpus repository,
at one thread and at every core (criterion 3).

## Per-pass profile (merged-calculator)

`sightline audit ../merged-calculator --config corpus/merged-calculator.toml
--profile corpus/results/profile-merged-calculator.json`, the facts-to-rules
span. An oracle pass is nested in the rule that first asks for it, so the rows
do not partition the wall. Passes at 1% or more are listed.

<!-- generated: profile-merged-calculator -->
| Pass | Wall | Share |
| --- | --- | --- |
| rule #10 over-constrained-param | 5.16 s | 35% |
| oracle pass 809 (counterfactual worlds) | 3.20 s | 22% |
| rule #11 structural-clones | 1.72 s | 12% |
| oracle pass 1 (diagnostics+edges) | 1.31 s | 9% |
| rule #5 proof-lifting | 1.02 s | 7% |
| oracle pass 807 (counterfactual worlds) | 0.97 s | 7% |
| oracle pass 808 (counterfactual worlds) | 0.94 s | 6% |
| oracle pass 405 (types) | 0.37 s | 3% |
| oracle pass 503 (types) | 0.23 s | 2% |
| oracle pass 121 (types) | 0.18 s | 1% |
| oracle pass 792 (counterfactual worlds) | 0.17 s | 1% |
| **total (facts→rules)** | **14.6 s** | oracle passes 12.1 s (83%) |
<!-- /generated: profile-merged-calculator -->

## Gate latency

`cargo xtask gate-bench` on a ten-file diff, median of 5. The budget is
criterion 5: 50 ms or less, 100% subset of the full audit, and zero findings
outside the diff.

| diff | budget | measured |
| --- | ---: | ---: |
| merged-calculator, ten `.py` files | 50 ms | 33 ms |
| turmoil, ten `.rs` files | 50 ms | 42 ms |

2026-08-31 run: subset 100% and zero findings outside the diff on both.
The fast gate's walk reads each directory entry's own file type instead of
a stat per entry, the per-file stacks build under rayon, and
`BuildMode::File` builds no checker and no git history. The gate detects
only the languages the diff spells, `discover` descends directories under
rayon, and each `Cargo.toml` parses once per build.

The hook case is one file per edit. Medians of 15 warm runs of `sightline
gate ROOT --files FILE`, 2026-08-31, process spawn floor ~7.5 ms included:
powertools-lambda-python 364-line `base.py` 20 ms, salvo 509-line `.rs`
19 ms, turmoil 2,451-line `.rs` 30 ms, merged-calculator 18,620-line
`damage.py` 48 ms.
This repository's PostToolUse hook (`banned` plus the gate, msys shell
spawns included) lands near 120 ms per edit. The harness that measures
those four, and the attempt log of the climb that produced them, are
[docs/gate-latency/](docs/gate-latency/).

## Fire rates

Findings per thousand lines per rule, over the three Python corpus
repositories.

<!-- generated: fire-rates -->
| Rule | P | S | M | Rule | P | S | M | Rule | P | S | M |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 651 | 172 | 159 | 23 | 19 | 220 | 284 | 42 | 7 | 2 | 4 |
| 2 | 5 | 5 | 46 | 24 | 2 | 6 | 28 | 44 | 0 | 3 | 3 |
| 3 | 0 | 0 | 1 | 26 | 1 | 39 | 124 | 47 | 2 | 0 | 1 |
| 5 | 10 | 14 | 37 | 27 | 19 | 27 | 82 | 48 | 1 | 3 | 5 |
| 6 | 3 | 0 | 0 | 29 | 68 | 80 | 0 | 49 | 0 | 1 | 0 |
| 7 | 2 | 1 | 1 | 32 | 83 | 3 | 28 | 50 | 531 | 73 | 190 |
| 9 | 0 | 0 | 2 | 33 | 19 | 1 | 24 | 53 | 8 | 0 | 1 |
| 10 | 29 | 5 | 241 | 35 | 11 | 5 | 11 | 54 | 1 | 2 | 2 |
| 11 | 199 | 241 | 645 | 36 | 5 | 1 | 9 | 55 | 54 | 54 | 102 |
| 12 | 18 | 0 | 10 | 37 | 2 | 7 | 19 | 56 | 2 | 0 | 1 |
| 14 | 12 | 5 | 18 | 38 | 13 | 0 | 5 | 57 | 0 | 0 | 11 |
| 18 | 0 | 1 | 6 | 39 | 106 | 2 | 4 | 58 | 0 | 7 | 13 |
| 20 | 0 | 8 | 10 | 40 | 0 | 2 | 0 | 59 | 0 | 3 | 0 |
| 21 | 3 | 3 | 1 | 41 | 0 | 0 | 5 | 60 | 11 | 1 | 4 |

Totals: P 1897, S 997, M 2137 findings (proved 5 / 5 / 46; indexed 457 / 369 / 1222).
Arms (P/S/M): #1 `lying-default` 21/123/0, `weak` 630/49/159; #11 `clone` 177/145/221, `clone-block` 21/91/422, `expr-clone` 1/5/2; #26 `computed-declaration` 1/16/124, `dynamic-all` 0/1/0, `star-import` 0/22/0; #27 `fan-out` 2/4/6, `price` 17/23/76; #32 `dead-import` 1/1/1, `dead-param` 10/0/13, `dead-symbol` 72/2/14; #33 `lying-return` 17/1/0, `mixed-returns` 1/0/0, `sentinel` 0/0/1, `undeclared-optional` 1/0/23; #35 `entangled` 1/2/6, `hoistable-import` 5/2/5, `import-cycle` 5/1/0; #36 `any-laundering` 4/1/9, `type-lies` 1/0/0; #37 `monomorphic` 0/0/1, `single-impl` 1/1/0, `unused-default` 1/6/18; #39 `comment-restates` 68/2/4, `docstring-restates` 36/0/0, `dunder-restates` 2/0/0; #58 `invalid-method-override` 0/7/0, `invalid-return-type` 0/0/13.
<!-- /generated: fire-rates -->

## Rust corpus

Walls, findings and oracle counters for the three Rust corpus repositories.

<!-- generated: rs-corpus -->
| repo | role | findings | wall | documents in / out | edges |
| --- | --- | ---: | ---: | ---: | ---: |
| doxx | clean | 92 | 15.2 s | 39 / 9571 | 1,845 |
| turmoil | mid | 150 | 8.8 s | 98 / 6423 | 5,797 |
| salvo | scale | 699 | 34.2 s | 274 / 11344 | 26,779 |
<!-- /generated: rs-corpus -->

Fire rates for the Rust rules over the same three:

<!-- generated: fire-rates-rs -->
| Rule | D | T | S | Rule | D | T | S | Rule | D | T | S |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 9 | 0 | 0 | 1 | 27 | 2 | 7 | 24 | 42 | 4 | 2 | 0 |
| 11 | 34 | 103 | 428 | 29 | 9 | 9 | 108 | 47 | 0 | 0 | 22 |
| 18 | 0 | 1 | 2 | 32 | 10 | 0 | 0 | 48 | 0 | 0 | 4 |
| 20 | 0 | 4 | 13 | 34 | 0 | 0 | 17 | 53 | 0 | 2 | 0 |
| 21 | 0 | 6 | 3 | 37 | 0 | 0 | 1 | 56 | 3 | 0 | 0 |
| 23 | 30 | 16 | 71 | 39 | 0 | 0 | 5 |  |  |  |  |

Totals: D 92, T 150, S 699 findings (proved 0 / 0 / 0; indexed 53 / 112 / 463).
Arms (D/T/S): #11 `clone` 4/103/360, `clone-block` 30/0/68; #34 `commented-code` 0/0/16, `noop-match` 0/0/1.
<!-- /generated: fire-rates-rs -->

## Verified fix coverage

`cargo xtask fix-check`, which `check --slow` runs on
powertools-lambda-python and doxx and `fix-check corpus/results <name>`
points at any corpus row. It checks three things per repository: every
emitted patch applies with `git apply`, a re-audit reports no finding the
patch's own `# sightline-fix:` headers name, and the target's own suite
passes. The Rust half patches the live tree, runs `cargo check` and `cargo
test` over the crate targets the pre-patch check compiled, and reverts.

Pending the close run.

## Precision

Sampled per tier against a seed pinned before judging (`cargo xtask
precision-sample`). The bars are 95% for proved, 80% for indexed and 70% for
heuristic. Per-rule and per-arm judged samples live in `data/precision.toml`,
which `sightline explain <id>` prints.

## Recall

The judges' blind lists against fresh audits of the pinned gauntlet clones,
tallied into `data/precision.toml` and quoted here. Recall is re-measured at
every campaign close, beside the precision round: a cut is priced on both sides
or it does not happen.

## Own repository

`sightline audit .` under `gate --full` reports zero blocking findings at every
unit close (criterion 12). `cargo xtask surface` counts non-test lines under
`crates/` against the cap criterion 11 sets.
