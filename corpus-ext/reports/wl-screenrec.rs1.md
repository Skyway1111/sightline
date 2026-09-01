# wl-screenrec (Rust) judge report

Repo: `../gauntlet-corpus/wl-screenrec` @ v0.3.2, edition 2024.
Prod tree: `build.rs` + `src/` (9 files, 4181 lines). Test code: `tests/cmdline.rs`
and the `#[cfg(test)]` modules in `src/fps_limit.rs` and `src/transform.rs`.

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/main.rs:1 | #27 | `main.rs` is 2431 of the crate's 4181 prod lines, 5x the next module. It holds `Args`, the `CaptureSource` trait, `State`, `EncState`, 11 `Dispatch` impls, geometry parsing, encoder/pixfmt selection, the history ring buffer, `main` and `execute`. Every task that touches any one of those symbols has to ingest all of it. | `struct State<S: CaptureSource> {` |
| P1-2 | src/main.rs:1 | #29 | The crate's largest module opens with `extern crate` and 95 lines of `use`; no `//!` header says what the file is or how the capture/negotiate/encode state machine is staged. | `extern crate ffmpeg_next as ffmpeg;` |
| P1-3 | src/avhw.rs:1 | #29 | 465-line ffmpeg hwcontext wrapper (VAAPI and Vulkan device/frame contexts, DRM modifier filtering, a self-referencing pinned struct) with no `//!` header. | `use std::{ffi::CString, path::Path, ptr::null_mut};` |
| P1-4 | src/audio.rs:1 | #29 | 404-line module implementing a three-stage handoff (`IncompleteAudioState` -> `AudioHandle` -> `AudioState` on a spawned thread) that no `//!` header explains; the ordering constraint is only discoverable by reading `finish`. | `use std::{` |
| P1-5 | src/main.rs:1789 | #23 | `EncState::new` is 220 lines: muxer open, codec query, hw device open, two frame contexts, filter graph, encoder params, a 3-arm `low_power` match whose Auto arm re-runs the whole 8-argument `make_video_params` on failure, then audio, header write and history state. | `fn new(` |
| P1-6 | src/main.rs:2043 | #23 | `on_encoded_packet` nests `match history_state` -> `while let Some(front)` -> `if let Some((key_idx, _))` -> `if current_history_size > *history_dur` -> a manual index-mutating `while i < final_idx` removal loop, with two `break`s whose reason lives only in trailing comments. | `fn on_encoded_packet(&mut self, mut encoded: Packet) {` |
| P1-7 | src/main.rs:1233 | #23 | `start_if_output_probe_complete` is a 4-arm match over `(geometry, output)` where three arms embed a search closure, an `eprintln!` with an inline join, `self.errored = true` and `return`; the error-exit protocol is repeated five times in one function. | `let (output, roi) = match (self.args.geometry, self.args.output.as_str()) {` |
| P1-8 | src/main.rs:1447 | #23 | `negotiate_format` chains a 3-branch device pick, a nested `fn negotiate_format_impl` (its own loop plus `if let`), a match on the `mem::replace`d stage with a nested result match per arm, and a closing 4-arm match on `in_flight_surface`. | `fn negotiate_format(` |
| P1-9 | src/main.rs:2331 | #23 | `execute` is a labeled `'outer` poll loop containing a match on poll result, a `for` over events, a match on token, a nested `for` over pending signals, a nested match on signal with `break 'outer`, and an `if let Err(...)`/`if`/`else` on the wayland read. | `let exit_code = 'outer: loop {` |
| P1-10 | src/main.rs:2234 | #23 | `main` runs eight sequential `if` guards (six of them with `&&`) that are pure argument validation, then three matches, before any work happens. The validation is a `fn validate(args: &Args)` that was never extracted. | `if !args.audio && args.audio_backend != DEFAULT_AUDIO_BACKEND {` |
| P1-11 | src/avhw.rs:107 | #23 | `create_frame_ctx` is one 138-line `unsafe` block whose body is an `if self.fmt == Pixel::VULKAN` whose then-branch is an entire `cfg`-gated Vulkan setup (two nested matches, an early `return Err`, four `transmute`s) and whose else-branch is a `let ... else` chain, all before a shared `if sts != 0`. | `pub fn create_frame_ctx(` |
| P1-12 | src/avhw.rs:277 | #23 | `vk_filter_drm_modifiers` nests `for modifier` -> `match get_..._properties2` -> `if max_extent < ...` (with `\|\|`) -> a `cfg`-gated inner `for m in &drm_modifier_props` -> `if ... && ...` -> `continue 'outer`, i.e. a labeled jump out of four levels. | `'outer: for modifier in in_modifiers {` |
| P1-13 | src/main.rs:1696 | #23 | `get_encoder` is a four-deep `if let`/`else` pyramid (`args.hw` -> `hw_codec_id` -> `find_by_name` -> `Some/None`) that resolves to `Option<Codec>` and is then re-matched twice more; the whole thing computes "pick a codec, warn if the preferred one is missing". | `let maybe_hw_codec = if args.hw {` |
| P1-14 | src/main.rs:1047 | #18 | `on_new_capture_format` narrates its phases in prose instead of splitting: `// destroy old frames` (1047), `// flush old filter & encoder` (1088), `// create a new encoder` (1103). Each label is a function boundary. | `// destroy old frames` |
| P1-15 | src/main.rs:2189 | #18 | `trigger_history` narrates four phases inside one `if let`: `// write history to container` (2189), `// find minumum PTS offset ...` (2191), `// grab this before we set history_state` (2207), `// transition history state` (2211). | `// write history to container` |
| P1-16 | src/filter.rs:39 | #18 | `video_filter` labels its phases `// src` (39), `// sink` (67) and `// sanity check` (110), each already bracketed by a bare block, which is the extract-function refactor spelled out but not taken. | `// src` |
| P1-17 | src/main.rs:918 | #11 | The block "bind the `WlOutput`, request its xdg-output for the callbacks, insert a 9-field all-`None` `PartialOutputInfo`" is written twice verbatim: `State::new` (911-931) and `OutputWentAwayState::new_wl_output` (690-706). The nine field initializers drift as a unit. | `partial_outputs.insert(` |
| P1-18 | src/main.rs:1401 | #11 | The `InFlightSurface::CopyQueued` teardown (`drop(av_mapping); cap.on_done_with_frame(wl_frame); wl_buffer.destroy();` inside an `if let ... else { panic!(...) }`) is duplicated in `on_copy_complete` (1354-1367) and `on_copy_fail` (1401-1414); only the last statement differs. | `if let InFlightSurface::CopyQueued {` |
| P1-19 | src/main.rs:1885 | #11 | The three-arm `EncodePixelFormat -> Pixel` unwrap is copied three times with the arms reordered: main.rs:1885-1889, main.rs:1080-1084, filter.rs:150-154. It is an inherent method (`fn sw(self) -> Pixel`) that was never written. | `let enc_pixfmt_av = match enc_pixfmt {` |
| P1-20 | src/main.rs:1896 | #11 | The 8-argument `make_video_params(args, enc_pixfmt, &encoder, (w, h), refresh, global_header, &mut hw_device_ctx, &mut frames_yuv)` call is written three times: 1896-1905, 1939-1948 (the low-power retry) and 1119-1128. The `#[allow(clippy::too_many_arguments)]` at 1627 is the symptom. | `let mut enc = make_video_params(` |
| P1-21 | src/main.rs:1866 | #11 | `EncState::new` (1866-1905) and `on_new_capture_format` (1075-1128) are the same five-step "build the encode pipeline" sequence (capture frame ctx, pixfmt unwrap, encode frame ctx, `make_video_params`, `video_filter`), differing only in field-path prefixes. A format change and a first start are one operation written twice. | `let mut frames_rgb = hw_device_ctx` |
| P1-22 | src/cap_ext_image_copy.rs:25 | #11 | Ten byte-identical no-op `Dispatch::event` bodies (this file at 25/37/48, cap_wlr_screencopy.rs:25, main.rs:710/766/778/859/871/2222) each spell out six underscore parameters. One `macro_rules! ignore_events` would hold all of them. | `fn event(` |
| P1-23 | tests/cmdline.rs:116 | #11 | `basic_vulkan` is `basic` (88-112) copied whole with one extra `.arg("--experimental-vulkan")` and a different temp filename; both assertion pairs and the wait block are identical. | `fn basic_vulkan() {` |
| P1-24 | tests/cmdline.rs:50 | #11 | `let wait_start = Instant::now(); cmd.wait().unwrap(); assert!(wait_start.elapsed() < Duration::from_secs(1));` appears four times (50, 80, 104, 133). The one-second shutdown budget lives in four places. | `let wait_start = Instant::now();` |
| P1-25 | src/main.rs:2151 | #11 | `.get("in").unwrap().source().flush().unwrap()` on a filter graph is written three times (main.rs:2151-2156, main.rs:1089-1095, audio.rs:136-141), and the mirrored `.get("out").unwrap().sink().frame(&mut f).is_ok()` drain loop twice (main.rs:2013-2019, audio.rs:99-106). | `self.video_filter` |
| P1-26 | Cargo.toml:44 | #34 | A commented-out dependency override block ships in the manifest: `# Uncomment for local dev` plus two path-dependency lines. Git holds the old values; the block only tells a reader that two published deps are sometimes not the ones built. | `# Uncomment for local dev` |
| P1-27 | src/audio.rs:319 | #34 | Two commented-out struct initializers sit inside a live `AudioState { ... }` literal: `// fifo: None,` (319) and `// audio_input,` (321). Both fields exist under other names, so the comments contradict the code beside them. | `// fifo: None,` |
| P1-28 | src/audio.rs:346 | #34 | A commented-out parameter `// input: &ffmpeg::Stream,` is left as the first line of `audio_filter`'s signature, above the live `input: &decoder::Audio`. | `// input: &ffmpeg::Stream,` |
| P1-29 | src/audio.rs:398 | #34 | A commented-out expression `// avchannelformat_to_string(avchannelformat_from_bits(codec_channel_layout.bits())),` is parked inside the live `format!` argument list, naming two functions that do not exist in the repo. | `// avchannelformat_to_string(avchannelformat_from_bits(codec_channel_layout.bits())),` |
| P1-30 | src/avhw.rs:355 | #36 | `#[allow(dead_code)]` on `get_drm_format_modifier_properties`, which is called at avhw.rs:288. The suppression is stale and now hides any future real deadness in that function. | `#[allow(dead_code)]` |
| P1-31 | src/main.rs:1849 | #36 | `#[allow(unreachable_code)]` guards a block containing an `info!` and a fallible `AvHwDevCtx::new_vulkan` call, none of it unreachable. The `error!` above it returns normally, so the lint has nothing to fire on and the allow silently covers whatever is edited in later. | `#[allow(unreachable_code)]` |
| P1-32 | src/main.rs:488 | #37 | `TypedObjectId<T>` is generic with a `PhantomData<T>`, plus generic `new` and `Debug` impls, but every one of its 12 uses is `TypedObjectId<WlOutput>`. The type parameter is a knob no caller turns. | `struct TypedObjectId<T>(ObjectId, PhantomData<T>);` |
| P1-33 | src/avhw.rs:24 | #37 | The `Usage` enum is threaded through `create_frame_ctx`'s public signature but is read only inside `#[cfg(feature = "experimental-vulkan")]`; in a default build the parameter is literally named `_usage` (avhw.rs:113). Both call sites pass a constant. | `pub enum Usage {` |
| P1-34 | src/audio.rs:286 | #37 | `IncompleteAudioState::finish(self, _args: &Args, octx: &...)` never reads `_args`; both call sites (main.rs:1977) pass it anyway. A parameter kept for a flexibility nobody exercises. | `pub fn finish(self, _args: &Args, octx: &format::context::Output) -> AudioHandle {` |
| P1-35 | src/cap_wlr_screencopy.rs:128 | #38 | The sentence "your compositor does not support zwp-linux-dmabuf and therefore is not support by wl-screenrec. See the README for supported compositors" is duplicated verbatim in main.rs:900; the same template with a different protocol name appears at cap_wlr_screencopy.rs:124 and main.rs:906. All four carry the same "is not support by" grammar bug, which is what copies drifting from one home looks like. | `.context("your compositor does not support zwp-linux-dmabuf and therefore is not support by wl-screenrec. See the README for supported compositors")?;` |
| P1-36 | src/main.rs:638 | #38 | The panic string "unwrap on non-complete EncConstructionStage" is written twice (631 and 638). The 638 copy is already wrong: `take_enc` accepts `OutputWentAway` as well, so the message misnames the state it rejects. | `_ => panic!("unwrap on non-complete EncConstructionStage"),` |
| P1-37 | tests/cmdline.rs:41 | #47 | `sleep(Duration::from_secs(10))` is the entire "record enough history" step of `history_clip_length`; nothing observes the recorder's readiness, so on a loaded runner the assertion window `(8s, 8.5s)` at 58-59 fails for timing reasons alone. | `sleep(Duration::from_secs(10));` |
| P1-38 | tests/cmdline.rs:46 | #47 | `sleep(Duration::from_secs(6))` after `SIGUSR1` is the only synchronization that the history flush happened; the test then asserts a 0.5s-wide duration band derived from this sleep. | `sleep(Duration::from_secs(6));` |
| P1-39 | tests/cmdline.rs:76 | #47 | `sleep(Duration::from_secs(5))` in `scale` waits for a recording whose only assertion is the output resolution; five seconds buys nothing the first encoded frame would not. | `sleep(Duration::from_secs(5));` |
| P1-40 | tests/cmdline.rs:100 | #47 | `sleep(Duration::from_secs(3))` in `basic`, asserted against `dur` in `(2.5s, 3.5s)`: the sleep is both the fixture and the oracle, so scheduler jitter is indistinguishable from a recorder bug. | `sleep(Duration::from_secs(3));` |
| P1-41 | tests/cmdline.rs:129 | #47 | `sleep(Duration::from_secs(3))` in `basic_vulkan`, same shape. The suite's four tests spend 27 wall-clock seconds sleeping. | `sleep(Duration::from_secs(3));` |
| P1-42 | src/main.rs:761 | none | `_ => todo!()` is the catch-all arm of the live `Dispatch<WlRegistry, GlobalListContents>` handler. `wl_registry`'s event enum is `#[non_exhaustive]`, so a compositor sending any future registry event panics a running recording and loses the file. `unreachable!` is not right either; ignore-and-log is. | `_ => todo!(),` |
| P1-43 | src/cap_ext_image_copy.rs:154 | none | Same shape on the frame path: `_ => todo!()` closes the `ExtImageCopyCaptureFrameV1` event match, so an unrecognized frame event from a newer compositor aborts mid-capture. Note the sibling handler at cap_wlr_screencopy.rs:83 chose `_ => {}` for the identical situation. | `_ => todo!(),` |
| P1-44 | src/fifo.rs:12 | none | `AudioFifo` is an RAII wrapper (`NonNull`, `unsafe impl Send`, allocation checked) with no `Drop`: `av_audio_fifo_free` is never called, so every non-variable-frame-size audio encode leaks the FIFO. Both other ffmpeg wrappers in the tree (`AvHwDevCtx`, `AvHwFrameCtx`) do implement `Drop`. | `pub struct AudioFifo(NonNull<AVAudioFifo>);` |
| P1-45 | src/fifo.rs:50 | none | `pop` discards `av_audio_fifo_read`'s return value, which is negative on error and the sample count otherwise. A short read silently hands the encoder a partly uninitialized frame; `push` at least returns its count. | `av_audio_fifo_read(` |
| P1-46 | src/audio.rs:221 | none | `format::open_with(&args.audio_device, ...).unwrap()` panics with a bare ffmpeg error code when `--audio-device` names a device that does not exist. Twelve lines above, the same function `bail!`s cleanly for an unknown `--audio-backend`, and it returns `anyhow::Result`, so the recovery path exists and is not used. | `.unwrap()` |
| P1-47 | src/audio.rs:129 | none | `fn fifo(&mut self) -> Option<&mut AudioFifo>` just re-wraps the field, and all three call sites (108, 109, 115) sit inside `if self.fifo.is_some()` and immediately `.unwrap()` it. Binding `let fifo = self.fifo.as_mut()?` once in `pop_from_filter` removes the method and the three unwraps. | `fn fifo(&mut self) -> Option<&mut AudioFifo> {` |
| P1-48 | src/main.rs:1795 | none | Parameter is spelled `history_alreday_triggered`, and the misspelling is propagated to the local at 1984. It is a named part of `EncState::new`'s signature, so every caller and every grep has to reproduce the typo. | `history_alreday_triggered: bool,` |
| P1-49 | tests/cmdline.rs:30 | none | `dbg!(wl_screenrec())` is left wrapping the binary path in four tests (30, 66, 91, 119). `dbg!` is a debugging aid, not a test fixture; it prints file and line noise on every run. | `let mut cmd = Command::new(dbg!(wl_screenrec()))` |

