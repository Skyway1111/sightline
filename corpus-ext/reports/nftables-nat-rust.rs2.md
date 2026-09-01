# nftables-nat-rust (rs2)

Repo: `../gauntlet-corpus/nftables-nat-rust` (Rust workspace: `nat-cli`,
`nat-common`, `nat-console`). Prod tree read: 11 `.rs` files, 4880 lines.
Test code (`#[cfg(test)]` modules, plus `nat-cli/src/ip.rs:71` which forgot the
attribute) judged only against #42/#44/#47/#56.

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | nat-common/src/lib.rs:1 | #29 | 1341-line crate root opens straight on `use clap::Parser;`. No `//!` header says what the crate is, so a reader learns "shared config model + legacy parser + validators" only by scrolling. | `use clap::Parser;` |
| P1-2 | nat-common/src/lib.rs:1 | #27 | The whole workspace's vocabulary (`NftCell`, `Protocol`, `IpVersion`, `Chain`, `TomlConfig`, `ParseError`) lives in one 1341-line file that all three crates import; every task touching any one type ingests all of it. | `pub mod logger;` |
| P1-3 | nat-common/src/lib.rs:87-104 | #11 | `Serialize`/`Deserialize` for `IpVersion` (87-104), `Chain` (152-169) and `Protocol` (207-224) are six impls with byte-identical bodies; only the type name differs. One `serde_plain`-style macro or a shared helper is the single home. | `serializer.serialize_str(&self.to_string())` |
| P1-4 | nat-common/src/lib.rs:65-85 | #11 | `From<String>` and `From<&str>` for `IpVersion` are the same 7-line match twice; `Chain` (132-150) and `Protocol` (181-199) repeat the same pair. Six copies of one dispatch; `From<String>` should call `From<&str>`. | `match version.to_lowercase().as_str() {` |
| P1-5 | nat-common/src/lib.rs:425 | #23 | `NftCell::try_from` is 231 lines with a DROP arm, a field-count arm, a protocol/ip-version arm and a construction arm, nested 5 deep (fn > if DROP > for cell > if let eq_pos > match key > if contains '-'). Cognitive complexity is far past any comprehension bar. | `fn try_from(line: &str) -> Result<Self, Self::Error> {` |
| P1-6 | nat-common/src/lib.rs:472-485 | #11 | The `"src_port"` arm and the `"dst_port"` arm (486-499) are the same 13-line range-parse block; only the two assignment targets differ. | `if value.contains('-') {` |
| P1-7 | nat-common/src/lib.rs:650-653 | #34 | Unreachable arm. Lines 523-551 already reject every `rule_type` outside REDIRECT/SINGLE/RANGE, so this `_ =>` and its duplicated message can never run. Dead weight that reads as a live error path. | `_ => Err(ParseError::InvalidFormat(format!(` |
| P1-8 | nat-common/src/lib.rs:803 | none | `parse_range_dport_and_domain_idx` returns `Result<_, ParseError>` but has no `Err` path: both exits are `Ok`. The signature makes every caller write `?` for an error that cannot happen. | `fn parse_range_dport_and_domain_idx(cells: &[&str]) -> Result<(Option<u16>, usize), ParseError> {` |
| P1-9 | nat-common/src/lib.rs:660 | #23 | `NftCell::validate` is 102 lines: four destructuring arms, the Drop arm alone nesting `if let` inside `if let` inside a match arm four times over. | `pub fn validate(&self) -> Result<(), String> {` |
| P1-10 | nat-common/src/lib.rs:724-732 | #11 | The `src_port` and `dst_port` validation blocks (724-732, 734-742) are the same 9-line shape; only the field names and the message noun differ. | `if let Some(port) = src_port {` |
| P1-11 | nat-common/src/lib.rs:201-205 | #32 | `impl From<Protocol> for String` has no caller anywhere in the workspace (no `String::from(protocol)`, no `let _: String = p.into()`). rustc's `dead_code` does not see unused trait impls, so it stays forever. | `impl From<Protocol> for String {` |
| P1-12 | nat-common/src/lib.rs:1185 | none | `test_drop_ipv4_with_ipv6_address_fails` asserts `err_msg.contains("IPv6格式")`, but no code path in the crate emits that string; `validate_ip_address` only ever produces `"格式无效"`. The test also builds the exact same `NftCell::Drop` as `test_drop_with_ipv6_address` (1169), which asserts `is_ok()`. One of the two is wrong; both cannot hold. | `assert!(err_msg.contains("IPv6格式"));` |
| P1-13 | nat-common/src/lib.rs:1205 | none | Same defect one test down: `test_drop_ipv6_with_ipv4_address_fails` asserts `contains("IPv4格式")` on a plain `192.168.1.1`, which `ipnetwork::IpNetwork::from_str` parses fine. The named behaviour (ip_version vs address-family cross-check) does not exist in `validate`; the tests describe a feature the code dropped. | `assert!(err_msg.contains("IPv4格式"));` |
| P1-14 | nat-common/src/lib.rs:1153-1340 | none | Twelve `test_drop_*` cases are the same 12-line `NftCell::Drop` literal with one field changed and one assertion. A table-driven case list would make the varying axis readable; as written the differences are invisible. | `let rule = NftCell::Drop {` |
| P1-15 | nat-common/src/logger.rs:5 | #32 | `use env_logger;` inside the fn body is a no-op: the crate is already in scope (2018+ paths) and line 1 imports from it. Dead import. | `use env_logger;` |
| P1-16 | nat-cli/src/main.rs:260-262 | #48 | `build_new_script` is a private one-line wrapper that forwards to `nft::build_script` unchanged, with exactly one call site (main.rs:224). The name promises reuse that never came; fold it. | `fn build_new_script(nat_cells: &[config::RuntimeCell]) -> Result<String, io::Error> {`<br>`    nft::build_script(nat_cells)` |
| P1-17 | nat-cli/src/main.rs:29 | #59 | `main` has no doc comment, yet a run creates `/etc/nftables-nat`, writes two kernel sysctls, rewrites every matching interface's `accept_ra`, execs `/usr/sbin/nft` and then never returns. Nothing on the first screen says the call is irreversible machine state. | `fn main() -> Result<(), Box<dyn std::error::Error>> {` |
| P1-18 | nat-cli/src/main.rs:66 | #59 | `global_prepare` writes `/proc/sys/net/ipv4/ip_forward` and `/proc/sys/net/ipv6/conf/all/forwarding` and flips `accept_ra` from 1 to 2 on live interfaces. Undocumented; the reader cannot walk any of it back. | `fn global_prepare() -> Result<(), io::Error> {` |
| P1-19 | nat-cli/src/main.rs:209 | #59 | `handle_loop` never returns: it re-reads config, rewrites `/etc/nftables-nat/nat-diy.nft` and reloads the kernel ruleset on a 60 s tick forever. The signature `Result<(), io::Error>` and the missing doc both hide that. | `fn handle_loop(args: &Args) -> Result<(), io::Error> {` |
| P1-20 | nat-cli/src/main.rs:216-220 | #11 | The debug/release poll interval is written twice, here and at 251-256, with bare `5` and `60` in both. Two homes for one policy; they will drift. | `if cfg!(debug_assertions) {`<br>`    sleep(Duration::from_secs(5));`<br>`} else {` |
| P1-21 | nat-cli/src/main.rs:233-236 | none | `File::create(FILE_NAME_SCRIPT)` failure is swallowed by `if let Ok(...)`, then line 238 runs `nft -f FILE_NAME_SCRIPT` regardless. On a write failure the kernel is loaded from the *previous* script while `latest_script` (232) already recorded the new one, so the loop never retries. | `let f = File::create(FILE_NAME_SCRIPT);`<br>`if let Ok(mut file) = f {`<br>`    file.write_all(script.as_bytes())?;` |
| P1-22 | nat-cli/src/main.rs:246-247 | none | stderr is logged at `error!` level unconditionally, so every successful `nft -f` emits an empty ERROR line. The two lines also use fully qualified `log::info!`/`log::error!` while `info`/`error` are imported at line 10 and used two lines above. | `log::error!("stderr: {}", String::from_utf8_lossy(&output.stderr));` |
| P1-23 | nat-cli/src/main.rs:1 | #29 | 341-line binary root with seven path constants, a sysctl subsystem and the service loop, and no `//!` header. | `#![deny(warnings)]` |
| P1-24 | nat-cli/src/config.rs:16-23 | #32 | `impl Display for RuntimeCell` has no consumer: the only place a `RuntimeCell` is printed is main.rs:229, which uses `{ele:?}` (Debug). Dead impl, invisible to rustc. | `impl Display for RuntimeCell {` |
| P1-25 | nat-cli/src/config.rs:72-83 | none | `read_config` drops every unparseable line with a `warn!` (via `parse_legacy_line`, 41-44) and returns `Ok`. A wholly corrupt config therefore yields an empty rule set, which main.rs flushes into the kernel as "delete every managed table". A parse failure should fail the reload, not silently unblock all traffic. | `Ok(cells)` |
| P1-26 | nat-cli/src/ip.rs:71 | none | `mod test` is missing `#[cfg(test)]`. Every other test module in the repo has it (nft.rs:1185, main.rs:264, config.rs:207, prepare.rs:251); this one compiles its imports into the shipped binary and is invisible to a `#[cfg(test)]`-scoped reader. | `#[allow(clippy::unwrap_used)]`<br>`mod test {` |
| P1-27 | nat-cli/src/ip.rs:80 | none | Four tests (80, 90, 100, 124) resolve `www.google.com` / `localhost` over the live network, and 115 depends on an NXDOMAIN. The suite fails on any offline or DNS-hijacking machine, and 124's "prefers cached address" assertion passes trivially whenever DNS returns one address. | `let domain = "www.google.com";` |
| P1-28 | nat-cli/src/ip.rs:61-65 | none | `.ok_or_else(|| io::Error::other("Failed to select IP address"))` is unreachable: line 45 already returned when `candidates` was empty, so `min_by_key` cannot be `None`. A phantom error path a reader must still evaluate. | `.ok_or_else(\|\| io::Error::other("Failed to select IP address"))?;` |
| P1-29 | nat-cli/src/prepare.rs:1 | #36 | A file-wide `#![allow(dead_code)]` blanket. It is what keeps P1-30's seven empty structs alive and it disarms the compiler for anything added later in the module. | `#![allow(dead_code)]` |
| P1-30 | nat-cli/src/prepare.rs:224-243 | #32 | Seven empty structs, `Metainfo`, `Table`, `Chain`, `Rule`, `Set`, `Map`, `Element`, each with a derive, none referenced anywhere. They are leftovers from an earlier serde shape superseded by the `NftablesEntry` variants at 153-222. | `#[derive(Debug, Serialize, Deserialize)]`<br>`struct Metainfo {}` |
| P1-31 | nat-cli/src/prepare.rs:151 | #34 | Commented-out attribute `// #[serde(untagged)]` left above the live `#[serde(rename_all = "snake_case")]`. Reads as intent no one can act on. | `// #[serde(untagged)]` |
| P1-32 | nat-cli/src/prepare.rs:104-133 | #11 | The IPv4 and IPv6 FORWARD-policy checks are the same 12-line block twice: a 6-clause `if`, an `info!`, a flag set. Only `"ip"`/`"ip6"`, the message and the target field differ. | `if family == "ip"`<br>`    && table == "filter"`<br>`    && name == "FORWARD"` |
| P1-33 | nat-cli/src/prepare.rs:87 | #36 | `#[allow(clippy::single_match)]` suppresses the lint instead of taking its advice; the `match entry { Chain {..} => ..., _ => {} }` is an `if let` written the long way, and the allow hides that from every later reader. | `#[allow(clippy::single_match)]` |
| P1-34 | nat-cli/src/prepare.rs:13 | #59 | `check_and_prepare` writes `/etc/nftables-nat/nat-prepare.nft` and execs `nft -f` on it, changing the host's `ip filter FORWARD` policy from drop to accept. Its only header is a two-line `//` note about Docker, not a doc comment naming that cost. | `pub(crate) fn check_and_prepare() -> Result<(), io::Error> {` |
| P1-35 | nat-cli/src/prepare.rs:61 | #23 | `check_current_ruleset` runs a subprocess, a serde parse, a loop, a match and two six-clause boolean conditions in one 80-line fn. | `fn check_current_ruleset() -> Result<CheckResult, io::Error> {` |
| P1-36 | nat-cli/src/nft.rs:1 | #29 | The largest module in the repo at 1507 lines, holding nine types and 30 free functions, opens on `use crate::config::RuntimeCell;`. No `//!` explains the emitter pipeline (collect into `Ruleset`, then `emit_script`). | `use crate::config::RuntimeCell;` |
| P1-37 | nat-cli/src/nft.rs:1 | #27 | Every emitter task loads all 1507 lines to touch one of them; `PortSpan`, `Family`, `NatMaps`, `FilterSets` and the whole `emit_*` family have no separation. | `const CT_MARK: &str = "0x4e4154";` |
| P1-38 | nat-cli/src/nft.rs:26-59 | #11 | `Family::name`, `addr_type`, `dnat_kw`, `localhost`, `snat_env` are five copies of one two-arm dispatch. A single `const` table keyed by variant (or one `fn attrs(self) -> &'static FamilyAttrs`) is the one home. | `fn addr_type(self) -> &'static str {`<br>`    match self {` |
| P1-39 | nat-cli/src/nft.rs:40-45 | none | `dnat_kw` returns exactly what `name` (26-31) returns: `"ip"` / `"ip6"`. Two names for one value; the callers at 878 and 889 could use `fam`, which is already in scope. | `fn dnat_kw(self) -> &'static str {` |
| P1-40 | nat-cli/src/nft.rs:202-232 | #11 | `dnat_mut`, `dnat_ip_mut`, `redirect_mut`, `shift_mut`: four identical bodies (Tcp arm, Udp arm, `Protocol::All => unreachable!()`), differing only in the two field names. | `fn dnat_mut(&mut self, protocol: Protocol) -> &mut Vec<MapElem> {` |
| P1-41 | nat-cli/src/nft.rs:303-329 | #11 | `saddr_mut`, `daddr_mut`, `dport_mut`, `sport_mut`: the same four-line chain dispatch four times. With P1-40 that is eight copies of one shape in one file. | `match chain {`<br>`    Chain::Input => &mut self.input_saddr,` |
| P1-42 | nat-cli/src/nft.rs:511-583 | #11 | `insert_redirect`, `insert_dnat`, `insert_dnat_ip`, `insert_dnat_shift` are four copies of `for proto in protocols(p) { if skip_conflict(..) { continue } bucket.push(Elem{..}) }`. The only real variation is the element built. | `for proto in protocols(protocol) {`<br>`    if skip_conflict(maps, *proto, span, "redirect") {`<br>`        continue;` |
| P1-43 | nat-cli/src/nft.rs:924-955 | #11 | `emit_map` and `emit_set` (1066-1089) are the same 24-line emitter: empty guard, interval scan, header, `elements = {`, comma-joined loop with a comment branch, `}`. Only the element rendering differs. | `let interval = elems.iter().any(\|e\| e.span.is_range());`<br>`out.push_str(&format!("add map {family} self-nat {name} {{\n"));` |
| P1-44 | nat-cli/src/nft.rs:1124-1175 | #11 | `emit_addr_drop_rules` and `emit_port_drop_rules` are the same three-branch (`all`/`tcp`/`udp`) push, differing only in the match expression inside the format string. Six near-identical `push_str(&format!(...))` calls across the two. | `if !sets.all.is_empty() {`<br>`    out.push_str(&format!(` |
| P1-45 | nat-cli/src/nft.rs:206 | none | Five `Protocol::All => unreachable!()` arms (206, 214, 222, 230, 248) encode "callers must pre-expand All via `protocols()`" as a panic instead of as a type. A `enum L4 { Tcp, Udp }` argument would make the four map accessors total. | `Protocol::All => unreachable!(),` |
| P1-46 | nat-cli/src/nft.rs:365-375 | none | `build_script` returns `Result<String, io::Error>` but has exactly one exit, `Ok(...)`; `add_rule`'s errors are downgraded to `warn!` at 371. A rule whose DNS target fails to resolve silently vanishes from the emitted ruleset while the caller sees success. | `warn!("Failed to build rule for {rule:?}: {e}");` |
| P1-47 | nat-cli/src/nft.rs:1036-1037 | none | `emit_drop_set_rules(out, fam, family.name(), ...)` passes the same string as both `fam` and `ip_prefix`, at both call sites, and `fam` was itself bound from `family.name()` at line 1000. One of the two parameters is dead. | `emit_drop_set_rules(out, fam, family.name(), Chain::Input, filter);` |
| P1-48 | nat-cli/src/nft.rs:598 | #23 | `add_drop` is a 72-line destructure plus a five-branch `dims == 1 && has_x` cascade, each branch re-testing with `if let Some(..)` what `has_x` already proved. | `if dims == 1 && has_src_ip {`<br>`    if let Some(ip) = src_ip {` |
| P1-49 | nat-cli/src/nft.rs:1198-1201 | none | `check_nft` returns silently when `/usr/sbin/nft` is absent, so on any non-Linux machine all 14 call sites of `check_nft(&script)` assert nothing and the suite still reports green. A skipped oracle should be visible. | `if !Path::new("/usr/sbin/nft").exists() {`<br>`    return;` |
| P1-50 | nat-console/src/main.rs:31 | none | The JWT signing key has a working default, so `nat-console --username u --password p` starts and mints tokens anyone can forge. A secret should have no default; make the flag required or generate one per boot. | `#[arg(long, default_value = "your-secret-key-change-in-production")]` |
| P1-51 | nat-console/src/main.rs:56-59 | #11 | `SocketAddr::new(args.host.unwrap_or(IpAddr::V6(Ipv6Addr::UNSPECIFIED)), args.port)` is computed here only to be logged, then recomputed identically at server.rs:37. Two homes for the listen address. | `let listen_addr = SocketAddr::new(`<br>`    args.host.unwrap_or(IpAddr::V6(Ipv6Addr::UNSPECIFIED)),` |
| P1-52 | nat-console/src/main.rs:8 | none | `type DynError` is declared and used once (line 52); server.rs:34 spells the identical type out longhand instead. The alias does not pay for itself in either direction. | `type DynError = Box<dyn std::error::Error + Send + Sync>;` |
| P1-53 | nat-console/src/server.rs:22-23 | none | `index.html` and `login.html` are baked in with `include_str!` *and* served again from a CWD-relative `static/` dir by the fallback at line 72. Two homes for the same asset, and which one a viewer gets depends on the process's working directory. | `const INDEX_HTML: &str = include_str!("../../static/index.html");`<br>`.fallback_service(ServeDir::new("static"))` |
| P1-54 | nat-console/src/server.rs:30-32 | #48 | `serve_login` is a private one-line handler with a single route (line 67). Line 70 already registers `/health` as an inline closure, so the file itself shows the shorter form. | `async fn serve_login() -> impl IntoResponse {`<br>`    Html(LOGIN_HTML)` |
| P1-55 | nat-console/src/handlers.rs:286 | #18 | `hybrid_auth_middleware` narrates itself in numbered phases: `// 1. 优先检查 Authorization header` (286), `// 2. Fallback: 检查 Cookie` (305), `// 3. 没有找到有效的认证信息` (321). Three phase labels spelling three functions. | `// 1. 优先检查 Authorization header` |
| P1-56 | nat-console/src/handlers.rs:292-302 | #11 | The Bearer branch and the Cookie branch (309-318) are the same 10-line `match Claims::decode { Ok => insert + next.run, Err => error! + UNAUTHORIZED }`. Only the log message differs. | `match Claims::<ClaimsPayload>::decode(token, &jwt_config) {`<br>`    Ok(claims) => {`<br>`        request.extensions_mut().insert(claims);` |
| P1-57 | nat-console/src/handlers.rs:61 | #20 | `.map_err(\|e\| { error!("...: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })` is written four times in this file (61-64, 84-87, 147-150, 152-155) with only the message literal changing. Name it once. | `.map_err(\|e\| {`<br>`    error!("密码验证失败: {:?}", e);`<br>`    StatusCode::INTERNAL_SERVER_ERROR` |
| P1-58 | nat-console/src/handlers.rs:185 | #20 | The tuple-returning twin of P1-57 appears four more times (185-191, 239-245, 252-258, 269-275): log the error, return `(StatusCode, format!(...))`. Eight copies of one error-mapping policy in a 323-line file. | `.map_err(\|e\| {`<br>`    error!("Failed to get config info: {:?}", e);`<br>`    (` |
| P1-59 | nat-console/src/handlers.rs:49-57 | #11 | The `UNAUTHORIZED` + "用户名或密码错误" response is built twice, identically, at 49-57 and 67-75. A single `fn unauthorized()` is the one home; as written the two messages can drift apart. | `Json(LoginResponse {`<br>`    success: false,`<br>`    message: "用户名或密码错误".to_string(),` |
| P1-60 | nat-console/src/handlers.rs:251-261 | #11 | `get_rules` and `get_rules_json` (268-278) share the same body; the only difference is `Html(format!("<pre>{}</pre>", rules))` versus `Json(RulesResponse { rules })`. | `let rules = get_nftables_rules().map_err(\|e\| {`<br>`    error!("Failed to get nftables rules: {:?}", e);` |
| P1-61 | nat-console/src/handlers.rs:1 | #29 | 323 lines carrying app state, five request/response types, six handlers and the auth middleware, with no `//!` header. | `use crate::config::{` |
| P1-62 | nat-console/src/config.rs:18 | #6 | `get_config_info` is named as a getter but on the fall-through path (line 37) it reads `/lib/systemd/system/nat.service` off disk and runs a clap parse over its `ExecStart` line. The effect is buried one call deep, exactly where it lies hardest. | `pub fn get_config_info(` |
| P1-63 | nat-console/src/config.rs:170 | #6 | `get_nftables_rules` spawns four `/usr/sbin/nft` subprocesses. A `get_`-named function that forks four processes misrepresents its contract to every handler that calls it. | `pub fn get_nftables_rules() -> Result<String, io::Error> {` |
| P1-64 | nat-console/src/config.rs:170 | #59 | Same function, the other half: it is the entry point behind `GET /api/rules` and `GET /rules`, spends four process spawns per request, and carries no doc comment saying so. No rate limit or cache anywhere above it. | `pub fn get_nftables_rules() -> Result<String, io::Error> {` |
| P1-65 | nat-console/src/config.rs:174-220 | #11 | Four copies of "run `nft list table <fam> <table>`, lossy-decode stdout, else a `# ... not found` placeholder" (174-181, 184-194, 197-207, 210-220). One helper taking `(family, table)` replaces all four. | `let output6 = Command::new("/usr/sbin/nft")`<br>`    .arg("list")`<br>`    .arg("table")` |
| P1-66 | nat-console/src/config.rs:174-181 | none | Inconsistent failure policy inside one function: the first spawn propagates with `?`, the other three swallow the error into a placeholder string. A missing `nft` binary therefore 500s, while a missing `nft` binary one line later renders as normal output. | `.output()?;` |
| P1-67 | nat-console/src/config.rs:47 | #18 | `detect_config_info_from_systemd` narrates four phases in prose: `// 查找 ExecStart 行` (47), `// 解析 ExecStart 行` (58), `// 构造 clap 解析用的参数数组` (76), `// 从 Args 中提取配置信息` (91). Each label is a function boundary spelled as a comment. | `// 查找 ExecStart 行` |
| P1-68 | nat-console/src/config.rs:1 | #29 | 229-line module holding config detection, the `ConfigFormat` model, its file I/O and the nft-shelling reader, with no `//!` header. | `use nat_common::{Args, TomlConfig};` |
| P1-69 | nat-cli/src/main.rs:67 | #38 | The path `"/usr/sbin/nft"` is hardcoded in four modules across two crates (main.rs:67 and :238, prepare.rs:22 and :63, nft.rs:794 shebang, config.rs:174/184/197/210), with no shared const. Every copy is a place the next fix (a configurable nft path, a `/usr/bin` distro) forgets. | `Command::new("/usr/sbin/nft").arg("-v").output()` |

