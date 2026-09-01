# narrowing round (unit K): #44 narrowed, then re-priced

Sheet: `corpus-ext/sheets/gate-k.wave1.tsv`. Round h1 judged #44 at 48 real of
82, 0.585, and named four false-positive shapes. This round narrows three of
them, proves on h1's own pool that no true positive is lost, and prices the
narrowed rule on trees h1 never audited.

#44 tautological-assertion: 7 real of 33, 0.212, on the fresh pool.

## The narrowing

Two changes in `crates/py-rules/src/tests_quality.rs`. Both are pure
restrictions, so the narrowed rule reports a subset of what the old one
reported and no tree can gain a finding.

A falsy constant marks an arm and is not a tautology. `compared` read every
one-operand constant as "asserts a constant". The rule's goal is that an
assertion which cannot fail specifies nothing, and `assert False` always
fails. It marks an arm the test must not reach, and deleting it deletes the
check. `always_true` keeps only the constants an assertion cannot fail on.
That is h1's S3, 24 rows, and h1's S4, 5 rows, whose `example_scripts/`
fixture bodies are all `assert 0` or `assert False` and need no path rule.

Only `assertTrue` and `assertFalse` put a truth value in their one argument.
h1's S8 was `assertLogs("djstripe.sync", level=...)` read as an assertion of
its first argument. wagtail spells the same misreading 174 times, as
`assertNumQueries(1)` and `assertTemplateUsed("...")`, which a list of names
cannot keep up with. Every one-argument `assert*` other than the two truth
asserts takes a subject or a spec, so `truth_arg` whitelists the two and the
`RAISES_CALLS` blacklist is gone. This change was written before any row of
this round was judged.

h1's S5, reflexivity of a comparison dunder the repository writes, is not
narrowed.

## True positives preserved

The nine h1 trees that fired were re-audited with the narrowed binary and the
firings diffed against `gate-h.wave1.tsv` row by row.

| | before | after |
| --- | --- | --- |
| true positives firing | 48 | 48 |
| false positives firing | 34 | 4 |
| findings outside the h1 sheet | 0 | 0 |
| tp / n | 48/82 = 0.585 | 48/52 = 0.923 |

No true positive stopped firing. The four survivors are h1's S5, all of it.

## S5 is a type question

Each S5 row turns on the static type of an operand, which no AST reading
reaches.

| row | shape | what blocks an AST reading |
| --- | --- | --- |
| `sqlglot/tests/dialects/test_dialect.py:204` | `snowflake_object = Snowflake()` | `sqlglot/dialects/__init__.py` binds `Snowflake` through a lazy module `__getattr__` over `importlib`, so no static import binds the name to a class |
| `sqlglot/tests/dialects/test_dialect.py:201` | `snowflake_class = Dialect["snowflake"]` | the same lazy registry, reached through `__class_getitem__` |
| `sqlglot/tests/test_expressions.py:1349` | `expr = parse_one(...)` | a factory whose return type sits behind three `@overload` declarations and two re-export hops |
| `pytest/testing/_py/test_local.py:1260` | `t1 = path1.join("a_path")` | a method call on a pytest fixture parameter, whose type only a checker knows |

A probe over the reachable versions of these four shapes, a re-exported
constructor, a subscript, an annotated factory and a method on a local, shows
`compares_repo_code` already handles the first and misses the other three.
Widening it to them adds surface and fixes none of the four rows that fire.
The home for S5 is the oracle, which moves #44 off `engine_class: "AST"` and
changes what a degraded run reports. This unit does not own that ruling.

## Pool

Ten public Python trees, none in h1's pool and none from the held-out split,
cloned under `<GAUNTLET_CORPUS_ROOT>/../gauntlet-topup-k/` and audited with
`--rules 44 --all`. Two panic before emitting a finding, so the pool is the
eight that completed. n = 33.

