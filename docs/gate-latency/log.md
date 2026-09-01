# Hillclimb: single-file gate latency (2026-08-31)

Ruler: `bench.ps1`, the median of 15 warm runs of `sightline gate ROOT --files
FILE [--config]`, four frozen cases, goldens (stdout+exit) as the equivalence
pin. Machine: the benchmarks.md reference machine. Spawn floor ~7 ms
(`--version` equals `cmd /c ver`).

Cases: lol-py = lol-predictor model.py (329 L, typical); mc-py =
merged-calculator damage.py (18.6 kL, stress); tur-rs = turmoil-fs lib.rs
(2.4 kL); sal-rs = salvo components.rs (509 L, 134-manifest workspace).

| # | hypothesis (mechanism) | lol-py | mc-py | tur-rs | sal-rs | verdict |
|---|---|---:|---:|---:|---:|---|
| 0 | baseline | 23.5 | 69.7 | 36.2 | 59.0 | . |
| 1 | hook: call built xtask.exe, not `cargo run` (~430 ms dispatch per edit); pure-shell JSON parse (msys ~30 ms per spawn) | . | . | . | . | KEPT (hook e2e ~600 to ~120 ms) |
| 2 | gate detects only languages the diff spells (a skipped language gates nothing; the detect fallback provably gates nothing) | 21.2 | 66.7 | 35.3 | 48.8 | KEPT |
| 3 | `any_name`: short-circuit unsorted detect walk replaces collect-and-sort `walk_names` | 20.9 | 66.6 | 34.2 | 40.0 | KEPT |
| 4 | parse each Cargo.toml once (`manifests()` shared by `crate_roots` and `lib_crates`) | 20.9 | 66.2 | 33.8 | 35.1 | KEPT |
| 5 | discover walks subdirectories under rayon, merged in child order (a Windows directory open costs ~30 us) | 19.7 | 66.1 | 32.9 | 28.4 | KEPT |
| 6 | #7: literal prefilter plus ASCII `(?-u:\b)` (a Unicode `\b` beside non-ASCII bytes falls off the DFA); #18: match under the cache lock (a cloned Regex starts with a cold scratch cache) | 20.0 | 55.7 | 33.2 | 27.0 | KEPT (#7 10.4 to 2.6, #18 8.3 to 0.6) |
| 7 | manifest reads under rayon | 19.0 | 56.9 | 31.5 | 22.7 | KEPT |
| 8 | ASCII `\b` in EXEMPT, NO_RAISE and DIRECTIVE regexes | 20.5 | 58.5 | 32.2 | 22.6 | REVERTED (those run on audit-path rules; the gate metric stayed flat) |
| 9 | mimalloc global allocator (the Windows heap serializes rayon's small allocations) | 19.5 | 50.8 | 31.7 | 19.6 | KEPT |
| 10 | PROTOCOL_RE's ~2.5 ms compile forced on a rayon worker during the facts build | 17.3 | 49.7 | 31.5 | 20.4 | KEPT |
| 11 | rs fn bodies par-filled in `warm` (per-symbol OnceLocks; #20 filled them one at a time) | 17.3 | 50.1 | 31.0 | 20.2 | KEPT (#20 3.1 to under 0.5) |
| 12 | rs qname map: home prefixes formatted once, not per file per crate (salvo qname 3.0 to 0.3) | 18.3 | 48.9 | 32.4 | 20.1 | KEPT |
| 13 | py package-ness read from the listing, not a stat per directory | 17.7 | 49.9 | 30.7 | 19.1 | KEPT |
| . | **final (probes stripped)** | **17.2** | **49.8** | **31.4** | **19.7** | goldens identical to baseline byte for byte, all cases |

Deltas: lol-py -27%, mc-py -29%, tur-rs -13%, sal-rs -67%. Hook e2e ~5x.

Stop predicate (15/40/20/15 ms, at least 8 iterations): iterations met,
targets not. The remainder is the ~7 ms Windows spawn floor plus single-file
parse and index (tree-sitter ~5.5 ms on 2.4 kL, ruff ~7 ms on 18.6 kL) and
single-module passes with no intra-module parallelism.

Best untried ideas, in value order:

- Intra-module parallelism in the rs index passes (items, scope and walk run
  in sequence for a one-file gate: ~8 ms on tur-rs).
- The rules wall still tracks the sum of rule walls under rayon (~13 ms on
  mc-py, largest rule 3 ms): find the remaining serializer.
- A resident gate (watch mode) drops spawn, config and walk to ~0.
- The reverted ASCII `\b` sweep, retried against the full-audit walls where
  those regexes actually run.
