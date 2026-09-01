# bob (bob-nvim 4.1.7) - judge report

Repo: `../gauntlet-corpus/bob`. Rust bin crate (`src/main.rs`, no lib), 25
files, 5384 lines of `.rs`. Read cold; no audit output seen.

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/consts.rs:16 | #34 | Three-line commented-out `LazyLock` regex definition sitting directly under the live `VERSION_REGEX` it once replaced; parses as Rust, git already remembers it. | `// pub static VERSION_REGEX: LazyLock<Regex> = LazyLock::new(\|\| {` |
| P1-2 | src/handlers/rollback_handler.rs:135 | #34 | Commented-out `humanized_duration += &format!(...)` left beside the `write!` that replaced it, and again at :151 for the days block. | `// humanized_duration += &format!("{} week{}", weeks, if weeks > 1 { "s" } else { "" });` |
| P1-3 | src/main.rs:1 | #29 | No module has a `//!` header anywhere in the crate (`grep -rn '^//!' src` is empty); `main.rs` is the crate root and says nothing about what bob is or how the modules divide. | `mod cli;` |
| P1-4 | src/handlers/use_handler.rs:1 | #29 | 754-line module, the largest in the crate, opens straight into `use` lines: no `//!` saying it owns proxy install, PATH mutation and the env-script copies. | `use anyhow::{Result, anyhow};` |
| P1-5 | src/handlers/install_handler.rs:1 | #29 | 735 lines covering download, checksum, rollback and build-from-source with no `//!` header naming those four jobs. | `use crate::config::{Config, ConfigFile};` |
| P1-6 | src/helpers/version/mod.rs:1 | #29 | 373 lines of version parsing plus GitHub lookups, no `//!`. | `pub mod nightly;` |
| P1-7 | src/github_requests.rs:1 | #29 | 336 lines of GitHub API models and calls, no `//!`; the module is `pub` off the crate root and is the only network seam. | `use anyhow::{Result, anyhow};` |
| P1-8 | src/cli.rs:1 | #29 | 391 lines mixing clap definitions, tracing setup and the whole dispatch table, no `//!`. | `use crate::{` |
| P1-9 | src/handlers/use_handler.rs:48 | #27 | `use_handler::start` and `switch` are hot (called from cli, sync_handler, rollback_handler) but live in a 754-line file every caller's reader must ingest whole. | `pub async fn start(` |
| P1-10 | src/handlers/install_handler.rs:66 | #27 | `install_handler::start` is entered from cli, use_handler and update_handler, and sits in a 735-line file whose other 600 lines are build-from-source plumbing. | `pub async fn start(` |
| P1-11 | src/handlers/install_handler.rs:66 | #23 | 127-line function with ~17 decision points (four nesting levels at :146), self-declared unmanageable by its own `#[allow(clippy::too_many_lines)]` at :65. | `#[allow(clippy::too_many_lines)]` |
| P1-12 | src/handlers/install_handler.rs:514 | #23 | `handle_building_from_source` is 130 lines, ~15 decision points, a `cfg_if` at each end, an anonymous block at :584 used only to scope git calls, plus `#[rustfmt::skip]`. | `async fn handle_building_from_source(version: &ParsedVersion, config: &Config) -> Result<PostDownloadVersionType> {` |
| P1-13 | src/handlers/use_handler.rs:345 | #23 | `add_to_path` mixes four early returns, a RefCell dance, an interactive prompt with a 120s timeout, a three-arm match and two `cfg` returns in 64 lines. | `async fn add_to_path(installation_dir: PathBuf, config: ConfigFile) -> Result<()> {` |
| P1-14 | src/handlers/use_handler.rs:442 | #23 | non-Windows `modify_path` nests three fallible matches, a `map_or_else` whose both arms return `Ok(())`, and a loop that returns `Ok(())` on the first failure. | `async fn modify_path(config: &ConfigFile, installation_dir: &str) -> Result<()> {` |
| P1-15 | src/helpers/unarchive.rs:203 | #23 | unix `expand` is 79 lines with a nested match/if/if ladder inside the entry loop and an error arm that only `println!`s. | `fn expand(downloaded_file: &LocalVersion) -> Result<()> {` |
| P1-16 | src/handlers/install_handler.rs:558 | #18 | Five numbered-in-prose phases narrated inside one function: "create neovim-git", "check if repo is initialized" (:571), "check if repo has a remote" (:586), "fetch version from origin" (:598), "checkout fetched files" (:606). Each is a function boundary spelled as a comment. | `// create neovim-git if it does not exist` |
| P1-17 | src/handlers/run_handler.rs:24 | #18 | Four phase comments narrate `start`: parse, existence check (:29), binary path (:38), run (:52). | `// Parse the specified version` |
| P1-18 | src/handlers/use_handler.rs:615 | #18 | Three phase comments inside `copy_env_files_if_not_exist`: "Ensure the env directory exists", "Define the file paths" (:618), "Check if the files exist and write the content if they don't" (:622). | `// Ensure the env directory exists` |
| P1-19 | src/handlers/install_handler.rs:456 | #48 | `set_position` is a private one-line method with exactly one call site (:384) and no other reference; it costs a name and a hop to forward to `self.pb.set_position`. | `fn set_position(&self, position: u64) {` |
| P1-20 | src/handlers/install_handler.rs:419 | #37 | `PbWrapper<'a, S>`'s `S` is instantiated only as `String` (the single `new` call at :371 passes `&version.tag_name`), and the `impl<'a, S> PbWrapper<'_, S>` at :425 declares a lifetime the impl never uses. | `struct PbWrapper<'a, S> {` |
| P1-21 | src/handlers/install_handler.rs:442 | #37 | `finish<P: AsRef<Path>>` has one call site (:390) passing a `&Path`; the parameter buys nothing. | `fn finish<P>(&self, root: P, file_type: S)` |
| P1-22 | src/handlers/install_handler.rs:649 | #37 | `windows_deps<S: AsRef<OsStr>>` is called once (:632) with three `String`s; every use names the same type. | `async fn windows_deps<S>(build_arg: S, build_type: S, folder_name: S) -> Result<()>` |
| P1-23 | src/handlers/use_handler.rs:584 | #37 | `EnvPaths<F, S>` has exactly one instantiation, aliased at :600 as `EnvPathsBufs = EnvPaths<FishScriptPath<PathBuf>, ShScriptPath<PathBuf>>`, and one producer. Two type parameters, one argument each. | `struct EnvPaths<F, S> {` |
| P1-24 | src/handlers/use_handler.rs:558 | #37 | `FishScriptPath<F>` / `ShScriptPath<S>` are single-field newtypes over `PathBuf` whose only behaviour is a `Deref`; nothing distinguishes them at any use site (`.to_str()`, `.exists()`). | `struct FishScriptPath<F>(F);` |
| P1-25 | src/config.rs:179 | #37 | Private trait `EnvVarProcessor` has exactly one `impl` (`Option<String>`, :183) and one call site pattern (`handle_envars`'s four-element array); the "polymorphism" its doc claims has one implementation. | `trait EnvVarProcessor {` |
| P1-26 | src/handlers/use_handler.rs:565 | #11 | `Deref` impls for `FishScriptPath` (:565) and `ShScriptPath` (:574) are byte-identical modulo the type name. | `fn deref(&self) -> &Self::Target {` |
| P1-27 | src/github_requests.rs:208 | #11 | `get_upstream_nightly` (:208) and `get_upstream_stable` (:243) have identical bodies differing only in the URL literal; one function taking the path would do. | `let response = make_github_request(` |
| P1-28 | src/handlers/install_handler.rs:461 | #11 | `file_type_ext` and `send_request` (:716) each carry the same `Nightly \|\| semver > 0.10.4` decision producing "shasum.txt" vs `<ext>.sha256sum`; the two copies must be kept in lockstep by hand. | `if version.version_type == VersionType::Nightly` |
| P1-29 | src/handlers/install_handler.rs:522 | #11 | The clang probe (:522) and the gcc probe (:526) are the same three-line `match Command::new(..).output()` block with one string changed. | `let is_clang_present = match Command::new("clang").output().await {` |
| P1-30 | src/handlers/update_handler.rs:57 | #11 | The stable block (:57-64) and the nightly block (:66-74) are the same five statements with the version name and match-arm order permuted. | `if is_version_installed(&stable.tag_name, &config.config).await? {` |
| P1-31 | src/handlers/rollback_handler.rs:131 | #11 | `humanize_duration` repeats the same separator-then-`write!` block three times for weeks (:131), days (:144) and hours (:160); a table over `[(weeks,"week"),(days,"day"),(hours,"hour")]` is one copy. | `if weeks != 0 {` |
| P1-32 | src/handlers/list_handler.rs:193 | #11 | `test_with_v_semvar` (:193), `test_as_stable` (:203), `test_with_nightly_and_date` (:213), `test_with_invalid_version` (:223) and `test_with_empty_string` (:245) are the same three-line body with one literal changed, and all five cases already appear in the `test_is_version` table at :172. | `let version = "v1.2.3";` |
| P1-33 | src/helpers/version/mod.rs:339 | #11 | `test_is_hash_with_valid_hash` (:339) through `test_is_hash_with_long_hash` (:369) are six two-line clones of each other, every case of which is already in the `version_expected` table at :316. | `let version = "abc123";` |
| P1-34 | src/handlers/list_handler.rs:187 | #11 | `test_is_version` walks the same table twice, once through a `match` on the expected bool and once through `assert_eq!`; the second loop asserts nothing the first did not. | `cases_expected.iter().for_each(\|(case, expected)\| {` |
| P1-35 | src/handlers/use_handler.rs:738 | #11 | The test re-implements the prod `_shell` arm of `modify_path` (:495-503) line for line (format the source line, loop the rc files, `append_to_rcfile`), so a change to the prod path leaves the test green on the old shape. | `let line = format!(". \"{}\"", env_path);` |
| P1-36 | src/helpers/directories.rs:151 | #6 | `get_downloads_directory` is named as a getter but creates the directory tree (`create_dir_all` at :163) as a side effect of being asked where it is. | `let is_folder_created = tokio::fs::create_dir_all(&data_dir).await.is_ok();` |
| P1-37 | src/helpers/version/mod.rs:148 | #6 | `get_version_sync_file_location` creates and writes the file when it is missing; a caller asking for a path gets a file created on disk. | `let mut file = File::create(path).await.context(format!(...))?;` |
| P1-38 | src/helpers/directories.rs:195 | #6 | `get_installation_directory` inherits the directory creation through its `get_downloads_directory` call at :199; the effect is one hop deeper and invisible at every call site. | `let mut installation_location = get_downloads_directory(config).await?;` |
| P1-39 | src/github_requests.rs:283 | #6 | `get_commits_for_nightly` (and `get_upstream_nightly`/`get_upstream_stable`) make unbounded network calls behind a `get_` name; the callee graph reaches `reqwest`. | `let response = make_github_request(client, format!(` |
| P1-40 | src/handlers/install_handler.rs:83 | #9 | The process working directory is used as shared mutable state: `install_handler::start` sets it (:83), `handle_building_from_source` sets it again (:569), and `use_handler::switch` sets it (:125); afterwards relative paths in unrelated modules (`"nightly/bob.json"` :243, `"used"` :148, `"stable"` :80) silently depend on which of them ran last. | `env::set_current_dir(&root)?;` |
| P1-41 | src/helpers/version/types.rs:122 | #32 | `LocalVersion::semver` is written once (install_handler.rs:396) and never read: no `.semver` access anywhere resolves to a `LocalVersion`. Dead field carried through every clone. | `pub semver: Option<Version>,` |
| P1-42 | src/handlers/mod.rs:40 | #32 | `PostDownloadVersionType::Hash` is constructed at install_handler.rs:644 and matched nowhere; the only matches on this enum test `Standard` and `None`, so a hash build silently falls through `start`'s unarchive branch. | `Hash,` |
| P1-43 | src/handlers/install_handler.rs:38 | #53 | The `# Errors` list for `start` names ten causes but omits `Err(anyhow!("Checksum mismatch!"))` at :161, the one error a caller most needs to distinguish. | `/// This function will return an error if:` |
| P1-44 | src/handlers/install_handler.rs:494 | #53 | `handle_building_from_source`'s `# Errors` section omits the Windows "Developer PowerShell" return (:518) and both `unknown error: {error}` returns (:565, :580). | `/// This function will return an error if:` |
| P1-45 | src/github_requests.rs:157 | #53 | `make_github_request`'s `# Errors` section names errors the body cannot produce ("while creating the `Client`", "if the URL is invalid"); the body only propagates `send`/`text`. A caller matching on that contract matches on fiction. | `/// * anyhow::Error - If an error occurs while creating the Client or if the URL is invalid.` |
| P1-46 | src/handlers/uninstall_handler.rs:33 | #53 | `# Errors` claims the function errors when "The version is currently in use"; the body warns and returns `Ok(())` (:51-54). The documented failure mode is unreachable. | `/// * The version is currently in use.` |
| P1-47 | src/handlers/install_handler.rs:55 | none | `# Panics` asserts "This function does not panic" while the body unwraps four times (:102 `nightly_version.as_ref().unwrap()`, :136, :239 `nightly_vec.pop().unwrap()`, :248 `target_commitish...unwrap()`). No listed rule reads the `# Panics` half. | `/// This function does not panic.` |
| P1-48 | src/helpers/unarchive.rs:312 | #36 | `#[allow(dead_code)]` on `remove_base_parent` is stale: both `expand` bodies call it (:137, :249). The suppression now only blinds the compiler if the function does go dead. | `#[allow(dead_code)]` |
| P1-49 | src/config.rs:159 | none | `#[allow(clippy::derivable_impls)]` guards a hand-written `Default` identical to the derive, kept by a comment about confirming with the author; the allow plus the comment plus the 13-line impl replace one derive. | `#[allow(clippy::derivable_impls)]` |
| P1-50 | src/handlers/use_handler.rs:357 | none | The RefCell pair is a no-op: `temp_path` holds a *copy* of the `Option<bool>` field (it is `Copy`), so `temp_path.replace(Some(..))` at :366 and :384 never touches `temp_config`, and the `save_to_file` that follows writes the unchanged config. The user's PATH answer is never persisted, so the prompt returns every run. | `let temp_path = std::cell::RefCell::new(temp_config.borrow().config.add_neovim_binary_to_path);` |
| P1-51 | src/config.rs:19 | #59 | `save_to_file` creates directories, writes a temp file and renames over the user's config with no doc comment at all; its cost (a durable write to a path outside the tree) is invisible at every call site. | `pub async fn save_to_file(&self) -> Result<()> {` |
| P1-52 | src/handlers/use_handler.rs:412 | #59 | Windows `modify_path` reads and rewrites `HKEY_CURRENT_USER\Environment\Path` with no doc comment; a persistent, machine-wide-for-the-user mutation with no first screen describing it. | `async fn modify_path(installation_dir: &str) -> Result<()> {` |
| P1-53 | src/handlers/use_handler.rs:442 | #59 | non-Windows `modify_path` appends a source line to every shell rc file it finds, and writes two env scripts, with no doc comment. | `async fn modify_path(config: &ConfigFile, installation_dir: &str) -> Result<()> {` |
| P1-54 | src/config.rs:206 | none | `env::var(extract).expect(...)` aborts the whole program on a config value naming an unset variable, inside a `fn process(&mut self) -> Result<()>` that has an error channel and never uses it. | `let var = env::var(extract).expect("Failed to get environment variable");` |
| P1-55 | src/handlers/install_handler.rs:380 | none | Placeholder error text shipped to users: a chunk read failure during download reports `hello`. | `let chunk = item.map_err(\|_\| anyhow!("hello"))?;` |
| P1-56 | src/helpers/unarchive.rs:232 | none | The unix progress bar total is the literal `1692` with a comment conceding it is unwise; the bar is wrong for every archive that is not that size. | `let totalsize = 1692; // hard coding this is pretty unwise, but you cant get the length of an archive in tar-rs unlike zip-rs` |
| P1-57 | src/handlers/install_handler.rs:539 | none | The cmake probe only returns on `NotFound`; every other `Err` (permission denied, exec format) falls out of the match and the build proceeds as if cmake were present. The git probe two lines down (:550) has the same hole, and clang/gcc at :522 invert it by treating any non-NotFound error as "present". | `match Command::new("cmake").output().await {` |
| P1-58 | src/handlers/rollback_handler.rs:132 | none | `if added_duration` inside the weeks block can never be true: `added_duration` is `false` at :129 and first set at :142, after this read. Dead branch. | `if added_duration {` |
| P1-59 | src/main.rs:10 | none | `use helpers::{..., version}` at the crate root gives the module a second name, and the crate uses both: `crate::version::parse_version_type` (cli.rs:8, run_handler.rs:25, update_handler.rs:56) and `crate::helpers::version::...` (uninstall_handler.rs:50, use_handler.rs:55). One module, two homes to grep. | `use helpers::{processes::handle_nvim_process, version};` |
| P1-60 | src/github_requests.rs:211 | #38 | `https://api.github.com/repos/neovim/neovim` is retyped in five literals across two modules (:211, :246, :289, helpers/version/mod.rs:294, list_remote_handler.rs:50), and `https://github.com/neovim/neovim.git` twice more (install_handler.rs:591, :595). The repo the tool manages has no single home. | `"https://api.github.com/repos/neovim/neovim/releases/tags/nightly",` |
| P1-61 | src/handlers/use_handler.rs:717 | #42 | `sh_get_rc_with_env_test` reaches its only assertion (:751) inside a `for` over `files`; an empty `files`, or the `return` at :746 on the first append failure, ends the test green with no verdict executed. | `async fn sh_get_rc_with_env_test() {` |
| P1-62 | src/helpers/mod.rs:59 | none | `get_platform_name_none` computes its expected value from the same `cfg!` chain the implementation branches on, so the test restates the code rather than pinning any platform's string. #44's record leaves the SUT-derived-expected mirror unreported, so no listed rule owns this. | `if cfg!(target_os = "windows") {` |
| P1-63 | src/handlers/use_handler.rs:651 | none | `copy_env_files_test` and `sh_get_rc_with_env_test` (:717) run against the developer's real config and real home directory (`ConfigFile::get`, `append_to_rcfile`), mutating shell rc files as a side effect of `cargo test`. | `let config = ConfigFile::get().await.unwrap();` |
| P1-64 | src/helpers/processes.rs:144 | none | `handle_nvim_process` busy-polls `try_wait` with a 200ms sleep for the whole life of the editor on Windows instead of awaiting the child; a wall-clock wait in prod, not a test. | `sleep(Duration::from_millis(200)).await;` |

