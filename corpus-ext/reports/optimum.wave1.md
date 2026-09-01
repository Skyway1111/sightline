# optimum — wave 1

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | optimum/gptq/utils.py:33 | #9 | Mutable default argument: `layers=[Conv1D, nn.Conv2d, nn.Linear]` is a shared list default on a public helper. | `def get_layers(module: nn.Module, layers=[Conv1D, nn.Conv2d, nn.Linear], prefix=None, name=""):` |
| P1-2 | optimum/gptq/quantizer.py:87 | #1 | Weak boundary type in public `__init__`: `Dict[str, any]` — lowercase builtin `any` used as the value type (both weak and wrong). | `meta: Optional[Dict[str, any]] = None,` |
| P1-3 | optimum/gptq/quantizer.py:89 | #1 | Public constructor accepts `*args, **kwargs` that are never stored or used — opaque contract, silently dropped. | `*args,`<br>`**kwargs,` |
| P1-4 | optimum/gptq/quantizer.py:711 | #1 | Bare unparameterized `Optional[Dict]`; `no_split_module_classes` is documented as "a list of layer class names" yet typed Dict. | `max_memory: Optional[Dict] = None,`<br>`no_split_module_classes: Optional[Dict] = None,` |
| P1-5 | optimum/runs_base.py:53 | #1 | Public `__init__` takes bare `dict` with no key/value types — every caller re-derives the shape. | `def __init__(self, run_config: dict):` |
| P1-6 | optimum/configuration_utils.py:299 | #1 | Public serializer returns `Dict[str, Any]`; the config's published shape is erased at the boundary. | `def to_dict(self) -> Dict[str, Any]:` |
| P1-7 | optimum/pipelines/__init__.py:65 | #1 | Public `pipeline` entrypoint uses `pipeline_class: Optional[Any]` and `**kwargs: Any`. | `pipeline_class: Optional[Any] = None,`<br>`**kwargs: Any,` |
| P1-8 | optimum/modeling_base.py:321 | #1 | Public `from_pretrained` ends in opaque `**kwargs` forwarded blindly to subclass loaders. | `token: Optional[Union[bool, str]] = None,`<br>`**kwargs,` |
| P1-9 | optimum/gptq/data.py:207 | #1 | `tokenizer: Any` recurs across every dataset loader (get_wikitext2/get_c4/get_dataset) — untyped boundary. | `dataset_name: str, tokenizer: Any, nsamples: int = 128, ...` |
| P1-10 | optimum/commands/optimum_cli.py:33 | #9 | Module-level mutable list `_OPTIMUM_CLI_SUBCOMMANDS = []` mutated from the `optimum_cli_subcommand` decorator (line 62) — action-at-a-distance registry. | `_OPTIMUM_CLI_SUBCOMMANDS = []` |
| P1-11 | optimum/gptq/data.py:170 | #11 | `get_c4_new` is a byte-for-byte copy of `get_c4` (line 142); only the error-message name differs. | `def get_c4_new(tokenizer, seqlen, nsamples, split="train"):` |
| P1-12 | optimum/utils/save_utils.py:31 | #11 | Four near-identical `try: preprocessors.append(AutoX.from_pretrained(...)) except Exception: pass` blocks (31,38,45,54); wants a loop over the Auto classes. | `preprocessors.append(AutoTokenizer.from_pretrained(...))`<br>`except Exception:`<br>`pass` |
| P1-13 | optimum/utils/import_utils.py:141 | #11 | `is_transformers_version` / `is_diffusers_version` / `is_torch_version` (141,150,159) are structural clones differing only in the guarded module flag. | `if not _transformers_available: return False`<br>`return compare_versions(version.parse(_transformers_version), operation, reference_version)` |
| P1-14 | optimum/utils/input_generators.py:236 | #11 | The `if framework == "pt": import torch ... else: import numpy ...` dispatch block is copy-pasted across random_float_tensor/constant_tensor/concat_inputs and more. | `if framework == "pt":`<br>`    import torch`<br>`    ...` |
| P1-15 | optimum/gptq/quantizer.py:399 | #18 | `quantize_model` narrates five numbered phases (`# Step 1`..`# Step 5` at 399,426,491,590,599) inside one ~270-line body — function boundaries spelled in prose. | `# Step 1: Prepare the data` |
| P1-16 | optimum/exporters/tasks.py:483 | #24 | Model class resolved via `getattr(importlib.import_module(library), class_name)` — runtime-constructed name defeats grep and whole-program guarantees. | `loaded_library = importlib.import_module(library)`<br>`return getattr(loaded_library, class_name)` |
| P1-17 | optimum/fx/parallelization/api.py:104 | #24 | `getattr(importlib.import_module("transformers"), model_arch[0])` builds the target class name from data at runtime. | `model_cls = getattr(importlib.import_module("transformers"), model_arch[0])` |
| P1-18 | optimum/commands/optimum_cli.py:143 | #24 | Subcommand modules imported by an assembled name `f"{ns}.{register_file.stem}"` (plugin discovery, but ungreppable). | `register_module = importlib.import_module(f"{commands_register_namespace}.{register_file.stem}")` |
| P1-19 | optimum/fx/optimization/transformations.py:273 | #3 | `hasattr(linear, "bias")` is always true for `nn.Linear` (bias attr always present, None when absent); the guard and its warning on line 274 are dead — should test `linear.bias is not None`. | `use_bias = any(hasattr(linear, "bias") for linear in linears)`<br>`if use_bias and not all(hasattr(linear, "bias") for linear in linears):` |
| P1-20 | optimum/utils/import_utils.py:63 | #6 | `_is_package_available` (an `is_*` predicate) mutates its caller's list via `pkg_distributions.append(pkg_name)` — a checker with a hidden write. | `pkg_distributions.append(pkg_name)` |
| P1-21 | optimum/commands/optimum_cli.py:87 | #19 | `to_visit.pop(0)` is O(n) run every iteration of the `while to_visit` BFS — quadratic; wants a `deque`. | `current_command_instance = to_visit.pop(0)` |
| P1-22 | optimum/utils/import_utils.py:254 | none | Error message f-string interpolates the imported module `version` object instead of the `package_version` argument — user sees a module repr, not the version. | `f"...but expected numpy<{version}. {message}"` |
| P1-23 | optimum/gptq/utils.py:82 | none | Dead no-op statement `pattern_candidate = pattern_candidate` inside the loop. | `for pattern_candidate in BLOCK_PATTERNS:`<br>`    pattern_candidate = pattern_candidate` |
| P1-24 | optimum/gptq/data.py:198 | none | `get_ptb` and `get_ptb_new` (198,202) are dead stubs that only `raise "deprecated"`; unreferenced (not in get_dataset_map, and get_dataset rejects ptb at line 238). | `def get_ptb(...):`<br>`    raise RuntimeError("Loading the `ptb` dataset was deprecated")` |
| P1-25 | optimum/fx/parallelization/utils.py:224 | none | `methods_to_patch: Dict[str, Callable]` is annotated as a dict but assigned a **list of tuples** — the type declaration is simply wrong. | `methods_to_patch: Dict[str, Callable] = [`<br>`    ("torch.nn.Linear.__init__", meta_init(nn.Linear.__init__)),` |
| P1-26 | optimum/fx/parallelization/utils.py:247 | none | Five-field record kept as a positional list `[module, attr, orig_fn, patch_fn, False]` accessed by magic index (`spec[-1]`, `spec[1]`, `spec[3]`); fields named only in the comment above — wants a dataclass/NamedTuple. | `# Module, Attribute, Patchee, Patcher, Status`<br>`self.patching_specs.append([module, attribute_name, orig_fn, patch_fn, False])` |
| P1-27 | optimum/utils/import_utils.py:220 | none | `is_gptqmodel_available()` returns `None` on the unavailable path (no explicit return) while every sibling `is_*_available` returns a bool — dishonest predicate. | `def is_gptqmodel_available():`<br>`    if _gptqmodel_available:` |
| P1-28 | optimum/modeling_base.py:199 | none | `push_to_hub` swallows `KeyError`/`NameError` with bare `pass`, silently dropping upload failures. | `except KeyError:`<br>`    pass`<br>`except NameError:` |
| P1-29 | optimum/exporters/utils.py:561 | #23 | Author-admitted high-complexity dispatch: a long `if/elif` chain on `task`/`model.config.model_type` selecting the submodel builder. | `# TODO: this succession of if/else strongly suggests a refactor is needed.` |
| P1-30 | optimum/modeling_base.py:204 | none | Implicit-Optional: parameters annotated `str` but defaulted to `None` (`git_user: str = None`, `git_email: str = None`). | `def git_config_username_and_email(self, git_user: str = None, git_email: str = None):` |

