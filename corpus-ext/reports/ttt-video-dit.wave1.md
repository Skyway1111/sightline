# Phase 1 — blind ideal sites: ttt-video-dit

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | ttt/infra/config_manager.py:358 | #9 | Mutable/eval-at-def default: `sys.argv[1:]` is evaluated once at function-definition time and shared, a classic default-arg trap. | `def parse_args(self, args_list: list = sys.argv[1:]):` |
| P1-2 | ttt/infra/config_manager.py:301 | #9 | `_config_types` is a class-level mutable dict (line 270); `_setup_eval_args` mutates it via `self`, so eval keys leak into every JobConfig instance. | `self._config_types["eval"] = EvalConfig` |
| P1-3 | ttt/infra/config_manager.py:443 | #1 | Public accessor returns bare `dict` (opaque boundary type) rather than a typed config mapping. | `def to_dict(self) -> dict:` |
| P1-4 | ttt/infra/optimizers.py:401 | #1 | Public `get_optimizer_and_scheduler` types the config as `Any` (it is a `JobConfig`) and returns `Tuple[..., Any]`. | `def get_optimizer_and_scheduler(model: nn.Module, config: Any) -> Tuple[..., Any]:` |
| P1-5 | ttt/infra/optimizers.py:58 | #1 | Return type `Tuple[List, List, List, List]` uses bare unparametrized `List` (== `List[Any]`) on a public classmethod. | `def categorize_parameters(cls, model) -> Tuple[List, List, List, List]:` |
| P1-6 | ttt/infra/optimizers.py:134 | #14 | The four parameter buckets returned by `categorize_parameters` are then passed as four separate positional params here — a data clump wanting one type. | `ttt_no_wd, ttt_with_wd, other_no_wd, other_with_wd,` |
| P1-7 | ttt/infra/optimizers.py:31 | #22 | `ParameterGroupManager` holds only class constants + class/static methods with no instance state — a namespace class of free functions. | `class ParameterGroupManager:` |
| P1-8 | ttt/infra/optimizers.py:267 | #22 | `LRScheduleFunctions` is a class of only `@staticmethod`s — free functions hiding in a class. | `class LRScheduleFunctions:` |
| P1-9 | ttt/models/cogvideo/dit.py:293 | #11 | The `nn.LayerNorm(...)` + `for param in X.parameters(): param.requires_grad = requires_grad` block is copy-pasted at lines 120,124,294,304,399,447. | `self.pre_seq_layernorm = nn.LayerNorm(...)` / `for param in ...: param.requires_grad = requires_grad` |
| P1-10 | ttt/models/cogvideo/dit.py:329 | #11 | The 6-way adaLN `chunk(6)` unpack + `.unsqueeze(1)` gating (329-349) is duplicated verbatim for the MLP branch (351-380); `pre_seq/pre_mlp_adaLN_modulation` (296,306) are identical. | `(shift_msa, scale_msa, gate_msa, ...) = self.pre_seq_adaLN_modulation(...).chunk(6, dim=1)` |
| P1-11 | ttt/models/cogvideo/dit.py:136 | #8 | `adapter_method` is a stringly-typed enum: the `in ("sft","qkvo","none")` membership check recurs (parallelisms.py:96, optimizers.py:169) and `== "sft"` is recomputed in every module init. | `assert config.adapter_method in ("sft", "qkvo", "none")` |
| P1-12 | ttt/models/cogvideo/utils.py:161 | #2 | `assert NotImplementedError` asserts a class object (always truthy) so the guard never fires — should be `raise`. | `assert NotImplementedError` |
| P1-13 | ttt/models/ssm/utils.py:74 | #2 | `0 <= 1` is a provably-constant true comparison; the assert reduces to `assert 1 < ndim`. | `assert 0 <= 1 < ndim` |
| P1-14 | ttt/models/ssm/utils.py:9 | #11 | `precompute_freqs_cis_3d` duplicates the freq-grid computation of `Rotary3DPositionEmbedding._precompute_freqs_cis` (cogvideo/utils.py:389); only polar-vs-sincos differs. | `dim_t = dim // 4` / `freqs_t = 1.0 / (theta ** (...))` |
| P1-15 | ttt/models/ssm/ttt_layer.py:360 | #11 | `TTTLinear.ttt` and `TTTMLP.ttt` (429) share identical scaffolding (tile states, `checkpoint_group_size`, use_kernel branch, permute/reshape); `init_device_mesh`/`init_weights` likewise. | `W1_states = torch.tile(self.W1.unsqueeze(0), dims=(B, 1, 1, 1))` |
| P1-16 | ttt/models/ssm/ops/ttt_linear.py:57 | #11 | `ttt_linear` and `ttt_mlp` (ops/ttt_mlp.py:70) are byte-for-byte identical apart from the extra W2/b2 keys — dict build, tree_map permute, empty_like, scan, permute return. | `inputs = tree_map(lambda x: x.permute(2, 0, 1, 3, 4), inputs)` |
| P1-17 | ttt/models/ssm/ops/utils.py:4 | #11 | `ln_fwd` and `ln_fused_l2_bwd` (21) repeat the same mean/var/std/x_hat/y layernorm block; a third copy is inlined in ttt_layer.py `ln_reconstruction_target`. | `mu = x.mean(dim=-1, keepdim=True)` / `x_hat = (x - mu) / std` |
| P1-18 | ttt/models/vae/regularizers.py:44 | #9 | Mutable default argument: `dims=[1, 2, 3]` is a shared list default. | `def nll(self, sample, dims=[1, 2, 3]):` |
| P1-19 | ttt/models/vae/autoencoder.py:77 | #9 | Mutable default `ignore_keys=list()` on the constructor and again on `init_from_ckpt` (86). | `ignore_keys=list(),` |
| P1-20 | ttt/models/vae/autoencoder.py:156 | none | Dead commented-out `forward` implementation (11 lines) left above the real one. | `# def forward(` |
| P1-21 | ttt/models/vae/autoencoder.py:202 | #2 | `n_samples = z.shape[0]` then `n_rounds = ceil(z.shape[0]/n_samples)` is provably 1; the `for n in range(n_rounds)` loop is a redundant single-iteration wrapper (same at 178-182). | `n_rounds = math.ceil(z.shape[0] / n_samples)` |
| P1-22 | ttt/models/cogvideo/weight_conversion/from_hf.py:37 | #26 | A ~40-branch `if/elif "...in key"` string-key remap table is assembled by control flow where a literal `{hf_key: repo_key}` dict would be data. | `if "patch_embed.proj.bias" in key:` |
| P1-23 | ttt/models/vae/utils.py:256 | #24 | Dynamic identifier construction: `get_obj_from_str` / `instantiate_from_config` (246) import + `getattr` classes named by config strings — opaque to static analysis. | `return getattr(importlib.import_module(module, package=None), cls)` |
| P1-24 | ttt/models/vae/utils.py:219 | #11 | `expand_dims_like` is defined identically here, in cogvideo/utils.py:52, and data/precomp_text.py:14; `exists`/`default`/`append_dims`/`append_zero` are likewise duplicated across vae/utils, cogvideo/utils, vae/attention, cp_enc_dec. | `def expand_dims_like(x, y):` |
| P1-25 | ttt/models/vae/attention.py:53 | none | Import from a module that does not exist in this repo (`checkpoint` actually lives in ttt.models.vae.utils); the whole file appears to be unwired vendored code. | `from modules.utils import checkpoint` |
| P1-26 | ttt/models/ssm/linear_triton.py:284 | #11 | `forward_sharded` (284) and `forward_unsharded` (311) have identical bodies differing only by the `local_map` decorator; same for backward pair and the entire TkMLP mirror (mlp_tk.py:316,347). | `return TritonLinear._forward_core(ctx, ttt_norm_weight, ...)` |
| P1-27 | ttt/models/cogvideo/sampler.py:30 | #22 | `PromptManager`, `ModelLoader` (76), `TextEncoder` (162) are all-`@staticmethod` namespace classes with no state; `VideoSaver` in sample.py:102 is the same. | `class PromptManager:` |
| P1-28 | ttt/datasets/preembedding_dataset.py:35 | none | `__getitem__` retries 10× and on exhaustion falls through returning `None` silently; the `except (TimeoutError, RuntimeError, Exception)` tuple is redundant (Exception subsumes the first two). | `for i in range(10):` / `except (TimeoutError, RuntimeError, Exception) as e:` |
| P1-29 | ttt/datasets/data_sampler.py:9 | #1 | Public constructor forwards opaque `*args, **kwargs` straight into `DistributedSampler`, hiding the real accepted parameters. | `def __init__(self, dataset, effective_rank, effective_world_size, *args, generator=None, **kwargs):` |
| P1-30 | ttt/models/vae/utils.py:14 | #9 | Module-global mutable state `_CONTEXT_PARALLEL_GROUP`/`_CONTEXT_PARALLEL_SIZE` mutated via `global` in `initialize_context_parallel` (25). | `_CONTEXT_PARALLEL_GROUP = None` |
| P1-31 | ttt/infra/checkpoint.py:41 | #13 | `_save`/`_load` are pure one-line forwarders to `dcp.save`/`dcp.load` — shallow middle-man wrappers. | `def _save(self, path, state_dict): dcp.save(state_dict=state_dict, checkpoint_id=path)` |
| P1-32 | ttt/infra/parallelisms.py:27 | #15 | `init_distributed(job_config)` demands the whole `JobConfig` but reads only `comm.init_timeout_seconds`; `get_world_info`/`apply_fsdp`/`DiscreteSampler` show the same wallet-parameter pattern. | `def init_distributed(job_config):` |
| P1-33 | ttt/infra/utils.py:31 | #6 | `get_time` is a `get_*` accessor that mutates hidden state on first call (writes a function attribute cache) — dishonest accessor. | `if not hasattr(get_time, "cached_time"): get_time.cached_time = datetime.datetime.now()` |
| P1-34 | train.py:125 | #7 | Comment-borne ordering contract ("must get data iterator after resuming") narrates a caller obligation that is enforced only by comment. | `# Must get data iterator after resuming from checkpoint` |
| P1-35 | data/precomp_video.py:105 | #18 | `precompute_episode` is segmented by labeled step comments (`# Skip if already processed`, `# Process video`, `# Add to batch`, `# Process batch`, `# Save encoded frames`) — phase markers wanting extraction. | `# Skip if already processed` |
| P1-36 | ttt/infra/train_iterator.py:130 | none | Stale comment: text says "average iteration time + 2 minutes" but the code subtracts `timedelta(minutes=6)`. | `effective_threshold = timedelta(minutes=self.timeout_minutes) - (avg_iter_td + timedelta(minutes=6))` |
| P1-37 | ttt/infra/parallelisms.py:77 | none | Dead assignment: `mesh`/`names` set in the `== 1` branch are immediately overwritten by `names, mesh = [], []` at line 81. | `mesh = [dp_sharding]` / `names, mesh = [], []` |
| P1-38 | ttt/infra/parallelisms.py:18 | none | Module constants `TRACE_BUFFER_SIZE`/`TRACE_FILE`/`DUMP_ON_TIMEOUT`/`ASYNC_ERROR_HANDLING`/`SKIP_CLEANUP` are defined but never referenced. | `TRACE_BUFFER_SIZE = "TORCH_NCCL_TRACE_BUFFER_SIZE"` |
| P1-39 | ttt/models/cogvideo/utils.py:143 | none | `make_beta_schedule` assigns `betas` only inside `if schedule == "linear"` then `return betas.numpy()`; any other value raises `UnboundLocalError` — the `schedule` param is effectively single-valued. | `if schedule == "linear": betas = ...` / `return betas.numpy()` |
| P1-40 | ttt/models/vae/utils.py:100 | none | `get_string_from_tuple` runs `eval(s)` on an input string and uses `type(t) == tuple` instead of `isinstance`. | `t = eval(s)` |

