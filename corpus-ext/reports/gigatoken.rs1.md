# gigatoken - Rust judge report (wave 1)

Repo: `../gauntlet-corpus/gigatoken` (Rust crate + PyO3 bindings; prod tree
`src/` = 60 files / ~27.4k lines, ~20.5k prod lines outside `#[cfg(test)]`).
Test code read for #42/#44/#47: `#[cfg(test)]` modules in `src/`, `benches/`.
`tests/` and `examples/` are Python here, so no Rust site lives there.

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/pretokenize/fast/cl100k.rs:61 | #11 | The 33-line `Fast*Pretokenizer` shell (struct with `bytes`/`state`, `new`/`with_pos`/`pos`, `impl Iterator::next`) is byte-identical modulo the two type names in 8 files: cl100k.rs:61, kimi.rs:40, nemotron.rs:38, o200k.rs:37, olmo3.rs:57, qwen2.rs:59, qwen3_5.rs:60, r50k.rs:463 (r50k differs only by one extra doc line). The sibling macro `impl_mask_pretoken_spans!` on the next line already proves the codebase's own fix. | `pub struct FastCl100kPretokenizer<'a> {` |
| P1-2 | src/pretokenize/fast/cl100k.rs:102 | #11 | `fn ws_token_end` (37 lines: the `\s*[\r\n]+ / \s+(?!\S) / \s+` whitespace-arm walker) is copied into 6 modules: cl100k.rs:102, olmo3.rs:98, qwen2.rs:100, qwen3_5.rs:163, o200k_family.rs:273, deepseek_v3.rs:176. Pairwise T2 similarity 0.86-0.89; the o200k copy is already const-generic and could host all of them. | `fn ws_token_end(bytes: &[u8], start: usize) -> usize {` |
| P1-3 | src/pretokenize/fast/cl100k.rs:143 | #11 | `fn advance_pos` (114 lines) is a whole-function clone at cl100k.rs:143 / olmo3.rs:139 / qwen2.rs:141 (similarity 0.965 and 0.947; qwen3_5.rs:204 at 0.687). The four schemes' SIMD masks were unified into `cl100k_family::family_algebra`, but the scalar path was left as four hand-maintained copies. | `fn advance_pos(bytes: &[u8], pos: usize) -> usize {` |
| P1-4 | src/pretokenize/fast/cl100k.rs:31 | #11 | `impl MaskScheme for Cl100kScheme` (lines 31-55) is byte-for-byte identical to `impl MaskScheme for Olmo3Scheme` (olmo3.rs:27-51), including the `true` extended flag and both `ClassTable::get()` comments; qwen2.rs:29 and qwen3_5.rs:30 differ only in one bool / one table name. The o200k family solved exactly this shape with const generics (o200k.rs:13); the cl100k family did not. | `impl MaskScheme for Cl100kScheme {` |
| P1-5 | src/bpe/mod.rs:519 | #11 | The survivor-compaction block (`let mut write = 0; let mut i = 0; while i < n { symbols[write] = symbols[i]; write += 1; i = next[i] as usize; }`) appears 6 times in one module: 519, 617, 711, 795, 885, 978. | `let mut write = 0;` |
| P1-6 | src/bpe/mod.rs:653 | #11 | `bpe_merge_symbols_short_neon` (647), `_avx512` (743) and `_avx2` (816) share a 17-line prologue (653-669) and a 34-line merge/list-surgery body (687-720) verbatim; only the min-rank scan differs. Three copies of the merge invariant, each with its own copy of the prefetch/aliasing comment block. | `debug_assert!((2..=SHORT_MERGE_MAX - 1).contains(&n));` |
| P1-7 | src/pretokenize/reference/simd.rs:412 | #11 | `fn count_pretokens` (95 lines) is a hand-maintained duplicate of `SimdPretokIter::next` (simd.rs:595) plus its helpers - the doc comment says so ("Same logic as next() but no Option/Pretoken wrapping"). Two dispatch tables for one grammar; the counting variant will drift when a class arm changes. | `/// Count pretokens without constructing them. Same logic as next() but no` |
| P1-8 | src/input/jsonl.rs:27 | #11 | `JsonLinesSlice::next` (27) and `JsonLinesReader::next` (74) are the same loop twice: skip blank lines, `sonic_rs::get_from_slice(line, &[field])`, `.as_str()?`, `Document::from(text.as_bytes().to_vec())`. The last three statements are identical; the field-extraction contract has two homes. | `let value = sonic_rs::get_from_slice(line, &[self.field]).ok()?;` |
| P1-9 | src/load_tokenizer/hf.rs:279 | #18 | `fn build_sentencepiece` (149 lines) narrates 5 labeled phases in prose banners at 293, 322, 336, 367, 375 ("Build vocab", "Extract byte-fallback token IDs", "Build merge table", "Normalizer and pre-tokenizer configuration", "Extract added tokens"). Each banner names a function that was not written. | `// --- Build vocab (preserving original HF IDs) ----------------------------` |
| P1-10 | src/pretokenize/fast/o200k_family.rs:1014 | #18 | `fn o200k_algebra` (249 lines) carries 8 phase banners at 1053, 1064, 1087, 1094, 1097, 1116, 1185, 1197 (absorbed tails / letters / digits / punct / bad zones / whitespace / Han runs / contractions). Eight named steps in one body. | `// --- Absorbed `[\r\n/]*` (Kimi `[\r\n]*`) tails -------------------------` |
| P1-11 | src/pretokenize/fast/cl100k_family.rs:379 | #18 | `fn family_algebra` (207 lines) carries 5 phase banners at 399, 417, 425, 428, 552 (letters / digits / punct / whitespace / contractions), the same prose decomposition as P1-10 in the sibling family. | `// --- Letters: `[^\r\n\p{L}\p{N}]?\p{L}+` -------------------------------` |
| P1-12 | src/pretokenize/fast/mask.rs:1189 | #18 | `fn fill_spans_two_phase_impl` (297 lines) is explicitly split in prose into "Phase A: harvest boundary positions." (1238) and "Phase B: flat emission with no data-dependent branch." (1383). The two phases share only `bufp`/`nb` and are two functions spelled as comments. | `// Phase A: harvest boundary positions.` |
| P1-13 | src/pretokenize/reference/simd.rs:595 | #18 | `SimdPretokIter::next` (53 lines) labels three phases in shouted banners at 604, 623, 630 ("FAST PATH: space + ASCII letter", "FAST PATH: ASCII letter", "REMAINING CASES: digit, other, whitespace, apostrophe, non-ASCII"). | `// ---- FAST PATH: space + ASCII letter (most common pattern) ----` |
| P1-14 | src/bindings/train.rs:52 | #18 | `fn train_bpe` (74 lines) is cut in prose into "--- FileSource: multi-file parallel processing ---" (70) and "--- Single bytes or file path ---" (86). The two arms build a `FileSourceSpec` and call `bpe_train::train_bpe` independently; they are two dispatch functions. | `// --- FileSource: multi-file parallel processing ---` |
| P1-15 | src/bpe/mod.rs:656 | #20 | The closure `let pack = \|rank: u32, i: usize\| (rank << 8) \| i as u32;` is written three times in one module (656, 752, 825), each time under its own copy of `const NO_MERGE_FLOOR: u32 = u32::MAX << 8;`. The rank/index packing format is a fact with three homes; a width change breaks two of them silently. | `let pack = \|rank: u32, i: usize\| (rank << 8) \| i as u32;` |
| P1-16 | src/pretokenize/fast/mask.rs:1189 | #23 | `fn fill_spans_two_phase_impl` is 297 lines with two const-generic dimensions (`X86_CRC`, `X86_TIER`), a `#[cfg]`-split tier `match` inside the batch loop, nested rebase/tail/overrun break conditions, and raw-pointer writes through `bufp`. Highest branch+nesting load in the crate by a wide margin. | `fn fill_spans_two_phase_impl<'a, S: MaskScheme, const X86_CRC: bool, const X86_TIER: u8>(` |
| P1-17 | src/pretokenize/fast/o200k_family.rs:1014 | #23 | `fn o200k_algebra` is 249 lines over four const-generic flags (`CONTRACTIONS`, `DIGITS3`, `SLASH`, `HAN`) with eight interleaved mask phases; every flag multiplies the paths a reader must hold at once. | `fn o200k_algebra<` |
| P1-18 | src/pretokenize/fast/o200k_family.rs:829 | #23 | `fn o200k_extended_masks` is 178 lines with the same four const-generic flags plus per-codepoint class dispatch and a deferred-zone fallback; second-densest branching in the module. | `fn o200k_extended_masks<` |
| P1-19 | src/bpe/sentencepiece.rs:1250 | #23 | `fn encode_units_impl` is 111 lines with a `RAW` const generic, a hand-drained 32-byte SIMD candidate bitmask, `every_mark` / `first_unit` / `last_mark_end` state carried across a nested block loop, and mark-vs-space branching inside it. | `fn encode_units_impl<const RAW: bool, F: FnMut(&[TokenId])>(` |
| P1-20 | src/pretokenize/fast/cl100k_family.rs:379 | #23 | `fn family_algebra` is 207 lines of mask algebra across five phases with cross-phase carry state; see P1-11 for the prose decomposition that shadows its real structure. | `fn family_algebra(` |
| P1-21 | src/bpe_train.rs:188 | #23 | `fn train_bpe` is 164 lines: it builds `contained_in_words` twice (a `HashMap<(u32,u32), BTreeSet<u32>>` and a `vec![vec![vec![]; 256]; 256]`), drives a `PriorityQueue` merge loop with tie-break branching, progress-bar side effects and stale-entry re-checks, all in one body. | `pub fn train_bpe<K: AsRef<[u8]> + Eq + Hash>(` |
| P1-22 | src/pretokenize/reference/simd.rs:412 | #23 | `fn count_pretokens` is 95 lines of nested class dispatch (space-then-letter, space-then-class `match` with an inner non-ASCII arm, then the top-level class `match` repeating it) with `continue` control flow at three depths. | `fn count_pretokens(bytes: &[u8], mut pos: usize) -> usize {` |
| P1-23 | src/bpe/tiktoken.rs:1 | #27 | 3308 lines total, 1760 prod, of which `impl Tokenizer` spans 339-1718 unbroken. `Tokenizer` is the crate's hottest symbol (re-exported at lib.rs:13, used by batch.rs, bindings, load_tokenizer, main.rs), so every task touching it ingests the largest file in the repo. | `pub struct Tokenizer {` |
| P1-24 | src/pretokenize/fast/o200k_family.rs:1 | #27 | 1495 prod lines hosting the shared o200k scalar+SIMD algebra for five schemes (o200k, kimi, nemotron, deepseek, and the qwen3_5 borrow of `ws_token_end`). Every scheme change pulls the whole file in. | `//! Shared implementation for the o200k regex family: o200k_base (GPT-4o,` |
| P1-25 | src/bpe/sentencepiece.rs:1 | #27 | 1495 prod lines carrying `SentencePieceBPE`, `EncodeState` (a public re-export at lib.rs:14), normalization, unit splitting, added-token handling and the SIMD encode kernel in one module with no internal section split. | `use crate::bpe::bpe_merge_symbols_ranked;` |
| P1-26 | src/pretokenize/fast/mask.rs:1 | #27 | 1487 lines, no test module, holding `MaskScheme`, `MaskState`, every x86/aarch64 tier wrapper and the 297-line two-phase filler. Any mask-scanner task loads all of it. | `//! Shared infrastructure for mask-scanner pretokenizers.` |
| P1-27 | src/bpe/tiktoken.rs:1 | #29 | The largest module in the crate (3308 lines) opens on a bare `use crate::bpe::pretoken_cache::ShortPretokenCache;`. No `//!` header anywhere in the file: nothing says what a `Tokenizer` is, which of the five `#[cfg(test)]` modules covers what, or how the pretoken cache relates to it. | `use crate::bpe::pretoken_cache::ShortPretokenCache;` |
| P1-28 | src/bpe/sentencepiece.rs:1 | #29 | 1618 lines, zero `//!` lines. A reader must infer the SentencePiece encode model (word units, `EncodeState`, byte fallback, U+2581 marking) from the first `const` declarations at line 10. | `use crate::bpe::bpe_merge_symbols_ranked;` |
| P1-29 | src/bpe/mod.rs:1 | #29 | 1218 lines, zero `//!` lines. The file opens on three `mod` declarations followed immediately by `madvise_hugepage`, whose own doc comment is the only thing explaining why the module exists; the BPE merge kernel family (five variants) that follows has no map. | `pub(crate) mod pretoken_cache;` |
| P1-30 | src/lib.rs:1 | #29 | The crate root: 567 prod lines wiring 8 internal modules, 4 public re-exports and the entire PyO3 surface, opening on `#![feature(portable_simd)]` with no `//!` crate doc. The one file every reader of this crate opens first says nothing about the crate. | `#![feature(portable_simd)]` |
| P1-31 | src/input/file_source.rs:1 | #29 | 515 lines with no `//!`; it substitutes an in-body banner at line 13 ("File format detection: compression and content format are independent"), which does not survive `cargo doc` and is not the first screen. | `use std::collections::HashMap;` |
| P1-32 | src/pretokenize/unicode.rs:1 | #29 | 547 lines of ICU-backed classification helpers and packed class tables with no `//!` header; a reader cannot tell from the top whether this is the reference oracle or the shipping path (it is both, depending on the caller). | `use icu::properties::props::{EnumeratedProperty, GeneralCategory, ...` |
| P1-33 | src/bpe_train.rs:1 | #29 | 351 lines, no `//!`. Trainer entry point, `Word` representation, tie-break policy and the priority-queue merge loop, with no statement of the training model or its invariants. | `use dashmap::DashMap;` |
| P1-34 | benches/unicode.rs:7 | #34 | 10-line commented-out `fibonacci` function - the untouched criterion project template - shipped in a benchmark file, plus its commented-out call site at line 36. | `// pub fn fibonacci(n: u64) -> u64 {` |
| P1-35 | benches/unicode.rs:18 | #34 | 5-line commented-out `use` plus `unicode_properties_classify` fn, kept with the note "Removed dependency since icu is ~95% faster". The measurement it records lives in the comment; git already holds the code. | `// use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};` |
| P1-36 | benches/unicode.rs:50 | #34 | 11-line commented-out `group.bench_with_input("unicode_properties", ...)` block: a disabled benchmark arm for the dependency deleted at P1-35. | `// group.bench_with_input(` |
| P1-37 | benches/unicode.rs:73 | #34 | 11-line commented-out `c.bench_function("unicode classify letter", ...)` block calling `unicode_classify`, a function that does not exist in this file, ending in the dangling `// c.bench_function("unicode pretokenize")`. | `// c.bench_function("unicode classify letter", \|b\| {` |
| P1-38 | src/pretokenize/pretokenize_traits.rs:9 | #37 | `pub(crate) trait PretokenCountable` has exactly one impl (the blanket `impl<'a, T, S> ... for T` at line 13) and exactly one call site in the crate (`pretokenize/mod.rs:683`). A crate-private extension trait wrapping a 5-line `fold` that one caller uses: a free function is the whole design. | `pub(crate) trait PretokenCountable<'a, S: BuildHasher + Default> {` |
| P1-39 | src/pretokenize/pretokenize_traits.rs:26 | #37 | `pub(crate) trait ParallelMergeCounts` has exactly one impl (blanket, line 30) and exactly one call site (`pretokenize/mod.rs:685`). Same shape as P1-38 in the same 49-line file, which exists only to hold these two single-use abstractions. | `pub(crate) trait ParallelMergeCounts<K, V, S> {` |
| P1-40 | src/pretokenize/fast/r50k.rs:1033 | #42 | `#[test] #[ignore] fn r50k_token_stats_owt` (56 lines) contains no assertion, no `panic!`, no `expect` on a computed value: it walks 10 MB of OWT, bins tokens into `counts[8]` and `eprintln!`s five percentages. A census printer wearing `#[test]`; it passes for any `advance_pos`. | `fn r50k_token_stats_owt() {` |
| P1-41 | src/pretokenize/fast/r50k.rs:1171 | #42 | `#[test] #[ignore] fn aa_r50k_advance_interleaved` (28 lines) is an A/A throughput harness: 7 timed rounds over 100 MB, `eprintln!` of MB/s and a ratio, no verdict on the ratio it exists to measure. Nothing fails if the two copies diverge by 10x. | `fn aa_r50k_advance_interleaved() {` |
| P1-42 | src/pretokenize/fast/cl100k_family.rs:718 | #42 | `#[test] #[ignore] fn family_vs_r50k_mask_compute_cost` (46 lines) times `R50kScheme` against `Cl100kScheme` over 1 GB and `eprintln!`s MB/s and cy/B. No assertion; the sibling `#[test]` at cl100k_family.rs:774 shares the shape. | `fn family_vs_r50k_mask_compute_cost() {` |
| P1-43 | src/pretokenize/fast/cl100k_family.rs:774 | #42 | `#[test] #[ignore] fn family_deferral_census` (52 lines) counts dirty batches by category and prints them; the expected rates it exists to guard (1.36% dirty, itemized) are recorded only in its doc comment, never asserted. A regression to 18% would still pass. | `fn family_deferral_census() {` |
| P1-44 | src/pretokenize/fast/mod.rs:326 | none | `neon_scan_letters` (33 lines of unsafe NEON) is `#[allow(dead_code)]` with a doc comment stating "NOT used by `scan_letters_from`: measured 0.83x ... Kept as a reference / benchmark baseline." Dead unsafe code in the prod tree that no benchmark actually calls. Matches #34's goal ("delete dead weight; git remembers old code") but the Rust #34 arm reads only commented-out blocks. | `#[allow(dead_code)]` |
| P1-45 | src/bpe/mod.rs:741 | none | `bpe_merge_symbols_short_avx512` (743) and `_avx2` (816), 73 and 81 lines of unsafe intrinsics, are `#[cfg_attr(not(test), allow(dead_code))]`: their only callers are at mod.rs:1204 and 1211 inside a `#[cfg(test)]` block. Two SIMD kernels compiled out of every shipped build (see also P1-6, where they are the clone group's other two copies). | `#[cfg_attr(not(test), allow(dead_code))]` |
| P1-46 | src/bpe_train.rs:223 | none | `println!("{} unique words", words.len());` on stdout from inside library `train_bpe`, which is reachable from the PyO3 binding (`bindings/train.rs:52`). A library writing to a caller's stdout, with no verbosity switch. | `println!("{} unique words", words.len());` |
| P1-47 | src/pretokenize/mod.rs:673 | none | `eprintln!("Using {n_threads} threads for pretokenization")` and `eprintln!("Pretokenization took {time_elapsed:?}")` (line 688) unconditionally in the library function `pretokenize_par_bytes`, plus an `Instant::now()` kept solely to print. Same problem as P1-46 on the pretokenize path. | `eprintln!("Using {n_threads} threads for pretokenization");` |
| P1-48 | benches/unicode.rs:3 | none | `use std::hint::black_box;` is imported and never used - the only call site was commented out at P1-34. A warning shipped in the bench target. | `use std::hint::black_box;` |

### Rules with no site found

- **#36** (`#[allow]` density): the densest module carries exactly one
  `#[allow]`, and every one is narrowly scoped and justified
  (`clippy::too_many_arguments` x3, `type_complexity` x2, one
  crate-level `large_enum_variant` in hf.rs, `unsafe_op_in_unsafe_fn` in
  avx512.rs). Nothing to report.
- **#38** (module-level literal duplication): no `const`/`static &str`
  declared at module level repeats anywhere in the crate. The heavily
  repeated literals (`"data/owt_train.txt"` in 11 modules, the ~50-case
  pretokenizer corpus in 6) all live inside `#[cfg(test)]` bodies as
  inline expressions, not module-level declarations.
- **#44** (tautological assertion): none. Every assertion compares a
  computed value against a scalar reference or a fixture.
- **#47** (sleepy test): no `sleep` call anywhere in the Rust tree.


## Phase 2 - audit finding verdicts

262 Rust rows in `corpus-ext/sheets/gigatoken.rs1.wave1.tsv`, judged at their
sites in the repo; every row carries its own `verdict` and `why` there. Filled
by `fill-gigatoken.py` in the shared scratch dir. Totals:

| rule | rows | real | fp |
|------|------|------|----|
| #11 structural-clones | 150 | 142 | 8 |
| #18 section-comments | 5 | 5 | 0 |
| #20 repeated-lambda | 11 | 4 | 7 |
| #23 cognitive-complexity | 70 | 57 | 13 |
| #27 purchase-price | 8 | 6 | 2 |
| #29 top-loading | 8 | 8 | 0 |
| #34 noop-code | 3 | 3 | 0 |
| #37 speculative-generality | 2 | 2 | 0 |
| #42 assertion-free-test | 5 | 3 | 2 |
| **total** | **262** | **230** | **32** |

#11 carries the audit: the `fast/` pretokenizer family, the five `bpe/mod.rs`
merge kernels, the `reference/` NEON helpers and the three unicode class-table
builders are all genuine multi-home facts, and 142 of 150 rows land on them.
#29, #34, #37 and #18 fired clean. The fp mass is concentrated in #20 (7 of 11)
and #23 (13 of 70).

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| #11 | shape-only pair: same 4-to-9-line skeleton over unrelated types or disjoint constants, nothing to unify | 6 | `src/bindings/bridge.rs:24:11:structural-clones:gigatoken::bindings::bridge::EncodeInput::as_bytes` |
| #11 | self-overlap: a nested `fn` item's statements also belong to its enclosing fn, so the block matches itself | 2 | `src/pretokenize/fast/o200k_family.rs:852:11:structural-clones:gigatoken::pretokenize::fast::o200k_family::o200k_extended_masks` |
| #20 | trivial projection closure (field access, method reference, one-element literal) | 3 | `src/batch.rs:132:20:repeated-lambda:gigatoken::batch::encode_chunk` |
| #20 | adapter onto a home that is already named (`parquet_err`, `encode_regions_ragged`) | 3 | `src/lib.rs:143:20:repeated-lambda:gigatoken::BPETokenizer::encode_batch` |
| #20 | one-call thunk deferring a lazy argument; no separable predicate and drift is a compile error | 1 | `src/batch.rs:712:20:repeated-lambda:gigatoken::batch::WorkerPool::with_worker` |
| #23 | single-concept byte scanner under ~40 lines: score is `loop`/`while` pairing plus short-circuit terms, not branching a reader holds | 9 | `src/pretokenize/fast/mod.rs:426:23:cognitive-complexity:gigatoken::pretokenize::fast::scan_other_from` |
| #23 | closure-argument nesting in a builder/registration DSL charged as control-flow nesting | 2 | `benches/pretokenize.rs:30:23:cognitive-complexity:gigatoken::benches::pretokenize::pretokenize_benches` |
| #23 | guard-clause early returns and `continue` guards in an otherwise linear function | 2 | `src/bpe/sentencepiece.rs:1126:23:cognitive-complexity:gigatoken::bpe::sentencepiece::SentencePieceBPE::encode_section_cb` |
| #27 | size threshold reaches a cohesive leaf module under ~600 prod lines whose hot-symbol count measures fan-in, not reading cost | 2 | `src/pretokenize/unicode.rs:1:27:price:gigatoken::pretokenize::unicode` |
| #42 | helper-verdict walk stops one hop short: `test -> check_streaming_all -> check_streaming`, which asserts | 2 | `src/pretokenize/fast/cl100k_family.rs:831:42:assertion-free:gigatoken::pretokenize::fast::cl100k_family::owt_tests::family_mask_matches_scalar_owt_full` |

Two notes that are not fp but are worth a rule author's time. #11 reports every
member of a group rather than exempting first copies, so one 9-copy shape
(`Fast*Pretokenizer::next`) costs nine rows; and at eight anchors it reports two
nested groups at the same line (a longer block shared by fewer definitions, plus
a shorter one shared by more, e.g. `src/bpe/mod.rs:653`), which one change
resolves together. Also, `src/lib.rs:1` is described as "4 top-level items" when
the module defines two `#[pyclass]` types, their two `#[pymethods]` blocks and
the `#[pymodule]` entry point; the finding (no `//!`) is still true.

## Phase 3 - reconciliation

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #11 | covered | 9 rows, anchored on `Fast*Pretokenizer::next` (cl100k.rs:89) rather than the struct at :61; the group names the 3-line `next` only, not the 33-line struct+ctor shell it belongs to |
| P1-2 | #11 | covered | reached via the clone-block row at cl100k.rs:103, but the 6-copy `ws_token_end` family is split into three groups (olmo3+qwen2, o200k_family+qwen3_5, and a 3-way block) - no finding names the whole family |
| P1-3 | #11 | covered | clone row at cl100k.rs:143 exactly; group is x2 (cl100k, olmo3), so qwen2.rs:141 at 0.947 similarity is outside it |
| P1-4 | #11 | covered | two rows inside the impl block (cl100k.rs:39 `batch_masks` x4, :48 `batch_masks_x86` x4) |
| P1-5 | #11 | threshold-miss | the 6-copy survivor-compaction tail (519, 617, 711, 795, 885, 978) is 3 top-level statements (`let`, `let`, `while`), under the >=5-statement block cutoff; the rule found the neighbouring min-rank scan (487, 569, 947) and list init (474, 556) instead |
| P1-6 | #11 | covered | exact: rows at bpe/mod.rs:653 (prologue) and :688/:772/:862 (the 10-statement surgery body) across all three SIMD variants |
| P1-7 | #11 | detector-miss | no group pairs `simd.rs:412 count_pretokens` with `SimdPretokIter::next` (:595) even though the doc comment says "Same logic as next()"; the counting variant drops the `Option`/`Pretoken` wrapping, so the digests diverge |
| P1-8 | #11 | detector-miss | no finding anywhere in `src/input/jsonl.rs`; `JsonLinesSlice::next` and `JsonLinesReader::next` share an identical 3-statement tail and the same blank-line/field-extract loop |
| P1-9 | #18 | covered | hf.rs:293, all 5 banners counted |
| P1-10 | #18 | covered | o200k_family.rs:1053, all 8 banners counted |
| P1-11 | #18 | covered | cl100k_family.rs:399, all 5 banners counted |
| P1-12 | #18 | detector-miss | `mask.rs:1189` labels exactly two phases as prose sentences (`// Phase A: harvest boundary positions.`, `// Phase B: flat emission ...`) with no `---` banner rule; the pattern appears to require the banner, so the crate's clearest two-phase function is the one it misses |
| P1-13 | #18 | covered | simd.rs:604 |
| P1-14 | #18 | covered | bindings/train.rs:70 |
| P1-15 | #20 | covered | bpe/mod.rs:656 exactly, x3 |
| P1-16 | #23 | covered | mask.rs:1189, scored 93 - the highest in the crate, matching my read |
| P1-17 | #23 | covered | o200k_family.rs:1014, scored 56 |
| P1-18 | #23 | covered | o200k_family.rs:829, scored 40 |
| P1-19 | #23 | covered | sentencepiece.rs:1250, scored 46 |
| P1-20 | #23 | covered | cl100k_family.rs:379, scored 32 |
| P1-21 | #23 | covered | bpe_train.rs:188, scored 50 |
| P1-22 | #23 | covered | simd.rs:412, scored 35 |
| P1-23 | #27 | detector-miss | `bpe/tiktoken.rs` (3308 lines, 1760 prod, the largest module in the crate and the home of the re-exported `Tokenizer`) draws no #27 row while eight smaller modules do; its symbols are almost all methods inside one 1380-line `impl Tokenizer`, so the hot-symbol index appears not to reach them. #29 fired on the same file, so the size fact was available |
| P1-24 | #27 | covered | o200k_family.rs:1 |
| P1-25 | #27 | covered | sentencepiece.rs:1 |
| P1-26 | #27 | covered | mask.rs:1 |
| P1-27 | #29 | covered | tiktoken.rs:1 |
| P1-28 | #29 | covered | sentencepiece.rs:1 |
| P1-29 | #29 | covered | bpe/mod.rs:1 |
| P1-30 | #29 | covered | lib.rs:1 |
| P1-31 | #29 | covered | input/file_source.rs:1 |
| P1-32 | #29 | covered | pretokenize/unicode.rs:1 |
| P1-33 | #29 | covered | bpe_train.rs:1 |
| P1-34 | #34 | covered | benches/unicode.rs:7, 10 lines |
| P1-35 | #34 | threshold-miss | the 5-line run at benches/unicode.rs:18 opens with a prose line ("Removed dependency since icu is ~95% faster"), leaving a 4-line `use` + fn run; either the prose line breaks the run or a `use` item plus an `fn` item does not parse as one unit. The rule caught the three runs on either side of it |
| P1-36 | #34 | covered | benches/unicode.rs:50, 11 lines |
| P1-37 | #34 | covered | benches/unicode.rs:73, 12 lines |
| P1-38 | #37 | covered | pretokenize_traits.rs:9 |
| P1-39 | #37 | covered | pretokenize_traits.rs:26 |
| P1-40 | #42 | detector-miss | `r50k.rs:1033 r50k_token_stats_owt` bins 10 MB of tokens and eprintln!s five percentages with no verdict at all, yet draws only a #23 row; its module sibling `aa_r50k_advance_interleaved` (:1171) has the same verdict-free measurement shape and did fire |
| P1-41 | #42 | covered | r50k.rs:1171 |
| P1-42 | #42 | detector-miss | `cl100k_family.rs:718 family_vs_r50k_mask_compute_cost` times two schemes over 1 GB and prints MB/s and cy/B with no assertion; the rule saw the function (it drew no #23 row only because it scored under threshold) but not the missing oracle |
| P1-43 | #42 | detector-miss | `cl100k_family.rs:774 family_deferral_census` counts dirty batches by category and prints them; its expected rates (1.36% dirty, itemized) live only in the doc comment. It drew a #23 row, so the body was parsed - the verdict check did not fire |
| P1-44 | none | inventory-gap | `neon_scan_letters` (fast/mod.rs:326), 33 lines of `#[allow(dead_code)]` unsafe NEON documented as "NOT used ... kept as a reference / benchmark baseline". Matches #34's goal, outside the Rust arm's commented-out-block reading |
| P1-45 | none | inventory-gap | `bpe_merge_symbols_short_avx512` / `_avx2` (bpe/mod.rs:743, :816) are `#[cfg_attr(not(test), allow(dead_code))]` with test-only callers: 154 lines of unsafe intrinsics compiled out of every shipped build. No Rust rule reads dead prod code |
| P1-46 | none | inventory-gap | `println!` on stdout from library `train_bpe` (bpe_train.rs:223), reachable from the PyO3 binding |
| P1-47 | none | inventory-gap | unconditional `eprintln!` x2 plus a timing `Instant` kept only to print, in library `pretokenize_par_bytes` (pretokenize/mod.rs:673, :688) |
| P1-48 | none | inventory-gap | unused `use std::hint::black_box;` at benches/unicode.rs:3, left behind by the commented-out block at P1-34 |

**Counts:** 34 covered, 7 detector-miss, 2 threshold-miss, 5 inventory-gap (48 sites).
