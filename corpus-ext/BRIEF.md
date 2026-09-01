# Judge briefs (verbatim templates; the committed coaching-check artifact)

`{repo}` = repo dir name, `{root}` = absolute path under `../gauntlet-corpus/`,
`{sightline}` = absolute path of this repo.

Committed reports and configs spell that root as `<GAUNTLET_CORPUS_ROOT>`; a
path beside it, not under it, spells as `<GAUNTLET_CORPUS_ROOT>/..`. Resolve
either against your own `../gauntlet-corpus/` clone. `corpus-ext/scrub_paths.py`
did the substitution mechanically: path spellings only, no judgment, count,
verdict, SHA or repository name changed.

## Phase 1 (spawn)

> You are a blind code-quality judge in a calibration campaign for an
> agent-ergonomics checker. You judge one Python repo cold, against a fixed
> set of ideals — you have never seen the checker's output and must not
> seek it.
>
> Repo under judgment (read-only, never edit or execute it): `{root}`
>
> The ideals: the rule inventory is #1-56 (51 and 52 reserved). For the precise meaning and goal
> of ANY rule run
> `{sightline}/.venv/Scripts/python -m sightline.cli explain <ruleid>`.
> Rule #41 is perf-in-hot-code-only: map a site to it only when the code is
> plausibly hot; cold-glue perf nits map to `none`.
>
> Forbidden, and grounds for discarding your report: running
> `sightline audit`, `gate`, or `fix` on anything; reading anything under
> `{sightline}/corpus-ext/`, `{sightline}/corpus/`, `{sightline}/docs/`,
> `{sightline}/src/`, or `{sightline}/tests/`; reading other judges'
> reports.
>
> Task — phase 1 of 3 (phases 2-3 come later in this conversation): read
> the repo cold and commit to a list of concrete improvement sites — places
> where the code falls short of the ideals in a way a competent reviewer
> would flag. Every site maps to one rule id, or `none` if no rule in the
> inventory covers it. Concrete means path:line and a claim specific enough
> to verify; no generalities, no quota — quality over volume, but be
> thorough: cover the whole prod tree, not the first files you open.
>
> Write your report to `{sightline}/corpus-ext/reports/{repo}.wave1.md`
> following the Phase 1 table in
> `{sightline}/corpus-ext/reports/SCHEMA.md` exactly. Then reply PASS with
> your site count (or BLOCKED with the reason). Your reply is bookkeeping;
> the report file is the deliverable.

## Phases 2-3 (continuation, sent only after the phase-1 report is committed)

> Your phase-1 list is committed. The checker's audit of this repo is now
> at `{audit_json}` — findings carry file, line, rule, tier, message.
>
> Phase 2: judge every finding at its site in `{root}` — verdict `real`
> (the finding names a genuine instance of its rule's ideal being violated)
> or `fp` (it does not), with a one-line why. Judge the site, not the
> wording.
>
> Phase 3: reconcile against your phase-1 list. Every phase-1 site is
> `covered` (some finding matches it) or a miss: `detector-miss` (a rule
> exists and should have fired), `threshold-miss` (rule fired elsewhere or
> nearly; site falls under a cutoff), `inventory-gap` (you mapped it
> `none`). Append the Phase 2 and Phase 3 tables to your report file per
> the schema. Do not edit phase-1 rows — they are committed evidence.
> Reply PASS with counts (findings judged, real, fp, covered, misses).

## Rounds 3+ (precision sheets; one phase)

`{sheet}` = `corpus-ext/sheets/{repo}.wave{N}.tsv`, `{notes}` =
`corpus-ext/reports/{repo}.g{round}.md`.

> You are a code-quality judge in a precision-calibration round for an
> agent-ergonomics checker. You judge one Python repo's audit findings at
> their sites and record one verdict per finding.
>
> Repo under judgment (read-only, never edit or execute it): `{root}`.
> The findings: `{sheet}`, a TSV with one row per finding (key, rule, slug,
> arm, tier, file, line, symbol, message, verdict, why), sorted by rule.
>
> The ideals: the rule inventory is #1-60 (4, 8, 13, 15, 16, 17, 19, 22, 25, 30, 43, 45, 46, 51, 52 retired). For the precise
> meaning and goal of ANY rule run
> `{sightline}/.venv/Scripts/python -m sightline.cli explain <ruleid>`.
> Rule #41 is perf-in-hot-code-only.
>
> Verdict per row: `real` — the finding names a genuine instance of its
> rule's ideal being violated at that site, one a competent reviewer of
> this repo would accept a change for; `fp` — it does not (the site is
> fine, the rule misread it, the "fix" would make the code worse, or the
> claim is false). Judge the site, not the wording; a true claim whose
> change would be a net loss for this code is `fp`. `why` is one line, and
> for every `fp` it names the reason concretely enough that a rule author
> can act on it. Never leave a row blank, never change a key, never drop or
> add rows; write only the verdict and why columns (a script that reads
> the TSV, fills them, and writes it back is the expected tool).
>
> Work rule by rule, writing the sheet after each rule; read the site and
> enough of its file to know what the code is for; when a rule fires many
> times on one shape, establish the shape once and verify a spread of its
> instances, but every row still gets its own verdict. Foreground only:
> never end your turn to wait on a command. When the sheet is complete,
> write `{notes}`: a table `| rule | fp class | count | example key |` of
> the false-positive shapes you saw (one row per shape, counts summing to
> the rule's fp total) and nothing else. Then reply PASS with rows / real
> / fp (or BLOCKED with the reason). The sheet and the notes are the
> deliverable; the reply is bookkeeping.
>
> Forbidden, and grounds for discarding your work: running `sightline
> audit`, `gate`, or `fix`; reading anything under `{sightline}/src/`,
> `{sightline}/tests/`, `{sightline}/docs/`, `{sightline}/corpus/`, or
> `{sightline}/corpus-ext/` other than your own sheet and notes.

Delta waves (wave >= 2): the sheet carries every earlier verdict; the judge
fills only blank rows, and the notes file gains a `## wave N` section.
