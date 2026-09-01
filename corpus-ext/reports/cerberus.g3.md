# cerberus — wave 1

| rule | fp class | count | example key |
|---|---|---|---|
| 14 | documented `_validate_<rule>(self, constraint, field, value)` plugin contract fixes the signature | 2 | cerberus/validator.py:1292:14:clump:definitions,field,value |
| 17 | neck inside a straight run of `self.x = x` constructor assignments | 2 | cerberus/errors.py:109:17:liveness-neck:cerberus.errors.ValidationError.__init__:109 |
| 17 | anchor lands mid multi-line statement, five lines before the function ends | 1 | cerberus/validator.py:1645:17:liveness-neck:cerberus.validator.InspectedValidator.__init__:1645 |
| 20 | trivial lambda repeated in two independent test schema literals | 2 | cerberus/tests/test_normalization.py:361:20:lambda:cerberus.tests.test_normalization:76f20f4c |
| 21 | Mapping-protocol delegation to the class's own backing dict | 2 | cerberus/schema.py:34:21:invariant:cerberus.schema.DefinitionSchema:7f69792b |
| 22 | template method over a primitive subclasses override | 2 | cerberus/errors.py:407:22:velcro:cerberus.errors.BaseErrorHandler.extend |
| 22 | documented public facade over a sibling public method | 1 | cerberus/errors.py:310:22:velcro:cerberus.errors.ErrorTree.fetch_errors_from |
| 23 | at-threshold score on a flat isinstance dispatch | 1 | cerberus/utils.py:61:23:cognitive-complexity:cerberus.utils.mapping_to_frozenset |
| 24 | the library's published name-dispatch extension mechanism | 2 | cerberus/validator.py:364:24:dynamic-id:getattr:364 |
| 25 | `__init__` seeding itself through a public method can never share a name stem | 1 | cerberus/schema.py:482:25:rename-delegation:cerberus.schema.Registry.__init__ |
| 26 | `Path(__file__).parent / ...` constant has no literal form | 1 | cerberus/benchmarks/__init__.py:4:26:computed-declaration:cerberus.benchmarks.DOCUMENTS_PATH |
| 29 | sphinx-quickstart `conf.py`, already labelled by its header comment | 1 | docs/conf.py:1:29:top-loading:conf |
| 32 | `_validate_*` / `_check_with_*` handlers reached only by getattr name dispatch | 27 | cerberus/validator.py:1128:32:dead-symbol:cerberus.validator.BareValidator._validate_allowed |
| 32 | Sphinx settings module: every top-level name is the consumed interface | 22 | docs/conf.py:38:32:dead-symbol:conf.extensions |
| 32 | `dummy_for_rule_validation` declarations the metaclass reads for rule schemas | 4 | cerberus/validator.py:1364:32:dead-symbol:cerberus.validator.BareValidator._validate_meta |
| 32 | documented public API whose callers are library users (docs/api.rst `:members:`) | 3 | cerberus/validator.py:543:32:dead-symbol:cerberus.validator.BareValidator.root_require_all |
| 32 | deprecated back-compat alias kept on purpose until the next major release | 2 | cerberus/errors.py:72:32:dead-symbol:cerberus.errors.KEYSCHEMA |
| 32 | false claim: version-conditional import re-exported via `__all__` and used by `cerberus/__init__.py` | 2 | cerberus/platform.py:43:32:dead-import:cerberus.platform:importlib_metadata |
| 33 | property getter merged with its setter under one qualified name | 9 | cerberus/validator.py:452:33:mixed-returns:cerberus.validator.BareValidator.allow_unknown |
| 33 | deprecated `_validate_type_<name>` test fixtures whose protocol is True-or-nothing | 4 | cerberus/tests/test_validation.py:755:33:mixed-returns:cerberus.tests.test_validation.test_custom_datatype.MyValidator._validate_type_objectid |
| 39 | one-line autodoc'd public accessor: any real docstring outweighs the body | 9 | cerberus/validator.py:551:39:comment-ratio:cerberus.validator.BareValidator.root_document |
| 39 | test docstring carrying the bug or platform rationale the asserts cannot | 3 | cerberus/tests/test_validation.py:939:39:comment-ratio:cerberus.tests.test_validation.test_novalidate_noerrors |
| 39 | docstring is executable data: the rule's constraint schema `literal_eval`ed by the metaclass | 2 | cerberus/validator.py:920:39:comment-ratio:cerberus.validator.BareValidator._normalize_rename_handler |
| 39 | section banner in a 1500-line class, not an annotation of the def below | 1 | cerberus/validator.py:625:39:comment-restates:cerberus.validator:625 |
| 48 | symmetric pair under one two-branch dispatcher; folding one breaks the pair | 5 | cerberus/validator.py:1493:48:fold:cerberus.validator.BareValidator.__validate_schema_mapping |
| 48 | rule handler reachable by name dispatch, docstring is its rule schema | 4 | cerberus/validator.py:712:48:fold:cerberus.validator.BareValidator._normalize_coerce |
| 48 | named step of a caller that is a pipeline of same-shaped steps | 2 | cerberus/validator.py:900:48:fold:cerberus.validator.BareValidator.__normalize_rename_fields |
| 48 | statement count understates a 15-line block with its own concern | 1 | cerberus/validator.py:209:48:fold:cerberus.validator.BareValidator.__store_config |
| 50 | benchmark fixture module's coerce callbacks read as a public boundary | 5 | cerberus/benchmarks/schemas/overalll_schema_2.py:8:50:unannotated:overalll_schema_2.to_bool |
| 56 | documented public API entry point (docs/api.rst `:members:`) | 2 | cerberus/validator.py:1053:56:test-only:cerberus.validator.BareValidator.validated |
| 56 | the benchmark suite's own fixtures, consumed by the benchmarks they were written for | 2 | cerberus/benchmarks/__init__.py:4:56:test-only:cerberus.benchmarks.DOCUMENTS_PATH |

## wave 2

| rule | fp class | count | example key |
|---|---|---|---|
| none | | | |
