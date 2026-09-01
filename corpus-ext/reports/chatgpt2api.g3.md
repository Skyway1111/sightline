# chatgpt2api — wave 1

| rule | fp class | count | example key |
|---|---|---|---|
| 1 | param/return is an external JSON wire payload (client body, upstream SSE frame, normalized OpenAI messages); `dict[str, Any]` is the honest type for an open vendor schema | 101 | api/image_inputs.py:169:1:weak:api.image_inputs.parse_image_edit_request:return |
| 1 | constructs an OpenAI-schema object whose only consumer is `json.dumps`; no caller indexes it | 25 | services/model_service.py:152:1:weak:services.model_service.ModelCatalogService.list_models:return |
| 1 | deliberately open sink / transparent forwarder / heterogeneous container (logger `message`, `LogService.add`, `*args` forwarder, `**kwargs` pass-through, JS-map transliteration, PoW `list[Any]`) | 9 | utils/log.py:93:1:weak:utils.log.Logger.debug:message |
| 1 | nested closure reported as a public boundary: the boundary test does not exclude nested defs | 2 | services/openai_backend_api.py:2039:1:weak:services.openai_backend_api.OpenAIBackendAPI._extract_image_reference_ids.walk:value |
| 2 | the annotation is a repo-declared claim about JSON parsed off disk/wire, not a proof; the isinstance is the boundary validation that makes it true | 10 | services/account_service.py:133:2:redundant:isinstance |
| 4 | the guard discharges the parameter's own declared type (optional-dict or `object`); callers narrowing first cannot license deleting a check the signature requires | 5 | api/support.py:61:4:caller-established:isinstance |
| 6 | read-only I/O is the function's whole purpose (API-client call, storage read, file served) — a cost, not a contract the name lies about | 22 | services/backup_service.py:266:6:dishonest-accessor:services.backup_service.CloudflareR2Client.list_objects |
| 6 | FastAPI route handler named for its HTTP verb; the flagged effect is the mandatory `require_admin`/`require_identity` read | 21 | api/accounts.py:164:6:dishonest-accessor:api.accounts.create_router.list_user_keys |
| 6 | `io` misattributed to a pure body: `PIL.Image.open(BytesIO(...))`, `time.time()`, tiktoken's encoder cache | 6 | services/protocol/conversation.py:236:6:dishonest-accessor:services.protocol.conversation.count_message_image_tokens |
| 6 | lazy memoization / read-through cache refresh behind `get_`/`list_`, or a name that already says `get_or_compute` | 6 | services/auth_service.py:89:6:dishonest-accessor:services.auth_service.AuthService.list_keys |
| 6 | test double whose recorded calls are the point of the double | 3 | test/test_model_catalog_service.py:38:6:dishonest-accessor:test.test_model_catalog_service.FakeBackend.list_models |
| 6 | the global write is an auth-token memo inside a helper, outside this function's contract | 2 | services/sub2api_service.py:271:6:dishonest-accessor:services.sub2api_service.list_remote_accounts |
| 8 | FastAPI path parameter the framework binds from the URL as `str` | 5 | api/accounts.py:179:8:primitive:key_id |
| 8 | the repeated predicate is an object-mode test (`not self.access_token`), not a value invariant a NewType carries | 1 | services/openai_backend_api.py:782:8:validation:not _P_self.access_token |
| 9 | module-private memo written only inside its own module under its own lock | 1 | services/sub2api_service.py:180:9:local-writers:services.sub2api_service._token_cache |
| 9 | foreign modules only call a service singleton's method; using a service is not mutating a global | 1 | services/log_service.py:113:9:shared-state:services.log_service.log_service |
| 10 | `set` -> `Collection` discards the O(1)-membership contract the set was chosen for; `AbstractSet` is the right widening | 3 | services/openai_backend_api.py:1729:10:over-constrained:services.openai_backend_api.OpenAIBackendAPI._looks_like_editable_primary:primary_mime_types |
| 10 | nested closure with one in-function call site: no external caller exists for the concrete type to shut out | 1 | services/protocol/conversation.py:432:10:over-constrained:services.protocol.conversation.sanitize_output_text.readable_annotation_part:parts |
| 11 | sibling FastAPI route declarations: distinct paths/models/handlers, merging hides the route table | 4 | api/ai.py:122:11:clone:acaab25e4c6b |
| 11 | trivial one-statement twins where parameterizing costs more than the copy | 2 | api/support.py:66:11:clone:24ce88094e5d |
| 11 | symmetric pair by design (encrypt/decrypt); merging behind a mode flag trades this rule for #51 | 2 | services/backup_service.py:48:11:clone:654a092dc4f8 |
| 11 | the shared body is already extracted (`_submit(kind=...)`); what is left is the named entry points de-duplication produces | 2 | services/editable_file_task_service.py:97:11:clone:b2064a14dbb9 |
| 13 | a named normalization/primitive (`_clean`, `_now_iso`, `_hash_key`, a large regex) with 2-46 call sites: the one home for the coercion | 16 | api/accounts.py:123:13:shallow:api.accounts._account_payload_token |
| 13 | facade method giving the class one home for a global config read, or its resource-lifecycle contract | 5 | services/backup_service.py:299:13:shallow:services.backup_service.CloudflareR2Client.close |
| 13 | test fake whose forwarding method must exist to satisfy the protocol | 4 | test/test_proxy_runtime_api.py:39:13:shallow:test.test_proxy_runtime_api.FakeConfig.get_proxy_settings |
| 14 | FastAPI dependency-injected parameters bound by annotation | 1 | api/ai.py:92:14:clump:authorization,body,request |
| 14 | minified argument names (`e`, `n`, `t`) of transliterated JS opcode closures | 1 | utils/turnstile.py:81:14:clump:e,n,t |
| 16 | the function already is pure-core-then-one-effect: a detail dict built, then one log call | 3 | services/editable_file_task_service.py:254:16:mutation-tail:services.editable_file_task_service.EditableFileTaskService._log_call |
| 16 | the flagged tail is the construction of the return value (dict literal, dataclass), not a state write | 2 | services/config.py:243:16:mutation-tail:services.config._normalize_proxy_runtime_settings |
| 16 | the tail is a retry loop, not an effect appended to a computation | 1 | services/protocol/conversation.py:1315:16:mutation-tail:services.protocol.conversation._generate_single_image |
| 16 | a migration script's `main`: compute-then-write in one place is the script | 1 | scripts/migrate_storage.py:151:16:mutation-tail:migrate_storage.main |
| 17 | a one-variable neck inside a single linear job (validate-then-fetch, guard-then-normalize, copy-then-default) | 5 | api/image_inputs.py:263:17:liveness-neck:api.image_inputs._download_image_url:263 |
| 17 | the neck sits between two independent nested route handlers inside `create_router` | 2 | api/accounts.py:397:17:liveness-neck:api.accounts.create_router:397 |
| 18 | the numbered comment enumerates the disjuncts of the single boolean expression below it, not phases of the function | 1 | services/protocol/conversation.py:597:18:sections:services.protocol.conversation.update_conversation_state |
| 19 | the list holds a handful of ids/status codes and its order is part of the result; a set changes semantics and the quadratic term is unmeasurable | 5 | services/config.py:211:19:linear-in-loop:services.config._normalize_status_codes:normalized |
| 20 | a single-expression sort key whose whole content is the field name | 3 | services/backup_service.py:600:20:lambda:services.backup_service:2e1d2030 |
| 21 | the expression is the class reading (or `len`-ing) its own field, or calling a helper it already extracted: encapsulation is already where it belongs | 15 | services/account_service.py:24:21:invariant:services.account_service.AccountService:7b85122c |
| 22 | a composed or facade operation on a service singleton: the API callers reach through the instance | 9 | services/account_service.py:858:22:velcro:services.account_service.AccountService.keepalive_refresh_tokens |
| 22 | test double implementing the interface it stands in for | 2 | test/test_image_storage_service.py:38:22:velcro:test.test_image_storage_service.FakeWebDAVClient.test |
| 23 | fits one screen (<=30 lines) as a shape ladder or recursive visitor; the score is inflated by this repo's `x or default` chaining, which counts as decisions | 20 | services/protocol/openai_v1_response.py:85:23:cognitive-complexity:services.protocol.openai_v1_response.extract_response_image |
| 23 | integration test or dev diagnostic script driving one linear scenario | 8 | scripts/init_proxy_config.py:127:23:cognitive-complexity:init_proxy_config.main |
| 23 | zero block nesting: a flat sequential normalizer read top to bottom | 6 | services/account_service.py:206:23:cognitive-complexity:services.account_service.AccountService._normalize_account |
| 23 | the enclosing body is straight-line; the score is the sum of independent nested defs (route handlers, closures) | 5 | api/accounts.py:160:23:cognitive-complexity:api.accounts.create_router |
| 23 | parent and nested child both reported, double-counting the same code | 1 | services/content_filter.py:40:23:cognitive-complexity:services.content_filter.request_shape |
| 23 | line-for-line transliteration of a vendor JS opcode dispatcher; decomposing breaks correspondence with the source | 1 | utils/turnstile.py:49:23:cognitive-complexity:utils.turnstile.solve_turnstile_token |
| 24 | generic ORM column access whose key arrives as a literal one hop away, so the name is greppable at the call site | 1 | services/storage/database_storage.py:100:24:dynamic-id:getattr:100 |
| 25 | `__del__` delegating to `close()` is Python's finalizer protocol; the dunder name is fixed by the language | 1 | services/openai_backend_api.py:229:25:rename-delegation:services.openai_backend_api.OpenAIBackendAPI.__del__ |
| 26 | a directory `Path` derived from `__file__` is not a member list a reader must execute to enumerate | 2 | scripts/migrate_storage.py:21:26:computed-declaration:migrate_storage.DATA_DIR |
| 27 | the router composition root's imports are its route table, not a reader's load | 1 | api/ai.py:1:27:fan-out:api.ai |
| 29 | the cost arm keys on line count with no evidence of cost: pure formatting, normalization or in-memory lookup | 18 | services/account_service.py:1000:29:cost-docstring:services.account_service.AccountService.get_text_access_token |
| 29 | route/app registration factory called once at startup; its length is independent handler bodies | 5 | api/accounts.py:160:29:cost-docstring:api.accounts.create_router |
| 29 | the module is exactly its documented class(es) with no loose top-level defs: the class docstring is the first screen | 3 | services/account_service.py:1:29:top-loading:services.account_service |
| 34 | optional/best-effort parse with the fallback assigned first or checked on the next line | 5 | services/openai_backend_api.py:857:34:swallowed-pass-print:services.openai_backend_api:857 |
| 34 | isolating a caller-supplied callback or the logging call so it cannot fail the work it reports on | 3 | services/openai_backend_api.py:2595:34:swallowed-pass-print:services.openai_backend_api:2595 |
| 34 | teardown/cleanup close (`__del__`-reachable, or in a `finally`) that must not raise | 2 | services/openai_backend_api.py:226:34:swallowed-pass-print:services.openai_backend_api:226 |
| 34 | supervisor loop or documented optional enrichment that records the error and continues by design | 2 | api/support.py:108:34:swallowed-pass-print:api.support:108 |
| 34 | an optional enrichment or diagnostic where the empty result is the normal path | 2 | services/account_service.py:700:34:swallowed-pass-print:services.account_service:700 |
| 36 | the ignores sit inside a test's `FakeConfig` indexing its own dict fixture; no prod prover is blinded | 1 | test/test_proxy_runtime_api.py:43:36:type-lies:test.test_proxy_runtime_api |
| 37 | the knob is exercised: a test passes the non-default value | 3 | services/account_service.py:1487:37:unused-default:services.account_service.AccountService.refresh_accounts:defer_invalid_removal |
| 39 | the comment/docstring carries a fact the code cannot (an upstream quirk, the method used, the reason for a callee) | 4 | services/protocol/conversation.py:126:39:comment-ratio:services.protocol.conversation.is_model_text_reply_instead_of_image |
| 40 | the plural test reads the last name token, so unit suffixes (`_secs`, `_days`), counted nouns (`_tokens`) and verb/prepositional-phrase objects (`cleanup_old_images`, `..._from_messages`) all read as plural | 13 | services/config.py:388:40:naming-proxy:services.config.ConfigStore.image_retention_days |
| 42 | a test double's protocol method named `test` caught by the name-based test detector | 1 | test/test_image_storage_service.py:38:42:assertion-free:test.test_image_storage_service.FakeWebDAVClient.test |
| 45 | the real verdict is `search.assert_not_called()`, a claim about the routing decision; the stub value is corroboration, not the only observation | 1 | test/test_chat_completion_cache.py:531:45:testing-the-double:test.test_chat_completion_cache.ChatCompletionCacheTests.test_chat_completions_search_like_model_does_not_trigger_search |
| 46 | a manual diagnostic script under `scripts/`, not a suite member; the try/except is how it reports status | 1 | scripts/test_storage.py:120:46:unfailable:test_storage.test_storage:57 |
| 48 | the name carries a policy, protocol or vocabulary the inline code does not state (filename safety, atomic write, decision vocabularies, the keepalive anchor) | 9 | services/content_filter.py:130:48:fold:services.content_filter._is_allow_decision |
| 48 | folding grows an already-long caller, or breaks a symmetric pair / coherent helper family | 7 | services/openai_backend_api.py:521:48:fold:services.openai_backend_api.OpenAIBackendAPI._conversation_payload |

## wave 2

| rule | fp class | count | example key |
|---|---|---|---|
| none | | | |
