# Mock-seam survey (#9 test-writers arm, 2026-08-28)

Method: AST walk of every test-path file in the three corpus repos and the
gauntlet clones that mock at all; a rebind is `monkeypatch.setattr` (object
or string target), `patch` / `mocker.patch` / `patch.object`, resolved
against the repo's own module set. Probe scripts in the session scratchpad,
not committed (CLAUDE.md traps).

## Shapes, per repo (test sites)

| repo | obj-patch | str-patch repo at-def | str-patch repo at-use | str-patch external | env/dict |
| --- | --- | --- | --- | --- | --- |
| lol-predictor | 408 | 46 | 6 | 3 | 7 |
| merged-calculator | 252 | 43 | 28 | 7 | 159 |
| ROFL-File-Information | 120 | 13 | 17 | 22 | 11 |
| forge | 173 | 3 | 24 | 61 | 18 |
| langextract | 78 | 21 | 2 | 121 | 20 |
| dj-stripe | 128 | 6 | 3 | 819 | 0 |
| claude-bug-bounty | 32 | 18 | 0 | 23 | 27 |
| chatgpt2api | 29 | 4 | 29 | 11 | 5 |
| authlib | 9 | 6 | 0 | 42 | 2 |

Ten more clones mock at fewer than 20 sites or not at all.

## Priced non-moves

| shape | population | verdict |
| --- | --- | --- |
| Per-site "test rebinds a repo symbol" (anti-slop `no-module-mocking`) | 460 sites on lol-predictor alone; `monkeypatch.setattr` is pytest's documented seam | not built: #45 `testing-the-double` priced the judges' read of doubles at 0/5; a per-site rule has no reading a judge accepts |
| Inert patch: patched at the def while the exercised code reads the name through a `from M import f` binding | 0 of 909 module-target patches across 12 repos; every at-def patch is on the module the test also calls | not built: null population |

## The arm's population

Prod symbols rebound from >= 3 test modules: lol-predictor 5 (survey) / 9
(sightline, whose resolver follows re-exports), merged-calculator 1 / 4,
dj-stripe 2 / 2, claude-bug-bounty 1 / 1, ROFL, forge, langextract,
chatgpt2api, featuretools, authlib, django-unfold 0. Judged sheets:
`corpus-ext/sheets/test-writers-*.wave1.tsv`.
