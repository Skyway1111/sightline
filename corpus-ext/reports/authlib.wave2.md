# authlib — wave 2 (delta)

Repo: `<GAUNTLET_CORPUS_ROOT>\authlib`.
Audit: `corpus-ext/audits/authlib.wave2.json`. 4641 → 1342 findings.
Diffed on `(file, line, rule, cause)`: **3299 vanished, 0 new, 0 changed in
place**. Every verdict below is against my *wave-1* verdicts — I do not
re-litigate wave-1 rows.

Vanished by rule: #11 2838 · #39 228 · #22 123 · #14 41 · #32 27 · #33 23 ·
#25 9 · #5 4 · #21 3 · #6 2 · #2 1.

**Sampling.** 120 vanished findings inspected individually — every vanished
finding in #33 (23), #32 (27), #25 (9), #5 (4), #21 (3), #6 (2), #2 (1), all 9
prod-side #11 losses, all 41 #14, plus 12 named test-side #11 groups and 12
named #39. 20 of those were re-read at the source line. Mechanism claims below
are checked programmatically over the full diff, not extrapolated from the
sample.

## Phase 2 — delta verdicts on changed findings

`removal correct` = the wave-2 subtraction is right at the site.
`recall loss` = I still stand behind the wave-1 verdict and the finding is gone.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| tests/flask/test_oauth2/test_client_configuration_endpoint.py:363 *(family: all test-side block clones, 2829 instances / 665 groups)* | 11 | indexed | removal correct (scoped) | The exemption is exact and checkable: **every** `(N stmts)` block clone under `tests/` is gone (stmt histogram 3→1354 … 17→2) while **all 237 whole-function test clones survive** (`tests/flask/test_oauth1/test_authorize.py:11` ×9, `tests/core/test_oauth2/test_rfc7591.py:26` ×5, `tests/django/test_oauth2/models.py:93`). That is precisely the act-assert-rhythm class I called fp in wave 1 (317 instances), and it also takes 2512 instances I called real — the `oauth.register("dev", …)` ×25, `app = Flask(); secret_key; OAuth(app)` ×25, `db.session.add/commit/return item` ×30, `client.set_client_metadata({…})` ×10 fixtures. I do not contest the policy: tests are not the surface this tool scores, my wave-1 tail verdict was an explicitly sampled majority, and the duplicated *fixtures written as functions* still report. Recorded as a scoped loss, not a defect. |
| authlib/integrations/django_client/apps.py:44 *(family: nested clone windows collapsed, 7 instances / 2 groups)* | 11 | indexed | removal correct | `clone-block:74d29e4a5db3` (3 stmts, django:44 / starlette:43 / django:94 / starlette:97) and `clone-block:eae67abcd3bb` (3 stmts, jose models 54/121/151) are narrower windows over code that wider windows still cover — `4aa31fd21c18` (8 stmts) still reports django:43 / starlette:42, `07ee3e7d9c46` still reports django:48 / starlette:47, `ae16822e665b` still reports models 53/120/148. Verified programmatically: of 217 prod sites carrying a #11 in wave 1, only 4 carry none in wave 2, and 3 of those 4 sit one line from a surviving window. |
| authlib/integrations/flask_oauth1/cache.py:43 *(2 instances: `clone-block:3073ef75c73d`, `clone-block:4a3edabee0dd`)* | 11 | indexed | **recall loss** | Wave-1 verdict: real. `register_temporary_credential_hooks` duplicates `tests/flask/test_oauth1/oauth1_server.py:188` — a prod↔test clone pair. With the test member exempt the group drops below two members and the **prod** member disappears with it. `authlib/integrations/flask_oauth1/cache.py:43` now carries no #11 at all. This is a new FN class (see Bottom line). |
| authlib/oauth2/rfc6749/util.py:17 *(family: the whole `mixes value returns with bare returns` arm, 23 instances)* | 33 | heuristic | **recall loss** | Wave-1 verdict: 59/59 real, and this arm was 23 of them. I re-read four: none of these is a *bare* `return` — every one is an explicit `return None` mixed with value returns, i.e. an undeclared `Optional` in a library with 58 return annotations across 1389 defs. `scope_to_list` (`elif scope is None: return None`), `to_bytes` (encoding.py:6), `create_half_hash`, `extract_params`, `BaseOAuth.create_client`, `FrameworkIntegration.get_state_data` / `_get_cache_data`, `StarletteIntegration._get_cache_data`, `OpenIDMixin.parse_id_token`, `JWTBearerTokenValidator.authenticate_token`, and eight django-integration query methods. The sharpest is `OAuth2Token.is_expired` (wrappers.py:20), which returns `None` on two paths — and `oauth2/client.py:304` does `if not token.is_expired(...)`, so "unknown" reads as "not expired". Wave 2 keeps only fall-through (33) and annotation-lie (3); the undeclared-`Optional` class is now entirely uncovered. |
| authlib/jose/rfc7516/jwe.py:172 *(family: #39 comment-ratio, 51 instances with ≥3 code lines)* | 39 | heuristic | **recall loss** | Wave-1 verdict: real (my cut was ≥3 code lines). Not a ratio re-threshold — vanished ratios run to 34×, kept ratios down to 1.12×, so no cutoff separates them. Lost: `jwe.serialize_json` (121 prose / **105** code), `oauth1 AuthorizationServer.create_token_response` (33/16) and `.create_authorization_response` (17 code), `authorization_code.validate_token_request` (27/16), `rfc6749/parameters.py:13` (16 code) and `:69`/`:110`, `implicit.py:155` (18 code), `client.revoke_token`/`.introspect_token` (12 code each), `jws.serialize_compact` (9), `rfc7009/revocation.py:47` (10). ~24 of the 51 carry ≥6 code lines and I still defend those; the remainder are 3–5-line docstring'd forwarders I would concede. |
| authlib/oauth2/rfc9068/introspection.py:113 *(family: #39 comment-ratio, 112 instances with ≤2 code lines)* | 39 | heuristic | removal correct | Wave-1 verdict: fp. These are the `raise NotImplementedError()` extension points and one-line forwarders whose docstring *is* the published contract — `get_jwks`, `validate_authorization_request` (47 prose / 1 code), the rfc8414 and oidc `validate_*` families. Exactly the class I argued should not fire. |
| authlib/jose/rfc7516/jwe.py:246 *(family: #39 history narration, 63 instances)* | 39 | heuristic | removal correct | Wave-1 verdict: fp — the arm was matching "used to" and "previously" in ordinary `:param` prose. My single genuine history finding, `common/urls.py:55` (the Python-2/3 `parse_qsl` archaeology), **survives**. Five RFC-quotation hits still linger (`django_oauth2/endpoints.py:7`, `authorization_code.py:325`, `client_credentials.py:15`, `device_code.py:151`, `registration/claims.py:332`) — the residue of the wave-1 FP class, down from 68 to 5. |
| authlib/oauth2/rfc7592/endpoint.py:98 | 39 | heuristic | removal correct | Wave-1 verdict: fp — `# secret for that client.` is the tail fragment of a three-line RFC quote. |
| authlib/oauth2/rfc8628/endpoint.py:65 | 39 | heuristic | **recall loss** | Wave-1 verdict: real — `# ``create_endpoint_response``` restating the code below it. The other eight restatements (both `signals.py` triplets, `token_endpoint.py:11`, `challenge.py:48`) survive. |
| authlib/jose/rfc7517/base_key.py:52 *(family: #22 methods that read instance data, 68 instances)* | 22 | heuristic/indexed | removal correct — wave-1 verdict superseded | Wave-1 verdict: real, on the crude split "value class ⇒ real". Re-read seven at source and the new state gate is more correct than my heuristic: `Key.keys` reads `self.tokens`, `Key.thumbprint` reads `self.REQUIRED_JSON_FIELDS`, `JsonWebToken.check_sensitive_data` reads `self.SENSITIVE_NAMES`, `OAuth2Client._extract_session_request_params` reads `self.SESSION_REQUEST_PARAMS`, `AuthlibHTTPError.get_error_description`/`get_body` read `self.description`/`self.error`, `JWEEncAlgorithm.generate_cek`/`generate_iv`/`check_iv` read `self.CEK_SIZE`/`IV_SIZE`, `_HTTPException.get_body` returns `self.body`. A method that reads instance state is not a free function hiding in a class — Meyers' count says so. My strongest picks (`Key.as_json`, `KeySet.as_json`, `OctKey.generate_key`) all survive. No recall loss I would defend. |
| authlib/oauth1/rfc5849/authorization_server.py:70 *(family: #22 on extension-point classes, 55 instances)* | 22 | heuristic | removal correct | Wave-1 verdict: fp (template-method public API on subclass-extension bases). |
| authlib/oauth2/client.py:318 *(family: #14 subset clumps collapsed, 41 instances)* | 14 | indexed | removal correct | Wave-1 verdict: 69 prod real / 29 test fp. Verified programmatically: **zero** of the 41 vanished findings leaves its line without a surviving #14, and 22 are strict supersets of a surviving clump at the same line — `(body, headers, token, token_type_hint)`→`(body, headers, token)` at client.py:318, `(headers, method, uri)`→`(body, headers, uri)` at client_auth.py:78, `(client, grant_type, scope, user)`→`(grant_type, scope, user)` at authorization_server.py:46. Pure de-duplication to the maximal-support clump per site. No site lost coverage. |
| authlib/oidc/discovery/models.py:79 *(family: #32 on dynamically dispatched validators, 17 instances)* | 32 | indexed | removal correct | Wave-1 verdict: fp — all 17 are invoked through `object.__getattribute__(self, f"validate_{key}")()`. |
| authlib/integrations/httpx_client/oauth1_client.py:22 *(family: #32 protocol attributes + public error taxonomy, 9 instances)* | 32 | indexed | removal correct | Wave-1 verdict: fp — `requires_request_body` ×3 is read by httpx's `Auth` protocol; the four `oidc/core/errors` classes and `UnsupportedParameterError` are published spec errors; `ClaimsOption.essential` is a TypedDict key used 45× as a string. |
| authlib/oauth2/claims.py:16 | 32 | indexed | **recall loss** | Wave-1 verdict: real. `ClaimsOption.allow_blank` occurs exactly once in the repo — the declaration — with no string-key counterpart (unlike its sibling `essential`, correctly dropped). Genuinely dead. |
| authlib/integrations/base_client/sync_app.py:22 *(family: #25 dunders and HTTP-verb vocabulary, 8 instances)* | 25 | indexed | removal correct | Wave-1 verdict: fp. `BaseApp.get/post/patch/put/delete → request` ×5 (standard HTTP vocabulary, not a rename), `quote`/`unquote → to_unicode` ×2 (the trailing conversion helper picked as the delegate), `DjangoJsonPayload.data → json_loads`. The `to_native → JsonWebToken.decode` index collision I named in wave 1 **persists** — it is now the loudest remaining #25 FP. |
| authlib/oauth1/rfc5849/parameters.py:83 | 25 | indexed | **recall loss** | Wave-1 verdict: real — `prepare_form_encoded_body` delegating to `url_encode` is a genuine mid-delegation rename. My other four #25 reals (`escape`, `unescape`, `parse_json`, `get_jti`) survive. |
| authlib/integrations/flask_client/__init__.py:24 *(family: #5 `: None` lifts, 4 instances)* | 5 | indexed | removal correct | Wave-1 verdict: fp, all four — `cache: None`, `query_client: None`, `client: None`, `user: None`. Exactly the over-narrow-lift failure mode. Clean hit. The three wrong-type lifts I also called fp (`grant_type: bool`, `token_generator_conf: int` ×2) **persist**. |
| authlib/jose/rfc7516/jwe.py:25 *(family: #21 calls to own helpers, 3 instances)* | 21 | heuristic | removal correct | Wave-1 verdict: fp — `self._validate_private_headers(...)`, `self._validate_sender_key(...)`, `self._validate_crit_headers(...)` are calls to private helpers, which *is* the encapsulation the rule asks for. Only 3 of the 23 I called fp went; 20 remain, including `self.handle_response(*args)` and `self.load_server_metadata()`. |
| tests/util.py:14 *(2 instances, with tests/clients/util.py:11)* | 6 | indexed | **recall loss** | Wave-1 verdict: real — `read_file_path` / `read_key_file` do file I/O behind a getter name. Low value (test helpers), and the 13 prod-side #6 findings all survive. |
| authlib/jose/rfc7516/models.py:73 | 2 | heuristic | removal correct | Wave-1 verdict: fp — "int and None have no overlap" only against the base-class default `IV_SIZE = None`; every concrete subclass sets 96 or 128. The sibling FP I named (`USER_CODE_TYPE == "digital"`, rfc8628/endpoint.py:128) **persists**. |

**Delta totals.** 3299 removed. By wave-1 verdicts: **600 I had called fp**
(#39 176, #11-test 317, #22-ext-point 55, #32 26, #25 8, #14-test 10, #5 4,
#21 3, #2 1) and **2699 I had called real**. Of those 2699:

| bucket | instances | standing |
|---|---|---|
| test-side #11 block clones | 2512 | scoped policy exemption — not contested |
| #22 methods that read instance state | 68 | wave-1 verdict superseded; new gate is more correct |
| #14 subset clumps | 31 | no site lost coverage (verified) |
| #11 nested windows | 7 | wider window still reports the same code |
| **lost TPs I still stand behind** | **81** | #33 ×23, #39 ×52, #11-prod ×2, #25 ×1, #32 ×1, #6 ×2 |

79 of the 81 are in `authlib/` (prod); 2 are test helpers.

Wave-1 anchors: of the 46 findings that made a wave-1 phase-1 site `covered`,
**44 survive**. The two casualties are P1-40 (`33@rfc6749/util.py:17`) and
P1-89 (`25@sync_app.py:22/31/40/49/58`). Every sync/async twin family I relied
on is intact — `create_logout_url`, `fetch_jwk_set`, `userinfo`,
`parse_id_token` (both windows), `_http_request`↔`_send_token_request`,
`load_server_metadata`, both `create_authorization_url` pairs, both
`fetch_access_token` pairs — all 2/2, as are the rfc8414 validator group (8/8),
the rfc9068 introspection↔revocation pair, the framework-app families, and the
signature.py hmac↔plaintext block.

## Phase 3 — delta reconciliation

Zero new findings, so no wave-1 unresolved claim could become covered: the 79
misses from wave 1 (48 detector-miss, 16 threshold-miss, 15 inventory-gap)
stand unchanged, including all eight zero-firing rules (#7, #8, #9, #19, #34,
#35, #38, #41 — still zero in wave 2). Rows below are the claims whose class
changed.

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-40 | #33 | detector-miss *(was covered)* | `scope_to_list` returns a list, `None`, or `.split()`; the `mixes value returns with bare returns` arm that named it is gone, and no other rule fires at rfc6749/util.py:17. |
| P1-89 | #13 | detector-miss *(was covered)* | The five `BaseApp.get/post/patch/put/delete` forwarders were covered only by #25, which now exempts HTTP-verb vocabulary. #13 still does not fire on them, so the site is uncovered again — though #22 still reports all five at velcro 100%, which is the FP class I named in wave 1. |
| P1-86 | #11 | covered *(unchanged, but weakened)* | `11@45` survives, so the duplication is still reported. Worth flagging: the wave-1 `33@sync_openid.py:39` finding — the `if "id_token" not in token: return None` guard that the async twin **lacks** — is gone with the bare-return arm. The clone is reported; the divergence inside it is now invisible from both sides (cf. P1-88). |

## Bottom line

**Did the fixes cost real recall on this repo? Yes, narrowly — 79 prod
findings, concentrated in two rules.** Everything else that vanished was
either a wave-1 FP (600), a scoped test exemption (2512), a verdict of mine
the new gate improved on (68), or de-duplication that left every site covered
(38). Prod #11 lost 4 sites out of 217 and 3 of those keep a neighbouring
window. Precision improved sharply where I predicted: #39's history arm went
68 FP → 5, #5's over-narrow-lift class is gone, #32's dynamic-dispatch and
sphinx-config classes are gone, #21's own-helper class is thinned.

**Two NEW classes, both absent from my wave-1 report:**

1. **FN — a prod↔test clone pair is now unreportable.** The test-block
   exemption removes the *group*, not just the test member, so a prod site
   whose only structural twin lives in `tests/` loses its finding.
   `authlib/integrations/flask_oauth1/cache.py:43` ↔
   `tests/flask/test_oauth1/oauth1_server.py:188` is the instance here; it
   carried two windows in wave 1 and carries nothing in wave 2. The fix is to
   exempt test *members* from the count while still reporting a group that
   retains a prod member.
2. **FN — the undeclared-`Optional` class is now entirely undetected.** #33
   kept fall-through and the literal `-> str … return None` lie, and dropped
   every "returns a value on one path, `return None` on another". In a library
   this thinly annotated that was #33's highest-yield arm — 23 findings, all of
   which I judged real, including `OAuth2Token.is_expired` returning `None`
   into a caller that spells `if not token.is_expired(...)`. If the ruling was
   that an explicit `return None` is ordinary Python, the arm should at least
   survive where the function carries a non-`Optional` return annotation or is
   a public boundary.

**Persisting wave-1 FP classes** (none new, none fixed): #3's three
None-guard-implied-by-iteration claims, #4's CSRF-guard claim at
sync_app.py:264, #25's `to_native → JsonWebToken.decode` index collision,
#5's `grant_type: bool` and `token_generator_conf: int` ×2, all three #40s,
#2's `USER_CODE_TYPE` class-default claim, #32's 15 sphinx-config variables,
and 31 #22 findings on template-method extension points.


## Wave 3 — confirmation

Audit: `corpus-ext/audits/authlib.wave3.json`. 1342 -> 1342. Diffed on
`(file, line, rule, cause)`: **2 new, 2 gone**, nothing else moved.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| authlib/integrations/flask_oauth1/cache.py:43 *(`clone-block:3073ef75c73d`, x3, 3 stmts)* | 11 | indexed | real | Restored at the **prod** site. Re-read both ends: `register_temporary_credential_hooks` registers `create_temporary_credential` / `get_temporary_credential` / `delete_temporary_credential` in a run that `tests/flask/test_oauth1/oauth1_server.py:188-191` repeats verbatim. The group message still names the test member, and the finding lands only on `authlib/` — the intended shape. |
| authlib/integrations/flask_oauth1/cache.py:43 *(`clone-block:4a3edabee0dd`, x2, 4 stmts)* | 11 | indexed | real | Same site, the wider 4-statement window. Both windows I lost in wave 2 are back. |
| authlib/integrations/django_oauth2/authorization_server.py:99 | 5 | indexed | fp — correctly removed | Wave-1 and wave-2 verdict: fp. `create_token_generator` branches `callable(...)` / `isinstance(..., str)` / `is True`; `int` is not one of the shapes it handles. |
| authlib/integrations/flask_oauth2/authorization_server.py:154 | 5 | indexed | fp — correctly removed | Same function, flask copy, same body discrimination. |

**New class 1 (prod<->test clone pairs) is resolved.** The policy is now
exactly right on this repo, verified over the whole audit: `#11` is 458
findings — 221 prod, 237 test, and **every one of the 237 test findings is a
whole-function clone** (`fn`), with zero test-side block clones. Test members
count toward group size without carrying findings, which is what restored the
`cache.py:43` pair. Prod `#11` sites went 217 -> 218; no prod site that
carried a `#11` in wave 1 is uncovered now except the three nested windows
whose wider window still reports.

**The two `#5` removals are the degenerate lifts I flagged**, though the guard
is narrower than the class. Two of the three wrong-type lifts are gone; the
third, `lift grant_type: bool` at `authlib/oauth2/rfc6750/token.py:74`,
**persists**. `generate` never calls `isinstance` on `grant_type` — it forwards
it to `_get_expires_in`, which discriminates by
`self.GRANT_TYPES_EXPIRES_IN.get(grant_type, ...)` against the string keys
`"authorization_code"` / `"implicit"` / `"password"` /
`"client_credentials"`. A body-isinstance guard cannot see that; a
dict-key-literal guard would. This is a narrowing of the wave-1 FP class, not a
new one.

**Bottom line: no remaining new class.** Wave 3 costs nothing and closes the
one FN class wave 2 introduced. The standing ledger for this repo is unchanged
otherwise: the `#33` explicit-`None` trade is adjudicated and recorded for
v-next as a typedness-gated design (not re-litigated here); the wave-1 FP
classes I named still persist unchanged (`#3` x3 None-guards, `#4`'s CSRF
guard, `#25`'s `to_native -> JsonWebToken.decode` index collision, `#5`'s
`grant_type: bool`, all three `#40`s, `#2`'s `USER_CODE_TYPE` class default,
`#32`'s 15 sphinx-config variables, 31 `#22` template-method findings); and the
eight zero-firing rules (`#7`, `#8`, `#9`, `#19`, `#34`, `#35`, `#38`, `#41`)
are still at zero in wave 3.
