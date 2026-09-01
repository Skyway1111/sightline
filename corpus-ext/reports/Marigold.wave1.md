# Marigold — wave 1

Repo: `<GAUNTLET_CORPUS_ROOT>\Marigold`
Prod tree judged: `marigold/`, `src/`, `script/` (no test tree exists).

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | marigold/marigold_depth_pipeline.py:167 | #1 | Public `__call__` takes `ensemble_kwargs: Dict = None` — bare `Dict` (no key/value types), splatted into `ensemble_depth` at :299, so the caller must read the callee to learn what is accepted. Same defect at marigold_iid_pipeline.py:250 and marigold_normals_pipeline.py:151. | `ensemble_kwargs: Dict = None,` |
| P1-2 | marigold/marigold_depth_pipeline.py:199 | #28 | `__call__`'s docstring documents args `scale_invariant` and `shift_invariant` that are not in its signature (they are `__init__`/config properties). A reader following the docstring passes a TypeError. | `scale_invariant (str, *optional*, defaults to True):` |
| P1-3 | marigold/marigold_depth_pipeline.py:219 | none | `processing_res` is `Optional[int]` and is defaulted from `self.default_processing_resolution`, itself `Optional[int] = None`; when the model config omits it the next line raises `TypeError: '>=' not supported between NoneType and int` instead of a config error. Same at iid:290, normals:197. | `assert processing_res >= 0` |
| P1-4 | marigold/marigold_depth_pipeline.py:381 | #6 | `encode_empty_text` is named as a computation but returns nothing and writes `self.empty_text_embed`; `single_infer` (:438) silently triggers that write, so a `@torch.no_grad()` "single prediction" method mutates pipeline state. Triplicated (iid:451, normals:346). | `self.empty_text_embed = self.text_encoder(text_input_ids)[0].to(self.dtype)` |
| P1-5 | marigold/marigold_depth_pipeline.py:227 | #18 | `__call__` is one 126-line function narrated by five banner phase comments (`Image Preprocess`, `Predicting depth`, `Test-time ensembling`, `Resize back`, `Colorize`) — the phases are function boundaries spelled in prose. Same shape at iid:298 and normals:205. | `# ----------------- Image Preprocess -----------------` |
| P1-6 | marigold/marigold_depth_pipeline.py:340 | #11 | `_check_inference_step` is a ~40-line near-identical copy in all three pipelines (iid:413, normals:310); only the model name inside the warning strings differs. Any change to the recommended-scheduler advice must be made three times. | `def _check_inference_step(self, n_step: int) -> None:` |
| P1-7 | marigold/marigold_depth_pipeline.py:479 | #11 | `encode_rgb` is byte-identical in all three pipelines (iid:549, normals:444) — same VAE encode + `latent_scale_factor` scaling, and the magic constant `0.18215` is declared three times (depth:118, iid:201, normals:109). | `rgb_latent = mean * self.latent_scale_factor` |
| P1-8 | marigold/util/ensemble.py:190 | none | The docstring (:56) promises that absolute predictions (`scale_invariant=False, shift_invariant=False`) skip alignment and just ensemble, but the post-ensemble normalization has no branch for that case and raises. The documented mode is unreachable. | `raise ValueError("Unrecognized alignment.")` |
| P1-9 | marigold/util/ensemble.py:252 | #11 | `ensemble_iid` is a copy of the `ensemble` closure inside `ensemble_depth` (:120-136): same mean/median + std/MAD branches, same error string, only the parameter name differs. | `prediction = torch.median(targets, dim=0, keepdim=True).values` |
| P1-10 | marigold/util/ensemble.py:73 | #28 | Docstring says `max_iter` "defaults to 2" and `tol` "defaults to 1e-3"; the signature (:47-48) says `max_iter=50, tol=1e-6`. An agent tuning the solver from the docstring is off by 25x. | `max_iter (int, *optional*, defaults to 2):` |
| P1-11 | marigold/util/image_util.py:132 | none | The error message interpolates `resample_method`, which is provably `None` on that branch, instead of the offending `method_str` — every bad-method report reads `Unknown resampling method: None`. | `raise ValueError(f"Unknown resampling method: {resample_method}")` |
| P1-12 | marigold/util/image_util.py:46 | none | `colorize_depth_maps` binds `depth` in an `isinstance` if/elif with no `else`; any other input raises `UnboundLocalError` at :51 rather than a TypeError. Repeated for `img_colored` at :71-74. | `if isinstance(depth_map, torch.Tensor): ... elif isinstance(depth_map, np.ndarray):` |
| P1-13 | marigold/util/image_util.py:38 | #1 | `colorize_depth_maps(depth_map, min_depth, max_depth, cmap, valid_mask)` is a public util with zero annotations while it accepts either ndarray or Tensor and returns whichever — every caller must read the body to learn the contract. | `def colorize_depth_maps(depth_map, min_depth, max_depth, cmap="Spectral", valid_mask=None):` |
| P1-14 | marigold/util/image_util.py:144 | #11 | `srgb2linear`/`linear2srgb`/`float2int`/`chw2hwc` are duplicated verbatim in `src/util/image_util.py` as `img_srgb2linear`(:88)/`img_linear2srgb`(:84)/`img_float2int`(:69)/`img_chw2hwc`(:49). Two homes, both live: `script/iid/eval.py` imports one set, `src/trainer/marigold_iid_trainer.py` the other. | `def srgb2linear(img): return img**2.2` |
| P1-15 | src/util/image_util.py:49 | #25 | The same four conversions are exported under two naming conventions (`chw2hwc` vs `img_chw2hwc`), so neither grep finds all call sites of one concept. | `def img_chw2hwc(chw):` |
| P1-16 | marigold/util/batchsize.py:60 | #28 | `find_batch_size` docstring documents `ensemble_size` and `input_res` but omits `dtype`, which is the parameter that selects the table rows (:77) and therefore the one a caller most needs described. | `dtype (torch.dtype)  # absent from the Args block` |
| P1-17 | marigold/marigold_iid_pipeline.py:210 | #1 | `target_properties: Optional[Dict[str, Any]] = None` — a wholly opaque boundary type carrying `target_names`, `prediction_space`, `up_to_scale` etc.; every consumer re-derives the schema by `.get()` with defaults (see :122, :126). | `target_properties: Optional[Dict[str, Any]] = None,` |
| P1-18 | marigold/marigold_iid_pipeline.py:230 | none | Same parameter is annotated `Optional[...] = None` and then unconditionally subscripted; constructing the pipeline without `target_properties` raises `TypeError: 'NoneType' object is not subscriptable`. The Optional is a lie. | `self.target_names = target_properties["target_names"]` |
| P1-19 | marigold/marigold_iid_pipeline.py:123 | none | Three-branch dispatch on `prediction_space` where two branches are `pass` and there is no `else`; an unknown/mistyped space silently falls through to the sRGB path instead of erroring. | `if prediction_space == "stack": pass` |
| P1-20 | marigold/marigold_iid_pipeline.py:100 | #15 | `fill_entry` demands the entire `target_properties` dict but only ever reads `target_properties[name]` (:122, :126) — the caller (`fill_outputs`, :410) already knows `name`, so the whole wallet is passed to use one pocket. | `target_properties: Optional[Dict[str, Any]] = None,` |
| P1-21 | src/trainer/marigold_depth_trainer.py:614 | #11 | `save_checkpoint` is **byte-identical** (48 lines, verified by diff) across all three trainers: depth:614-661, normals:581-628, iid:649-696. | `def save_checkpoint(self, ckpt_name, save_train_state):` |
| P1-22 | src/trainer/marigold_depth_trainer.py:663 | #11 | `load_checkpoint` byte-identical across the three trainers (depth:663-699, normals:630-666, iid:698-734). Same for `_train_step_callback` (depth:423-452 / normals:411-440) and `_get_next_seed` (depth:603-613 / normals:570-580). | `def load_checkpoint(self, ckpt_path, load_trainer_state=True, resume_lr_scheduler=True):` |
| P1-23 | src/trainer/marigold_depth_trainer.py:74 | #11 | The three trainer `__init__` bodies are ~95% identical for 110+ lines (optimizer, LR schedule, DDPM/DDIM scheduler swap, metric tracker, periods, multi-res-noise block, internal counters) — depth:74-186 vs iid:78-201 vs normals. | `self.cfg: OmegaConf = cfg` |
| P1-24 | src/trainer/marigold_depth_trainer.py:61 | #14 | The 10-parameter group `(cfg, model, train_dataloader, device, out_dir_ckpt, out_dir_eval, out_dir_vis, accumulation_steps, val_dataloaders, vis_dataloaders)` travels together through three trainer constructors and the three `train.py` call sites — a `TrainerContext`/`OutputDirs` wanting a name. | `out_dir_ckpt, out_dir_eval, out_dir_vis,` |
| P1-25 | src/trainer/marigold_depth_trainer.py:145 | #24 | Metric functions are resolved by `getattr` on a module from config strings, so no grep from `abs_relative_difference` reaches this call site and no tool can prove `src/util/metric.py` exports are live. Four more sites: normals_trainer:148, script/depth/eval.py:133, script/normals/eval.py:112. | `self.metric_funcs = [getattr(metric, _met) for _met in cfg.eval.eval_metrics]` |
| P1-26 | src/trainer/marigold_depth_trainer.py:591 | #12 | `_metric.__str__()` calls the dunder directly where `str(_metric)` is the vocabulary; the value is appended to `sample_metric`, which in this function is never read afterwards — dead list plus a hand-rolled builtin. Duplicated at script/depth/eval.py:216. | `sample_metric.append(_metric.__str__())` |
| P1-27 | src/trainer/marigold_depth_trainer.py:653 | #12 | Touch-a-marker-file written as manual `open`/`close` with no context manager, where `pathlib.Path(...).touch()` is the stdlib idiom; a raise between the two lines leaks the handle. | `f = open(os.path.join(ckpt_dir, self._get_backup_ckpt_name()), "w")` |
| P1-28 | src/trainer/marigold_depth_trainer.py:482 | none | Best-metric test mixes `and`/`or` across four operands with no parentheses; correctness rests on precedence, and the "maximize" arm is easy to misread as guarded by the "minimize" arm. | `if ("minimize" == self.main_val_metric_goal and main_eval_metric < self.best_metric or ...)` |
| P1-29 | src/trainer/marigold_depth_trainer.py:415 | none | `stack_depth_images` returns `stacked` bound only in the 4-dim/3-dim branches; a 2-dim input raises `UnboundLocalError` instead of a shape error. | `if 4 == len(depth_in.shape): ... elif 3 == len(depth_in.shape):` |
| P1-30 | src/trainer/marigold_depth_trainer.py:403 | #13 | `encode_rgb` adds one assert and forwards to `self.model.encode_rgb`; `encode_depth` (:408) is two forwards. Both price a hop into the trainer that the pipeline already provides. | `latent = self.model.encode_rgb(image_in); return latent` |
| P1-31 | src/trainer/marigold_depth_trainer.py:172 | none | `mr_noise_strength`/`annealed_mr_noise`/`mr_noise_downscale_strategy` are defined only inside `if self.apply_multi_res_noise:`; the attribute set of the object depends on config, so any reader (or checkpoint-resume path) must know the flag to know the object's shape. | `if self.apply_multi_res_noise: self.mr_noise_strength = ...` |
| P1-32 | src/trainer/marigold_depth_trainer.py:634 | none | Misspelled identifier `scheduelr_path` in checkpoint saving — a name no grep for `scheduler_path` will find. | `scheduelr_path = os.path.join(ckpt_dir, "scheduler")` |
| P1-33 | src/util/logging_util.py:91 | #9 | `tb_logger` is a module-level mutable singleton mutated from at least five modules (`set_dir` in the three `train.py`s, `.writer`/`log_dict` in all three trainers and in `log_slurm_job_id`) — training state written by action at a distance. | `tb_logger = MyTrainingLogger()` |
| P1-34 | src/util/logging_util.py:72 | none | `MyTrainingLogger` declares `writer: SummaryWriter` but only binds it in `set_dir`; `log_dict` and `log_slurm_job_id` will `AttributeError` if the (unenforced, undocumented) must-call-`set_dir`-first protocol is violated. | `writer: SummaryWriter` |
| P1-35 | src/util/logging_util.py:104 | none | `global tb_logger` declared in a function that only reads it — a no-op statement that misleads a reader into thinking the module global is rebound here. | `global tb_logger` |
| P1-36 | src/util/logging_util.py:95 | #1 | `init_wandb(enable: bool, **kwargs)` forwards an opaque bag straight to `wandb.init`; the two call sites in each `train.py` build the dict from config, so nothing in the repo states what keys are legal. | `def init_wandb(enable: bool, **kwargs):` |
| P1-37 | src/util/seeding.py:53 | #2 | `initial_seed` is annotated `int`, so the `is None` guard is unreachable under the declared type — either the guard is dead or the annotation is wrong (callers pass `Union[int, None]`, e.g. trainer:606). | `if initial_seed is None: logging.warning("initial_seed is None, ...")` |
| P1-38 | src/util/seeding.py:55 | #9 | `generate_seed_sequence` reseeds the *process-global* `random` module to build a private sequence, silently perturbing every other consumer of `random` (dataset augmentation at base_normals_dataset.py:190 etc.). | `random.seed(initial_seed)` |
| P1-39 | src/util/metric.py:235 | #11 | `sub5_error`, `sub7_5_error`, `sub11_25_error`, `sub22_5_error`, `sub30_error` (:235-257) are five copies of one 3-line body differing only in a threshold literal. | `return round(100.0 * (np.sum(cosine_error < 5) / num_pixels), 4)` |
| P1-40 | src/util/metric.py:148 | #13 | `delta1_acc`/`delta2_acc`/`delta3_acc` are three one-line forwards to `threshold_percentage` that add only a literal; they exist because the `getattr` metric registry needs zero-arg-configurable names. | `def delta1_acc(pred, gt, valid_mask): return threshold_percentage(pred, gt, 1.25, valid_mask)` |
| P1-41 | src/util/metric.py:92 | #11 | `rmse_linear` (:92-104), `rmse_log` (:107-117) and `i_rmse` (:160-172) are the same masked-RMSE body three times, differing only in how `diff` is formed. | `diff2 = torch.pow(diff, 2); mse = torch.sum(diff2, (-1, -2)) / n; rmse = torch.sqrt(mse)` |
| P1-42 | src/util/metric.py:65 | none | `actual_output = output; actual_target = target` are pure renames used once each — noise repeated in three metrics (:65, :78, :93). | `actual_output = output` |
| P1-43 | src/util/metric.py:284 | #6 | `compute_iid_metric` is named as a pure computation but writes into the caller's tensors (`pred[invalid_mask] = 0`, `gt[invalid_mask] = 0`); `unsqueeze` at :273 returns a view, so the zeroing propagates back to the caller's ground truth. | `pred[invalid_mask] = 0` |
| P1-44 | src/util/metric.py:135 | #12 | Two full-size `zeros`/`ones` tensors are allocated per call just to feed `torch.where` a boolean cast that `(max_d1_d2 < threshold_val).float()` performs allocation-free. | `zero = torch.zeros(*output.shape); one = torch.ones(*output.shape)` |
| P1-45 | src/util/metric.py:54 | none | `MetricTracker.avg` has zero call sites in the repo — dead public surface on a class used by every trainer and eval script. | `def avg(self, key): return self._data.average[key]` |
| P1-46 | src/util/metric.py:38 | #1 | `MetricTracker(*keys, writer=None)` takes untyped varargs as its whole schema; every construction site passes a splatted list comprehension (`MetricTracker(*[m.__name__ for m in ...])`), so the key set is knowable only at runtime. | `def __init__(self, *keys, writer=None):` |
| P1-47 | src/util/loss.py:34 | #1 | `get_loss(loss_name, **kwargs)` fans an opaque kwargs bag into six different constructors with incompatible signatures (`SILogRMSELoss` needs `lamb` and `alpha`, `MeanAbsRelLoss` takes none) — misconfiguration surfaces as a TypeError deep inside the factory. | `def get_loss(loss_name, **kwargs):` |
| P1-48 | src/util/loss.py:133 | #11 | `SILogRMSELoss.__call__` (:133-151) is a copy of `SILogMSELoss.__call__` (:96-116) — identical masking, `first_term`/`second_term` algebra — differing only in the final sqrt/alpha line. | `second_term = self.lamb * torch.pow(torch.sum(diff, (-1, -2)), 2) / (n**2)` |
| P1-49 | src/util/loss.py:88 | none | IDE-template placeholder `(_type_)` left in two shipped docstrings (:88 and :124), and `alpha:` at :125 is documented with an empty description. | `lamb (_type_): lambda, lambda=1 -> scale invariant` |
| P1-50 | src/util/loss.py:136 | none | Three lines of commented-out reference implementation retained inside `__call__`, alongside a commented-out `super().__init__()` at :73 — history kept in the source rather than in git. | `# diff = log_depth_pred[valid_mask] - log_depth_gt[valid_mask]` |
| P1-51 | src/util/depth_transform.py:59 | #12 | Abstract base hand-rolled by assigning attributes and then `raise NotImplementedError` in `__init__`; `abc.ABC` + `@abstractmethod` is the stdlib mechanism, and the two assignments before the raise are dead. Subclass then re-does them at :91-92 without `super().__init__()`. | `self.norm_min = norm_min; self.norm_max = norm_max; raise NotImplementedError` |
| P1-52 | src/util/depth_transform.py:35 | none | `get_depth_normalizer` returns either a bare `identical` closure or a `ScaleShiftDepthNormalizer`; the two returns share no interface (`far_plane_at_max`, `denormalize` exist only on one), and base_depth_dataset.py:239 reads `far_plane_at_max` unguarded. | `def identical(x): return x` |
| P1-53 | src/util/data_loader.py:130 | none | Two consecutive `if new_batch_sampler is None:` blocks (:130 and :134) test the same condition — the first should be folded into the second. | `if new_batch_sampler is None: kwargs["drop_last"] = ...` |
| P1-54 | src/util/data_loader.py:124 | #24 | Loader attributes are read with `getattr` over a string table, so no grep from `prefetch_factor`/`persistent_workers` reaches this site and a renamed DataLoader attribute degrades silently to the table default. | `k: getattr(dataloader, k, _PYTORCH_DATALOADER_KWARGS[k])` |
| P1-55 | src/util/data_loader.py:68 | none | `SkipBatchSampler.total_length` has no call sites anywhere in the repo — dead property. | `def total_length(self): return len(self.batch_sampler)` |
| P1-56 | src/util/multi_res_noise.py:50 | #11 | Four `downscale_strategy` branches (:50, :64, :73, :85) repeat the same 8-line upsample-and-accumulate loop; only the computation of `r` differs. The shared body should take the ratio as a parameter. | `noise += (up_sampler(torch.randn(b, c, w, h, generator=generator, device=device).to(x)) * strength**i)` |
| P1-57 | src/util/multi_res_noise.py:48 | none | The base noise draw uses `x.device` while every in-loop draw uses the `device` parameter resolved at :44 — passing an explicit `device` different from `x.device` silently mixes devices. | `noise = torch.randn(x.shape, device=x.device, generator=generator)` |
| P1-58 | src/util/multi_res_noise.py:42 | none | `b, c, w, h = x.shape` mislabels a `[B, C, H, W]` tensor: `w` holds the height. `up_sampler(size=(w, h))` then reads as transposed, which only works because the labels are consistently wrong. | `b, c, w, h = x.shape` |
| P1-59 | src/util/lr_scheduler.py:62 | none | A matplotlib plotting demo (with a commented-out alternative config and a function-body `import matplotlib.pyplot`) ships inside the production scheduler module and writes `lr_scheduler.png` into the CWD. | `plt.savefig("lr_scheduler.png")` |
| P1-60 | src/util/slurm_util.py:40 | #13 | `get_local_scratch_dir` is a forward-only wrapper over one `os.getenv("TMPDIR")`; `is_on_slurm` (:34) additionally shadows its own function name with a local. | `local_scratch_dir = os.getenv("TMPDIR"); return local_scratch_dir` |
| P1-61 | src/util/config_util.py:74 | none | Debug entry point hardcoding a CWD-relative config path lives in a library module; running it from anywhere but the repo root fails. | `conf = recursive_load_config("config/train_base.yaml")` |
| P1-62 | src/trainer/__init__.py:43 | #13 | `get_trainer_cls` is a one-line dict lookup wrapper adding no validation, no error message, and no type — callers gain nothing over `trainer_cls_name_dict[name]`. | `def get_trainer_cls(trainer_name): return trainer_cls_name_dict[trainer_name]` |
| P1-63 | src/dataset/base_depth_dataset.py:76 | none | `rgb_transform` is accepted, stored at :97, and never read anywhere in the repo; `_load_rgb_data` (:156) hardcodes the identical formula instead. A configurable knob that silently does nothing. | `rgb_transform=lambda x: x / 255.0 * 2 - 1,` |
| P1-64 | src/dataset/base_depth_dataset.py:77 | #1 | `**kwargs` is accepted and dropped; `get_dataset` (src/dataset/__init__.py:101) splats the entire dataset config into this constructor, so any misspelled config key is silently swallowed by all three base datasets (also base_normals_dataset.py:53, base_iid_dataset.py:63). | `**kwargs,` |
| P1-65 | src/dataset/base_depth_dataset.py:73 | #1 | `augmentation_args: dict = None` — bare `dict` on the public constructor for a structure later accessed by attribute (`self.augm_args.lr_flip_p`, :259), i.e. the annotation is both weak and wrong. Same at base_normals_dataset.py:51, base_iid_dataset.py:61. | `augmentation_args: dict = None,` |
| P1-66 | src/dataset/base_depth_dataset.py:108 | #12 | `True if cond else False` where `bool(cond)`/the bare expression is the idiom; repeated verbatim in base_normals_dataset.py:75 and base_iid_dataset.py:85. | `self.is_tar = (True if os.path.isfile(dataset_dir) and tarfile.is_tarfile(dataset_dir) else False)` |
| P1-67 | src/dataset/base_depth_dataset.py:213 | #7 | The subclass contract is narrated in a comment rather than enforced: the base silently returns the undecoded image, so a dataset author who forgets the override gets wrong depths instead of `NotImplementedError`. | `#  Replace code below to decode depth according to dataset definition` |
| P1-68 | src/dataset/base_depth_dataset.py:87 | #11 | `__len__`, `__getitem__`, `_get_data_item`, `_read_image`, `_read_rgb_file`, `__del__` and the tar bootstrap are duplicated near-verbatim across base_depth_dataset.py, base_normals_dataset.py (:84-153, :262) and base_iid_dataset.py (:91-132, :198) — three copies of one dataset skeleton. | `def __getitem__(self, index): rasters, other = self._get_data_item(index)` |
| P1-69 | src/dataset/base_iid_dataset.py:193 | none | `_augment_data` has no `return`; its caller does `rasters = self._augment_data(rasters)` (:175), so enabling augmentation for IID training sets `rasters` to `None`. The depth/normals twins do return. | `def _augment_data(self, rasters): if random.random() < ...: rasters = {...}` |
| P1-70 | src/dataset/base_iid_dataset.py:33 | #9 | Importing this module mutates process-wide `os.environ` as a side effect, before any caller can decide; the `# ruff: noqa: E402` on the next line documents that the author knew imports were being bent around it. | `os.environ["OPENCV_IO_ENABLE_OPENEXR"] = "1"` |
| P1-71 | src/dataset/base_iid_dataset.py:159 | none | `_load_targets_data` returns an empty dict instead of raising, so an IID dataset subclass that forgets the override yields batches with no targets and training fails far from the cause. | `def _load_targets_data(self, rel_paths): outputs = {}; return outputs` |
| P1-72 | src/dataset/base_normals_dataset.py:202 | #8 | Dataset identity is inferred from a magic resolution literal, twice in one method (:202 and :212), each with the comment `# only blur if Hypersim sample` — a predicate re-validated at multiple sites that wants to be a dataset property. | `and rasters["rgb_int"].shape[-2] == 768` |
| P1-73 | src/dataset/base_normals_dataset.py:197 | none | `_augment_data` moves every raster to CUDA unconditionally whenever it runs in the main process; on a CPU-only or MPS run (both supported by `script/*/run.py`) augmentation crashes. | `rasters = {k: v.cuda() for k, v in rasters.items()}` |
| P1-74 | src/dataset/base_normals_dataset.py:81 | none | This base opens the tar eagerly in `__init__` while its depth/iid twins open lazily in `_read_image`; a `tarfile` handle created before DataLoader worker fork is shared across processes. Clone drift with a correctness consequence. | `if self.is_tar: self.tar_obj = tarfile.open(self.dataset_dir)` |
| P1-75 | src/dataset/kitti_dataset.py:130 | none | `reshape` is not in-place and the result is discarded — the statement is a no-op, and the following `logical_and` only works by broadcasting accident. | `eval_mask.reshape(valid_mask.shape)` |
| P1-76 | src/dataset/kitti_dataset.py:98 | none | `kitti_benchmark_crop` binds `out` only for 2-dim and 3-dim inputs; a 4-dim batch raises `UnboundLocalError`. | `if 2 == len(input_img.shape): ... elif 3 == len(input_img.shape):` |
| P1-77 | src/dataset/oasis_dataset.py:34 | #13 | Six dataset modules exist only to declare an empty `pass` subclass that adds nothing: oasis:34, hypersim_dataset.py:59, ibims:34, interiorverse:39, nyu:73, scannet:56. Each costs an import, a file, and a lookup for zero behavior. | `class OasisNormalsDataset(BaseNormalsDataset): pass` |
| P1-78 | src/dataset/mixed_sampler.py:97 | none | Shipped docstring is the unedited IDE template header `"""_summary_`. | `"""_summary_` |
| P1-79 | src/dataset/mixed_sampler.py:47 | none | `self.base_sampler = None` is never read or written again anywhere in the repo — dead attribute on the sampler that drives all mixed-dataset training. | `self.base_sampler = None` |
| P1-80 | src/dataset/mixed_sampler.py:107 | none | `__iter__` pops from `self.raw_batches`, mutating the sampler as it yields; the object is single-use-per-refill and two concurrent iterations interfere. Nothing in the class or its docstring says so. | `batch_raw = self.raw_batches[idx_ds].pop()` |
| P1-81 | src/dataset/mixed_sampler.py:121 | none | A `if __name__ == "__main__"` "Unit test" with a nested `SimpleDataset` (whose `__init__` shadows the builtin `len`) lives in the production module; there is no test tree, so this is the repo's only test and it is unrunnable by any runner. | `# Unit test` |
| P1-82 | src/dataset/__init__.py:95 | #12 | `.keys()` on a membership test — `in dict` is the idiom and avoids building a view. | `elif cfg_data_split.name in dataset_name_class_dict.keys():` |
| P1-83 | src/dataset/__init__.py:79 | #1 | `get_dataset(cfg_data_split, base_data_dir, mode, **kwargs)` has an unannotated config param, an opaque kwargs bag, and a six-arm `Union` return (three dataset types plus their lists) — the caller cannot know from the signature whether it gets one dataset or many. | `) -> Union[BaseDepthDataset, BaseIIDDataset, BaseNormalsDataset, List[...], ...]:` |
| P1-84 | src/dataset/hypersim_dataset.py:78 | none | Manual `del` of three locals immediately after they were wrapped into `rasters` — the tensors are still referenced by the dict, so the statement frees nothing. | `del albedo_raw, shading_raw, residual_raw` |
| P1-85 | script/depth/train.py:70 | #29 | The entire 310-line program is an un-named `if __name__` block with no module docstring and no `main()`: nothing here is importable, testable, or callable, and the file's first screen says nothing about what it costs to run. Identical shape in script/normals/train.py:66 and script/iid/train.py:66. | `if "__main__" == __name__:` |
| P1-86 | script/depth/train.py:226 | #12 | Directory copy, tar and recursive delete are shelled out through `os.system` with an unquoted f-string interpolation, where `shutil.copytree`/`tarfile`/`shutil.rmtree` are the stdlib vocabulary; `rm -rf` also makes the snapshot step Unix-only. | `os.system(f"rsync --relative -arhvz --quiet --filter=':- .gitignore' --exclude '.git' . '{_temp_code_dir}'")` |
| P1-87 | script/depth/train.py:282 | none | `logging.debug` is given a second positional argument as if it were `print`; the format string has no `%s`, so this logs a `TypeError` traceback from the logging internals instead of the augmentation config. | `logging.debug("Augmentation: ", cfg.augmentation)` |
| P1-88 | script/depth/train.py:380 | none | The whole training run is wrapped in `except Exception: logging.exception(e)`, so a crashed training job logs and exits 0 — no scheduler, CI, or wrapper script can detect the failure. | `except Exception as e: logging.exception(e)` |
| P1-89 | script/depth/train.py:169 | #12 | Four consecutive `if not os.path.exists(d): os.makedirs(d)` blocks (:169-179) reimplement `os.makedirs(..., exist_ok=True)` — which the very same file uses correctly elsewhere (script/depth/run.py:168). | `out_dir_ckpt = os.path.join(out_dir_run, "checkpoint"); if not os.path.exists(out_dir_ckpt): os.makedirs(out_dir_ckpt)` |
| P1-90 | script/depth/train.py:32 | none | `import os` appears twice (:32 before the `sys.path` hack and :38 in the sorted block); the same duplicate is in script/depth/infer.py:32/39, script/depth/eval.py:32/39 and script/depth/run.py. | `import os` |
| P1-91 | script/depth/train.py:34 | none | Every one of the 12 entry-point scripts begins by mutating `sys.path` to reach the repo root — a packaging workaround copy-pasted 12 times that also makes import order significant. | `sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..")))` |
| P1-92 | script/depth/train.py:76 | #11 | script/depth/train.py and script/normals/train.py differ by 20 lines out of 381 (verified by diff): same argparse block, same wandb/tensorboard bootstrap, same slurm copy, same loader construction. script/iid/train.py is a third copy. | `description="Marigold : Monocular Depth Estimation : Training"` |
| P1-93 | script/depth/infer.py:208 | #2 | `dataset` was just annotated `BaseDepthDataset` on the previous statement; the `isinstance` assert re-checks a fact the annotation already asserts. | `assert isinstance(dataset, BaseDepthDataset)` |
| P1-94 | script/depth/infer.py:248 | #2 | `seed` is unconditionally bound to an `int` at :168 (`seed = int(time.time())` in the `None` branch), so this `is None` test can never be true and `generator = None` is unreachable. | `if seed is None: generator = None` |
| P1-95 | script/depth/infer.py:181 | none | `if "y" == response: pass` — an empty branch written to document intent; and the "invalid input" path recurses (:188) instead of looping, so a piped-EOF stdin recurses until it raises. | `if "y" == response: pass` |
| P1-96 | script/depth/infer.py:172 | none | A batch inference script blocks on interactive `input()` when the output dir exists; there is no `--force`/`--yes` flag, so the script cannot run unattended (which is its only use). | `response = (input(f"The directory '{directory}' already exists. Are you sure to continue? (y/n): ")...)` |
| P1-97 | script/depth/eval.py:164 | none | The prediction file is loaded before the existence check that is supposed to skip it — `np.load` raises `FileNotFoundError` at :164 and the `continue` at :168 is unreachable. | `depth_pred = np.load(pred_path).astype(np.float32)` then `if not os.path.exists(pred_path): ... continue` |
| P1-98 | script/depth/eval.py:59 | #11 | The `eval_metrics` list is duplicated verbatim between this script (:59-70) and `config/train_marigold_depth.yaml:84-94`; the two are consumed by the same `getattr(metric, ...)` mechanism and can drift silently. | `eval_metrics = ["abs_relative_difference", "squared_relative_difference", ...]` |
| P1-99 | script/depth/eval.py:172 | none | `scale` and `shift` are unpacked from both alignment branches (:172 and :188) and never used. | `depth_pred, scale, shift = align_depth_least_square(...)` |
| P1-100 | script/depth/eval.py:233 | none | `metric_tracker.result()` (a pandas `dict(...)` build) is called twice in one expression to get its keys and then its values. | `tabulate([metric_tracker.result().keys(), metric_tracker.result().values()])` |
| P1-101 | script/depth/run.py:168 | none | `os.makedirs(output_dir, exist_ok=True)` is called at :168 and again at :239 inside the `torch.no_grad()` block — the second is pure noise. | `os.makedirs(output_dir, exist_ok=True)` |
| P1-102 | script/depth/run.py:166 | none | Variable named `output_dir_tif` points at a directory literally named `depth_bw` into which `.png` files are written (:279) — the name is wrong twice over. | `output_dir_tif = os.path.join(output_dir, "depth_bw")` |
| P1-103 | script/depth/run.py:273 | #11 | The "warn if the target exists, then write" trio is copy-pasted three times in one loop body (:273, :280, :288) with only the path variable changing. | `if os.path.exists(npy_save_path): logging.warning(f"Existing file: '{npy_save_path}' will be overwritten")` |
| P1-104 | script/depth/run.py:47 | #11 | `EXTENSION_LIST = [".jpg", ".jpeg", ".png"]` is declared identically in script/depth/run.py:47, script/normals/run.py:47 and script/iid/run.py:48 — one fact, three homes, none importable. | `EXTENSION_LIST = [".jpg", ".jpeg", ".png"]` |
| P1-105 | script/normals/dataset_preprocess/hypersim/hypersim_util.py:35 | #11 | This 74-line module is byte-identical to script/iid/dataset_preprocess/hypersim_lighting/hypersim_util.py except that the function is renamed `tone_map` -> `tone_map_hypersim`; script/depth/dataset_preprocess/hypersim/hypersim_util.py is a third, 21-line-divergent copy. | `def tone_map(rgb, entity_id_map):` |
| P1-106 | script/normals/dataset_preprocess/hypersim/hypersim_util.py:35 | #25 | The same tone-mapping function is reachable under two names in two packages (`tone_map` and `tone_map_hypersim`), so neither name greps to all uses. | `def tone_map_hypersim(rgb, entity_id_map):` |
| P1-107 | script/depth/dataset_preprocess/hypersim/hypersim_util.py:31 | #12 | numpy and its functions are imported through `pylab` — matplotlib's deprecated MATLAB-emulation namespace — pulling a GUI plotting stack into a data-preprocessing utility to get `np.percentile`. | `from pylab import count_nonzero, clip, np` |
| P1-108 | script/depth/dataset_preprocess/hypersim/preprocess_hypersim.py:39 | none | Sibling module imported by bare name with no `sys.path` setup (unlike every other script in the repo), so the file only runs when CWD is its own directory. | `from hypersim_util import dist_2_depth, tone_map` |
| P1-109 | script/depth/dataset_preprocess/hypersim/preprocess_hypersim.py:68 | none | `os.makedirs` without `exist_ok=True` on the split output dir — the script cannot be resumed or re-run after a partial failure, and the failure comes 30 lines before any work. | `os.makedirs(split_output_dir)` |
| P1-110 | script/depth/dataset_preprocess/hypersim/preprocess_hypersim.py:61 | none | Jupyter cell markers (`# %%` at :61 and :65) left in a shipped CLI script — notebook residue that reads as structure but marks nothing. | `# %%` |
| P1-111 | script/depth/dataset_preprocess/hypersim/preprocess_hypersim.py:135 | none | `rgb_path` is rebound from the *input*-relative HDF5 path (:85) to an *output*-relative PNG path in the middle of the loop body; the same name means two different things 50 lines apart. | `rgb_path = os.path.join(scene_path, rgb_name)` |
| P1-112 | config/train_marigold_depth.yaml:54 | none | `optimizer.name: Adam`, `lr_scheduler.name: IterExponential` and `pipeline.name` are declared in every training config but never read by any code — the trainers hardcode `Adam(...)` (marigold_depth_trainer.py:105) and `IterExponential(...)` (:108), reading only the `.kwargs` subtrees. Config that looks pluggable and is not. | `optimizer:` / `  name: Adam` |
| P1-113 | src/util/alignment.py:98 | #13 | `disparity2depth` is a pure forward to `depth2disparity` with an opaque `**kwargs` — a middle-man whose only content is the claim that the transform is its own inverse. | `def disparity2depth(disparity, **kwargs): return depth2disparity(disparity, **kwargs)` |
| P1-114 | src/util/alignment.py:86 | none | `depth2disparity` binds `disparity` in an isinstance if/elif with no else; a list or scalar raises `UnboundLocalError` at :91. | `if isinstance(depth, torch.Tensor): ... elif isinstance(depth, np.ndarray):` |
| P1-115 | src/util/alignment.py:39 | none | `return_scale_shift` is a boolean flag that changes the arity of the return (3-tuple vs 1 value), untyped and defaulted to `True`; both call sites pass it explicitly, so the default serves nobody. | `return_scale_shift=True,` |
| P1-116 | src/util/metric.py:37 | none | `MetricTracker` carries a 3-column pandas `DataFrame` and does `.loc`-indexed scalar arithmetic per sample to maintain three running counters — a pandas dependency and per-update indexing cost for what a dict of running sums does. | `self._data = pd.DataFrame(index=keys, columns=["total", "counts", "average"])` |
| P1-117 | src/util/metric.py:45 | none | Reset writes through `.values[:]` on a column slice, which pandas does not guarantee to be a view — a documented copy-vs-view trap; `self._data[:] = 0` is the supported form. | `self._data[col].values[:] = 0` |
| P1-118 | src/util/metric.py:304 | none | Three of four `lstsq` outputs are unpacked and unused, and the returned `x` is a `[1,1]` tensor used downstream as a scalar multiplier (metric.py:268) — the shape contract is implicit. | `x, residuals, rank, s = torch.linalg.lstsq(A_flattened.float(), b_flattened.float())` |
| P1-119 | marigold/marigold_depth_pipeline.py:455 | none | Loop variable `i` is unused in the denoising loop (also iid:525, normals), and `logvar` is unpacked and discarded in every `encode_rgb` (:493) — three copies of the same dead binding. | `for i, t in iterable:` |
| P1-120 | src/trainer/marigold_depth_trainer.py:208 | #17 | `train` is a 193-line function whose live-local set collapses at the `if accumulated_step >= self.gradient_accumulation_steps:` boundary (:348): everything above is per-micro-batch tensor work, everything below is per-effective-iteration bookkeeping that shares only two scalars. The function is already two functions. | `if accumulated_step >= self.gradient_accumulation_steps:` |