## Phase 2 - audit finding verdicts

Sheet: `corpus-ext/sheets/wl-screenrec.rs1.wave1.tsv` (21 findings, one row each,
verdict and why filled there; keys untouched).

| rule | rows | real | fp |
|------|------|------|----|
| rs:11 structural-clones (clone-block) | 2 | 2 | 0 |
| rs:23 cognitive-complexity | 6 | 6 | 0 |
| rs:27 purchase-price (price) | 1 | 1 | 0 |
| rs:29 top-loading | 7 | 7 | 0 |
| rs:47 sleepy-test | 5 | 5 | 0 |
| **total** | **21** | **21** | **0** |

Every finding landed on a site this judge had independently listed in phase 1, or
on an obvious extension of one. The single clone group the checker found is the
better catch of the two of us: it anchors on `EncState::flush` / `AudioState::flush`
as whole 5-statement blocks, where phase 1 (P1-25) had only named the shared
`.get("in").unwrap().source().flush().unwrap()` idiom. Judging it at the site turned
up a live consequence of the duplication that phase 1 missed: `EncState::flush`
calls `send_eof` unguarded at main.rs:2158, while the third copy of the same
sequence at main.rs:1097 guards it with `enc_video_has_been_fed_any_frames` against
the ffmpeg pre-first-frame crash its own comment documents. A recorder killed
before its first encoded frame takes the unguarded path.