## Phase 2 - audit finding verdicts

Verdicts are written into `corpus-ext/sheets/bob.rs2.wave1.tsv` (12 rows, the
rules this round judges). Filler script:
`sightline-rs2/judge-tmp/fill-bob.py`.

| rule | rows | real | fp |
|------|------|------|----|
| rs:6 dishonest-accessor | 11 | 7 | 4 |
| rs:48 fold-candidate | 1 | 1 | 0 |
| **total** | **12** | **8** | **4** |

The seven real #6 rows all rest on one edge: `get_downloads_directory`
`create_dir_all`s the tree it reports, so every predicate and getter that
reaches it provisions storage while answering a question. The four fp rows are
the pure path lookups, where the whole callee graph is read-only.

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| rs:6 | read-only lookup: the callee graph mutates nothing, the effect class is `reads-world` alone, and reading the resource the name names is the function's contract | 4 | `src/helpers/directories.rs:25:6:dishonest-accessor:bob_nvim::helpers::directories::get_home_dir` |

Actionable shape for the rule author: `reads-world` on its own does not
separate a lie from a lookup. Every one of the four fp sites reads only the
resource its name promises (`get_home_dir` reads `$HOME`, `get_config_file`
reads `$BOB_CONFIG` plus one `metadata` probe, `ConfigFile::get` reads the
config file). Every one of the seven real sites reaches a *write*
(`create_dir_all`, `File::create`). Gating #6 on a mutating effect in the
callee graph, with `reads-world` demoted to evidence rather than cause, would
have taken all four without losing one real row here.

