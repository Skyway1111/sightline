# top-up (round 4) — wave 1

| rule | fp class | count | example key |
| --- | --- | --- | --- |
| 6 | `read_*` on a byte cursor: consuming and advancing is the documented contract | 5 | `rofl/decode/_stream.py:32:6:dishonest-accessor:rofl.decode._stream.ByteStream.read_u8` |
| 6 | the only effect is a read-through memo / lazy table fill, invisible to callers | 3 | `src/calculator/stats.py:255:6:dishonest-accessor:src.calculator.stats.get_item_stats` |
| 6 | the flagged global is the module's shared DB session factory that every query function opens | 2 | `src/db.py:630:6:dishonest-accessor:src.db.list_feedback` |
| 6 | mutates-arg on a container the function itself just built (nothing observable escapes) | 1 | `src/calculator/optimizer.py:840:6:dishonest-accessor:src.calculator.optimizer.get_purchase_items` |
| 8 | opaque id/label string whose only hand-checks are emptiness or defaults, so a NewType (no runtime validation) discharges none of them | 4 | `src/league_predictor/analytics/team_snapshot.py:25:8:primitive:team_id` |
| 30 | the last hop is a stdlib or Enum attribute (`Path.name`, `Enum.value`) on the record the function was handed | 3 | `patch_compiler/inputs.py:481:30:demeter:patch_compiler.inputs.stage_discovery_workspace:3` |
| 30 | the chain is a repo-wide idiom every reader knows (`x.config.<section>.<option>.value`, `parent.frame.screen`) | 2 | `slack/slack_user.py:53:30:demeter:slack.slack_user.name_from_user_profile:3` |
| 30 | reach into the module's own value-bag record that every helper there is handed by design | 1 | `src/calculator/damage.py:16749:30:demeter:src.calculator.damage._expose_weakness_pool:3` |
| 41 | the regex sits in a documented-rare fallback branch, not on the hot path | 2 | `src/calculator/champions/scaling.py:150:41:perf:re-in-loop:src.calculator.champions.scaling._parse_compound_unit:150` |
| 42 | acceptance case: the oracle is that the call does NOT raise, with every rejecting case pinned by a `pytest.raises` sibling | 11 | `tests/test_cast_dependency.py:381:42:assertion-free:tests.test_cast_dependency.TestImportGate.test_a_diamond_is_not_a_cycle` |
| 42 | the assertions live in a subprocess body and `subprocess.run(..., check=True)` is the verdict | 2 | `tests/lifecycle/test_contract.py:133:42:assertion-free:tests.lifecycle.test_contract.test_get_import_does_not_pull_per_model_modules` |
| 46 | the handler is a documented second accepted outcome (a fail-closed raise); the other branch is still asserted | 5 | `tests/test_heimerdinger_multihit.py:883:46:unfailable:tests.test_heimerdinger_multihit.TestMalformedInputs.test_missing_w_row_fails_closed_not_silent_zero:881` |
| 46 | the handler returns the run's pass/fail verdict, which `__main__` turns into the exit code | 1 | `scripts/test_storage.py:120:46:unfailable:test_storage.test_storage:57` |
| 54 | the tag spellings coincide across independent per-format adapters; no shared kind exists to hold one dispatch | 1 | `glmv_reward/scripts/gui_agent/OSWorld.py:61:54:kind-switch:key,type` |
| 56 | a reference implementation kept as the test oracle for a shipping fast path or shipped data | 3 | `src/league_predictor/predraft/ratings/skill_filter/replay.py:149:56:test-only:league_predictor.predraft.ratings.skill_filter.replay.replay_for_training` |
| 56 | a hand-run tool the repo publishes in its own index or module docstring (research toolkit, analysis library, developer lookup) | 4 | `rofl/inspect.py:66:56:test-only:rofl.inspect.find_value` |
| 56 | the writer/harness for a shipped artifact or protocol whose only invocation is by hand or from the test that is the harness | 2 | `src/league_predictor/draft_grading/leagueofgraphs_matchups.py:374:56:test-only:league_predictor.draft_grading.leagueofgraphs_matchups.build_matchup_cache` |
| 57 | the key belongs to a repo-wide record schema other producers share and other readers take generically | 1 | `src/calculator/participant_timeline.py:723:57:dead-key:src.calculator.participant_timeline._guardian_selection_template:target_scope` |
