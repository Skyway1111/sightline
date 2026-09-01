# Hierarchical-Localization — wave 1

Repo: `<GAUNTLET_CORPUS_ROOT>\Hierarchical-Localization`
Prod tree judged: `hloc/**` (all 62 .py) + `setup.py`. `third_party/*` are empty
submodule mounts (vendored, excluded). Notebooks not judged as code.
Judged blind: no sightline output read or run.

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | hloc/__init__.py:27 | none | `version = version.parse(...)` rebinds the module-level `packaging.version` import to a `Version` instance; after this block `hloc.version` is no longer the module, and any later `version.parse` in this file is an AttributeError. | `version = version.parse(found_version)` |
| P1-2 | setup.py:4 | none | `description` is a one-element list where setuptools requires a str; the published metadata carries the repr of a list. | `description = ['Tools and baselines for visual localization and mapping']` |
| P1-3 | setup.py:9 | #26 | The package version is recovered by string-splitting `hloc/__init__.py` and `eval`-ing the fragment instead of being read as a literal declaration. | `version = eval(f.read().split('__version__ = ')[1].split()[0])` |
| P1-4 | hloc/extract_features.py:21 | #29 | The block documenting `confs` is a bare string expression placed after the imports, so it is a no-op statement, not the module docstring. The module (and match_features.py:17, same pattern) therefore has no docstring at all. | `"""` / `A set of standard configurations that can be directly selected from the command` |
| P1-5 | hloc/extract_features.py:154 | #24 | The OpenCV/PIL interpolation constant is assembled at runtime from a config string, so `cv2.INTER_AREA` etc. are unreachable by grep. Same shape at :160 for `PIL.Image`. | `interp = getattr(cv2, "INTER_" + interp[len("cv2_") :].upper())` |
| P1-6 | hloc/extract_features.py:233 | #1 | `conf: Dict` on the package's primary public entry point: the caller must read the body to learn that `"preprocessing"` and `"model"`/`"name"` and `"output"` are all required keys. Same at match_features.py:156, :211, match_dense.py:233, :292, :335, :539, triangulation.py (via `Dict[str, Any]`). | `def main(` / `    conf: Dict,` |
| P1-7 | hloc/extract_features.py:283 | #2 | `dt != np.float16` is implied by `dt == np.float32`; the conjunct can never falsify the guard. | `if (dt == np.float32) and (dt != np.float16):` |
| P1-8 | hloc/extract_features.py:301 | none | In the OSError handler `grp` is unbound whenever `fd.create_group` itself raised (the exact disk-full case the handler is written for), so the recovery path raises NameError and hides the real error. | `del grp, fd[name]` |
| P1-9 | hloc/extract_features.py:170 | #9 | `default_conf` is a mutable dict held at class level and shared by every instance/subclass; nothing copies it (contrast base_model.py:17, which copies `required_inputs` but not `default_conf`). Same at match_dense.py:165. | `default_conf = {` |
| P1-10 | hloc/extract_features.py:253 | #30 | `main` reaches through the dataset object and rewrites its internal `names` list from outside the class, so `ImageDataset` cannot maintain any invariant about it. | `dataset.names = [n for n in dataset.names if n not in skip_names]` |
| P1-11 | hloc/match_features.py:112 | #13 | `WorkQueue.put` is a forward-only wrapper over `self.queue.put` that adds nothing; callers pay a hop for no meaning. | `def put(self, data):` / `    self.queue.put(data)` |
| P1-12 | hloc/match_features.py:217 | #33 | `match_from_paths` is annotated `-> Path` but has a bare `return` at :234 and falls off the end at :255; it never returns a Path on any path. | `) -> Path:` |
| P1-13 | hloc/match_features.py:100 | #32 | The loop variable `thread` is never used — the loop exists only to run N times, which the code does not say. | `for thread in self.threads:` / `    self.queue.put(None)` |
| P1-14 | hloc/match_features.py:186 | #1 | Two boundary-type lies in one signature: `match_path: Path = None` is an implicit Optional, and `List[Tuple[str]]` declares 1-tuples while every caller passes 2-tuples (`for i, j in pairs_all` at :189). | `def find_unique_new_pairs(pairs_all: List[Tuple[str]], match_path: Path = None):` |
| P1-15 | hloc/match_dense.py:23 | #34 | 14 lines of commented-out example invocations sit at the top of the module, ahead of `confs`. | `# Default usage:` / `# dense_conf = confs['loftr']` / `# features, matches = main(dense_conf, pairs, images, export_dir=outputs)` |
| P1-16 | hloc/match_dense.py:478 | #9 | Mutable default `= []` that is actually mutated: `feature_paths_refs.append(feature_path_q)` at :500 appends into the shared default object, so a second call in the same process starts with the previous run's paths. | `feature_paths_refs: Optional[List[Path]] = [],` |
| P1-17 | hloc/match_dense.py:237 | #9 | Mutable default argument on `match_dense`. | `existing_refs: Optional[List] = [],` |
| P1-18 | hloc/match_dense.py:341 | #9 | Two mutable `defaultdict(list)` defaults that the body writes into (`cpdict[name] = ...` at :413, `del bindict[name]` at :431). | `cpdict: Dict[str, Iterable] = defaultdict(list),` / `bindict: Dict[str, List[Counter]] = defaultdict(list),` |
| P1-19 | hloc/match_dense.py:548 | #33 | `main` is annotated `-> Path` but returns a 2-tuple at :585, which is what its callers unpack (pipeline_loftr.py:41). | `) -> Path:` ... `return features_q, matches` |
| P1-20 | hloc/match_dense.py:481 | #33 | `match_and_assign` is annotated `-> Path`; it bare-`return`s at :507 and falls off the end at :534. | `) -> Path:` |
| P1-21 | hloc/match_dense.py:124 | #33 | `get_unique_matches` returns a bare list `[0]` on the 1-D branch and a `(match_ids, scores)` tuple otherwise; its only caller unpacks two values at :154, so the first branch is a ValueError. | `if len(match_ids.shape) == 1:` / `    return [0]` |
| P1-22 | hloc/match_dense.py:449 | #37 | The `keypoints: Union[List[Path], Dict[...]]` list arm is never exercised by any prod caller (the only call, :534, passes `cpdict`), and it is provably broken: `load_keypoints` has no `kpts_as_bin` parameter (def at :292) and returns a 2-tuple, not a dict. | `keypoints = load_keypoints({}, keypoints, kpts_as_bin=set([]))` |
| P1-23 | hloc/match_dense.py:104 | #6 | `assign_keypoints` reads as a query but mutates two caller-owned lists in place (`other_cpts.append`, `ref_bins.append`), which is how `cpdict`/`bindict` get filled at :322. The effect is nowhere in the signature or a docstring. | `other_cpts.append(cpt)` / `if ref_bins is not None:` / `    ref_bins.append(Counter())` |
| P1-24 | hloc/match_dense.py:57 | none | The `loftr_superpoint` config declares the same `"output"` string as `loftr_aachen` (:49), so the two configs collide on one match file. | `"loftr_superpoint": {` / `    "output": "matches-loftr_aachen",` |
| P1-25 | hloc/match_dense.py:345 | #41 | `sum(pairs, ())` flattens by repeated tuple concatenation — quadratic in the number of pairs, on the aggregation path that runs over the whole pair set. The same file already uses the linear idiom at :488 (`chain.from_iterable`). Same shape at :177 and :450. | `required_queries = set(sum(pairs, ()))` |
| P1-26 | hloc/match_dense.py:258 | #19 | Membership test against `existing_refs` inside the per-pair dense-matching loop, where the parameter is declared `Optional[List]` (:237) — linear per iteration by the declared contract. | `if name0 in existing_refs:` |
| P1-27 | hloc/match_dense.py:350 | #12 | List comprehension fed to `set()` where a set comprehension is the idiom; also `set([])` at :449 and `[1.0 for _ in range(kps.shape[0])]` at :320 (`[1.0] * n`). | `required_queries -= set([k for k, v in cpdict.items() if isinstance(v, np.ndarray)])` |
| P1-28 | hloc/match_dense.py:554 | none | A `str` is assigned to a parameter annotated `Optional[Path]`, and the very next branch (:556) dispatches on `isinstance(features, Path)` — the annotation and the control flow contradict each other. | `if features is None:` / `    features = "feats_"` |
| P1-29 | hloc/localize_sfm.py:80 | #1 | Opaque `**kwargs` in a public signature, forwarded at :110 into `QueryLocalizer.localize`, which declares no `**kwargs` (:58) — so any keyword a caller supplies is a TypeError. No caller supplies one. | `**kwargs,` ... `ret = localizer.localize(kpq, mkp_idxs, mp3d_ids, query_camera, **kwargs)` |
| P1-30 | hloc/localize_sfm.py:104 | #19 | Linear `not in` against a Python list inside the per-match loop of the hot localization path; `kp_idx_to_3D` is a `defaultdict(list)` that grows with matches. | `if id_3D not in kp_idx_to_3D[idx]:` / `    kp_idx_to_3D[idx].append(id_3D)` |
| P1-31 | hloc/localize_sfm.py:45 | #41 | `set(frame_ids)` is rebuilt from the same loop-invariant list on every BFS iteration of the covisibility clustering. | `connected_frames &= set(frame_ids)` |
| P1-32 | hloc/localize_sfm.py:123 | #39 | History narration in a comment ("anymore") explaining a past removal that git already records. | `"points3D_xyz": None,  # we don't log xyz anymore because of file size` |
| P1-33 | hloc/localize_sfm.py:140 | #1 | Bare `Dict` plus implicit Optional; the nested shape actually required is only discoverable from :154. | `config: Dict = None,` |
| P1-34 | hloc/localize_inloc.py:131 | none | Two HDF5 handles are opened without a context manager and never closed, unlike every other h5py site in the repo. | `feature_file = h5py.File(features, "r", libver="latest")` / `match_file = h5py.File(matches, "r", libver="latest")` |
| P1-35 | hloc/localize_inloc.py:24 | none | A local alias is bound for `grid_sample`, used on the next line, then not used on the line after for the identical call — one of the two spellings is noise. | `grid_sample = torch.nn.functional.grid_sample` ... `interp_nn = torch.nn.functional.grid_sample(` |
| P1-36 | hloc/localize_inloc.py:103 | none | `np.concatenate` on a list that is empty whenever every retrieved pair was skipped by the `skip` filter at :87; the failure surfaces as an opaque numpy error. | `all_mkpq = np.concatenate(all_mkpq, 0)` |
| P1-37 | hloc/localize_inloc.py:18 | #11 | `interpolate_scan` and `pipelines/7Scenes/create_gt_sfm.py:28 interpolate_depth` are near-verbatim clones — same alias, same two-line comment, same bilinear/nearest/where/valid body — differing only in `.flatten()`. | `interp_lin = grid_sample(scan, kp, align_corners=True, mode="bilinear")[0, :, 0]` |
| P1-38 | hloc/utils/io.py:37 | #33 | Declared `-> np.ndarray`, but returns `(p, uncertainty)` when `return_uncertainty=True` — which is exactly how triangulation.py:118 and :129 call it. | `) -> np.ndarray:` ... `if return_uncertainty:` / `    return p, uncertainty` |
| P1-39 | hloc/utils/io.py:69 | #33 | `Tuple[np.ndarray]` declares a one-element tuple; the body returns two. | `def get_matches(path: Path, name0: str, name1: str) -> Tuple[np.ndarray]:` ... `return matches, scores` |
| P1-40 | hloc/utils/parsers.py:58 | #13 | Body is a single forwarding call with one changed default. | `def names_to_pair_old(name0, name1):` / `    return names_to_pair(name0, name1, separator="_")` |
| P1-41 | hloc/utils/parsers.py:29 | none | `assert` used to validate parsed file content in library code — stripped under `python -O`, after which the function silently returns an empty list. Same pattern at localize_sfm.py:142-144, triangulation.py:203-206, match_features.py:228. | `assert len(images) > 0` |
| P1-42 | hloc/utils/parsers.py:8 | #11 | `logger = logging.getLogger(__name__)` is copy-pasted into 7 modules (also read_write_model.py:40, extractors/netvlad.py:13, pipelines/4Seasons/utils.py:17, 7Scenes/utils.py:7, Cambridge/utils.py:16, RobotCar/colmap_from_nvm.py:22) while the rest of the repo imports the one configured logger from `hloc/__init__.py:14`. | `logger = logging.getLogger(__name__)` |
| P1-43 | hloc/utils/base_model.py:38 | #24 | The whole extractor/matcher plugin layer is resolved by a runtime-built module path, so there is no static edge from `confs[...]["model"]["name"]` to any file in `hloc/extractors/`; an agent cannot grep from config to implementation. | `module_path = f"{root.__name__}.{model}"` / `module = __import__(module_path, fromlist=[""])` |
| P1-44 | hloc/utils/base_model.py:48 | #34 | Commented-out alternative implementation left as the last line of the function. | `# return getattr(module, 'Model')` |
| P1-45 | hloc/utils/base_model.py:10 | #9 | Mutable class attributes shared by every subclass; `required_inputs` is defensively copied at :17 but `default_conf` is not, so `self.default_conf` aliases the class object. | `default_conf = {}` / `required_inputs = []` |
| P1-46 | hloc/utils/geometry.py:11 | #41 | `to_homogeneous(p2d_i)` is recomputed at :13 (a full `np.pad` allocation) inside the epipolar-error routine that triangulation.py runs once per image pair. | `l2d_j = to_homogeneous(p2d_i) @ j_E_i.T` ... `dist = np.abs(np.sum(to_homogeneous(p2d_i) * l2d_i, axis=1))` |
| P1-47 | hloc/triangulation.py:252 | #24 | `eval` on a CLI-supplied `key=value` fragment, with `hasattr`/`getattr` on the runtime-supplied key at :247/:253 — arbitrary execution and zero greppability for the option names. | `value = eval(value)` |
| P1-48 | hloc/triangulation.py:277 | none | `--mapper_options` is never registered on the parser (:263-274), so `args.pop("mapper_options")` raises KeyError: the module's `__main__` path cannot run at all. | `mapper_options = parse_option_args(` / `    args.pop("mapper_options"), pycolmap.IncrementalMapperOptions()` |
| P1-49 | hloc/triangulation.py:265 | #25 | The CLI flag is `--reference_sfm_model` but `main` (:192) takes `reference_model`; `main(**args, ...)` at :281 is an unexpected-keyword TypeError. The delegation edge is broken by a name that does not match. | `parser.add_argument("--reference_sfm_model", type=Path, required=True)` |
| P1-50 | hloc/triangulation.py:136 | #41 | A full HDF5 open + read of the pair's matches happens *before* the `matched` dedup check at :138 that throws the work away; hoisting the check above :126 skips the whole inner body for every reversed pair. | `matches = get_matches(matches_path, name0, name1)[0]` / `if len({(id0, id1), (id1, id0)} & matched) > 0:` / `    continue` |
| P1-51 | hloc/triangulation.py:129 | #41 | `get_keypoints` reopens and closes the feature file for `name1` on every inner iteration; the same `name1` recurs across outer iterations with no cache — an N+1 read over the pair graph. | `kps1, noise1 = get_keypoints(features_path, name1, return_uncertainty=True)` |
| P1-52 | hloc/triangulation.py:36 | #32 | Four consecutive loops bind `camera_id`, `rig_id`, `frame_id`, `image_id` and use none of them (the writes pass `use_*_id=True` instead). | `for camera_id, camera in reconstruction.cameras.items():` / `    db.write_camera(camera, use_camera_id=True)` |
| P1-53 | hloc/triangulation.py:73 | #11 | The symmetric-pair dedup idiom is duplicated verbatim in `import_matches` and `geometric_verification` (:138/:140), including the `> 0` on a set intersection. | `if len({(id0, id1), (id1, id0)} & matched) > 0:` / `    continue` |
| P1-54 | hloc/triangulation.py:21 | #9 | `OutputCapture` writes a third-party module global and restores it to a hard-coded `True` on exit rather than the value it found — nesting or a non-default starting state silently loses the setting. | `pycolmap.logging.alsologtostderr = False` ... `pycolmap.logging.alsologtostderr = True` |
| P1-55 | hloc/reconstruction.py:99 | #33 | Declared `-> pycolmap.Reconstruction`, returns `None` at :114; `main` (:155) has the same annotation and propagates the None to callers, which the pipelines then use unchecked. | `) -> pycolmap.Reconstruction:` ... `logger.error("Could not reconstruct any model!")` / `return None` |
| P1-56 | hloc/reconstruction.py:117 | #12 | Hand-rolled argmax over a dict: eight lines reimplementing `max(reconstructions, key=lambda i: reconstructions[i].num_reg_images())`. | `largest_index = None` / `largest_num_images = 0` / `for index, rec in reconstructions.items():` |
| P1-57 | hloc/reconstruction.py:65 | none | `pycolmap.Database.open` used without `with`, leaking the handle, while :25, :54, :169, :213 in the same package all use the context manager. | `num_images = pycolmap.Database.open(database_path).num_images()` |
| P1-58 | hloc/reconstruction.py:85 | #4 | `options or {}` re-establishes an invariant the only caller already established: `run_reconstruction` replaces None at :103-105 and always passes a dict at :109. | `options=options or {},` |
| P1-59 | hloc/reconstruction.py:156 | #11 | `reconstruction.main` and `triangulation.main` (:203) share a ~25-line block: the three `assert *.exists()`, `mkdir`, `database = sfm_dir / "database.db"`, the identical 8-line `import_features`/`import_matches` pair, and the `if not skip_geometric_verification` branch. | `assert features.exists(), features` / `assert pairs.exists(), pairs` / `assert matches.exists(), matches` |
| P1-60 | hloc/pairs_from_exhaustive.py:34 | #11 | The `ref_list` ladder is a copy of the `image_list` ladder at :19 with one identifier left un-renamed: the guard tests `image_list`, so a list `ref_list` passed with `image_list=None` falls into the ValueError branch. | `elif isinstance(image_list, collections.Iterable):` / `    names_ref = list(ref_list)` |
| P1-61 | hloc/pairs_from_exhaustive.py:44 | #12 | Nested index loops with a `j <= i` skip reimplement `itertools.combinations` / `itertools.product`. | `for i, n1 in enumerate(names_q):` / `    for j, n2 in enumerate(names_ref):` / `        if self_matching and j <= i:` |
| P1-62 | hloc/pairs_from_exhaustive.py:51 | #11 | The "Found N pairs" log plus join-and-write tail is duplicated verbatim across pairs_from_exhaustive.py:51-53, pairs_from_covisibility.py:49-51, pairs_from_retrieval.py:116-118 and near-verbatim at pairs_from_poses.py:56-58. | `logger.info(f"Found {len(pairs)} pairs.")` / `with open(output, "w") as f:` / `    f.write("\n".join(" ".join([i, j]) for i, j in pairs))` |
| P1-63 | hloc/pairs_from_covisibility.py:14 | #32 | `cameras` is unpacked from `read_model` and never referenced; the reader pays for a name that carries nothing. | `cameras, images, points3D = read_model(model)` |
| P1-64 | hloc/pairs_from_covisibility.py:43 | #3 | The assertion restates the postcondition of the `argsort` two lines above: `ind_top` was just sorted by `-covis_num`, so element 0 is the max by construction. | `assert covis_num[ind_top[0]] == np.max(covis_num)` |
| P1-65 | hloc/pairs_from_retrieval.py:112 | none | `self` is bound as an ordinary local in a module-level function, so every reader's first parse of the name is wrong. | `self = np.array(query_names)[:, None] == np.array(db_names)[None]` |
| P1-66 | hloc/pairs_from_retrieval.py:45 | #41 | The HDF5 file is opened and closed once per name inside the loop instead of once per distinct file; over a database of N images that is N opens on the retrieval path. | `for n in names:` / `    with h5py.File(str(path[name2idx[n]]), "r", libver="latest") as fd:` |
| P1-67 | hloc/pairs_from_retrieval.py:38 | #37 | `key` is a knob no call site ever turns: both calls (:107, :108) take the default. | `def get_descriptors(names, path, name2idx=None, key="global_descriptor"):` |
| P1-68 | hloc/pairs_from_retrieval.py:74 | #1 | Nine wholly unannotated parameters, six of them mutually-constraining optionals whose legal combinations are only discoverable by reading `parse_names` and the branch at :89-104. | `def main(` / `    descriptors,` / `    output,` |
| P1-69 | hloc/visualization.py:13 | #9 | Mutable default `selected=[]` on a public entry point; same at :73. | `reconstruction, image_dir, color_by="visibility", selected=[], n=1, seed=0, dpi=75` |
| P1-70 | hloc/visualization.py:77 | #1 | Opaque `**kwargs` in a public signature, forwarded at :97 into `visualize_loc_from_log`; the accepted names (`top_k_db`, `dpi`) are invisible at the boundary. | `**kwargs,` |
| P1-71 | hloc/utils/viz_3d.py:92 | #2 | `if size is not None` on a parameter declared `size: float = 1.0`: under the declared type the else branch at :96-97 is unreachable. | `size: float = 1.0,` ... `if size is not None:` |
| P1-72 | hloc/utils/viz_3d.py:18 | #11 | Two different `to_homogeneous` implementations live in sibling modules under `hloc/utils/` (this one uses `np.concatenate`, geometry.py:5 uses `np.pad`); one fact, two homes. | `def to_homogeneous(points):` / `    pad = np.ones((points.shape[:-1] + (1,)), dtype=points.dtype)` |
| P1-73 | hloc/utils/viz_3d.py:142 | #13 | A three-deep forwarding chain: `plot_cameras` (:174) forwards to `plot_image_colmap` (:156), which forwards to `plot_camera_colmap`, which forwards to `plot_camera` — each hop only re-spells arguments. | `def plot_camera_colmap(` ... `plot_camera(` |
| P1-74 | hloc/utils/viz_3d.py:176 | #32 | `image_id` is bound and never used. | `for image_id, image in reconstruction.images.items():` |
| P1-75 | hloc/colmap_from_nvm.py:32 | #11 | `quaternion_to_rotation_matrix` duplicates `read_write_model.qvec2rotmat` (:513) — the same 3x3 formula, in a module that already imports five names from read_write_model.py. | `def quaternion_to_rotation_matrix(qvec):` |
| P1-76 | hloc/colmap_from_nvm.py:72 | none | The NVM file is opened bare and never closed, twelve lines after the intrinsics file is read with `with`. | `nvm_f = open(nvm_path, "r")` |
| P1-77 | hloc/colmap_from_nvm.py:50 | #11 | `read_nvm_model` and `pipelines/RobotCar/colmap_from_nvm.py:25` are a ~100-line near-verbatim clone (identical NVM header skip, point loop, keypoint fill-in and Image construction); only the intrinsics source and a `.replace("png","jpg")` differ. | `def read_nvm_model(nvm_path, intrinsics_path, image_ids, camera_ids, skip_points=False):` |
| P1-78 | hloc/pipelines/RobotCar/colmap_from_nvm.py:51 | #34 | A commented-out assertion, left where the clone's original (colmap_from_nvm.py:77) has a live one. | `# assert num_images == len(cameras), (num_images, len(cameras))` |
| P1-79 | hloc/utils/read_write_model.py:462 | #12 | `if <cond>: return True` / `return False` — the boolean is the condition. | `        and os.path.isfile(os.path.join(path, "points3D" + ext))` / `    ):` / `        return True` |
| P1-80 | hloc/utils/read_write_model.py:73 | #12 | `dict([...list comprehension of pairs...])` where a dict comprehension is the idiom; twice, at :73 and :76. | `CAMERA_MODEL_IDS = dict(` / `    [(camera_model.model_id, camera_model) for camera_model in CAMERA_MODELS]` |
| P1-81 | hloc/utils/read_write_model.py:493 | #11 | Drift between copies of one expression: the cameras/images lines append `ext` inside `os.path.join`, the points3D line appends it outside. Four sites (:493, :497, :505, :509) with two spellings. | `points3D = read_points3D_text(os.path.join(path, "points3D") + ext)` |
| P1-82 | hloc/utils/read_write_model.py:103 | none | The builtin `bytes` is shadowed by a local. | `bytes = struct.pack(endian_character + format_char_sequence, *data)` |
| P1-83 | hloc/utils/read_write_model.py:109 | #11 | Three copies of the same text-reader skeleton (`while True` / `readline` / `if not line: break` / `strip` / skip blank-or-`#`) at :117-123, :212-218 and :350-356. | `while True:` / `    line = fid.readline()` / `    if not line:` |
| P1-84 | hloc/utils/read_write_model.py:486 | #34 | `except FileNotFoundError: raise FileNotFoundError(...)` — the handler re-raises the same type with a new message and no `from`, discarding the original cause. | `except FileNotFoundError:` / `    raise FileNotFoundError(` |
| P1-85 | hloc/utils/read_write_model.py:201 | #32 | A writer returns its input; no call site uses the value (same at :510). | `def write_cameras_binary(cameras, path_to_model_file):` ... `return cameras` |
| P1-86 | hloc/matchers/__init__.py:1 | #32 | `get_matcher` has zero references anywhere in the repo (`dynamic_load` is used instead), and it is also broken: no module in `hloc/matchers/` defines a `Model` attribute. Its sibling `hloc/extractors/__init__.py` is empty. | `def get_matcher(matcher):` / `    mod = __import__(f"{__name__}.{matcher}", fromlist=[""])` / `    return getattr(mod, "Model")` |
| P1-87 | hloc/matchers/loftr.py:19 | #9 | `cfg = default_cfg` is an alias, not a copy: the next line mutates kornia's imported module-level dict in place, so the threshold from the first LoFTR instance leaks into every later one in the process. | `cfg = default_cfg` / `cfg["match_coarse"]["thr"] = conf["match_threshold"]` |
| P1-88 | hloc/extractors/superpoint.py:41 | #9 | An instance initializer permanently rebinds a function on a third-party module; once any SuperPoint is built with `fix_sampling`, every later SuperPoint in the process is patched too, with no way back. | `if conf["fix_sampling"]:` / `    superpoint.sample_descriptors = sample_descriptors_fix_sampling` |
| P1-89 | hloc/extractors/dir.py:22 | #9 | Import-time mutation of the global module table, aliasing a private sklearn module. | `sys.modules["sklearn.decomposition.pca"] = sklearn.decomposition._pca` |
| P1-90 | hloc/extractors/megaloc.py:13 | #25 | The name every caller uses is `megaloc` (extract_features.py:146), the module is `megaloc.py`, but the class is `MegaPlaces` — no shared token stem across the delegation edge. Every other extractor keeps the stem (`superpoint`/`SuperPoint`, `dir`/`DIR`, `netvlad`/`NetVLAD`). | `class MegaPlaces(BaseModel):` |
| P1-91 | hloc/extractors/dog.py:106 | none | `torch.topk` returns a `(values, indices)` namedtuple; the whole namedtuple is then used as an index on five tensors, so the top-k path indexes with both tensors instead of the indices. | `indices = torch.topk(scores, self.conf["max_keypoints"])` / `keypoints = keypoints[indices]` |
| P1-92 | hloc/extractors/dog.py:63 | #24 | Device enum member selected by a runtime-built attribute name. | `device=getattr(pycolmap.Device, "cuda" if use_gpu else "cpu"),` |
| P1-93 | hloc/extractors/aliked.py:23 | #12 | Identity comprehensions where `list(...)` is the vocabulary; three in one return statement (also disk.py:29-30). | `"keypoints": [f for f in features["keypoints"]],` / `"keypoint_scores": [f for f in features["keypoint_scores"]],` |
| P1-94 | hloc/extractors/netvlad.py:13 | #32 | The module-level `logger` (and therefore the `logging` import at :1) is never used anywhere in the file. | `logger = logging.getLogger(__name__)` |
| P1-95 | hloc/extractors/d2net.py:28 | none | The checkpoint is fetched by shelling out to `wget`, which is neither a declared dependency nor present on Windows, while the same repo already downloads checkpoints two other ways (netvlad.py:66 `torch.hub.download_url_to_file`, dir.py:48 `gdown`). | `cmd = [` / `    "wget",` / `    "https://dusmanu.com/files/d2-net/" + conf["model_name"],` |
| P1-96 | hloc/pipelines/Aachen_v1_1/pipeline_loftr.py:104 | #32 | The `__main__` block parses arguments and then ends: `run(args)` is missing, so the module's only function is unreachable and the script silently does nothing. Contrast Aachen/pipeline.py:109. | `args = parser.parse_args()` |
| P1-97 | hloc/pipelines/RobotCar/pipeline.py:143 | #32 | Same defect: `args = parser.parse_args()` is the last line and `run(args)` is never called. | `args = parser.parse_args()` |
| P1-98 | hloc/pipelines/CMU/pipeline.py:119 | none | `args.slice` does not exist — the flag is `--slices` (:87) — so every run dies with AttributeError; and because :121 is `if` rather than `elif`, the `*` branch's assignment would be discarded even if it ran. | `if args.slice == "*":` / `    slices = TEST_SLICES` / `if "-" in args.slices:` |
| P1-99 | hloc/pipelines/CMU/pipeline.py:125 | #24 | `eval` on a raw CLI string as the slice-list parser. | `slices = eval(args.slices)` |
| P1-100 | hloc/pipelines/CMU/pipeline.py:63 | #32 | The value assigned on :63 is overwritten on the very next line; the name says the reference descriptors are used, and they are not. | `global_descriptors = extract_features.main(retrieval_conf, ref_images, outputs)` / `global_descriptors = extract_features.main(retrieval_conf, query_images, outputs)` |
| P1-101 | hloc/pipelines/CMU/pipeline.py:76 | none | `query_list` is built at :44 and written at :62, then never used: `localize_sfm.main` is handed the Aachen-shaped glob path instead, which does not exist in the CMU layout. | `dataset / "queries/*_time_queries_with_intrinsics.txt",` |
| P1-102 | hloc/pipelines/7Scenes/pipeline.py:48 | #9 | The pipeline mutates `match_features.confs` in place — `matcher_conf` is the shared module-level dict, not a copy — so every later consumer of `confs["superglue"]` in the process sees `sinkhorn_iterations = 5`. | `matcher_conf = match_features.confs["superglue"]` / `matcher_conf["model"]["sinkhorn_iterations"] = 5` |
| P1-103 | hloc/pipelines/7Scenes/pipeline.py:35 | #38 | The feature-file name `"feats-superpoint-n4096-r1024"` is re-declared in four modules (extract_features.py:30, match_features.py:262, here, Cambridge/pipeline.py:30), and here and in Cambridge the whole `superpoint_aachen` config dict is re-typed rather than referenced. | `"output": "feats-superpoint-n4096-r1024",` |
| P1-104 | hloc/pipelines/Aachen/pipeline.py:17 | #11 | `Aachen/pipeline.py` and `Aachen_v1_1/pipeline.py:16` are near-verbatim clones — same `run` body (same comments, same call order), and the identical 26-line argparse block is copied across five pipeline modules (Aachen, Aachen_v1_1, pipeline_loftr, CMU, RobotCar). | `def run(args):` / `    # Setup the paths` / `    dataset = args.dataset` |
| P1-105 | hloc/pipelines/Aachen/pipeline.py:33 | #39 | Comments that restate the line under them, copied into three pipeline modules: `# list the standard configurations available` above two `logger.info("Configs for ...")` calls, `# pick one of the configurations for extraction and matching` (:37) above three `confs[...]` lookups, and `outputs = args.outputs  # where everything will be saved` (:22). | `# list the standard configurations available` / `logger.info("Configs for feature extractors:\n%s", pformat(extract_features.confs))` |
| P1-106 | hloc/pipelines/Aachen_v1_1/pipeline_loftr.py:23 | none | `outputs.mkdir()` without `parents=True, exist_ok=True`, unlike every sibling pipeline; it raises on a nested output path and on any second run. | `outputs.mkdir()` |
| P1-107 | hloc/pipelines/4Seasons/localize.py:41 | #29 | The module body is the program: `parse_args()` at :41 and a pipeline that irreversibly deletes dataset images at :69 both execute on plain import, with no module docstring, no `main`, and no `__main__` guard. Same shape in prepare_reference.py:20. | `args = parser.parse_args()` ... `delete_unused_images(seq_images, timestamps)` |
| P1-108 | hloc/pipelines/4Seasons/localize.py:15 | none | Doubled path separator in the `training` entry, where the other three entries use a single `/`. | `"training": "RelocalizationFilesTrain//relocalizationFile_recording_2020-03-24_17-36-22.txt",` |
| P1-109 | hloc/pipelines/4Seasons/localize.py:66 | #39 | The comment's first line is a truncated sentence that states nothing, and the two-line block (with the typo "unsused") is copy-pasted into prepare_reference.py:37-38 where the first line has been rewritten. | `# Not all query images that are used for the evaluation` / `# To save time in feature extraction, we delete unsused images.` |
| P1-110 | hloc/pipelines/4Seasons/utils.py:213 | #9 | Mutable default list argument. | `def evaluate_submission(submission_dir, relocs, ths=[0.1, 0.2, 0.5]):` |
| P1-111 | hloc/pipelines/Cambridge/utils.py:41 | #11 | The SIMPLE_RADIAL rescale block — assert model, `sx`/`sy`, `assert sx == sy`, `params * np.array([sx, sx, sy, 1.0])` — is duplicated verbatim at :80-85 in the same file. | `assert camera.model == "SIMPLE_RADIAL"` / `sx = w / camera.width` / `sy = h / camera.height` |
| P1-112 | hloc/pipelines/7Scenes/create_gt_sfm.py:106 | #2 | The `p3did == -1` guard can never fire: `invalid_p3D_ids` is built at :93/:102 exclusively from `p3D_ids[p3D_ids != -1]`. | `for p3did in invalid_p3D_ids:` / `    if p3did == -1:` / `        continue` |
| P1-113 | hloc/pipelines/7Scenes/create_gt_sfm.py:14 | #11 | The six-line `pycolmap.Camera(camera_id=..., model=..., width=..., height=..., params=...)` reconstruction from the namedtuple camera is duplicated at :62. | `pycolmap_camera = pycolmap.Camera(` / `    camera_id=camera.id,` / `    model=camera.model,` |
| P1-114 | hloc/pipelines/7Scenes/pipeline.py:16 | #38 | The seven-element `SCENES` list literal is declared at module level in two modules (also create_gt_sfm.py:143); a scene added to one is missed by the other. | `SCENES = ["chess", "fire", "heads", "office", "pumpkin", "redkitchen", "stairs"]` |
| P1-115 | hloc/pipelines/7Scenes/utils.py:5 | none | Absolute `from hloc....` import from inside the `hloc` package, where every sibling pipeline module uses relative imports (`from ...utils...`); also Cambridge/utils.py:6. It breaks if the package is ever vendored under another name. | `from hloc.utils.read_write_model import read_model, write_model` |
| P1-116 | hloc/match_dense.py:1 | #29 | The six largest prod modules carry no module docstring and no cost statement on their entry points: match_dense.py (606 lines), match_features.py (268), localize_sfm.py (235), triangulation.py (281), reconstruction.py (234), visualization.py (165). Each `main` runs GPU inference or full SfM over an entire dataset. | `import argparse` |
| P1-117 | README.md:180 | none | The documented match-file key is the deprecated format: `names_to_pair` (parsers.py:54) joins with `/`, not `_`; `_` is `names_to_pair_old` (:58), kept only for backward-compatible reads. | ``each key corresponds to the string `path0.replace('/', '-')+'_'+path1.replace('/', '-')` `` |

