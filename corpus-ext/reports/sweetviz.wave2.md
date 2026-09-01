# sweetviz — wave 2 (delta)

Audit: `corpus-ext/audits/sweetviz.wave2.json` (354 -> 322; 37 vanished, 5 new).
Phase-1 list is not re-opened; wave-1 rows stand as committed evidence.
Re-tallied totals: **282 real / 40 fp** (wave 1: 294/60) — precision 83.1% -> 87.6%.

## Phase 2 — new/changed findings

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| sweetviz/graph_numeric.py:227 | #34 | heuristic | real | the 25-line commented-out block (:227-251) is the largest in the repo; this is the wave-1 detector-miss I named at P1-57 — **existing class, recall fixed** |
| sweetviz/graph_cat.py:227 | #34 | heuristic | real | the 7-line commented-out `# else: # TARGET BOOL: NO compare TARGET` block; second half of my P1-55, whose `:203` half already fired — **existing class** |
| sweetviz/sv_html.py:355 | #2 | heuristic | fp | `compare_dict` is annotated bare `dict`, but the sole caller (`generate_html_detail`:59) passes `feature.get("compare")`, which is None on every no-compare run — the guard is live — **existing class** (wave-1 #2 fps at sv_html.py:136/:184/:214, identical cause) |
| sweetviz/sv_html.py:423 | #2 | heuristic | fp | same site and same cause as :355, in `generate_html_detail_text` — **existing class** |
| sweetviz/sv_html.py:454 | #2 | heuristic | fp | same site and same cause, inner guard of `generate_html_detail_text` — **existing class** |

**These three are not new sites.** They are the exact three lines that carried
`#4` findings in wave 1, which I judged `fp` for the same reason. The `#4` arm
was retired at those lines and the identical unsound claim re-emitted under
`#2`. The false positive migrated rules; it was not resolved. Two of the five
wave-1 `#4` fps (`dataframe_report.py:523`, `sv_html.py:102`) survive unchanged,
so `#4` is now 0 real / 2.

## Phase 2b — vanished findings, checked for lost true positives

All 37 sampled (task floor was 12), grouped by rule with my wave-1 verdict.

| rule | vanished | my wave-1 verdict | assessment |
|------|----------|-------------------|------------|
| #2 | dataframe_report.py:52, :84, :129, :286 | all `fp` | correct kills — the four implicit-Optional-default cases. But see the P1-88 regression below. |
| #4 | sv_html.py:355, :423, :454 | all `fp` | net zero: re-emitted as #2 at the same lines. |
| #11 | series_analyzer_cat.py:33, :106; series_analyzer_text.py:24 (all `x3 (3 stmts)`) | all `real` | **no site lost**: the surviving `x2 (4 stmts)` findings at :33, :106 and :22 name the same duplication. These were redundant overlapping views. |
| #12 | dataframe_report.py:422, :425 | both `real` | **correct fix** — exactly the dedupe bug I reported (duplicate `rule`+`file:line` differing only in `symbol`). One copy of each survives. |
| #21 | dataframe_report.py:23 (`generate_comet_friendly_html()` recurs) | `fp` | correct kill. |
| #22 | dataframe_report.py:319; feature_config.py:28, :40; sv_types.py:86 | all `real` | 4 nominal lost TPs — **and I now think the removals are right, i.e. my wave-1 verdicts were too literal.** Meyers' encapsulation count does not transfer to a language with no access control: in Python every attribute is public, so velcro-100% fires on any method that merely reads its own instance state. The one survivor, `graph.py:24` (a class of 8 statics whose `__init__` no subclass calls), is the only shape of #22 worth reporting here. |
| #33 | dataframe_report.py:323 (`__getitem__` mixes value/bare returns) | `real` | **genuine lost TP.** I flagged the wording as wrong in wave 1 (all returns are explicit; none are bare), but the site — a mapping accessor silently returning None instead of raising KeyError — is real and now has zero coverage. The annotated cases (`get_target_type`, `get_type`) still fire, so only the unannotated-accessor case was lost. |
| #39 | 16 findings | 12 `fp`, 4 `real` | the big win: all 12 fps of the three classes I named (verbatim BSD license blocks ×2, numpydoc API docstrings and parameter descriptions ×7, script usage header ×1, `__init__.py` present-tense comment, `sv_html_formatters.py:63`). **4 lost TPs**: `series_analyzer.py:10` (a comment restating `value_counts()`), and `series_analyzer.py:29`, `:45`, `series_analyzer_numeric.py:65` (commented-out code the restatement arm was catching). Those four sites now have zero coverage — #34's block arm does not reach them because each is 1-2 lines, under the block bar. That is my wave-1 P1-30 threshold-miss losing its accidental backstop. |
| #40 | sv_html.py:92; sv_html_formatters.py:7, :29 | all `fp` | correct kills — all three were noun-modifier plurals. #40 is now silent in this repo; P1-24 (`count_fraction_of_true`) stays a detector-miss. |

