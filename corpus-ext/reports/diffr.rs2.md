# diffr (rs2) judge report

Repo: `../gauntlet-corpus/diffr` at `2152742`. Bin crate, no lib: `src/main.rs`
(880) + `src/cli_args.rs` (396) + `src/diffr_lib/mod.rs` (693) +
`src/diffr_lib/best_projection.rs` (221) are prod; `src/tests_app.rs` (125),
`src/tests_cli.rs` (277), `src/diffr_lib/tests_lib.rs` (743) are test code.
Read cold, no audit output seen.

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/main.rs:1 | #29 | 880-line crate root opens on a `use`; no `//!` header says what the binary is or how its five concerns (config, hunk buffer, escape scanner, hunk-header parser, output) relate. `diffr_lib/mod.rs` has such a header, so the omission is local. | `use std::fmt::{Debug, Display, Error as FmtErr, Formatter};` |
| P1-2 | src/main.rs:32 | #27 | One 880-line module carries the config record, the paint/process engine, the ANSI escape scanner, the `@@` header parser and the output helper. Every task touching any hot symbol (`AppConfig`, `output`, `HunkBuffer`) ingests all five. | `pub struct AppConfig {` |
| P1-3 | src/main.rs:117 | #32 | `#[derive(Default)]` on `ExecStats` is reached by nothing: the only construction is `ExecStats::new`, which hand-lists the same four zeros. Either the derive is dead or `new` should be `..Default::default()`. | `#[derive(Default)]`<br>`struct ExecStats {` |
| P1-4 | src/main.rs:148 | #48 | `report` is a private one-line body with exactly one prod call edge (main.rs:576) and no other reference; it buys a name and a hop over `self.report_into(&mut std::io::stderr())`. | `fn report(&self) -> std::io::Result<()> {`<br>`    self.report_into(&mut std::io::stderr())` |
| P1-5 | src/main.rs:152 | #37 | `report_into<W: Write>` has exactly one instantiation in the whole crate, `std::io::Stderr` (line 149); no test calls it either. A generic no caller varies. | `fn report_into<W>(&self, w: &mut W) -> std::io::Result<()>` |
| P1-6 | src/main.rs:323 | none | The comment promises a count `paint_line` does not produce: the fn returns `io::Result<()>` and counts no snakes. A stale contract every reader must disprove against the signature. | `// Returns the number of completely printed snakes` |
| P1-7 | src/main.rs:324 | #23 | `paint_line` is 70 lines of nested control: a scan loop, three closures (one capturing and rebinding `pending`), a `while let` over a peekable with two `break`s and two `continue`s, plus a special case. Well past the cognitive bar. | `fn paint_line<Stream, Positions>(` |
| P1-8 | src/main.rs:324 | #37 | Both type parameters are monomorphic: `Stream` is only ever the `StandardStreamLock` threaded from `run` (530-533), `Positions` only ever `best_projection::SharedSegments`. The same `Stream` parameter is repeated on `process`, `process_with_stats` and `output`. | `where`<br>`    Stream: WriteColor,`<br>`    Positions: Iterator<Item = (usize, usize)>,` |
| P1-9 | src/main.rs:338 | #18 | Two labeled phases narrated inside one function: line 338 `skip leading token and leading spaces`, line 361 `special case: all whitespaces`. Each names a step that wants a fn boundary. | `// XXX: skip leading token and leading spaces`<br>`// special case: all whitespaces` |
| P1-10 | src/main.rs:379 | none | `if hi < lo { continue; }` sits in `while let Some((lo, hi)) = shared.peek()` and advances neither the iterator nor `y`, so a shared span with `data_lo < hi < y` spins the loop forever. Every other exit calls `shared.next()` or moves `y`. | `if hi < lo {`<br>`    continue;`<br>`}` |
| P1-11 | src/main.rs:407 | #23 | `process` destructures nine fields, then runs a per-line loop with a peek-guarded early `continue`, a two-arm match whose plus branch builds a 4-tuple of borrows, and nested `has_line_numbers` checks. 90 lines, past the bar. | `fn process<Stream>(&mut self, out: &mut Stream) -> io::Result<()>` |
| P1-12 | src/main.rs:498 | #48 | `push_added` is a private one-line body, one prod call edge (549), no other reference: `self.push_aux(line, true)`. The bool it forwards is already spelled at the call site by the match arm. | `fn push_added(&mut self, line: &[u8]) {`<br>`    self.push_aux(line, true)` |
| P1-13 | src/main.rs:502 | #48 | `push_removed`, same shape: one-line body, one prod call edge (550), no other reference. | `fn push_removed(&mut self, line: &[u8]) {`<br>`    self.push_aux(line, false)` |
| P1-14 | src/main.rs:528 | #59 | `run` is the program's whole engine and carries no doc line: it blocks reading stdin until EOF, locks and writes stdout for the life of the process, and prints timings to stderr. The cost lives entirely off the reader's ability to walk it back. | `fn run(&mut self) -> io::Result<()> {` |
| P1-15 | src/main.rs:537 | #18 | `run` narrates two phases in prose: line 537 `process hunks` over the read loop, line 573 `flush remaining hunk`. That is a function boundary spelled as a comment. | `// process hunks`<br>`// flush remaining hunk` |
| P1-16 | src/main.rs:593 | #37 | `output<Stream: WriteColor>` is instantiated with exactly one type in the crate. Nothing else implements `WriteColor` here and no test substitutes a buffer, so the parameter buys nothing. | `fn output<Stream>(` |
| P1-17 | src/main.rs:652 | none | `index_of` reimplements `buf.iter().position(\|c\| *c == target)` as a 12-line `loop`/`match` over an `enumerate`. One call site (629). | `fn index_of(buf: &[u8], target: u8) -> Option<usize> {`<br>`    let mut it = buf.iter().enumerate();` |
| P1-18 | src/main.rs:690 | none | A hand-maintained 20-entry table of decimal bounds plus a `binary_search` in `width1` computes what `u64::checked_ilog10` gives directly; 22 lines of literals a reader must verify digit by digit, and tests_app.rs:68 exists only to re-verify them. | `const WIDTH: [u64; 20] = [` |
| P1-19 | src/main.rs:736 | #56 | The hand-written `Debug for HunkHeader` (8 lines) has no prod reader: no `{:?}` of a `HunkHeader` anywhere in `src/`. Its only edge is tests_app.rs:33, where `assert_eq!` requires the bound. Delete both, or derive it. | `impl Debug for HunkHeader {` |
| P1-20 | src/main.rs:745 | #32 | `Display for HunkHeader` is reached by nothing at all: no `{}` formatting of a `HunkHeader` in prod or test. rustc's `dead_code` never reports a trait impl, so it stays invisible. | `impl Display for HunkHeader {`<br>`    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtErr> {`<br>`        Debug::fmt(&self, f)` |
| P1-21 | src/main.rs:768 | #6 | `looking_at` reads as a pure lookahead predicate but mutates the cursor: it calls `self.skip_escape_code()`, which advances `self.i`. Callers at 787 and 798 loop on it, so the parser moves inside what its name calls a query. | `fn looking_at<M>(&mut self, matcher: M) -> bool`<br>`    self.skip_escape_code();`<br>`    self.i < self.buf.len() && matcher(self.buf[self.i])` |
| P1-22 | src/main.rs:787 | #20 | The closure body `x.is_ascii_whitespace()` is written four times in this module (787, 849, 862, 866), plus two renamed twins at 351 and 515. A named predicate would carry it once. | `while self.looking_at(\|x\| x.is_ascii_whitespace()) {` |
| P1-23 | src/cli_args.rs:1 | #29 | 396-line module, no `//!` header. Nothing states that this module owns the whole flag grammar and that most of its helpers terminate the process rather than return. | `use super::AppConfig;` |
| P1-24 | src/cli_args.rs:52 | none | `usage` and `help` (59) drop the `write` result with `let _`; a short write silently truncates the help text. `write_all` is the call that means what these two intend. | `let _ = std::io::stderr().write(txt.as_bytes());` |
| P1-25 | src/cli_args.rs:63 | #11 | The four face names live twice: as string pairs in `EnumString::data` (66-71) and again as `write!` arms in `Display::fmt` (78-83). Adding a face means editing both, and the second copy is what the error messages print. | `("added", Added),`<br>`Added => write!(f, "added"),` |
| P1-26 | src/cli_args.rs:220 | #11 | Three `FromStr` impls (220, 227, 234) carry a byte-identical body modulo the error constructor: `tryparse(input).map_err(ArgParsingError::X)`. A blanket impl over `EnumString` plus one error carrier removes two copies. | `fn from_str(input: &str) -> Result<Self, Self::Err> {`<br>`    tryparse(input).map_err(ArgParsingError::FaceName)` |
| P1-27 | src/cli_args.rs:241 | #37 | `ignore<T>` is monomorphic: every one of its five call sites (280-284) passes `&mut ColorSpec`. It is also a no-op a trailing semicolon already performs, so the generic exists to spell discarding. | `fn ignore<T>(_: T) {}` |
| P1-28 | src/cli_args.rs:318 | #11 | `color` (318) and `large_diff` (337) are the same six-line body with one call swapped: take the flag, take the value, `die_error(parse_X(&spec, config))`, else `missing_arg(arg)`. | `let arg = args.next().unwrap();`<br>`if let Some(spec) = args.next() {`<br>`    die_error(parse_color_arg(&spec, config))` |
| P1-29 | src/cli_args.rs:363 | #18 | `parse_options` narrates three labeled sections inside one match: `generic flags` (363), `documented flags` (367), `hidden flags` (371). The grouping is a table the code refuses to be. | `// generic flags`<br>`// documented flags`<br>`// hidden flags` |
| P1-30 | src/diffr_lib/mod.rs:342 | #27 | The module's own header says it holds the Myers diff algorithms, but it also carries `LineSplit`, a byte buffer with line bookkeeping used only by `main`'s hunk loop, inside a 693-line file. Every reader of `diff` pays for it. | `pub struct LineSplit {` |
| P1-31 | src/diffr_lib/mod.rs:56 | #32 | The hand-written `Debug for Tokenization` (14 lines, lossy-decodes every token into a `Vec<String>`) is reached by nothing: no `{:?}` of a `Tokenization` in prod or test. | `impl Debug for Tokenization<'_> {` |
| P1-32 | src/diffr_lib/mod.rs:77 | #32 | Same for `Debug for TokenizationRange` (18 lines): its only potential reader is `#[derive(Debug)] pub struct DiffInput` at 172, and no `DiffInput` is ever formatted anywhere in the crate. | `impl<'a> Debug for TokenizationRange<'a> {` |
| P1-33 | src/diffr_lib/mod.rs:192 | none | `to_owned` owns nothing: it returns a `Self` holding the same borrows with the same `'a`, so the name promises the `ToOwned` contract while delivering a copy of two index ranges. `restart`/`whole` says what line 532 actually wants. | `pub fn to_owned(&'a self) -> Self {`<br>`    Self::new(self.added(), self.removed(), self.large_diff_threshold)` |
| P1-34 | src/diffr_lib/mod.rs:282 | #11 | `diff_sequences_kernel_forward` (282) and `diff_sequences_kernel_backward` (311) are 27-line twins differing in three tokens: the `v(k+1)`/`v(k-1)` comparison order, `y = x - k` versus `y = x - (k + delta)`, and the walk direction. Both are `cfg(test)` reference kernels, and the drift risk is against the prod kernel they check. | `let mut x = if k == -d \|\| k != d && ctx.v(k - 1) < ctx.v(k + 1) {` |
| P1-35 | src/diffr_lib/mod.rs:437 | #23 | `diff_sequences_kernel_bidirectional` is 55 lines of two hand-rolled `while k <= d` sweeps, each with a three-way conditional initializer, an inner walk loop and a compound early return. | `fn diff_sequences_kernel_bidirectional(` |
| P1-36 | src/diffr_lib/mod.rs:450 | #11 | Within that one function the forward sweep (450-469) and the backward sweep (470-489) are the same eight-statement block twice, differing only in sign and comparison direction. Every fix to the snake bookkeeping has two homes. | `let mut k = -d;`<br>`while k <= d {` |
| P1-37 | src/diffr_lib/mod.rs:524 | #23 | `diff` nests a local `enum Task`, a local `fn trivial_diff`, a worklist `while let` + `match`, an `if let` with an `@` binding and struct destructuring, and a three-deep branch on `d`, `len` and `sp`. | `pub fn diff(input: &DiffInput, v: &mut Vec<isize>, dst: &mut Vec<Snake>) {` |
| P1-38 | src/diffr_lib/mod.rs:668 | #36 | `#[allow(clippy::needless_range_loop)]` sits on a loop that is not a range loop: it iterates `src[ofs..].grapheme_indices()`. The suppression names a lint the code can no longer trigger, so it silences whatever that lint would catch here in the future for nothing. | `#[allow(clippy::needless_range_loop)]`<br>`for (grapheme_start, _, g) in src[ofs..].grapheme_indices() {` |
| P1-39 | src/diffr_lib/best_projection.rs:1 | #29 | 221 lines implementing a BFS over `Coord` frontiers with a `prev` backpointer map and a path reconstruction, and no `//!` header. The doc that exists (54, 86) is per-item; nothing says what the file as a whole computes or why the search terminates. | `use std::collections::hash_map::Entry::*;` |
| P1-40 | src/diffr_lib/best_projection.rs:88 | #23 | `optimize_partition` runs a frontier loop over a `for` over `get_indexes`, with a break, a two-branch early exit on `found_seq`, a compound `&&`/`\|\|` promotion condition, an entry match with a `continue`, then a second reconstruction loop. 85 lines. | `pub fn optimize_partition(seq: &Tokenization, lcs: &Tokenization) -> NormalizationResult {` |
| P1-41 | src/diffr_lib/best_projection.rs:182 | #48 | A private one-line `to_isize` with exactly one call edge (118) and no other reference, and it is a second home for `diffr_lib::to_isize` (mod.rs:637) with the debug-assertion branch quietly dropped. Two conversions with different checking, one name. | `fn to_isize(input: usize) -> isize {`<br>`    isize::try_from(input).unwrap()` |
| P1-42 | src/diffr_lib/tests_lib.rs:325 | #44 | `range_equality_test` asserts that two array literals written two lines above are equal and that a third differs. No diffr symbol is touched: it pins `[u8; 3]`'s stdlib `PartialEq`, so it cannot fail on any change to this crate. | `let range_a = [1, 2, 3];`<br>`assert!(range_a == range_b);`<br>`assert!(range_a != range_c);` |