## Phase 2 — audit finding verdicts

301 findings. Rows are grouped where a rule fires many near-identical times
(representative site + count); mixed-verdict rules are split into a real row
and an fp row so every finding is accounted for. Group counts sum to 301.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| weak-boundary group x24 (repr: optimizers.py:401 config:Any; config_manager.py:443 to_dict->dict; autoencoder.py:106 **kwargs; regularizers.py:65 ->Any) | 1 | heuristic | real | Each site genuinely carries `Any`/bare `dict`/opaque `**kwargs` on a public boundary; nn.Module.forward `**kwargs` is idiomatic torch but still hides the contract. |
| ttt/models/cogvideo/utils.py:579 | 2 | heuristic | real | `self.guider` is constructed as `DynamicCFG` in `__init__`, so `isinstance(self.guider, DynamicCFG)` is always true and the else-branch dead. |
| ttt/infra/train_iterator.py:59 | 4 | indexed | fp | The `step is None` check guards the value reassigned from `get_latest_checkpoint_step()` (which returns Optional), not the caller's arg — the check is live; detector conflated param-step with reassigned-step. |
| proof-lift type group x4 (utils.py:25 context_parallel_size:int; checkpoint.py:47 path:str; preembedding_dataset.py:82 batch_size:int; utils.py:256 reload:bool) | 5 | indexed | real | Each proposes the param's true always-held type; sound, useful annotations. |
| proof-lift value group x2 (regularizers.py:25 kl:other=None; train_submitit.py:86 main:input_args=None) | 5 | indexed | fp | Over-narrow lifts to `None` on deliberately-optional params — `kl` supports a second distribution (live else-branch), `main(input_args)` is a programmatic entry point; incidental single-caller usage, exactly the PA§8 FP mode. |
| dishonest-accessor group x7 (repr: train.py:25 get_batch mutates-arg; sampler.py:34 get_prompts io; logging.py:149 get_latest_checkpoint_step io) | 6 | indexed | real | Each `get_*` has an undeclared effect (arg mutation, file/stdout io); genuine honesty violation. |
| ttt/models/configs.py:90 | 6 | indexed | fp | `get_preset` is a factory: it constructs a `ModelConfig` and calls `.update()` on that freshly-built return value — no incoming argument is mutated, so "mutates-arg" mislabels normal construction. |
| ttt/models/vae/utils.py:327 | 7 | heuristic | fp | `get_nested_attribute`'s docstring describes dispatch behavior (int→index), not a caller-must-ensure precondition — descriptive doc, not an unencoded protocol. |
| mutable-default group x3 (autoencoder.py:77, autoencoder.py:86 ignore_keys=list(); regularizers.py:44 dims=[1,2,3]) | 9 | heuristic | real | Genuine shared mutable default arguments. |
| ttt/models/cogvideo/utils.py:464 | 10 | indexed | real | `DiscreteDenoiser.forward` only indexes `cond["crossattn"]`, so `Mapping` suffices where `Dict` is demanded; widening verified. |
| structural-clone group x55 (repr: cp_enc_dec.py<->cp_enc_dec_test.py whole-module dup; expand_dims_like x3; linear_triton/mlp_tk forward_sharded==forward_unsharded) | 11 | indexed | real | All are genuine duplicated function bodies; the bulk is the near-total cp_enc_dec/cp_enc_dec_test copy plus real helper/wrapper dups. Tiny-helper clones (default, expand_dims_like) are low-value but real. |
| shallow-wrapper group x9 (attention.py:101 FeedForward.forward->self.net; cp_enc_dec.py:340/344/348/352 + cp_enc_dec_test.py:291/295/299/303 conv_* -> Function.apply) | 13 | indexed | real | Each body is a single forwarding call; the autograd `.apply` wrappers are idiomatic but structurally are middle-men. |
| ttt/models/vae/attention.py:60 | 13 | indexed | fp | `uniq` is a dedup idiom (`{el:True...}.keys()`), not a forward to another function — a #12-style reimplementation, not a middle-man wrapper. |
| data-clump group x16 (repr: cp_enc_dec.py:131 dim,input_,kernel_size x26; linear_triton.py:16 W1_init,b1_init,ttt_norm_*; dit.py:163 seq_metadata,text_emb,vid_emb; ttt_linear.py:57 XK,XQ,XV) | 14 | indexed | real | Each group genuinely travels together across k signatures and wants a type; QKV/seq-triples are inherent ML groups but the detection is accurate. |
| wallet config/model group x9 (repr: optimizers.py:200/401 model->named_parameters; parallelisms.py:92 model/job_config; parallelisms.py:57 job_config; dit.py:91 SSMGating config) | 15 | mixed | real | Rich config/model objects where only 1-2 members are used — the canonical wallet; a narrower protocol or field extraction genuinely helps. |
| wallet ctx/tensor group x28 (repr: linear_triton.py:284/356 ctx; mlp_tk.py:316 ctx; :311 W1_init:.to; :356 grad_L_XQW_batch:.contiguous; utils.py:231 mean_flat:tensor; ssm/utils.py:56 x) | 15 | mixed | fp | `ctx` is a framework-mandated autograd context (not narrowable); tensor params show "only .to/.contiguous used" only because the real use is inside an opaque triton/TK kernel — narrowing a Tensor to a 2-method protocol is not a genuine over-ask. |
| ttt/models/cogvideo/weight_conversion/from_hf.py:148 | 16 | heuristic | real | `main` builds `state_dict` across a long loop then writes it out (save/makedirs/dcp/remove) — a genuine compute-then-write tail. |
| ttt/infra/logging.py:180 | 16 | heuristic | fp | `init_logger` is imperative logger setup (setLevel/addHandler are effects, not pure compute); there is no pure computation to split from the `os.environ` write. |
| liveness-neck group x3 (precomp_video.py:180; ssm/utils.py:50 precompute_freqs_cis_3d; ttt_layer.py:65 TTTBase.__init__) | 17 | heuristic | real | Each has a genuine single-variable liveness neck; valid (if low-value) structural split points, report-tier only. |
| velcro namespace-static group x3 (optimizers.py:40 is_ttt_parameter, :45 should_skip_weight_decay, :58 categorize_parameters) | 22 | heuristic | real | `ParameterGroupManager`'s static/class methods take their data as args and use only class constants — genuine free functions namespaced in a class. |
| velcro instance-method group x30 (repr: regularizers.py:25 kl; logging.py:90 write; train_iterator.py:42 add_metric; sampler.py:234 sample; config_manager.py:443 to_dict) | 22 | heuristic | fp | These are cohesive methods operating on their own instance state; velcro fires only because the attributes aren't name-mangled (public), which is not weak encapsulation — a reviewer would not extract `kl`/`write`/`sample` to free functions. |
| dynamic-id group x7 (config_manager.py:285/400 setattr; vae/utils.py:105 eval, :261/263 import_module+getattr, :349 getattr) | 24 | heuristic | real | Each constructs an identifier from a string (setattr/getattr/eval/import_module) — genuinely unfindable by search and blinds analysis. |
| purchase-price group x16 (repr: cogvideo/utils.py:30 to_local fan-in 14 in 711-line module; vae/utils.py:43; config_manager.py:358) | 27 | indexed | real | Small high-fan-in symbols live in 400-711 line grab-bag modules, so every importer pays the whole file — a genuine navigability cost; report-tier signal. |
| docs/training.md:27, :35 | 28 | indexed | fp | `checkpoint.init_state_dir` is a valid **config option** (CheckpointConfig.init_state_dir, config_manager.py:138, used as `job_config.checkpoint.init_state_dir`); the resolver wrongly treated the config-section.field path as a Python module symbol. |
| top-loading group x62 (repr: cp_enc_dec.py:1 975 lines no docstring; dit.py:1; train.py:49 main 151 lines no cost docstring) | 29 | heuristic | real | Each accurately reports an absent module/heavy-entry-point docstring. Weakest-grounded rule (doc-presence, AE§1.5) and low-value on small modules/cold scripts, but the instances are correct. |
| health-ratio/demeter group x13 (repr: parallelisms.py:148 model.dit.final_layer.linear... 4 hops; sample.py:177 self.job_config.eval.input_file 3 hops) | 30 | heuristic | real | Each is a genuine multi-hop attribute chain; config/model navigation chains are idiomatic but technically Demeter reaches, report-tier only. |

