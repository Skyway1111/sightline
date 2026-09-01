# ab-av1 - wave 1

Repo: `../gauntlet-corpus/ab-av1` (Rust 2024, bin crate, 5916 lines of `.rs`
under `src/`, no `tests/`, `benches/` or `examples/` dirs; test code is
`#[test]` fns and `#[cfg(test)] mod test` inline in the prod modules).

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/command/sample_encode.rs:1 | #29 | Largest module in the crate (875 lines) opens with `mod cache;` and no `//!` header. A reader hitting it has no statement of what the module owns (CLI args, the sample-encode stream, `EncodeResult`, `Output`, `Status`, `Update`, the stdout formatter). | `mod cache;` |
| P1-2 | src/command/args/encode.rs:1 | #29 | 801-line module, no `//!` header; it holds the `Encode` CLI args, the `Encoder` codec-defaults table, `KeyInterval`, `PixelFormat` and ~280 lines of tests, and nothing on the first screen says so. | `use crate::{` |
| P1-3 | src/command/crf_search.rs:1 | #29 | 673 lines, no `//!` header, even though this is the module that carries the whole search algorithm and its q/crf conversion contract. | `mod err;` |
| P1-4 | src/command/args/vmaf.rs:1 | #29 | 463 lines, no `//!` header; the module encodes the whole libvmaf lavfi-construction and model-selection policy. | `use crate::command::args::PixelFormat;` |
| P1-5 | src/process.rs:1 | #29 | 420 lines, no `//!` header; the module mixes exit-code helpers, ffmpeg stderr parsing, the `Chunks` ring buffer, a `Stream` impl and the `CommandExt`/`ArgString` arg-building traits. | `pub mod child;` |
| P1-6 | src/command/encode.rs:1 | #29 | 263 lines, no `//!` header, while its sibling `sample.rs`, `vmaf.rs`, `xpsnr.rs`, `ffmpeg.rs`, `ffprobe.rs`, `temporary.rs` and `command/args.rs` all have one. The convention exists and this module breaks it. | `use crate::{` |
| P1-7 | src/sample.rs:1 | #29 | The `//!` header is wrong: this module copies sample clips (`sample::copy`), but its header is `//! ffmpeg logic`, byte-identical in intent to `ffmpeg.rs:1`'s `//! ffmpeg encoding logic`. A header that names the wrong subject is worse than none. | `//! ffmpeg logic` |
| P1-8 | src/command/sample_encode.rs:144 | #27 | `run` plus six public types plus the `EncodeResults` trait live in one 875-line file that `crf_search.rs`, `auto_encode.rs` and `sample_encode/cache.rs` all import symbols from. Every task touching a hot symbol (`Output`, `Status`, `Update`, `EncodeResult`) pays the whole file. | `pub fn run(` |
| P1-9 | src/command/args/encode.rs:18 | #27 | `Encode`, `Encoder`, `PixelFormat` and `KeyInterval` are imported by nearly every command module (`sample_encode`, `crf_search`, `encode`, `auto_encode`, `command/vmaf`, `command/xpsnr`, `ffmpeg`, `ffprobe`), and all four are hot symbols inside an 801-line file. | `pub struct Encode {` |
| P1-10 | src/command/crf_search.rs:52 | #27 | `SearchArgs`, `Sample`, `Error` and `guess_progress` are consumed by `auto_encode.rs`, but they sit in a 673-line file that also carries the interpolation algorithm, `QualityConverter` and the human/json rendering. | `pub struct SearchArgs {` |
| P1-11 | src/vmaf.rs:11 vs src/xpsnr.rs:12 | #11 | `vmaf::run` and `xpsnr::run` are the same function: same info! shape, same `Command::new("ffmpeg")` builder modulo `-an -sn -dn` and input order, same `AddOnDropChunkStream` wrap, same `async_stream::stream!` body with `parsed_done` and the same three `Item` arms. One parameterised runner would carry both. | `let mut vmaf = crate::process::child::AddOnDropChunkStream::from(` |
| P1-12 | src/vmaf.rs:84 vs src/xpsnr.rs:80 | #11 | `VmafOut::try_from_chunk` and `XpsnrOut::try_from_chunk` are the same three-step body (push chunk, look for a score line, else try a progress line, else None); only the score extractor differs. The two enums `VmafOut`/`XpsnrOut` are also structurally identical (`Progress(FfmpegOut) \| Done(f32) \| Err`). | `if let Some(progress) = FfmpegOut::try_parse(chunks.last_line()) {` |
| P1-13 | src/command/vmaf.rs:46 vs src/command/xpsnr.rs:43 | #11 | The two command entry points are line-for-line the same function (bar setup, two probes, `nframes().or_else`, `duration.as_ref().or`, `set_length`, the pinned stream, `ProgressLogger`, the four-arm match, `bar.finish()`, `println!`). Only the lavfi expression and the message prefix differ. | `let nframes = dprobe.nframes().or_else(\|_\| rprobe.nframes());` |
| P1-14 | src/command/sample_encode.rs:620 vs :626 | #11 | `mean_vmaf_score` and `mean_xpsnr_score` have identical three-line bodies differing only in the field read. | `let mut scores = self.iter().filter_map(\|r\| r.vmaf_score).peekable();` |
| P1-15 | src/command/sample_encode.rs:632 vs :651 | #11 | `estimate_encode_size_by_duration` and `estimate_encode_time` share the same six-statement skeleton (empty guard, single-full-pass guard, sample_duration sum, sample_factor, sum, scale). A drift in one guard silently desynchronises the size and time predictions. | `let sample_factor = input_duration.as_secs_f64() / sample_duration.as_secs_f64();` |
| P1-16 | src/command/sample_encode.rs:513 vs :540 | #11 | `print_attempt` and `log_attempt` destructure the same five fields and build the same percent / VMAF / XPSNR / cache message in a different order; the two renderings of one fact will drift. | `let Self { sample_size, encoded_size, vmaf_score, xpsnr_score, from_cache, .. } = self;` |
| P1-17 | src/command/crf_search.rs:186 vs src/command/auto_encode.rs:111 | #11 | The `Update::Status` arm is duplicated verbatim: the same nested `sample_encode::Status` destructure, `guess_progress` call, `TerseF32`, the full-pass prefix match and the three-arm fps-message match. ~25 lines, identical. | `bar.set_position(crf_search::guess_progress(crf_run, progress, thorough) as _);` |
| P1-18 | src/command/sample_encode.rs:307 vs :356 | #11 | The xpsnr scoring block and the vmaf scoring block inside `run` are the same nine-statement shape (yield init Status, build lavfi, call `run`, `pin!`, `ProgressLogger`, `while let` over four match arms yielding a Status and calling `logger.update`). | `let mut logger = ProgressLogger::new("ab_av1::xpsnr", Instant::now());` |
| P1-19 | src/command/sample_encode.rs:331 and :387 | #11 | Inside those two blocks the progress fraction is written four times as the same hand-inlined `(a + b + sample_idx * sample_duration_us * 2) / (sample_duration_us * samples * 2)` arithmetic with different numerators. One `fn progress_frac(..)` would hold the invariant that the four expressions must agree on. | `\|\| (sample_duration_us * samples * 2) as f32` |
| P1-20 | src/process/child.rs:56 vs :63 | #11 | `log_waiting` and `log_abort_wait` are the same two-arm terminal/non-terminal match differing only in the message strings. | `match std::io::stderr().is_terminal() {` |
| P1-21 | src/command/args/encode.rs:222 vs :264 | #11 | The `enc_args` and `enc_input_args` flat_map closures are the same `split_once('=')` split-into-two-args logic written twice (the second only differing by `.into_iter()`); the `-svtav1-params` special case in the first is the kind of divergence this duplication invites. | `if let Some((opt, val)) = arg.split_once('=') {` |
| P1-22 | src/command/sample_encode.rs:88, crf_search.rs:156, encode.rs:53, auto_encode.rs:66, command/vmaf.rs:54, command/xpsnr.rs:51 | #11 | The same four-line progress-bar construction (`ProgressBar::new(..).with_style(ProgressStyle::default_bar().template(..)?.progress_chars(PROGRESS_CHARS))` then `enable_steady_tick(Duration::from_millis(100))`) is copied into six modules. | `bar.enable_steady_tick(Duration::from_millis(100));` |
| P1-23 | src/command/encode.rs:88 vs src/command/auto_encode.rs:57 | #11 | The same-file guard, including its two-line message, is duplicated between `encode::run` and `auto_encode`. | `overwrite_input \|\| !is_same_file(&output, &args.input).unwrap_or(false),` |
| P1-24 | src/command/args.rs:168 vs src/command/args/vmaf.rs:76 | #11 | `Xpsnr::fps` and `Vmaf::fps` are the same one-liner over differently named fields; both encode the "0 disables" contract, so the contract has two homes. | `Some(self.xpsnr_fps).filter(\|r\| *r > 0.0)` |
| P1-25 | src/command/sample_encode.rs:734 vs src/command/crf_search.rs:471 | #11 | The `image`/`video stream` label match is copied into both renderers, as is the "predicted {enc_description} size {size} ({percent}) taking {time}" sentence they build from it. | `let enc_description = match image { true => "image", false => "video stream" };` |
| P1-26 | src/ffprobe.rs:76, :86, :125 | #20 | `\|s\| s.codec_type.as_deref() == Some("video")` is written three times in one module (plus the `Some("audio")` twin at :65 and :69). Name the predicate once. | `.filter(\|s\| s.codec_type.as_deref() == Some("video"))` |
| P1-27 | src/command/crf_search.rs:22, src/command/auto_encode.rs:20, src/command/sample_encode.rs:85 | #38 | `1024 * 1024 * 1024` is declared as `BAR_LEN` in three modules of one crate. `crf_search::guess_progress` returns positions scaled by its own copy while `auto_encode` sets them on a bar sized by another: the value is a cross-module contract with three homes. | `const BAR_LEN: u64 = 1024 * 1024 * 1024;` |
| P1-28 | src/command/sample_encode.rs:90, src/command/crf_search.rs:158, src/command/auto_encode.rs:42 | #38 | The prefixed progress-bar template literal is written out in full in three modules (the third names it `SPINNER_RUNNING`, proving it deserves a name). | `"{spinner:.cyan.bold} {elapsed_precise:.bold} {prefix} {wide_bar:.cyan/blue} ({msg}eta {eta})"` |
| P1-29 | src/command/encode.rs:55, src/command/vmaf.rs:56, src/command/xpsnr.rs:53 | #38 | The unprefixed progress-bar template literal is likewise spelled out in three modules. Combined with P1-28 there are five copies of two near-identical format strings and no single home for the bar's look. | `.template("{spinner:.cyan.bold} {elapsed_precise:.bold} {wide_bar:.cyan/blue} ({msg}eta {eta})")?` |
| P1-30 | src/command/sample_encode.rs:144 | #23 | `run` is a ~320-line `try_stream!` body: a spawned task, a receive loop, a cache match, a nested encode-progress loop, then two nested scoring loops each with a four-arm match and an inner branch on the other scorer. Cognitive complexity is far past any reading budget; the xpsnr and vmaf halves are the natural extractions. | `async_stream::try_stream! {` |
| P1-31 | src/command/crf_search.rs:251 | #23 | `run` holds the whole search: a 15-field destructure, the q conversion setup, an unbounded `for run in 1..`, an inner stream loop, then two mirrored five-arm `match` blocks over the bound search with guards. The `unreachable!()` at the end is the tell that the control flow no longer fits in the head. | `for run in 1.. {` |
| P1-32 | src/command/args/encode.rs:185 | #23 | `to_ffmpeg_args` is ~150 lines: svt-av1 param assembly, two flat_map arg expansions, a keyint branch, two default-arg merge loops, a "none" splice loop and two reserved-arg validation loops over a locally built `HashMap`. | `pub fn to_ffmpeg_args(` |
| P1-33 | src/command/auto_encode.rs:41 | #23 | `auto_encode` mixes output-name defaulting, the same-file guard, two bar styles, a six-arm match over `crf_search::Update` with a nested error branch, then a second bar and the final `encode::run` call. | `while let Some(update) = crf_search.next().await {` |
| P1-34 | src/command/encode.rs:64 | #23 | `run` interleaves output defaulting, the overwrite guard, ffmpeg arg construction, bar length maths, the downmix rule, the encode loop, two verify passes and the stream-size report. The three `verify` flags are resolved by two rebindings mid-function (`let verify_decode = verify \|\| verify_decode;`). | `let verify_decode = verify \|\| verify_decode;` |
| P1-35 | src/command/crf_search.rs:148 | #23 | `crf_search` wraps a five-arm `Update` match, each arm with its own nested branching (an `inspect_err` closure that itself branches on `Error::NoGoodCrf` and on `StdoutFormat`), and terminates in `unreachable!()`. | `let update = update.inspect_err(\|e\| {` |
| P1-36 | src/ffmpeg.rs:207 | #37 | `VCodecSpecific` is a private trait with exactly one impl (`for Arc<str>`, :215) and it is a second home for encoder-specific knowledge: `Encoder` in `command/args/encode.rs:365` already owns `default_crf_increment`, `default_min_crf`, `default_max_crf`, `default_ffmpeg_args` and `default_ffmpeg_input_args` for the same codec names. Two files must be edited to add a codec. | `trait VCodecSpecific {` |
| P1-37 | src/process/child.rs:102 | #37 | `Exited` is a private single-method trait with exactly one impl, used at exactly one call site (`add`, :24 and :29). A free `fn exited(s: &mut ProcessChunkStream) -> bool` in the same module says the same thing without the trait. | `trait Exited {` |
| P1-38 | src/command/sample_encode.rs:196, :242, :421, :434 | #18 | `run` narrates its phases in prose ("Start creating copy samples async...", "encode sample", "Early clean...", "ensure sample_task completed"). Four labelled phases inside one function body are four function boundaries written as comments. | `// Start creating copy samples async, this is IO bound & not cpu intensive` |
| P1-39 | src/command/encode.rs:121, :167, :202 | #18 | `run` is likewise phase-narrated ("only downmix if achannels > 3", "verified before moving into place...", "print output info"), each comment marking a block that could be its own named function. | `// print output info` |
| P1-40 | src/command/crf_search.rs:531 | #42 | `parse_stdout_format` has no verdict on the parsed value: its only oracle is `expect` on the `Result`, so it passes for any successful parse, including one that routed `json` to the wrong field. Assert on `args.stdout_format` instead. | `Args::try_parse_from(["crf-search", "-i", "vid.mkv", "--stdout-format", "json"])` |
| P1-41 | src/command/args/encode.rs:795 | #44 | This assertion cannot fail: `"hcv1"` is a typo of the tag under test (`"hvc1"`, asserted three lines above), so no code path could ever produce it. The check the author meant (no `hvc1` when the user set `hev1`) is not being made. | `.any(\|w\| w[0].as_str() == "-tag:v" && w[1].as_str() == "hcv1"),` |
| P1-42 | src/command/args/encode.rs:602 vs :667 | none | `svtav1_to_ffmpeg_args_default_over_3m` and `svtav1_to_ffmpeg_args_default_under_3m` are the same 60-line test body (same `Encode` literal, same `Ffprobe` literal, same nine-field destructure, same seven asserts) differing in the probe duration and three expected values. A table-driven pair would remove 55 duplicated lines. #11 reads prod only, so no listed rule covers a clone in test code. | `let FfmpegEncodeArgs { input, vcodec, vfilter, pix_fmt, crf, preset, output_args, input_args, video_only } = enc` |
| P1-43 | src/vmaf.rs:187 vs src/xpsnr.rs:218 vs src/xpsnr.rs:287 | none | The chunk-feeding loop (`const CHUNK_SIZE: usize = 64;` then the `while start_idx < ffmpeg.len()` slice/parse/collect body) is copied three times across the two test modules. Same reason as P1-42: test-code duplication is outside #11's prod scope. | `let chunk = &ffmpeg[start_idx..(start_idx + CHUNK_SIZE).min(FFMPEG_OUT.len())];` |
| P1-44 | src/xpsnr.rs:227 | none | Commented-out code left in place: the `println!` that the sibling tests at :196 and :296 still run live. #34 (rs) needs a run of >=3 parseable comment lines and reads prod only, so nothing covers a single commented-out line in a test module. | `// println!("* {}", String::from_utf8_lossy(chunk).trim());` |
| P1-45 | src/command/args/vmaf.rs:71 | none | `parse_vmaf_arg` is an infallible function typed as fallible: it is `Ok(arg.to_owned().into())` with no failure path, wired in as `value_parser = parse_vmaf_arg` where clap's own `Arc<str>` conversion would do. Compare `parse_svt_arg` / `parse_enc_arg` in `command/args/encode.rs`, which do validate. No listed rule reaches an unexercised `Result` on a free function. | `fn parse_vmaf_arg(arg: &str) -> anyhow::Result<Arc<str>> { Ok(arg.to_owned().into()) }` |
| P1-46 | src/xpsnr.rs:11 | none | A `// TODO` marks a known defect in shipped behaviour ("fix progress update to account for fps"), and it is the same progress arithmetic P1-19 flags as duplicated four ways. No listed rule reads TODO markers. | `// TODO: fix progress update to account for fps` |

