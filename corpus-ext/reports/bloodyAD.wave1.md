# bloodyAD — wave 1 (blind ideal sites)

Repo judged cold against research/SUMMARY.md §3-4. Prod tree only
(`bloodyAD/**`, excluding `tests/`, `.github/`, and vendored Impacket
`formatters/structure.py` which carries a Fortra copyright header).

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | bloodyAD/network/config.py:24 | #1 | Public dataclass field typed bare `list` and defaulted `None` — weak boundary type that also lies (never a list until `__post_init__`) | `krb_args: list = None` |
| P1-2 | bloodyAD/utils.py:637 | #1 | Public helper params typed bare `list` (no element type) on a repo-wide-reachable function | `async def connectReachable(conn, srv_names: list, ports: list = [389, 636, 3268, 3269]):` |
| P1-3 | bloodyAD/network/ldap.py:34 | #1 | Bare `list` return annotation (also line 49 `showRecoverable`) — no element type at a public boundary | `def phantomRoot() -> list:` |
| P1-4 | bloodyAD/cli_modules/get.py:53 | #1 | `zone: str = None` — annotation contradicts default; the CLI-facing signature claims non-optional str (same pattern remove.py:50-54 `ttl: int = None` etc.) | `async def dnsDump(conn, zone: str = None, no_detail: bool = False, transitive: bool = False):` |
| P1-5 | bloodyAD/utils.py:637 | #9 | Mutable list literal as default arg | `ports: list = [389, 636, 3268, 3269]` |
| P1-6 | bloodyAD/cli_modules/get.py:388 | #9 | Mutable list literal as default arg | `c: list = [],` |
| P1-7 | bloodyAD/cli_modules/set.py:16 | #9 | Mutable list literal as default arg | `async def object(conn, target: str, attribute: str, v: list = [], ...):` |
| P1-8 | bloodyAD/cli_modules/add.py:22 | #9 | Mutable list literal (with a real DN element) as default arg | `t: list = ["CN=Administrator,CN=Users,DC=Current,DC=Domain"]` |
| P1-9 | bloodyAD/cli_modules/set.py:31 | #9 | Rules-facing code mutates an imported badldap module-level dict at call time (shared mutable module state, cross-module) | `MSLDAP_BUILTIN_ATTRIBUTE_TYPES_ENC["msDS-AllowedToActOnBehalfOfOtherIdentity"] = typeconversion.multi_sd` |
| P1-10 | bloodyAD/utils.py:445 | #9 | Module-level mutable singleton `global_lazy_adschema` mutated from a second module (bloodhound.py:150-151 reassigns `.dn_dict`/`.conn`) | `global_lazy_adschema = LazyAdSchema()` |
| P1-11 | bloodyAD/cli_modules/set.py:331 | #9 | Monkeypatches an external exception class's `__str__` on the class object at runtime (global side effect) | `badldap.commons.exceptions.LDAPModifyException.__str__ = lambda self: error_str` |
| P1-12 | bloodyAD/cli_modules/add.py:287 | #11 | SID-resolution block (`if "s-1-" in x.lower(): sid=x else: async-for-break objectSid`) duplicated ~7× across add.py (dcsync/genericAll/rbcd) and remove.py (dcsync/genericAll/rbcd) | `if "s-1-" in trustee.lower():\n    trustee_sid = trustee\nelse: ...` |
| P1-13 | bloodyAD/cli_modules/add.py:113 | #11 | "fetch first result" loop (`entry=None; async for e in ...bloodysearch(...): entry=e; break`) reimplemented 16× across the tree instead of one helper (≈`anext`) | `entry = None\nasync for e in ldap.bloodysearch(...):\n    entry = e; break` |
| P1-14 | bloodyAD/network/ldap.py:631 | #11 | Port→scheme map literal duplicated verbatim 3× (ldap.py:631, ldap.py:726, utils.py:649) | `schemes = {389: "ldap", 636: "ldaps", 3268: "gc", 3269: "gc-ssl"}` |
| P1-15 | bloodyAD/cli_modules/get.py:401 | #11 | `attributesSD` list + the whole `resolve_sd` render loop duplicated between `object` (356-377) and `search` (401-429) | `attributesSD = ["nTSecurityDescriptor", "msDS-GroupMSAMembership", ...]` |
| P1-16 | bloodyAD/cli_modules/add.py:298 | #11 | SDFlags control construction (`req_flags = ...SDFlagsRequestValue({"Flags":...}); controls=[("1.2.840.113556.1.4.801",True,req_flags.dump())]`) duplicated in dcsync/genericAll (add.py) and dcsync/genericAll (remove.py) and set.owner | `controls = [("1.2.840.113556.1.4.801", True, req_flags.dump())]` |
| P1-17 | bloodyAD/cli_modules/remove.py:293 | #11 | `uac` old-value fetch + IndexError fallback block is a near-verbatim clone of add.py:665-683 (only the final bit-op differs) | `try:\n    entry = None\n    async for e in ldap.bloodysearch(target, attr=["userAccountControl"], raw=True): ...` |
| P1-18 | bloodyAD/cli_modules/get.py:123 | #11 | in-addr.arpa reversal block (`ip_addr=...split; decimals.reverse(); while len<4 append "0"`) duplicated at get.py:207-213 within the same file | `decimals = ip_addr[0].split(".")\ndecimals.reverse()\nwhile len(decimals) < 4: decimals.append("0")` |
| P1-19 | bloodyAD/utils.py:118 | #6 | `groupBy` mutates its caller-supplied `grouping_order` via `.pop()` — undeclared write to an argument, effect not in the name | `grouping_key = grouping_order.pop()` |
| P1-20 | bloodyAD/cli_modules/get.py:468 | #12 | Loop-shape reimplementation of `list()` as a lambda | `(lambda a: [b for b in a])` |
| P1-21 | bloodyAD/cli_modules/set.py:152 | #18 | `password` is a ~180-line function partitioned by labeled phase comments ("# Complexity check", "# Pwd length check", "# Pwd age check") — sequential-step structure that wants splitting | `# Complexity check ... # Pwd length check ... # Pwd age check` |
| P1-22 | bloodyAD/cli_modules/add.py:22 | #18 | `badSuccessor` (~190 lines) is organized into narrated phases ("# First we try to find a OU...", "# Check if one of the DCs...", "# If post patch...") | `# Check if one of the DCs is Windows Server 2025 or higher` |
| P1-23 | bloodyAD/network/config.py:34 | #18 | `__post_init__` sequenced by labeled phases ("# Resolve dc ip", "# Parse krb args", "# Handle case where password is hashes...", "# Handle case where certificate...") | `# Resolve dc ip ... # Parse krb args ... # Handle case where password is hashes for NTLM auth` |
| P1-24 | bloodyAD/cli_modules/msldap.py:12 | #22 | `_MSLDAPWrapper` is a class whose members are all `@staticmethod` free functions (no instance state) — a namespace pretending to be a class | `class _MSLDAPWrapper:` |
| P1-25 | bloodyAD/cli_modules/msldap.py:83 | #24 | Dynamic identifier construction: attribute fetched by string-built name (`getattr(msldapcc, f'do_{method_name}')`, also `getattr(MSLDAPClientConsole, attr_name)` line 48) | `full_method_name = f'do_{method_name}'\nmethod = getattr(msldapcc, full_method_name)` |
| P1-26 | bloodyAD/cli_modules/msldap.py:145 | #24 | Function bodies built from an f-string source and `exec`'d — opaque dynamic construction that no static tool can follow | `exec(func_code, local_vars)` |
| P1-27 | bloodyAD/cli_modules/msldap.py:202 | #26 | The module's entire public command surface is assembled by code (`globals()[method_name] = wrapper_func` in a loop) rather than declared literally | `globals()[method_name] = wrapper_func` |
| P1-28 | bloodyAD/main.py:2 | #26 | Star import — and from `patch`, a module that is 100% commented-out (patch.py:1-29), so it imports nothing while hiding provenance | `from bloodyAD.patch import *` |
| P1-29 | bloodyAD/network/ldap.py:429 | none | `interTrustOp` (and its nested `partitionOp`) is defined but never called anywhere in the tree — dead code | `async def interTrustOp(self, partition_map, op_params, op_name="bloodysearch"):` |
| P1-30 | bloodyAD/patch.py:1 | none | Entire module is commented-out dead code (28 lines, no live statement); still imported by main.py | `# import os\n# # Waiting for asysocks 0.2.18` |
| P1-31 | bloodyAD/network/ldap.py:680 | none | Bare `except:` swallowing all errors and emitting a debug `print` left in library code | `except:\n    print("There was some error here")` |
| P1-32 | bloodyAD/network/ldap.py:215 | none | No-op `try/except Exception as e: raise e` — catches then re-raises unchanged, adding nothing | `except Exception as e:\n    raise e` |
| P1-33 | bloodyAD/network/ldap.py:740 | none | `searchInPartition` returns `{}` on the no-server path but a list everywhere else — inconsistent return type from one function (also searchInForest:630) | `return {}` |
| P1-34 | bloodyAD/cli_modules/add.py:241 | none | Missing comma bug: builds a malformed DN `cn=Computers<domainNC>` (no separator); same defect user() add.py:716 | `container = "cn=Computers" + ldap.domainNC` |
| P1-35 | bloodyAD/cli_modules/get.py:166 | none | `str + dict` in the error path — `record` is a dict, so this raises TypeError instead of logging | `LOG.error("KeyError for record: " + record)` |