## Phase 2 - audit finding verdicts

Judged this round: the 8 rows of `corpus-ext/sheets/nftables-nat-rust.rs2.wave1.tsv`
(rs:6 and rs:48). Verdicts and one-line reasons are written there; the table below
is the summary. The audit's other 35 findings (#11 x21, #29 x7, #23 x5, #27 x1,
#18 x1) are unjudged this round and enter only phase 3.

Filled by `<GAUNTLET_CORPUS_ROOT>/../sightline-rs2/judge-tmp/fill-nftables-nat-rust.py`.
Columns 1-9 verified byte-identical to the committed sheet; only `verdict` and `why`
were written.

**8 rows judged: 4 real, 4 fp.** rs:6 2/5 real, rs:48 2/3 real.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| nat-console/src/config.rs:18 | #6 | indexed | real | `get_config_info` reads `/lib/systemd/system/nat.service` off disk and clap-parses its `ExecStart` line on the fall-through path (37 to 44-89); the effect is buried one call deep behind a getter name. |
| nat-console/src/config.rs:170 | #6 | indexed | real | `get_nftables_rules` forks four `/usr/sbin/nft` subprocesses (174, 184, 197, 210); a `get_`-named free function that spawns four processes per call misstates its cost, and both handlers call it once per request with no cache. |
| nat-console/src/handlers.rs:138 | #6 | indexed | fp | axum route handler, registered at server.rs:55 as `get(get_config).post(save_config)`: the name mirrors the HTTP method, not an accessor contract, and reading the config it serves is the route's purpose. |
| nat-console/src/handlers.rs:251 | #6 | indexed | fp | axum route handler for `GET /rules` (server.rs:57); the spawn is inherited wholly from `get_nftables_rules`, already flagged at config.rs:170, so the fix belongs at the callee. |
| nat-console/src/handlers.rs:268 | #6 | indexed | fp | axum route handler for `GET /api/rules` (server.rs:56); third report of the one defect at config.rs:170. |
| nat-cli/src/main.rs:260 | #48 | indexed | real | `build_new_script` forwards to `nft::build_script` unchanged, one line, one call site (224); the wrapper adds no name the caller does not already have. |
| nat-cli/src/nft.rs:95 | #48 | indexed | fp | inherent method on `PortSpan` beside `single`/`range`/`is_range`/`emit`: it is that value type's API, and the body is interval arithmetic whose inline form at the `.find()` closure (255) is the classic off-by-one. |
| nat-cli/src/nft.rs:499 | #48 | indexed | real | a 3-parameter signature for one disjunction whose clauses each read for themselves; inlined at its only call site (421) it is one line with no meaning lost. |

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| #6 | web route handler whose name is fixed by the HTTP verb it serves (axum `get(...)`/`post(...)` registration): the world-reading is the route's contract, and the effect, where real, is inherited from an already-flagged callee | 3 | `nat-console/src/handlers.rs:138:6:dishonest-accessor:nat_console::handlers::get_config` |
| #48 | one-line inherent method that is part of a small value type's coherent API, where the inline form is error-prone arithmetic rather than a self-evident expression; the single call site is an artifact of the type's age, not of a shallow abstraction | 1 | `nat-cli/src/nft.rs:95:48:fold:nat_cli::nft::PortSpan::overlaps` |