Rules with no site found in this repo, checked deliberately: #9 (no `static` at
all, only `const`), #34 (no commented-out code run of three or more lines; no
match whose every arm re-returns its own pattern), #38 (four prod modules, no
module-level string literal repeated in three of them), #42 (every `#[test]`
reaches a verdict, directly or through `test_cli` / `diff_sequences_test_aux` /
`check_split`), #47 (no sleep anywhere), #53 (no `# Errors` section exists to be
incomplete).

## Phase 2 - audit finding verdicts

Sheet: `corpus-ext/sheets/diffr.rs2.wave1.tsv`, 5 rows, all `rs:48`. 3 real, 2 fp.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| src/diffr_lib/mod.rs:45 | rs:48 | indexed | fp | `TokenMap` is a newtype and `get` is its only read path: folding it makes `Tokenization::new` pun `token_map.0` and hand-write the unwrap that names the map's invariant, so the fold buys a hop by leaking the representation. |
| src/main.rs:148 | rs:48 | indexed | real | One-line body, one edge (main.rs:576 `self.stats.report()?`), no other reference; all it adds over `report_into` is the choice of stderr, which the single caller can spell itself. |
| src/main.rs:498 | rs:48 | indexed | real | One-line forward to `push_aux` with one edge (main.rs:548); the bool it hides is already spelled by the `Some(b'+')` arm it is called from, so the hop costs a name and tells the reader nothing new. |
| src/main.rs:502 | rs:48 | indexed | real | Same shape as `push_added`: one-line forward to `push_aux`, one edge (main.rs:549), and the false it passes is already spelled by the `Some(b'-')` arm at the call site. |
| src/main.rs:757 | rs:48 | indexed | fp | Canonical Rust constructor: the body is the struct literal for its own private-field type, so folding moves field initialization to the call site and leaves the type without a `new`; exempt `fn new(..) -> Self`. |