Two rows are worth calling out as judged-real-at-the-edge rather than comfortable:
`rs:23` on `vk_filter_drm_modifiers` scores exactly at the threshold (15 of 15), and
`rs:29` on `cap_wlr_screencopy.rs` is a 164-line file with one top-level item. Both
survive because of repo-specific facts (a labeled `continue` escaping four levels;
two interchangeable capture backends whose files are indistinguishable on their
first screen), not because the generic claim is strong. A crate with a different
shape could make either of them an fp.

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| (none) | no false positives in this audit | 0 | n/a |

Counts sum to each rule's fp total, which is 0 for all five rules that fired.

## Phase 3 - reconciliation

49 phase-1 sites: 16 covered, 33 missed (11 detector-miss, 14 threshold-miss,
8 inventory-gap).

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #27 | covered | `src/main.rs:1:27:price:wl_screenrec` |
| P1-2 | #29 | covered | `src/main.rs:1:29:top-loading:wl_screenrec` |
| P1-3 | #29 | covered | `src/avhw.rs:1:29:top-loading:wl_screenrec::avhw` |
| P1-4 | #29 | covered | `src/audio.rs:1:29:top-loading:wl_screenrec::audio` |
| P1-5 | #23 | covered | cc 29 |
| P1-6 | #23 | covered | cc 25 |
| P1-7 | #23 | threshold-miss | `start_if_output_probe_complete` scored under 15 while six siblings cleared it. Its cost is five copies of the `eprintln!` + `self.errored = true` + `return` exit protocol across four match arms, which cognitive complexity prices as ordinary branching. |
| P1-8 | #23 | threshold-miss | `negotiate_format` scored under 15. A nested `fn negotiate_format_impl` carries a loop and an `if let` out of the parent's score, so hoisting complexity into an inner fn lowers the number without lowering the reading cost. |
| P1-9 | #23 | covered | cc 34, the crate's highest |
| P1-10 | #23 | covered | cc 20 |
| P1-11 | #23 | threshold-miss | `avhw::create_frame_ctx` scored under 15 despite 138 lines in one `unsafe` block. Its sibling `vk_filter_drm_modifiers` fired, so cfg-gated code is parsed; this one just distributes its branching flatly. |
| P1-12 | #23 | covered | cc 15, exactly at the bar |
| P1-13 | #23 | covered | cc 21 |
| P1-14 | #18 | detector-miss | rs:18 fired nowhere in the crate. `on_new_capture_format` narrates three phases (`// destroy old frames` 1047, `// flush old filter & encoder` 1088, `// create a new encoder` 1103) in one 110-line fn. The labels are bare imperatives with no numbering or "step"/"phase" keyword, which is the likely gap. |
| P1-15 | #18 | detector-miss | Same shape in `trigger_history`: four unnumbered phase labels (2189, 2191, 2207, 2211) inside one `if let`. |
| P1-16 | #18 | detector-miss | `video_filter` labels `// src` (39) and `// sink` (67), each already bracketed by a bare block, plus `// sanity check` (110). Single-word labels above a block are the cheapest possible signal and were not read. |
| P1-17 | #11 | threshold-miss | The duplicated bind/get_xdg_output/insert of a 9-field `PartialOutputInfo` (main.rs:911-931 vs 690-706) is 3 statements, under the arm's >=5 cutoff. Statement count underprices it: the risk lives in the nine field initializers inside one `insert` call, not in the statement count. |
| P1-18 | #11 | threshold-miss | The `InFlightSurface::CopyQueued` teardown (main.rs:1354-1367 vs 1401-1414) is a 4-statement `if let` body, one under the cutoff. |
| P1-19 | #11 | detector-miss | The three-arm `EncodePixelFormat -> Pixel` match is copied three times (main.rs:1885, main.rs:1080, filter.rs:150) as a single `let` binding each. Neither rs arm models a repeated *expression*; the py rule has a repeated-attribute-walk arm and the rs rule has no counterpart. |
| P1-20 | #11 | detector-miss | Same expression-level shape: the 8-argument `make_video_params` call written three times (1896, 1939, 1119). The `#[allow(clippy::too_many_arguments)]` at 1627 is a second, independent marker of it. |
| P1-21 | #11 | detector-miss | This one should have fired: `EncState::new` (1866-1905) and `on_new_capture_format` (1075-1128) are the same five-step pipeline build (capture frame ctx, pixfmt unwrap, encode frame ctx, `make_video_params`, `video_filter`), well over the 5-statement bar. The copies differ only by a `cs.enc.` field-path prefix on every receiver and by interleaved unrelated statements, so either receiver-path normalization or contiguity is what dropped it. |
| P1-22 | #11 | threshold-miss | Ten byte-identical empty `Dispatch::event` bodies (cap_ext_image_copy.rs:25/37/48, cap_wlr_screencopy.rs:25, main.rs:710/766/778/859/871/2222) have zero statements, so both arms skip them. Correct as a cutoff, but ten copies of a six-parameter signature is real duplication a macro would erase. |
| P1-23 | #11 | detector-miss | `basic_vulkan` (tests/cmdline.rs:116-141) is a whole-function T2 copy of `basic` (88-112) with one extra `.arg`, exactly the fn-body arm's shape. It was not read because family B scopes to prod: no rs:11, rs:23 or rs:29 finding touches `tests/`. A whole-function clone in a test suite is the same defect as one in prod. |
| P1-24 | #11 | threshold-miss | The `wait_start` / `wait()` / `assert!(elapsed < 1s)` block repeats four times (50, 80, 104, 133) but is 3 statements, so the cutoff binds even before the prod-only scope noted on P1-23 does. |
| P1-25 | #11 | covered | `src/main.rs:2150` + `src/audio.rs:135`; the checker's block anchoring is broader than the idiom phase 1 named. |
| P1-26 | #34 | detector-miss | The commented-out dependency block (Cargo.toml:44-46) is out of the arm's reach: it reads runs of >=3 non-doc comment lines that *Rust* parses, and this is TOML. Commented-out config is the same defect as commented-out code and the crate's only >=3-line instance of it. |
| P1-27 | #34 | threshold-miss | `// fifo: None,` (319) and `// audio_input,` (321) are commented-out initializers inside a live struct literal, but they are two 1-line runs separated by a live line, under the >=3-line run cutoff. Contiguity is the wrong proxy here: a commented field inside a struct literal is unambiguous. |
| P1-28 | #34 | threshold-miss | `// input: &ffmpeg::Stream,` (audio.rs:346) is a 1-line run. |
| P1-29 | #34 | threshold-miss | `// avchannelformat_to_string(...)` (audio.rs:398) is a 1-line run inside a live `format!` argument list, naming two functions that do not exist in the repo. |
| P1-30 | #36 | threshold-miss | rs:36 prices per-module `#[allow]` *density*; avhw.rs has 2 allows in 465 lines, far under any bar. The defect here is staleness, not density: `#[allow(dead_code)]` at 355 sits on a function called at 288, so the lint it silences cannot fire and the allow now only hides future deadness. |
| P1-31 | #36 | threshold-miss | Same density cutoff, same staleness shape: `#[allow(unreachable_code)]` at main.rs:1849 guards a block with nothing unreachable in it. A "this allow cannot fire" arm would catch both at zero density. |
| P1-32 | #37 | detector-miss | rs:37's only arm is a non-public trait with one impl. `TypedObjectId<T>` (main.rs:488) is a generic type with a `PhantomData<T>` and generic `new`/`Debug` impls whose twelve uses are all `TypedObjectId<WlOutput>` - a monomorphic type parameter, which the rs rule does not model though py's monomorphic arm does. |
| P1-33 | #37 | detector-miss | `Usage` (avhw.rs:24) is threaded through a public signature but read only under `#[cfg(feature = "experimental-vulkan")]`; in a default build the parameter is spelled `_usage` (avhw.rs:113) and both call sites pass a constant. |
| P1-34 | #37 | detector-miss | `IncompleteAudioState::finish(self, _args: &Args, ...)` (audio.rs:286) never reads `_args`. py has an unused-default arm; rs has no unused-parameter arm. |
| P1-35 | #38 | threshold-miss | The "your compositor does not support ... is not support by wl-screenrec" sentence is duplicated verbatim across two modules (cap_wlr_screencopy.rs:128, main.rs:900) with two more copies of the template at cap_wlr_screencopy.rs:124 and main.rs:906. It misses on both counts: inline `.context(...)` arguments rather than module-level declarations, and 2 modules rather than 3. The shared grammar bug in all four copies is the drift the rule exists to price. |
| P1-36 | #38 | threshold-miss | `"unwrap on non-complete EncConstructionStage"` appears twice (main.rs:631, 638), inline and within one module. The 638 copy is already wrong: `take_enc` accepts `OutputWentAway` too. |
| P1-37 | #47 | covered | `tests/cmdline.rs:41` |
| P1-38 | #47 | covered | `tests/cmdline.rs:46` |
| P1-39 | #47 | covered | `tests/cmdline.rs:76` |
| P1-40 | #47 | covered | `tests/cmdline.rs:100` |
| P1-41 | #47 | covered | `tests/cmdline.rs:129` |
| P1-42 | none | inventory-gap | `_ => todo!()` as the catch-all of a live `#[non_exhaustive]` protocol event match (main.rs:761) panics a running recording and loses the file. No listed rule covers a panic-on-unknown-input arm. |
| P1-43 | none | inventory-gap | Same shape on the frame path (cap_ext_image_copy.rs:154), where the sibling handler at cap_wlr_screencopy.rs:83 chose `_ => {}` for the identical situation. |
| P1-44 | none | inventory-gap | `AudioFifo` (fifo.rs:12) is an RAII wrapper with no `Drop`, so `av_audio_fifo_free` is never called, while both sibling ffmpeg wrappers in the crate do implement `Drop`. A missing-`Drop`-among-siblings check has no rule. |
| P1-45 | none | inventory-gap | `pop` (fifo.rs:50) discards `av_audio_fifo_read`'s return, which is negative on error; `push` returns its count. |
| P1-46 | none | inventory-gap | `format::open_with(...).unwrap()` (audio.rs:221) panics on a bad `--audio-device` inside a fn that returns `anyhow::Result` and `bail!`s cleanly twelve lines above. |
| P1-47 | none | inventory-gap | `fn fifo(&mut self)` (audio.rs:129) re-wraps a field that all three call sites unwrap immediately after an `is_some()` guard. |
| P1-48 | none | inventory-gap | `history_alreday_triggered` (main.rs:1795) is a misspelling in a signature, propagated to the local at 1984. |
| P1-49 | none | inventory-gap | `dbg!(wl_screenrec())` left in four tests (tests/cmdline.rs:30, 66, 91, 119). |

