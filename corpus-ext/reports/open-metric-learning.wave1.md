# open-metric-learning — wave 1

Judged blind against research/SUMMARY.md §3 (rules #1–31). Prod tree read:
all of `oml/` except vendored `oml/models/*/external*` (upstream DINO / CLIP /
unicom / ECAPA copies). `tests/`, `pipelines/`, `docs/` not judged.

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | oml/interfaces/retrieval.py:12 | #1 | The whole postprocessor contract is `*args, **kwargs -> Any`, untyped and unannotated. Every implementer must `# type: ignore` its real signature (algo.py:16, algo.py:48, pairwise.py:54), and the one caller has to recover the contract by runtime introspection. | `def process(self, *args, **kwargs) -> Any:  # type: ignore` |
| P1-2 | oml/metrics/embeddings.py:215 | #1 | Consequence of P1-1: the caller discovers which arguments `process` accepts with `inspect.signature` at runtime instead of from the declared type. | `args = remove_unused_kwargs(args, self.postprocessor.process)` |
| P1-3 | oml/interfaces/datasets.py:35 | #1 | The central dataset contract is `Dict[str, Any]`; the actual keys (`input_tensors_key`, `index_key`, `labels_key`) are named only in the docstring prose, so no caller can be type-checked against a batch. Repeated at :14, :58, :120. | `def __getitem__(self, item: int) -> Dict[str, Any]:` |
| P1-4 | oml/interfaces/metrics.py:22 | #1 | Both abstract methods of `IBasicMetric` are `(*args: Any, **kwargs: Any) -> Any`; the required output shape of `compute_metrics` is given as a docstring code-block (lines 43–51), not a type. | `def setup(self, *args: Any, **kwargs: Any) -> Any:` |
| P1-5 | oml/interfaces/metrics.py:25 | #7 | Call-order protocol narrated in prose instead of encoded: nothing prevents `update_data` before `setup`, and `EmbeddingMetrics.setup` is what allocates `self.acc`. | `Has to be called before the first call of ``self.update_data()``.` |
| P1-6 | oml/registry/losses.py:21 | #1 | `**kwargs: Dict[str, Any]` declares *each keyword value* to be a dict — the annotation is wrong, not merely weak. Same wrong annotation in miners.py:22, models.py:41/52/62, optimizers.py:21/25, samplers.py:20/26, schedulers.py:37/41, loggers.py:31, postprocessors.py:17, transforms.py:41. | `def get_criterion(name: str, **kwargs: Dict[str, Any]) -> nn.Module:` |
| P1-7 | oml/interfaces/models.py:12 | #9 | `pretrained_models` is a mutable dict on the base class shared by every `IExtractor` subclass that doesn't override it; `IPairwiseModel` subclasses redeclare their own empty copies (siamese.py:65, :148). | `pretrained_models: Dict[str, Any] = {}` |
| P1-8 | oml/models/meta/projection.py:98 | #9 | `from_pretrained` mutates the class-level `pretrained_models` dict permanently: it pops `extractor_creator` and writes `extractor`. A second call for the same weights raises `KeyError: 'extractor_creator'`. | `ini["extractor"] = ini.pop("extractor_creator")()  # type: ignore` |
| P1-9 | oml/losses/triplet.py:183 | #9 | Mutable default argument: one `AllTripletsMiner` instance is constructed at import time and shared by every `TripletLossWithMiner` that omits `miner`. | `miner: ITripletsMiner = AllTripletsMiner(),` |
| P1-10 | oml/models/texts/huggingface.py:48 | #26 | Typo `__all_` (one trailing underscore) means the module declares no `__all__` at all, and the single name it lists does not exist in the module — the class is `HFWrapper`. | `__all_ = ["HuggingFaceWrapper"]` |
| P1-11 | oml/const.py:26 | #26 | Module constant computed by a function that raises on any platform other than linux/darwin/win, so `import oml.const` — the root of the import graph — can fail at import time. | `CACHE_PATH = get_cache_folder()` |
| P1-12 | oml/registry/transforms.py:51 | #26 | `TRANSFORMS_FOR_PRETRAINED`'s 33 values are all *executed* at import time (`get_normalisation_resize_hypvit(...)`, `unicom.transform(224)`), so importing the registry builds 33 transform objects; a reader must run the code to know the table. | `"resnet50_moco_v2": get_normalisation_resize_hypvit(im_size=256, crop_size=224),` |
| P1-13 | oml/registry/samplers.py:14 | #26 | Registry assembled by dict-splat rather than declared literally; grep for `"category_balance"` finds the entry but not that it is in `SAMPLERS_REGISTRY`. Same at transforms.py:38. | `SAMPLERS_REGISTRY = {**SAMPLERS_CATEGORIES_BASED, "balance": BalanceSampler}` |
| P1-14 | oml/utils/misc.py:50 | #28 | Docstring names a parameter `x0` that does not exist; the parameter is `value`. | `Indices of the all elements equal to x0` |
| P1-15 | oml/utils/misc_torch.py:459 | #28 | Docstring names `explained_variance_ths`, a symbol that exists nowhere in the repo; the parameter is `pcf_variance`. Same docstring also claims `Returns: List of ...` while the body returns a Tensor (line 494). | `Function estimates the number of principal axes that are required to explain the `explained_variance_ths`` |
| P1-16 | oml/utils/misc_torch.py:105 | #2 | The `if` is provably always true: line 101 asserts `len(x1.shape) == 2` four lines earlier. Dead guard around the core of the function. | `assert len(x1.shape) == len(x2.shape) == 2` / `if len(x1.shape) == 2:` |
| P1-17 | oml/losses/arcface.py:45 | #2 | `smoothing_epsilon` is annotated `float` with default `0`; the `is None` disjunct can never be true, so the assert is half-dead and the annotation and the check disagree about the domain. | `smoothing_epsilon is None or 0 <= smoothing_epsilon < 1` |
| P1-18 | oml/retrieval/retrieval_results.py:249 | #2 | `dataset_query` is annotated `IVisualizableDataset`; the isinstance guard is unprovable-false by the declared type. Same pattern at :252 and at :301 (`dataset: IQueryGalleryDataset` re-checked for `IQueryGalleryDataset`). | `if not isinstance(dataset_query, IVisualizableDataset):` |
| P1-19 | oml/utils/misc_torch.py:196 | #2 | `data: TData` is `Tuple[List, Tensor, ndarray]`, so `isinstance(data, (list, tuple))` is always true and the `else` branch at :198 is unreachable under the annotation. The related `assert len(ids) == len(data)` at :191 compares against the tuple's arity (always 3). | `if isinstance(data, (list, tuple)):` |
| P1-20 | oml/datasets/images.py:50 | #2 | `bbox`'s elements come from `int(row[...])` on the previous line, so none can be `None`; the guard is unreachable (an actual `None` would already have raised inside `int`). | `bbox = None if any(coord is None for coord in bbox) else bbox` |
| P1-21 | oml/datasets/audios.py:126 | #2 | `spec_repr_func` is non-Optional with a function default, so it is always truthy and the `or` fallback is dead. | `self._spectral_function = spec_repr_func or default_spec_repr_func` |
| P1-22 | oml/datasets/audios.py:93 | #2 | `convert_to_mono: bool` re-checked with isinstance in the same signature's body. | `assert isinstance(convert_to_mono, bool), "'convert_to_mono' must be a boolean."` |
| P1-23 | oml/functional/metrics.py:181 | #11 | `calc_cmc`, `calc_precision`, `calc_map` are one AST shape three times: same `check_if_nonempty_positive_integers` preamble, same inner `*_single(is_correct, n_gt, k_)` with the identical 4-line empty/zero-gt guard, same `for k in top_k: items = tqdm(...) if verbose else zip(...)` loop. Copies at :274 and :353. | `if n_gt == 0 and len(is_correct) == 0: return 1.0` |
| P1-24 | oml/functional/metrics.py:114 | #11 | `reduce_metrics` and `take_unreduced_metrics_by_mask` (:128) are the same recursive dict walk differing only in the leaf operation; one parameterised walker would remove the copy. | `for k, v in metrics_to_reduce.items(): if isinstance(v, (Tensor, np.ndarray)): output[k] = v.mean()` |
| P1-25 | oml/datasets/dataframe.py:114 | #11 | `DFQueryGalleryLabeledDataset.__init__` is a verbatim line-union of `DFLabeledDataset.__init__` (:47) and `DFQueryGalleryDataset.__init__` (:83); `__len__`, `__getitem__`, `get_labels`, `get_label2category`, `get_query_ids`, `get_gallery_ids` are all duplicated character-for-character across the three classes. | `self._query_ids = BoolTensor(df[IS_QUERY_COLUMN]).nonzero().squeeze()` |
| P1-26 | oml/datasets/texts.py:110 | #11 | Three constructors (`TextLabeledDataset` :110, `TextQueryGalleryLabeledDataset` :145, `TextQueryGalleryDataset` :176) contain the identical `TextBaseDataset(...)` construction; `oml/datasets/images.py` :191/:235/:270 repeats the same shape a further three times with `ImageBaseDataset`. Six copies of one factory. | `dataset = TextBaseDataset(texts=df[TEXTS_COLUMN], tokenizer=tokenizer, max_length=max_length,` |
| P1-27 | oml/datasets/audios.py:100 | #11 | The `extra_data` length-validation block is verbatim in `AudioBaseDataset` (:100), `ImageBaseDataset` (images.py:96) and `TextBaseDataset` (texts.py:44); the matching `for key, record in self.extra_data.items()` merge block is verbatim at audios.py:195, images.py:152, texts.py:79. Belongs on `IBaseDataset`. | `assert all(len(record) == len(paths) for record in extra_data.values()),` |
| P1-28 | oml/models/vit_clip/extractor.py:21 | #11 | `vitb16_224`, `vitb32_224` (:32), `vitl14_224` (:43), `vitl14_336` (:54) are the same 8-line constructor differing only in four integers — a parameter table written as four functions. | `return VisionTransformer(output_dim=512, input_resolution=224, layers=12, width=768, patch_size=16, heads=12)` |
| P1-29 | oml/models/meta/projection.py:68 | #11 | The checkpoint-loading sequence (torch.load → pull `state_dict` → strip criterion → strip prefix → `load_state_dict(strict=True)`) is copy-pasted across five extractors: projection.py:68, siamese.py:108, vit_clip/extractor.py:156, audio/ecapa_tdnn/extractor.py:67, resnet/extractor.py:217. | `loaded = torch.load(weights, map_location="cpu", weights_only=False)` |
| P1-30 | oml/models/utils.py:61 | #11 | `patch_float` and `patch_device` (:84) share an identical 8-line preamble (graph collection, `forward1` append) and an identical recursion tail; only the inner node predicate differs. | `graphs = [module.graph] if hasattr(module, "graph") else []` |
| P1-31 | oml/samplers/category_balance.py:51 | #11 | The 17-line validation block (six `raise ValueError` clauses, verbatim messages) is duplicated in `DistinctCategoryBalanceSampler.__init__` at distinct_category_balance.py:82. | `if not 1 <= n_categories <= len(unique_categories): raise ValueError(...)` |
| P1-32 | oml/samplers/category_balance.py:84 | #11 | Same-function duplicate: `category2labels` is computed at :47–50 and then recomputed identically into `self._category2labels` at :84–87. The sibling sampler (distinct_category_balance.py:112) assigns the first result instead. | `self._category2labels = {category: {label for label, cat in self._label2category.items() if category == cat}` |
| P1-33 | oml/lightning/modules/extractor.py:99 | #11 | `configure_optimizers` is byte-identical to pairwise_postprocessing.py:88; so is `get_progress_bar_dict` (:112 vs :108) and the DDP subclass `__init__` (:138 vs :116). Three clone pairs between two sibling modules. | `if isinstance(self.scheduler, ReduceLROnPlateau): scheduler["monitor"] = self.monitor_metric` |
| P1-34 | oml/lightning/modules/pairwise_postprocessing.py:101 | #11 | Drifted clone of extractor.py:118: same `on_epoch_start` freeze logic, but the extractor version raises `ValueError` when the model is not `IFreezable` while this one silently does nothing. Two homes, two behaviours. | `if self.freeze_n_epochs and isinstance(self.model, IFreezable):` |
| P1-35 | oml/retrieval/retrieval_results.py:225 | #11 | `visualize_qg` (:225) and `visualize` (:279) are the same 45-line body: name capture, two isinstance TypeErrors, `nq1 != nq2` check, verbose print, two closures, forward to `visualize_with_functions`. Only how the two closures resolve the item differ. | `nq1, nq2 = len(self.retrieved_ids), len(dataset_query)` |
| P1-36 | oml/retrieval/postprocessors/algo.py:16 | #11 | `ConstantThresholding.process` and `AdaptiveThresholding.process` (:48) share the same skeleton: `is_empty` guard, two accumulator lists, `zip(rr.distances, rr.retrieved_ids)` loop, identical `RetrievalResults(..., gt_ids=deepcopy(rr.gt_ids))` tail. | `rr_upd = RetrievalResults(distances=distances_upd, retrieved_ids=rids_upd, gt_ids=deepcopy(rr.gt_ids))` |
| P1-37 | oml/miners/inbatch_all_tri.py:131 | #11 | `get_available_triplets_naive` is a second, independent implementation of `get_available_triplets` (:45) living in the prod tree; it is referenced only from `tests/test_oml/test_miners/test_inbatch_all_tri.py`. A test oracle shipped as library code. | `def get_available_triplets_naive(labels: List[int], max_out_triplets: int = maxsize) -> TTripletsIds:` |
| P1-38 | oml/interfaces/models.py:14 | #13 | `IExtractor.extract` is a forward-only hop over `self.forward` that adds nothing; it exists only so callers can spell the intent differently (metrics path uses `extract`, training path uses `__call__`). | `def extract(self, x: Tensor) -> Tensor: return self.forward(x)` |
| P1-39 | oml/datasets/texts.py:121 | #13 | Six identical one-line `visualize` forwarders: texts.py:121, :155, :187 and images.py:204, :248, :282, each `return self._dataset.visualize(item=item, color=color)`. | `def visualize(self, item: int, color: TColor) -> np.ndarray: return self._dataset.visualize(item=item, color=color)` |
| P1-40 | oml/losses/triplet.py:163 | #13 | `last_logs` is a forward-only property with a copy-pasted docstring in five places: triplet.py:85, :163, :261 and arcface.py:99, :155. Two of them (:163, arcface.py:155) forward verbatim to a wrapped object. | `return self.criterion.last_logs` |
| P1-41 | oml/registry/models.py:52 | #13 | `get_extractor` and `get_pairwise_model` (:62) are single forwarding calls into `_get_model` that add only a constant registry argument; `get_transforms` (transforms.py:41) is the same with a pointless temporary. | `return _get_model(model_name=model_name, registry=EXTRACTORS_REGISTRY, **kwargs)` |
| P1-42 | oml/metrics/embeddings.py:34 | #15 | `calc_retrieval_metrics_rr` demands the whole `RetrievalResults` object but reads exactly two attributes (`retrieved_ids`, `gt_ids`) and forwards neither — the classic narrow-the-demand case. | `retrieved_ids=rr.retrieved_ids, gt_ids=rr.gt_ids,` |
| P1-43 | oml/datasets/images.py:184 | #10 | The three DF-backed image datasets declare `transform: Optional[albu.Compose]`, but the object they forward it to (`ImageBaseDataset`, :74) accepts `Optional[TTransforms]` and explicitly supports `torchvision.transforms.Compose` (:120). The public wrappers are narrower than the thing they wrap. | `transform: Optional[albu.Compose] = None,` |
| P1-44 | oml/datasets/images.py:288 | #1 | A public entry point takes its two most important arguments as bare `Any`, while the same concept is typed `TTransforms` one file over. | `transforms_train: Any,` / `transforms_val: Any,` |
| P1-45 | oml/datasets/texts.py:16 | #1 | Public type aliases that are literally `Any`, used across four constructors' published signatures. Same at models/texts/huggingface.py:8–9 (`THFModel`, `TBatchEncoding`). | `TTokenizer = Any` |
| P1-46 | oml/ddp/utils.py:23 | #1 | Default value is a `str` but the parameter is annotated `torch.device` — a straightforward type error; the sibling function at :53 gets it right with `Union[torch.device, str]`. | `def merge_list_of_dicts(list_of_dicts: List[Dict[str, Any]], device: torch.device = "cpu") -> Dict[str, Any]:` |
| P1-47 | oml/lightning/callbacks/metric.py:125 | #1 | Annotated `-> Exception` but the body unconditionally raises and never returns; callers at :86 and :115 rely on the raise, so the declared type is a lie a checker can catch. | `def _raise_computation_error(self) -> Exception:` |
| P1-48 | oml/losses/arcface.py:96 | #1 | Annotated `-> torch.Tensor`, returns `None`. | `def _log_accuracy_on_batch(self, logits: torch.Tensor, y: torch.Tensor) -> torch.Tensor:` |
| P1-49 | oml/functional/metrics.py:461 | #1 | Declared `-> List[Tensor]` but the success path returns a Tensor (`n_components / embeddings.shape[1]`, :514) — the doctest at :504 confirms `tensor([0.2000, 0.5000])`. Only the `except` fallback (:519) returns a list, so the two paths return different types. | `def calc_pcf(embeddings: Tensor, pcf_variance: Tuple[float, ...]) -> List[Tensor]:` |
| P1-50 | oml/utils/misc_torch.py:150 | #1 | Declared `-> bool`, returns a 0-d Tensor (`(diffs >= 0).all()`), which callers then use in `if not is_sorted_tensor(d)` (retrieval_results.py:64). | `return (diffs >= 0).all()` |
| P1-51 | oml/models/meta/siamese.py:105 | #1 | `pretrained_models: Dict[str, Any]` lets one shape of record coexist with another: this site unpacks the entry as a 3-tuple while every other extractor indexes it as a dict with `url`/`hash`/`fname` keys (vit_clip:146, projection.py:63, ecapa:62). The weak type is why the divergence is invisible. | `url_or_fid, hash_md5, fname = self.pretrained_models[weights]  # type: ignore` |
| P1-52 | oml/utils/io.py:103 | #1 | `-> str` with an explicit `return None` silenced by `# type: ignore`; the `attempt` counter at :93 also hand-rolls `enumerate`. | `return None  # type: ignore` |
| P1-53 | oml/utils/misc_torch.py:205 | #1 | A `@contextmanager` annotated with the yielded type instead of a generator type, so the decorated symbol's real signature is invisible to callers. | `def temporary_setting_model_mode(model: torch.nn.Module, set_train: bool) -> torch.nn.Module:` |
| P1-54 | oml/ddp/patching.py:83 | #1 | `__iter__` is a generator (it yields at :87) but is annotated `-> TAllSamplers`, i.e. it claims to return a sampler. | `def __iter__(self) -> TAllSamplers:` |
| P1-55 | oml/datasets/dataframe.py:23 | #1 | Parameter is annotated non-Optional but the first statement handles `None`, and all three call sites (:55, :90, :123) pass an `Optional[Dict[str, Any]]`. | `def update_extra_data(dataset: IBaseDataset, df: pd.DataFrame, extra_data: Dict[str, Any]) -> IBaseDataset:` |
| P1-56 | oml/datasets/dataframe.py:32 | #6 | `update_extra_data` writes to two objects it does not own — the caller's `extra_data` dict (:27, :30) and `dataset.extra_data` (:32) — then returns the dataset, so call sites read as functional (`dataset = update_extra_data(dataset, df, extra_data)`). | `dataset.extra_data.update(extra_data)` |
| P1-57 | oml/models/utils.py:9 | #6 | `remove_criterion_in_state_dict`, `remove_prefix_from_state_dict` (:34) and `filter_state_dict` (:52) all mutate the caller's `state_dict` in place and *also* return it; every call site is written `loaded = f(loaded)`, hiding that the original is destroyed. | `del state_dict["criterion.weight"]` ... `return state_dict` |
| P1-58 | oml/metrics/accumulation.py:104 | #6 | `update_data` inserts an internal bookkeeping key into the caller's `data_dict`; the caller (`EmbeddingMetrics.update`, embeddings.py:193) passes a freshly built dict so it is invisible today, but the write is outside the declared contract. | `data_dict[self._indices_key] = indices` |
| P1-59 | oml/utils/misc_torch.py:84 | #6 | `cat_two_sorted_tensors_and_keep_it_sorted` writes into its input `x1` in place; the name promises a concatenation. Same class of hidden write in `assign_2d` (:55), which mutates `x` and returns it. | `x1[need_scaling] = x1[need_scaling] * scale[need_scaling] - eps` |
| P1-60 | oml/datasets/audios.py:49 | #6 | A `parse_*` function writes back into the caller's DataFrame. | `df[START_TIME_COLUMN] = df[START_TIME_COLUMN].fillna(0.0)` |
| P1-61 | oml/utils/misc.py:166 | #6 | `compare_dicts_recursively` is declared `-> bool` but can only ever return `True`: every disagreement path raises `AssertionError`. Under `python -O` the asserts vanish and the function returns `True` unconditionally. | `assert d2[k] == v, f"Key name: {k}..."` / `return True` |
| P1-62 | oml/ddp/patching.py:113 | #24 | Attribute names are pulled out of `inspect.signature(DataLoader.__init__)` and applied with `getattr`, so no grep or whole-program pass can tell which loader attributes this reads; the set changes with the installed torch version. | `extracted[parameter] = getattr(loader, parameter)` |
| P1-63 | oml/lightning/modules/extractor.py:81 | #24 | Duck-typed probes for a protocol nobody declares: `criterion` is typed `Optional[nn.Module]`, which has neither `criterion_name` nor `last_logs` (:84). The convention is implemented by five loss classes but written down in no interface. | `loss_name = (getattr(self.criterion, "criterion_name", "") + "_loss").strip("_")` |
| P1-64 | oml/metrics/embeddings.py:203 | #24 | `getattr` probe for `top_n`, an attribute that `IRetrievalPostprocessor` does not declare (only `PairwiseReranker` has it, pairwise.py:47). The adjacent `# todo: refactor` admits it. | `top_n = getattr(self.postprocessor, "top_n", len(self.dataset.get_gallery_ids()))` |
| P1-65 | oml/losses/triplet.py:248 | #24 | Same probe against the miner: `ITripletsMiner` declares no `last_logs`. | `self._last_logs.update(getattr(self.miner, "last_logs", {}))` |
| P1-66 | oml/miners/pairs.py:24 | #20 | The same nontrivial lambda body appears on two adjacent lines; it deserves a name (`as_unordered_pair`). | `zip(*list(set(list(map(lambda x: tuple(sorted([x[0], x[1]])), zip(ii_a, ii_p))))))` |
| P1-67 | oml/miners/inbatch_nhard_tri.py:131 | #19 | Inside a loop over every anchor, two full-length boolean scans (`idx_anch_pos == idx_anch`, `idx_anch_neg == idx_anch`) rebuild a mask over the whole triplet list — quadratic in batch size where a single group-by would do. | `positives = hardest_positive[idx_anch_pos == idx_anch][self.positive_slice]` |
| P1-68 | oml/ddp/utils.py:30 | #19 | The reference key/type set `set((k, type(v)) for k, v in list_of_dicts[0].items())` is rebuilt on every iteration of the enclosing loop over `list_of_dicts` — loop-invariant work made quadratic. | `assert set((k, type(v)) for k, v in list_of_dicts[0].items()) == set(` |
| P1-69 | oml/utils/misc_torch.py:167 | #12 | `try: len(val); return True / except: return False` reimplements `isinstance(val, Sized)` (or `hasattr(val, "__len__")`) with a bare `except Exception`. | `try: len(val); return True` |
| P1-70 | oml/utils/misc_torch.py:306 | #12 | `{k: v for k, v in self.items()}` on a `MutableMapping` is `dict(self)`. Related: `__delitem__` at :291 calls `self.dict.__delitem__(key)` and returns its result instead of `del self.dict[key]`. | `return {k: v for k, v in self.items()}` |
| P1-71 | oml/losses/triplet.py:250 | #12 | The reduction switch is reimplemented inline even though `get_reduced` (imported at :7 and used at :81) is exactly this function, and the same `reduction` domain is validated by assert in three other places (:41, :134, :196). | `if self.reduction == "mean": loss = loss.mean()` |
| P1-72 | oml/models/audio/ecapa_tdnn/extractor.py:68 | #12 | Hand-rolled `dict.get` with a default, where the three sibling extractors write `state_dict.get("state_dict", state_dict)` (projection.py:69, siamese.py:109, vit_clip:157). | `state_dict = state_dict["state_dict"] if "state_dict" in state_dict else state_dict` |
| P1-73 | oml/losses/arcface.py:51 | #12 | `list()` inside `sorted()` is redundant — `sorted` takes any iterable. Repeated at category_balance.py:70, :82, distinct_category_balance.py:79, :110. | `mapper = {l: i for i, l in enumerate(sorted(list(set(label2category.values()))))}` |
| P1-74 | oml/models/vit_clip/extractor.py:139 | #8 | The input resolution is recovered by string-splitting the arch key rather than being declared beside the constructor; the same `weights: str` key type threads through `from_pretrained`, `pretrained_models`, and `get_transforms_for_pretrained` untyped. | `self.input_size = int(arch.split("_")[-1])` |
| P1-75 | oml/metrics/embeddings.py:166 | #8 | Metric names are bare strings duplicated across module boundaries: `"fnmr@fmr"` and `"pcf"` are produced in functional/metrics.py:456 and :101 and re-spelled here as an exclusion list; a rename in one home silently breaks the other. | `self.metrics_to_exclude_from_visualization = ["fnmr@fmr", "pcf", *metrics_to_exclude_from_visualization]` |
| P1-76 | oml/functional/metrics.py:1 | #29 | No module docstring anywhere in `oml/` (all 90 first-party modules): the largest, most-imported modules — functional/metrics.py (547 lines), utils/misc_torch.py (520), retrieval/retrieval_results.py (460) — open straight into imports. | `from collections import defaultdict` |
| P1-77 | oml/lightning/pipelines/parser.py:57 | none | `strategy` is set to the string `"auto"` in the non-DDP branch (:46), which is truthy, so `check_is_config_for_ddp` returns `True` for *every* config. `is_ddp` at train.py:79 / validate.py:34 is therefore always true and the non-DDP `trainer.fit(..., train_dataloaders=...)` branch is unreachable. | `return bool(cfg["strategy"])` |
| P1-78 | oml/datasets/audios.py:142 | none | Downmix averages the wrong axis: `torchaudio.load` returns `[channels, frames]` (confirmed by `_trim_or_pad` at :161 treating `shape[1]` as length), and the guard tests `shape[0] != 1`, but the mean is taken over `dim=1` — it averages over time, collapsing the audio to one sample per channel. | `if self._convert_to_mono and audio.shape[0] != 1: audio = audio.mean(dim=1, keepdim=True)` |
| P1-79 | oml/retrieval/retrieval_results.py:362 | none | `self.gt_ids[query_idx]` is indexed unguarded, but `gt_ids` is `Optional` and the *same method* guards it nine lines later at :371. Calling `visualize_as_html` on results without ground truth raises `TypeError`. | `color_as_label = GREEN if ret_idx in self.gt_ids[query_idx] else RED` |
| P1-80 | oml/samplers/category_balance.py:53 | none | The error path itself crashes: `param` is an `int`/`float` *value*, which has no `__name__`, so a non-int argument raises `AttributeError` instead of the intended `TypeError`. Duplicated at distinct_category_balance.py:84. | `raise TypeError(f"{param.__name__} must be int, {type(param)} given")` |
| P1-81 | oml/registry/models.py:46 | none | The guard runs *after* the model has already been constructed on the previous line, so the `ValueError` it raises can never prevent the double-weights situation it describes. The condition at :37 is also dead as written: `.get("weights", "")` returns `""`, never `None`, unless the key exists with an explicit `None`. | `model = registry[model_name](extractor=inside_extractor, **kwargs)` / `raise_if_needed(extractor_cfg, kwargs, model_name)` |
| P1-82 | oml/metrics/accumulation.py:153 | none | The dedupe loop overwrites `storage[self._indices_key]` while using it as the key source for every other entry. Correctness depends on `__element_indices` being iterated last, which holds only because `update_data` appends it last to `keys` (:103) and dicts preserve insertion order. | `for key, data in storage.items(): storage[key] = unique_by_ids(storage[self._indices_key], data)[1]` |
| P1-83 | oml/utils/misc_torch.py:204 | none | `temporary_setting_model_mode` has no `try/finally`, so any exception in the wrapped block leaves the model stuck in eval mode; `_inference` (inference/abstract.py:40) runs the whole inference loop inside it. The sibling context manager `matplotlib_backend` (misc.py:232) does use `try/finally`. | `model.train(set_train)` / `yield model` / `model.train(prev_mode)` |
| P1-84 | oml/functional/metrics.py:68 | none | The per-category comprehension iterates every *query's* category rather than the unique categories, so `take_unreduced_metrics_by_mask` runs `n_query` times instead of `n_categories` times and the dict is rebuilt on every duplicate. `calc_topological_metrics` (:104) gets this right with `np.unique`. | `metrics_cat = {c: take_unreduced_metrics_by_mask(metrics, query_categories == c) for c in query_categories}` |
| P1-85 | oml/retrieval/retrieval_results.py:63 | none | Validation order is inverted: sortedness is checked first, but `is_sorted_tensor` asserts `x.ndim == 1` internally (misc_torch.py:157), so a 2-D input dies on a bare `AssertionError` before reaching the friendly `RuntimeError` at :70 that was written for it. The bare `100` at :66 is also unrelated to the `100` defaults at :46 and :131. | `if not is_sorted_tensor(d): raise RuntimeError(f"Distances must be sorted: {d}.")` |
| P1-86 | oml/utils/io.py:143 | none | Downloaded-checkpoint verification uses `startswith` against the expected md5, so a truncated hash passes — `ExtractorWithMLP.pretrained_models` (projection.py:26) ships an 8-character `"hash"`. The sibling validator `check_exists_and_validate_md5` (:64) uses `==`, so "hash matches" means two different things in one module. | `if not calc_hash(save_path).startswith(hash_md5):` |
| P1-87 | oml/utils/misc.py:208 | none | `np.resize` is not an image resize — it tiles or truncates the flat buffer. Any figure whose canvas is not already 256x256 is silently garbled rather than rescaled. | `image = np.resize(image, (256, 256, 3))` |
| P1-88 | oml/utils/misc.py:241 | none | `remove_unused_kwargs` is absent from `__all__` yet imported cross-module by registry/losses.py:9, registry/samplers.py:7 and metrics/embeddings.py:31; `TCfg` is re-exported implicitly here and imported from *both* `oml.utils.misc` (losses.py:9) and `oml.const` (loggers.py:4). Same omission at pipelines/parser.py:107, which drops `parse_logger_from_config` despite train.py:17 importing it. | `__all__ = ["CompatibilityError", "adapt_argument_as_kwarg", "find_value_ids", ...]` |
| P1-89 | oml/miners/pairs.py:21 | none | Reaches through the public API into another object's private method; `ITripletsMinerInBatch.sample` (interfaces/miners.py:88) is the supported entry and is what applies `_check_input_labels`, which this call skips. | `ii_a, ii_p, ii_n = self._miner._sample(features, labels=labels)` |
| P1-90 | oml/lightning/callbacks/metric.py:82 | none | The callback reads the batch index with the global constant `INDEX_KEY` instead of the dataset's configurable `index_key`, so any dataset constructed with a non-default `index_key` silently breaks metric accumulation. | `self.metric.update_data(data=outputs, indices=outputs[INDEX_KEY])` |
| P1-91 | oml/models/utils.py:18 | none | Name and assert message both say "starting with", but the predicate is a substring test, so a key containing the trial key mid-string yields a wrong prefix and the second assert (:27) then fails with a confusing message. | `keys_starting_with_trial_key = [k for k in state_dict.keys() if trial_key in k]` |
| P1-92 | oml/registry/transforms.py:112 | none | A bare `except Exception` around the whole transform-serialisation body turns every failure — including bugs in `adapt_argument_as_kwarg` — into a `print`, so a silently missing transform log looks identical to an unserialisable transform. Same swallow-and-continue at functional/metrics.py:515 and misc_torch.py:171. | `except Exception: print(f"We are not able to interpret {key} as albumentations transforms and log them as a file.")` |
| P1-93 | oml/inference/abstract.py:100 | none | `torch.load(..., weights_only=False)` on a user-supplied cache path executes arbitrary pickle payloads. Repeated at projection.py:68, siamese.py:108, vit_clip/extractor.py:156, ecapa/extractor.py:67, resnet/extractor.py:217 — every checkpoint path in the library. | `outputs = torch.load(cache_path, map_location="cpu", weights_only=False)` |
| P1-94 | oml/registry/__init__.py:24 | none | Behaviour is selected by comparing the *display label* of a registry row, so renaming the printed heading changes which attribute is read; the two loop bodies are otherwise identical. | `if name == "Augmentations":` |

## Phase 2 — audit finding verdicts

532 findings judged. Grouped where a rule fires many near-identical times;
exceptions split out so every finding is accounted for. Counts in the `why`.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| all 55 sites (e.g. oml/interfaces/retrieval.py:12, oml/interfaces/metrics.py:22, oml/datasets/images.py:288, oml/losses/triplet.py:86) | #1 | heuristic | real | x55 — every site is a genuine `Any`/bare-dict/opaque-`**kwargs` on a public signature; the batch/interface ones are OML's own contract typed as `Any`. |
| 19 of 21 (e.g. oml/samplers/category_balance.py:52, oml/retrieval/retrieval_results.py:249, oml/utils/misc_torch.py:196, oml/datasets/images.py:50) | #2 | proved | real | x19 — genuine locally-redundant checks: within-function narrowing re-tests and isinstance/`is None` provable from accurate annotations. |
| oml/miners/cross_batch.py:77 | #2 | proved | fp | `n is not None` is "redundant" only because `n: int = None` is mis-annotated; the default is `None` and the guard is load-bearing. |
| oml/datasets/dataframe.py:24 | #2 | proved | fp | `extra_data is None` flagged redundant from a non-Optional annotation that all three callers actually violate by passing `None`; guard is real. |
| oml/metrics/accumulation.py:41 | #4 | indexed | real | `isinstance(num_samples, int)` on a param already typed `int` — genuine redundant check. |
| all 14 (e.g. oml/registry/losses.py:32, oml/utils/download_mock_dataset.py:86, oml/const.py:10, tests/test_oml/test_ddp/utils.py:22) | #6 | indexed | real | x14 — `get_*`/accessor-named functions with real effects: cfg mutation, network download, file I/O, env reads. |
| oml/utils/misc_torch.py:150 | #7 | heuristic | fp | `is_sorted_tensor` docstring narrates a behavioral assumption ("assume sorted by default"), not a caller-must-call-X protocol; no receipt/precondition to lift. |
| all 3 (oml/datasets/audios.py:64/65/255) | #8 | indexed | real | `input_tensors_key`/`index_key`/`labels_key: str` recur in 16/13/11 signatures — genuine NewType candidates. |
| all 6 (oml/retrieval/retrieval_results.py:330, oml/utils/misc.py:166 x2, tests x3) | #10 | indexed | real | x6 — concrete container demanded where the body uses only iteration/`.items`/`.keys`; a protocol suffices. |
| all 58 (e.g. oml/datasets/dataframe.py:70, oml/datasets/texts.py:100, oml/registry/optimizers.py:25, oml/lightning/modules/extractor.py:99, pipelines/.../train_cars.py:21) | #11 | indexed | real | x58 — every cited group is a true AST clone (registry `*_by_cfg`, dataset `__init__`/`__getitem__`, lightning module twins, converter argparsers, augmentation builders). |
| 25 of 26 (e.g. oml/datasets/images.py:204, oml/interfaces/models.py:14, oml/models/meta/siamese.py:54, oml/lightning/callbacks/metric.py:140) | #13 | indexed | real | x25 — bodies are a single forwarding call adding nothing (visualize forwarders, `extract`->`forward`, `predict`->`forward`, hook->`_check_loaders`). |
| oml/losses/triplet.py:227 | #13 | indexed | fp | `avg_d` is not a pure forward — it adds `.clone().detach()` on both args and `.mean()` on the result. |
| all 48 (e.g. oml/datasets/audios.py:59, oml/functional/metrics.py:17, oml/losses/triplet.py:32, oml/samplers/balance.py:45) | #14 | indexed | real | x48 — genuine recurring parameter groups wanting a type (dataset config trio, metric top-k trio, sampler params). Note: the many overlapping subsets on audios.py:248 over-report one underlying clump. |
| 10 of 12 (e.g. oml/metrics/embeddings.py:34, oml/lightning/callbacks/metric.py:135, oml/utils/misc.py:166) | #15 | heuristic/indexed | real | x10 — rich object passed, only <=2 attributes used (`rr`->gt_ids/retrieved_ids, `trainer`->val_dataloaders, `pl_module`->current_epoch/logger). |
| oml/miners/miner_with_bank.py:65, oml/miners/inbatch_hard_tri.py:44 | #15 | heuristic | fp | x2 — "uses only `.clone`/`.detach`/`.shape`" of a Tensor: the whole tensor is needed; a {clone,shape} protocol is not a real narrowing. |
| oml/utils/download_mock_dataset.py:261 | #17 | heuristic | real | Long function with a live-variable neck — a legitimate split-point report. |
| all 4 (tests/test_outdated_docs.py:19, tests/.../test_arcface.py:68, tests/.../test_models_creation.py:38/63) | #18 | heuristic | real | x4 — functions that narrate >=2 labeled phases in comments; each phase is a function boundary spelled in prose. |
| oml/utils/misc.py:35 | #19 | heuristic | real | `name in parameters` (a list) inside a `for` loop over candidates — O(n*m) lexical scan. |
| all 4 (oml/miners/pairs.py:24, oml/lightning/pipelines/logging.py:114, tests, pipelines/.../convert_cars.py:30) | #20 | heuristic | real | x4 — same nontrivial lambda body repeated >=2x in a module; each wants a name. |
| oml/datasets/audios.py:54 | #21 | heuristic | real | `self.get_audio(item)` recurs across 3 methods of `AudioBaseDataset` — a class-scoped repeated expression. |
| 21 of 75 (e.g. oml/utils/misc_torch.py:496, oml/losses/arcface.py:67, oml/metrics/accumulation.py:125, oml/retrieval/retrieval_results.py:109) | #22 | heuristic | real | x21 — genuine helper methods (private calc/plot helpers, `is_empty`, `n_retrieved_items`, `fc`) that touch only the public surface. |
| 54 of 75 (e.g. oml/models/*/extractor.py forward/feat_dim, lightning training_step/validation_step/configure_optimizers, logger log_*, IRetrievalPostprocessor.process) | #22 | heuristic | fp | x54 — polymorphic overrides, abstract-property implementations, framework hooks and interface methods: bound to be members by a base contract, so the "could be a free function" ideal does not apply. |
| oml/ddp/patching.py:112, oml/ddp/patching.py:113, tests/test_imports.py:115 | #24 | heuristic | real | x3 — attribute/module names constructed at runtime (`hasattr`/`getattr`/`import_module`), unfindable by grep. |
| 36 of 39 (pipelines main_hydra->pipeline x16, oml/miners/inbatch_all_tri.py:32, oml/lightning/callbacks/metric.py:140/143, oml/models/vit_dino/extractor.py:234, tests) | #25 | indexed | real | x36 — thin delegations whose caller/callee names share no token stem; the call chain is un-greppable. |
| oml/utils/misc_torch.py:388, oml/losses/arcface.py:70, tests/.../test_datasets.py:50 | #25 | indexed | fp | x3 — idiomatic `__init__`->`_fit`, dunder `__call__` delegation, and `smooth_labels`->`label_smoothing` (which do share the smooth/label tokens). |
| oml/const.py:27 | #26 | heuristic | real | `TMP_PATH` assembled by executing `tempfile.gettempdir()` — a reader must run it to know the value. |
| all 25 (e.g. oml/utils/misc_torch.py:114, oml/functional/metrics.py:17, oml/retrieval/retrieval_results.py:45) | #27 | indexed | real | x25 — genuine high-fan-in symbols living in 415–547-line grab-bag modules; reading one pays the whole file. |
| CONTRIBUTING.md:39, CONTRIBUTING.md:40 | #28 | indexed | real | x2 — specific repo paths named in docs that do not resolve. |
| oml/utils/download_mock_dataset.py:35 | #28 | indexed | fp | `oml.daloroserver.com` is an external URL host, not a repo path/symbol; integrity != network reachability. |
| pipelines/README.md:39/53/66/128 | #28 | indexed | fp | x4 — `pipeline.py`/`registry.py`/`config.yaml` are inline tutorial-example filenames in an "oversimplified example", not pointers to repo files. |
| heavy entry points x10 (e.g. oml/metrics/embeddings.py compute_metrics, oml/datasets/images.py get_retrieval_images_datasets, pipelines converters build_*_df) | #29 | heuristic | real | x10 — genuinely heavy functions (full knn, CSV+image scan, network download) with no cost-declaring docstring. |
| top-loading, big prod modules x31 (e.g. oml/functional/metrics.py:1 547L, oml/utils/misc_torch.py:1 520L, oml/retrieval/retrieval_results.py:1 460L) | #29 | heuristic | real | x31 — substantial modules opening straight into imports with no orientation; a real top-loading gap. |
| top-loading, small prod modules x28 (e.g. oml/transforms/images/albumentations.py:1 37L, oml/registry/schedulers.py:1 47L, oml/lightning/pipelines/validate.py:1 89L) | #29 | heuristic | fp | x28 — flagging <150-line modules for a missing module docstring is doc-presence scoring, the research's own anti-rule. |
| top-loading, test/pipeline modules x50 (e.g. tests/test_oml/*, pipelines/.../convert_*.py) | #29 | heuristic | fp | x50 — module docstrings on self-describing test/script files are a doc-presence ask with no navigation payoff. |
| 6 of 7 (oml/models/vit_dino/extractor.py:262, oml/models/audio/ecapa_tdnn/extractor.py:76, oml/ddp/patching.py:148, oml/models/resnet/extractor.py:164, oml/models/vit_dino/extractor.py:215, oml/utils/images/images.py:114) | #30 | heuristic | real | x6 — genuine Demeter chains reaching 3–4 hops into a parameter/attribute's internals. |
| oml/lightning/callbacks/metric.py:127 | #30 | heuristic | fp | `self.metric.__class__.__name__` is idiomatic type introspection, not a train-of-dots reach into structure. |

## Phase 3 — reconciliation

Every phase-1 site classified. `covered` where a finding matches site+rule
(±3 lines); `threshold-miss` where the rule fired on the same clone group or
under a cutoff but not my exact line; `detector-miss` where the rule shipped
and should have fired here (many under-fired: #9 and #12 produced zero
findings; several #1/#13 sites are concrete-but-wrong return types the AST
check does not model); `inventory-gap` where I mapped `none`.

| P1 id | rule | class | note |
|-------|------|-------|------|
| P1-1 | #1 | covered | exact — IRetrievalPostprocessor.process flagged |
| P1-2 | #1 | detector-miss | site is a call, not a signature; #1 only reads signatures |
| P1-3 | #1 | detector-miss | IBaseDataset.__getitem__ Dict[str,Any] not flagged (fired elsewhere) |
| P1-4 | #1 | covered | exact — IBasicMetric.setup |
| P1-5 | #7 | detector-miss | #7 fired once (misc_torch:150) but missed this call-order protocol |
| P1-6 | #1 | detector-miss | `**kwargs: Dict[str,Any]` registry sites not flagged as opaque-kwargs |
| P1-7 | #9 | detector-miss | #9 produced zero findings repo-wide |
| P1-8 | #9 | detector-miss | #9 never fired |
| P1-9 | #9 | detector-miss | #9 never fired — mutable-default miner missed |
| P1-10 | #26 | detector-miss | typo'd `__all_` / nonexistent name not caught |
| P1-11 | #26 | covered | exact — CACHE_PATH/computed const region |
| P1-12 | #26 | detector-miss | TRANSFORMS_FOR_PRETRAINED computed table not flagged |
| P1-13 | #26 | detector-miss | dict-splat registry not flagged |
| P1-14 | #28 | detector-miss | docstring param `x0` — #28 checks paths, not param-name integrity |
| P1-15 | #28 | detector-miss | docstring symbol `explained_variance_ths` not caught |
| P1-16 | #2 | detector-miss | `len(x)==2` literal comparison not modeled by pyright unnecessary-check |
| P1-17 | #2 | detector-miss | half-dead `smoothing_epsilon is None` not flagged |
| P1-18 | #2 | covered | exact — retrieval_results isinstance |
| P1-19 | #2 | covered | exact — misc_torch:196 TData isinstance |
| P1-20 | #2 | covered | exact — images.py:50 `coord is None` |
| P1-21 | #2 | detector-miss | dead `spec_repr_func or ...` fallback not flagged |
| P1-22 | #2 | detector-miss | `isinstance(convert_to_mono, bool)` not flagged |
| P1-23 | #11 | detector-miss | calc_cmc/precision/map triple-clone not grouped |
| P1-24 | #11 | detector-miss | reduce/take_unreduced walk pair not grouped |
| P1-25 | #11 | threshold-miss | dataframe clone group fired at :70/:140, not this __init__ line |
| P1-26 | #11 | threshold-miss | texts __init__ clone fired at :100, sibling line |
| P1-27 | #11 | threshold-miss | audios __init__ clone fired at :248, sibling line |
| P1-28 | #11 | detector-miss | vit_clip 4-constructor clone not grouped |
| P1-29 | #11 | detector-miss | checkpoint-load 5-way clone not grouped |
| P1-30 | #11 | detector-miss | patch_float/patch_device clone not grouped |
| P1-31 | #11 | detector-miss | sampler validation-block clone not grouped |
| P1-32 | #11 | detector-miss | same-function recomputed category2labels not caught |
| P1-33 | #11 | covered | exact — lightning configure_optimizers twin |
| P1-34 | #11 | threshold-miss | on_epoch_start drifted twin — fired nearby in same module |
| P1-35 | #11 | detector-miss | visualize_qg/visualize 45-line twin not grouped |
| P1-36 | #11 | detector-miss | thresholding process twin not grouped |
| P1-37 | #11 | detector-miss | naive-triplets second implementation not caught |
| P1-38 | #13 | covered | exact — IExtractor.extract |
| P1-39 | #13 | covered | exact — visualize forwarders |
| P1-40 | #13 | detector-miss | last_logs forwarding property not flagged (#13 fired at :227) |
| P1-41 | #13 | detector-miss | get_extractor/get_pairwise_model forwarders not flagged |
| P1-42 | #15 | covered | exact — calc_retrieval_metrics_rr wallet param |
| P1-43 | #10 | detector-miss | narrower-than-wrapped `albu.Compose` not flagged (#10 fired elsewhere) |
| P1-44 | #1 | covered | exact — get_retrieval_images_datasets transforms:Any |
| P1-45 | #1 | detector-miss | `TTokenizer = Any` alias not flagged |
| P1-46 | #1 | covered | exact — merge_list_of_dicts |
| P1-47 | #1 | detector-miss | `-> Exception` concrete-wrong return outside #1 scope |
| P1-48 | #1 | detector-miss | `-> Tensor` returns None — concrete-wrong return outside #1 scope |
| P1-49 | #1 | detector-miss | calc_pcf `-> List[Tensor]` mismatch not modeled |
| P1-50 | #1 | detector-miss | is_sorted_tensor `-> bool` returns Tensor not modeled |
| P1-51 | #1 | detector-miss | siamese 3-tuple vs dict pretrained record not flagged |
| P1-52 | #1 | detector-miss | `-> str` returns None (io.py) not modeled |
| P1-53 | #1 | detector-miss | contextmanager yield-type annotation not flagged |
| P1-54 | #1 | detector-miss | `__iter__ -> TAllSamplers` generator mis-annotation not modeled |
| P1-55 | #1 | detector-miss | non-Optional param taking None not flagged by #1 (it is by #2, as fp) |
| P1-56 | #6 | detector-miss | update_extra_data hidden dual mutation not caught |
| P1-57 | #6 | detector-miss | state_dict in-place mutators not caught |
| P1-58 | #6 | detector-miss | update_data writes caller dict — not caught |
| P1-59 | #6 | detector-miss | cat_two_sorted in-place x1 write not caught |
| P1-60 | #6 | detector-miss | parse_start_times DataFrame write not caught |
| P1-61 | #6 | detector-miss | compare_dicts_recursively assert-only `-> bool` not caught |
| P1-62 | #24 | covered | exact — ddp/patching getattr |
| P1-63 | #24 | detector-miss | criterion_name duck-probe not flagged |
| P1-64 | #24 | detector-miss | postprocessor top_n getattr probe not flagged |
| P1-65 | #24 | detector-miss | miner last_logs getattr probe not flagged |
| P1-66 | #20 | covered | exact — pairs.py:24 repeated lambda |
| P1-67 | #19 | detector-miss | nhard per-anchor full mask scan not flagged |
| P1-68 | #19 | detector-miss | ddp loop-invariant set rebuild not flagged |
| P1-69 | #12 | detector-miss | #12 produced zero findings repo-wide |
| P1-70 | #12 | detector-miss | #12 never fired |
| P1-71 | #12 | detector-miss | #12 never fired |
| P1-72 | #12 | detector-miss | #12 never fired |
| P1-73 | #12 | detector-miss | #12 never fired |
| P1-74 | #8 | detector-miss | arch string-split obsession not caught (#8 fired on key params only) |
| P1-75 | #8 | detector-miss | duplicated metric-name strings not caught |
| P1-76 | #29 | covered | exact — no module docstring, big module |
| P1-77 | none | inventory-gap | always-True strategy -> dead non-DDP branch; no rule models it |
| P1-78 | none | inventory-gap | wrong-axis downmix correctness bug; no rule |
| P1-79 | none | inventory-gap | unguarded Optional gt_ids index; no rule |
| P1-80 | none | inventory-gap | `param.__name__` AttributeError in error path; #2 fired at :52 on a different concern |
| P1-81 | none | inventory-gap | guard runs after construction; no rule |
| P1-82 | none | inventory-gap | order-dependent dedupe overwrite; no rule |
| P1-83 | none | inventory-gap | missing try/finally in ctx manager; #27 fired at :205 unrelated |
| P1-84 | none | inventory-gap | O(n_query) category loop; no rule |
| P1-85 | none | inventory-gap | inverted validation order / bare AssertionError; no rule |
| P1-86 | none | inventory-gap | md5 `startswith` weak verification; no rule |
| P1-87 | none | inventory-gap | np.resize misuse for image resize; no rule |
| P1-88 | none | inventory-gap | __all__ omissions / dual import homes; no rule (adjacent to #26) |
| P1-89 | none | inventory-gap | reaches into `_sample` private; #20 fired at :24 unrelated |
| P1-90 | none | inventory-gap | global INDEX_KEY vs configurable index_key; no rule |
| P1-91 | none | inventory-gap | substring vs prefix name/assert mismatch; no rule |
| P1-92 | none | inventory-gap | bare except swallowing bugs; no rule |
| P1-93 | none | inventory-gap | weights_only=False pickle exec; no rule (security) |
| P1-94 | none | inventory-gap | behaviour keyed on display label; no rule |