## Phase 3 — reconciliation

Every phase-1 site classified. covered = a finding matches the site; else an
FN class. 10 covered, 21 detector-miss, 1 threshold-miss, 8 inventory-gap.

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #9 | detector-miss | #9 mutable-default detector fires on literal `[]`/`{}`; the `sys.argv[1:]` slice default is a list built at def-time but not a literal, so it was missed. |
| P1-2 | #9 | detector-miss | Class-level `_config_types` dict mutated via `self` — shared mutable state in #9's spirit, but the detector only implements mutable-default-args. |
| P1-3 | #1 | covered | Finding config_manager.py:443 "return bare dict". |
| P1-4 | #1 | covered | Finding optimizers.py:401 config:Any + return Any. |
| P1-5 | #1 | detector-miss | `Tuple[List,List,List,List]` bare unparametrized `List` not treated as Any/bare-dict/**kwargs by the #1 detector. |
| P1-6 | #14 | threshold-miss | The 4-list bucket recurs in only 2 signatures (categorize returns / _create takes); below the detector's k=3 clump threshold. |
| P1-7 | #22 | covered | Findings optimizers.py:40/45/58 flag ParameterGroupManager's methods as velcro. |
| P1-8 | #22 | detector-miss | `LRScheduleFunctions` staticmethods use zero class members, so velcro (uses-only-public-interface) doesn't score them — the detector misses the pure namespace class. |
| P1-9 | #11 | detector-miss | Repeated `for p in X.parameters(): p.requires_grad=…` is a sub-function statement block; #11 clone detection is function-level. |
| P1-10 | #11 | detector-miss | The adaLN chunk(6) duplication is intra-/cross-block within one function, below function-level clone granularity. |
| P1-11 | #8 | detector-miss | Rule #8 fired 0 times repo-wide; the recurring `adapter_method in {...}` predicate was not detected. |
| P1-12 | #2 | detector-miss | `assert NotImplementedError` (always-true) isn't an isinstance/None/compare that the pyright-based #2 detector recognizes (and untyped code stays ungrounded). |
| P1-13 | #2 | detector-miss | Constant `0 <= 1` comparison in the assert not caught by the oracle-based #2 in untyped code. |
| P1-14 | #11 | detector-miss | `precompute_freqs_cis_3d` vs `Rotary._precompute_freqs_cis` differ (polar vs sin/cos), so not matched as a T2/T3 clone. |
| P1-15 | #11 | detector-miss | `TTTLinear.ttt`/`TTTMLP.ttt` differ by the W2/b2 path, below clone-similarity threshold (they got #29 cost-docstring findings, not #11). |
| P1-16 | #11 | detector-miss | `ttt_linear`/`ttt_mlp` op scaffolds not matched as clones despite near-identical structure. |
| P1-17 | #11 | detector-miss | `ln_fwd`/`ln_fused_l2_bwd` shared LN block is a sub-function fragment, not a whole-function clone. |
| P1-18 | #9 | covered | Finding regularizers.py:44 mutable default `dims`. |
| P1-19 | #9 | covered | Findings autoencoder.py:77 and :86 mutable default `ignore_keys`. |
| P1-20 | none | inventory-gap | Commented-out dead code is covered by no rule. |
| P1-21 | none | inventory-gap | Provably-constant `n_rounds=ceil(N/N)=1` redundant loop is arithmetic redundancy, outside #2's annotation-based scope and any other rule. |
| P1-22 | #26 | detector-miss | Rule #26 fired 0 times; the 40-branch if/elif key-remap that wants a literal dict was not detected. |
| P1-23 | #24 | covered | Findings vae/utils.py:261/263 dynamic import_module+getattr in get_obj_from_str. |
| P1-24 | #11 | covered | Finding "expand_dims_like x3" across precomp_text.py:14, cogvideo/utils.py:52, vae/utils.py:219. |
| P1-25 | none | inventory-gap | Broken import `from modules.utils import checkpoint` (unresolvable) is counted in provenance but is covered by no finding-rule. |
| P1-26 | #11 | covered | Finding forward_sharded==forward_unsharded at linear_triton.py:284/311 (and mlp_tk mirror). |
| P1-27 | #22 | detector-miss | PromptManager/ModelLoader/TextEncoder/VideoSaver are zero-self staticmethod namespaces; velcro needs self-using public methods, so it misses them (it instead over-fired on stateful methods). |
| P1-28 | none | inventory-gap | Silent `None`-on-retry-exhaustion and redundant `except (TimeoutError, RuntimeError, Exception)` tuple are covered by no rule. |
| P1-29 | #1 | detector-miss | RandomFaultTolerantSampler.__init__'s forwarded `**kwargs` (with `*args`) was not flagged though other **kwargs boundaries were. |
| P1-30 | #9 | detector-miss | Module-global `_CONTEXT_PARALLEL_GROUP` mutated via `global` is squarely #9 ("module-level mutable mutated") but the detector only implements mutable-defaults. |
| P1-31 | #13 | detector-miss | `_save`/`_load` one-line forwards to `dcp.save`/`dcp.load` not flagged (private underscore methods forwarding to an external module fn). |
| P1-32 | #15 | covered | The job_config-wallet pattern I flagged is detected at the sibling sites I named — parallelisms.py:92/57 (apply_parallelisms/get_world_mesh job_config); init_distributed itself (1 attr) wasn't flagged. |
| P1-33 | #6 | detector-miss | `get_time` writes a function attribute (`get_time.cached_time`); the effect-inference tracks mutates-arg/io, not function-attribute caching. |
| P1-34 | #7 | detector-miss | The inline `# Must get data iterator after resuming` ordering comment isn't a docstring, and #7 only scanned docstrings. |
| P1-35 | #18 | detector-miss | Rule #18 fired 0 times; the labeled step/section comments in precompute_episode were not detected. |
| P1-36 | none | inventory-gap | Stale comment (says "2 minutes", code uses `timedelta(minutes=6)`) is covered by no rule. |
| P1-37 | none | inventory-gap | Dead assignment (mesh/names set then overwritten) is covered by no rule. |
| P1-38 | none | inventory-gap | Unused module constants (TRACE_*/SKIP_CLEANUP) are dead-code, covered by no rule. |
| P1-39 | none | inventory-gap | `make_beta_schedule` UnboundLocalError-on-non-linear branch is a latent bug, covered by no rule. |
| P1-40 | none | covered | The eval at line 105 in the same function is caught by #24 dynamic-id (vae/utils.py:105) — I under-mapped it to `none`. |
