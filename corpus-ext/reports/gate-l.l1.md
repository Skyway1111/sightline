# gate posture round (unit L): the #31 reader widened, then #31 judged

Sheet: `corpus-ext/sheets/gate-l.wave1.tsv`. Every #31 firing in the pool is a
row. Nothing was sampled, because no tree fired more than 20.

#31 boundary-contracts reads 8 real of 110, 0.073, against the 0.80 bar its
indexed tier is held to and the 0.70 floor the registered gate rule fixes
(`corpus-ext/decisions.tsv release/gate-judged-rule`).

`load_importlinter` read one file, a root `.importlinter`, so #31 fired zero on
every tree unit H audited. This branch widens the reader to the three files
import-linter reads, in the same commit as the measurement below.

## What import-linter reads

Taken from `seddonym/import-linter` at `master`, not inferred from this tree.

| file | format | reader | section | contracts |
| --- | --- | --- | --- | --- |
| `setup.cfg` | INI | `IniFileUserOptionReader` | `[importlinter]` | `[importlinter:contract:<id>]` |
| `.importlinter` | INI | same | `[importlinter]` | same |
| `pyproject.toml` | TOML | `TomlFileUserOptionReader` | `[tool.importlinter]` | `[[tool.importlinter.contracts]]` |

Precedence is that order. `configuration.py` registers the readers `ini` then
`toml`. `IniFileUserOptionReader.potential_config_filenames` is
`["setup.cfg", ".importlinter"]` and the TOML reader's is `["pyproject.toml"]`.
`read_options` returns the first file that yields options. A file that declares
the section and no contract still wins, and the search stops there.

`tox.ini` is not one of them. Unit H's note and the `release/gate-judged` row
list `setup.cfg, tox.ini and pyproject.toml`. The docs list three files, and
neither reader names `tox.ini`. This reader takes the three import-linter takes.

Two session options decide what reaches the graph, and both sit in the same
table.

- `exclude_type_checking_imports`. "Any import made under an `if TYPE_CHECKING:`
  statement will not be added to the graph."
- `include_external_packages`. Imports of external packages become available
  for checking.

`docs/contract_types/index.md` gives the wildcard grammar. `*` stands in for one
module name, `**` includes subpackages too. `mypackage.*` matches
`mypackage.foo` and not `mypackage.foo.bar`. `mypackage.**` matches both.

langextract declares three contracts, not the four the `release/gate-judged` row
records, which counted the `[tool.importlinter]` header. Its listing prints
`"linter": []` before this change and all three contracts after, and it fires
zero either way, because its code obeys them.

## Pool

64 public Python trees that declare import-linter contracts, cloned to
`<CODE_ROOT>/gauntlet-contracts/` and pinned in its `PINS.tsv`. GitHub code
search over `[tool.importlinter]`, `importlinter:contract` and `.importlinter`
matched 591 repositories. 457 declared their config at the repository root,
which is the only place this reader looks. 314 of those were fetched and read,
and 232 declared two or more contracts. The 64 cloned take the highest contract
counts and the highest star counts, so the pool is not only small repositories.

56 audited, 8 stopped the binary (see Blocked), 15 fired, n = 110.

| tree | rows | tree | rows | tree | rows |
| --- | --- | --- | --- | --- | --- |
| nilearn | 18 | openedx-platform | 8 | CodeLeash | 3 |
| trade_xv2 | 16 | GeneLab | 6 | infa2td | 3 |
| AutoSkillit | 15 | checkov | 5 | Automodel | 1 |
| mnemos | 12 | mlrun | 5 | guidellm | 1 |
| forze | 11 | Simple-Secrets-Manager | 5 | treg | 1 |

The other 41 audited trees fired zero. Unit H's 26-tree pool still fires zero
after the widening, because langextract is the only tree in it that declares a
contract and langextract complies.

## Shapes

Every false positive is one of three shapes, and each is #31 reading a contract
with semantics the repo did not ask for. Representatives of each shape were
read at their sites before the class was applied to its rows.

| verdict | shape | count |
| --- | --- | --- |
| fp | S1 the contract's own `ignore_imports` exempts the import, through a `*` or `**` expression `covers()` cannot match | 52 |
| fp | S2 the import sits under `if TYPE_CHECKING:` in a repo that sets `exclude_type_checking_imports` | 43 |
| fp | S3 the site is `importlib.import_module(...)`, which a static graph never holds | 7 |
| real | R1 a static import statement crossing a declared boundary that no option or ignore exempts | 8 |

S1 and S2 together are 95 of the 102 false positives.