Note for adjudication, outside this sheet: the audit carries 27 `rs:42` findings
(13 in tests_lib.rs, 14 in tests_cli.rs) whose message is "asserts nothing".
My blind phase-1 read recorded no #42 site here on the ground that every one of
those tests reaches its verdict through a repo helper it calls
(`diff_sequences_test` / `diff_sequences_test_tokenized` -> `diff_sequences_test_aux`,
`test_cli` -> `StringTest::test`, `check_split`), which the rule's own text says
it follows. Those rows are not mine to judge this round; the disagreement is
recorded so a later sheet can price it.

## Phase 3 - reconciliation

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #29 | covered | 29 at main.rs:1, "880 lines, 23 top-level items". |
| P1-2 | #27 | threshold-miss | #27 fired on no module in the repo; main.rs is the largest at 880 lines and still sits under the price bar. |
| P1-3 | #32 | detector-miss | The rs reading walks `pub` items and delegates private ones to rustc; rustc's `dead_code` does not report an unused derived impl either, so nothing covers the unreached `derive(Default)`. |
| P1-4 | #48 | covered | 48 at main.rs:148, judged real in phase 2. |
| P1-5 | #37 | detector-miss | The monomorphic type-parameter arm fired nowhere; `W` has exactly one instantiation, `io::Stderr`. |
| P1-6 | none | inventory-gap | No rule reads a comment against the signature it sits on. |
| P1-7 | #23 | covered | 23 at main.rs:324, cognitive complexity 22. |
| P1-8 | #37 | detector-miss | A finding does sit at main.rs:324, but it is #23's complexity; the monomorphic `Stream`/`Positions` claim is unreported. |
| P1-9 | #18 | threshold-miss | #18 fired nowhere; these two labels are narration (`XXX:`, `special case:`) rather than numbered phases, so the site is the weakest of my three #18 claims. |
| P1-10 | none | inventory-gap | A `continue` that advances neither the peekable nor `y`; correctness sits outside the inventory. |
| P1-11 | #23 | covered | 23 at main.rs:407, cognitive complexity 19. |
| P1-12 | #48 | covered | 48 at main.rs:498, judged real. |
| P1-13 | #48 | covered | 48 at main.rs:502, judged real. |
| P1-14 | #59 | threshold-miss | #59 fired nowhere; `run` spends on a blocking stdin pipe read and a stdout lock held for the process's life, which the off-machine catalog appears not to count. |
| P1-15 | #18 | detector-miss | `// process hunks` and `// flush remaining hunk` are the rule's own shape: two labeled phases narrated in one function. |
| P1-16 | #37 | detector-miss | `output<Stream>` has one instantiation in the crate and no test substitutes a buffer. |
| P1-17 | none | inventory-gap | A stdlib reimplementation (`slice::position`); no rule reads that. |
| P1-18 | none | inventory-gap | A hand-maintained constant table replaceable by `checked_ilog10`; no rule reads that. |
| P1-19 | #56 | detector-miss | A trait impl is neither a `pub` item nor an edge target the index resolves, so a `Debug` impl reached only by a test `assert_eq!` is invisible to the rs reading. |
| P1-20 | #32 | detector-miss | Same blind spot for a wholly unreached `Display` impl; rustc's `dead_code` never reports trait impls, so the complement does not cover it either. |
| P1-21 | #6 | threshold-miss | `looking_at` is a `&mut self -> bool` predicate that advances the cursor through `skip_escape_code`; the accessor-name lexicon does not carry the name, so the shape goes unread. |
| P1-22 | #20 | threshold-miss | The four copies of `x.is_ascii_whitespace()` are a single method call on the parameter and fall under the nontriviality bar. |
| P1-23 | #29 | covered | 29 at cli_args.rs:1, "396 lines, 34 top-level items". |
| P1-24 | none | inventory-gap | `write` where `write_all` is meant; no rule reads a dropped short-write. |
| P1-25 | #11 | threshold-miss | The two copies are a slice literal and a match, so a blind structural digest cannot group them; #11 did fire twice elsewhere in this same file. |
| P1-26 | #11 | threshold-miss | #11's clone arm grouped 5- and 6-statement bodies (cli_args 318/337, mod.rs 637/645); these three `from_str` bodies are one line each and fall under the size bar. |
| P1-27 | #37 | detector-miss | `ignore<T>` has five call sites, all passing `&mut ColorSpec`. |
| P1-28 | #11 | covered | 11 at cli_args.rs:318 and :337, "structural clone x2: color, large_diff". |
| P1-29 | #18 | detector-miss | Three labeled sections inside `parse_options`. |
| P1-30 | #27 | threshold-miss | 693-line module, no #27 anywhere in the repo. |
| P1-31 | #32 | detector-miss | Trait-impl blind spot again: 14 lines of unreached hand-written `Debug`. |
| P1-32 | #32 | detector-miss | Same, 18 lines, and its only potential reader (`derive(Debug)` on `DiffInput`) is itself never formatted. |
| P1-33 | none | inventory-gap | A name that promises the `ToOwned` contract while returning borrows; no rule reads that class of dishonest name. |
| P1-34 | #11 | threshold-miss | The twin kernels differ in several leaves (`y = x - k` against `y = x - (k + delta)`, the loop guards, `= x` against `= x - 1`), so a T2 digest separates them. |
| P1-35 | #23 | covered | 23 at mod.rs:437, cognitive complexity 24. |
| P1-36 | #11 | threshold-miss | Same leaf-level drift between the forward and backward sweeps inside the one function. |
| P1-37 | #23 | covered | 23 at mod.rs:524, cognitive complexity 30. |
| P1-38 | #36 | threshold-miss | One `#[allow]` in 693 lines is under any density bar, and the defect here is staleness (the lint it names cannot fire on a `grapheme_indices` loop), which the density arm does not read. |
| P1-39 | #29 | covered | 29 at best_projection.rs:1, "221 lines, 8 top-level items". |
| P1-40 | #23 | covered | 23 at best_projection.rs:88, cognitive complexity 37, the repo's highest. |
| P1-41 | #11 | threshold-miss | Judge error in the phase-1 row: `to_isize` has three call sites (118, 159, 160), not one, so #48 was right to stay silent. The surviving half of the claim is the duplicate conversion across modules, and #11 grouped only the same-module `to_isize`/`to_usize` pair at mod.rs:637/645. |
| P1-42 | #44 | detector-miss | `assert!(range_a == range_b)` over two adjacent call-free array literals is the rule's ideal; reaching it needs constant propagation through the two `let` bindings. |

Totals: 42 sites, 12 covered, 30 misses (12 detector-miss, 12 threshold-miss,
6 inventory-gap). The covered set is exactly the three rules with a body of
Rust judgment behind them (#23 5/5, #29 4/4 of my prod claims, #11 1 of 5,
#48 3 of 4 real claims); every site of the six unmeasured Rust readings I
recorded (#6, #32, #36, #37, #56, #59) went unreported.

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| rs:48 | constructor: `fn new(..) -> Self` whose body is the struct literal of its own type, folding it moves field initialization to the call site | 1 | src/main.rs:757:48:fold:diffr::LineNumberParser::new |
| rs:48 | sole read path over a private field of a newtype, folding it leaks the representation and the invariant the body names | 1 | src/diffr_lib/mod.rs:45:48:fold:diffr::diffr_lib::TokenMap::get |
