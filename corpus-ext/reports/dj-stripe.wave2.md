# dj-stripe — blind judge report (wave 2, delta)

Repo: `<GAUNTLET_CORPUS_ROOT>\dj-stripe`
Audit: `corpus-ext/audits/dj-stripe.wave2.json` — 652 findings (wave 1: 2150).
Delta computed on `(file, line, rule, symbol, message)`: **3 new, 1501 vanished,
649 unchanged.** Provenance is unchanged from wave 1 (family P still silent,
python-env still unresolved, 296 unresolved imports).

Wave-1 rows are not edited. This file carries phase-2 rows for new/changed
findings only, a vanished-population audit, and phase-3 rows for claims whose
status changed.

## Correction to wave 1 (own error, found by the delta)

The coordinator's brief names 2 new findings; the diff shows **3**. The third is
`djstripe/settings.py:203` (#22), judged below.

More importantly, the delta exposed a **wave-1 judging error of mine**. My
wave-1 grouped row "djstripe/enums.py — 431 enum members ... fp" silently
absorbed 6 findings that are *class*-level, not member-level, and 5 of those are
true positives I should have separated:

| finding | verdict (corrected) | why |
|---|---|---|
| djstripe/enums.py:436 `IntentStatus` | real | Defined and never used; the only other mention is a TODO comment at :479 ("then PaymentIntentStatus/SetupIntentStatus can inherit from IntentStatus"). |
| djstripe/enums.py:470 `OrderStatus` | real | Definition only, repo-wide. |
| djstripe/enums.py:747 `SourceCodeVerificationStatus` | real | Definition only. |
| djstripe/enums.py:753 `SourceRedirectFailureReason` | real | Definition only. |
| djstripe/enums.py:759 `SourceRedirectStatus` | real | Definition only. |
| djstripe/enums.py:894 `DjstripePaymentMethodType` | fp | Documented public API — `docs/changes/2_x.md:113` and `:205` tell users to migrate *to* it. Same class as `Subscription.is_period_current`. |

**Wave-1 rule #32 is therefore restated as 10 real / 565 fp** (was 5/570), and
the wave-1 totals as **878 real / 1272 fp**. Grouping a 431-instance family by
file rather than by message shape cost me five true positives; that is a
judging-method defect on my side, not a checker defect. All six survive into
wave 2.

## Phase 2 — new/changed findings

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| djstripe/models/base.py:469 | 5 | indexed | real | New. `_stripe_object_field_to_foreign_key` declares `stripe_account=None` with no annotation; its one prod caller (base.py:434) forwards `stripe_account: str \| None` from `_stripe_object_to_record`. The proposed `None \| str` is exactly the established invariant, the body only forwards it, and the receipt reports no caller errors. Instance of the wave-1 #5 class (all 3 wave-1 #5s were real); the union-join worked correctly here. |
| djstripe/event_handlers.py:54 | 5 | indexed | real (with a defect — see NEW FP CLASS 1) | New. `djstripe_receiver(signal_names)` is an unannotated parameter on the module's public decorator, so the site genuinely wants a lift. But the proposed `list[str] \| str` is **over-narrow against the function's own body**: line 88 is `isinstance(signal_names, (list, tuple))` and the docstring at :65 says "(list or tuple or str)". 102 call sites happen to pass only `str`/`list`, so the join is "established" — and applying it would encode a type the body and the docs both contradict. |
| djstripe/settings.py:203 | 22 | heuristic | fp | New, and an instance of the wave-1 #22 class, not a new one: `DjstripeSettings.get_subscriber_model` calls its own public `get_subscriber_model_string()` and is part of the settings object's published API (`djstripe_settings.get_subscriber_model()`). The wave-2 state gate narrowed #22 from 71 to 3 but did not close the class — it still cannot separate "free function hiding in a class" from "the object's published behaviour", and it manufactured a fresh instance while doing so. |

Changed findings: none. Every one of the 649 survivors carries a byte-identical
`(file, line, rule, symbol, message)` tuple, so all wave-1 verdicts on them
stand (as corrected above).

## Vanished-population audit

1501 findings vanished. Sampled 24 named sites across the 7 largest classes,
plus **full-population correlations** for the two classes where a sample would
have been misleading (#39-ratio and #32). Verdict per class is on whether the
removal cost true positives.

| class (rule, wave1 -> wave2) | sampled / correlated | lost TPs? |
|---|---|---|
| #32 enum members (431 -> 0) | Correlated: all 431 member-level rows gone, all 6 class-level rows kept | **No** — clean. The exemption discriminates member-of-enum (consumed via `__choices__`) from the enum class itself, which is exactly the right cut. This is the single best fix in the wave. |
| #32 admin declarative (71 -> 0) | Spot-checked admin.py:34 `list_display`, :133, :152 `search_fields`, admin_inline.py:16, :46, forms.py:81, actions.py:20, views.py:23 (8) | **No** — all 8 were wave-1 fps. |
| #32 ORM fields (52 -> 0 fields, 9 `Meta` rows kept) | Spot-checked payment_methods.py:255, sigma.py:43, identity.py:16, connect.py:121, core.py:1577 (5) | **No** for fields. But the `Meta` inner-class rows (`db_table` x5, `unique_together` x2, `ordering`, `get_latest_by`) and `urls.py` `app_name`/`urlpatterns` **were not exempted** — 11 of my wave-1 fps survive untouched. |
| **#32 dead methods and classes (5 -> 1)** | Checked all 5 wave-1 TPs directly | **YES — 4 lost.** `core.py:1484 _api_cancel`, `core.py:1495 _api_confirm`, `pricing_table.py:9 PricingTable`, `pricing_table.py:13 .merchant` all vanished; only `settings.py:22 ZERO_DECIMAL_CURRENCIES` survives. See NEW FN CLASS 1. |
| #22 velcro (71 -> 3) | Spot-checked test_views.py:58, tests/__init__.py:160, fields.py:24, core.py:84, billing.py:1315, base.py:577 (6) | **No** — all 6 were wave-1 fps. But 2 old fps survive (admin/actions.py:90, :98) and 1 new fp was added (settings.py:203). |
| **#11 test block clones (737 -> 0)** | Checked the 4 largest/highest-repetition sites I had called real: test_views.py:545 (4 stmts x13), test_account.py:539 (x10), test_event_handlers.py:2432 (12 stmts), :2332 (5 stmts) | **YES — all 4 gone, and with them all 365 block clones I judged real** (>=5 statements or >=10 repetitions). The 372 I judged fp are correctly gone too. The exemption is test-tree-wide and size-blind. See NEW FN CLASS 2. |
| #11 djstripe (28 -> 27) | Checked base.py:908, core.py:1779, admin/views.py:27 survive; only process_events.py:53 removed | **No** — the one removal was my sole #11 fp (the coincidental 3-assignment cross-tree match). Prod block clones were correctly kept. |
| **#39 ratio arm (86 -> 53)** | Correlated all 86 against "docstring contains a reST `:param:`/`:type:`/`:returns:` field": vanished = 32 with a field / 1 without; survived = 10 with / 43 without | **YES — 32 lost.** The fix exempts reST-documented functions, which *is* the `:param api_key: ... Defaults to djstripe_settings.STRIPE_SECRET_KEY / :type api_key: string` boilerplate family I judged real, while keeping the Stripe-semantics docstrings I judged fp. Net effect: 32 TPs removed, 1 FP removed. Spot-verified lost: checks.py:15, base.py:62, :214, :706, core.py:1495. Spot-verified surviving fps: account.py:46, :59, webhooks.py:117, base.py:308, managers.py:168, core.py:711. See NEW FN CLASS 3. |
| #39 history arm (34 -> 10) | Checked all 5 wave-1 TPs + the 10 survivors | **YES — 2 lost**: `_stripe_errors.py:78` ("replaces what used to be a per-object ladder") and `tests/__init__.py:4` ("Originally collected using API VERSION 2015-07-28"). 7 of my 29 fps survive (base.py:380, core.py:507, :1519, tests/__init__.py:1135, test_event_handlers.py:2173, test_subscription.py:1093, :1096) — the word-trigger bug I named is narrowed, not fixed. |
| #39 restate arm (12 -> 11) | Checked all 12 | **YES — 1 lost**, and it is the *only* removal: `apps.py:32 # Set app info`, which I judged real. All 3 of my restate fps (tests/__init__.py:368, :386, asserts.py:19) survive. This arm moved backwards by one on both axes. |
| #14 data-clump (84 -> 32) | Checked the 6 djstripe rows | **No.** The 4 removals at account.py:220 are 4 of the 5 redundant nested subsets of one clump — the dedup I asked for; the site is still named (account.py:220 + base.py:824 both survive). Tests: 78 -> 30, so 48 of my fps went and 30 remain. |
| #25 rename-delegation (16 -> 0) | All 16 were wave-1 fps | **No** — clean, but see the phase-3 regression on P1-19. |
| #4 interprocedural (2 -> 0) | Both were wave-1 fps (both actively falsified) | **No** — clean. |
| #30 Demeter (6 -> 1) | Checked all 6 | **No** — the 5 removed were fps; the 1 survivor (admin/actions.py:35) is also a wave-1 fp. |
| **#33 return-honesty (17 -> 13)** | Checked all 4 removals | **YES — 1 lost.** The "mixes value returns with bare returns" arm was cut whole. It took my 3 fps (account.py:154, billing.py:1295, core.py:1310) *and* `event_handlers.py:430`, which I judged real: `_handle_crud_like_event` returns a model, `None`, or — on the delete path at line 472 — the `(int, dict)` tuple from `QuerySet.delete()`. Cutting the arm rather than fixing its "bare return" test discarded the arm's one true positive. |
| #28 doc-path (17 -> 15) | Checked all 17 | **No lost TPs** (there were none), but only the 2 `release.yml` fps were fixed. The 12 changelog-removal fps, the 2 Django check-id fps and the `docker-compose.yaml` fp all survive — my largest #28 class is untouched. |

**Lost true positives: 40 unambiguous** (#32 x4, #33 x1, #39 x35) **plus 365
ratchet-tier #11 test block clones** where my wave-1 line (>=5 statements or
>=10 repetitions) was a judgment call the fix declined to honour. Total 405.

## Bottom line — new classes

Yes: **three FN classes and one FP class that are not in my wave-1 report.**

**NEW FN CLASS 1 — #32's framework exemption is model-scoped, not
declaration-scoped.** Wave 1's FP class was "framework-*declarative* attributes
(fields, `Meta`, admin options) are ORM/registry-consumed". The fix instead
appears to exempt *anything defined on a Django-model-derived class*, so it
silenced a whole unimported model class (`PricingTable`) and two genuinely dead
private methods (`_api_cancel`, `_api_confirm`) — 4 of the rule's 5 true
positives. The tell is that `Meta` options and `urls.py` module globals, which
*are* the declarative class, were left in. The exemption is aimed at the wrong
axis: it should key on "is this name consumed by a framework registry" (fields,
`Meta`, `list_display`, `urlpatterns`, enum members), not on "does this symbol
live in a model".

**NEW FN CLASS 2 — #11's test exemption is size- and count-blind.** My wave-1
row drew the line explicitly: 3-4-statement arrange/act blocks are locality, not
duplication; blocks of >=5 statements or repeated >=10 times are genuine
fixture/parametrize sites. The fix removed the whole test block-clone arm, so a
12-statement duplicated block (test_event_handlers.py:2432) and a 4-statement
setup written out 13 times (test_views.py:545) now report nothing, while the
82 whole-function test clones survive. Rule #11 is a ratchet, so a conservative
cut is defensible — but the *stated* wave-1 class was narrower than what shipped.

**NEW FN CLASS 3 — #39's ratio arm was inverted, not fixed.** This is the most
serious regression in the wave and the one I did not predict. The exemption
tracks the presence of a reST field (`:param:`/`:type:`/`:returns:`) with
near-perfect correlation: 32 of 33 removals had one, 43 of 53 survivors did not.
But reST boilerplate *is* the restatement — `:param api_key: ... Defaults to
djstripe_settings.STRIPE_SECRET_KEY` restates a default that is already in the
signature, and `base.py:706` is a 50-line docstring of empty `:param` stubs.
What survives is the class I called fp: docstrings carrying Stripe field
semantics, doc URLs and hook contracts (account.py:46/:59, webhooks.py:117,
base.py:308). The arm now fires on the docstrings that earn their length and is
silent on the ones that do not. My wave-1 row named the discriminator — "the
surplus restates the signature" vs "carries external semantics" — and the fix
used the opposite proxy.

**NEW FP CLASS 1 — #5 lifts without body-usage widening.** `event_handlers.py:54`
proposes `signal_names: list[str] | str` for a parameter whose own body does
`isinstance(signal_names, (list, tuple))` and whose docstring says "list or tuple
or str". The call-site join is correct and the counterfactual receipt passes;
what is missing is the widening step that SUMMARY §4.3 names as the *required*
mitigation for this rule ("body-usage widening to protocols ... over-narrow lifts
are the design's main false-positive engine"). Wave 1's three #5 findings were
all simple `str` lifts with no body-usage conflict, so this class could not
appear then. A lift should be checked against the guarded branches in its own
body before it is proposed; here the body is one `isinstance` away.

Persisting classes worth restating, since the fix did not reach them: the #28
changelog-removal FPs (12), the #22 published-behaviour FPs (now 3, one of them
newly minted), the #39 history-arm word triggers (7), and the #32 `Meta`/`urls`
declarative FPs (11).

## Phase 3 — reconciliation delta

Only claims whose status changed since wave 1 are listed. No previously
unresolved phase-1 site became covered: all 3 new findings land on sites that
were not in my phase-1 list.

| P1 id | rule | class (was -> now) | note |
|-------|------|--------------------|------|
| P1-19 | #13 | covered -> detector-miss | The 5 #25 findings at managers.py:49-65 were the only findings naming that site; #25 went to zero, so the five one-line `with_status` forwarders are now unreported by any rule. Removing a whole-rule FP class also removed the only coverage of a real #13 site. |
| P1-56 | #32 | covered -> detector-miss | `PricingTable` was one of two phase-1 sites the checker got exactly right; NEW FN CLASS 1 silenced it. |
| P1-47 | #33 | covered (unchanged) | Retained: the #39 finding at checks.py:15 was lost to NEW FN CLASS 3, but #5 and #13 still name the site. |

All other phase-1 classifications stand: **26 covered** (was 29), 45
detector-miss (was 43), 15 threshold-miss, 9 inventory-gap, and the two large
threshold-misses P1-1/P1-2 (the 321-instance `stripe_data.get` accessor family)
remain entirely unreported — nothing in this wave moved the clone floor.

---

# Wave 3 (confirmation delta)

Audit: `corpus-ext/audits/dj-stripe.wave3.json` — 647 findings (wave 2: 652).
Delta on `(file, line, rule, symbol, message)`: **5 new, 10 gone, 642
unchanged.** Rule movement: #5 5->4, #11 109->110, #32 21->12, #39 74->78.
Provenance unchanged.

## Phase 2 — new findings (5)

| finding (path:line) | rule | tier | verdict | class |
|---------------------|------|------|---------|-------|
| djstripe/models/billing.py:607 | 39 | heuristic | real | **Resurrected TP.** `UpcomingInvoice.default_tax_rates`, 4 prose lines over 1 code line: "Gets the default tax rates associated with this upcoming invoice." restates the property name, and the `:return:` field is empty. Exactly the wave-1 verdict; the contentful-field requirement put it back. |
| tests/apps/example/.../regenerate_test_fixtures.py:344 | 39 | heuristic | real | **Resurrected TP.** `fake_json_ids`, 7:3 — "Replace real ids with fakes ones in the JSON fixture" plus `:param`/`:return` stubs over a 3-line body. |
| tests/apps/example/.../regenerate_test_fixtures.py:357 | 39 | heuristic | real | **Resurrected TP.** `unfake_json_ids`, 8:7 — the mirror docstring of :344. |
| djstripe/event_handlers.py:191 | 39 | heuristic | fp | **Resurrected FP**, instance of the wave-1 #39-ratio FP class. `handle_payment_method_event`'s 17 prose lines carry the detach rationale (why a `card_`-prefixed detach is treated as a delete, why legacy `src_…` sources 404 on the payment-methods endpoint) — non-derivable external behaviour, which is what a docstring is for. |
| djstripe/management/commands/djstripe_process_events.py:53 | 11 | indexed | fp | **Resurrected FP**, and it is the single #11 false positive I named in wave 1. Verified again in source: `event_ids = options["ids"]; failed = options["failed"]; type_filter = options["type"]` (process_events.py:53-55) against `id_ = old_obj["id"]; customer_id = old_obj["customer"]; type_ = old_obj["type"]` (regenerate_test_fixtures.py:608-610) — three subscript-assignments off unrelated dicts in unrelated code, with no shared concept to extract. The *rule* behind the restoration is right (a clone with a prod member must not be exempted just because its twin lives in tests); this instance is the pre-existing "three trivial assignments normalise identically" threshold issue, not a new class. |

New: **3 real, 2 fp.**

## Gone (10) — no lost TPs

| gone | verdict in wave 1 | cost |
|---|---|---|
| 9 x #32 nested `Meta` options — base.py:134 `get_latest_by`, base.py:1098 + billing.py:84 `unique_together`, billing.py:323 `ordering`, issuing.py:17/:46/:86/:119/:148 `db_table` | all fp | none |
| 1 x #5 event_handlers.py:54 `signal_names: list[str] \| str` | real site / defective proposal | none — the correct `base.py:469` lift survives |

**Zero true positives lost this wave.** All four wave-1/2 claims fixed here were
removals of findings I had judged false.

## Confirmation of wave-2 claims

**NEW FP CLASS 1 (#5 lifts without body-usage widening) — RESOLVED.** The
`signal_names` lift is gone, killed by a body-`isinstance` guard, which is
precisely the widening step SUMMARY §4.3 names as the required mitigation.
Discrimination confirmed rather than blunt suppression: `base.py:469`
(`stripe_account: None | str`, no body guard, single caller, correct join)
survives, as do all three wave-1 #5s. Rule #5 is now 4 findings, 4 real.

**Meta residue (#32) — RESOLVED for the nested-`Meta` half, not for the rest.**
All 9 `Meta` rows I named are gone. The other 2 I named in the same breath —
`urls.py:16 app_name` and `urls.py:23 urlpatterns` — are still reported, and
they are the identical defect one scope up: module-level globals read by name by
Django's URL loader. Alongside them 4 more fps persist (`apps.py:26` x3
side-effect imports, `enums.py:894 DjstripePaymentMethodType`, which
`docs/changes/2_x.md:113/:205` tell users to migrate *to*). Every one is an
instance of the registry-consumption class already recorded for v-next, so I am
not re-litigating — but the exemption still keys on *nested inside a `Meta`
class* rather than on *consumed by a framework registry*, which is why it stops
at the class boundary.

Rule #32 now stands at **12 findings, 6 real / 6 fp** — precision 50%, up from
10/575 (1.7%) in wave 1, with 5 of its 6 TPs being the dead enum classes my own
wave-1 grouping error had buried.

**#39 ratio arm — partially recovered.** The contentful-field requirement
restored 3 of my true positives and 1 false positive. Of the 50 ratio findings I
judged real in wave 1, **26 are now reported and 24 are still silent**: base.py
:62/:214/:257/:282/:706/:824/:848, billing.py :946/:1183/:1519/:1552/:1581,
core.py :825/:1443/:1465/:1484/:1495, connect.py:382, account.py:194,
payment_methods.py:436, checks.py:15, event_handlers.py:412, settings.py:131,
utils.py:15. These are one family: the `:param api_key: The api key to use for
this request. Defaults to djstripe_settings.STRIPE_SECRET_KEY. / :type api_key:
string` block, pasted across 20 methods. Its fields are *contentful* by any text
measure and *pure restatement* by meaning — the default is in the signature, the
type is in the name. `base.py:706` shows the exemption's shape most clearly: 50
docstring lines that are mostly empty `:param data:` / `:param field_name:` /
`:return:` stubs, exempted because one field (`:param current_ids:`) does carry
prose. The predicate is any-contentful-field; it needs to be does-the-content
add anything the signature lacks. This is the wave-2 NEW FN CLASS 3, narrowed by
about an eighth, not closed.

## Bottom line

**No remaining new class.** Every unresolved item in wave 3 is an instance of a
class already named in wave 1 or wave 2:

- #32 `urls.py` globals, `apps.py` side-effect imports, `DjstripePaymentMethodType`
  — registry-consumption / documented-public-API classes (wave 1), deferred to v-next.
- #39 ratio residue (24 sites) — wave-2 NEW FN CLASS 3, narrowed.
- #39 `event_handlers.py:191` — wave-1 ratio FP class (prose carrying external semantics).
- #11 `process_events.py:53` — wave-1 #11 FP (trivial-assignment normalisation), plus
  the wave-2 test-block trade, both recorded.
- #11 P1-1/P1-2 — the 321-instance `stripe_data.get` accessor family is still
  wholly unreported across all three waves; the clone floor has not moved.

Phase-3 delta: **P1-19 regressed further** — the #25 washout in wave 2 removed
its only coverage and nothing in wave 3 restored it. No previously unresolved
phase-1 site became covered. Standing tally: **26 covered, 45 detector-miss,
15 threshold-miss, 9 inventory-gap.**