### What the reconciliation says

The checker's precision on this repo is 21/21, and its recall against a blind list
is 16/49. Read by rule rather than by row, that splits three ways:

- **Rules that fired did their whole job.** rs:23, rs:27, rs:29 and rs:47 covered
  every site phase 1 raised for them but three cognitive-complexity functions that
  fell under the bar, and rs:29 found four modules phase 1 had let pass. No tuning
  is owed here.
- **rs:11 is the one rule with real headroom.** Nine phase-1 clone sites, one
  covered. Two shapes account for seven of the eight misses: repeated
  *expressions* (a copied match, a copied 8-argument call), which no rs arm models
  though the py rule has an arm for exactly that; and blocks of 3 to 4 statements,
  just under the cutoff. P1-21 is the one that should have fired on the current
  arms and did not, so it is the single most informative row for a rule author.
- **The two silent rules are silent for structural reasons, not because the repo
  is clean.** rs:18 has three good sites and fired nowhere, all three using bare
  unnumbered labels. rs:34, rs:36, rs:37 and rs:38 each have live sites that miss
  by a definitional edge (TOML not Rust, 1-line runs not 3, density not staleness,
  monomorphic generics not single-impl traits, inline literals not module-level
  ones). Four of those five edges are cutoffs, which makes them cheap to test
  against and dangerous to widen without a fresh precision round.

The eight inventory-gap rows cluster: five of them (P1-42 through P1-46) are the
same underlying posture, code that converts an expected runtime condition into a
panic on a path that already has an error channel. That is a coherent rule the
inventory does not have.