**Lost true positives: 9 findings by my wave-1 verdicts** (#22 ×4, #33 ×1, #39
×4), of which I retract the four #22 calls on reflection. **5 sites are left
with no coverage at all**: `dataframe_report.py:323`, `series_analyzer.py:10`,
`:29`, `:45`, `series_analyzer_numeric.py:65`.

One residual fp of a class the fix otherwise closed: `update_jquery.py:5` still
reads "comment narrates history" on the script's "This should be run from the
root sweetviz directory" instructions header, while its twin at `:1` was
correctly removed. #39 is now 5 real / 6.

## Phase 3 — previously unresolved claims, re-reconciled

Only rows that changed class. All other wave-1 phase-3 classes stand.

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-57 | #34 | covered | 34\|graph_numeric.py:227 — the 25-line block now fires; was the starkest detector-miss in wave 1 (a 3-line block in utils.py caught, this one missed) |
| P1-88 | #34 | detector-miss | **regression**: the only finding covering this site was 2\|dataframe_report.py:129, removed as an implicit-Optional fp. The site's real defect — `(0 if target_feature_name is not None else 0)`, both arms `0` — is genuine and now unreported. Removing a false positive removed the only signal at a true site. |

Wave-1 misses that remain open and unchanged: #6, #7, #8, #9, #26, #37 still
produce no finding anywhere in this repo; family P (#41) is still silent by
provenance ("no hot-roots config"), costing all five hot-path sites; #12's
catalog still lacks the ten idioms listed in wave 1; #32 still does not cover
unread locals (six sites); #19 still matches only `list.index()`.

## Bottom line

**No new FP class and no new FN class.** Every one of the 5 new findings and
every one of the 9 lost TPs falls inside a class already named in the wave-1
report. Three things are worth the adjudicator's attention anyway:

1. **A fix that migrated FPs across rules.** `#4` at `sv_html.py:355/:423/:454`
   became `#2` at the same three lines with the same unsound claim. The `#2`
   fix suppressed no-overlap when the *declared default* is None but not when
   the *caller* supplies None via `.get()`, so #2 is still 4 real / 25 — its
   precision barely moved (0.154 -> 0.160) while #39's went 0.41 -> 0.83.
2. **Removing an FP can uncover a true site.** P1-88 regressed from covered to
   detector-miss because its only finding was a correctly-killed fp. Wherever a
   rule is the sole reporter at a line, an fp fix needs a check that no genuine
   defect at that line goes dark.
3. **The #39 fix cut deeper than the fp classes.** 12 of 16 removals were the
   fps I named; the other 4 were restatement/commented-out-code findings I
   judged real, and those sites have no other rule reaching them. If the
   restatement arm was narrowed rather than the license/docstring cases
   excluded specifically, that is over-correction worth re-checking.
