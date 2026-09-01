# log4rs (Rust) - judge report, wave 2

Repo: `../gauntlet-corpus/log4rs` (log4rs 1.4.0, edition 2021, rust-version 1.82).
Prod tree read: all 29 files under `src/`. Test tree read: `tests/`,
`benches/`, `examples/`, every `#[cfg(test)] mod`.

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/encode/pattern/mod.rs:262 | #11 | The `"h"\|"highlight"` arm (262-278), `"D"\|"debug"` (279-295), `"R"\|"release"` (296-312) and `""` (343-359) are four byte-identical 5-statement blocks differing only in the `FormattedChunk` variant they wrap. One helper taking the variant constructor removes three copies. | `if formatter.args.len() != 1 {` / `return Chunk::Error("expected exactly one argument".to_owned());` |
| P1-2 | src/encode/pattern/mod.rs:206 | #23 | `impl From<Piece> for Chunk::from` runs 206-364 as a single expression: an outer 3-arm match, an inner 14-arm match on `formatter.name`, and inside the `"d"` arm two more nested matches over a `for` loop. Well past the 15 bar and the hardest function in the crate to read. | `fn from(piece: Piece<'a>) -> Chunk {` |
| P1-3 | src/encode/pattern/mod.rs:1 | #27 | 1229 lines, 774 of them prod, is the largest module in the repo and holds `Chunk`, `FormattedChunk`, `StringBasedWriter`, `PatternEncoder` and the whole 140-line pattern grammar doc. Any task touching pattern encoding pays for all of it. | `//! A simple pattern-based encoder.` |
| P1-4 | src/encode/pattern/mod.rs:554 | #11 | In `kv_parsing`, the `key` block (554-567) and the `default` block (569-582) are the same 7-statement nested match, differing only in the two error strings and the `None` fallback. | `Piece::Text(key) => key.to_owned(),` / `Piece::Error(e) => return Err(e),` |
| P1-5 | src/encode/pattern/mod.rs:616 | #23 | `FormattedChunk::encode` (616-702) is a 20-arm match with three nested matches (`Line`, and two inside `Highlight`) and four `for` loops. | `fn encode(&self, w: &mut dyn encode::Write, record: &Record<'_>) -> io::Result<()> {` |
| P1-6 | src/append/rolling_file/policy/compound/trigger/time.rs:209 | #23 | `get_next_time` is seven sequential `if let TimeTriggerInterval::X(n) = interval` blocks, each with a nested `if modulate` and an early `return`, plus interleaved `let` hoists that only some branches use. A `match` on the enum collapses it. | `let increment = if modulate { n - year % n } else { n };` |
| P1-7 | src/append/rolling_file/policy/compound/trigger/time.rs:22 | #11 | `TimeTriggerConfig` is declared twice under complementary cfgs (22-31 and 36-43) with the same three fields and the same doc lines. The copies have already drifted: line 37's doc ends in a stray `Q` that line 23 does not have. | `/// The date/time interval between log file rolls.Q` |
| P1-8 | src/append/rolling_file/policy/compound/trigger/time.rs:279 | none | The seven `if let`s above cover all seven variants of `TimeTriggerInterval`, so this panic is unreachable by construction, yet every reader has to prove that. A `match` makes the compiler prove it and deletes the line. | `panic!("Should not reach here!");` |
| P1-9 | src/append/rolling_file/policy/compound/trigger/time.rs:139 | #11 | `TimeTriggerInterval`'s `visit_str` (135-191) is a structural copy of `size.rs`'s `visit_limit` `visit_str` (59-96): same split-on-first-non-digit, same parse-or-invalid-value, same `eq_ignore_ascii_case` else-if ladder, same trailing `match result`. | `let (number, unit) = match v.find(\|c: char\| !c.is_ascii_digit()) {` |
| P1-10 | src/append/rolling_file/policy/compound/mod.rs:32 | #11 | `struct Trigger` + its `Deserialize` impl (32-55) and `struct Roller` + its impl (59-82) are byte-identical apart from the type name. `struct Policy` in `rolling_file/mod.rs:57-81` is a third copy differing only in the `None` branch. | `let kind = match map.remove(&Value::String("kind".to_owned())) {` |
| P1-11 | src/filter/mod.rs:64 | #11 | `FilterConfig`'s hand-written `Deserialize` is the sixth copy of the "drain `kind` out of a `BTreeMap<Value, Value>`, keep the rest as `Value::Map`" body. Siblings: `append/mod.rs:129`, `encode/mod.rs:60`, `rolling_file/mod.rs:64`, `compound/mod.rs:38` and `compound/mod.rs:65`. | `let mut map = BTreeMap::<Value, Value>::deserialize(d)?;` |
| P1-12 | src/encode/writer/console.rs:127 | #11 | `#[cfg(unix)] mod imp` (127-229) and `#[cfg(target_family = "wasm")] mod imp` (232-320) are ~90 duplicated lines: identical `use` block, identical `Writer`/`WriterLock` newtypes, identical `io::Write` and `encode::Write` impls. Only the `ColorMode::Auto` arm of `stdout`/`stderr` differs (isatty check vs none). | `pub struct Writer(AnsiWriter<StdWriter>);` |
| P1-13 | src/priv_io.rs:28 | #11 | `impl io::Write for StdWriter` (28-56) and `impl io::Write for StdWriterLock` (63-91) are identical modulo the type name: the same four methods, each a two-arm match delegating to the inner writer. | `StdWriter::Stdout(ref mut w) => w.write(buf),` / `StdWriter::Stderr(ref mut w) => w.write(buf),` |
| P1-14 | src/encode/writer/simple.rs:13 | #11 | `impl io::Write for SimpleWriter` (13-29) is byte-identical to `ansi.rs:13-29`. The same four-method `write`/`flush`/`write_all`/`write_fmt` delegation block appears about twelve times across `simple.rs`, `ansi.rs`, `priv_io.rs`, `writer/console.rs` and `append/console.rs`; a `macro_rules!` or a single generic wrapper owns it once. | `fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {` / `self.0.write_all(buf)` |
| P1-15 | src/encode/writer/console.rs:15 | #20 | The closure `\|var\| var != "0"` appears three times in `color_mode` (lines 15, 18, 26), each wrapping an `env::var(..).map(..).unwrap_or(..)`. One named `env_flag(name, default)` helper replaces all three and makes the default asymmetry (false, false, true) explicit. | `let no_color = std::env::var("NO_COLOR")` / `.map(\|var\| var != "0")` |
| P1-16 | src/config/runtime.rs:206 | #11 | `RootBuilder::appender`/`appenders` (206-222) and `LoggerBuilder::appender`/`appenders` (357-373) are identical apart from the return type. `ConfigBuilder::appender`/`appenders`/`logger`/`loggers` (65-92) is the same push/extend shape a third time. | `self.appenders.extend(appenders.into_iter().map(Into::into));` / `self` |
| P1-17 | src/config/runtime.rs:463 | #32 | `ConfigError::__Extensible` is never constructed or matched anywhere in the repo. It is a hand-rolled non-exhaustive marker kept alive only by `#![allow(clippy::manual_non_exhaustive)]` at lib.rs:302, and `#[non_exhaustive]` has been stable since well before the pinned rust-version 1.82. Delete both. | `#[doc(hidden)]` / `#[error("Reserved for future use")]` / `__Extensible,` |
| P1-18 | src/lib.rs:580 | #11 | `handle_error`'s whole body is repeated inline at lib.rs:462 as the default error closure of `SharedLogger::new`, so the crate has two homes for its "how do we report a nonfatal error" decision. `new` should pass `Box::new(handle_error)`. | `let _ = writeln!(io::stderr(), "log4rs: {}", e);` |
| P1-19 | src/config/raw.rs:90 | #36 | Module-wide `#![allow(deprecated)]` with nothing deprecated in scope: the crate's only `#[deprecated]` item is `LogFile::len` (`rolling_file/mod.rs:124`), which raw.rs never names. raw.rs also carries five more `#[allow]`s (104, 177, 479, 528, 531), the highest allow density in `src/`. | `#![allow(deprecated)]` |
| P1-20 | src/append/console.rs:64 | #36 | The allow and its comment justify the hand-written match by "1.40 compat", but Cargo.toml pins `rust-version = "1.82"`. `matches!(self, Self::Tty(_))` is available and deletes both the allow and the match. | `// 1.40 compat` / `#[allow(clippy::match_like_matches_macro)]` |
| P1-21 | src/append/file.rs:44 | #36 | `#[allow(dead_code)]` claims the `path` field exists for debug only, but the struct's `derive_more::Debug` derive reads it (it carries no `#[debug(skip)]`, unlike the `file` field), so `dead_code` cannot fire and the allow blinds a future real one. | `#[allow(dead_code)] // reason = "debug purposes only"` / `path: PathBuf,` |
| P1-22 | src/config/file.rs:1 | #29 | 232 lines carrying the public `init_file`/`load_config_file` entry points, the format dispatch, and the whole background reload thread, with no `//!` header. It is the only file-config module and a reader has to infer that from the `use` block. | `use std::{` |
| P1-23 | src/encode/pattern/parser.rs:1 | #29 | 276 lines of the pattern grammar's hand-written lexer with no `//!` header. The one line of orientation it has is a plain `//` comment, so it does not reach rustdoc or a reader of the module page. | `// cribbed to a large extent from libfmt_macros` |
| P1-24 | src/config/file.rs:22 | #59 | `init_file`'s doc covers stderr reporting and the feature gate but never says that a non-empty `refresh_rate` spawns a permanent `"log4rs refresh"` OS thread (188-191) that polls the file forever and swaps the process-global logger config out from under every caller. The reader cannot walk that cost back. | `/// Any nonfatal errors encountered when processing the configuration are` / `/// reported to stderr.` |
| P1-25 | src/config/file.rs:141 | #48 | `read_config` is a two-line private wrapper that binds `fs::read_to_string(path)?` and re-wraps it in `Ok`. It adds no path handling, no error context and no default; its three call sites (28, 65, 216) should call `fs::read_to_string` directly. | `let s = fs::read_to_string(path)?;` / `Ok(s)` |
| P1-26 | src/append/file.rs:130 | #32 | `date_time_format` takes `&self` and never reads it: the body (131-157) touches only `path` and the module constants. The receiver is dead weight in the signature and forces the caller at line 109 to have a builder in hand. | `fn date_time_format(&self, path: PathBuf) -> PathBuf {` |
| P1-27 | src/append/rolling_file/mod.rs:125 | #11 | `LogFile::len` (125-127) and `LogFile::len_estimate` (135-137) have identical one-line bodies and byte-identical six-line doc comments (118-123 and 129-134). The deprecated one should delegate, and the doc should live once. | `pub fn len(&self) -> u64 { self.len }` |
| P1-28 | src/append/rolling_file/policy/compound/roll/fixed_window.rs:246 | none | A logging library writes to the process's stdout on the compression error path, unconditionally and unformattably, then still propagates the error. This is stray debug output: the error already reaches the configured `err_handler`. | `compression.compress(&file, &dst_0).inspect_err(\|e\| {` / `println!("err compressing: {:?}, dst: {:?}: {}", file, dst_0, e);` |
| P1-29 | src/append/rolling_file/policy/compound/roll/fixed_window.rs:369 | #47 | The `wait_for_roller` test helper sleeps a fixed 100 ms and is called nine times across six tests in this file, so the suite burns ~0.9 s of wall clock and is flaky by construction under load. The `cond_pair` lock it takes on the next line is the real synchronisation point; the sleep before it is the guess. | `std::thread::sleep(std::time::Duration::from_millis(100));` / `let _lock = roller.cond_pair.0.lock();` |
| P1-30 | benches/rotation.rs:32 | #47 | A fixed 5 ms sleep sits inside the 999-iteration measurement loop, so the benchmark spends ~5 s asleep and the anomaly threshold at line 142 is calibrated against that pacing rather than against the rotation work. | `for _ in 1..iters {` / `thread::sleep(Duration::from_millis(5));` |
| P1-31 | src/encode/writer/console.rs:524 | #42 | Test `basic` has no assertion: it returns early when stdout is not a console (which is the normal CI case, so it is usually a no-op), and otherwise only `unwrap`s writes. It passes whatever `set_style` emits. | `let w = match ConsoleWriter::stdout() {` / `Some(w) => w,` / `None => return,` |
| P1-32 | src/encode/writer/ansi.rs:90 | #42 | Test `basic` has no assertion. It writes escape sequences into the harness's real stdout instead of into a `Vec<u8>` it could compare against the expected `\x1b[0;31;44;1m` bytes, so no ANSI regression can fail it. | `let stdout = io::stdout();` / `let mut w = AnsiWriter(stdout.lock());` |
| P1-33 | src/append/file.rs:214 | #42 | Test `create_directories` has no assertion. Its name promises that `foo/bar/` was created; the body only `unwrap`s the builder, so the test still passes if `build` stops creating parents and merely opens a path that happens to exist. | `FileAppender::builder()` / `.build(tempdir.path().join("foo").join("bar").join("baz.log"))` / `.unwrap();` |
| P1-34 | src/append/file.rs:223 | #42 | Test `append_false` has no assertion. Nothing checks that `append(false)` truncated anything; compare `rolling_file/mod.rs:455` (`truncate`), which writes bytes, rebuilds and asserts the file is empty. | `.append(false)` / `.build(tempdir.path().join("foo.log"))` / `.unwrap();` |
| P1-35 | src/filter/threshold.rs:118 | #44 | `test_filter_new_vs_struct` asserts a one-field constructor against a struct literal of the same one field under a derived `PartialEq`. There is no behaviour between the two sides that could differ, so the assertion cannot fail. | `ThresholdFilter::new(LevelFilter::Info),` / `ThresholdFilter { level: LevelFilter::Info }` |
| P1-36 | examples/custom_config.rs:21 | none | Lines 21-120 are copied verbatim from `examples/custom.rs:26-125` (`MyFilter`, `MyEncoder`, `MyAppender` and all three impls); the file admits it at line 122. The copies have already drifted: the same panic reads `"Invalid log level"` here and `"Unexpected log level"` at custom.rs:120. This is #11's shape, but examples sit outside its scope. | `/// The code above is same as `examples/custom.rs`.` |
| P1-37 | examples/log_to_file_with_rolling_and_time_trigger.rs:1 | none | This example and `log_to_file_with_rolling_and_size_trigger.rs` are ~95% identical: same six consts, same imports modulo the trigger, same 60-line `main`. Only the trigger construction and `RUN_TIME` differ. Same scope caveat as P1-36. | `const TIME_BETWEEN_LOG_MESSAGES: Duration = Duration::from_millis(10);` |
| P1-38 | src/lib.rs:24 | none | Both intra-doc links point at `trigger/tine/`, a typo for `time` and `onstartup`, so the two links on the crate's front page are dead. | `//!         - [time](append/rolling_file/policy/compound/trigger/tine/struct.TimeTriggerDeserializer.html#configuration)` |
| P1-39 | src/encode/pattern/mod.rs:52 | none | The crate-facing formatter doc says highlight is "intense red for errors, red for warnings, blue for info", but `FormattedChunk::encode` (654-659) uses intense Red for Error, Yellow for Warn, Green for Info and Cyan for Trace. Three of the four documented colours are wrong and Trace is undocumented. | `//!   style is intense red for errors, red for warnings, blue for info, and` |
| P1-40 | src/append/file.rs:96 | none | The doc says `$ENV{name_here}` "will be replaced by `name_here`"; it is replaced by the environment variable's *value* (`append/mod.rs:74`). The wrong sentence is copied to three more places: `file.rs:169`, `rolling_file/mod.rs:265` and `rolling_file/mod.rs:307`. | `/// - `$ENV{name_here}`: This pattern will be replaced by `name_here`.` |
| P1-41 | src/append/rolling_file/policy/compound/trigger/onstartup.rs:63 | none | The deserializer's `# Configuration` block shows only `kind: onstartup` and a blank line. `min_size` is the sole field `OnStartUpTriggerConfig` has (line 18), it has a non-obvious default of 1, and it appears nowhere in the docs. | `/// kind: onstartup` / `///` / `/// ```` |
| P1-42 | src/config/runtime.rs:477 | none | The `check_logger_name` case table repeats `("asdf::jkl::", false)` at lines 477 and 480, so one of the eight rows tests nothing new and the reader has to diff them to notice. | `("asdf::jkl::", false),` / `("asdf:jkl", false),` |
| P1-43 | src/encode/pattern/parser.rs:206 | #23 | `Parser::next` (206-275) is a six-arm match on the peeked char where four arms each nest an if/else on `consume`, one nests a further if/else, and the `'\\'` arm nests a second six-arm match. | `fn next(&mut self) -> Option<Piece<'a>> {` |
| P1-44 | src/encode/pattern/parser.rs:222 | #11 | The `'}'` (222-229), `'('` (230-237) and `')'` (238-245) arms are three copies of the same double-char-escape block, and the five sub-arms of the `'\\'` arm (249-268) are five more copies of a two-line advance-and-return. | `self.it.next();` / `if self.consume('}') { Some(Piece::Text("}")) } else {` |

