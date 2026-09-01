# forge — wave 1

| rule | fp class | count | example key |
| --- | --- | --- | --- |
| 1 | framework passthrough: the value is a user-registered tool's own return, so Any is the honest contract | 3 | src/forge/core/runner.py:96:1:weak:forge.core.runner.WorkflowRunner.run:return |
| 2 | try-body assignment narrowed into a `finally` that is reachable before it runs | 2 | src/forge/proxy/_installer.py:429:2:redundant:comparison |
| 2 | runtime validation of untrusted input (wire JSON, manifest, model-emitted args) whose annotation nothing enforces | 5 | tests/eval/dataset_builder.py:104:2:redundant:isinstance |
| 2 | check made "redundant" by a lying default under `# type: ignore`, while the check is what supplies the value | 1 | src/forge/proxy/server.py:155:2:redundant:comparison |
| 2 | mutable instance attribute narrowed across awaits, where a concurrent stop() nulls it | 2 | src/forge/server.py:389:2:redundant:comparison |
| 2 | oracle inference on a parametrized test fixture claims a class the fixture really is | 1 | tests/unit/test_proxy_proxy.py:342:2:redundant:isinstance |
| 7 | exception-contract prose ("raises KeyError, treat as fatal") read as a caller protocol | 1 | scripts/migrate_eval_jsonl_gguf_identity.py:82:7:protocol-doc:82 |
| 9 | the finally-block restore of a rebind counted as a second monkeypatch of the same seam | 2 | scripts/smoke_test_proxy.py:1571:9:monkeypatch:forge.server._setup_managed_backend |
| 10 | param iterated twice in the body, so widening to Iterable admits a generator that is empty on the second pass | 2 | src/forge/guardrails/guardrails.py:164:10:over-constrained:forge.guardrails.guardrails.Guardrails.record:executed |
| 11 | stdlib/protocol boilerplate with no shared fact (socket probe, BaseHTTPRequestHandler reply, try-lookup-except-KeyError) | 4 | scripts/smoke_test_proxy.py:283:11:clone:1641b11ae63e |
| 11 | normalized assert run collides across unrelated tests | 1 | scripts/smoke_test_proxy.py:566:11:clone-block:cbb7de376b97 |
| 11 | shape-only clone whose entire content is the differing literal (exception message, prompt text) | 4 | src/forge/prompts/nudges.py:22:11:clone:ded6cb15ec86 |
| 18 | the labeled phases are already extracted functions; the comments restate the callee docstrings | 1 | src/forge/context/strategies.py:231:18:sections:forge.context.strategies.TieredCompact._compact_with_initial_usage |
| 18 | labels name the domain's phases or elements of a list literal, not sequential sections of the function | 4 | tests/unit/test_strategies.py:350:18:sections:tests.unit.test_strategies.TestTieredEscalation.test_per_phase_thresholds |
| 18 | each labeled phase is a single call; extracting would create one-call functions | 1 | src/forge/server.py:819:18:sections:forge.server.ServerManager._start_spawned_with_budget |
| 23 | short flat validation/guard chain: many independent raises, nothing nests | 5 | scripts/standalone/release.py:150:23:cognitive-complexity:scripts.standalone.release.validate_manifest |
| 23 | flat option-setting or line-appending builder ("if x is not None: extend") | 4 | src/forge/server.py:85:23:cognitive-complexity:forge.server._render_launch_command |
| 23 | one loop or one ordered ladder with early exits; a reader holds one branch at a time | 6 | src/forge/prompts/templates.py:238:23:cognitive-complexity:forge.prompts.templates.rescue_tool_call |
| 24 | import_module over a tuple of literal module names two lines above the call — fully greppable | 1 | src/forge/proxy/__main__.py:303:24:dynamic-id:import_module:303 |
| 27 | price arm on a module under ~600 lines that is one cohesive read (protocol surface, one client, one converter) | 9 | src/forge/clients/base.py:1:27:price:forge.clients.base |
| 27 | fan-out on a composition seam whose imports are one each of the collaborators it wires | 1 | src/forge/proxy/handler.py:1:27:fan-out:forge.proxy.handler |
| 28 | template placeholder in a how-to ("Create src/forge/clients/yourbackend.py") | 2 | CONTRIBUTING.md:100:28:doc-path:src/forge/clients/yourbackend.py |
| 28 | dated ADR plan/sequencing narrative naming a path that has since moved | 5 | docs/decisions/008-stateful-eval-scenarios.md:1254:28:doc-path:scenarios.py |
| 28 | ADR whose status line scopes its module list to another branch | 6 | docs/decisions/009-bfcl-integration.md:17:28:doc-path:schema_adapter.py |
| 28 | explicitly hypothetical or deferred name ("adapters would live in ...") | 1 | docs/decisions/011-guardrail-middleware.md:333:28:doc-symbol:forge.guardrails.adapters |
| 28 | a package's `__init__` module is not matched by the symbol resolver | 2 | docs/decisions/010-tool-resolution-error.md:46:28:doc-symbol:forge.__init__ |
| 32 | import whose purpose is the ImportError probe inside try/except, already marked `# noqa: F401` | 2 | scripts/smoke_test_proxy.py:895:32:dead-import:scripts.smoke_test_proxy:anthropic |
| 33 | fall-off path unreachable: the body awaits a Future that never resolves | 1 | tests/unit/test_batch_eval_lifecycle.py:539:33:lying-return:tests.unit.test_batch_eval_lifecycle.test_recovery_backoffs_circuit_and_whole_run_timeout_remain_exact.never_returns |
| 36 | narrowly-coded pragmas on dynamic pydantic construction where no static spelling exists | 1 | src/forge/core/workflow.py:29:36:type-lies:forge.core.workflow |
| 37 | Protocol whose second implementation is a test double injected through the same seam | 2 | src/forge/proxy/_installer.py:135:37:single-impl:forge.proxy._installer.Transport |
| 37 | injection seam exercised only from tests, invisible to a prod-only call-site count | 3 | src/forge/proxy/_profiles.py:33:37:unused-default:forge.proxy._profiles._managed_profile_root:system |
| 39 | the bigram "no longer" inside a present-tense rationale or postcondition | 6 | src/forge/guardrails/response_validator.py:96:39:comment-history:forge.guardrails.response_validator:96 |
| 42 | the oracle is "this call does not raise", the complement of a pytest.raises sibling | 2 | tests/unit/test_proxy_proxy.py:509:42:assertion-free:tests.unit.test_proxy_proxy.TestLifecycle.test_stop_before_start_noop |
| 44 | `assert False` as the fail-if-reached marker of a manual try/except raises idiom | 1 | tests/unit/test_context_manager.py:315:44:tautology:tests.unit.test_context_manager.TestCompactEvent.test_frozen:315 |
| 47 | the sleep is the poll interval of a bounded condition wait, not a fixed wall-clock wait | 1 | tests/integration/platform_acceptance/test_windows_installer.py:207:47:sleepy:tests.integration.platform_acceptance.test_windows_installer.test_windows_locked_slot_preserves_uninstall_retry_path:207 |
| 48 | the helper is the "before" half of a deliberate side-by-side repro | 1 | scripts/repro_issue_121.py:34:48:fold:scripts.repro_issue_121._model_name_buggy |
| 58 | override differs from the base only in underscore-prefixed parameter names (unused-arg convention) | 2 | scripts/standalone/smoke.py:36:58:invalid-method-override:scripts.standalone.smoke:36:16 |
| 59 | main by another name: called only from main() under the `__name__` guard and described by the module docstring | 7 | scripts/standalone/lifecycle_smoke.py:623:59:cost-docstring:scripts.standalone.lifecycle_smoke.run_lifecycle |
| 59 | framework callback (BaseHTTPRequestHandler.do_POST), not an entry point a caller chooses | 1 | scripts/standalone/smoke.py:39:59:cost-docstring:scripts.standalone.smoke.MockBackend.__init__.Handler.do_POST |