## Phase 2 — audit finding verdicts

163 findings. Deps were unavailable, so oracle/WP rules (2, 5) infer from
unresolved types — noted where it drives a verdict. Grouped where a rule
fires near-identically; every finding line is accounted for. Counts:
**real 127, fp 36.**

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| config.py:114 | 2 | proved | real | `isinstance(self.certificate, str)` on a `str`-typed field is genuinely redundant (isinstance narrowing is sound). |
| config.py:43 | 2 | proved | fp | `krb_args is not None` guard is live — the field defaults to `None`; the "no overlap" claim only holds under the wrong `krb_args: list = None` annotation. |
| set.py:162 | 2 | heuristic | fp | same mis-annotation (`oldpass: str = None`); the None-check is needed, not redundant. |
| accesscontrol.py:86 (createACE:sid) | 5 | proved | fp | proposes `sid: int` but the sole prod caller passes `SID.to_bytes()` (bytes); a mis-lift from deps-unresolved type inference. |
| utils.py:14 (addRight:object_type), utils.py:56 (delRight:object_type) | 5 | indexed | real | every prod caller leaves `object_type=None` — a genuine (if degenerate) established invariant; the param is effectively dead. |
| dns.py:67 (fromDict:data) | 5 | indexed | real | unannotated param; both callers pass `str` — a real liftable boundary (the `int` arm of the union is spurious but the invariant holds). |
| ldaptypes.py:229 (removePriv:priv) | 5 | indexed | real | sole caller passes an int mask; correct lift. |
| md4.py:112/116/120/124 (F/G/H/lrot) | 5 | indexed | real | bitwise round helpers; all callers pass int — correct lifts. |
| ldap.py:448/303/250/419/220, utils.py:88/14/56/497/445, ldaptypes.py:223 | 27 | indexed | real | all 11: high-fan-in symbols (up to fan-in 39) in 557–772-line modules — accurate purchase-price measurement (report-tier). |
| drawing.py:79, drawing.py:91 | 11 | indexed | real | `child_head`/`last_child_head` are structural clones differing only in the box glyph. Low value (vendored asciitree; divergence is intentional). |
| add.py:313 (port,priority,weight ×4 sigs), add.py:313 (data,dnstype,port,preference,priority,ttl,weight ×3), ldap.py:553 (allow_gc,conn,dns ×3) | 14 | indexed | real | genuine recurring parameter groups (DNS record fields; forest-search context) that want a type. |
| md4.py:112 (x,y,z ×3) | 14 | indexed | fp | x,y,z are the three operands of pure bitwise round functions — a coincidental clump, not a domain concept wanting a type. |
| ldap.py:356 (get_is_gc) | 6 | indexed | real | an accessor-named method that performs a network LDAP search — a real hidden effect. |
| add.py:32/470/279/415/505, remove.py:205/11/137, set.py:112, get.py:62/321, utils.py:637 | 15 | indexed/heuristic | real | 12 sites: take the rich `conn`/handler but use only `.getLdap`/`.conf`/`.copy` — genuine demand-narrowing candidates. |
| exceptions.py:26 (SymbolFormatter.format:record) | 15 | heuristic | fp | signature is fixed by the `logging.Formatter.format` override contract — `record` cannot be narrowed. |
| utils.py:637 (connectReachable:srv_names) | 10 | indexed | real | only iterated; concrete `list` over-constrains where `Iterable` suffices. |
| dns.py:115/132/183/262, formatters.py:47/78/89, drawing.py:48, traversal.py:18, cryptography.py:143 | 13 | indexed | real | all 10 are single-forwarding-call bodies (the rule's shape). Several are justified formatter-registry adapters / strategy defaults — report-tier flags the shape, correctly. |
| dns.py:115, dns.py:132 (toDict→formatCanonical) | 25 | indexed | real | delegation to a differently-stemmed method name — the call chain is genuinely un-greppable. |
| md4.py:62 (__str__→hexdigest) | 25 | indexed | fp | dunder delegation has no expected name stem; idiomatic, not a naming inconsistency. |
| tests/lab/README.md:56/118/136 (tests/secrets.json) | 28 | indexed | fp | the lab README instructs the user to create/update `tests/secrets.json` (a gitignored secret) — intentionally absent, not a broken doc path. |
| 26 module `top-loading` + 14 `cost-docstring` (adschema.py:1 … addRight:14) | 29 | heuristic | real | docstrings are genuinely absent on heavy modules/entry points (the rule's own presence/cost ideal). But this is ~25% of the whole audit — low-value, near-presence; flag for calibration. |
| main.py:40 (amain), formatters.py:161 (getFormatters) | 17 | heuristic | real | live-variable necks in genuinely long/assembly functions — plausible split points. |
| accesscontrol.py:119 (createEmptySD) | 17 | heuristic | fp | a 17-line constructor; the "neck" is not a meaningful split. |
| utils.py:345 (domResolve), dns.py:171 (DNS_COUNT_NAME.fromCanonical) | 16 | heuristic | real | compute-then-write tails present (weak; store-at-end is idiomatic for `from*` builders). |
| config.py:175 (copy) | 30 | heuristic | real | 7-hop Demeter chain `self._ldap._con.auth.…get_all_tgt_kirbis`. |
| md4.py:87 | 20 | ast | real | `lambda x: x % 4` repeated 3× in one module — should be named. |
| utils.py:284 (LazyAdSchema) | 21 | heuristic | real | `self._resolveAll()` retry recurs across 3 getX methods — a real distributed pattern. |
| tests/test_formatters.py:5, tests/test_msldap_module.py:6 | 21 | heuristic | fp | repeated `assert`/`fail` across test methods is idiomatic test structure, not an invariant to encapsulate. |
| asciitree (render/node_label/child_head/child_tail/last_child_head/last_child_tail/get_children), exceptions (SymbolFormatter.format), md4 (hexdigest/digest/hexbytes), utils (addguid/addsid/adddn), dns (A.toDict/AAAA.toDict), test_functional ×4, ldap (searchInForest/getTrusts/is_gc) | 22 | heuristic | fp | all 23: every flagged method is a polymorphic override (Strategy/Formatter), a hash-API method, a state-mutating method, or a TestCase method — none is relocatable to a free function. Velcro proxy misfires on inheritance. |
| ldap.py:518 | 7 | heuristic | fp | the comment justifies an implementation choice (why no async lock), not a caller-must-call-first precondition wanting a receipt type. |
| main.py:107/207, msldap.py:145/84/48/202, ldap.py:434, exceptions.py:14, asciitree/util.py:4, asciitree/traversal.py:46, test_msldap_module.py:121/122/32/35/48/51/62/65/83 | 24 | ast/heuristic | real | all 19: genuine dynamic identifier construction (import_module/vars/exec/globals/getattr/hasattr/setattr on computed names) — each blinds search/WP, including the msldap reflection tests. |
| main.py:183 (amain:submodnames) | 19 | heuristic | real | `arg in submodnames` (list) inside a per-arg loop — O(n·m). |
| set.py:16, add.py:22, get.py:388, utils.py:637 | 9 | ast | real | mutable list literals as default args. |
| config.py:168 (copy:**kwargs), ldap.py:220 (bloodyadd:**kwargs) | 1 | ast | real | opaque `**kwargs` on public boundaries. |
| ldaptypes.py:498 (ACE_TYPE_MAP), utils.py:210 (REVERSE_ACCESS_RIGHTS), main.py:2 (star-import bloodyAD.patch) | 26 | ast | real | declarations assembled by code / hidden by star import — a reader must execute to know the members. |

## Phase 3 — reconciliation

Every phase-1 site classified. **covered 9, detector-miss 17,
threshold-miss 2, inventory-gap 7.**

| P1 id | rule | class | note |
|-------|------|-------|------|
| P1-1 | #1 | detector-miss | #1 fired only on `**kwargs`; bare `list`/`list=None` boundary at config.py:24 not caught. |
| P1-2 | #1 | covered | site utils.py:637 srv_names flagged — under #10 (over-constrained) rather than #1, but the site is covered. |
| P1-3 | #1 | detector-miss | bare `list` return on phantomRoot/showRecoverable not caught by #1. |
| P1-4 | #1 | detector-miss | `zone: str = None` (annotation lies) not caught by #1; only its downstream redundant-guard surfaced (#2, as an FP). |
| P1-5 | #9 | covered | mutable-default `ports` flagged at utils.py:637. |
| P1-6 | #9 | covered | mutable-default `c` flagged at get.py:388. |
| P1-7 | #9 | covered | mutable-default `v` flagged at set.py:16. |
| P1-8 | #9 | covered | mutable-default `t` flagged at add.py:22. |
| P1-9 | #9 | detector-miss | #9 implements only mutable-default; the cross-module mutation of imported `MSLDAP_BUILTIN_ATTRIBUTE_TYPES_ENC` (set.py:31) is in-scope but missed. |
| P1-10 | #9 | detector-miss | `global_lazy_adschema` mutated from a second module not flagged by #9 (only surfaced via #27 purchase-price). |
| P1-11 | #9 | detector-miss | runtime monkeypatch of `LDAPModifyException.__str__` (set.py:331) not caught. |
| P1-12 | #11 | detector-miss | SID-resolution block clones ×7 far exceed any count cutoff; the async-for/dict-subscript shape defeated clone normalization. |
| P1-13 | #11 | detector-miss | "fetch first result" clone ×16 not matched — same normalization gap. |
| P1-14 | #11 | threshold-miss | `schemes` dict clone ×3 is a 1-line literal below the clone min-size; #11 fired elsewhere (drawing.py). |
| P1-15 | #11 | detector-miss | duplicated `attributesSD` + resolve_sd loop (object vs search) not matched. |
| P1-16 | #11 | detector-miss | duplicated SDFlags-control construction block not matched. |
| P1-17 | #11 | detector-miss | near-verbatim `uac` clone (add vs remove) not matched. |
| P1-18 | #11 | threshold-miss | in-addr reversal block is x2 (first-copy exempt) and small — under the ratchet cutoff. |
| P1-19 | #6 | detector-miss | #6 implements only dishonest get_*-accessors; `groupBy` mutating its list arg (utils.py:118) is the same honesty ideal but out of the detector's reach. |
| P1-20 | #12 | detector-miss | no #12 idiom findings at all; `[b for b in a]`→`list()` not in the catalog. |
| P1-21 | #18 | detector-miss | #18 fired nowhere; the labeled phases in `password` not caught (function even carries a docstring so #29 also skipped it). |
| P1-22 | #18 | detector-miss | labeled phases in `badSuccessor` not caught by #18 (only #29 cost-docstring + #15 fired on its inner getWeakOU). |
| P1-23 | #18 | detector-miss | labeled phases in `__post_init__` not caught by #18. |
| P1-24 | #22 | detector-miss | the true positive — `_MSLDAPWrapper`'s all-`@staticmethod` class — was skipped while #22 fired 23× on polymorphic overrides (all FP). Staticmethods appear excluded from velcro. |
| P1-25 | #24 | covered | `getattr(msldapcc, f'do_{name}')` flagged at msldap.py:84. |
| P1-26 | #24 | covered | `exec(func_code)` flagged at msldap.py:145. |
| P1-27 | #26 | covered | site msldap.py:202 flagged — under #24 (dynamic-id globals) rather than #26, but the assembled-declaration site is covered. |
| P1-28 | #26 | covered | star-import flagged at main.py:2. |
| P1-29 | none | inventory-gap | dead code (`interTrustOp` never called) — no rule; only its inner `getattr` surfaced via #24. |
| P1-30 | none | inventory-gap | fully commented-out `patch.py` module — no dead-module rule (its star-import was caught at main.py:2). |
| P1-31 | none | inventory-gap | bare `except:` + leftover debug `print` — no rule. |
| P1-32 | none | inventory-gap | no-op `try/except: raise e` — no rule. |
| P1-33 | none | inventory-gap | inconsistent return type (`{}` vs list) — no rule fired. |
| P1-34 | none | inventory-gap | missing-comma DN bug — logic bug, no rule. |
| P1-35 | none | inventory-gap | `str + dict` TypeError path — no rule. |