S1 is the matcher. `covers(pkg, m)` in `py-provers/src/imports.rs` is
`m == pkg || m starts with pkg + "."`. That is the right test for a contract's
`source_modules` and the wrong one for `ignore_imports`, where repositories
write the expressions the wildcard grammar defines. openedx-platform opens its
ignore list with `**.tests.** -> **` under the comment "Test code can break
these layering rules", and all 8 of its rows are test modules. mlrun writes
`server.py.framework.utils.projects.* -> services` and
`server.py.framework.tests.integration.db.* -> services`, which covers all 5 of
its rows. nilearn writes four `nilearn.plotting.*.* -> nilearn.glm.**` entries,
which covers all 18.

The 52 S1 rows come from 12 distinct ignore expressions, and 11 of them match
their importer and their target under the documented grammar alone. The
twelfth is mlrun's `... -> services`, where the import is `services.api.crud`
and the target is read as a package covering its descendants, which is how
mlrun's own TODO comment reads it. <!-- prose-ok: names a comment in mlrun -->
Those 5 rows are the only ones resting on
that reading. Judging all 5 `real` instead would put #31 at 13 of 110, 0.118,
which changes no band.

S2 is the session option. checkov, guidellm, treg, forze, AutoSkillit,
CodeLeash, infa2td, GeneLab and Automodel all set
`exclude_type_checking_imports`, and the imports #31 reports in them are
annotation-only imports the repo told its linter to drop. #31 reads no session
option.

S3 is the dynamic import. mnemos comments its own config to say that
lifecycle.py "wires the persistence backends at startup (cf. the
`importlib.import_module` calls ...)". The repo reached for a dynamic import to
keep the edge out of the contract, and `dynamic_target` puts it back.

A #31 that honoured `ignore_imports` wildcards and `exclude_type_checking_imports`
would read 8 of 15 on this pool, and one that also left dynamic targets alone
would read 8 of 8. That narrowing is not this round's work, since a fresh seed
and a fresh draw come after any tuning.

## Two defects the sheet shows, outside precision

One statement can be many rows. `from mnemos.api.routes.consultations import
(a, b, c, d)` is four rows at one line, one per imported name. All four share a
`cause`, so the sheet key does not separate them either.

One import can be one row per contract. nilearn's 18 rows are 9 imports, each
matched by both the 'main architecture' and the 'reporting' layers contract.
openedx-platform's 8 rows are 4 imports. Only `evidence.detail` tells them
apart.

Both inflate what a reader sees at one site. Neither is priced above, because
each such row's claim is true wherever its shape is `real`.

## False-positive classes

| rule | fp class | count | example key |
| --- | --- | --- | --- |
| 31 | `ignore_imports` wildcard the matcher cannot read | 52 | `openedx-platform/openedx/core/djangoapps/content_tagging/tests/test_handlers.py:10:31:boundary:layers:openedx.core.djangoapps.content_tagging->openedx.core.djangoapps.content_libraries` |
| 31 | `exclude_type_checking_imports` set, import under the guard | 43 | `checkov/checkov/common/typing.py:14:31:boundary:forbidden:checkov.common->checkov.terraform` |
| 31 | dynamic `importlib.import_module` target | 7 | `mnemos/mnemos/core/lifecycle.py:282:31:boundary:layers:mnemos.core->mnemos.persistence` |

## Recommendation

The registered rule fixes the bands before the sample. Under 0.70 the rule moves
to RATCHET. #31 reads 0.073, so the recommendation is RATCHET. The posture
decision is the maintainer's.

0.073 is the lowest reading in `data/precision.toml`. GATE blocks wherever it
runs and is never written to a baseline, so on this evidence a first-time user
with an import-linter config is blocked at 13 of every 14 findings, on lines
their own config file already exempts.

## Blocked

8 of the 64 trees stop the release binary before it emits a finding, on the
panic unit H recorded for bloodyAD.

```
crates\py-provers\src\scope.rs:147:39: index out of bounds: the len is 28 but the index is 91
crates\py-provers\src\scope.rs:149:18: a function symbol's node is a def
```

data-dashboard-api, kitaru, mngr, noesis, posthog, posthog-foss, posttrain and
sqlfluff. It reproduces on the binary built before this branch's change, so the
widening is not the cause.

The cause sits outside this unit's slice. `build.rs` sets `Module.id` from
`enumerate()` over `tree.modules`, then inserts each module into
`facts.modules` keyed by qname, and `Scope::func_def` reads
`facts.modules.get_index(self.module)`. Two source modules that share a qname
make the second `insert` overwrite rather than grow the map, so every module
after it holds an `id` one past its own index and `Scope` reads another
module's node list. It stops the binary on 8 of 64 public Python trees, posthog
among them.
