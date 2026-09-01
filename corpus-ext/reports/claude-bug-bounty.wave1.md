# claude-bug-bounty — wave 1

Blind judgment. Repo read cold against the #1-41 ideal inventory. Prod tree
covered: `engine.py`, `agent.py`, `brain.py`, `serve.py`, `memory/`, `mcp/`,
`scripts/`, `demo/`, `tools/`. `agents/` and `web3/` are docs-only (no `.py`);
`tests/` excluded per prod-only scope.

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | engine.py:96 | #32 | `COMMAND_ALIASES` dict defined but never referenced anywhere (aliases are re-declared inline in `main()`'s `add_parser`) | `COMMAND_ALIASES = {` |
| P1-2 | engine.py:504 | #13 | `cmd_triage` body is a single forwarding call to `cmd_validate` | `def cmd_triage(args):` / `cmd_validate(args)` |
| P1-3 | agent.py:99 | #32 | `Brain`, `BRAIN_SYSTEM`, `MODEL_PRIORITY` imported from brain but never used in agent.py (only `OLLAMA_HOST`/`_pick_model` are) | `from brain import Brain, BRAIN_SYSTEM, MODEL_PRIORITY, OLLAMA_HOST, _pick_model` |
| P1-4 | agent.py:1565 | #32 | `StructuredTool` and `inspect` (1566) imported inside `build_langgraph_agent`, never used; `AIMessage` (1564) also unused | `from langchain_core.tools import tool as lc_tool, StructuredTool` / `import inspect` |
| P1-5 | agent.py:794 | #39 | `_filter_recon_urls_to_scope` docstring runs ~25 lines (794-818), far longer than its ~19-line body, and narrates review history | `"""Drop out-of-scope URLs ... See SECURITY-REVIEW-2026-08-22.md finding #2 follow-up. (...)"""` |
| P1-6 | agent.py:584 | #39 | `dispatch()` carries a ~30-line inline comment block (584-614) narrating prior bugs/rationale rather than the code | `# Time-budget gate: ... See SECURITY-REVIEW-2026-08-22.md finding #16 (LOW-MEDIUM).` |
| P1-7 | agent.py:783 | #28 | docstring cites `SECURITY-REVIEW-2026-08-22.md`, a file absent from the repo; referenced from ~6 sites in this file alone (535/593/614/783/799/813) | `SECURITY-REVIEW-2026-08-22.md finding #2."""` |
| P1-8 | agent.py:1665 | #1 | public entry point `run_agent_hunt(...) -> dict` returns a bare untyped `dict` | `) -> dict:` |
| P1-9 | agent.py:578 | #1 | `ToolDispatcher.dispatch(self, name: str, args: dict) -> str` takes a bare `dict` tool-args boundary | `def dispatch(self, name: str, args: dict) -> str:` |
| P1-10 | brain.py:2042 | #28 | `run_command` docstring references the missing `SECURITY-REVIEW-2026-08-22.md`; also present at brain.py:2308 | `_sanitize_exploit_command is defense in depth ... (see SECURITY-REVIEW-2026-08-22.md finding #1).` |
| P1-11 | brain.py:2288 | #39 | `auto_triage_and_exploit` docstring closes with a paragraph of review-history narration (2300-2309: "could previously trigger ... finding #14, MEDIUM") | `... 25 * (1 + 6) = 175 completions in a single run (SECURITY-REVIEW-2026-08-22.md finding #14, MEDIUM).` |
| P1-12 | brain.py:2286 | none | `EXPLOIT_ROUND_CAP = 6` duplicates the hardcoded `range(6)` in `exploit_finding` (2222); the constant exists but the loop still hardcodes the literal | `EXPLOIT_ROUND_CAP = 6` |
| P1-13 | brain.py:1956 | #1 | `_TOOL_INSTALL: dict` (and `_TOOL_ALIASES: dict`, 1992) annotated as bare `dict` | `_TOOL_INSTALL: dict = {` |
| P1-14 | memory/audit_log.py:16 | #32 | `SchemaError` imported but never referenced in the module | `from memory.schemas import validate_audit_entry, make_audit_entry, SchemaError` |
| P1-15 | memory/audit_log.py:33 | #1 | public `log(self, entry: dict) -> None` takes a bare `dict`; same weak boundary on `read_all`/`count_by_session`/`check_request` (84/106/302) | `def log(self, entry: dict) -> None:` |
| P1-16 | memory/audit_log.py:358 | #13 | `record_failure` is a single forwarding call to `self._breaker.record_failure`; `record_success` (362) likewise | `return self._breaker.record_failure(host)` |
| P1-17 | memory/schemas.py:196 | #38 | timestamp format literal `"%Y-%m-%dT%H:%M:%SZ"` repeated verbatim in four factories (196/240/323/355) with no shared constant | `"ts": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),` |
| P1-18 | memory/schemas.py:81 | #1 | validators take and return bare `dict` (`validate_journal_entry(entry: dict) -> dict`, also 124/147/263) | `def validate_journal_entry(entry: dict) -> dict:` |
| P1-19 | memory/schemas.py:303 | #39 | docstring gives the same value (`'hunt'`) as two distinct cases ("interactive or 'hunt' for autopilot"); the distinction it narrates is a no-op | `or 'hunt' for autopilot (both map to the hunt action type).` |
| P1-20 | memory/pattern_db.py:69 | #1 | public `save(self, entry: dict) -> bool` bare `dict`; `read_all`/`match` return `list[dict]` (103/139) | `def save(self, entry: dict) -> bool:` |
| P1-21 | mcp/hackerone-mcp/server.py:34 | #34 | `except ImportError` branch re-assigns `_SSL_CTX` to the identical `ssl.create_default_context()` already set — a no-op handler | `_SSL_CTX = ssl.create_default_context()` |
| P1-22 | mcp/hackerone-mcp/server.py:282 | #12 | `main()` hand-rolls a `--program/--limit` arg loop (297-306) reimplementing `argparse` | `def main():` |
| P1-23 | scripts/dork_runner.py:9 | #32 | `import time` (9) and `import random` (12) never used | `import time` |
| P1-24 | scripts/dork_runner.py:189 | #9 | module-level `DORK_CATEGORIES["all"]` mutated in place by a top-level loop at import time | `DORK_CATEGORIES["all"].extend(dorks)` |
| P1-25 | demo/app.py:35 | #39 | docstring (11) says override host with `DEMO_HOST`, but code reads `APP_HOST` — comment names a var the code never consults | `HOST = os.environ.get("APP_HOST", "127.0.0.1")` |
| P1-26 | demo/app.py:133 | #39 | comment claims it quiets the per-request access log, but `log_message` still writes every request to stderr — comment contradicts behavior | `# Quiet the default per-request access log so the recording stays clean.` |
| P1-27 | demo/app.py:256 | #34 | bare `except Exception: pass` silently swallows banner-load failures | `except Exception:` / `pass` |
| P1-28 | tools/banner.py:22 | #32 | `Iterable` imported from typing, never used | `from typing import Iterable, Optional, Sequence, Tuple, Union` |
| P1-29 | tools/cors_scanner.py:34 | #32 | `field` imported from dataclasses, never used | `from dataclasses import dataclass, field` |
| P1-30 | tools/cors_scanner.py:45 | #32 | `LOW` constant never referenced (and is assigned the surprising value `"MEDIUM_LOW"`) | `CRITICAL, HIGH, MEDIUM, LOW, INFO = "CRITICAL", "HIGH", "MEDIUM", "MEDIUM_LOW", "INFO"` |
| P1-31 | tools/cors_scanner.py:79 | #32 | `_registrable` is an identity no-op (`return host`) that is never called | `def _registrable(host: str) -> str:` / `return host` |
| P1-32 | tools/cors_scanner.py:172 | #34 | both arms of the ternary evaluate to `MEDIUM`; the `if/else` is dead weight | `sev = MEDIUM if test.weakness in {...} else MEDIUM` |
| P1-33 | tools/dom_xss_harness.py:76 | #19 | `k not in names` membership test against the list being built in the same comprehension — O(n·m) | `names += [k for k in _split_fragment_params(parsed.fragment) if k not in names]` |
| P1-34 | tools/dom_xss_harness.py:162 | #34 | `except Exception: pass` silently swallows screenshot failures | `except Exception:  # noqa: BLE001` / `pass` |
| P1-35 | tools/eol_check.py:99 | #1 | pervasive `dict[str, Any]`/`list[dict[str, Any]]` on public signatures (82/99/125/185) | `def find_release(cycles: list[dict[str, Any]], version: str) -> dict[str, Any] | None:` |
| P1-36 | tools/h1_idor_scanner.py:33 | #9 | module-level mutable `FINDINGS = []` mutated by `flag()` from multiple sites | `FINDINGS     = []    # collected findings` |
| P1-37 | tools/h1_idor_scanner.py:36 | #1 | `gql(token, query, variables: dict = None) -> dict` — bare `dict` param and return | `def gql(token: str, query: str, variables: dict = None) -> dict:` |
| P1-38 | tools/h1_idor_scanner.py:470 | #34 | bare `except:` swallows all errors, coercing status to 0 | `except:` / `status = 0` |
| P1-39 | tools/h1_idor_scanner.py:134 | #8 | `token_a/token_b: str`, `report_id: str`, `user_id: str`, `program_handle: str` threaded as bare strings through ~13 functions (primitive obsession / data clump) | `def test_report_idor(token_a: str, token_b: str, report_id: str):` |
| P1-40 | tools/h1_idor_scanner.py:18 | #32 | `from typing import Optional` — `Optional` never used | `from typing import Optional` |
| P1-41 | tools/h1_mutation_idor.py:19 | #32 | `import urllib.parse` never used | `import urllib.parse` |
| P1-42 | tools/h1_mutation_idor.py:30 | #39 | comment narrates changelog ("this fallback used to disable verification outright…") + missing-doc reference | `# finding #9 — this fallback used to disable verification outright,` |
| P1-43 | tools/h1_mutation_idor.py:295 | #8 | hardcoded magic user-id string `"2617918"` embedded mid-function | `user_a_id = "2617918"` |
| P1-44 | tools/h1_oauth_tester.py:29 | #1 | `request(method, path, headers: dict = None, data: dict = None, extra_headers: dict = ...)` — three bare-`dict` params | `def request(method: str, path: str, headers: dict = None, data: dict = None,` |
| P1-45 | tools/h1_race.py:15 | #32 | `import sys` (15) and `from typing import Optional` (19) never used | `import sys` |
| P1-46 | tools/h1_race.py:29 | #32 | module-level `RESULTS = []` never read or appended (each test uses a local `responses`) | `RESULTS = []` |
| P1-47 | tools/hai_payload_builder.py:27 | #32 | `import sys` never used | `import sys` |
| P1-48 | tools/hai_payload_builder.py:52 | #32 | `build_report` parameter `method="sneaky"` is never referenced in the body | `def build_report(visible_text, hidden_injection, method="sneaky"):` |
| P1-49 | tools/hai_probe.py:10 | #32 | `import sys` never used | `import sys` |
| P1-50 | tools/hunt.py:562 | #34 | `generate_reports` is a wired-in no-op stub (only logs "removed" and returns 0) yet is still called at 755/859 | `log("warn", "report_generator.py has been removed. ...")` / `return 0` |
| P1-51 | tools/intel_engine.py:39 | #38 | ANSI color constants (`RED="\033[91m"` …) duplicated verbatim as module-level constants across intel_engine.py, learn.py, mindmap.py, token_scanner.py, validate.py | `RED    = "\033[91m"` |
| P1-52 | tools/intel_engine.py:18 | #32 | `from datetime import datetime, timezone` — neither used | `from datetime import datetime, timezone` |
| P1-53 | tools/intel_engine.py:48 | #1 | public functions return bare `dict`/`list[dict]` (48/148/227) | `def load_memory_context(memory_dir: str, target: str) -> dict:` |
| P1-54 | tools/jwt_scanner.py:84 | #13 | `confuse_rs256_to_hs256` body is a single forwarding call to `forge_hs256_with_key` | `return forge_hs256_with_key(token, public_key_pem, set_claims)` |
| P1-55 | tools/lead_board.py:137 | #32 | `STATUS_ICON` constant defined but never referenced (`show()` hardcodes emoji inline) | `STATUS_ICON = {"new": "•", "investigating": "🔬", "killed": "☠️ ",` |
| P1-56 | tools/learn.py:31 | #34 | `_SSL_CTX` set to `ssl.create_default_context()` then the `except ImportError` re-sets it to the identical value — no-op branch | `_SSL_CTX = ssl.create_default_context()` |
| P1-57 | tools/learn.py:130 | #1 | `fetch_url(url, headers: dict = None, data: bytes = None, ...) -> dict | None` bare-`dict` boundary | `def fetch_url(url: str, headers: dict = None, data: bytes = None, timeout: int = 10) -> dict | None:` |
| P1-58 | tools/mindmap.py:13 | #32 | `import sys` never used | `import sys` |
| P1-59 | tools/mindmap.py:226 | #11 | checklist type-map + sort + impact→badge logic duplicated between `build_checklist` (226-259) and `main` (342-362) | `type_map = {"website": WEBSITE_CHECKS, ...}` |
| P1-60 | tools/safe_http.py:67 | #1 | public `safe_urlopen(..., **kwargs)` takes opaque `**kwargs` (also `_one_hop`, 35) | `def safe_urlopen(req: urllib.request.Request, timeout: float = 10, max_redirects: int = 5, **kwargs):` |
| P1-61 | tools/scope_checker.py:74 | #39 | two consecutive comment lines describe behavior with no accompanying code ("urlparse handles this, but be safe" / "should already exclude port") | `# Strip port if present (urlparse handles this, but be safe)` |
| P1-62 | tools/sneaky_bits.py:83 | #39 | `tag_encode` kept "for reference" with docstring/comment narrating it was "PATCHED on Hai" — history narration on retained dead-for-reference code | `"""Encode text using Unicode Tags (U+E0000 range) - PATCHED on Hai."""` |
| P1-63 | tools/target_selector.py:45 | #34 | `except (subprocess.TimeoutExpired, json.JSONDecodeError, Exception)` — the broad `Exception` makes the listed classes redundant, then only prints and continues (also 64) | `except (subprocess.TimeoutExpired, json.JSONDecodeError, Exception) as e:` |
| P1-64 | tools/target_selector.py:224 | #19 | `identifier not in domains` membership test against a list built in the same loop — O(n·m) | `if "." in identifier and identifier not in domains:` |
| P1-65 | tools/token_scanner.py:584 | #19 | `_deduplicate` scans a `seen` list inside the finding loop — O(n²) dedupe | `for f in findings:` / `for s in seen:` |
| P1-66 | tools/token_scanner.py:548 | #41 | pattern regex `re.compile`d per file inside the scan loop instead of compiling the table once — quadratic recompiles in the token-scan hot path | `compiled = re.compile(regex)` |
| P1-67 | tools/validate.py:301 | #1 | gate functions return `tuple[bool, dict]` with opaque `dict` (301/354/406/462); `load_json_file` returns bare `dict` (237) | `def gate1_is_real(vuln_type: str = "") -> tuple[bool, dict]:` |
| P1-68 | tools/validate.py:36 | #39 | comment narrates changelog ("this fallback used to disable verification outright, silently exposing every request…") | `# ... this fallback used to disable verification outright,` |
| P1-69 | tools/waf_response_analyzer.py:349 | none | `_insecure_ssl_context()` now returns a verifying context; the name lies about behavior (docstring admits "kept to avoid touching call sites") — a naming/behavior contract violation #40 doesn't cover (not is_/has_/plural) | `def _insecure_ssl_context() -> ssl.SSLContext:` |
| P1-70 | tools/waf_response_analyzer.py:346 | #9 | module-level `_INSECURE_CTX` cache mutated via `global` inside `_insecure_ssl_context` | `_INSECURE_CTX: ssl.SSLContext | None = None` |
| P1-71 | tools/waf_response_analyzer.py:224 | #12 | hand-rolled order-preserving dedup (seen-set + out-list loop) reimplements `dict.fromkeys` | `seen: set[str] = set()` / `out: list[str] = []` / `for h in hits:` |
| P1-72 | tools/waf_response_analyzer.py:55 | #1 | `VENDORS: list[dict[str, Any]]` table and `dict[str, Any]` returns pervasive (360/436/452/543/713) | `VENDORS: list[dict[str, Any]] = [` |
| P1-73 | tools/waf_encoder.py:159 | #12 | `mysql_version_comment` re-implements the `/*!50000{kw}*/` keyword-wrapping already done by `sql_comment_inject` (83-90) | `def mysql_version_comment(...)` |
| P1-74 | tools/zendesk_idor_test.py:27 | #9 | module-level config `SUBDOMAIN/EMAIL/API_TOKEN/BASE_URL/AUTH` mutated from `configure_from_args` via `global` | `global SUBDOMAIN, EMAIL, API_TOKEN, BASE_URL, AUTH` |
| P1-75 | tools/zendesk_idor_test.py:156 | #32 | parameter `my_user_id` never used inside `test_ticket_idor` | `def test_ticket_idor(my_user_id):` |
| P1-76 | tools/zendesk_idor_test.py:310 | #34 | `except Exception: pass` pure swallow (also 136 swallow-and-print) | `except Exception:` / `pass` |
| P1-77 | tools/zero_day_fuzzer.py:28 | #32 | `import time` (28) unused; `parse_qs`/`urlencode`/`urlunparse` (31) imported but only `urlparse` used | `import time` |
| P1-78 | tools/_spray_oauth.py:56 | #38 | `USER_AGENT = "Mozilla/5.0 ... Bug-Bounty-Research"` duplicated verbatim as a module constant in _spray_http_form.py:57 (and echoed inline across the h1_* modules) | `USER_AGENT = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Bug-Bounty-Research"` |
| P1-79 | tools/_spray_oauth.py:59 | #13 | `env()` is a one-line forwarder over `os.environ.get`, duplicated in _spray_http_form.py:60 | `def env(name: str, default: str = "") -> str:` / `return os.environ.get(name, default)` |
| P1-80 | tools/_spray_oauth.py:43 | #11 | the certifi/SSL-context fallback block (with its history-narration comment) is duplicated across _spray_oauth.py, _spray_http_form.py, validate.py, learn.py, h1_mutation_idor.py | `# certifi/SSL fallback (duplicated block)` |
| P1-81 | tools/_spray_http_form.py:48 | none | comment says "fallback to unverified only if certifi is unavailable" and this branch actually sets `check_hostname=False; verify_mode=CERT_NONE` — diverges from the hardened siblings (behavior/security inconsistency, not a clean rule hit) | `SSL_CTX.check_hostname = False` / `SSL_CTX.verify_mode = ssl.CERT_NONE` |
| P1-82 | tools/_spray_oauth.py:145 | #9 | `AUDIT_LOG_PATH` used as an implicit module global assigned inside `main()` via `global`, read by `audit()` (same pattern in _spray_http_form.py:198) | `global AUDIT_LOG_PATH` |
| P1-83 | tools/auth_session.py:79 | #13 | `add_cookie`/`add_bearer`/`add_api_key` (79-89) are thin forwarders to `add_header` that only prepend a fixed prefix | `def add_cookie(self, cookie: str) -> None:` / `self.add_header(f"Cookie: {cookie}")` |
| P1-84 | tools/dashboard.py:320 | #32 | `phase_skip` is defined and exercised only by a test — never called by `TailParser` or the CLI in the prod tree | `def phase_skip(self, key: str, reason: str = ""):` |
| P1-85 | tools/oob_listener.py:42 | #1 | `oob_payloads(...) -> dict[str, list[dict]]` and `correlate(interactions: list[dict], payloads: dict[str, list[dict]])` — nested opaque maps at the boundary | `def oob_payloads(domain: str, classes: list[str] | None = None) -> dict[str, list[dict]]:` |