| tree | url | pin | #44 rows |
| --- | --- | --- | --- |
| LibCST | `github.com/Instagram/LibCST` | `d9a255843b5cdbecc6834684d233bce1f2987f9d` | 20 |
| hypothesis | `github.com/HypothesisWorks/hypothesis` | `a8dcd7422a325926693b5464f73349e361562b7c` | 6 |
| sqlalchemy | `github.com/sqlalchemy/sqlalchemy` | `934c55201b9dce67cef6ad805b2fe833f7415abd` | 4 |
| wagtail | `github.com/wagtail/wagtail` | `454773af9e08aa02c235f8229a6966a87cfb2560` | 3 |
| celery | `github.com/celery/celery` | `e522ec899ebd78c3e8365cef5417f482736efefe` | 0 |
| poetry | `github.com/python-poetry/poetry` | `e33ce99067f6a28537aebd23caabc2c49aae5ed8` | 0 |
| rich | `github.com/Textualize/rich` | `9d8f9a372cc5916fd4781fec207ced7ddac2f08f` | 0 |
| scrapy | `github.com/scrapy/scrapy` | `53eb8d60bcd0160633f6513478f958ed5a457363` | 0 |
| mypy | `github.com/python/mypy` | `6934a9d01cab6226a64b38642e124368f8ddc653` | blocked |
| tornado | `github.com/tornadoweb/tornado` | `0096f2897c98facdcd9716009ee934a7381af5ef` | blocked |

Nothing was drawn. The pool is 33 rows and each is a sheet row with its own
verdict, so the seed labels the round as h1's does.

wagtail fired 177 rows before the truth-argument whitelist and 3 after. That
change is why this pool is 33 rows and not 207, and it landed before any
verdict was recorded.

## Shapes

| verdict | shape | count |
| --- | --- | --- |
| real | the call under test rebound the loop variable, so `x == x` lost its expected side, h1's S1 | 4 |
| real | reflexive `assertEqual` on a class that writes no comparison dunder, so the line is object identity | 2 |
| real | the expected value computed on the line above is never read | 1 |
| fp | a repo-defined `assertX(input, expected)` helper read as an operand comparison | 18 |
| fp | reflexivity of a comparison dunder the repository writes, h1's S5 | 6 |
| fp | a `@property` read twice, which is the determinism test spelled without a call | 2 |

## False-positive classes

| rule | fp class | count | example key |
| --- | --- | --- | --- |
| 44 | `assertCodemod(before, after)` runs the codemod on `before` and compares the result with `after`. One string passed twice asserts the codemod is a no-op here, a real check that can fail | 18 | `LibCST/libcst/codemod/commands/tests/test_remove_unused_imports.py:21:44:tautology:libcst.codemod.commands.tests.test_remove_unused_imports.RemoveUnusedImportsCommandTest.test_double_import:21` |
| 44 | reflexivity of a comparison dunder the repository writes, on an operand `compares_repo_code` cannot type: a `@given` parameter, an enum under `@total_ordering`, an index object | 6 | `hypothesis/hypothesis/tests/conjecture/test_choice.py:301:44:tautology:hypothesis.tests.conjecture.test_choice.test_choice_node_equality:301` |
| 44 | `x.prop == x.prop` where `prop` is a `@property` the repository writes, which is the determinism test the rule already leaves unreported when it is spelled `f() == f()` | 2 | `wagtail/wagtail/tests/test_page_model.py:4277:44:tautology:wagtail.tests.test_page_model.TestPageCacheKey.test_cache_key_consistent:4277` |

## The next narrowing

The 18 `assertCodemod` rows are 55% of this pool and 69% of its false
positives, and they are a hole in a ruling this repository already made.
`compared` guards a repo-defined `assert_*` helper by requiring it to resolve
to a library home, which `library_assert` does and the test
`library_assert_functions_are_operand_assertions` pins. The guard runs on the
snake_case spelling alone. A camelCase `assertX` the repository defines walks
in unguarded, and its two arguments are whatever that repository made them.
For LibCST they are an input and an expected output. The fix is the same
ruling on both spellings: an `assert*` name that resolves to a symbol the
repository defines is not `unittest`'s operand comparison.

It is not made here. These rows are judged, and a rule fixed after its numbers
arrive belongs to a later round with a fresh seed and a fresh draw.

## Blocked

mypy `6934a9d0` and tornado `0096f289` both panic the release binary before
any finding, on clean trees at their pins:

```
thread '<unnamed>' panicked at crates\py-provers\src\scope.rs:147:39:
index out of bounds: the len is 1 but the index is 58
```

That is the panic h1 recorded for bloodyAD, at the same line. Three of the 34
public Python trees these two rounds audited stop the binary on it. Outside
this unit's slice.