## Phase 2 — audit finding verdicts

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| optimum/gptq/quantizer.py:400 | 2 | proved | fp | `dataset` is documented to also hold tokenized dicts; annotation `Union[List[str],str]` is too narrow, so `isinstance(dataset[0], str)` really distinguishes str- vs dict-lists. |
| optimum/gptq/quantizer.py:417 | 2 | proved | fp | Final `isinstance(...,list)` dispatch guards the `else: raise` for runtime types outside the narrow annotation. |
| optimum/gptq/quantizer.py:765 | 2 | proved | fp | `backend` is also accepted as a `BACKEND` enum (line 767); annotation `str` too narrow, isinstance is load-bearing. |
| optimum/utils/testing_utils.py:177 | 2 | proved | fp | `filter_params_func` is a user callback that can return None despite its inferred non-optional return type. |
| optimum/subpackages.py:43 | 2 | proved | fp | `dist.metadata["Name"]` is typed `str` by typeshed but returns None for a missing header at runtime. |
| optimum/commands/base.py:97 | 2 | proved | fp | Runtime validation of subclass-declared `SUBCOMMANDS`; a contributor can put a non-CommandInfo despite the annotation. |
| optimum/fx/optimization/transformations.py:411 | 2 | proved | fp | `nn.Linear.bias` is Optional at runtime (None when bias=False); torch stub types it non-optional — guard is necessary. |
| optimum/fx/parallelization/parallel_layers/linear.py:54 | 2 | proved | fp | Same torch-stub bias issue: `linear.bias is not None` is load-bearing. |
| optimum/fx/parallelization/parallel_layers/linear.py:131 | 2 | proved | fp | Same torch-stub bias issue. |
| optimum/commands/optimum_cli.py:56 | 2 | proved | fp | Public decorator validates `parent_command`; a caller can pass a non-subclass despite the `Optional[Type[...]]` annotation. |
| optimum/exporters/tasks.py:848 | 2 | proved | fp | `_infer_task_from_model_or_model_class` can return None; the None-guard+raise is the intended contract. |
| optimum/fx/optimization/transformations.py:667 | 2 | proved | fp | torch-stub: `linear.bias` optional at runtime. |
| optimum/fx/optimization/transformations.py:668 | 2 | proved | fp | torch-stub: `bn1d.weight` optional at runtime. |
| optimum/fx/optimization/transformations.py:669 | 2 | proved | fp | torch-stub: `bn1d.bias` optional at runtime. |
| optimum/fx/optimization/transformations.py:546 | 2 | proved | fp | torch-stub: `bn2d.weight` optional at runtime. |
| optimum/fx/optimization/transformations.py:547 | 2 | proved | fp | torch-stub: `bn2d.bias` optional at runtime. |
| optimum/fx/parallelization/parallel_layers/linear.py:175 | 2 | proved | fp | torch-stub: `self.bias` optional at runtime (`if self.bias is not None`). |
| tests/fx/optimization/test_transformations.py:283 | 2 | proved | fp | torch-stub param optionality in a test assertion. |
| tests/fx/optimization/test_transformations.py:291 | 2 | proved | fp | Same torch-stub param optionality. |
| optimum/fx/optimization/transformations.py:246 | 2 | proved | fp | `_get_bias` explicitly returns zeros when `linear.bias is None`; the guard is necessary (torch stub wrong). |
| optimum/gptq/quantizer.py:171 | 2 | heuristic | real | `self.format = format.lower()` is always a str, so `isinstance(self.format, str)` and its `else` branch are genuinely dead. |
| optimum/modeling_base.py:335 | 2 | heuristic | real | `revision: str = "main"` is never None; `if revision is not None` is always true (also a latent bug — meant to detect "explicitly set"). |
| optimum/modeling_base.py:209 | 2 | heuristic | fp | Implicit-Optional `git_user: str = None`; the guard is load-bearing since the default IS None. |
| optimum/modeling_base.py:217 | 2 | heuristic | fp | Implicit-Optional `git_email: str = None`; guard load-bearing. |
| #1 group (69) — repr: gptq/quantizer.py:334, modeling_base.py:321, exporters/tasks.py:1094, utils/preprocessing/base.py:138 | 1 | heuristic | real | Genuine weak boundary types on public signatures: opaque `**kwargs`, `tokenizer: Any`, bare dicts — all match the rule; some mirror transformers by convention but still weaken the contract. |
| #5 group (4) — repr: fx/optimization/transformations.py:68, utils/input_generators.py:64/78, utils/normalized_config.py:74 | 5 | indexed | real | Params established identically at all prod call sites — valid lift/narrow candidates. |
| #6 group (9) — repr: exporters/tasks.py:1083/533, utils/file_utils.py:48, configuration_utils.py:130, fx/parallelization/utils.py:341 | 6 | indexed | real | Accessor-named funcs (`get_*`/`is_*`/`find_*`) with real IO or arg-mutation effects; get_logger/get_verbosity are mild (lazy logger config) but do carry hidden effects. |
| tests/fx/parallelization/test_tensor_parallel.py:74 | 8 | indexed | real | `model_id: str` recurs across 4 signatures — genuine NewType candidate. |
| optimum/gptq/data.py:121 | 8 | indexed | fp | `not is_datasets_available()` is a repeated import guard, not a value-validation predicate; the "encode as a type" remedy doesn't apply. |
| #9 group (3): gptq/utils.py:33, fx/parallelization/decomp.py:200, fx/parallelization/passes.py:564 | 9 | heuristic | real | All genuine mutable default arguments. |
| #10 group (17) — repr: utils/preprocessing/*.py (12), configuration_utils.py:97, exporters/utils.py:527, utils/testing_utils.py:40 | 10 | indexed | real | Concrete `Dict`/`List` demanded where the body only uses Mapping/Iterable ops — genuine protocol-widening sites. |
| #11 group (24) — repr: gptq/data.py:142/170, utils/import_utils.py:141/150/159, utils/logging.py:197/206, utils/preprocessing image/text:92/93 | 11 | indexed | real | Genuine AST-normalized clone groups (incl. the byte-identical get_c4/get_c4_new). |
| #13 group (14) — repr: commands/base.py:137, fx/.../dist_ops.py:133/137, modeling_base.py:164, passes.py:572 | 13 | indexed | real | Bodies that are a single forwarding call (some are idiomatic autograd `.apply` / interface `reverse` wrappers, but mechanically forward-only). |
| optimum/gptq/data.py:86 | 13 | indexed | fp | `pad_block` is not a single forward — it builds a tuple, moves device, cats, and `.long()`s; detector mis-fired. |
| #14 group (33) — repr: utils/input_generators.py:115/377, exporters/tasks.py:533, fx/parallelization/api.py:35, gptq/data.py:120 | 14 | indexed | real | Recurring parameter clumps (dtype/framework family; hub-options; normalized_config/task) that genuinely want a type; overlapping subsets inflate the count but each is a true clump. |
| #15 group (12) — repr: fx/.../utils.py:70/82/121, exporters/utils.py:192/523, testing_utils.py:154, input_generators.py:1051/1397 | 15 | mixed | real | Rich objects (fx Node, config, Mapping) passed where ≤k attrs are used — genuine demand-narrowing sites. |
| optimum/fx/parallelization/utils.py:435 | 15 | indexed | fp | `model_name_or_path` is already a `str` using only `.replace` — a str is not a rich wallet object. |
| optimum/fx/parallelization/utils.py:462 | 15 | indexed | fp | Same: `str` param using only `.replace`, not a wallet parameter. |
| tests/utils/prepare_for_doc_test.py:46 | 15 | heuristic | fp | `code: str` using only `.split` — str is already minimal. |
| optimum/fx/parallelization/distributed/dist_ops.py:117 | 15 | heuristic | fp | `ctx` is torch autograd's mandated `backward` argument; cannot be narrowed to a protocol. |
| optimum/fx/parallelization/distributed/dist_ops.py:129 | 15 | heuristic | fp | Same framework-mandated autograd `ctx`. |
| #17 group (3): gptq/quantizer.py:163, gptq/quantizer.py:802, commands/env.py:30 | 17 | heuristic | fp | Single-crossing "necks" at import/attr-store lines; too weak to be an actionable split point (env.run:30 is an import line). |
| optimum/gptq/quantizer.py:399 | 18 | heuristic | real | `quantize_model` narrates Step 1–5 across one ~270-line body. |
| #20 group (2): fx/parallelization/decomp.py:112/118 | 20 | heuristic | real | Trivial eta-wrapper lambdas (`lambda x: from_fun(x)`) repeated 7×/5×; should be named or passed directly. |
| optimum/fx/parallelization/decomp.py:78 | 21 | heuristic | real | `track_tensor_tree(...)` recurs across 4 methods of one prod class — genuine encapsulation candidate. |
| #21 test group (10) — repr: tests/pipelines/test_pipelines.py:34 (8), test_transformations.py:96, test_quantization.py:261 | 21 | heuristic | fp | Repeated `self.assert*` / dummy-input calls across independent test methods are idiomatic test structure, not a class invariant to encapsulate. |
| #22 group (24) — repr: fx/parallelization/passes.py:97/121/124/133/138, op_handlers.py:34/45/58, core.py:42 | 22 | heuristic | real | Methods that touch only the public interface (Meyers velcro metric); low-signal but the mechanical claim holds. |
| #24 group (48) — repr: exporters/tasks.py:483/1176/1177, fx/parallelization/utils.py, normalized_config.py, commands/optimum_cli.py:143, fx/parallelization/api.py:104 | 24 | heuristic | real | getattr/setattr/import_module with runtime-constructed names — all genuinely ungreppable and blind whole-program analysis. |
| #25 group (20) — repr: import_utils.py:141/150/159, input_generators.py:549/1411, modeling_base.py:108, utils.py:287/293 | 25 | indexed | fp | Every edge is a legitimate interface/dunder/helper delegation (`__call__`→forward, `is_*_version`→compare_versions, `generate`→random_int_tensor); none is a misleading cross-layer rename. |
| optimum/utils/import_utils.py:277 | 26 | heuristic | fp | `BACKENDS_MAPPING` is an `OrderedDict([...])` literal table; its lambda/format values are inherent to lazy import-checking, not gratuitous code-assembly. |
| #27 group (10) — repr: exporters/tasks.py:118/1236, fx/parallelization/utils.py:40/56/63/86, passes.py:572 | 27 | indexed | real | Accurate purchase-price measurements: high-fan-in symbols in 494–1321-line modules; severity varies. |
| optimum/commands/optimum_cli.py:105 | 28 | indexed | real | Docstring names `optimum.commands.register`, a PEP-420 namespace absent from this repo (genuinely unresolvable here). |
| docs/README.md:136 | 28 | indexed | real | `utils.ModelOutput` does not resolve in the tree. |
| optimum/commands/register/README.md:7 | 28 | indexed | real | `optimum.commands.BaseOptimumCLICommand` is not re-exported (lives in `.base`); doesn't resolve. |
| docs/README.md:331 | 28 | indexed | fp | `build_pr_documentation.yml` exists in `.github/workflows/`; resolver missed that dir. |
| docs/README.md:332 | 28 | indexed | fp | `build_main_documentation.yml` exists in `.github/workflows/`. |
| docs/README.md:391 | 28 | indexed | fp | `build_pr_documentation.yml` exists (duplicate ref). |
| docs/README.md:392 | 28 | indexed | fp | `build_main_documentation.yml` exists (duplicate ref). |
| #29 group (47) — repr: fx/optimization/transformations.py:1, gptq/quantizer.py:1, fx/parallelization/passes.py:1, loss.py, op_handlers.py | 29 | heuristic | real | Verified: the heavy fx/gptq modules genuinely have no module docstring; accurate top-loading-absence reports (soft signal). |
| #30 group (10) — repr: gptq/quantizer.py:773, runs_base.py:254, fx/.../utils.py:113, input_generators.py:703/1459 | 30 | heuristic | real | Genuine ≥3-hop attribute/param reach chains. |
| optimum/exporters/utils.py:97 | 30 | heuristic | fp | `pipeline.__class__.__name__` is the idiomatic class-name lookup, not a Demeter reach. |