## Phase 2 — audit finding verdicts

1262 findings judged (real 1130 / fp 132). Homogeneous families are grouped into
one row with an instance count + named spot-checks (>=10 where large); split
families get a real-row and an fp-row whose counts sum to the family total. Judged
the site, not the wording.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| memory/schemas.py:83,126,149,265; tools/scope_checker.py:45; tools/auth_session.py:55 | #2 | proved | real (6/6) | isinstance where the param is annotated the same type being checked — type-sound redundancy; the guard is dead under the declared annotation (the honest fix widens the annotation to object/Mapping, since these validate untrusted input) |
| agent.py:578,1665; tools/eol_check.py:99,125; memory/schemas.py:81,124; tools/safe_http.py:67; tools/h1_oauth_tester.py:29; memory/audit_log.py:33; brain.py:762 (+79 more) | #1 | heuristic | real (90/90) | genuine weak/dishonest boundary types: bare dict/list, contains-Any, opaque **kwargs, and str/dict=None implicit-Optional defaults; strongest family |
| brain.py:270(cc229),578(cc194),2486(186),551(110); engine.py:222(82); agent.py:1318(53); tools/lead_board.py:288(49); tools/waf_response_analyzer.py:543(46) (+84 more) | #23 | heuristic | real (92/92) | cognitive complexity genuinely over threshold (branchy dispatch/validator/provider chains); advisory, low-end (=15) marginal but above the tool's own bar |
| engine.py:66 (ok/info/warn/err); learn.py:51/server.py:49/validate.py:142 (_escape_graphql_string x3); _spray_oauth.py:63 (env_required); h1_mutation_idor.py:172 / target_selector.py:307 / breach_checker.py:215 (main argparse); tests/test_scope_checker.py:13 (x16); h1_idor_scanner.py:259; memory/schemas.py:83 (+495 more) | #11 | indexed | real (506/506) | structural clones; first copies exempt by design, so each reported instance is a genuine 2nd+ duplicate (332 prod / 174 test); tiny-helper x2 pairs are low-value but AST-identical |
| docs/README.md:11 (payloads.md) | #28 | indexed | real (1) | a repo doc links a sibling doc that does not exist |
| OPENCODE.md:31; commands/bypass-403.md:69-71; commands/cloud-recon.md:41; skills/credential-attack/SKILL.md:8; skills/bug-bounty/SKILL.md:1132; web3/36-solidity-audit-mcp.md:240; docs/TODOS.md:77; tests/test_bypass_403_review_fixes.py:1 (+62 more) | #28 | indexed | fp (70) | detector treats runtime-OUTPUT filenames (bypass_hits.txt, arjun.json, gitleaks.json), user-created configs (opencode.json), external wordlists (rockyou.txt) and in-prose target-repo examples (package.json/setup.py) as broken repo-path refs — none are this repo's files |
| agent.py:119,121; brain.py:649; engine.py:61; tools/intel_engine.py:39-42; tools/h1_idor_scanner.py:29; tools/h1_race.py:28 (+59 more) | #38 | indexed | real (66/66) | module-level literals duplicated across >=3 modules — ANSI codes (10/7/5/4/3 modules) and https://hackerone.com (4 modules) |
| engine.py:96 (COMMAND_ALIASES); agent.py:99,1563-1566; tools/cors_scanner.py:79; brain.py:1744,1769,1936,1111; agent.py:126; tools/lead_board.py:137; scripts/dork_runner.py:9,12; tools/banner.py:22; tools/hunt.py:35 (+47 more) | #32 | indexed | real (62/62) | genuine dead symbols/imports/unused params; IDX correctly did NOT flag hunt._resolve_* (used cross-module by agent.py), confirming soundness |
| brain.py:1 (module docstring ineffective — placed after `from __future__`); brain.py:2486; tools/hunt.py:760; agent.py:1760; engine.py:699; tools/validate.py:834; tools/h1_mutation_idor.py:105 (+51 more) | #29 | heuristic | real (58/58) | heavy entry points with no cost docstring; brain:1 is a real subtle bug; marginal at ~30-line low end; advisory |
| agent.py:794; tools/scope_checker.py:133; tools/safe_http.py:35; tools/crlf_scanner.py:45; brain.py:970,2307,1694 (history); tools/sneaky_bits.py:139,144; tools/hunt.py:41,574,825; tools/intel_engine.py:334,376 (+18 more) | #39 | heuristic | real (32) | genuine comment restatement, history narration, and prose-outweighs-function |
| tools/dashboard.py:498 (3v2); tools/dom_xss_harness.py:39 (2v1); tools/sast_scan.py:48 (2v1); mcp/hackerone-mcp/server.py:31; tests/test_auth_session.py:189; tests/test_recon_adapter.py:279; tests/test_token_scanner.py:462; tools/h1_race.py:172; tools/llm_redteam.py:81 | #39 | heuristic | fp (9) | trivial 1-2 line funcs whose docstring merely "outweighs" one line, and section-divider banners flagged as code-restatement |
| agent.py:517 (ToolDispatcher fi25); brain.py:752 (Brain fi14); tools/token_scanner.py:476 (fi31); agent.py:578 (dispatch fi20); tools/dashboard.py:228 (+32 more) | #27 | indexed | real (37/37) | high-fan-in symbols in large modules; report-only; color/enum-member constants are low-value but the metric holds |
| tools/target_selector.py:307; tools/h1_oauth_tester.py:182; engine.py:642; tools/hunt.py:572 (0-crossing clean seams); agent.py:1330; brain.py:2520 (+31 more) | #17 | heuristic | real (37/37) | liveness necks are real structural seams; lowest-value rule (REPORT, never-gate), advisory |
| tests/test_false_positives.py:79+ (monkeypatch x14); tools/validate.py:301 (vuln_type.lower); tools/dashboard.py:154 (title.upper); tools/jwt_scanner.py:117,99,61,72 (token.split); tools/recon_adapter.py:229; agent.py:983 (+15 more) | #15 | indexed | fp (34/34) | over-fires: monkeypatch/tmp_path are pytest fixtures (not narrowable to a protocol) and the flagged params are already-minimal str primitives, not rich wallet objects with unused attrs |
| agent.py:578,1072,1111; tools/breach_checker.py:75; tools/oob_listener.py:113; tools/sast_scan.py:69; memory/schemas.py:56,62,75; tools/scope_checker.py:24,93 (+21 more) | #10 | indexed | real (32/32) | over-constrained container params; oracle-verified widening (list/dict → Iterable/Mapping) with no new errors |
| tools/h1_idor_scanner.py:36 (variables, 22 sites); tools/h1_mutation_idor.py:54 (20); tools/safe_http.py:67 (max_redirects, 18); tools/zero_day_fuzzer.py:65 (data/timeout, 14); memory/audit_log.py:133 (+11 more) | #37 | indexed | real (16/16) | params never overridden across prod call sites; a few (timeout) are testability seams but mechanically unexercised in prod |
| agent.py:453,899,972,1139,1314; brain.py:280,810,1703,2077,2155; tools/dom_xss_harness.py:162; tools/zendesk_idor_test.py:136,310; tools/h1_idor_scanner.py:438 (+11 more) | #34 | heuristic | real (25/25) | broad except that only pass/print — #34 explicitly targets swallow-with-print |
| tools/sneaky_bits.py:149; tools/hai_probe.py:180; tools/auth_session.py:73; tools/zero_day_fuzzer.py:591,509; tools/h1_idor_scanner.py:113; memory/schemas.py:370 (+5 more) | #16 | heuristic | real (12/12) | pure computation with the state-write confined to the final statements — genuine mutation tails |
| memory/audit_log.py:106,53,227; tools/scope_checker.py:104,93; tools/auth_session.py:227,79,83,87; memory/pattern_db.py:139 | #22 | heuristic | real (10) | methods using only the class's public interface — genuine free-function candidates |
| tools/auth_session.py:158 (from_sources) | #22 | heuristic | fp (1) | classmethod factory — legitimately a member though it uses only the public constructor |
| tools/multipart_mutator.py:40; tools/h1_idor_scanner.py:134,198; brain.py:460,2031,863; tools/learn.py:130; agent.py:1198 | #14 | indexed | real (8) | genuine parameter data-clumps threaded through several signatures |
| tests/test_agent_dispatcher_hardening.py:58; tests/test_hackerone_mcp.py:18; tests/test_scope_checker.py:138 | #14 | indexed | fp (3) | the recurring triples are pytest fixture-injection params (fake_hunt/memory/tmp_path), not a data clump wanting a type |
| brain.py:551; mcp/hackerone-mcp/server.py:227,179; memory/audit_log.py:106; tools/hai_probe.py:85,105; tools/h1_mutation_idor.py:40; brain.py:1223; tools/scope_checker.py:38 | #6 | indexed | real (9/9) | accessor-named (get_/list_/count_/is_) methods with real io effects (HTTP fetches, file reads, stderr writes) |
| engine.py:67 (info, 11 sites); engine.py:69 (err, 7); tools/lead_board.py:219 (3); tools/hai_payload_builder.py:52 (2) | #5 | indexed | real (4) | unencoded invariants established across multiple prod call sites — legitimate lift proposals |
| tools/lead_board.py:288 (1 site); tools/validate.py:118 (1 site); tests/test_agent_dispatcher_hardening.py:39 (1 site) | #5 | indexed | fp (3) | single-call-site lifts — insufficient evidence / the over-narrow-lift FP the design explicitly warns against |
| engine.py:504 (cmd_triage); memory/audit_log.py:358,362 (record_failure/success); agent.py:1103 (close); tests/test_agent_dispatcher_scope.py:39 | #13 | indexed | real (5) | body is a single pass-through forwarding call that adds nothing |
| tools/h1_idor_scanner.py:80 (make_gid); tools/jwt_scanner.py:43 (b64url_encode) | #13 | indexed | fp (2) | not pure pass-throughs — make_gid injects the gid://hackerone/... format, b64url_encode strips padding; both add domain logic |
| agent.py:501 (recent_observations) | #40 | heuristic | real (1) | noun-phrase name reads as a collection accessor but returns a joined str |
| agent.py:880,955,906,934,918 (_summarize_*/_read_*); memory/rotation.py:117 (purge_backups); tools/multipart_mutator.py:32 (_part_headers) | #40 | heuristic | fp (7) | verb-phrase names (summarize/read/purge X) whose scalar return is a rendered summary/count — no reader is misled; _part_headers returns the serialized header block (naturally one bytes value) |
| tools/dashboard.py:308,329,288,320 (phase_* -> _emit_plain) | #25 | indexed | real (4) | delegation to a differently-stemmed helper — the call chain is not name-greppable from the caller |
| tools/auth_session.py:264 (__str__ -> describe) | #25 | indexed | fp (1) | __str__ is a standard dunder thin-delegator; not a navigability defect |
| tools/intel_engine.py:265; tools/learn.py:318; tools/mindmap.py:245; tests/test_false_positives.py:70 | #20 | heuristic | real (4/4) | same non-trivial lambda body repeated within a module — name it |
| brain.py:752; tools/dashboard.py:228; tools/zero_day_fuzzer.py:107; memory/audit_log.py:152; tools/auth_session.py:43 | #21 | heuristic | real (5/5) | same expression pattern recurs across several methods of one class — encapsulation candidate |
| tools/waf_response_analyzer.py:556; tools/hai_probe.py:126; memory/audit_log.py:310 | #18 | heuristic | real (3/3) | functions narrate multiple labeled phases in prose — each phase is a function boundary |
| agent.py:419 (TOOL_NAMES) | #26 | heuristic | real (1) | membership assembled by a set-comprehension over a table — a reader must execute it to know the members |
| engine.py:54 (CONFIG); tools/dashboard.py:54 (CURSOR_UP) | #26 | heuristic | fp (2) | single computed Path/escape-string constants — there are no "members" to hide; rule misapplied |
| demo/app.py:146 (do_GET) | #33 | heuristic | real (1) | verified: bare return at line 209 alongside value returns — callers cannot rely on the result |
| tools/h1_idor_scanner.py:134 (report_id in 8 sigs) | #8 | indexed | real (1) | genuine primitive-obsession / NewType candidate |
| memory/rotation.py:27 (rotate) | #7 | heuristic | real (1) | docstring narrates a caller-must-hold-the-lock precondition — encode it |
| agent.py:1273 (_pick_tool_capable_model) | #19 | heuristic | real (1) | pref in available (list) inside a loop — O(n*m), use a set |