### Rules with no site found

* **#34** (commented-out code, prod): none. The only commented-out line in the
  tree is in a test module (P1-44). Every other `//` run in prod is genuine
  explanation.
* **#36** (`#[allow]` density): none. The crate carries exactly one `#[allow]`
  in 5916 lines (`src/command/sample_encode/cache.rs:12`,
  `clippy::too_many_arguments`) and denies `unused_crate_dependencies` at the
  manifest. Nothing here blinds the build.
* **#47** (sleepy test): none. No `sleep` anywhere in the tree; the one
  wall-clock wait is a prod lock retry (`cache.rs:93`), not a test.

## Phase 2 - audit finding verdicts

32 findings across 7 rules. **25 real, 7 fp.** The full `why` for each
row is in `corpus-ext/sheets/ab-av1.rs1.wave1.tsv`; the `why` column below is
its short form. Two rows share the key `src/command/xpsnr.rs:118:11:clone-block:204504d41fb0`
(same site, two symbols); both are judged the same and both are listed.

| rule | findings | real | fp |
|------|----------|------|----|
| rs:11 | 12 | 8 | 4 |
| rs:20 | 2 | 1 | 1 |
| rs:23 | 4 | 4 | 0 |
| rs:27 | 1 | 1 | 0 |
| rs:29 | 8 | 8 | 0 |
| rs:37 | 4 | 2 | 2 |
| rs:42 | 1 | 1 | 0 |
| **total** | **32** | **25** | **7** |

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| src/command/args/encode.rs:389 | rs:11 | indexed | fp | Two unrelated codec lookup tables (crf floor vs image extension) share only the match-on-as_str shape. |
| src/command/args/encode.rs:408 | rs:11 | indexed | fp | Other member of the same group; the shape is the codec dispatch idiom, used by four sibling methods. |
| src/command/sample_encode.rs:620 | rs:11 | indexed | real | mean_vmaf_score / mean_xpsnr_score identical but for the field read. |
| src/command/sample_encode.rs:626 | rs:11 | indexed | real | Second member of that pair; one averaging rule with two homes. |
| src/command/vmaf.rs:54 | rs:11 | indexed | real | Eight-statement bar-and-probe setup shared with command::xpsnr. |
| src/command/vmaf.rs:80 | rs:11 | indexed | real | Six-statement progress drain shared with command::xpsnr. |
| src/command/xpsnr.rs:51 | rs:11 | indexed | real | Mirror of the setup clone; the whole command is a copy of command::vmaf. |
| src/command/xpsnr.rs:78 | rs:11 | indexed | real | Mirror of the drain clone; duplicated progress-reporting contract. |
| src/command/xpsnr.rs:118 | rs:11 | indexed | fp | add_filter is nested inside lavfi, so its body is counted as its own and as its parent's; no second copy exists. |
| src/command/xpsnr.rs:118 | rs:11 | indexed | fp | add_filter is nested inside lavfi, so its body is counted as its own and as its parent's; no second copy exists. |
| src/process/child.rs:56 | rs:11 | indexed | real | log_waiting / log_abort_wait differ only in message strings. |
| src/process/child.rs:63 | rs:11 | indexed | real | Second member of that pair; the terminal-vs-log routing rule is written twice. |
| src/ffmpeg.rs:100 | rs:20 | heuristic | fp | Body occurs twice, not three times; the third site is a different closure over a different type, merged by parameter-name normalization. |
| src/ffprobe.rs:76 | rs:20 | heuristic | real | Video predicate at :76, :86, :125 plus the audio twin at :65, :69. |
| src/command/args/encode.rs:185 | rs:23 | heuristic | real | 150 lines, seven distinct arg-assembly and validation passes. |
| src/command/auto_encode.rs:41 | rs:23 | heuristic | real | Defaulting, guard, two bar styles, a six-arm Update match with a nested error branch, then the encode. |
| src/command/crf_search.rs:148 | rs:23 | heuristic | real | Five-arm match with branching arms plus a branching inspect_err closure, ending in unreachable!(). |
| src/command/encode.rs:64 | rs:23 | heuristic | real | Eight concerns in one body, with the verify flags rebound mid-function. |
| src/command/sample_encode.rs:1 | rs:27 | indexed | real | 875 lines, seven exported types, three importing modules. |
| src/command/args/encode.rs:1 | rs:29 | heuristic | real | 801 lines, opens on a use block. |
| src/command/args/vmaf.rs:1 | rs:29 | heuristic | real | 463 lines carrying the lavfi and model-selection policy. |
| src/command/auto_encode.rs:1 | rs:29 | heuristic | real | Weakest of the eight at 206 lines; the Args doc is clap help, not a module statement. |
| src/command/crf_search.rs:1 | rs:29 | heuristic | real | 673 lines, opens on `mod err;`. |
| src/command/encode.rs:1 | rs:29 | heuristic | real | 263 lines and it owns the output-naming helpers other modules import. |
| src/command/sample_encode.rs:1 | rs:29 | heuristic | real | Largest module in the crate, opens on `mod cache;`. |
| src/command/xpsnr.rs:1 | rs:29 | heuristic | real | Weakest of the eight at 179 lines, but it also exports lavfi to sample_encode. |
| src/process.rs:1 | rs:29 | heuristic | real | 420 lines mixing five unrelated concerns; the module most needing one orienting line. |
| src/command.rs:21 | rs:37 | indexed | fp | Extension trait on std Duration; one impl is the pattern, and callers need method position. |
| src/command/sample_encode.rs:593 | rs:37 | indexed | fp | Extension trait on the foreign type Vec<EncodeResult>; the single impl is structural. |
| src/ffmpeg.rs:207 | rs:37 | indexed | real | Codec semantics hung off Arc<str> while Encoder already owns the same codec table. |
| src/process/child.rs:102 | rs:37 | indexed | real | One-method trait with two call sites in one fn; a private free fn is simpler. |
| src/command/crf_search.rs:532 | rs:42 | indexed | real | Only oracle is expect on the Result; the parsed value is never inspected. |

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| rs:11 | unrelated small dispatch tables normalized into one group: same `match self.as_str()` skeleton, different return types and unrelated facts | 2 | `src/command/args/encode.rs:389:11:clone:4e52c4ebac5f` |
| rs:11 | nested fn grouped with its own parent: the inner fn's statements are counted once as its body and once inside the enclosing fn, so the group has one physical member | 2 | `src/command/xpsnr.rs:118:11:clone-block:204504d41fb0` |
| rs:20 | trivial one-token method-forward closures merged across receiver types by parameter-name normalization; the reported count of 3 is also false (the body appears twice) | 1 | `src/ffmpeg.rs:100:20:closure:ab_av1::ffmpeg:897e2898` |
| rs:37 | extension trait on a foreign type (`Duration`, `Vec<_>`): a single impl is the only shape the pattern can have, not unexercised flexibility | 2 | `src/command.rs:21:37:single-impl:ab_av1::command::SmallDuration` |

