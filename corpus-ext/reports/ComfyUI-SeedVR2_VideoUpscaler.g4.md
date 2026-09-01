# ComfyUI-SeedVR2_VideoUpscaler — wave 1

| rule | fp class | count | example key |
|---|---|---|---|
| 1 | generic call forwarder: `*args`/`**kwargs` are handed straight to a callee (gradient_checkpointing, init_causal_conv3d, MMModule.forward, convert_to_ddp), so the accepted set is the callee's | 16 | src/models/dit_3b/nadit.py:31:1:weak:nadit.gradient_checkpointing:*args |
| 1 | `ctx: Any` on a torch.autograd.Function — an ad-hoc attribute bag no ctx type admits | 6 | src/common/distributed/ops.py:95:1:weak:src.common.distributed.ops.SeqAllToAll.forward:ctx |
| 1 | `**kwargs` absorbs sibling blocks' extra keys so one loop can call heterogeneous diffusers blocks; removing it breaks the dispatch | 4 | src/models/video_vae_v3/modules/attn_video_vae.py:115:1:weak:attn_video_vae.Upsample3D.forward:**kwargs |
| 1 | `List[Any]`/`Dict[str, Any]` over a wrapped module's own arg list — the elements really are anything the callee takes | 4 | src/models/dit_3b/mm.py:27:1:weak:mm.get_args:args |
| 1 | Any is the only truthful return: dynamic import / config-named construction | 2 | src/common/config.py:91:1:weak:src.common.config.import_item:return |
| 2 | scalar-or-per-layer broadcast param annotated as one form only; the isinstance is the scalar arm | 11 | src/models/dit_7b/nadit.py:102:2:redundant:isinstance |
| 2 | torch/diffusers stubs cannot express a None attribute or a None-holding ModuleList entry, so the None guard is the live path | 10 | src/models/dit_3b/normalization.py:60:2:redundant:comparison |
| 2 | the annotation is a lie #1 already flags on the same def (None default under a non-Optional type) | 6 | src/models/dit_3b/attention.py:118:2:redundant:comparison |
| 2 | device-normalization idiom: the branch accepts a device string, so the annotation should widen, not the check disappear | 5 | src/optimization/memory_manager.py:112:2:redundant:isinstance |
| 2 | too-narrow vararg element type on a cleanup helper callers feed possibly-absent tensors | 1 | src/optimization/memory_manager.py:510:2:redundant:comparison |
| 10 | widening `set` to `Collection[object]` drops the O(1) hash contract for a membership test inside a per-parameter loop | 1 | src/core/model_loader.py:751:10:over-constrained:src.core.model_loader._report_parameter_mismatches:loaded_names |
| 11 | run of argparse `add_argument` declarations with distinct flags, helps and defaults — no shared fact to home | 2 | inference_cli.py:1372:11:clone-block:09b7fb1bca22 |
| 11 | run of `name = d.get("key", default)` unpack statements over different dicts and keys | 2 | src/interfaces/video_upscaler.py:343:11:clone-block:a40e0fe8584a |
| 14 | clump anchored by the ambient `debug` logger, which drags two arbitrary neighbours in | 15 | src/core/model_loader.py:547:14:clump:debug,model,target_device |
| 18 | the labeled phases are already separate function calls — the boundary the rule asks for exists | 3 | inference_cli.py:966:18:sections:inference_cli._process_frames_core |
| 18 | the numbered lines are one enumerated rationale or a two-part boolean condition, not phase labels | 3 | src/models/dit_3b/rope.py:33:18:sections:rope.RotaryEmbeddingBase.__init__ |
| 20 | the repeat is the canonical two-token sort key `x[1]`; naming it adds indirection with nothing that can drift | 1 | src/utils/debug.py:351:20:lambda:src.utils.debug:7279506a |
| 23 | at/near the bar on a short flat body — a ternary run, a nested lookup loop, or two defensive except arms | 4 | src/utils/constants.py:89:23:cognitive-complexity:src.utils.constants.get_all_model_files |
| 24 | the name is a literal from a list/dict two lines up (temp_attrs, cached_config_attrs, an encode/decode pair) — grep finds it | 11 | src/optimization/memory_manager.py:1001:24:dynamic-id:hasattr:1001 |
| 24 | the name is runtime model data (named_buffers/named_children/a dotted state-dict path); no source identifier exists to find | 8 | src/core/model_loader.py:674:24:dynamic-id:setattr:674 |
| 29 | small single-concept module whose filename already says what it is; the docstring would restate it | 1 | src/models/dit_3b/normalization.py:1:29:top-loading:normalization |
| 32 | availability probe: the import sits in a try/except ImportError whose only purpose is the test (three already carry `# noqa: F401`) | 7 | src/optimization/compatibility.py:20:32:dead-import:src.optimization.compatibility:early_config_prune |
| 32 | dataclass field whose name does occur — as a keyword at every construction site; deleting it breaks them | 2 | src/utils/model_registry.py:28:32:dead-symbol:src.utils.model_registry.ModelInfo.precision |
| 32 | the symbol is `_`, the conventional throwaway target of a tuple unpack | 1 | inference_cli.py:82:32:dead-symbol:inference_cli._ |
| 37 | drop-in shim whose parameter list mirrors F.interpolate so call sites swap 1:1 (and `mode` still drives the fallback branch) | 4 | src/common/half_precision_fixes.py:59:37:monomorphic:src.common.half_precision_fixes.safe_interpolate_operation:mode |
| 39 | the comment-history arm keys on the words "no longer", catching present-tense lifecycle and FSDP rationale | 8 | src/core/generation_phases.py:795:39:comment-history:src.core.generation_phases:795 |
| 41 | cold glue: a once-per-install download of two multi-GB files, where connection reuse saves nothing measurable | 1 | src/utils/downloads.py:143:41:perf:http-in-loop:src.utils.downloads.download_with_resume:143 |
| 50 | return-only slot whose one truthful annotation is Any (a `module(*args, **kwargs)` passthrough, an untyped cache read) | 4 | src/common/cache.py:45:50:unannotated:src.common.cache.Cache.get |
| 55 | wrapper mirroring a third-party API's published positional order, with call sites relaying identically-named locals | 4 | src/optimization/compatibility.py:287:55:positional-width:src.optimization.compatibility.call_flash_attn_2_varlen |
| 58 | torch.autograd.Function.backward vararg narrowing — what torch's own docs prescribe, with no accepted fix | 2 | src/common/distributed/ops.py:144:58:invalid-method-override:src.common.distributed.ops:144:8 |
| 59 | an nn.Module forward or tensor-math helper inside the model itself, where torch.cat/torch.chunk is the file's subject, not a hidden cost | 19 | src/models/video_vae_v3/modules/attn_video_vae.py:110:59:cost-docstring:attn_video_vae.Upsample3D.forward |
| 60 | the call graph missed a live caller: a call through a package `__init__` re-export, and an aliased import shadowed by a same-named fallback def | 2 | src/models/dit_3b/nablocks/__init__.py:23:60:dead-by-graph:src.models.dit_3b.nablocks.get_nablock |
