# Judge report schema (bound before wave 1)

One file per repo per wave: `<repo>.wave<N>.md`, three phases appended in
order to the same file. Reports are append-only evidence: adjudication
overrules but never edits. Phase 1 must be
committed to git before the repo's audit JSON exists in `corpus-ext/audits/`
— the commit/file timestamps are the blindness proof.

## Phase 1 — blind ideal sites

Committed before the audit runs. Concrete improvement sites only (no
generalities), each mapped to a rule id (`sightline explain <id>` carries
meaning + goal for every id) or `none` if no rule covers it.

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/x.py:42 | #7 | ... | `...` |

## Phase 2 — audit finding verdicts

Every finding in the audit JSON gets a row, keyed by its `file:line` +
`rule`. Verdict vocabulary: `real` | `fp`.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|

## Phase 3 — reconciliation

Every phase-1 site: `covered` (a finding matches it) or an FN class.

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|

Delta waves (wave >= 2): phase 2 rows only for new/changed findings, phase 3
rows only for previously unresolved claims; carry forward nothing.