## Phase 2 - audit finding verdicts

324 findings, one row each, keyed by file:line + rule. Verdicts: 273 real / 51 fp.
Provenance caveats read before judging: family P (#41) silent (no hot-roots config),
python-env unresolved (44 unresolved imports, density 0.71).

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| hloc/extract_features.py:234 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/localize_sfm.py:80 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/localize_sfm.py:140 | #1 | heuristic | real | 'config: Dict = None' - the default contradicts the annotation and the key shape is undeclared. |
| hloc/localize_sfm.py:140 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/match_dense.py:233 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/match_dense.py:293 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/match_dense.py:335 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/match_dense.py:473 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/match_dense.py:539 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/match_features.py:156 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/match_features.py:186 | #1 | heuristic | real | 'match_path: Path = None' - implicit Optional; the annotation excludes the value every caller passes. |
| hloc/match_features.py:211 | #1 | heuristic | real | bare dict / Any / opaque **kwargs on a published signature: the accepted shape is discoverable only from the body. |
| hloc/reconstruction.py:34 | #1 | heuristic | real | Dict[str, Any] / Any-returning option bag on a public boundary - the option names live only inside pycolmap. |
| hloc/reconstruction.py:63 | #1 | heuristic | real | Dict[str, Any] / Any-returning option bag on a public boundary - the option names live only inside pycolmap. |
| hloc/reconstruction.py:98 | #1 | heuristic | real | Dict[str, Any] / Any-returning option bag on a public boundary - the option names live only inside pycolmap. |
| hloc/reconstruction.py:153 | #1 | heuristic | real | Dict[str, Any] / Any-returning option bag on a public boundary - the option names live only inside pycolmap. |
| hloc/reconstruction.py:154 | #1 | heuristic | real | Dict[str, Any] / Any-returning option bag on a public boundary - the option names live only inside pycolmap. |
| hloc/triangulation.py:177 | #1 | heuristic | real | Dict[str, Any] / Any-returning option bag on a public boundary - the option names live only inside pycolmap. |
| hloc/triangulation.py:201 | #1 | heuristic | real | Dict[str, Any] / Any-returning option bag on a public boundary - the option names live only inside pycolmap. |
| hloc/triangulation.py:240 | #1 | heuristic | real | Dict[str, Any] / Any-returning option bag on a public boundary - the option names live only inside pycolmap. |
| hloc/utils/viz.py:139 | #1 | heuristic | real | **kwargs forwarded across a hop (viz_3d chains three deep); no caller can see the accepted names. |
| hloc/utils/viz_3d.py:143 | #1 | heuristic | real | **kwargs forwarded across a hop (viz_3d chains three deep); no caller can see the accepted names. |
| hloc/utils/viz_3d.py:161 | #1 | heuristic | real | **kwargs forwarded across a hop (viz_3d chains three deep); no caller can see the accepted names. |
| hloc/utils/viz_3d.py:174 | #1 | heuristic | real | **kwargs forwarded across a hop (viz_3d chains three deep); no caller can see the accepted names. |
| hloc/visualization.py:77 | #1 | heuristic | real | **kwargs forwarded across a hop (viz_3d chains three deep); no caller can see the accepted names. |
| hloc/match_dense.py:576 | #2 | proved | real | declared Optional[Path]; after the None and list arms the Path isinstance is provably true - removable dead weight. |
| hloc/match_features.py:193 | #2 | heuristic | fp | fp: the guard is live (match_from_paths:231 passes None); the defect is the implicit-Optional annotation, already caught as #1 finding 11, not a removable check. |
| hloc/pairs_from_exhaustive.py:21 | #2 | proved | real | after the (str, Path) arm only List[str] remains, so the Iterable test is always true and the ValueError branch is dead. |
| hloc/utils/viz.py:130 | #2 | heuristic | fp | fp: visualization.py:65 and :163 pass lcolor=None - the guard is exercised; 'str' was inferred from an unannotated default. |
| hloc/utils/viz_3d.py:92 | #2 | proved | real | 'size: float = 1.0' is non-Optional and no caller passes None, so the else branch at :96 is unreachable. |
| hloc/pairs_from_retrieval.py:17 | #4 | indexed | fp | fp: both call sites (:101, :104) forward main's db_prefix/query_prefix, which default to None - callers do not establish the invariant. |
| hloc/colmap_from_nvm.py:50 | #5 | indexed | fp | fp: proposes 'nvm_path: bool' for a path that is handed to open() - the lifted type is nonsense. |
| hloc/pipelines/4Seasons/utils.py:213 | #5 | indexed | real | 'ths' is unannotated with a mutable default and one call site; Iterable[float] is a genuine, safe narrowing. |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:25 | #5 | indexed | fp | fp: the same nonsensical 'nvm_path: bool' lift as finding 32. |
| hloc/colmap_from_nvm.py:50 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/localize_inloc.py:40 | #6 | indexed | real | 'get_scan_pose' opens and parses a transformation file; the get_ name promises a cheap in-memory lookup. |
| hloc/pipelines/4Seasons/utils.py:20 | #6 | indexed | real | 'get_timestamps' globs and reads every matching file; the get_ name hides that cost from every call site. |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:25 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/utils/read_write_model.py:109 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/utils/read_write_model.py:135 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/utils/read_write_model.py:204 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/utils/read_write_model.py:241 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/utils/read_write_model.py:342 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/utils/read_write_model.py:374 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/utils/read_write_model.py:473 | #6 | indexed | fp | fp: a read_* function whose whole contract is reading a file announces the effect the rule calls a lie. |
| hloc/match_dense.py:237 | #9 | heuristic | real | genuine mutable default argument shared across calls. |
| hloc/match_dense.py:341 | #9 | heuristic | real | genuine mutable default argument shared across calls. |
| hloc/match_dense.py:342 | #9 | heuristic | real | genuine mutable default argument shared across calls. |
| hloc/match_dense.py:478 | #9 | heuristic | real | the worst instance: the default list is actually mutated (:500 appends feature_path_q), so entries accumulate across calls in one process. |
| hloc/pipelines/4Seasons/utils.py:213 | #9 | heuristic | real | genuine mutable default argument shared across calls. |
| hloc/visualization.py:13 | #9 | heuristic | real | genuine mutable default argument shared across calls. |
| hloc/visualization.py:73 | #9 | heuristic | real | genuine mutable default argument shared across calls. |
| hloc/match_dense.py:233 | #10 | indexed | real | the body only iterates or indexes, so the concrete container is a demand the code does not need; the widening was counterfactually verified. |
| hloc/match_dense.py:293 | #10 | indexed | real | the body only iterates or indexes, so the concrete container is a demand the code does not need; the widening was counterfactually verified. |
| hloc/match_dense.py:335 | #10 | indexed | real | the body only iterates or indexes, so the concrete container is a demand the code does not need; the widening was counterfactually verified. |
| hloc/triangulation.py:48 | #10 | indexed | real | the body only iterates or indexes, so the concrete container is a demand the code does not need; the widening was counterfactually verified. |
| hloc/triangulation.py:58 | #10 | indexed | real | the body only iterates or indexes, so the concrete container is a demand the code does not need; the widening was counterfactually verified. |
| hloc/triangulation.py:100 | #10 | indexed | real | the body only iterates or indexes, so the concrete container is a demand the code does not need; the widening was counterfactually verified. |
| hloc/triangulation.py:240 | #10 | indexed | real | the body only iterates or indexes, so the concrete container is a demand the code does not need; the widening was counterfactually verified. |
| hloc/colmap_from_nvm.py:72 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:79 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:84 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:88 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:92 | #11 | indexed | real | genuinely duplicated 12-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:107 | #11 | indexed | real | genuinely duplicated 10-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:107 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:115 | #11 | indexed | real | genuinely duplicated 6-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:142 | #11 | indexed | real | genuinely duplicated 6-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:149 | #11 | indexed | real | genuinely duplicated 9-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/colmap_from_nvm.py:179 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.main, hloc.localize_inloc.main, hloc.localize_sfm.main +2 more |
| hloc/colmap_from_nvm.py:180 | #11 | indexed | real | genuinely duplicated 9-statement block; partners: hloc.colmap_from_nvm.main, hloc.pipelines.RobotCar.colmap_from_nvm.main |
| hloc/extract_features.py:258 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.extract_features.main, hloc.match_dense.match_dense, hloc.match_features.match_from_paths |
| hloc/extractors/megaloc.py:18 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.extractors.megaloc.MegaPlaces._init, hloc.extractors.openibl.OpenIBL._init |
| hloc/extractors/megaloc.py:22 | #11 | indexed | real | genuinely duplicated ?-statement block; partners: hloc.extractors.megaloc.MegaPlaces._forward, hloc.extractors.openibl.OpenIBL._forward |
| hloc/extractors/openibl.py:17 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.extractors.megaloc.MegaPlaces._init, hloc.extractors.openibl.OpenIBL._init |
| hloc/extractors/openibl.py:21 | #11 | indexed | real | genuinely duplicated ?-statement block; partners: hloc.extractors.megaloc.MegaPlaces._forward, hloc.extractors.openibl.OpenIBL._forward |
| hloc/localize_inloc.py:23 | #11 | indexed | real | genuinely duplicated 6-statement block; partners: hloc.localize_inloc.interpolate_scan, hloc.pipelines.7Scenes.create_gt_sfm.interpolate_depth |
| hloc/localize_inloc.py:124 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.main, hloc.localize_inloc.main, hloc.localize_sfm.main +2 more |
| hloc/localize_inloc.py:134 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.localize_inloc.main, hloc.localize_sfm.main |
| hloc/localize_inloc.py:162 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.localize_inloc.main, hloc.localize_sfm.main |
| hloc/localize_sfm.py:142 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.main, hloc.localize_inloc.main, hloc.localize_sfm.main +2 more |
| hloc/localize_sfm.py:157 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.localize_inloc.main, hloc.localize_sfm.main |
| hloc/localize_sfm.py:215 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.localize_inloc.main, hloc.localize_sfm.main |
| hloc/match_dense.py:239 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.extract_features.main, hloc.match_dense.match_dense, hloc.match_features.match_from_paths |
| hloc/match_dense.py:362 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.match_dense.aggregate_matches, hloc.match_dense.assign_matches |
| hloc/match_dense.py:453 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.match_dense.aggregate_matches, hloc.match_dense.assign_matches |
| hloc/match_dense.py:485 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.match_dense.match_and_assign, hloc.match_features.match_from_paths |
| hloc/match_features.py:126 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.match_features.FeaturePairsDataset.__getitem__ |
| hloc/match_features.py:132 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.match_features.FeaturePairsDataset.__getitem__ |
| hloc/match_features.py:229 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.match_dense.match_and_assign, hloc.match_features.match_from_paths |
| hloc/match_features.py:236 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.extract_features.main, hloc.match_dense.match_dense, hloc.match_features.match_from_paths |
| hloc/pipelines/4Seasons/utils.py:68 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.4Seasons.utils.parse_poses, hloc.pipelines.4Seasons.utils.parse_relocalization |
| hloc/pipelines/4Seasons/utils.py:88 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.4Seasons.utils.parse_poses, hloc.pipelines.4Seasons.utils.parse_relocalization |
| hloc/pipelines/7Scenes/create_gt_sfm.py:33 | #11 | indexed | real | genuinely duplicated 6-statement block; partners: hloc.localize_inloc.interpolate_scan, hloc.pipelines.7Scenes.create_gt_sfm.interpolate_depth |
| hloc/pipelines/7Scenes/pipeline.py:29 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.7Scenes.pipeline.run_scene, hloc.pipelines.Cambridge.pipeline.run_scene, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/7Scenes/pipeline.py:30 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.7Scenes.pipeline.run_scene, hloc.pipelines.CMU.pipeline.run_slice, hloc.pipelines.Cambridge.pipeline.run_scene +1 more |
| hloc/pipelines/Aachen/pipeline.py:19 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/Aachen/pipeline.py:20 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run |
| hloc/pipelines/Aachen/pipeline.py:23 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/Aachen/pipeline.py:24 | #11 | indexed | real | genuinely duplicated 10-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run |
| hloc/pipelines/Aachen/pipeline.py:24 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run +1 more |
| hloc/pipelines/Aachen/pipeline.py:24 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen/pipeline.py:35 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen/pipeline.py:38 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.CMU.pipeline.run_slice +1 more |
| hloc/pipelines/Aachen/pipeline.py:50 | #11 | indexed | real | genuinely duplicated 7-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run |
| hloc/pipelines/Aachen/pipeline.py:50 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/Aachen/pipeline.py:55 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen_v1_1/pipeline.py:18 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen_v1_1/pipeline.py:20 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run |
| hloc/pipelines/Aachen_v1_1/pipeline.py:23 | #11 | indexed | real | genuinely duplicated 10-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run |
| hloc/pipelines/Aachen_v1_1/pipeline.py:23 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run +1 more |
| hloc/pipelines/Aachen_v1_1/pipeline.py:23 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen_v1_1/pipeline.py:36 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen_v1_1/pipeline.py:39 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.CMU.pipeline.run_slice +1 more |
| hloc/pipelines/Aachen_v1_1/pipeline.py:45 | #11 | indexed | real | genuinely duplicated 7-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run |
| hloc/pipelines/Aachen_v1_1/pipeline.py:45 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/Aachen_v1_1/pipeline.py:50 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen_v1_1/pipeline_loftr.py:18 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen_v1_1/pipeline_loftr.py:24 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run +1 more |
| hloc/pipelines/Aachen_v1_1/pipeline_loftr.py:24 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen_v1_1/pipeline_loftr.py:34 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/Aachen_v1_1/pipeline_loftr.py:45 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run |
| hloc/pipelines/CMU/pipeline.py:38 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.7Scenes.pipeline.run_scene, hloc.pipelines.CMU.pipeline.run_slice, hloc.pipelines.Cambridge.pipeline.run_scene +1 more |
| hloc/pipelines/CMU/pipeline.py:44 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.CMU.pipeline.run_slice, hloc.pipelines.Cambridge.pipeline.run_scene |
| hloc/pipelines/CMU/pipeline.py:51 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.CMU.pipeline.run_slice +1 more |
| hloc/pipelines/Cambridge/pipeline.py:22 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.7Scenes.pipeline.run_scene, hloc.pipelines.Cambridge.pipeline.run_scene, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/Cambridge/pipeline.py:23 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.7Scenes.pipeline.run_scene, hloc.pipelines.CMU.pipeline.run_slice, hloc.pipelines.Cambridge.pipeline.run_scene +1 more |
| hloc/pipelines/Cambridge/pipeline.py:25 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.CMU.pipeline.run_slice, hloc.pipelines.Cambridge.pipeline.run_scene |
| hloc/pipelines/Cambridge/utils.py:30 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Cambridge.utils.create_query_list_with_intrinsics, hloc.pipelines.Cambridge.utils.scale_sfm_images |
| hloc/pipelines/Cambridge/utils.py:63 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Cambridge.utils.create_query_list_with_intrinsics, hloc.pipelines.Cambridge.utils.evaluate |
| hloc/pipelines/Cambridge/utils.py:77 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Cambridge.utils.create_query_list_with_intrinsics, hloc.pipelines.Cambridge.utils.scale_sfm_images |
| hloc/pipelines/Cambridge/utils.py:105 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Cambridge.utils.create_query_list_with_intrinsics, hloc.pipelines.Cambridge.utils.evaluate |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:46 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:53 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:62 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:66 | #11 | indexed | real | genuinely duplicated 12-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:81 | #11 | indexed | real | genuinely duplicated 10-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:81 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:89 | #11 | indexed | real | genuinely duplicated 6-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:116 | #11 | indexed | real | genuinely duplicated 6-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:123 | #11 | indexed | real | genuinely duplicated 9-statement block; partners: hloc.colmap_from_nvm.read_nvm_model, hloc.pipelines.RobotCar.colmap_from_nvm.read_nvm_model |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:153 | #11 | indexed | real | genuinely duplicated 9-statement block; partners: hloc.colmap_from_nvm.main, hloc.pipelines.RobotCar.colmap_from_nvm.main |
| hloc/pipelines/RobotCar/pipeline.py:54 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/RobotCar/pipeline.py:58 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.7Scenes.pipeline.run_scene, hloc.pipelines.Cambridge.pipeline.run_scene, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/RobotCar/pipeline.py:59 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.7Scenes.pipeline.run_scene, hloc.pipelines.CMU.pipeline.run_slice, hloc.pipelines.Cambridge.pipeline.run_scene +1 more |
| hloc/pipelines/RobotCar/pipeline.py:60 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.RobotCar.pipeline.run |
| hloc/pipelines/RobotCar/pipeline.py:61 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline_loftr.run +1 more |
| hloc/pipelines/RobotCar/pipeline.py:67 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.CMU.pipeline.run_slice +1 more |
| hloc/pipelines/RobotCar/pipeline.py:83 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.pipelines.Aachen.pipeline.run, hloc.pipelines.Aachen_v1_1.pipeline.run, hloc.pipelines.RobotCar.pipeline.run |
| hloc/reconstruction.py:156 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.main, hloc.localize_inloc.main, hloc.localize_sfm.main +2 more |
| hloc/reconstruction.py:156 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.reconstruction.main, hloc.triangulation.main |
| hloc/triangulation.py:115 | #11 | indexed | real | genuinely duplicated 6-statement block; partners: hloc.triangulation.geometric_verification |
| hloc/triangulation.py:126 | #11 | indexed | real | genuinely duplicated 6-statement block; partners: hloc.triangulation.geometric_verification |
| hloc/triangulation.py:203 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.colmap_from_nvm.main, hloc.localize_inloc.main, hloc.localize_sfm.main +2 more |
| hloc/triangulation.py:204 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.reconstruction.main, hloc.triangulation.main |
| hloc/utils/io.py:50 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.utils.io.find_pair |
| hloc/utils/io.py:57 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.utils.io.find_pair |
| hloc/utils/read_write_model.py:118 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_cameras_text, hloc.utils.read_write_model.read_images_text, hloc.utils.read_write_model.read_points3D_text |
| hloc/utils/read_write_model.py:213 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_cameras_text, hloc.utils.read_write_model.read_images_text, hloc.utils.read_write_model.read_points3D_text |
| hloc/utils/read_write_model.py:218 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.utils.read_write_model.read_images_text, hloc.utils.read_write_model.read_points3D_text |
| hloc/utils/read_write_model.py:225 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_images_binary, hloc.utils.read_write_model.read_images_text |
| hloc/utils/read_write_model.py:251 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.utils.read_write_model.read_images_binary, hloc.utils.read_write_model.read_points3D_binary |
| hloc/utils/read_write_model.py:271 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_images_binary, hloc.utils.read_write_model.read_images_text |
| hloc/utils/read_write_model.py:351 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_cameras_text, hloc.utils.read_write_model.read_images_text, hloc.utils.read_write_model.read_points3D_text |
| hloc/utils/read_write_model.py:356 | #11 | indexed | real | genuinely duplicated 5-statement block; partners: hloc.utils.read_write_model.read_images_text, hloc.utils.read_write_model.read_points3D_text |
| hloc/utils/read_write_model.py:361 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_points3D_binary, hloc.utils.read_write_model.read_points3D_text |
| hloc/utils/read_write_model.py:384 | #11 | indexed | real | genuinely duplicated 4-statement block; partners: hloc.utils.read_write_model.read_images_binary, hloc.utils.read_write_model.read_points3D_binary |
| hloc/utils/read_write_model.py:399 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_points3D_binary, hloc.utils.read_write_model.read_points3D_text |
| hloc/utils/read_write_model.py:491 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_model |
| hloc/utils/read_write_model.py:495 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.read_model |
| hloc/utils/read_write_model.py:503 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.write_model |
| hloc/utils/read_write_model.py:507 | #11 | indexed | real | genuinely duplicated 3-statement block; partners: hloc.utils.read_write_model.write_model |
| hloc/extractors/aliked.py:23 | #12 | heuristic | real | identity comprehension where list() is the vocabulary. |
| hloc/extractors/aliked.py:24 | #12 | heuristic | real | identity comprehension where list() is the vocabulary. |
| hloc/match_dense.py:314 | #12 | heuristic | real | membership test against .keys() on a dict - drop .keys(). |
| hloc/extractors/superpoint.py:44 | #13 | indexed | fp | fp: _forward is the abstract method BaseModel requires of every plugin; the hop is the plugin boundary and cannot be removed. |
| hloc/match_features.py:112 | #13 | indexed | real | WorkQueue.put forwards to self.queue.put and adds nothing. |
| hloc/matchers/superglue.py:30 | #13 | indexed | fp | fp: the same required BaseModel._forward adapter as finding 176. |
| hloc/extract_features.py:233 | #14 | indexed | real | the pipeline threads this same parameter group through every stage; it is the missing paths/config type. |
| hloc/extract_features.py:233 | #14 | indexed | real | the pipeline threads this same parameter group through every stage; it is the missing paths/config type. |
| hloc/match_dense.py:172 | #14 | indexed | real | the pipeline threads this same parameter group through every stage; it is the missing paths/config type. |
| hloc/match_dense.py:538 | #14 | indexed | real | the pipeline threads this same parameter group through every stage; it is the missing paths/config type. |
| hloc/match_dense.py:538 | #14 | indexed | real | the pipeline threads this same parameter group through every stage; it is the missing paths/config type. |
| hloc/reconstruction.py:29 | #14 | indexed | real | the pipeline threads this same parameter group through every stage; it is the missing paths/config type. |
| hloc/reconstruction.py:93 | #14 | indexed | real | the pipeline threads this same parameter group through every stage; it is the missing paths/config type. |
| hloc/utils/viz_3d.py:55 | #14 | indexed | real | fig/color/name recur across the plotly helpers - a style object is the normal refactor. |
| hloc/localize_sfm.py:16 | #15 | heuristic | real | takes a whole Reconstruction to read .images and .points3D - a narrower protocol would make the BFS testable. |
| hloc/localize_sfm.py:73 | #15 | heuristic | real | takes the localizer for .localize and .reconstruction, the latter only to index points3D. |
| hloc/matchers/nearest_neighbor.py:6 | #15 | heuristic | fp | fp: 'sim' is a torch.Tensor and .topk/.device are the natural Tensor interface - there is no narrower type to demand. |
| hloc/pipelines/4Seasons/utils.py:20 | #15 | heuristic | fp | fp: .name/.parent IS the pathlib.Path interface; narrowing would replace Path with a hand-rolled stand-in. |
| hloc/pipelines/4Seasons/utils.py:183 | #15 | heuristic | fp | fp: .name/.parent IS the pathlib.Path interface; narrowing would replace Path with a hand-rolled stand-in. |
| hloc/pipelines/4Seasons/utils.py:213 | #15 | heuristic | fp | fp: .name/.parent IS the pathlib.Path interface; narrowing would replace Path with a hand-rolled stand-in. |
| hloc/triangulation.py:57 | #15 | heuristic | real | takes a pycolmap.Database to call two write_* methods - a writer protocol is a real narrowing. |
| hloc/triangulation.py:99 | #15 | heuristic | real | takes a whole Reconstruction for .cameras/.images only. |
| hloc/utils/parsers.py:34 | #15 | heuristic | fp | fp: .name/.parent IS the pathlib.Path interface; narrowing would replace Path with a hand-rolled stand-in. |
| hloc/utils/viz_3d.py:174 | #15 | heuristic | real | takes a whole Reconstruction for .cameras/.images only. |
| hloc/localize_inloc.py:33 | #17 | heuristic | fp | fp: a 20-line numeric routine; splitting at the neck would separate the bilinear/nearest pass from its own validity mask. |
| hloc/pipelines/7Scenes/create_gt_sfm.py:43 | #17 | heuristic | fp | fp: the same 20-line numeric routine as finding 197. |
| hloc/match_features.py:190 | #19 | heuristic | fp | fp: 'pairs' is a set() built at :188, so the membership test is O(1). The rule's real sites (localize_sfm:104, match_dense:258) went unfound. |
| hloc/localize_sfm.py:58 | #22 | heuristic | real | QueryLocalizer is a two-attribute namespace; localize touches only those, so it reads as a free function over (reconstruction, config). |
| hloc/match_features.py:100 | #22 | heuristic | fp | fp: WorkQueue has no private interface at all, so velcro is vacuously 100%; join genuinely needs threads and queue. |
| hloc/match_features.py:106 | #22 | heuristic | fp | fp: the same vacuous 100% - thread_fn is the worker body and needs the instance queue. |
| hloc/match_features.py:112 | #22 | heuristic | real | put is the forward-only method already named by #13 - the same genuine site. |
| hloc/colmap_from_nvm.py:50 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/extract_features.py:178 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/extract_features.py:233 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/extractors/dog.py:45 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/localize_sfm.py:73 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/localize_sfm.py:130 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/match_dense.py:72 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/match_dense.py:334 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/pairs_from_covisibility.py:12 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/pairs_from_exhaustive.py:11 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/pairs_from_retrieval.py:16 | #23 | heuristic | fp | fp: complexity 16 against a threshold of 15, on a 20-line three-branch function that reads cleanly - threshold noise. |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:25 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/triangulation.py:99 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/visualization.py:12 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/visualization.py:101 | #23 | heuristic | real | genuinely branchy: the reported complexity matches a function a reader must hold whole. |
| hloc/extract_features.py:154 | #24 | heuristic | real | the identifier is assembled at runtime, so the target is unreachable by grep and whole-program analysis is blinded. |
| hloc/extract_features.py:160 | #24 | heuristic | real | the identifier is assembled at runtime, so the target is unreachable by grep and whole-program analysis is blinded. |
| hloc/extractors/dog.py:63 | #24 | heuristic | real | the identifier is assembled at runtime, so the target is unreachable by grep and whole-program analysis is blinded. |
| hloc/matchers/__init__.py:2 | #24 | heuristic | real | __import__ of a runtime-built module path - and the reason the 13 model classes look unreferenced (see the #32 rows). |
| hloc/pipelines/CMU/pipeline.py:125 | #24 | heuristic | real | eval on a raw CLI string. |
| hloc/triangulation.py:247 | #24 | heuristic | real | the identifier is assembled at runtime, so the target is unreachable by grep and whole-program analysis is blinded. |
| hloc/triangulation.py:252 | #24 | heuristic | real | eval on a CLI-supplied key=value fragment. |
| hloc/triangulation.py:253 | #24 | heuristic | real | the identifier is assembled at runtime, so the target is unreachable by grep and whole-program analysis is blinded. |
| hloc/utils/base_model.py:40 | #24 | heuristic | real | the plugin loader: no static edge exists from confs[...]['name'] to any file under hloc/extractors. |
| setup.py:10 | #24 | heuristic | real | eval of a fragment cut out of hloc/__init__.py to recover the version. |
| hloc/utils/parsers.py:54 | #25 | indexed | fp | fp: names_to_pair calls str.join; the checker resolved '.join' to WorkQueue.join - a call-graph misresolution, not a rename. |
| hloc/utils/read_write_model.py:73 | #26 | heuristic | real | CAMERA_MODEL_IDS is assembled by a comprehension; a grep for a model id lands on the loop, not the members. |
| hloc/utils/read_write_model.py:76 | #26 | heuristic | real | CAMERA_MODEL_NAMES is assembled by a comprehension, and it is the dict other modules import. |
| hloc/utils/read_write_model.py:46 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:50 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:55 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:56 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:204 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:241 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:473 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:501 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:513 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/utils/read_write_model.py:535 | #27 | indexed | real | a widely imported symbol whose Read costs 588 lines of namedtuples, six readers, six writers and a CLI. |
| hloc/colmap_from_nvm.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/colmap_from_nvm.py:50 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/extract_features.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/extract_features.py:233 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/extractors/netvlad.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/localize_inloc.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/localize_inloc.py:68 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/localize_inloc.py:123 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/localize_sfm.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/localize_sfm.py:16 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/localize_sfm.py:73 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/localize_sfm.py:130 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/match_dense.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/match_dense.py:72 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/match_dense.py:232 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/match_dense.py:292 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/match_dense.py:334 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/match_dense.py:472 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/match_dense.py:538 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/match_features.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/match_features.py:210 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pairs_from_covisibility.py:12 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pairs_from_exhaustive.py:11 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pairs_from_retrieval.py:74 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/4Seasons/utils.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/pipelines/7Scenes/create_gt_sfm.py:76 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/7Scenes/pipeline.py:19 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/Aachen/pipeline.py:17 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/Aachen_v1_1/pipeline.py:16 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/Aachen_v1_1/pipeline_loftr.py:16 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/CMU/pipeline.py:36 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/Cambridge/pipeline.py:18 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/Cambridge/utils.py:93 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/pipelines/RobotCar/colmap_from_nvm.py:25 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/pipelines/RobotCar/pipeline.py:52 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/reconstruction.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/reconstruction.py:59 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/reconstruction.py:93 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/reconstruction.py:142 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/triangulation.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/triangulation.py:99 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/triangulation.py:190 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/utils/read_write_model.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/utils/viz_3d.py:180 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/visualization.py:1 | #29 | heuristic | real | a module this size carries no docstring - the first screen says nothing about what it is. |
| hloc/visualization.py:12 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/visualization.py:68 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/visualization.py:101 | #29 | heuristic | real | heavy entry point (dataset-scale I/O or GPU inference) with no cost statement for its callers. |
| hloc/extractors/netvlad.py:106 | #30 | heuristic | real | reaches self.netvlad.score_proj.weight to overwrite a submodule weight; a load_weights method on NetVLADLayer is the honest shape. |
| hloc/extractors/aliked.py:6 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/extractors/d2net.py:15 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/extractors/dir.py:25 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/extractors/dog.py:19 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/extractors/megaloc.py:13 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/extractors/netvlad.py:42 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/extractors/openibl.py:7 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/extractors/r2d2.py:13 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/matchers/__init__.py:1 | #32 | indexed | real | genuinely dead: nothing imports get_matcher, and getattr(mod, 'Model') names an attribute no matcher module defines. |
| hloc/matchers/adalam.py:8 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/matchers/lightglue.py:6 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/matchers/loftr.py:10 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/matchers/nearest_neighbor.py:27 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/matchers/superglue.py:10 | #32 | indexed | fp | fp: live plugin code reached through base_model.dynamic_load's __import__ + issubclass scan - unreferenced by name only because of the dynamic loader (#24 finding 227). |
| hloc/utils/viz.py:139 | #32 | indexed | real | genuinely dead: zero references anywhere, notebooks included; only viz.py's own docstring advertises it. |
| hloc/utils/viz_3d.py:23 | #32 | indexed | fp | fp: the documented notebook API - visualize_sfm_2d (7 hits), visualize_loc (3), init_figure (1), plot_reconstruction (1) all occur in the repo's .ipynb files, which the index does not read. |
| hloc/utils/viz_3d.py:180 | #32 | indexed | fp | fp: the documented notebook API - visualize_sfm_2d (7 hits), visualize_loc (3), init_figure (1), plot_reconstruction (1) all occur in the repo's .ipynb files, which the index does not read. |
| hloc/visualization.py:12 | #32 | indexed | fp | fp: the documented notebook API - visualize_sfm_2d (7 hits), visualize_loc (3), init_figure (1), plot_reconstruction (1) all occur in the repo's .ipynb files, which the index does not read. |
| hloc/visualization.py:68 | #32 | indexed | fp | fp: the documented notebook API - visualize_sfm_2d (7 hits), visualize_loc (3), init_figure (1), plot_reconstruction (1) all occur in the repo's .ipynb files, which the index does not read. |
| hloc/localize_sfm.py:58 | #33 | heuristic | fp | fp: both returns are explicit values (return None / return ret), there is no bare return, and the caller at :111 checks 'is not None'. |
| hloc/match_dense.py:472 | #33 | heuristic | real | declares -> Path; bare return at :507 and falls off the end at :534. |
| hloc/match_features.py:210 | #33 | heuristic | real | declares -> Path; bare return at :234 and falls off the end at :255. |
| hloc/reconstruction.py:93 | #33 | heuristic | real | declares -> pycolmap.Reconstruction; returns None at :114, and main propagates it unchecked. |
| hloc/match_features.py:160 | #37 | indexed | real | the Path-mode / reference-features / overwrite knobs are set by no prod caller - the dual-mode API is exercised in one direction only. |
| hloc/match_features.py:161 | #37 | indexed | real | the Path-mode / reference-features / overwrite knobs are set by no prod caller - the dual-mode API is exercised in one direction only. |
| hloc/match_features.py:162 | #37 | indexed | real | the Path-mode / reference-features / overwrite knobs are set by no prod caller - the dual-mode API is exercised in one direction only. |
| hloc/utils/read_write_model.py:81 | #37 | indexed | real | COLMAP files are always little-endian; the knob is untouched at 13 call sites. |
| hloc/utils/read_write_model.py:93 | #37 | indexed | real | the same dead endian knob at 19 call sites. |
| hloc/extract_features.py:41 | #39 | heuristic | fp | fp: the comment states why the config resizes to 1600px - rationale the code cannot carry, not history. |
| hloc/utils/read_write_model.py:15 | #39 | heuristic | fp | fp: line 15 sits inside the BSD license header ('its contributors may be used to endorse...'). |
| hloc/utils/read_write_model.py:81 | #39 | heuristic | fp | fp: the docstring is the only spec for the struct format characters - prose the code genuinely cannot carry. |
| hloc/utils/read_write_model.py:93 | #39 | heuristic | fp | fp: the same binary-format spec; the empty ':param fid:' is noise but the block is earned. |
| hloc/utils/viz.py:54 | #39 | heuristic | fp | fp: the docstring documents array shapes and the colors polymorphism that the unannotated signature does not. |

## Phase 3 - reconciliation

All 117 phase-1 sites. 33 covered, 84 misses (47 detector-miss, 15 threshold-miss,
22 inventory-gap). "Covered" counts a finding at the same site even when it landed
under a different rule id than I mapped.

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | none | inventory-gap | rebinding an imported module name to a value has no rule. |
| P1-2 | none | inventory-gap | list-where-str in package metadata has no rule. |
| P1-3 | #26 | covered | finding #24 @ setup.py:10 names the same `eval` statement. |
| P1-4 | #29 | covered | finding #29 top-loading @ extract_features.py:1. |
| P1-5 | #24 | covered | findings #24 @ :154 and :160. |
| P1-6 | #1 | covered | finding #1 @ :234. |
| P1-7 | #2 | detector-miss | #2 ran with the oracle enabled but never reached `(dt == np.float32) and (dt != np.float16)`; the repo's own trap bars equality from the proved tier, and no heuristic arm replaced it. |
| P1-8 | none | inventory-gap | possibly-unbound name in an exception handler has no rule. |
| P1-9 | #9 | threshold-miss | #9 fired 7x, all on mutable *defaults*; the mutable class-attribute arm is outside its two shapes. |
| P1-10 | #30 | detector-miss | #30 fired once (netvlad); writing another object's attribute from outside never fired. |
| P1-11 | #13 | covered | finding #13 @ match_features.py:112. |
| P1-12 | #33 | covered | finding #33 lying-return @ :210. |
| P1-13 | #32 | detector-miss | unused loop variable is below #32's name-level (symbol/param/import) scope. |
| P1-14 | #1 | covered | finding #1 lying-default @ :186. |
| P1-15 | #34 | detector-miss | #34 returned zero findings repo-wide despite two commented-out code blocks. |
| P1-16 | #9 | covered | finding #9 @ :478. |
| P1-17 | #9 | covered | finding #9 @ :237. |
| P1-18 | #9 | covered | findings #9 @ :341 and :342. |
| P1-19 | #33 | detector-miss | #33 caught the three `-> Path` returns-None cases but not `-> Path` returning a 2-tuple. |
| P1-20 | #33 | covered | finding #33 lying-return @ :472. |
| P1-21 | #33 | detector-miss | branch returning `[0]` vs a 2-tuple never surfaced. |
| P1-22 | #37 | detector-miss | #37 fired only on unused-defaults; the never-exercised (and broken) union arm was missed. |
| P1-23 | #6 | detector-miss | #6 fired 11x, all on read_* io; mutating a caller-owned list argument never fired. |
| P1-24 | none | inventory-gap | two configs colliding on one output filename has no rule. |
| P1-25 | #41 | threshold-miss | family P silent: provenance says no hot-roots config, so #41 could not fire anywhere. |
| P1-26 | #19 | detector-miss | #19's one firing was a false positive elsewhere; this list-typed membership in a loop was missed. |
| P1-27 | #12 | threshold-miss | #12 fired at match_dense.py:314 in the same file; the catalog carries no `set([...])`/repeat-list entry. |
| P1-28 | none | inventory-gap | assigning a str into an `Optional[Path]` parameter has no rule (its downstream effect surfaced as #2 finding @ :576). |
| P1-29 | #1 | covered | finding #1 opaque **kwargs @ :80. |
| P1-30 | #19 | detector-miss | `not in` against a defaultdict(list) value inside the hot loop never fired. |
| P1-31 | #41 | threshold-miss | family P silent. |
| P1-32 | #39 | detector-miss | #39's comment-history arm fired twice, both false; the genuine "anymore" narration was missed. |
| P1-33 | #1 | covered | findings #1 @ :140 (both lying-default and bare dict). |
| P1-34 | none | inventory-gap | unclosed file handle / missing context manager has no rule. |
| P1-35 | none | inventory-gap | half-used local alias has no rule. |
| P1-36 | none | inventory-gap | unguarded concatenate over a possibly-empty list has no rule. |
| P1-37 | #11 | covered | findings #11 @ localize_inloc.py:23 and create_gt_sfm.py:33. |
| P1-38 | #33 | detector-miss | flag-dependent tuple return under `-> np.ndarray` was missed, though a caller (triangulation:118) exercises it. |
| P1-39 | #33 | detector-miss | `Tuple[np.ndarray]` declaring a 1-tuple while returning 2 was missed. |
| P1-40 | #13 | detector-miss | #13 fired 3x on methods; the module-level forwarder `names_to_pair_old` was missed. |
| P1-41 | none | inventory-gap | assert-as-validation (stripped under -O) has no rule. |
| P1-42 | #11 | threshold-miss | the repeated `logger = getLogger(__name__)` is a 1-statement module-level clone, below #11's 3-statement block floor. |
| P1-43 | #24 | covered | finding #24 __import__ @ base_model.py:40. |
| P1-44 | #34 | detector-miss | #34 silent. |
| P1-45 | #9 | threshold-miss | same class-attribute shape as P1-9. |
| P1-46 | #41 | threshold-miss | family P silent. |
| P1-47 | #24 | covered | findings #24 @ :247, :252, :253. |
| P1-48 | none | inventory-gap | argparse key popped but never registered has no rule. |
| P1-49 | #25 | detector-miss | #25's one firing was a misresolution; the real CLI-flag/parameter name mismatch was missed. |
| P1-50 | #41 | threshold-miss | family P silent. |
| P1-51 | #41 | threshold-miss | family P silent. |
| P1-52 | #32 | detector-miss | four unused loop variables, below #32's name-level scope. |
| P1-53 | #11 | threshold-miss | the dedup idiom is a 2-statement block, below the 3-statement floor (#11 did fire on the 6-statement self-clone at :115/:126 in the same function). |
| P1-54 | #9 | detector-miss | #9's "module-level mutable mutated from another module" arm never fired anywhere in the repo. |
| P1-55 | #33 | covered | finding #33 lying-return @ reconstruction.py:93. |
| P1-56 | #12 | detector-miss | catalog carries no hand-rolled-`max` entry. |
| P1-57 | none | inventory-gap | leaked handle / inconsistent context-manager use has no rule. |
| P1-58 | #4 | detector-miss | #4's one firing was false; this genuine single-caller-establishes-it guard was missed. |
| P1-59 | #11 | covered | findings #11 @ reconstruction.py:156 and triangulation.py:203/:204. |
| P1-60 | #11 | detector-miss | no finding at :34; #2 finding 28 names the sibling ladder at :21 but the un-renamed identifier in the copy went unnamed. |
| P1-61 | #12 | detector-miss | catalog carries no combinations/product entry. |
| P1-62 | #11 | detector-miss | the 3-statement write tail recurs in 4 modules and no clone finding landed on any of them. |
| P1-63 | #32 | detector-miss | unused local from a tuple unpack. |
| P1-64 | #3 | detector-miss | #3 returned zero findings repo-wide. |
| P1-65 | none | inventory-gap | `self` used as an ordinary local has no rule. |
| P1-66 | #41 | threshold-miss | family P silent. |
| P1-67 | #37 | threshold-miss | #37's unused-default hits all had 13-19 prod call sites; `key` has 2, under the cutoff. |
| P1-68 | #1 | detector-miss | #1 fires on weak annotations; nine wholly unannotated public params are outside its arms. |
| P1-69 | #9 | covered | findings #9 @ visualization.py:13 and :73. |
| P1-70 | #1 | covered | finding #1 opaque **kwargs @ :77. |
| P1-71 | #2 | covered | finding #2 (proved) @ viz_3d.py:92. |
| P1-72 | #11 | detector-miss | two same-named functions with different bodies are outside structural-clone hashing. |
| P1-73 | #13 | detector-miss | the 3-deep forwarding chain was missed (#1 findings 22-24 flag the same functions' **kwargs instead). |
| P1-74 | #32 | detector-miss | unused loop variable. |
| P1-75 | #11 | detector-miss | a semantically identical quaternion-to-matrix with a different expression tree is outside structural hashing. |
| P1-76 | none | inventory-gap | unclosed file handle has no rule. |
| P1-77 | #11 | covered | 22 clone findings across colmap_from_nvm.py and pipelines/RobotCar/colmap_from_nvm.py. |
| P1-78 | #34 | detector-miss | #34 silent. |
| P1-79 | #12 | detector-miss | catalog carries no if-return-True/return-False entry. |
| P1-80 | #12 | covered | findings #26 computed-declaration @ :73 and :76 - the same site under a different rule. |
| P1-81 | #11 | covered | findings #11 @ :491/:495 and :503/:507. |
| P1-82 | none | inventory-gap | shadowing a builtin has no rule. |
| P1-83 | #11 | covered | findings #11 @ :118, :213, :351. |
| P1-84 | #34 | detector-miss | re-raise of the same exception type with the cause dropped never fired. |
| P1-85 | #32 | detector-miss | a return value no caller consumes is outside #32's scope. |
| P1-86 | #32 | covered | finding #32 dead-symbol @ matchers/__init__.py:1 (and #24 @ :2). |
| P1-87 | #9 | detector-miss | in-place mutation of an imported third-party module global never fired. |
| P1-88 | #9 | detector-miss | monkeypatching a third-party module attribute never fired. |
| P1-89 | #9 | detector-miss | sys.modules mutation at import time never fired. |
| P1-90 | #25 | detector-miss | the config-name/class-name stem mismatch was missed; the same line drew a (false) #32 dead-symbol instead. |
| P1-91 | none | inventory-gap | indexing with a torch namedtuple instead of its .indices has no rule. |
| P1-92 | #24 | covered | finding #24 getattr @ dog.py:63. |
| P1-93 | #12 | covered | findings #12 @ aliked.py:23 and :24 (it missed the third at :25's siblings in disk.py). |
| P1-94 | #32 | detector-miss | an unused module-level assignment (and its import) went unflagged. |
| P1-95 | none | inventory-gap | shelling out to a non-dependency binary has no rule. |
| P1-96 | #32 | detector-miss | `run` is never called anywhere, but a module-level script body appears to count as a root. |
| P1-97 | #32 | detector-miss | same. |
| P1-98 | none | inventory-gap | reading a nonexistent argparse attribute has no rule. |
| P1-99 | #24 | covered | finding #24 eval @ CMU/pipeline.py:125. |
| P1-100 | #32 | detector-miss | dead store (value overwritten on the next line) is outside #32's scope. |
| P1-101 | none | inventory-gap | a computed-and-written file that is then not used has no rule. |
| P1-102 | #9 | detector-miss | in-place mutation of another module's `confs` dict never fired - the most damaging #9 site in the repo. |
| P1-103 | #38 | threshold-miss | #38 returned zero findings; the literal is module-level in only 1 of the 4 modules, under the ">=3 modules at module level" bar. |
| P1-104 | #11 | covered | ~30 clone findings across the five pipeline modules. |
| P1-105 | #39 | detector-miss | #39's comment-restates-code arm produced nothing repo-wide. |
| P1-106 | none | inventory-gap | mkdir without parents/exist_ok, inconsistent with siblings, has no rule. |
| P1-107 | #29 | threshold-miss | #29 fired on 49 sites but not on the two 4Seasons script modules: with 0 top-level defs they fall under its "N top-level defs" bar, so the heaviest entry points in the repo are the ones it skipped. |
| P1-108 | none | inventory-gap | a doubled path separator in a data literal has no rule. |
| P1-109 | #39 | detector-miss | truncated/copy-pasted comment never fired. |
| P1-110 | #9 | covered | finding #9 @ 4Seasons/utils.py:213. |
| P1-111 | #11 | covered | findings #11 @ Cambridge/utils.py:30 and :77. |
| P1-112 | #2 | detector-miss | a guard whose condition is excluded by the filter two lines above never fired. |
| P1-113 | #11 | detector-miss | the twice-written 6-line pycolmap.Camera construction was missed (#11 did fire on interpolate_depth in the same file). |
| P1-114 | #38 | threshold-miss | 2 modules, under #38's >=3 bar. |
| P1-115 | none | inventory-gap | absolute-vs-relative intra-package import style has no rule (#35 covers cycles only). |
| P1-116 | #29 | covered | findings #29 top-loading @ match_dense.py:1, match_features.py:1, localize_sfm.py:1, triangulation.py:1, reconstruction.py:1, visualization.py:1. |
| P1-117 | none | inventory-gap | README documenting the deprecated key format is a doc/code semantic mismatch; #28 checks resolution, not semantics. |

### Reading of the two lists together

- **Where the checker beat me:** the intra-function clones I skipped (triangulation
  `geometric_verification` :115/:126, `io.find_pair` :50/:57, `FeaturePairsDataset.__getitem__`),
  the whole #14 data-clump family (the pipeline really does thread one missing paths type
  through every stage), #10 over-constrained containers, #27 purchase price on
  read_write_model.py, and the dead `endian_character` knobs.
- **Where it lost:** every one of the four in-place mutations of another module's
  state (#9's cross-module arm), all of #34 (two commented-out blocks, one
  cause-dropping re-raise), all of #3, both real #19 sites, all four remaining
  #33 shapes, and both real #39 sites - #39's five firings were all false.
- **Systematic FP causes:** the dynamic loader (#24 finding 227) makes 13 live plugin
  classes look dead (#32); .ipynb is not indexed, so 4 documented public functions
  look dead; `read_*` is treated as an accessor prefix, so 9 honest file readers look
  dishonest (#6); `.name`/`.parent` on a Path is read as a wallet parameter (#15).
  Those four causes account for 32 of the 51 false positives.
- **Volume:** #11 (113) and #29 (49) are half the report. Both are real at nearly
  every site, but the ~30 pipeline clone rows and the 49 doc rows say one thing each,
  repeated - grouping by clone hash and by module would carry the same information at
  a fraction of the reading cost.