## Phase 3 - reconciliation

46 phase-1 sites: **19 covered, 9 detector-miss, 13 threshold-miss, 5 inventory-gap.**

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #29 | covered | `src/command/sample_encode.rs:1` fires, same premise (875 lines, no `//!`). |
| P1-2 | #29 | covered | `src/command/args/encode.rs:1`. |
| P1-3 | #29 | covered | `src/command/crf_search.rs:1`. |
| P1-4 | #29 | covered | `src/command/args/vmaf.rs:1`. |
| P1-5 | #29 | covered | `src/process.rs:1`. |
| P1-6 | #29 | covered | `src/command/encode.rs:1`. The rule also fired on two modules I had left out (auto_encode 206 lines, command/xpsnr 179); I judged both real, so its cutoff sits below mine, not above. |
| P1-7 | #29 | detector-miss | `sample.rs` has a `//!`, so the rule passes it; the header says `ffmpeg logic`, which is ffmpeg.rs's subject, not this module's. The check is presence-only and cannot see a header that names the wrong thing. |
| P1-8 | #27 | covered | `src/command/sample_encode.rs:1`, 2 hot symbols counted. |
| P1-9 | #27 | threshold-miss | args/encode.rs is 801 lines and exports `Encode`, `Encoder`, `PixelFormat` and `KeyInterval` to eight modules, but no #27 finding; it falls under the hot-symbol or size cutoff that sample_encode cleared. |
| P1-10 | #27 | threshold-miss | Same cutoff: crf_search.rs is 673 lines and hands `SearchArgs`, `Sample`, `Error` and `guess_progress` to auto_encode. |
| P1-11 | #11 | detector-miss | `vmaf::run` and `xpsnr::run` are near-identical whole functions, but each body is an `async_stream::stream!` invocation; nothing in the crate's macro bodies produced a clone digest. The rule found the command-level wrappers around them and missed the pair itself. |
| P1-12 | #11 | threshold-miss | `VmafOut::try_from_chunk` / `XpsnrOut::try_from_chunk` share a three-step body but differ in the score-extraction call, so the pair sits under the fn-clone similarity bar that the mean_* pair cleared. |
| P1-13 | #11 | covered | Four findings: `command/vmaf.rs:54` + `command/xpsnr.rs:51` (8 stmts) and `command/vmaf.rs:80` + `command/xpsnr.rs:78` (6 stmts). |
| P1-14 | #11 | covered | `sample_encode.rs:620` and `:626`. |
| P1-15 | #11 | threshold-miss | `estimate_encode_size_by_duration` / `estimate_encode_time` share a six-statement skeleton in the same impl as the covered mean_* pair, but differ in return type and two constants; under the bar. |
| P1-16 | #11 | threshold-miss | `print_attempt` / `log_attempt` share the five-field destructure and the same rendered facts in a different order; the reordering puts them under the bar. |
| P1-17 | #11 | detector-miss | ~25 verbatim lines duplicated between `crf_search.rs:186` and `auto_encode.rs:111`, the largest exact copy in the crate. Both sit inside a `match` arm rather than a straight-line statement run, and the block arm did not group them. |
| P1-18 | #11 | detector-miss | The xpsnr and vmaf scoring blocks inside `sample_encode::run` are the same nine-statement shape, but they live inside `async_stream::try_stream!` and are invisible for the same reason as P1-11. |
| P1-19 | #11 | detector-miss | The four hand-inlined progress fractions are expression-level and inside the same macro body. |
| P1-20 | #11 | covered | `process/child.rs:56` and `:63`. |
| P1-21 | #11 | threshold-miss | Two copies of the `split_once('=')` flat_map closure, below the group size the rule reports, and closure bodies are not fn bodies. |
| P1-22 | #11 | covered | Two of the six copies are grouped (`command/vmaf.rs:54`, `command/xpsnr.rs:51`) because they sit inside a longer shared run. The other four (sample_encode.rs:88, crf_search.rs:156, encode.rs:53, auto_encode.rs:66) were not, so the six-way group is understated as a pair. |
| P1-23 | #11 | threshold-miss | The duplicated same-file guard is one `ensure!` statement, under the five-statement block bar. |
| P1-24 | #11 | threshold-miss | `Xpsnr::fps` / `Vmaf::fps` are one-statement fn bodies, under the clone bar. |
| P1-25 | #11 | threshold-miss | The `image` / `video stream` label match is a four-line block, under the five-statement bar. |
| P1-26 | #20 | covered | `src/ffprobe.rs:76`, same three sites. |
| P1-27 | #38 | detector-miss | `BAR_LEN = 1024 * 1024 * 1024` is declared at module level in three modules of one crate, which is exactly #38's shape, but the rs check reads string literals only, so an integer const of the same provenance is invisible. No #38 finding fired anywhere in this repo. |
| P1-28 | #38 | threshold-miss | The prefixed bar template is the same string literal in three modules, but two of the three declare it inside a function; the rule requires module level. |
| P1-29 | #38 | threshold-miss | Same scope cutoff for the unprefixed template: three modules, all three function-local. |
| P1-30 | #23 | detector-miss | `sample_encode::run` is the largest and most branched function in the crate and scored nothing, because its whole body is `async_stream::try_stream! { .. }`; the cc walk does not descend into the macro. This is the single biggest gap in the audit. |
| P1-31 | #23 | detector-miss | `crf_search::run` for the same reason: an unbounded `for run in 1..` with two mirrored five-arm matches, all inside `try_stream!`. |
| P1-32 | #23 | covered | cc 34. |
| P1-33 | #23 | covered | cc 27. |
| P1-34 | #23 | covered | cc 38. |
| P1-35 | #23 | covered | cc 28. |
| P1-36 | #37 | covered | `src/ffmpeg.rs:207`. |
| P1-37 | #37 | covered | `src/process/child.rs:102`. The rule also fired on two extension traits over foreign types that I did not list and judged fp. |
| P1-38 | #18 | threshold-miss | Four phase-narrating comments in `sample_encode::run`, but none carries a phase label the rule can key on, and the body is inside `try_stream!` as in P1-30. |
| P1-39 | #18 | threshold-miss | Same shape in `encode::run`: unlabelled prose phase markers. |
| P1-40 | #42 | covered | `crf_search.rs:532`. |
| P1-41 | #44 | detector-miss | The assertion cannot fail because its expected literal `hcv1` is a typo of the `hvc1` under test, but the expression is full of method calls, so it is not a self-comparison or a constant. Catching this needs a near-miss check between a literal and the literals asserted around it. |
| P1-42 | none | inventory-gap | Test-code clone: #11 reads prod only. |
| P1-43 | none | inventory-gap | Test-code clone across two test modules. |
| P1-44 | none | inventory-gap | Commented-out line in a test module: #34 reads prod and wants a run of three. |
| P1-45 | none | inventory-gap | An infallible fn typed as fallible; no listed rule reads an unexercised `Result`. |
| P1-46 | none | inventory-gap | A TODO marking a known defect; no listed rule reads TODO markers. |

### The one structural cause behind most of the misses

Six misses (P1-11, P1-18, P1-19, P1-30, P1-31, and half of P1-38) are one
defect: `async_stream::stream!` / `try_stream!` bodies are opaque to the AST
passes. Those macros hold `sample_encode::run`, `crf_search::run`,
`vmaf::run` and `xpsnr::run`, which together are the four functions doing the
real work in this crate. Every #23 finding that did fire is on a plain fn,
and the two largest functions in the repo scored nothing at all. In a crate
built on async-stream this reads as a checker with a blind spot exactly
where the complexity is.
