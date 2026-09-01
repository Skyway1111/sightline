# gate posture round (unit H): #44 and #31 judged

Sheet: `corpus-ext/sheets/gate-h.wave1.tsv`. Every #44 firing in the pool is a
row. Nothing was sampled, because no single tree fired more than 40 (the
largest is mitmproxy at 25). Each row was read at its site and judged against
the rule's stated goal, that an assertion which cannot fail specifies nothing.

#44 tautological-assertion: 48 real of 82, 0.585, under the 0.70 heuristic bar
its tier is held to.

#31 boundary-contracts: 0 findings in the whole pool. n = 0, so no posture is
judged for it.

## Pool

26 Python trees, each audited with `--rules 44,31 --all`. The 23 gauntlet
calibration clones are the whole non-held-out Python split (`manifest*.json`;
optimum, ttt-video-dit, MLAlgorithms, django-unfold, peewee and
claude-bug-bounty are held out and were not audited). bloodyAD is the 24th and
could not be audited at all, see Blocked. The three pinned corpus trees and a
two-tree top-up bring the pool to n = 82. The top-up was drawn because the
shape the campaign asked about, `assert False` as a marker, fires almost
nowhere in the gauntlet split.

| source | trees | #44 rows | drawn |
| --- | --- | --- | --- |
| gauntlet calibration, `<GAUNTLET_CORPUS_ROOT>/` | 23 | 31 | exhaustive, every tree |
| pinned corpus, `<GAUNTLET_CORPUS_ROOT>/../{sqlglot,powertools-lambda-python,merged-calculator}` | 3 | 6 | exhaustive |
| top-up, `<GAUNTLET_CORPUS_ROOT>/../gauntlet-topup-h/{pytest,mitmproxy}` | 2 | 45 | exhaustive |

Trees that fired: mitmproxy 25, pytest 20, open-metric-learning 15, authlib 13,
merged-calculator 3, sqlglot 3, dj-stripe 1, fastapi-users 1, forge 1. The
other 17 fired zero. Top-up SHAs: pytest `2cd217e5`, mitmproxy `2ac5b089`.

## Shapes

Both columns are what a later narrowing would be narrowing.

| verdict | shape | count |
| --- | --- | --- |
| real | S1 rebound operand: the fixture name was overwritten by the call under test, so `x == x` lost its expected side | 14 |
| real | S2 `assert True` closing a did-not-raise body | 27 |
| real | S6 reflexive compare where `==` is builtin (namedtuple, dict lookup, type identity) | 5 |
| real | S7 the assert statement lost its operand (a split string literal, a dropped `in`) | 2 |
| fp | S3 unreachability marker: `assert False` or `assert 0` in a try/except `else`, an except arm, an if/elif exhaustiveness arm, or as the subject under test | 24 |
| fp | S4 pytest's `testing/example_scripts/` fixture data, which must fail or must never run | 5 |
| fp | S5 reflexivity of a comparison dunder the repo writes | 4 |
| fp | S8 `assertLogs` read as an assertion of its first argument | 1 |

S3 alone is 71% of the false positives, and S3 with S4 is 85%. A #44 that left
the `assert False` family alone where it sits in an arm reached only on
failure would read 48/53 = 0.906 on this pool, and one that also skipped
`example_scripts/`-style fixture trees would read 48/48. That narrowing is not
this round's work: the sampler's protocol wants a fresh seed and a fresh draw
after any tuning.

## False-positive classes

| rule | fp class | count | example key |
| --- | --- | --- | --- |
| 44 | should-have-raised marker in the `else` arm of a try/except | 13 | `pytest/testing/test_runner.py:496:44:tautology:testing.test_runner.TestExecutionNonForked.test_keyboardinterrupt_propagates:496` |
| 44 | must-not-raise marker in an except arm | 4 | `mitmproxy/test/mitmproxy/addons/test_view.py:263:44:tautology:test.mitmproxy.addons.test_view.test_load:263` |
| 44 | exhaustiveness arm of a parametrized if/elif chain | 4 | `pytest/testing/test_assertion.py:158:44:tautology:testing.test_assertion.TestImportHookInstallation.test_conftest_assertion_rewrite:158` |
| 44 | the `assert False` is the subject under test, and its source text is what the test then matches | 3 | `pytest/testing/code/test_code.py:181:44:tautology:test_code.TestTracebackEntry.test_getsource:181` |
| 44 | fixture script under `testing/example_scripts/`, run as data by another test | 5 | `pytest/testing/example_scripts/unittest/test_setup_skip.py:17:44:tautology:test_setup_skip.Test.test_foo:17` |
| 44 | reflexivity of a comparison dunder the repo writes, which `compares_repo_code` missed: a metaclass `__eq__`, a factory function's return, a fixture-built value | 4 | `sqlglot/tests/dialects/test_dialect.py:204:44:tautology:tests.dialects.test_dialect.TestDialect.test_compare_dialects:204` |
| 44 | `assertLogs` is a log-capturing context manager, not an assertion. `RAISES_CALLS` exempts assertRaises and assertWarns but not it | 1 | `dj-stripe/tests/test_sync.py:68:44:tautology:tests.test_sync.TestSyncSubscriber.test_sync_fail:68` |

## #31: the zero is not silence

langextract is the one tree in the pool that declares import contracts, and it
declares four of them under `[tool.importlinter]` in `pyproject.toml`.
`sightline debug dump --layer listing` on it prints `"linter": []`.
`load_importlinter` at `crates/py-facts/src/inputs.rs:468` reads only a root
`.importlinter` INI file, and import-linter equally reads `setup.cfg`,
`tox.ini` and `pyproject.toml`. So #31's zero across 26 trees is a config
reader that misses the modern spelling, not 26 compliant repositories. Not
fixed here: widening the reader changes the rule's reach, which this round is
not authorized to do.

## Blocked

bloodyAD (gauntlet calibration, `0422904a`, clean tree) panics the release
binary before it emits any finding, single-threaded and multi-threaded, whole
rule set and restricted alike:

```
thread '<unnamed>' panicked at crates\py-provers\src\scope.rs:147:39:
index out of bounds: the len is 342 but the index is 583
...
thread '<unnamed>' panicked at crates\py-provers\src\scope.rs:149:18:
a function symbol's node is a def
```

A `Scope` holds a node index from a module other than its own `self.module`.
Outside this unit's slice, and it stops the binary on one of 24 public Python
trees.