## Phase 3 — reconciliation

covered 44 · detector-miss 34 · threshold-miss 4 · inventory-gap 3 (of 85).

| P1 id | rule | class | note |
|-------|------|-------|------|
| P1-1 | #32 | covered | engine.COMMAND_ALIASES flagged |
| P1-2 | #13 | covered | engine.cmd_triage flagged |
| P1-3 | #32 | covered | agent:99 Brain/BRAIN_SYSTEM/MODEL_PRIORITY all flagged |
| P1-4 | #32 | covered | agent:1564-1566 StructuredTool/inspect/AIMessage flagged |
| P1-5 | #39 | covered | agent:794 prose-outweighs flagged |
| P1-6 | #39 | detector-miss | dispatch()'s inline history-narration comment block (584) not flagged; #39 caught length/restatement, not this comment |
| P1-7 | #28 | detector-miss | #28 scans .md docs only; the SECURITY-REVIEW-2026-08-22.md refs in .py docstrings/comments are not scanned |
| P1-8 | #1 | covered | agent.run_agent_hunt bare-dict return flagged |
| P1-9 | #1 | covered | agent.dispatch args:dict flagged (also #10) |
| P1-10 | #28 | detector-miss | same as P1-7 — brain.py:2042 docstring doc-ref not scanned |
| P1-11 | #39 | covered | brain:2307 history narration in the auto_triage docstring flagged |
| P1-12 | none | inventory-gap | EXPLOIT_ROUND_CAP=6 vs hardcoded range(6): no rule covers intra-module magic-number duplication |
| P1-13 | #1 | detector-miss | #1 flags signature params/returns, not the bare-dict annotation on a class-level variable (_TOOL_INSTALL) |
| P1-14 | #32 | covered | audit_log SchemaError unused flagged |
| P1-15 | #1 | covered | audit_log.log entry:dict flagged |
| P1-16 | #13 | covered | audit_log.record_failure forward flagged |
| P1-17 | #38 | threshold-miss | timestamp format repeats in 4 factories of ONE module; #38 requires >=3 MODULES |
| P1-18 | #1 | covered | schemas validators bare-dict flagged |
| P1-19 | #39 | detector-miss | docstring's self-cancelling "'hunt' or 'hunt'" distinction is semantic; #39 detects restatement/history/length only |
| P1-20 | #1 | covered | pattern_db.save entry:dict flagged |
| P1-21 | #34 | detector-miss | no-op except-ImportError re-assign isn't swallow-with-pass/print; #34 didn't fire |
| P1-22 | #12 | detector-miss | rule #12 (idiom catalog) produced zero findings; the hand-rolled argparse loop not caught |
| P1-23 | #32 | covered | dork_runner time+random unused flagged |
| P1-24 | #9 | detector-miss | rule #9 (shared mutable module state) produced zero findings |
| P1-25 | #39 | detector-miss | docstring names wrong env var (DEMO_HOST vs APP_HOST) — semantic, not a #39 class |
| P1-26 | #39 | detector-miss | comment contradicts behavior — not restatement/history/length |
| P1-27 | #34 | covered | demo.app._banner broad except flagged |
| P1-28 | #32 | covered | banner Iterable unused flagged |
| P1-29 | #32 | covered | cors_scanner field unused flagged |
| P1-30 | #32 | detector-miss | LOW is a tuple-unpack target; not flagged dead (vulture-style skips unpack targets) |
| P1-31 | #32 | covered | cors_scanner._registrable flagged |
| P1-32 | #34 | detector-miss | both-arms-identical ternary (dead branch) is not a #34 class |
| P1-33 | #19 | detector-miss | #19 fired only once (agent:1273); dom_xss list-membership-in-loop missed |
| P1-34 | #34 | covered | dom_xss:162 broad except flagged |
| P1-35 | #1 | covered | eol_check dict[str,Any] flagged |
| P1-36 | #9 | detector-miss | rule #9 produced zero findings |
| P1-37 | #1 | covered | h1_idor.gql bare-dict flagged |
| P1-38 | #34 | covered | h1_idor test_graphql_csrf broad except flagged (438, same block) |
| P1-39 | #8 | covered | h1_idor report_id flagged (also #14) |
| P1-40 | #32 | covered | h1_idor Optional unused flagged |
| P1-41 | #32 | detector-miss | import urllib.parse verified unused but not flagged (submodule-import gap) |
| P1-42 | #39 | detector-miss | changelog comment ("used to disable verification outright") not caught here |
| P1-43 | #8 | detector-miss | hardcoded magic user-id "2617918" — no rule for inline magic constants |
| P1-44 | #1 | covered | h1_oauth request bare-dict params flagged |
| P1-45 | #32 | covered | h1_race Optional unused flagged (site covered; sys not separately flagged) |
| P1-46 | #32 | covered | h1_race RESULTS unused flagged |
| P1-47 | #32 | covered | hai_payload_builder sys unused flagged |
| P1-48 | #32 | covered | hai_payload_builder.build_report method param unused flagged |
| P1-49 | #32 | covered | hai_probe sys unused flagged |
| P1-50 | #34 | covered | hunt.generate_reports site flagged (as #32 unused param) — same dead-weight site surfaced |
| P1-51 | #38 | covered | intel_engine color literals across 5 modules flagged |
| P1-52 | #32 | covered | intel_engine datetime/timezone unused flagged |
| P1-53 | #1 | covered | intel_engine bare-dict returns flagged |
| P1-54 | #13 | detector-miss | confuse_rs256_to_hs256 forwards with modified args; #13 didn't fire (it fired for b64url_encode instead) |
| P1-55 | #32 | covered | lead_board STATUS_ICON flagged |
| P1-56 | #34 | detector-miss | no-op SSL except-ImportError re-assign; #34 didn't fire |
| P1-57 | #1 | covered | learn.fetch_url bare-dict flagged |
| P1-58 | #32 | covered | mindmap sys unused flagged (line 14) |
| P1-59 | #11 | covered | mindmap clone findings cover the checklist/main duplication |
| P1-60 | #1 | covered | safe_http **kwargs flagged |
| P1-61 | #39 | detector-miss | comment lines describing nothing (no annotated code) — not a #39 class |
| P1-62 | #39 | detector-miss | "PATCHED on Hai / kept for reference" history narration on retained dead code not caught |
| P1-63 | #34 | detector-miss | verified: target_selector:45 broad except(...,Exception) prints+continues — should match #34, missed |
| P1-64 | #19 | detector-miss | target_selector list-membership-in-loop missed |
| P1-65 | #19 | detector-miss | token_scanner O(n^2) dedupe missed |
| P1-66 | #41 | detector-miss | rule #41 (perf catalog) produced zero findings (needs committed micro-bench) |
| P1-67 | #1 | threshold-miss | dict nested inside tuple[bool, dict] return; #1 flags top-level bare dict, not nested |
| P1-68 | #39 | detector-miss | changelog comment in validate not caught |
| P1-69 | none | inventory-gap | _insecure_ssl_context name-lies (now verifying); #40's is_/has_/plural scope doesn't cover it |
| P1-70 | #9 | detector-miss | rule #9 produced zero findings |
| P1-71 | #12 | detector-miss | rule #12 produced zero findings (hand-rolled dedup) |
| P1-72 | #1 | covered | waf_response_analyzer Any-boundary flagged (classify 546/547, calibrate 452, diff_bodies 713) |
| P1-73 | #12 | detector-miss | rule #12 produced zero findings (mysql_version_comment reimpl) |
| P1-74 | #9 | detector-miss | rule #9 produced zero findings |
| P1-75 | #32 | detector-miss | verified: zendesk my_user_id param unused but not flagged |
| P1-76 | #34 | covered | zendesk:310 broad except flagged |
| P1-77 | #32 | covered | zero_day time + parse_qs/urlencode/urlunparse unused flagged |
| P1-78 | #38 | threshold-miss | USER_AGENT is module-level in only 2 modules; #38 requires >=3 |
| P1-79 | #13 | detector-miss | env() one-line forwarder not caught (#11 flagged the sibling env_required as a clone) |
| P1-80 | #11 | covered | _spray_oauth SSL-fallback block among its clone findings |
| P1-81 | none | inventory-gap | _spray_http_form verify-disabled SSL inconsistency mapped none |
| P1-82 | #9 | detector-miss | rule #9 produced zero findings |
| P1-83 | #13 | covered | auth_session add_cookie/bearer/api_key flagged under #22 (same thin-method site) |
| P1-84 | #32 | detector-miss | phase_skip is prod-unused but test-referenced; #32 counts test refs, so not flagged dead |
| P1-85 | #1 | threshold-miss | oob_payloads returns fully-parameterized dict[str,list[dict]] — under #1's bare-dict cutoff |