## Phase 3 - reconciliation

Convention used, stated so it can be overruled: **covered** means a finding names the
same defect at the same site. A finding at the same line under a rule about a
different property is not coverage (P1-19 and P1-64 are the two sites where this
bites: #23 and #6 fire on those exact symbols, but the undocumented-cost claim is
unreported). Where I mapped a site to a rule whose Rust record does not in fact
reach it, the row is `detector-miss` per the class definition and the note says so.

**69 sites: 17 covered, 52 misses (19 detector-miss, 14 threshold-miss, 19 inventory-gap).**

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #29 | covered | #29 nat-common/src/lib.rs:1 (1341 lines, 13 items). |
| P1-2 | #27 | covered | #27 nat-common/src/lib.rs:1, and it names the hot symbols (Protocol 22, NftCell 14, Chain 11). |
| P1-3 | #11 | threshold-miss | The six `Serialize`/`Deserialize` impls are one statement each (`serializer.serialize_str(&self.to_string())`); the clone arm found the `fmt`/`from` pairs in the same file but not these. Body-size cutoff. |
| P1-4 | #11 | threshold-miss | The rule fired on the same shape two enums over (`Chain::from` 133 + `Protocol::from` 182, group x2) but the repo has six `From` bodies of one shape; `IpVersion`'s pair at 65-85 and the three `&str` twins are outside the group. Group under-counts. |
| P1-5 | #23 | covered | #23 lib.rs:425, cc 73. |
| P1-6 | #11 | detector-miss | The `"src_port"` and `"dst_port"` arms (472-485, 486-499) are a 13-line repeated block inside one fn; no finding. Same class as P1-10/32/56/59/65: repeated blocks *within* a function body go unseen while whole-`fn` clones are caught. |
| P1-7 | #34 | detector-miss | My #34 mapping was wrong and this is really an inventory gap: rs:34 covers commented-out code and identity matches, not an unreachable `_` arm. No rule in the inventory owns dead branches. |
| P1-8 | none | inventory-gap | `Result` with no `Err` path. |
| P1-9 | #23 | covered | #23 lib.rs:660, cc 43. |
| P1-10 | #11 | detector-miss | Intra-function repeated block (724-732 vs 734-742). See P1-6. |
| P1-11 | #32 | detector-miss | Dead `impl From<Protocol> for String`. No #32 finding anywhere in the repo; rs:32's closed-world deletion prover works on `pub` items, and an unused trait impl is neither an item it enumerates nor something rustc's `dead_code` complement reports. |
| P1-12 | none | inventory-gap | Test asserts an error string no code path emits. |
| P1-13 | none | inventory-gap | As P1-12. |
| P1-14 | none | inventory-gap | Twelve near-identical test bodies; #11 reads prod only, by design. |
| P1-15 | #32 | detector-miss | Redundant `use env_logger;` inside `logger::init`. rs:32 has no dead-import arm; the Python reading does (`dead-import`, 62/63). Inventory asymmetry between the two languages. |
| P1-16 | #48 | covered | #48 main.rs:260, judged **real** in phase 2. |
| P1-17 | #59 | detector-miss | `main` creates dirs, writes two sysctls, rewrites live `accept_ra`, execs `nft`, never returns. Zero #59 findings in a repo whose whole job is off-machine state. |
| P1-18 | #59 | detector-miss | `global_prepare` writes `/proc/sys/...`. See P1-17. |
| P1-19 | #59 | detector-miss | #23 fires on this exact symbol (main.rs:209, cc 18) but for complexity; the "reloads the kernel ruleset every 60 s, undocumented" half is unreported. |
| P1-20 | #11 | threshold-miss | Duplicated debug/release poll policy (216-220 vs 251-256); two branches of two statements, under the block arm's size floor. |
| P1-21 | none | inventory-gap | Swallowed `File::create` failure, then `nft -f` on the stale file. |
| P1-22 | none | inventory-gap | Unconditional `error!` on empty stderr. |
| P1-23 | #29 | covered | #29 nat-cli/src/main.rs:1 (341 lines, 16 items). |
| P1-24 | #32 | detector-miss | Dead `impl Display for RuntimeCell`. Same cause as P1-11: unused trait impls are invisible to both #32 and rustc. |
| P1-25 | none | inventory-gap | Unparseable config lines dropped to `Ok(vec![])`, which flushes the kernel tables. |
| P1-26 | none | inventory-gap | `mod test` missing `#[cfg(test)]`. Note the second-order effect: this module's contents are prod by the checker's own reckoning, so #42/#44/#47 would read it as prod and every prod rule would read it as test-shaped noise. |
| P1-27 | none | inventory-gap | Four tests depend on live DNS. |
| P1-28 | none | inventory-gap | Unreachable `ok_or_else`. |
| P1-29 | #36 | threshold-miss | `#![allow(dead_code)]` over a 504-line module. rs:36 measures per-module allow *density*; one file-wide allow scores near zero on density while doing the most damage of any allow in the repo. Density is the wrong measure for a crate- or module-scoped `#![allow]`. |
| P1-30 | #32 | detector-miss | Seven empty unreferenced structs (224-243). The interaction is the point: rs:32's complement says rustc's `dead_code` owns private items, but `#![allow(dead_code)]` at prepare.rs:1 disarms exactly that complement. Where the allow is present, the complement claim fails and #32 is the only reader left. |
| P1-31 | #34 | threshold-miss | Commented-out `#[serde(untagged)]` is one line; the arm needs a run of >=3. |
| P1-32 | #11 | detector-miss | The ip/ip6 FORWARD checks are a 12-line block twice inside one fn. See P1-6. |
| P1-33 | #36 | threshold-miss | `#[allow(clippy::single_match)]`. See P1-29. |
| P1-34 | #59 | detector-miss | `check_and_prepare` rewrites the host's `ip filter FORWARD` policy. See P1-17. |
| P1-35 | #23 | threshold-miss | `check_current_ruleset` (subprocess + serde + loop + match + two 6-clause conditions) scored under 15 while `handle_loop` scored 18. A long `&&` chain apparently counts once; that is the boundary this site sits on. |
| P1-36 | #29 | covered | #29 nat-cli/src/nft.rs:1 (1507 lines, 44 items). |
| P1-37 | #27 | threshold-miss | nft.rs is 166 lines *larger* than the one module #27 flagged, but its symbols are private and single-file, so the hot-symbol count stays low. The price a reader pays to open it is the same either way. |
| P1-38 | #11 | threshold-miss | `Family`'s five identical two-arm dispatches (26-59) are one statement each; the arm caught the eight-member `*_mut` families but not these. Same size floor as P1-3. |
| P1-39 | none | inventory-gap | `dnat_kw` returns exactly what `name` returns. |
| P1-40 | #11 | covered | #11 nft.rs:202/210/218/226, group x4. |
| P1-41 | #11 | covered | #11 nft.rs:303/310/317/324, group x6: it found two members I missed (`Ruleset::nat_mut` 349, `filter_mut` 356). |
| P1-42 | #11 | covered | #11 nft.rs:511/530/543, group x3. I claimed x4; `insert_dnat_shift` (562) carries two extra fields and is correctly outside the group. |
| P1-43 | #11 | threshold-miss | `emit_map`/`emit_set` were not paired, though the arm did pair `emit_redirect_rule`/`emit_dnat_rule` (957/966), which I missed. My pair diverges in the element-rendering branch, so the digest splits them; the shared 24-line skeleton is the same. |
| P1-44 | #11 | covered | #11 nft.rs:1124/1151. |
| P1-45 | none | inventory-gap | Five `Protocol::All => unreachable!()` arms encoding a precondition as a panic. |
| P1-46 | none | inventory-gap | `build_script` returns `Result` with one `Ok` exit; unresolvable rules are warned and dropped. |
| P1-47 | none | inventory-gap | `fam` and `ip_prefix` are the same string at both call sites. |
| P1-48 | #23 | covered | #23 nft.rs:598, cc 23. |
| P1-49 | none | inventory-gap | `check_nft` returns silently when `/usr/sbin/nft` is absent, disarming 14 call sites. |
| P1-50 | none | inventory-gap | Working default for the JWT signing key. |
| P1-51 | #11 | threshold-miss | One-statement duplication across two modules; under the size floor, and cross-module. |
| P1-52 | none | inventory-gap | `DynError` alias used once, spelled longhand elsewhere. |
| P1-53 | none | inventory-gap | Static assets both embedded and served from a CWD-relative dir. |
| P1-54 | #48 | detector-miss | `serve_login` is registered as `get(serve_login)`, so it is passed as a value and has no call edge; the prover's one-call-site test never sees it. Concrete cause: fn-item-as-value references. |
| P1-55 | #18 | covered | #18 handlers.rs:286, "3 labeled phases". The only #18 finding in the repo, and it is the numbered one. |
| P1-56 | #11 | detector-miss | The Bearer and Cookie branches are a 10-line block twice inside one fn. See P1-6. |
| P1-57 | #20 | detector-miss | Four `map_err(\|e\| { error!(...); INTERNAL_SERVER_ERROR })` closures in one module, identical bar the message literal. Zero #20 findings; the sameness test appears literal-sensitive, and literal-differing is the shape this smell almost always takes. |
| P1-58 | #20 | detector-miss | Four more of the tuple-returning twin. See P1-57. |
| P1-59 | #11 | detector-miss | The `UNAUTHORIZED` response built twice, 9 lines each, inside one fn. See P1-6. |
| P1-60 | #11 | threshold-miss | `get_rules`/`get_rules_json` differ only in the final wrap; both fns carry a #6 finding but no #11. Just past the sameness bar. |
| P1-61 | #29 | covered | #29 nat-console/src/handlers.rs:1 (323 lines, 15 items). |
| P1-62 | #6 | covered | #6 nat-console/src/config.rs:18, judged **real**. |
| P1-63 | #6 | covered | #6 nat-console/src/config.rs:170, judged **real**. |
| P1-64 | #59 | detector-miss | #6 fires on this symbol for the name; the "four process spawns per request, undocumented, uncached" cost claim is unreported. See P1-19. |
| P1-65 | #11 | detector-miss | Four ~10-line `nft list table` blocks inside one fn. See P1-6. |
| P1-66 | none | inventory-gap | First spawn propagates, the other three swallow. |
| P1-67 | #18 | threshold-miss | Four phase labels in `detect_config_info_from_systemd`, but unnumbered prose; the one #18 finding in the repo is the numbered set at handlers.rs:286. The cutoff is the numbering, not the phase count. |
| P1-68 | #29 | covered | #29 nat-console/src/config.rs:1 (229 lines, 8 items). |
| P1-69 | #38 | threshold-miss | `"/usr/sbin/nft"` in four modules across two crates, but always as an inline argument literal; rs:38 requires module-level declarations, which this repo never uses. |