## Phase 2 — audit finding verdicts

272 findings. Grouped where a rule fires many near-identical times; fp
exceptions split out so every finding line is accounted for.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| trainer save/load_checkpoint, _train_step_callback, _get_next_seed, validate, visualize, encode_rgb, _replace_unet_conv_in; pipeline encode_rgb/_check_inference_step/encode_empty_text; metric.py:235-255 sub*_error; ensemble.py:120/252; base_* dataset __getitem__/__del__/_read_image/_read_rgb; dataset _read_depth_file x5; image_util chw2hwc/float2int; hypersim_util:35 (x75) | 11 | indexed | real | Verified by diff: trainer methods byte-identical across depth/normals/iid; sub*_error and _read_depth_file differ only by a literal; two live image_util homes. Genuine clone groups. |
| module-docstring-absent on substantial modules (trainers 666-734L, pipelines 479-585L, entry scripts 271-381L, metric 338L, base datasets, ensemble, run/infer/eval) + heavy IO/training entry points (train, validate, save/load_checkpoint, validate_single_dataset, process_sample, create_interiorverse_target, multi_res_noise_like) (x58) | 29 | heuristic | real | Non-trivial modules and genuinely heavy entry points ship with no top-loading map or cost declaration. |
| batchsize:1(90L), alignment:1(99L), hypersim_util:1(95L), preprocess_interiorverse_iid:1(94L), interiorverse_dataset:1(83L); hypersim_util:35 tone_map(39L)x3, align_depth_least_square:35(47L), quantile_map:308(30L) (x10) | 29 | heuristic | fp | Trivial single-purpose modules and modest pure functions — flagging doc-presence here is the over-fire the inventory warns against. |
| encode_rgb/encode_depth forwarders (x4), _replace_unet_conv_in x3, fill_outputs, log_dict, total_length (x10) | 22 | heuristic | real | Forwarders and model/config helpers touching only public interface — genuine free-functions-hiding. |
| template overrides _get_data_path/_get_valid_mask/_load_rgb_data/_load_depth_data/_augment_data/_read_* across datasets + validate_single_dataset x2 + scale_back (x18) | 22 | heuristic | fp | Polymorphic override hooks are methods by necessity (dispatch); orchestration methods legitimately use many self attrs. |
| trainer _replace_unet_conv_in (self.model.unet.conv_in.weight), __init__/train/save/load_checkpoint/validate_single_dataset (self.cfg.lr_scheduler.kwargs.*, self.model.unet.config), pipeline _check_inference_step (self.scheduler.config.*) (x21) | 30 | heuristic | real | Genuine Demeter chains 3-4 hops into model internals and nested config. |
| marigold_depth_pipeline:234, marigold_iid_pipeline:305, marigold_normals_pipeline:212, src/util/image_util:44 (x4) | 2 | proved/heur | real | isinstance on a Union after the other arm is narrowed — genuinely redundant per the declared contract. |
| marigold_depth_pipeline:324 (color_map), *_trainer:595/560/625 (save_to_dir), ensemble:158 (max_res), base_* dataset:226/174/176 (augm_args), diode:57/eth3d:58 (tar_obj), seeding:53 (initial_seed) (x11) | 2 | proved/heur | fp | Guard is load-bearing at runtime (param nullable at call sites / lazy-init); redundancy is an artifact of a too-narrow annotation. Deleting breaks — not removable noise. |
| metric:64/148, loss:57, pipeline __call__:155, trainer __init__:61 + validate_single_dataset:511 + load_checkpoint:663, single_infer:397, base_depth __init__:62 (x10) | 14 | indexed | real | Recurring param groups (output/target/valid_mask; trainer ctor bundle; pipeline call bundle) that want a named type. |
| get_loss:34, get_dataset:79, init_wandb:95, disparity2depth:98, denormalize:71/128 (**kwargs); target_properties:100, eval_dict_to_text:123 (bare dict) (x8) | 1 | heuristic | real | Opaque kwargs bags and bare dict on public boundaries — callers must read the body to learn the contract. |
| single_infer:473/438, _replace_unet_conv_in:202/205 (x4) | 17 | heuristic | real | Compute methods with a real encode/loop/decode (or two-conv) phase structure at the neck. |
| trainer __init__:164/181/167 (x3) | 17 | heuristic | fp | A constructor setting many attributes legitimately necks; not a split signal. |
| pipeline classes (fan-in 11-12, 479-585L) + *Output classes (fan-in 7) (x6) | 27 | indexed | real | Hot symbols in large modules an agent must ingest whole. |
| src/dataset/__init__.py:78 get_dataset (x1) | 6 | indexed | real | `get_`-named factory that opens the filename-list file on construction — effect hidden behind an accessor name. |
| get_pred_name:271, get_path_stem:100, read_img_from_file:124, is_on_slurm:34, get_local_scratch_dir:40 (x5) | 6 | indexed | fp | get_pred_name/get_path_stem are pure string builders (os.path mis-classed as io); read_img_from_file honestly names its IO; is_on_slurm/get_local_scratch_dir are env reads that are the declared purpose. |
| disparity2depth->depth2disparity, denormalize->scale_back, delta1/2/3_acc->threshold_percentage (x5) | 25 | indexed | real | Delegations whose caller/callee share no name stem — un-greppable / semantically reversed chain. |
| getattr(metric,_met) x3, getattr(dataloader,k), eval getattr (x5) | 24 | heuristic | real | Identifiers built from config strings — unfindable by search, blind whole-program guarantees. |
| marigold_iid_pipeline:148 fill_entry (x1) | 16 | heuristic | real | 14 pure processing statements then a batch of entry.* writes — genuine compute/effect split. |
| *_trainer train (267/223/226) (x3) | 16 | heuristic | fp | train() mutates state throughout the loop; not a pure core with an end-only mutation tail. |
| base_normals:44 tarfile.open in 3 methods; trainer:60/64/63 backup-save idiom in 3 methods (x4) | 21 | heuristic | real | Same self-rooted expression repeated across methods of one class — wants encapsulation. |
| marigold/*_pipeline __call__:227/298/205 (x3) | 18 | heuristic | real | __call__ narrates its phases with banner comments — boundaries spelled in prose. |
| preprocess_hypersim_iid:46 psnr x2, logging_util:123 eval_dict_to_text (x3) | 15 | heuristic | fp | psnr does elementwise image1-image2 (full use; analyzer undercounts __sub__); eval_dict_to_text reads keys+values = the whole mapping. |
| logging_util:78 set_dir (tb_log_dir) (x1) | 5 | indexed | real | tb_log_dir is a str at the sole call site; lifting the annotation is sound. |
| src/util/image_util:60 img_int2float (dtype) (x1) | 5 | indexed | fp | The dtype=None path is exercised (read_img_from_buffer calls it with no dtype); lifting would be wrong. |
| src/util/alignment:98 disparity2depth (x1) | 13 | indexed | real | Body is a single forward to depth2disparity — a hop that adds no meaning. |
| src/util/logging_util:123 eval_dict_to_text (x1) | 10 | indexed | real | Demands concrete dict for val_metrics while the body uses only keys/values — a Mapping suffices. |

Phase 2 totals: 272 judged — 218 real, 54 fp.

## Phase 3 — reconciliation

| P1 id | rule | class | note |
|-------|------|-------|------|
| P1-1 | #1 | detector-miss | `ensemble_kwargs: Dict` (bare Dict) not flagged; #1 fired elsewhere. |
| P1-2 | #28 | detector-miss | #28 fired 0x; docstring arg-name drift uncaught. |
| P1-3 | none | inventory-gap | Optional-then-`>=` None crash — no rule. |
| P1-4 | #6 | detector-miss | encode_empty_text flagged by #11 (clone) but not #6 (its state-write). |
| P1-5 | #18 | covered | #18 at __call__:227/298/205. |
| P1-6 | #11 | covered | #11 _check_inference_step iid:413/normals:310. |
| P1-7 | #11 | covered | #11 encode_rgb x3. |
| P1-8 | none | inventory-gap | ensemble absolute-mode unreachable raise — no rule. |
| P1-9 | #11 | covered | #11 ensemble.py:252. |
| P1-10 | #28 | detector-miss | #28 fired 0x; docstring default drift uncaught. |
| P1-11 | none | inventory-gap | wrong var in error string — no rule. |
| P1-12 | none | inventory-gap | UnboundLocalError on missing else — no rule. |
| P1-13 | #1 | detector-miss | colorize_depth_maps fully un-annotated; #1 keys on Any/dict/kwargs. |
| P1-14 | #11 | covered | #11 image_util float2int:137/chw2hwc:79. |
| P1-15 | #25 | detector-miss | img_ dual-naming not flagged; #25 fired elsewhere. |
| P1-16 | #28 | detector-miss | #28 fired 0x. |
| P1-17 | #1 | covered | #1 target_properties:100. |
| P1-18 | none | inventory-gap | Optional lie subscript crash — no rule. |
| P1-19 | none | inventory-gap | silent fallthrough on unknown prediction_space — no rule. |
| P1-20 | #15 | detector-miss | fill_entry wallet not flagged; #15 fired on psnr/eval_dict. |
| P1-21 | #11 | covered | #11 save_checkpoint x3. |
| P1-22 | #11 | covered | #11 load_checkpoint/_train_step_callback/_get_next_seed x3. |
| P1-23 | #11 | threshold-miss | trainer __init__ ~95% identical but attr diffs kept it under the clone cutoff. |
| P1-24 | #14 | covered | #14 depth_trainer:61 ctor param group. |
| P1-25 | #24 | covered | #24 getattr(metric,_met). |
| P1-26 | #12 | detector-miss | #12 fired 0x; `__str__()` idiom uncaught. |
| P1-27 | #12 | detector-miss | #12 fired 0x; manual open/close uncaught. |
| P1-28 | none | inventory-gap | and/or precedence risk — no rule. |
| P1-29 | none | inventory-gap | UnboundLocalError — no rule. |
| P1-30 | #13 | covered | encode_rgb trainer forwarder flagged by #22 (velcro) + #11 at site. |
| P1-31 | none | inventory-gap | config-dependent attribute set — no rule. |
| P1-32 | none | inventory-gap | misspelled `scheduelr_path` — no rule. |
| P1-33 | #9 | detector-miss | #9 fired 0x; tb_logger global mutation uncaught. |
| P1-34 | none | inventory-gap | unenforced must-call-set_dir-first — no rule (#7 fired 0x). |
| P1-35 | none | inventory-gap | no-op `global` — no rule. |
| P1-36 | #1 | covered | #1 init_wandb:95. |
| P1-37 | #2 | covered | #2 seeding.py:53. |
| P1-38 | #9 | detector-miss | #9 fired 0x; random.seed global perturbation uncaught. |
| P1-39 | #11 | covered | #11 metric.py:235 sub*_error. |
| P1-40 | #13 | covered | delta1/2/3_acc flagged by #25 + #14 at site. |
| P1-41 | #11 | threshold-miss | rmse_linear/log/i_rmse differ in the diff line — under the T2 cutoff. |
| P1-42 | none | inventory-gap | rename-only locals — no rule. |
| P1-43 | #6 | detector-miss | compute_iid_metric mutates caller tensors; not flagged. |
| P1-44 | #12 | detector-miss | #12 fired 0x; zeros/ones-where uncaught. |
| P1-45 | none | inventory-gap | dead `avg` method — no dead-code rule. |
| P1-46 | #1 | detector-miss | MetricTracker(*keys) varargs schema not flagged. |
| P1-47 | #1 | covered | #1 get_loss:34. |
| P1-48 | #11 | threshold-miss | SILogRMSE __call__ near-clone of SILogMSE — under cutoff. |
| P1-49 | none | inventory-gap | `(_type_)` docstring placeholder — no rule. |
| P1-50 | none | inventory-gap | commented-out code — no rule. |
| P1-51 | #12 | detector-miss | #12 fired 0x; hand-rolled ABC uncaught. |
| P1-52 | none | inventory-gap | heterogeneous return interface — no rule. |
| P1-53 | none | inventory-gap | duplicate consecutive `if` — no rule. |
| P1-54 | #24 | covered | #24 data_loader getattr. |
| P1-55 | none | inventory-gap | dead `total_length` (#22 flagged velcro, not dead) — no dead-code rule. |
| P1-56 | #11 | detector-miss | intra-function branch dup; clone detector is inter-function. |
| P1-57 | none | inventory-gap | device mix (x.device vs param) — no rule. |
| P1-58 | none | inventory-gap | transposed w/h labels — no rule. |
| P1-59 | none | inventory-gap | plotting demo in prod module — no rule. |
| P1-60 | #13 | detector-miss | get_local_scratch_dir wrapper not flagged (#6 flagged it instead). |
| P1-61 | none | inventory-gap | CWD-relative debug entry — no rule. |
| P1-62 | #13 | detector-miss | get_trainer_cls one-line lookup wrapper not flagged. |
| P1-63 | none | inventory-gap | unused `rgb_transform` param — no dead-code rule. |
| P1-64 | #1 | detector-miss | base-dataset `**kwargs` sinks not flagged (get_dataset kwargs was). |
| P1-65 | #1 | detector-miss | `augmentation_args: dict` not flagged by #1 (#2 flagged its guard). |
| P1-66 | #12 | detector-miss | #12 fired 0x; True-if-else-False uncaught. |
| P1-67 | #7 | detector-miss | #7 fired 0x; comment-borne subclass contract uncaught. |
| P1-68 | #11 | covered | #11 base-dataset __getitem__/__del__/_read_image x3. |
| P1-69 | none | inventory-gap | `_augment_data` missing return -> None — no rule. |
| P1-70 | #9 | detector-miss | #9 fired 0x; import-time os.environ mutation uncaught. |
| P1-71 | none | inventory-gap | empty-dict-not-raise stub — no rule. |
| P1-72 | #8 | detector-miss | #8 fired 0x; magic-768 dataset predicate uncaught. |
| P1-73 | none | inventory-gap | unconditional .cuda() — no rule. |
| P1-74 | none | inventory-gap | eager-vs-lazy tar open drift — no rule. |
| P1-75 | none | inventory-gap | no-op reshape — no rule. |
| P1-76 | none | inventory-gap | UnboundLocalError 4-dim — no rule. |
| P1-77 | #13 | detector-miss | empty `pass` subclasses not flagged. |
| P1-78 | none | inventory-gap | `_summary_` docstring template — no rule. |
| P1-79 | none | inventory-gap | dead `base_sampler` attr — no rule. |
| P1-80 | none | inventory-gap | mutating single-use iterator — no rule. |
| P1-81 | none | inventory-gap | unrunnable `__main__` unit test — no rule. |
| P1-82 | #12 | detector-miss | #12 fired 0x; `.keys()` membership uncaught. |
| P1-83 | #1 | covered | #1 get_dataset:79. |
| P1-84 | none | inventory-gap | no-op `del` — no rule. |
| P1-85 | #29 | covered | #29 train.py:1. |
| P1-86 | #12 | detector-miss | #12 fired 0x; os.system shell-outs uncaught. |
| P1-87 | none | inventory-gap | logging.debug used like print — no rule. |
| P1-88 | none | inventory-gap | except-all swallows exit code — no rule. |
| P1-89 | #12 | detector-miss | #12 fired 0x; hand-rolled makedirs(exist_ok) uncaught. |
| P1-90 | none | inventory-gap | duplicate `import os` — no rule. |
| P1-91 | none | inventory-gap | sys.path hack x12 — no rule. |
| P1-92 | #11 | detector-miss | train.py `__main__` bodies (not defs) escape the clone detector. |
| P1-93 | #2 | detector-miss | `assert isinstance` after annotation not flagged. |
| P1-94 | #2 | detector-miss | infer.py `seed is None` (always int) redundant guard not flagged. |
| P1-95 | none | inventory-gap | empty branch + recursion on EOF — no rule. |
| P1-96 | none | inventory-gap | interactive input() in batch script — no rule. |
| P1-97 | none | inventory-gap | np.load before existence check — no rule. |
| P1-98 | #11 | detector-miss | eval_metrics list dup across code/YAML; #11 is code-only. |
| P1-99 | none | inventory-gap | unused scale/shift unpack — no rule. |
| P1-100 | none | inventory-gap | result() called twice — no rule. |
| P1-101 | none | inventory-gap | duplicate makedirs — no rule. |
| P1-102 | none | inventory-gap | misnamed output_dir_tif — no rule. |
| P1-103 | #11 | detector-miss | in-loop warn/write triplet; intra-function, not flagged. |
| P1-104 | #11 | detector-miss | EXTENSION_LIST constant dup x3; #11 targets function clones. |
| P1-105 | #11 | covered | #11 hypersim_util.py:35. |
| P1-106 | #25 | detector-miss | tone_map/tone_map_hypersim rename not flagged. |
| P1-107 | #12 | detector-miss | #12 fired 0x; pylab-for-numpy uncaught. |
| P1-108 | none | inventory-gap | bare-name sibling import — no rule. |
| P1-109 | none | inventory-gap | makedirs without exist_ok — no rule. |
| P1-110 | none | inventory-gap | `# %%` notebook markers — no rule. |
| P1-111 | none | inventory-gap | rgb_path rebound mid-loop — no rule. |
| P1-112 | none | inventory-gap | dead config keys (optimizer/lr_scheduler.name) — no rule. |
| P1-113 | #13 | covered | #13 disparity2depth:98. |
| P1-114 | none | inventory-gap | UnboundLocalError depth2disparity — no rule. |
| P1-115 | none | inventory-gap | return-arity flag — no rule. |
| P1-116 | none | inventory-gap | pandas MetricTracker overkill — no rule. |
| P1-117 | none | inventory-gap | .values[:] copy-vs-view trap — no rule. |
| P1-118 | none | inventory-gap | implicit lstsq shape contract — no rule. |
| P1-119 | none | inventory-gap | unused loop var / logvar — no rule. |
| P1-120 | #17 | detector-miss | #17 fired on __init__/single_infer/_replace, not on train's neck. |

Phase 3 totals: 120 sites — 22 covered, 38 detector-miss, 3 threshold-miss, 57 inventory-gap.