## Phase 3 - reconciliation

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #34 | covered | `#34 src/consts.rs:16`, "3 commented-out code lines". |
| P1-2 | #34 | threshold-miss | Two one-line commented statements (:135, :151), under the >=3-line run the same rule caught in consts.rs. |
| P1-3 | #29 | threshold-miss | #29 fired on 13 modules; `main.rs` (57 lines) is under the size bar. The crate-wide absence of `//!` is still the finding's own evidence. |
| P1-4 | #29 | covered | `#29 src/handlers/use_handler.rs:1` (754 lines, 13 items). |
| P1-5 | #29 | covered | `#29 src/handlers/install_handler.rs:1`. |
| P1-6 | #29 | covered | `#29 src/helpers/version/mod.rs:1`. |
| P1-7 | #29 | covered | `#29 src/github_requests.rs:1`. |
| P1-8 | #29 | covered | `#29 src/cli.rs:1`. |
| P1-9 | #27 | threshold-miss | #27 took only `install_handler` (1 hot symbol). `use_handler` is 19 lines larger and holds `start` plus `switch`, but neither cleared the hot bar. |
| P1-10 | #27 | covered | `#27 src/handlers/install_handler.rs:1`, hot symbol `start (4)`. |
| P1-11 | #23 | covered | `#23 install_handler.rs:66`, cc 40. |
| P1-12 | #23 | covered | `#23 install_handler.rs:514`, cc 17. |
| P1-13 | #23 | threshold-miss | `use_handler::start` fired at cc 16 but `add_to_path` did not; its RefCell/timeout/match ladder scored under 15. |
| P1-14 | #23 | threshold-miss | non-Windows `modify_path` is `#[cfg(not(target_family = "windows"))]`; a Windows audit does not score the body. |
| P1-15 | #23 | threshold-miss | unix `expand` is `#[cfg(unix)]`; same cfg blind spot. Note the crate's two heaviest platform bodies are both invisible to a single-platform run. |
| P1-16 | #18 | detector-miss | Five phase comments in one function and #18 fired nowhere in the repo. |
| P1-17 | #18 | detector-miss | Four phase comments in a 33-line function; no #18 row. |
| P1-18 | #18 | detector-miss | Three phase comments in `copy_env_files_if_not_exist`; no #18 row. |
| P1-19 | #48 | covered | `#48 install_handler.rs:456`, the one #48 row in the audit. |
| P1-20 | #37 | detector-miss | #37 fired nowhere; `PbWrapper`'s `S` has one argument and the impl declares an unused lifetime. |
| P1-21 | #37 | detector-miss | `finish<P>` has one call site with one type. |
| P1-22 | #37 | detector-miss | `windows_deps<S>` has one call site passing three `String`s. |
| P1-23 | #37 | detector-miss | `EnvPaths<F, S>` has exactly one instantiation, aliased at :600. |
| P1-24 | #37 | detector-miss | `FishScriptPath`/`ShScriptPath` are single-use newtypes over `PathBuf`. |
| P1-25 | #37 | threshold-miss | The rs:37 reading requires the single impl to land on a type the repo owns; `EnvVarProcessor`'s one impl is on `Option<String>`, a foreign type, so the site falls under the rule's own restriction. |
| P1-26 | #11 | threshold-miss | Two identical two-line `deref` bodies, under the clone digest's size floor. |
| P1-27 | #11 | detector-miss | `get_upstream_nightly` / `get_upstream_stable` are whole-`fn` T2 clones differing in one literal, exactly the clone arm's shape, and #11 fired nowhere in the repo. |
| P1-28 | #11 | threshold-miss | The duplicated shasum decision is an if/else expression, not a >=5-statement block. |
| P1-29 | #11 | threshold-miss | The clang and gcc probes are one statement each. |
| P1-30 | #11 | threshold-miss | Five-statement blocks whose match-arm order differs; #23 took the enclosing function (cc 19) instead. |
| P1-31 | #11 | threshold-miss | Three weeks/days/hours blocks of about four statements each, under the >=5 floor. |
| P1-32 | #11 | detector-miss | Five near-identical whole test `fn` bodies in one module; the clone arm reads `fn` bodies. |
| P1-33 | #11 | detector-miss | Six near-identical whole test `fn` bodies in `version_is_hash_tests`. |
| P1-34 | #11 | threshold-miss | The two redundant table loops are one statement each. |
| P1-35 | #11 | threshold-miss | The prod half of the pair sits in a `cfg(not(windows))` function a Windows audit does not read. |
| P1-36 | #6 | covered | `#6 directories.rs:151`, judged real in phase 2. |
| P1-37 | #6 | detector-miss | `get_version_sync_file_location` creates and writes the file it reports (`File::create` + `write_all` at :153) and drew no #6 row, while eight weaker sites did. The strongest #6 site in the repo is the one it missed. |
| P1-38 | #6 | covered | `#6 directories.rs:195`, judged real. |
| P1-39 | #6 | detector-miss | No #6 row anywhere in `github_requests.rs`; `get_upstream_nightly` / `get_upstream_stable` / `get_commits_for_nightly` all make unbounded network calls behind a `get_` name. |
| P1-40 | #9 | threshold-miss | The rs:9 reading covers a shared `static` written by three functions of one module; process cwd, written from three modules via `env::set_current_dir`, is the same ideal in a form the reading does not reach. |
| P1-41 | #32 | detector-miss | `LocalVersion::semver` is a `pub` field written once and read nowhere; #32 fired on no item in the crate. |
| P1-42 | #32 | threshold-miss | `PostDownloadVersionType::Hash` is constructed at install_handler.rs:644, so a resolved edge does reach it; the rule's reading counts construction as a use, and a variant nothing matches slips through. |
| P1-43 | #53 | detector-miss | The `# Errors` section omits `Err(anyhow!("Checksum mismatch!"))`, exactly "a section missing an error the body returns", and #53 fired nowhere. |
| P1-44 | #53 | detector-miss | `handle_building_from_source`'s section omits three returned errors. |
| P1-45 | #53 | threshold-miss | Inverse shape: the section names errors the body cannot return. The rule reads the missing direction only. |
| P1-46 | #53 | threshold-miss | Inverse shape again: a documented failure the body answers with `Ok(())`. |
| P1-47 | none | inventory-gap | A false `# Panics` ("does not panic" over four unwraps) has no owner; #53 reads `# Errors` and clippy's `missing_panics_doc` reads only the absent section. |
| P1-48 | #36 | threshold-miss | One stale `#[allow(dead_code)]` in a 319-line module is far under any density bar; a per-module density reading cannot see a single suppression that is provably unnecessary. |
| P1-49 | none | inventory-gap | A hand-written impl identical to its derive, kept alive by an `#[allow]`, is covered by no rule of the inventory. |
| P1-50 | none | inventory-gap | The RefCell no-op (a `Copy` field copied, mutated, and the original saved) is a correctness bug; no inventory rule reads value flow. |
| P1-51 | #59 | detector-miss | `save_to_file` writes durably outside the tree with no doc at all and no #59 row exists in the audit. |
| P1-52 | #59 | detector-miss | Windows `modify_path` rewrites the user's registry PATH undocumented. |
| P1-53 | #59 | detector-miss | non-Windows `modify_path` appends to every shell rc file undocumented. |
| P1-54 | none | inventory-gap | An `expect` that aborts the process inside a function returning `Result` has no owner in the inventory. |
| P1-55 | none | inventory-gap | Placeholder error text (`anyhow!("hello")`) shipped to users; no rule reads message quality. |
| P1-56 | none | inventory-gap | A hard-coded magic total with a comment conceding it is wrong; no rule owns it. |
| P1-57 | none | inventory-gap | Three tool probes with inconsistent and partly inverted error handling; no rule reads error-handling polarity. |
| P1-58 | none | inventory-gap | A branch that can never be true (`added_duration` read before its first write); #34's rs reading covers commented-out code and identity matches, not unreachable branches. |
| P1-59 | none | inventory-gap | Two crate paths for one module, both in use; no rule reads import-path duplication. |
| P1-60 | #38 | threshold-miss | The rs:38 reading needs module-level literals in >=3 modules; the five copies of the neovim repo URL are inline in `fn` bodies, so a real one-fact-many-homes site is out of reach. |
| P1-61 | #42 | threshold-miss | The test does contain an `assert!`, just on a path an empty `files` or the early `return` skips; the rule reads presence, not reachability. |
| P1-62 | none | inventory-gap | The SUT-derived-expected mirror is explicitly left unreported by #44's record, so no rule owns it by design. |
| P1-63 | none | inventory-gap | Tests that mutate the developer's real home directory and rc files; no rule reads test side effects. |
| P1-64 | none | inventory-gap | A wall-clock poll loop in prod; #47 reads tests only. |

### Phase 3 counts

| class | count |
|-------|-------|
| covered | 12 |
| detector-miss | 19 |
| threshold-miss | 21 |
| inventory-gap | 12 |
| **total** | **64** |

Audit findings outside my phase-1 list (not judged this round): `#23` at
cli.rs:324 (cc exactly 15), list_remote_handler.rs:46, update_handler.rs:52
and use_handler.rs:48, and seven further `#29` module rows. The two rules the
repo would most reward are the two that fired zero times: #11 (nine clone
sites, two of them whole-`fn` T2 groups) and #37 (five monomorphic
abstractions in two modules).
