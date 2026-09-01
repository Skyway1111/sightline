| rule | fp class | count | example key |
|---|---|---|---|
| 9 | prod symbol is a function (data loader / engine entry), not state; tests rebind the importers' bindings for a handful of doubles while the suite runs it live | 2 | src/calculator/pipeline.py:1309:9:test-writers:src.calculator.pipeline.run_fight |
| 9 | frozen registry constant (MappingProxyType) never rebound in prod; tests doctor it to exercise import-time validators and @cache projections | 1 | src/calculator/trigger_stream.py:1737:9:test-writers:src.calculator.trigger_stream.CAPABILITIES |