## Phase 2 - audit finding verdicts

Sheet: `corpus-ext/sheets/log4rs.rs2.wave1.tsv`, 5 rows (rs:6 x1, rs:37 x1,
rs:48 x3). Judged at the site; the full `why` for each row lives in the sheet.
Totals: 5 judged, 1 `real`, 4 `fp`.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| src/append/rolling_file/mod.rs:217 | #6 | indexed | real | `get_writer` opens the log path with `create(true).truncate(!self.append)` (219-224) and rebinds the caller's `Option`, so a call named like a getter can create or empty a file on disk. The message understates the effect as `reads-world`, but the site is the ideal's exact shape. |
| src/config/raw.rs:137 | #37 | indexed | fp | The single impl is a blanket `impl<T> ErasedDeserialize for DeserializeEraser<T> where T: Deserialize` (149-151), covering unboundedly many types including third-party ones registered via `Deserializers::insert`. The trait is the object-safe erasure of `Deserialize`'s associated `Config` type, required by the `Arc<dyn ErasedDeserialize<Trait = T>>` map at raw.rs:168; it cannot be removed. |
| src/append/mod.rs:33 | #48 | indexed | fp | One of a named sibling pair whose bodies differ by exactly one alternative; the name is what tells a reader the two call sites use different character classes, and the helper carries its own comment recording the regex it replaced. |
| src/append/mod.rs:39 | #48 | indexed | fp | Same sibling-pair exemption, and the fold is worse here: the call site is a match guard (mod.rs:62), so inlining yields a three-alternative guard. |
| src/lib.rs:437 | #48 | indexed | fp | Folding makes the caller reach through the wrapper into its private field (`appender.appender.flush()` at lib.rs:575) and deletes one half of the `append`/`flush` pair mirroring the `Append` trait on the same wrapper struct. |

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| #37 | blanket impl counted as one implementation (the impl is generic over a bound, not over one type) | 1 | `src/config/raw.rs:137:37:single-impl:log4rs::config::raw::ErasedDeserialize` |
| #48 | one-line predicate that is one of a named sibling set differing only in body, and carries its own comment | 2 | `src/append/mod.rs:33:48:fold:log4rs::append::env_util::is_env_var_start` |
| #48 | one-line delegation on a wrapper type whose inlined form dereferences a field of the receiver | 1 | `src/lib.rs:437:48:fold:log4rs::Appender::flush` |