### What the misses cluster into

Four shapes account for 34 of the 52 misses:

1. **Repeated blocks inside one function body** (P1-6, 10, 32, 56, 59, 65: 6 sites).
   #11 caught every whole-`fn` clone group in the repo, and no intra-function
   block. This is the single largest recall hole here.
2. **No #59 finding at all** (P1-17, 18, 19, 34, 64: 5 sites) in a repo whose
   binaries write kernel sysctls, rewrite `/etc/nftables-nat/*.nft` and reload the
   host ruleset on a timer. Nothing was reported.
3. **Small-body clone groups** (P1-3, 4, 20, 38, 51, 60: 6 sites). Bodies of one or
   two statements repeated five or six times are under the size floor, so the two
   most-repeated shapes in the workspace (the six serde impls, the five `Family`
   accessors) are unreported while the four-member `*_mut` group is caught.
4. **Dead code the compiler cannot see** (P1-11, 15, 24, 30: 4 sites): two unused
   trait impls, a redundant import, and seven empty structs whose rustc complement
   is disarmed by the `#![allow(dead_code)]` five lines above them.

The `#![allow(dead_code)]` interaction (P1-29, P1-30) is worth its own line: #36
scores a file-wide `#![allow]` as near-zero density, and #32 defers private items to
rustc, so the one attribute in this repo that blinds the compiler falls in the gap
between the two rules that exist to catch it.