## Phase 3 — reconciliation

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #9 | covered | #9 fires at gptq/utils.py:33 (get_layers mutable default). |
| P1-2 | #1 | detector-miss | `Dict[str, any]` (lowercase builtin `any`) not caught; #1 keys on `typing.Any`. |
| P1-3 | #1 | detector-miss | Opaque `*args, **kwargs` on `GPTQQuantizer.__init__:89` not flagged (other #1 kwargs were). |
| P1-4 | #1 | detector-miss | Bare `Optional[Dict]` at load_quantized_model:711 not flagged. |
| P1-5 | #1 | detector-miss | Bare `dict` param `run_config` (runs_base:53) not flagged though file was analyzed. |
| P1-6 | #1 | covered | #1 fires at configuration_utils.py:299 (to_dict return). |
| P1-7 | #1 | covered | #1 fires at pipelines/__init__.py:64-65. |
| P1-8 | #1 | covered | #1 fires at modeling_base.py:321 (from_pretrained **kwargs). |
| P1-9 | #1 | covered | #1 fires at gptq/data.py:207 (tokenizer: Any). |
| P1-10 | #9 | detector-miss | Module-level mutable registry mutated cross-module by a decorator is in #9's remit but only mutable-defaults fired. |
| P1-11 | #11 | covered | #11 fires at gptq/data.py:170 (get_c4/get_c4_new). |
| P1-12 | #11 | detector-miss | Four repeated try/except blocks live inside one function; clone detector works at cross-function granularity. |
| P1-13 | #11 | covered | #11 fires at import_utils.py:141/150/159 (is_*_version). |
| P1-14 | #11 | threshold-miss | The repeated `if framework=="pt"` branch is a small intra-method fragment below the clone-size threshold. |
| P1-15 | #18 | covered | #18 fires at gptq/quantizer.py:399 (Step 1–5). |
| P1-16 | #24 | covered | #24 fires at exporters/tasks.py:483. |
| P1-17 | #24 | covered | #24 fires at fx/parallelization/api.py:104. |
| P1-18 | #24 | covered | #24 fires at commands/optimum_cli.py:143. |
| P1-19 | #3 | detector-miss | `hasattr(linear,"bias")` always-true guard: rule #3 (contract-implied-guard) emitted zero findings across the repo. |
| P1-20 | #6 | detector-miss | `_is_package_available` is `is_`-named with an arg-mutation effect but was not among the 9 #6 findings (leading-underscore / effect missed). |
| P1-21 | #19 | detector-miss | `pop(0)` in a while-loop: rule #19 (linear-op-in-loop) emitted zero findings. |
| P1-22 | none | inventory-gap | Error f-string interpolating the wrong object — no rule covers it. |
| P1-23 | none | inventory-gap | No-op self-assignment — no rule covers it. |
| P1-24 | none | inventory-gap | Dead deprecated stubs — no dead-function rule in the inventory. |
| P1-25 | none | inventory-gap | Misdeclared container type (annotated dict, assigned list) — no rule. |
| P1-26 | none | inventory-gap | Positional list used as an ad-hoc record — no rule (nearest #8/#14 don't match). |
| P1-27 | none | inventory-gap | `is_*` predicate that returns None on one path — no rule. |
| P1-28 | none | inventory-gap | Silent `except: pass` swallow — no rule covers bare-except swallowing. |
| P1-29 | #23 | detector-miss | Author-admitted long if/elif dispatch: rule #23 (cognitive-complexity) emitted zero findings. |
| P1-30 | none | covered | Same defect surfaced as #2 at modeling_base.py:209/217 — though #2 diagnoses the guard as redundant, inverting the real fix (the annotation should be `Optional[str]`). |