Observed while judging, outside the judged set: `#42` at
`src/filter/threshold.rs:84` reads as an fp of the same kind the sheet does
not cover. `test_cfg_deserialize` verdicts through `serde_test`'s
`assert_de_tokens` / `assert_de_tokens_error` (103, 106, 111), which are
external assert helpers the arm does not recognise as a verdict.

## Phase 3 - reconciliation

Audit: 49 findings (#11 x30, #23 x8, #27 x2, #29 x2, #42 x2, #48 x3, #37 x1,
#6 x1). Zero findings under `tests/`, `benches/` or `examples/`.
Phase-1 sites: 44. `covered` 11, `detector-miss` 12, `threshold-miss` 12,
`inventory-gap` 9.

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #11 | threshold-miss | The four identical arms are 3 statements each (a guard plus two bindings); the clone-block arm fired on 5-statement blocks at raw.rs:180-228, so the site sits just under the block-size cutoff. |
| P1-2 | #23 | covered | #23 src/encode/pattern/mod.rs:206, cc 49. |
| P1-3 | #27 | threshold-miss | #27 fired on raw.rs (552 lines) and lib.rs (717) but not on the 1229-line pattern/mod.rs, the largest module in the repo: its symbols are not hot enough (`PatternEncoder` has few cross-module readers), so size alone never reaches the bar. |
| P1-4 | #11 | covered | Landed via #23 at pattern/mod.rs:549 (`kv_parsing`, cc 15) rather than #11; the finding names the same symbol and the same lines, and extracting the shared block is what resolves both. |
| P1-5 | #23 | covered | #23 src/encode/pattern/mod.rs:616, cc 28. |
| P1-6 | #23 | covered | #23 time.rs:209, cc 28. |
| P1-7 | #11 | detector-miss | A `struct` declared twice under complementary cfgs is a duplicated item declaration; #11's rs arms read `fn` bodies and statement blocks, so no arm can ever pair two type definitions even though the drift (the stray `Q` at time.rs:37) is exactly what the ideal warns about. |
| P1-8 | none | inventory-gap | Unreachable `panic!` closing an exhaustive if-let chain over an enum. |
| P1-9 | #11 | threshold-miss | The two `visit_str` bodies share their whole skeleton but differ in ladder arity (5 units vs 7) and in construction (`checked_mul` vs enum variants), so the blind digest cannot match them. |
| P1-10 | #11 | covered | #11 compound/mod.rs:39 and :66, group x3 with `FilterConfig::deserialize`. |
| P1-11 | #11 | covered | #11 src/filter/mod.rs:65, same x3 group. |
| P1-12 | #11 | detector-miss | The ~90 duplicated lines of the unix and wasm `mod imp` are invisible: the audit found the within-module `Writer::stdout`/`stderr` x2 pair at writer/console.rs:145 and :160, which are inside the unix branch only, so only the active cfg branch was read. cfg-gated sibling modules are the classic home of this duplication. |
| P1-13 | #11 | covered | #11 priv_io.rs:29, 36, 43, 50, 64, 71, 78, 85. |
| P1-14 | #11 | threshold-miss | The arm fired on the two-arm-match form of this delegation (console.rs, priv_io.rs) but on none of the single-expression form (`self.0.write(buf)`) in simple.rs, ansi.rs and writer/console.rs, which is a larger group. A one-expression body falls under the triviality floor. |
| P1-15 | #20 | threshold-miss | Three occurrences in one module meets the stated count bar, so the block is the nontriviality test: `var != "0"` is a single binary comparison. No #20 finding anywhere in the audit. |
| P1-16 | #11 | covered | #11 runtime.rs:215 and :366 (`RootBuilder::appenders` / `LoggerBuilder::appenders`). The singular `appender` halves at 206 and 357 went unpaired, the same one-expression floor as P1-14. |
| P1-17 | #32 | detector-miss | No #32 finding anywhere. `ConfigError::__Extensible` is an enum variant, not an item, so the closed-world item walk never enumerates it, and the deletion the ideal wants (variant plus the `#![allow(clippy::manual_non_exhaustive)]` that keeps it) is never proposed. |
| P1-18 | #11 | detector-miss | `handle_error`'s body is duplicated inside a closure at lib.rs:462. The arm reads `fn` bodies, so a fn-versus-closure pair is structurally unpairable regardless of size. |
| P1-19 | #36 | threshold-miss | No #36 finding anywhere. raw.rs carries 6 allows in 552 lines (~1.1%), the highest density in `src/`, still under the cutoff. |
| P1-20 | #36 | threshold-miss | A single stale allow in a 264-line module can never reach a density bar, yet the falsifier is local and cheap: the allow's own comment says "1.40 compat" against a manifest `rust-version = "1.82"`. |
| P1-21 | #36 | threshold-miss | Same shape as P1-20: one allow, and the falsifier is that the `derive_more::Debug` derive already reads the field the allow calls dead. |
| P1-22 | #29 | covered | #29 src/config/file.rs:1 (232 lines, 7 top-level items). |
| P1-23 | #29 | covered | #29 src/encode/pattern/parser.rs:1 (276 lines, 5 top-level items). |
| P1-24 | #59 | detector-miss | No #59 finding anywhere. `init_file` spawns a named OS thread that outlives the call and polls forever (file.rs:188-191) and swaps the process-global logger, with a doc that mentions neither: the ideal's exact case. |
| P1-25 | #48 | threshold-miss | `read_config` has three call sites (28, 65, 216) and the arm needs one. The wrapper is empty on any count, so call-site count is the wrong gate for an identity wrapper over a stdlib call. |
| P1-26 | #32 | detector-miss | The rs reading of #32 has no counterpart to the py record's dead-param arm, so an unread `&self` receiver has no detector at all. |
| P1-27 | #11 | threshold-miss | Both bodies are the single expression `self.len`; the same triviality floor as P1-14. The six-line doc comment duplicated verbatim above each is not read by any arm. |
| P1-28 | none | inventory-gap | `println!` to process stdout on an error path inside a logging library. |
| P1-29 | #47 | detector-miss | The 100 ms sleep is in `wait_for_roller`, a `#[cfg(test)]` helper with no `#[test]` attribute, called nine times from six tests. "Inside a test" misses one level of helper indirection, which is where sleeps get parked. |
| P1-30 | #47 | detector-miss | Zero findings anywhere under `benches/`, `examples/` or `tests/`, so the sleep in the benchmark's measurement loop is out of reach of every rule, not only #47. |
| P1-31 | #42 | detector-miss | The arm fired on raw.rs:523, an unwrap-only body, but not on this one, which is also unwrap-only (writer/console.rs is analysed: #11 fired there at 145, 160, 360, 378). |
| P1-32 | #42 | detector-miss | Zero findings of any rule in ansi.rs; `basic` is unwrap-only like raw.rs:523. |
| P1-33 | #42 | detector-miss | `create_directories` is a plain `.build(..).unwrap();`, the same shape as the raw.rs:523 hit. |
| P1-34 | #42 | detector-miss | `append_false`, same shape as P1-33. |
| P1-35 | #44 | threshold-miss | No #44 finding anywhere. The left side is `ThresholdFilter::new(..)`, a call, and the arm requires a call-free expression, so a constructor asserted against a literal of its own one field is out of scope by construction. |
| P1-36 | none | inventory-gap | ~100 verbatim lines shared by two examples, already drifted. #11's shape, but examples are outside its scope. |
| P1-37 | none | inventory-gap | Two ~95% identical example files. Same scope caveat. |
| P1-38 | none | inventory-gap | Two dead intra-doc links on the crate front page (`trigger/tine/`). |
| P1-39 | none | inventory-gap | Module doc names three highlight colours the code does not use. |
| P1-40 | none | inventory-gap | `$ENV{}` doc says "replaced by `name_here`" in four places; it is replaced by the value. |
| P1-41 | none | inventory-gap | The onstartup `# Configuration` block omits `min_size`, the config's only field. |
| P1-42 | none | inventory-gap | A duplicated row in the `check_logger_name` case table. |
| P1-43 | #23 | covered | #23 src/encode/pattern/parser.rs:206, cc 33. |
| P1-44 | #11 | threshold-miss | The `'}'`, `'('` and `')'` arms are 2 to 3 statements each, under the block arm's cutoff, and the five sub-arms of the `'\\'` arm are 2 statements each. |

### Findings the phase-1 list did not name

Nine of the 49 findings sit at sites I did not list: #23 at append/mod.rs:44
(`expand_env_vars`, cc 24) and runtime.rs:98 (`build_lossy`, cc 19), which I
read and let pass; #23 at pattern/mod.rs:419 (`chunk_end`, cc 15 exactly);
#11 at raw.rs:180-228 (the four cfg-gated `d.insert(..)` blocks in
`Deserializers::default`, a registration table I would not fold) and
raw.rs:345 / runtime.rs:437 (`AppenderErrors::handle` and
`ConfigErrors::handle`, a real x2 clone I missed); #11 at
writer/console.rs:360 and :378 (`RawConsole::set_style`'s foreground and
background blocks, a real x2 clone I missed); #42 at raw.rs:523 (`empty`,
correct, I missed it). The two `handle` clones and the two `set_style` blocks
are genuine sites my list should have carried.
