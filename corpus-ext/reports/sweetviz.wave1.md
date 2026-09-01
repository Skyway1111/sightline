# sweetviz — wave 1

Repo: `<GAUNTLET_CORPUS_ROOT>\sweetviz`
Prod tree judged: `sweetviz/*.py` (23 modules, 3925 LoC). No test tree in the repo.
Judged cold against the #1–41 inventory; no sightline output consulted.

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | sweetviz/from_profiling_pandas.py:95 | #32 | `is_url` is defined and never referenced anywhere in the package (no py, no template). Dead. | `def is_url(series: pd.Series, counts: dict) -> bool:` |
| P1-2 | sweetviz/from_profiling_pandas.py:127 | #32 | `is_path` never referenced; its only helper `str_is_path` (:107) is live solely for it, so both are dead. | `def is_path(series, counts) -> bool:` |
| P1-3 | sweetviz/from_profiling_pandas.py:138 | #32 | `is_date` never referenced anywhere. Dead. | `def is_date(series) -> bool:` |
| P1-4 | sweetviz/from_profiling_pandas.py:138 | #13 | `is_date` is a forward-only wrapper: it assigns one library call to a local and returns it, adding nothing. | `is_date_value = pd.api.types.is_datetime64_dtype(series)` / `return is_date_value` |
| P1-5 | sweetviz/from_profiling_pandas.py:91 | #13 | `could_be_numeric` body is a single forwarding call to `pd.api.types.is_numeric_dtype`; the only caller (type_detection.py:63) pays a hop for no meaning. | `def could_be_numeric(series: pd.Series) -> bool:` / `    return pd.api.types.is_numeric_dtype(series)` |
| P1-6 | sweetviz/from_profiling_pandas.py:119 | #12 | `if path.is_absolute(): return True / else: return False` reimplements returning the predicate itself. | `if path.is_absolute():` / `    return True` / `else:` |
| P1-7 | sweetviz/from_profiling_pandas.py:64 | #39 | History narration in a comment: a TODO plus a dated "UPDATE 11-2023: NO IT DIDN'T!!!" retro-log that git already keeps. | `# TODO: CHECK THIS CASE ACTUALLY WORKS` / `# UPDATE 11-2023: NO IT DIDN'T!!! using series, not... keys (?!)` |
| P1-8 | sweetviz/from_dython.py:45 | #32 | `DROP_SAMPLES` (:45) and `DROP_FEATURES` (:46) and `SKIP` (:47) are module constants referenced nowhere — only `REPLACE`/`DROP` are read. | `DROP_SAMPLES = 'drop_samples'` / `DROP_FEATURES = 'drop_features'` / `SKIP = 'skip'` |
| P1-9 | sweetviz/from_dython.py:103 | #37 | `nan_strategy` / `nan_replace_value` are never overridden: all four prod call sites (dataframe_report.py:463,466,472,477) pass positional series only, so the `DROP` arm is unreachable flexibility. | `nan_strategy=REPLACE,` / `nan_replace_value=DEFAULT_REPLACE_VALUE):` |
| P1-10 | sweetviz/from_dython.py:62 | #37 | `convert(data, to)`'s `'list'` and `'dataframe'` arms are dead: the only two call sites (:228, :229) pass `'array'`. Single-implementation dispatch on a magic string. | `elif to == 'list':` |
| P1-11 | sweetviz/from_dython.py:89 | #2 | `isinstance(x, list)` is provably always true — `x` was rebound to a list comprehension at :85 — so the `else` branch is unreachable. | `if isinstance(x, list):` / `    return arr[0].tolist(), arr[1].tolist()` |
| P1-12 | sweetviz/from_dython.py:134 | #12 | Iterating `.keys()` then re-indexing the same `Counter` reimplements `.items()`, paying a hash lookup per element. | `for xy in xy_counter.keys():` / `    p_xy = xy_counter[xy] / total_occurrences` |
| P1-13 | sweetviz/from_dython.py:141 | #7 | An argument-order precondition is narrated in a comment (and repeated in the docstring at :147) rather than encoded — the docstring then documents `x` before `y` while the signature is `(y, x)`. | `# IMPORTANT: look at the order of arguments y and x` / `def theils_u(y,` |
| P1-14 | sweetviz/from_dython.py:234 | #41 | Hot: `correlation_ratio` runs `np.argwhere(fcat == i)` once per category — a full pass over `fcat` per category, O(rows x categories) — inside the O(n^2) association loop. `np.bincount`/groupby is one pass. | `for i in range(0, cat_num):` / `    cat_measures = measurements[np.argwhere(fcat == i).flatten()]` |
| P1-15 | sweetviz/from_dython.py:178 | #41 | Hot: `theils_u` already replaced NaNs at :174-175, then `conditional_entropy(x, y)` re-runs the same two O(n) list comprehensions at :126-127. Doubled work on every one of the n^2 cat-cat pairs. | `s_xy = conditional_entropy(x, y)` |
| P1-16 | sweetviz/from_dython.py:126 | #11 | The four-line nan-strategy preamble is copy-pasted verbatim into `conditional_entropy` (:126), `theils_u` (:174) and `correlation_ratio` (:222). | `if nan_strategy == REPLACE:` / `    x, y = replace_nan_with_value(x, y, nan_replace_value)` / `elif nan_strategy == DROP:` |
| P1-17 | sweetviz/from_dython.py:61 | none | `data.as_matrix()` was removed from pandas in 1.0; the DataFrame→array arm raises `AttributeError` on every supported pandas. Latent break, not a rule. | `converted = data.as_matrix()` |
| P1-18 | sweetviz/comet_ml_logger.py:5 | #34 | Four bare `except:` swallows in one 39-line module (:5, :19, :26, :35); three of them print and continue, hiding the real exception. | `except:` / `    comet_installed = False` |
| P1-19 | sweetviz/config.py:13 | #34 | Commented-out code left at module scope (:13 and :17) beside the live loader. | `# print("Config: " + os.path.abspath('sweetviz_defaults.ini'))` |
| P1-20 | sweetviz/utils.py:9 | #7 | "IMPORTANT: assuming value_counts is ALREADY SORTED" is a caller-must precondition carried only in prose — never asserted, never encoded in a type. | `# IMPORTANT: assuming value_counts is ALREADY SORTED` |
| P1-21 | sweetviz/utils.py:16 | #34 | Superseded code left commented beside its replacement at :16, :21, :37-38, :57-59 — six lines of dead alternative in a 66-line module. | `# clamped_series = pd.Series(value_counts[0:categories_shown_as_is])` |
| P1-22 | sweetviz/utils.py:15 | #39 | `# Fix for #10` appears twice (:15, :19) as issue-history narration attached to lines whose behaviour it does not explain. | `# Fix for #10` |
| P1-23 | sweetviz/utils.py:47 | #19 | `ind in value_counts` is a pandas index lookup performed inside a loop over the whole target index — a scan per element where a vectorised `reindex(fill_value=0)` is one pass. | `for ind in matched_series.index:` / `    if ind in value_counts:` |
| P1-24 | sweetviz/sv_math.py:5 | #40 | `count_fraction_of_true` names one scalar but returns a 2-tuple `(fraction, num_true)`; both call sites (graph_cat.py:195,198,218,221) index `[0]` and discard the second element, so the name reads wrong and half the return is dead. | `return num_true / total if total > 0.0 else 0.0, num_true` |
| P1-25 | sweetviz/type_detection.py:6 | #1 | Public boundary takes an untyped `counts: dict` (a bag keyed by magic strings such as `"value_counts_without_nan"`) and declares `-> object`, so no caller can learn what goes in or comes out. | `def determine_feature_type(series: pd.Series, counts: dict,` / `        must_be_this_type: FeatureType, which_dataframe: str) -> object:` |
| P1-26 | sweetviz/series_analyzer.py:61 | #33 | `add_series_base_stats_to_dict` is annotated `-> dict` but has no return statement — it always returns `None`. Both callers (:135, :137) ignore the value, so the annotation only misleads. | `def add_series_base_stats_to_dict(series: pd.Series, counts: dict, updated_dict: dict) -> dict:` |
| P1-27 | sweetviz/series_analyzer.py:94 | #12 | `True if <cond> else False` reimplements `bool(<cond>)` — and re-derives the predicate `FeatureToProcess.is_target()` (sv_types.py:87) that is already available on the object being read. | `returned_feature_dict["is_target"] = True if to_process.order == -1 else False` |
| P1-28 | sweetviz/series_analyzer.py:48 | #12 | Same `True if ... else False` shape again, this time wrapping a membership test. | `fill_using_strings = True if my_counts[to_fill].index.dtype.name in ('category', 'object') else False` |
| P1-29 | sweetviz/series_analyzer.py:50 | #19 | `key not in my_counts[to_fill]` is a Series membership test inside a loop over the other Series' items — quadratic in distinct values by construction. | `for key, value in other_counts[to_fill].items():` / `    if key not in my_counts[to_fill]:` |
| P1-30 | sweetviz/series_analyzer.py:44 | #34 | Commented-out code at :44-45, :28-29, :37-39, :80, :151-152 — five separate stubs in a 154-line module. | `# IGNORING NAN FOR NOW AS IT CAUSES ISSUES [FIX]` / `# to_fill_list = ["value_counts_with_nan", "value_counts_without_nan"]` |
| P1-31 | sweetviz/series_analyzer_text.py:6 | #15 | `do_detail_text(to_process, ...)` demands the whole `FeatureToProcess` wallet but reads exactly two attributes (`compare_counts`, `source_counts`) and forwards the object nowhere. | `def do_detail_text(to_process: FeatureToProcess, updated_dict: dict):` |
| P1-32 | sweetviz/series_analyzer_text.py:16 | none | `num_values_compare` is bound only under `if to_process.compare_counts is not None` (:16) but read at :32 under a second, separate guard — a latent `UnboundLocalError` shape (same pattern at series_analyzer_cat.py:20 vs :123). | `if to_process.compare_counts is not None:` / `    num_values_compare = updated_dict["compare"]["base_stats"]["num_values"].number` |
| P1-33 | sweetviz/series_analyzer_cat.py:47 | #11 | The BOOL/NUM target-stat block at :47-63 is duplicated near-verbatim at :80-95 (only `target_stats`→`target_stats_compare` and `source_*`→`compare_*` differ), and again in condensed form at :111-121 / :122-133. | `if to_process.predetermined_type_target == FeatureType.TYPE_BOOL:` |
| P1-34 | sweetviz/series_analyzer_cat.py:45 | #41 | Hot: `to_process.source == row["name"]` builds a full-length boolean mask per category inside the loop over `category_counts` — O(rows x categories) where one `groupby` is O(rows). The code's own `# TODO: OPTIMIZE: CACHE FROM GRAPH?` (:40) concedes it. | `this_value_target_only = to_process.source_target[to_process.source == row["name"]]` |
| P1-35 | sweetviz/series_analyzer_numeric.py:82 | #34 | A 10-line commented-out block (:82-91) at module indentation, plus more at :25-26, :61-65, :79-80. | `# detail["frequent_values"] = pd.DataFrame(counts["value_counts_without_nan"].head(num_to_show))` |
| P1-36 | sweetviz/series_analyzer_numeric.py:25 | #39 | History narration: a comment recording that a stat "was unused" and its removed implementation. | `# MAD was unused!!!` / `# stats["mad"] = (series - series.mean()).abs().mean() # deprecated: series.mad()` |
| P1-37 | sweetviz/sv_types.py:51 | none | `FeatureToProcess.__init__` writes `source.name = str(source.name)` on the caller's live pandas Series (and the same for `compare`, `source_target`, `compare_target` at :53-57) — a constructor mutating user-owned data. No inventory rule covers input-argument mutation. | `source.name = str(source.name)` |
| P1-38 | sweetviz/sv_types.py:32 | none | `__int__` returns `self.number`, which is `None` whenever `total_for_percentage == 0` (:25-26); `int(NumWithPercent(...))` then raises `TypeError` instead of returning an int. | `def __int__(self):` / `    return self.number` |
| P1-39 | sweetviz/graph.py:29 | #34 | `Graph.__init__` ends in a bare `return` that does nothing; the same no-op tail appears at graph_legend.py:99, graph_numeric.py:268, graph_associations.py:211, series_analyzer_cat.py:135 and :154. | `return` |
| P1-40 | sweetviz/graph.py:25 | #32 | `Graph.__init__` is never executed: no subclass (`GraphCat`, `GraphNumeric`, `GraphAssoc`, `GraphLegend`) calls `super().__init__()` and nothing instantiates `Graph` directly, so `self.data` (:28) is dead and `self.index_for_css` (:26) is silently missing on `GraphCat` instances that templates read. | `self.index_for_css = "graph"` / `self.data = {}` |
| P1-41 | sweetviz/graph.py:156 | none | `fm.fontManager.ttflist.extend(font_list)` is immediately overwritten by `fm.fontManager.ttflist = font_list` on the next line — the extend can never be observed. | `fm.fontManager.ttflist.extend(font_list)` / `fm.fontManager.ttflist = font_list` |
| P1-42 | sweetviz/graph.py:143 | #11 | The two-line font-dir discovery is copy-pasted into both arms of the same `if` (:143-144 and :148-149) instead of hoisted above it. | `font_dirs = (Path(__file__).parent / "fonts",)` / `font_files = fm.findSystemFonts(fontpaths=font_dirs)` |
| P1-43 | sweetviz/graph.py:79 | #34 | `except Exception: continue` around `get_window_extent` swallows every failure class without naming one. | `except Exception:` / `    continue` |
| P1-44 | sweetviz/graph.py:173 | #13 | `Graph.format_smart` is a forward-only static method to `sv_html_formatters.fmt_smart`, adding only a `pos` parameter it discards. | `def format_smart(x, pos=None):` / `    return sv_html_formatters.fmt_smart(x)` |
| P1-45 | sweetviz/graph.py:1 | #32 | `import matplotlib` is unused — only `matplotlib.pyplot` and `matplotlib.font_manager` are imported separately and used. | `import matplotlib` |
| P1-46 | sweetviz/graph_legend.py:25 | #32 | `to_fractions` is defined and never called; `text1_elem` (:48, :73) and `text2_elem` (:58, :91) are assigned and never read. | `def to_fractions(pos_in_pix):` |
| P1-47 | sweetviz/graph_legend.py:2 | #32 | Four unused imports in a 98-line module: `pandas as pd` (:2), `matplotlib.ticker as mtick` (:5), `sv_html_formatters` (:9), `PercentFormatter` (:12); `FeatureToProcess` (:10) is unused too. | `import pandas as pd` |
| P1-48 | sweetviz/graph_legend.py:87 | #34 | Dangling commented-out fragments (`#+ f" ({...})"` at :87 and :90) and a three-line commented block at :93-95. | `#+ f" ({dataframe_report.compare_name})"` |
| P1-49 | sweetviz/graph_cat.py:135 | #34 | Bare `except: pass` wraps `tick_names.index(OTHERS_GROUPED)` — swallows every error to express "value may be absent". | `except:` / `    pass` |
| P1-50 | sweetviz/graph_cat.py:13 | #1 | `plot_grouped_bars` takes opaque `**kwargs` forwarded straight into four different matplotlib calls, so no caller can learn what it accepts. | `def plot_grouped_bars(tick_names: List[str], data_lists: List[List], \` / `        colors: List[str], gap_percent: float, axis_obj = None, \` / `        orientation: str = 'vertical', **kwargs):` |
| P1-51 | sweetviz/graph_cat.py:33 | #11 | The vertical/horizontal bar pair at :33-40 (axis_obj arm) is duplicated at :43-50 (plt arm); the four bodies differ only in the receiver. | `if orientation == 'vertical':` / `    plt.xticks(locations_centered, tick_names)` |
| P1-52 | sweetviz/graph_cat.py:154 | #11 | The per-tick target-average loop at :154-163 is duplicated at :176-184 (compare), and the BOOL variant at :191-200 is duplicated at :216-223 — four copies of one loop shape in one method. | `for name in tick_names:` / `    if name == OTHERS_GROUPED:` |
| P1-53 | sweetviz/graph_cat.py:145 | #32 | `bar_width` is unpacked from `plot_grouped_bars` and never read; correspondingly the second element of that return (:53) is dead at its only call site. | `category_centers, bar_width = \` |
| P1-54 | sweetviz/graph_cat.py:64 | #8 | `which_graph: str` encodes two facts (kind + bin count) in one primitive, re-parsed by `== "mini"`, `.find("detail") != -1` and `.split("-")` at graph_cat.py:58,64,67,76 and graph_numeric.py:15,21,22,32,41. A type would validate it once. | `is_detail = which_graph.find("detail") != -1` |
| P1-55 | sweetviz/graph_cat.py:203 | #34 | Two commented-out blocks in one method: :203-208 (6 lines) and :227-233 (7 lines), plus :52, :144, :201, :224. | `# ax2 = axs.twiny()` |
| P1-56 | sweetviz/graph_numeric.py:146 | none | `source_bins_series = source_bins_series.fillna(...)` inside the compare branch re-fills the *source* series (already filled at :119) instead of `compare_bins_series` — a copy-paste bug and a provable no-op. Same line repeated at :207. | `source_bins_series = source_bins_series.fillna(num_bins - 1)` |
| P1-57 | sweetviz/graph_numeric.py:227 | #34 | A 25-line commented-out block (:227-251) inside `__init__`, plus :102, :180, :183, :265-266. | `# elif to_process.compare is not None:` |
| P1-58 | sweetviz/graph_numeric.py:92 | none | `warnings.filterwarnings("once", ...)` at :92 is written as a restore of the `"ignore"` set at :83, but it installs a different global filter rather than restoring the previous state; `warnings.catch_warnings()` is the honest form. | `warnings.filterwarnings(` / `    "once", category=np.exceptions.VisibleDeprecationWarning` |
| P1-59 | sweetviz/graph_numeric.py:14 | #18 | `GraphNumeric.__init__` narrates its phases in banner comments — `# MAIN DATA ("Under" target)` (:57), `# TARGET` (:109), `# Finalize Graph` (:255) — three labeled phases that are function boundaries spelled in prose. | `# MAIN DATA ("Under" target)` / `# ---------------------------------------------` |
| P1-60 | sweetviz/graph_associations.py:425 | #32 | `filter_best_corr` is never called (its only mention is the commented-out :446); inside it, `ordered` (:435) is computed and dropped and the function returns nothing. | `def filter_best_corr(correlation_dataframe):` |
| P1-61 | sweetviz/graph_associations.py:124 | #19 | `combined.index(feature)` is a linear list scan performed inside a doubly-nested loop over the same list — O(n^3) where the outer loop already knows the index. Repeated at :144, :160, :162, :200. | `for associated_feature_name in combined:` / `    graph_data.at[combined.index(feature), associated_feature_name] = \` |
| P1-62 | sweetviz/graph_associations.py:119 | #11 | The nested fill loop is copy-pasted four times in one constructor: :119-128 (all), :139-148 (cat-cat), :154-166 (num-num), :195-204 (cat-num). | `for feature in combined:` / `    for associated_feature_name in combined:` |
| P1-63 | sweetviz/graph_associations.py:230 | #1 | `heatmap(y, x, figure_size, **kwargs)` hides eleven required/optional inputs in `**kwargs`, including `dataframe_report`, which is not optional — it is indexed at :363 and raises `KeyError` if absent. | `def heatmap(y, x, figure_size, **kwargs):` |
| P1-64 | sweetviz/graph_associations.py:329 | #32 | `kwargs_pass_on` (:329) is built and never used — its only consumer is the commented-out `ax.scatter` at :392-399. `marker` (:327) is likewise assigned and never read. | `kwargs_pass_on = {k:v for k,v in kwargs.items() if k not in [` |
| P1-65 | sweetviz/graph_associations.py:349 | #32 | `delta_in_pix` is computed at :349 and unconditionally overwritten at :369 before any read — a dead store. | `delta_in_pix = ax.transData.transform((1, 1)) - ax.transData.transform((0, 0))` |
| P1-66 | sweetviz/graph_associations.py:353 | #41 | Hot: `do_wrapping` re-wraps both axis labels per heatmap cell — the loop runs over all n^2 feature pairs, and `wrap_custom` is a per-character Python loop. The wrapped names already exist as `x_names`/`y_names` (:308, :317); a dict lookup replaces the recomputation. | `wrapped_x_name = do_wrapping(cur_x, wrap_x)` / `wrapped_y_name = do_wrapping(cur_y, wrap_y)` |
| P1-67 | sweetviz/graph_associations.py:304 | #12 | Four identity comprehensions in fifteen lines reimplement `list()` / `sorted(set(...))` / `.values()`: :304, :306, :313, :315, :334, :335, :344, :345. | `x_names = [t for t in kwargs['x_order']]` / `x_names = [t for t in sorted(set([v for v in x]))]` |
| P1-68 | sweetviz/graph_associations.py:10 | #32 | `from textwrap import wrap` is unused — its only mention is the commented-out :300. | `from textwrap import wrap` |
| P1-69 | sweetviz/graph_associations.py:186 | none | `DataFrame.append` was removed in pandas 2.0; the `cat-num` branch (:186, :189) calls it, so that graph raises `AttributeError`. The same deprecation was already fixed in utils.py:36 but not here — the one-fix-many-copies failure. | `graph_data = graph_data.append(pd.Series(empty_row_dict, name=categorical))` |
| P1-70 | sweetviz/graph_associations.py:147 | none | Only the `"all"` branch sets the index to `UNIQUE_INDEX_NAME`; `cat-cat` (:147), `num-num` (:165) and `cat-num` (:203) name it `'index'`, yet `corrplot` unconditionally melts on `id_vars=UNIQUE_INDEX_NAME` (:448) — a `KeyError` for those three graph kinds. | `graph_data['index'] = categoricals` |
| P1-71 | sweetviz/graph_associations.py:437 | #37 | `corrplot`'s `size_scale=100` and `marker='s'` defaults are never overridden and `size_scale` is never read — the real value is fetched from config at :478 and `marker` is discarded inside `heatmap`. | `def corrplot(correlation_dataframe, dataframe_report, size_scale=100, marker='s'):` |
| P1-72 | sweetviz/graph_associations.py:438 | #39 | A 9-line pasted REPL transcript sits as a comment above the first statement, and another 15-line transcript at :450-466 — prose that outweighs the four-line function body. | `#              PassengerId  Survived    Pclass  ...     SibSp     Parch      Fare` |
| P1-73 | sweetviz/sv_html.py:27 | #32 | A leftover debug global `hello = "Superduper"` is registered on the Jinja environment and referenced by no template. | `jinja2_env.globals["hello"] = "Superduper"` |
| P1-74 | sweetviz/sv_html.py:474 | #32 | `generate_html_detail_target_numeric` (:474) and `generate_html_detail_target_cat` (:483) are unreferenced; the file labels them itself. | `#UNUSED yet:` / `def generate_html_detail_target_numeric(feature_dict: dict, compare_dict: dict):` |
| P1-75 | sweetviz/sv_html.py:471 | #39 | The marker comment `#UNUSED yet:` is repeated three times in a row at :471-473 and again at :480-482 — six comment lines conveying one fact. | `#UNUSED yet:` / `#UNUSED yet:` / `#UNUSED yet:` |
| P1-76 | sweetviz/sv_html.py:46 | #34 | `render_index` (:46-47) and the `summary_pos` it computes (:48) are dead: the next line unconditionally overwrites the field with `0.0`. Three of the four lines in the loop body do nothing. | `feature["summary_pos"] = render_index * config["Layout"].getint("summary_spacing")` / `feature["summary_pos"] = 0.0` |
| P1-77 | sweetviz/sv_html.py:175 | #11 | `generate_html_summary_text` (:175-228) and `generate_html_detail_text` (:414-468) are a ~45-line near-verbatim clone pair — they differ only in template name, three config keys and `summary_count`/`detail_count`. | `full_list = feature_dict["detail"]["full_count"]` / `feature_dict["detail"]["summary_count"] = full_list[:max_text_rows]` |
| P1-78 | sweetviz/sv_html.py:175 | #6 | `generate_html_summary_text` is named as a pure renderer returning a string, but writes `feature_dict["detail"]["summary_count"]` (:197), truncates every `elem["name"]` in place (:203) and appends a synthetic row (:224). Same hidden-effect shape in `generate_html_detail_cat` (:367). | `elem["name"] = elem["name"][:max_text_display_length]` |
| P1-79 | sweetviz/sv_html.py:271 | #32 | `spacing` is read from config and never used in `generate_html_detail_numeric`; `max_num` (:305) is likewise assigned only in the dead `else` arm. | `spacing = config["Layout"].getint("cat_detail_col_spacing")` |
| P1-80 | sweetviz/sv_html.py:138 | #12 | `np.isnan(...) == False` compares a bool to a literal instead of negating it. | `if np.isnan(feature_dict["stats"]["range"]) and \` / `     np.isnan(compare_dict["stats"]["range"]) == False:` |
| P1-81 | sweetviz/sv_html.py:99 | #1 | The whole render layer publishes bare `dict` on its boundary — `feature_dict: dict, compare_dict: dict` at :99, :131, :169, :175, :232, :240, :265, :315, :414 — nine public signatures where the contract is a magic-string bag. | `def create_summary_numeric_group_data(feature_dict: dict, compare_dict: dict):` |
| P1-82 | sweetviz/sv_html.py:12 | #9 | `jinja2_env` is a module-level mutable mutated from another module: `load_layout_globals_from_config` (:29-39) rewrites its `globals` and is called four times from dataframe_report.py (:43, :544, :585, :620). Render output depends on call order. | `jinja2_env = Environment(lstrip_blocks = True,` |
| P1-83 | sweetviz/sv_html.py:34 | #26 | The template's layout constants are assembled by iterating the config section rather than declared, so no reader (or grep) can see which globals templates may reference. | `for element in config["Layout"]:` / `    layout_globals[element] = config["Layout"].getint(element)` |
| P1-84 | sweetviz/sv_html_formatters.py:62 | #11 | `fmt_smart` (:62-89) and `fmt_smart_range` (:103-129) are the same eleven-branch magnitude ladder with the threshold variable renamed; `fmt_smart_range_tight` (:131-160) is a third copy with two extra rungs. | `elif absolute < 0.001:` / `    return f"{Decimal(float(value)):.2e}"` |
| P1-85 | sweetviz/sv_html_formatters.py:103 | none | The parameter named `range` shadows the builtin throughout `fmt_smart_range` and `fmt_smart_range_tight` (:131). | `def fmt_smart_range(value: float, range: float) -> str:` |
| P1-86 | sweetviz/dataframe_report.py:16 | #32 | `from sweetviz.config import config` is imported twice in the same header, at :16 and again at :21. | `from sweetviz.config import config` |
| P1-87 | sweetviz/dataframe_report.py:127 | #32 | `exponential_checks` is computed and never read — the warning at :177 recomputes `number_features * number_features` inline. | `exponential_checks = number_features * number_features` |
| P1-88 | sweetviz/dataframe_report.py:129 | #34 | `(0 if target_feature_name is not None else 0)` — both arms are `0`, so the whole conditional is a no-op the reader must still evaluate. | `progress_chunks = ratio_progress_of_df_summary_vs_feature \` / `                    + number_features + (0 if target_feature_name is not None else 0)` |
| P1-89 | sweetviz/dataframe_report.py:335 | #32 | `DataframeReport.get_predetermined_type` is never called (all three call sites use `FeatureConfig.get_predetermined_type`), and both of its branches return the same constant, so the `if` is dead inside a dead method. | `if feature_predetermined_types is None:` / `    return sa.FeatureType.TYPE_UNSUPPORTED` / `return sa.FeatureType.TYPE_UNSUPPORTED` |
| P1-90 | sweetviz/dataframe_report.py:343 | #33 | `sanitize_bool` is annotated `-> bool` but `if value is bool: return value` returns the unconstrained input — and the test itself is an identity check against the `bool` *type*, which no real boolean satisfies, so the arm is both a type lie and unreachable. | `def sanitize_bool(value) -> bool:` / `    if value is bool:` / `        return value` |
| P1-91 | sweetviz/dataframe_report.py:358 | #33 | `get_target_type(self) -> FeatureType` returns `None` when there is no target (:359-360); `get_type(...) -> FeatureType` (:363) does the same at :368. Every caller must re-derive that the contract is optional. | `def get_target_type(self) -> FeatureType:` / `    if self._target is None:` / `        return None` |
| P1-92 | sweetviz/dataframe_report.py:246 | #12 | `zip(seq, range(0, len(seq)))` reimplements `enumerate(seq)` — and swaps the conventional order while doing it. | `for cur_series_name, cur_order_index in zip(filtered_series_names_in_source,` / `                                         range(0, len(filtered_series_names_in_source))):` |
| P1-93 | sweetviz/dataframe_report.py:190 | #12 | A list comprehension filtering for equality reimplements a membership test; the result is then only used as `targets_found[0]`, which is `target_feature_name` itself. | `targets_found = [item for item in filtered_series_names_in_source` / `                 if item == target_feature_name]` |
| P1-94 | sweetviz/dataframe_report.py:83 | #12 | `[cur_name for cur_name, cur_series in source_df.items()]` materialises every column's data to collect names, where `list(df.columns)` is the vocabulary word. Repeated at :93 and :101. | `all_source_names = [cur_name for cur_name, cur_series in source_df.items()]` |
| P1-95 | sweetviz/dataframe_report.py:544 | #11 | The nine-line "load globals → set layout/scale → set positions → generate detail → generate associations → generate page" sequence is copy-pasted four times: :528-537, :544-553, :585-594, :620-629. | `sv_html.load_layout_globals_from_config()` / `self.page_layout = layout` / `self.scale = scale` |
| P1-96 | sweetviz/dataframe_report.py:555 | #12 | Manual `open`/`write`/`close` instead of a `with` block, so the handle leaks if `write` raises. Same three lines at :631-633. | `f = open(filepath, 'w', encoding="utf-8")` / `f.write(self._page_html)` / `f.close()` |
| P1-97 | sweetviz/dataframe_report.py:131 | #12 | `DummyFile` hand-rolls a null sink that `io.StringIO`, `open(os.devnull, "w")` or tqdm's own `disable=True` already provide. | `class DummyFile(object):` / `    def write(self, x):` / `        pass  # Do nothing` |
| P1-98 | sweetviz/dataframe_report.py:441 | #41 | Hot: the association loop iterates every ordered pair and recomputes each symmetric measure twice, even though `mirror_association` (:449, :473, :503) has already written the reverse entry. The skip that would halve the work is present but commented out at :444-446. The module itself warns this path is quadratic (:173-180). | `for other in features_to_process:` / `# for other in [of for of in features_to_process if of.source.name != feature_name]:` |
| P1-99 | sweetviz/dataframe_report.py:121 | #19 | `skipped not in all_source_names and skipped not in all_compare_names` are list scans inside a loop over the skip list; `key not in all_source_names` at :114 is the same shape. Sets make both O(1). | `for skipped in fc.skip:` / `    if skipped not in all_source_names and skipped not in all_compare_names:` |
| P1-100 | sweetviz/dataframe_report.py:249 | #19 | `cur_series_name in compare_df.columns` is an Index scan performed once per feature — O(features^2) over the column list. | `if compare_df is not None and cur_series_name in \` / `        compare_df.columns:` |
| P1-101 | sweetviz/dataframe_report.py:371 | #22 | `summarize_dataframe` never touches `self` — it is a free function hiding in the class. `use_config_if_none` (:522) is the same: no `self` in the body. | `def  summarize_dataframe(self, source: pd.DataFrame, name: str, target_dict: dict, skip: List[str]):` |
| P1-102 | sweetviz/dataframe_report.py:327 | #21 | The `self._target is not None` / `self._target["name"]` / `self._target["type"]` guard-and-index trio is re-established in `__getitem__` (:327), `get_target_type` (:359), `get_type` (:365) and `__init__` (:231, :238) — an invariant enforced at every call site instead of in a type. | `elif self._target is not None and key == self._target["name"]:` |
| P1-103 | sweetviz/dataframe_report.py:31 | #18 | `DataframeReport.__init__` narrates thirteen labeled phases in banner comments (:31, :36, :61, :105, :117, :124, :147, :170, :184, :225, :244, :269, :284, :292) across 294 lines — every one of them a function boundary spelled in prose. | `# Parse analysis parameter` / `# Parse verbosity parameter` |
| P1-104 | sweetviz/dataframe_report.py:1 | #29 | 653-line module, the package's core, with no module docstring — and its 294-line entry point `__init__` carries no cost note despite performing the quadratic association pass it warns about at :173. | `from typing import Union, List, Tuple` |
| P1-105 | sweetviz/dataframe_report.py:24 | #27 | `DataframeReport` is the hottest symbol in the package (re-exported by `__init__.py:20`, constructed by all three public API functions) yet lives in the largest module — any agent touching the entry point ingests 653 lines to reach it. | `class DataframeReport:` |
| P1-106 | sweetviz/dataframe_report.py:151 | #39 | Dated changelog entries left as code comments: `# UPDATE 2021-02-05: Count the target as an actual feature!!! It is!!!` at :151 and :160, `# NEW (12-14-2020): ...` at :75 (and feature_config.py:19). git already holds this. | `# UPDATE 2021-02-05: Count the target as an actual feature!!! It is!!!` |
| P1-107 | sweetviz/dataframe_report.py:652 | #34 | `log_comet` ends in a bare `except:` that prints and discards — the fifth swallow-with-print in the package. | `except:` / `    print("log_comet(): error logging HTML report.")` |
| P1-108 | sweetviz/dataframe_report.py:305 | none | `associations_html_source` is assigned `True` here and a rendered HTML string at :534/:550 — one attribute carrying two unrelated types, used as both a flag and a payload. | `self.associations_html_source = True # Generated later in the process` |
| P1-109 | sweetviz/dataframe_report.py:543 | none | `f"'layout' parameter must be either 'widescreen' or 'vertical'"` is an f-string with no placeholders; same at :583 and :619. | `raise ValueError(f"'layout' parameter must be either 'widescreen' or 'vertical'")` |
| P1-110 | sweetviz/dataframe_report.py:319 | #1 | `verbose_print(self, *args, **kwargs)` publishes a fully opaque signature on a public method. | `def verbose_print(self, *args, **kwargs):` |
| P1-111 | sweetviz/graph.py:11 | #35 | Import cycle: `graph` → `sv_html_formatters` (:11) → `graph_associations` (sv_html_formatters.py:3) → `graph` (graph_associations.py:6). No member of the trio can be imported alone. | `from sweetviz import sv_html_formatters` |
| P1-112 | sweetviz/sv_html.py:83 | #35 | `sweetviz.__version__` is read here although `sv_html.py` never imports `sweetviz` — the name is only bound as a side effect of `import sweetviz.sv_html_formatters` (:5), and resolves only because the package `__init__` has already run. A second, hidden edge of the same cycle. | `output = template.render(dataframe=dataframe_report, version=sweetviz.__version__)` |
| P1-113 | sweetviz/sv_public.py:4 | #35 | `sv_public` imports `sweetviz.dataframe_report` but then calls `sweetviz.DataframeReport` — the package attribute, not the module it imported — so the module is loaded through a cycle back into its own `__init__`. Repeated at :22 and :46. | `import sweetviz.dataframe_report` / `report = sweetviz.DataframeReport(source, target_feat, None,` |
| P1-114 | sweetviz/sv_public.py:8 | #1 | The three public API entry points annotate `target_feat: str = None` and `feat_cfg: FeatureConfig = None` — non-Optional annotations whose declared default violates them, at the library's published boundary. | `def analyze(source: Union[pd.DataFrame, Tuple[pd.DataFrame, str]],` / `            target_feat: str = None,` |
| P1-115 | sweetviz/sv_public.py:8 | #29 | The package's entire public surface (`analyze`, `compare`, `compare_intra`) carries no docstrings at all, so the API an agent is most likely to be asked about is undocumented in-tree. | `def analyze(source: Union[pd.DataFrame, Tuple[pd.DataFrame, str]],` |
| P1-116 | sweetviz/sv_public.py:17 | none | The parameter `compare` shadows the enclosing function name `compare`, making the function unable to recurse or refer to itself. | `def compare(source: Union[pd.DataFrame, Tuple[pd.DataFrame, str]],` / `            compare: Union[pd.DataFrame, Tuple[pd.DataFrame, str]],` |
| P1-117 | sweetviz/sv_public.py:12 | #11 | `analyze` (:12-14) and `compare` (:22-24) are the same two-statement body differing only in which argument is passed as the compare frame. | `report = sweetviz.DataframeReport(source, target_feat, None,` |
| P1-118 | sweetviz/feature_config.py:11 | #12 | `type(param) == list or type(param) == tuple` reimplements `isinstance(param, (list, tuple))` and rejects subclasses; the same `type(x) ==` shape recurs at dataframe_report.py:62, :65, :88, :94 and graph_cat.py:142. | `if type(param) == list or type(param) == tuple:` |
| P1-119 | sweetviz/feature_config.py:7 | #1 | `skip`, `force_cat`, `force_text`, `force_num` are annotated bare `Tuple` (unparameterised) on the public config object while the body actually accepts list, tuple, str or None. | `def __init__(self, skip: Tuple = None,` / `        force_cat: Tuple = None, force_text: Tuple = None,` |
| P1-120 | sweetviz/feature_config.py:23 | #11 | Four identical `rename_index(make_list(...))` lines (:23-26) and four identical `returned.extend(...)` lines (:42-45) encode the same four-field list twice; adding a fifth force-type means editing both blocks. | `self.skip = rename_index(make_list(skip))` |
| P1-121 | sweetviz/update_jquery.py:15 | #26 | The jQuery version to install and the path it patches are executable module-level statements rather than a declared constant table, and the whole script is a bare try/except at import scope — importing this module performs a network download. | `old_version = "3.4.1"` / `new_version = "3.7.1"` |
| P1-122 | sweetviz/update_jquery.py:40 | #34 | `except Exception as ex: print(...)` swallows every failure of the download-and-patch and exits 0, so a broken update reports success. | `except Exception as ex:` / `    print("Error in updating jQuery; old version not replaced.")` |
| P1-123 | sweetviz/graph_associations.py:1 | #29 | 481-line module with no module docstring; its first 45 lines are a vendored license block, so the first screen tells a reader nothing about what the module does. | `import math` |
| P1-124 | sweetviz/sv_html.py:1 | #29 | 486-line render module, no module docstring, and no function in it is documented — the boundary between the report object and the Jinja templates is entirely undescribed. | `import numpy as np` |

## Phase 2 — audit finding verdicts

354 findings judged at their sites. 294 `real`, 60 `fp`. Per-rule precision:
#1 65/65, #2 4/26, #4 0/5, #10 10/10, #11 55/61, #12 20/20, #13 1/1, #14 2/2,
#15 3/3, #18 8/8, #19 3/3, #21 1/2, #22 5/5, #23 21/22, #27 11/11, #28 0/1,
#29 28/32, #32 25/29, #33 4/4, #34 18/18, #35 1/1, #39 9/22, #40 0/3.

Two findings share `dataframe_report.py:23` + `#21` and differ only in message;
four pairs in `#12` share file:line+rule and differ only in `symbol`
(`dataframe_report.py:422`, `:425`). Both survived dedupe, so each is given its
own row here in message order.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| sweetviz/dataframe_report.py:26 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/dataframe_report.py:27 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/dataframe_report.py:29 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/dataframe_report.py:319 | #1 | heuristic | real | callers cannot learn what is accepted, and heatmap even requires a kwarg (dataframe_report) that the signature never names |
| sweetviz/dataframe_report.py:337 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/dataframe_report.py:371 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/dataframe_report.py:387 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/dataframe_report.py:405 | #1 | heuristic | real | returns an untyped magic-string bag that every caller must re-derive |
| sweetviz/feature_config.py:7 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/feature_config.py:8 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/feature_config.py:8 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/feature_config.py:9 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/from_profiling_pandas.py:35 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/from_profiling_pandas.py:62 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/from_profiling_pandas.py:85 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/from_profiling_pandas.py:95 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/graph_associations.py:230 | #1 | heuristic | real | callers cannot learn what is accepted, and heatmap even requires a kwarg (dataframe_report) that the signature never names |
| sweetviz/graph_cat.py:15 | #1 | heuristic | real | callers cannot learn what is accepted, and heatmap even requires a kwarg (dataframe_report) that the signature never names |
| sweetviz/series_analyzer.py:9 | #1 | heuristic | real | returns an untyped magic-string bag that every caller must re-derive |
| sweetviz/series_analyzer.py:43 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer.py:43 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer.py:61 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer.py:61 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer.py:61 | #1 | heuristic | real | returns an untyped magic-string bag that every caller must re-derive |
| sweetviz/series_analyzer.py:79 | #1 | heuristic | real | returns an untyped magic-string bag that every caller must re-derive |
| sweetviz/series_analyzer_cat.py:9 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer_cat.py:137 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer_numeric.py:9 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer_numeric.py:31 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer_numeric.py:31 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer_numeric.py:31 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer_numeric.py:94 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer_text.py:6 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/series_analyzer_text.py:39 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:99 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:99 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:131 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:131 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:169 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:169 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:175 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:175 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:232 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:232 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:240 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:240 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:265 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:265 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:315 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:315 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:414 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:414 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:474 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:474 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:483 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_html.py:483 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/sv_public.py:9 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/sv_public.py:10 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/sv_public.py:19 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/sv_public.py:20 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/sv_public.py:30 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/sv_public.py:31 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/sv_types.py:46 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/sv_types.py:47 | #1 | heuristic | real | implicit-Optional at a published boundary: the annotation excludes the value the declared default supplies |
| sweetviz/type_detection.py:6 | #1 | heuristic | real | the parameter is an untyped magic-string bag; the contract lives nowhere |
| sweetviz/dataframe_report.py:52 | #2 | heuristic | fp | the check is live at runtime: the declared default is None, so the no-overlap claim rests on the wrong annotation rather than a redundant guard |
| sweetviz/dataframe_report.py:65 | #2 | heuristic | fp | the list arm is reached at runtime - sv_public.compare_intra passes a list; the Union annotation is wrong, not the branch |
| sweetviz/dataframe_report.py:84 | #2 | heuristic | fp | the check is live at runtime: the declared default is None, so the no-overlap claim rests on the wrong annotation rather than a redundant guard |
| sweetviz/dataframe_report.py:94 | #2 | heuristic | fp | the list arm is reached at runtime - sv_public.compare_intra passes a list; the Union annotation is wrong, not the branch |
| sweetviz/dataframe_report.py:129 | #2 | heuristic | fp | the check is live at runtime: the declared default is None, so the no-overlap claim rests on the wrong annotation rather than a redundant guard |
| sweetviz/dataframe_report.py:286 | #2 | heuristic | fp | the check is live at runtime: the declared default is None, so the no-overlap claim rests on the wrong annotation rather than a redundant guard |
| sweetviz/dataframe_report.py:338 | #2 | proved | real | the param is annotated bare dict with no default and the method is never called - the guard is dead, and both branches return the same constant |
| sweetviz/from_dython.py:89 | #2 | heuristic | real | x was rebound to a list comprehension one line above, so the else branch is genuinely unreachable regardless of annotations |
| sweetviz/graph_cat.py:142 | #2 | heuristic | fp | pyright typed the index elements as int, but utils.get_clamped_value_counts genuinely inserts OTHERS_GROUPED into tick_names, so the comparison is live |
| sweetviz/graph_cat.py:155 | #2 | heuristic | fp | pyright typed the index elements as int, but utils.get_clamped_value_counts genuinely inserts OTHERS_GROUPED into tick_names, so the comparison is live |
| sweetviz/graph_cat.py:157 | #2 | heuristic | fp | pyright typed the index elements as int, but utils.get_clamped_value_counts genuinely inserts OTHERS_GROUPED into tick_names, so the comparison is live |
| sweetviz/graph_cat.py:178 | #2 | heuristic | fp | pyright typed the index elements as int, but utils.get_clamped_value_counts genuinely inserts OTHERS_GROUPED into tick_names, so the comparison is live |
| sweetviz/graph_cat.py:192 | #2 | heuristic | fp | pyright typed the index elements as int, but utils.get_clamped_value_counts genuinely inserts OTHERS_GROUPED into tick_names, so the comparison is live |
| sweetviz/graph_cat.py:194 | #2 | heuristic | fp | pyright typed the index elements as int, but utils.get_clamped_value_counts genuinely inserts OTHERS_GROUPED into tick_names, so the comparison is live |
| sweetviz/graph_cat.py:217 | #2 | heuristic | fp | pyright typed the index elements as int, but utils.get_clamped_value_counts genuinely inserts OTHERS_GROUPED into tick_names, so the comparison is live |
| sweetviz/series_analyzer.py:83 | #2 | heuristic | real | self.source is assigned unconditionally in FeatureToProcess.__init__, so the None arm is genuinely dead |
| sweetviz/series_analyzer_numeric.py:46 | #2 | heuristic | fp | the check is live at runtime: the declared default is None, so the no-overlap claim rests on the wrong annotation rather than a redundant guard |
| sweetviz/sv_html.py:136 | #2 | heuristic | fp | the check is live at runtime: the declared default is None, so the no-overlap claim rests on the wrong annotation rather than a redundant guard |
| sweetviz/sv_html.py:184 | #2 | heuristic | fp | the check is live at runtime: the declared default is None, so the no-overlap claim rests on the wrong annotation rather than a redundant guard |
| sweetviz/sv_html.py:214 | #2 | heuristic | fp | the check is live at runtime: the declared default is None, so the no-overlap claim rests on the wrong annotation rather than a redundant guard |
| sweetviz/sv_html_formatters.py:13 | #2 | proved | fp | the None guard is live - jinja passes NumWithPercent.number, which is None when the total is 0; the annotation is wrong, not the check |
| sweetviz/sv_html_formatters.py:41 | #2 | proved | fp | the None guard is live - jinja passes NumWithPercent.number, which is None when the total is 0; the annotation is wrong, not the check |
| sweetviz/sv_html_formatters.py:53 | #2 | proved | fp | the None guard is live - jinja passes NumWithPercent.number, which is None when the total is 0; the annotation is wrong, not the check |
| sweetviz/sv_html_formatters.py:163 | #2 | proved | fp | the None guard is live - jinja passes NumWithPercent.number, which is None when the total is 0; the annotation is wrong, not the check |
| sweetviz/sv_html_formatters.py:168 | #2 | proved | fp | the None guard is live - jinja passes NumWithPercent.number, which is None when the total is 0; the annotation is wrong, not the check |
| sweetviz/sv_types.py:91 | #2 | heuristic | real | self.source is assigned unconditionally in FeatureToProcess.__init__, so the None arm is genuinely dead |
| sweetviz/dataframe_report.py:523 | #4 | indexed | fp | the callers do not establish it - every discovered caller passes feature_dict.get('compare') or an Optional-defaulted argument, which is None on the no-compare path |
| sweetviz/sv_html.py:102 | #4 | indexed | fp | the callers do not establish it - every discovered caller passes feature_dict.get('compare') or an Optional-defaulted argument, which is None on the no-compare path |
| sweetviz/sv_html.py:355 | #4 | indexed | fp | the callers do not establish it - every discovered caller passes feature_dict.get('compare') or an Optional-defaulted argument, which is None on the no-compare path |
| sweetviz/sv_html.py:423 | #4 | indexed | fp | the callers do not establish it - every discovered caller passes feature_dict.get('compare') or an Optional-defaulted argument, which is None on the no-compare path |
| sweetviz/sv_html.py:454 | #4 | indexed | fp | the callers do not establish it - every discovered caller passes feature_dict.get('compare') or an Optional-defaulted argument, which is None on the no-compare path |
| sweetviz/dataframe_report.py:371 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/dataframe_report.py:387 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/from_profiling_pandas.py:35 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/from_profiling_pandas.py:62 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/from_profiling_pandas.py:85 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/from_profiling_pandas.py:95 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/series_analyzer.py:43 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/series_analyzer.py:61 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/series_analyzer_numeric.py:31 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/sv_html.py:99 | #10 | indexed | real | the body only indexes or iterates the argument; the widening was machine-verified, so the concrete type is a demand the code never makes |
| sweetviz/dataframe_report.py:531 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.generate_comet_friendly_html, sweetviz.dataframe_repo |
| sweetviz/dataframe_report.py:540 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/dataframe_report.py:540 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/dataframe_report.py:547 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.generate_comet_friendly_html, sweetviz.dataframe_repo |
| sweetviz/dataframe_report.py:565 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/dataframe_report.py:573 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/dataframe_report.py:580 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/dataframe_report.py:588 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.generate_comet_friendly_html, sweetviz.dataframe_repo |
| sweetviz/dataframe_report.py:616 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/dataframe_report.py:616 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/dataframe_report.py:623 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.generate_comet_friendly_html, sweetviz.dataframe_repo |
| sweetviz/dataframe_report.py:636 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/dataframe_report.py:644 | #11 | indexed | real | genuine copy - sweetviz.dataframe_report.DataframeReport.show_html, sweetviz.dataframe_report.DataframeReport. |
| sweetviz/graph_associations.py:110 | #11 | indexed | real | genuine copy - sweetviz.graph_associations.GraphAssoc.__init__ |
| sweetviz/graph_associations.py:139 | #11 | indexed | real | genuine copy - sweetviz.graph_associations.GraphAssoc.__init__ |
| sweetviz/graph_associations.py:195 | #11 | indexed | real | genuine copy - sweetviz.graph_associations.GraphAssoc.__init__ |
| sweetviz/graph_associations.py:209 | #11 | indexed | real | genuine copy - sweetviz.graph_associations.GraphAssoc.__init__ |
| sweetviz/graph_associations.py:303 | #11 | indexed | real | genuine copy - sweetviz.graph_associations.heatmap |
| sweetviz/graph_associations.py:312 | #11 | indexed | real | genuine copy - sweetviz.graph_associations.heatmap |
| sweetviz/graph_cat.py:80 | #11 | indexed | real | genuine copy - sweetviz.graph_cat.GraphCat.__init__ |
| sweetviz/graph_cat.py:92 | #11 | indexed | real | genuine copy - sweetviz.graph_cat.GraphCat.__init__ |
| sweetviz/graph_cat.py:243 | #11 | indexed | real | genuine copy - sweetviz.graph_cat.GraphCat.__init__, sweetviz.graph_numeric.GraphNumeric.__init__ |
| sweetviz/graph_legend.py:67 | #11 | indexed | real | genuine copy - sweetviz.graph_legend.GraphLegend.__init__ |
| sweetviz/graph_legend.py:83 | #11 | indexed | real | genuine copy - sweetviz.graph_legend.GraphLegend.__init__ |
| sweetviz/graph_numeric.py:262 | #11 | indexed | real | genuine copy - sweetviz.graph_cat.GraphCat.__init__, sweetviz.graph_numeric.GraphNumeric.__init__ |
| sweetviz/series_analyzer_cat.py:10 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.series_analyzer_text.do_detail_tex |
| sweetviz/series_analyzer_cat.py:31 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.series_analyzer_text.do_detail_tex |
| sweetviz/series_analyzer_cat.py:33 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical |
| sweetviz/series_analyzer_cat.py:33 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.series_analyzer_text.do_detail_tex |
| sweetviz/series_analyzer_cat.py:50 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical |
| sweetviz/series_analyzer_cat.py:82 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical |
| sweetviz/series_analyzer_cat.py:104 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.sv_html.generate_html_detail_text, |
| sweetviz/series_analyzer_cat.py:106 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical |
| sweetviz/series_analyzer_cat.py:106 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.series_analyzer_text.do_detail_tex |
| sweetviz/series_analyzer_cat.py:138 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.analyze, sweetviz.series_analyzer_text.analyze |
| sweetviz/series_analyzer_text.py:7 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.series_analyzer_text.do_detail_tex |
| sweetviz/series_analyzer_text.py:22 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.series_analyzer_text.do_detail_tex |
| sweetviz/series_analyzer_text.py:24 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.series_analyzer_text.do_detail_tex |
| sweetviz/series_analyzer_text.py:40 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.analyze, sweetviz.series_analyzer_text.analyze |
| sweetviz/sv_html.py:106 | #11 | indexed | fp | a literal display table of appended row dicts - a declarative list, not a fact with a home; extracting it would fight #26 declaration-literalness |
| sweetviz/sv_html.py:107 | #11 | indexed | fp | a literal display table of appended row dicts - a declarative list, not a fact with a home; extracting it would fight #26 declaration-literalness |
| sweetviz/sv_html.py:116 | #11 | indexed | fp | a literal display table of appended row dicts - a declarative list, not a fact with a home; extracting it would fight #26 declaration-literalness |
| sweetviz/sv_html.py:120 | #11 | indexed | fp | a literal display table of appended row dicts - a declarative list, not a fact with a home; extracting it would fight #26 declaration-literalness |
| sweetviz/sv_html.py:121 | #11 | indexed | fp | a literal display table of appended row dicts - a declarative list, not a fact with a home; extracting it would fight #26 declaration-literalness |
| sweetviz/sv_html.py:126 | #11 | indexed | fp | a literal display table of appended row dicts - a declarative list, not a fact with a home; extracting it would fight #26 declaration-literalness |
| sweetviz/sv_html.py:169 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_summary_cat, sweetviz.sv_html.generate_html_summary_target_cat,  |
| sweetviz/sv_html.py:176 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_detail_text, sweetviz.sv_html.generate_html_summary_text |
| sweetviz/sv_html.py:206 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_detail_text, sweetviz.sv_html.generate_html_summary_text |
| sweetviz/sv_html.py:211 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.sv_html.generate_html_detail_text, |
| sweetviz/sv_html.py:215 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_detail_text, sweetviz.sv_html.generate_html_summary_text |
| sweetviz/sv_html.py:240 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_summary_cat, sweetviz.sv_html.generate_html_summary_target_cat,  |
| sweetviz/sv_html.py:266 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_detail_cat, sweetviz.sv_html.generate_html_detail_numeric |
| sweetviz/sv_html.py:316 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_detail_cat, sweetviz.sv_html.generate_html_detail_numeric |
| sweetviz/sv_html.py:415 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_detail_text, sweetviz.sv_html.generate_html_summary_text |
| sweetviz/sv_html.py:446 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_detail_text, sweetviz.sv_html.generate_html_summary_text |
| sweetviz/sv_html.py:451 | #11 | indexed | real | genuine copy - sweetviz.series_analyzer_cat.do_detail_categorical, sweetviz.sv_html.generate_html_detail_text, |
| sweetviz/sv_html.py:455 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_detail_text, sweetviz.sv_html.generate_html_summary_text |
| sweetviz/sv_html.py:474 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_summary_cat, sweetviz.sv_html.generate_html_summary_target_cat,  |
| sweetviz/sv_html.py:483 | #11 | indexed | real | genuine copy - sweetviz.sv_html.generate_html_summary_cat, sweetviz.sv_html.generate_html_summary_target_cat,  |
| sweetviz/sv_html_formatters.py:41 | #11 | indexed | real | genuine copy - sweetviz.sv_html_formatters.fmt_percent, sweetviz.sv_html_formatters.fmt_percent1d |
| sweetviz/sv_html_formatters.py:53 | #11 | indexed | real | genuine copy - sweetviz.sv_html_formatters.fmt_percent, sweetviz.sv_html_formatters.fmt_percent1d |
| sweetviz/dataframe_report.py:325 | #12 | heuristic | real | exact idiom instance - membership tests the dict: drop .keys() |
| sweetviz/dataframe_report.py:422 | #12 | heuristic | real | exact idiom instance - membership tests the dict: drop .keys() |
| sweetviz/dataframe_report.py:422 | #12 | heuristic | real | exact idiom instance - membership tests the dict: drop .keys() |
| sweetviz/dataframe_report.py:425 | #12 | heuristic | real | exact idiom instance - membership tests the dict: drop .keys() |
| sweetviz/dataframe_report.py:425 | #12 | heuristic | real | exact idiom instance - membership tests the dict: drop .keys() |
| sweetviz/dataframe_report.py:430 | #12 | heuristic | real | exact idiom instance - membership tests the dict: drop .keys() |
| sweetviz/dataframe_report.py:435 | #12 | heuristic | real | exact idiom instance - membership tests the dict: drop .keys() |
| sweetviz/graph_associations.py:304 | #12 | heuristic | real | exact idiom instance - use list(kwargs['x_order']) |
| sweetviz/graph_associations.py:306 | #12 | heuristic | real | exact idiom instance - use list(sorted(set([v for v in x]))) |
| sweetviz/graph_associations.py:306 | #12 | heuristic | real | exact idiom instance - use list(x) |
| sweetviz/graph_associations.py:313 | #12 | heuristic | real | exact idiom instance - use list(kwargs['y_order']) |
| sweetviz/graph_associations.py:315 | #12 | heuristic | real | exact idiom instance - use list(sorted(set([v for v in y]))) |
| sweetviz/graph_associations.py:315 | #12 | heuristic | real | exact idiom instance - use list(y) |
| sweetviz/graph_associations.py:335 | #12 | heuristic | real | exact idiom instance - use list(x_to_num) |
| sweetviz/graph_associations.py:337 | #12 | heuristic | real | exact idiom instance - use list(y_to_num) |
| sweetviz/graph_associations.py:344 | #12 | heuristic | real | exact idiom instance - use list(x_to_num.values()) |
| sweetviz/graph_associations.py:345 | #12 | heuristic | real | exact idiom instance - use list(y_to_num.values()) |
| sweetviz/graph_associations.py:431 | #12 | heuristic | real | exact idiom instance - membership tests the dict: drop .keys() |
| sweetviz/series_analyzer.py:48 | #12 | heuristic | real | exact idiom instance - use bool(my_counts[to_fill].index.dtype.name in ('category', 'object')) |
| sweetviz/series_analyzer.py:94 | #12 | heuristic | real | exact idiom instance - use bool(to_process.order == -1) |
| sweetviz/from_profiling_pandas.py:91 | #13 | indexed | real | the body is one forwarding call to pd.api.types.is_numeric_dtype; the hop adds no meaning |
| sweetviz/sv_html.py:265 | #14 | indexed | real | the three parameters travel together through every signature in the layer and want one type |
| sweetviz/sv_public.py:8 | #14 | indexed | real | the three parameters travel together through every signature in the layer and want one type |
| sweetviz/graph_associations.py:437 | #15 | heuristic | real | the body reads only the two named members and never forwards the object |
| sweetviz/series_analyzer_text.py:6 | #15 | heuristic | real | the body reads only the two named members and never forwards the object |
| sweetviz/sv_html.py:42 | #15 | heuristic | real | the body reads only the two named members and never forwards the object |
| sweetviz/dataframe_report.py:458 | #18 | heuristic | real | the banner comments mark extractable phases inside one function body |
| sweetviz/graph_cat.py:106 | #18 | heuristic | real | the banner comments mark extractable phases inside one function body |
| sweetviz/graph_numeric.py:58 | #18 | heuristic | real | the banner comments mark extractable phases inside one function body |
| sweetviz/series_analyzer_cat.py:14 | #18 | heuristic | real | the banner comments mark extractable phases inside one function body |
| sweetviz/sv_html.py:179 | #18 | heuristic | real | the banner comments mark extractable phases inside one function body |
| sweetviz/sv_html.py:269 | #18 | heuristic | real | the banner comments mark extractable phases inside one function body |
| sweetviz/sv_html.py:319 | #18 | heuristic | real | the banner comments mark extractable phases inside one function body |
| sweetviz/sv_html.py:418 | #18 | heuristic | real | the banner comments mark extractable phases inside one function body |
| sweetviz/graph_associations.py:124 | #19 | heuristic | real | list.index() is a linear scan run inside a doubly-nested loop over the same list - O(n^3) by construction |
| sweetviz/graph_associations.py:144 | #19 | heuristic | real | list.index() is a linear scan run inside a doubly-nested loop over the same list - O(n^3) by construction |
| sweetviz/graph_associations.py:160 | #19 | heuristic | real | list.index() is a linear scan run inside a doubly-nested loop over the same list - O(n^3) by construction |
| sweetviz/dataframe_report.py:23 | #21 | heuristic | real | self._target['type'] is re-derived in three methods instead of living behind one accessor |
| sweetviz/dataframe_report.py:23 | #21 | heuristic | fp | a repeated call to one of the class's own methods is reuse, not an invariant re-established at each site |
| sweetviz/dataframe_report.py:319 | #22 | heuristic | real | the method touches only public attributes; Meyers' encapsulation count is satisfied |
| sweetviz/feature_config.py:28 | #22 | heuristic | real | the method touches only public attributes; Meyers' encapsulation count is satisfied |
| sweetviz/feature_config.py:40 | #22 | heuristic | real | the method touches only public attributes; Meyers' encapsulation count is satisfied |
| sweetviz/graph.py:24 | #22 | heuristic | real | Graph is a namespace of statics whose __init__ no subclass ever calls - it owns no state |
| sweetviz/sv_types.py:86 | #22 | heuristic | real | the method touches only public attributes; Meyers' encapsulation count is satisfied |
| sweetviz/dataframe_report.py:24 | #23 | heuristic | real | measured cognitive complexity 97 (threshold 15) |
| sweetviz/dataframe_report.py:387 | #23 | heuristic | real | measured cognitive complexity 18 (threshold 15) |
| sweetviz/dataframe_report.py:418 | #23 | heuristic | real | measured cognitive complexity 96 (threshold 15) |
| sweetviz/feature_config.py:7 | #23 | heuristic | fp | fires at exactly the threshold on a 20-line constructor with three small branches - a boundary-inclusive comparison on a function nobody would call complex |
| sweetviz/from_dython.py:51 | #23 | heuristic | real | measured cognitive complexity 42 (threshold 15) |
| sweetviz/from_profiling_pandas.py:35 | #23 | heuristic | real | measured cognitive complexity 22 (threshold 15) |
| sweetviz/graph.py:63 | #23 | heuristic | real | measured cognitive complexity 29 (threshold 15) |
| sweetviz/graph_associations.py:55 | #23 | heuristic | real | measured cognitive complexity 16 (threshold 15) |
| sweetviz/graph_associations.py:89 | #23 | heuristic | real | measured cognitive complexity 106 (threshold 15) |
| sweetviz/graph_associations.py:230 | #23 | heuristic | real | measured cognitive complexity 60 (threshold 15) |
| sweetviz/graph_cat.py:57 | #23 | heuristic | real | measured cognitive complexity 83 (threshold 15) |
| sweetviz/graph_numeric.py:14 | #23 | heuristic | real | measured cognitive complexity 60 (threshold 15) |
| sweetviz/series_analyzer.py:43 | #23 | heuristic | real | measured cognitive complexity 16 (threshold 15) |
| sweetviz/series_analyzer.py:79 | #23 | heuristic | real | measured cognitive complexity 41 (threshold 15) |
| sweetviz/series_analyzer_cat.py:9 | #23 | heuristic | real | measured cognitive complexity 77 (threshold 15) |
| sweetviz/sv_html.py:175 | #23 | heuristic | real | measured cognitive complexity 18 (threshold 15) |
| sweetviz/sv_html.py:315 | #23 | heuristic | real | measured cognitive complexity 19 (threshold 15) |
| sweetviz/sv_html.py:414 | #23 | heuristic | real | measured cognitive complexity 18 (threshold 15) |
| sweetviz/sv_html_formatters.py:62 | #23 | heuristic | real | measured cognitive complexity 56 (threshold 15) |
| sweetviz/sv_html_formatters.py:103 | #23 | heuristic | real | measured cognitive complexity 56 (threshold 15) |
| sweetviz/sv_html_formatters.py:131 | #23 | heuristic | real | measured cognitive complexity 56 (threshold 15) |
| sweetviz/type_detection.py:6 | #23 | heuristic | real | measured cognitive complexity 56 (threshold 15) |
| sweetviz/dataframe_report.py:23 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/dataframe_report.py:358 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/dataframe_report.py:363 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/graph_associations.py:52 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/graph_associations.py:53 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/graph_associations.py:88 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/sv_html.py:29 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/sv_html.py:42 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/sv_html.py:52 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/sv_html.py:69 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| sweetviz/sv_html.py:92 | #27 | indexed | real | the symbol is hot and its container forces the whole file into every reader's context |
| README.md:65 | #28 | indexed | fp | README:65 tells users NOT to create a file named sweetviz.py; demanding that path resolve inverts the sentence |
| sweetviz/dataframe_report.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/dataframe_report.py:418 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/dataframe_report.py:539 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/dataframe_report.py:577 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/from_dython.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/from_dython.py:51 | #29 | heuristic | fp | a cheap pure helper flagged on line count alone - doc-presence scoring, which the research explicitly rules out |
| sweetviz/graph.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/graph.py:140 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/graph_associations.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/graph_associations.py:55 | #29 | heuristic | fp | a cheap pure helper flagged on line count alone - doc-presence scoring, which the research explicitly rules out |
| sweetviz/graph_associations.py:230 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/graph_associations.py:437 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/graph_cat.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/graph_cat.py:13 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/graph_numeric.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/series_analyzer.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/series_analyzer.py:9 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/series_analyzer.py:79 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/series_analyzer_cat.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/series_analyzer_cat.py:9 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/series_analyzer_numeric.py:31 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/series_analyzer_text.py:6 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/sv_html.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/sv_html.py:99 | #29 | heuristic | fp | a cheap pure helper flagged on line count alone - doc-presence scoring, which the research explicitly rules out |
| sweetviz/sv_html.py:131 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/sv_html.py:175 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/sv_html.py:265 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/sv_html.py:315 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/sv_html.py:414 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/sv_html_formatters.py:1 | #29 | heuristic | real | the module genuinely opens with no statement of what it is - no module in the package has one |
| sweetviz/type_detection.py:6 | #29 | heuristic | real | the entry point does real work (I/O, rendering, or a quadratic pass) and declares none of it |
| sweetviz/utils.py:6 | #29 | heuristic | fp | a cheap pure helper flagged on line count alone - doc-presence scoring, which the research explicitly rules out |
| sweetviz/config.py:2 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/dataframe_report.py:134 | #32 | indexed | fp | flush is duck-typed protocol - tqdm calls it on the file object it is handed |
| sweetviz/dataframe_report.py:539 | #32 | indexed | fp | documented public API (README lines 83/128/140/233); name-level liveness inside the package cannot see library consumers |
| sweetviz/dataframe_report.py:577 | #32 | indexed | fp | documented public API (README lines 83/128/140/233); name-level liveness inside the package cannot see library consumers |
| sweetviz/dataframe_report.py:648 | #32 | indexed | fp | documented public API (README lines 83/128/140/233); name-level liveness inside the package cannot see library consumers |
| sweetviz/from_dython.py:45 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/from_dython.py:46 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/from_dython.py:47 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/from_profiling_pandas.py:95 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/from_profiling_pandas.py:127 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/from_profiling_pandas.py:138 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph.py:1 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph.py:8 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_associations.py:10 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_associations.py:425 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_associations.py:437 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_legend.py:2 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_legend.py:5 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_legend.py:9 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_legend.py:10 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_legend.py:12 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/graph_legend.py:25 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/series_analyzer_numeric.py:5 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/series_analyzer_numeric.py:31 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/sv_html.py:2 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/sv_html.py:474 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/sv_html.py:483 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/sv_math.py:1 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/sv_public.py:1 | #32 | indexed | real | confirmed unreferenced across the whole tree, templates included |
| sweetviz/dataframe_report.py:323 | #33 | heuristic | real | a mapping accessor that silently returns None instead of raising - callers cannot rely on the result (the returns are explicit None, not bare) |
| sweetviz/dataframe_report.py:358 | #33 | heuristic | real | the annotation names a type the body contradicts on a live path |
| sweetviz/dataframe_report.py:363 | #33 | heuristic | real | the annotation names a type the body contradicts on a live path |
| sweetviz/series_analyzer.py:61 | #33 | heuristic | real | the annotation names a type the body contradicts on a live path |
| sweetviz/comet_ml_logger.py:19 | #34 | heuristic | real | bare or broad except that only prints or passes - an error strategy that discards the error |
| sweetviz/comet_ml_logger.py:26 | #34 | heuristic | real | bare or broad except that only prints or passes - an error strategy that discards the error |
| sweetviz/comet_ml_logger.py:35 | #34 | heuristic | real | bare or broad except that only prints or passes - an error strategy that discards the error |
| sweetviz/dataframe_report.py:279 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/dataframe_report.py:444 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/dataframe_report.py:652 | #34 | heuristic | real | bare or broad except that only prints or passes - an error strategy that discards the error |
| sweetviz/graph_associations.py:129 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/graph_associations.py:294 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/graph_associations.py:378 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/graph_associations.py:392 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/graph_cat.py:135 | #34 | heuristic | real | bare or broad except that only prints or passes - an error strategy that discards the error |
| sweetviz/graph_cat.py:203 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/graph_legend.py:93 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/series_analyzer_numeric.py:82 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/sv_html.py:80 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/sv_html.py:275 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/update_jquery.py:40 | #34 | heuristic | real | bare or broad except that only prints or passes - an error strategy that discards the error |
| sweetviz/utils.py:57 | #34 | heuristic | real | dead alternative code preserved in comments; git already holds it |
| sweetviz/graph.py:11 | #35 | indexed | real | genuine cycle - graph imports sv_html_formatters, which imports graph_associations, which imports graph |
| sweetviz/__init__.py:19 | #39 | heuristic | fp | the comment states what the imported class is, in the present tense; no history in it |
| sweetviz/from_dython.py:29 | #39 | heuristic | fp | the matched lines sit inside a verbatim BSD license block, not narration |
| sweetviz/from_dython.py:101 | #39 | heuristic | fp | a proper numpydoc API docstring on a public statistical function - penalising documentation by line ratio is the doc-presence trap inverted |
| sweetviz/from_dython.py:123 | #39 | heuristic | fp | the matched lines are numpydoc parameter descriptions, not history |
| sweetviz/from_dython.py:142 | #39 | heuristic | fp | a proper numpydoc API docstring on a public statistical function - penalising documentation by line ratio is the doc-presence trap inverted |
| sweetviz/from_dython.py:171 | #39 | heuristic | fp | the matched lines are numpydoc parameter descriptions, not history |
| sweetviz/from_dython.py:189 | #39 | heuristic | fp | a proper numpydoc API docstring on a public statistical function - penalising documentation by line ratio is the doc-presence trap inverted |
| sweetviz/from_dython.py:219 | #39 | heuristic | fp | the matched lines are numpydoc parameter descriptions, not history |
| sweetviz/from_profiling_pandas.py:107 | #39 | heuristic | fp | a real Args/Returns docstring on an 8-line function - documentation, not narration |
| sweetviz/graph_associations.py:33 | #39 | heuristic | fp | the matched lines sit inside a verbatim BSD license block, not narration |
| sweetviz/graph_associations.py:214 | #39 | heuristic | real | genuine comment-discipline violation at this site |
| sweetviz/graph_associations.py:437 | #39 | heuristic | real | genuine comment-discipline violation at this site |
| sweetviz/series_analyzer.py:10 | #39 | heuristic | real | genuine comment-discipline violation at this site |
| sweetviz/series_analyzer.py:29 | #39 | heuristic | real | the matched line is commented-out code wearing a comment's clothes - cruft under either rule |
| sweetviz/series_analyzer.py:45 | #39 | heuristic | real | the matched line is commented-out code wearing a comment's clothes - cruft under either rule |
| sweetviz/series_analyzer_numeric.py:65 | #39 | heuristic | real | the matched line is commented-out code wearing a comment's clothes - cruft under either rule |
| sweetviz/sv_html_formatters.py:63 | #39 | heuristic | fp | the comment explains intent the code cannot carry, in the present tense |
| sweetviz/update_jquery.py:1 | #39 | heuristic | fp | the module header states what the script does and how to run it - exactly the top-loading #29 asks for |
| sweetviz/update_jquery.py:5 | #39 | heuristic | fp | the module header states what the script does and how to run it - exactly the top-loading #29 asks for |
| sweetviz/update_jquery.py:14 | #39 | heuristic | real | "Set the old version and the new version" above `old_version = "3.4.1"` restates the two lines it annotates |
| sweetviz/update_jquery.py:27 | #39 | heuristic | real | "Find and replace all instances of importing the old version" restates the read/replace/write it labels |
| sweetviz/update_jquery.py:37 | #39 | heuristic | real | "Delete the old version of jQuery" restates the `remove(...)` on the next line |
| sweetviz/sv_html.py:92 | #40 | heuristic | fp | the plural word is a noun modifier naming the format or subject ('commas', 'parentheses', 'associations'), not a promise of plural return cardinality |
| sweetviz/sv_html_formatters.py:7 | #40 | heuristic | fp | the plural word is a noun modifier naming the format or subject ('commas', 'parentheses', 'associations'), not a promise of plural return cardinality |
| sweetviz/sv_html_formatters.py:29 | #40 | heuristic | fp | the plural word is a noun modifier naming the format or subject ('commas', 'parentheses', 'associations'), not a promise of plural return cardinality |

## Phase 3 — reconciliation

124 phase-1 sites: 48 `covered`, 48 `detector-miss`, 15 `threshold-miss`,
13 `inventory-gap`.

Six rules produced no finding anywhere in this repo — #6, #7, #8, #9, #26, #37 —
and account for 12 of the detector-misses on their own. Family P (#41) was
silent by provenance ("no hot-roots config"), which costs all five of my
hot-path sites. The largest single-rule gaps are #12 (10 catalog entries the
repo instantiates and the catalog lacks, including `zip(seq, range(len(seq)))`
for `enumerate`) and #32 (six unread-local sites: the rule covers imports,
params and symbols but not local bindings).

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #32 | covered | 32\|from_profiling_pandas.py:95 - is_url unreferenced |
| P1-2 | #32 | covered | 32\|from_profiling_pandas.py:127 - is_path unreferenced (str_is_path, live only for it, not named) |
| P1-3 | #32 | covered | 32\|from_profiling_pandas.py:138 - is_date unreferenced |
| P1-4 | #13 | threshold-miss | #13 fired on could_be_numeric only; is_date's body is assign-then-return, two statements, so the single-forwarding-call shape does not match |
| P1-5 | #13 | covered | 13\|from_profiling_pandas.py:91 |
| P1-6 | #12 | detector-miss | the catalog carries bool-ternary but not the if/return-True/return-False form of the same idiom |
| P1-7 | #39 | detector-miss | #39 fired elsewhere in this file (:107) but not on the dated 'UPDATE 11-2023: NO IT DIDN'T!!!' retro-log, which is the clearest history narration in the repo |
| P1-8 | #32 | covered | 32\|from_dython.py:45,46,47 - all three constants named |
| P1-9 | #37 | detector-miss | #37 produced no findings at all in this repo; four call sites all take the defaults |
| P1-10 | #37 | detector-miss | #37 silent; the 'list'/'dataframe' arms of convert() are unreachable from any call site |
| P1-11 | #2 | covered | 2\|from_dython.py:89 - unnecessary isinstance |
| P1-12 | #12 | detector-miss | catalog has keys-membership but not iterate-.keys()-then-reindex, which is the same dict idiom |
| P1-13 | #7 | detector-miss | #7 produced no findings at all; the argument-order precondition is narrated in both a comment and the docstring |
| P1-14 | #41 | threshold-miss | family P silent by provenance - no hot-roots config, so the hot-reachable set was empty and no perf shape could be scoped |
| P1-15 | #41 | threshold-miss | family P silent by provenance (same cutoff) |
| P1-16 | #11 | detector-miss | the 4-statement nan-strategy preamble repeats in three sibling functions and clears the x2/3-stmt bar the detector used elsewhere, yet no clone fired anywhere in from_dython.py |
| P1-17 | none | inventory-gap | calling an API removed from the pinned dependency - no rule covers dependency-version breakage |
| P1-18 | #34 | covered | 34\|comet_ml_logger.py:19,26,35 (the module-level bare except at :5 was not named, but the site is covered) |
| P1-19 | #34 | detector-miss | no #34 finding in config.py; two commented-out lines at module scope |
| P1-20 | #7 | detector-miss | #7 silent - 'IMPORTANT: assuming value_counts is ALREADY SORTED' is a textbook caller-must precondition |
| P1-21 | #34 | threshold-miss | #34 fired at utils.py:57 (a 3-line block); the :16 and :21 commented lines are isolated singles under the block bar |
| P1-22 | #39 | detector-miss | no #39 finding in utils.py; 'Fix for #10' twice is issue-history narration |
| P1-23 | #19 | detector-miss | #19 matched list.index() only; Series membership inside a loop over the index was not seen |
| P1-24 | #40 | detector-miss | #40 fired three times, all on format-word plurals; a scalar-named function returning a 2-tuple was not seen |
| P1-25 | #1 | covered | 1\|type_detection.py:6 - bare dict param (the '-> object' half of the site was not named) |
| P1-26 | #33 | covered | 33\|series_analyzer.py:61 |
| P1-27 | #12 | covered | 12\|series_analyzer.py:94 - bool-ternary |
| P1-28 | #12 | covered | 12\|series_analyzer.py:48 - bool-ternary |
| P1-29 | #19 | detector-miss | Series membership inside a loop over another Series' items; same detector gap as P1-23 |
| P1-30 | #34 | threshold-miss | the commented-out stubs in series_analyzer.py are 1-2 lines each, under the block bar; #39 caught two of them at :29/:45 as restatement instead |
| P1-31 | #15 | covered | 15\|series_analyzer_text.py:6 - names both attributes exactly |
| P1-32 | none | inventory-gap | conditionally-bound local read under a different guard - no rule covers definite-assignment shapes |
| P1-33 | #11 | covered | 11\|series_analyzer_cat.py:50 and :82 - the x2 3-stmt target-stat pair |
| P1-34 | #41 | threshold-miss | family P silent by provenance; the code's own TODO concedes the O(rows x categories) mask |
| P1-35 | #34 | covered | 34\|series_analyzer_numeric.py:82 - 10 commented-out lines |
| P1-36 | #39 | threshold-miss | #39 fired at :65 in this file but not on the 'MAD was unused!!!' pair at :25-26 |
| P1-37 | none | inventory-gap | a constructor mutating the caller's live pandas Series - no rule covers input-argument mutation |
| P1-38 | none | inventory-gap | __int__ returning None - no rule covers dunder protocol contracts |
| P1-39 | #34 | detector-miss | the trailing no-op 'return' shape (six occurrences across the package) is not in the noop-code arm |
| P1-40 | #32 | detector-miss | #22 fired at graph.py:24 on a different claim; the fact that no subclass calls Graph.__init__, leaving self.data dead, was not found |
| P1-41 | none | inventory-gap | extend-then-overwrite dead store - no rule covers unread writes |
| P1-42 | #11 | threshold-miss | the duplicated pair is 2 statements, under the 3-statement clone bar the detector used elsewhere |
| P1-43 | #34 | threshold-miss | #34's broad-except arm requires a pass/print body; this handler's body is 'continue', so it fell outside the pattern |
| P1-44 | #13 | detector-miss | a static method forwarding to a module function with one discarded extra param was not matched |
| P1-45 | #32 | covered | 32\|graph.py:1 - unused matplotlib import (and :8 importlib_resources, which I missed) |
| P1-46 | #32 | covered | 32\|graph_legend.py:25 - to_fractions (the unused text1_elem/text2_elem locals were not named) |
| P1-47 | #32 | covered | 32\|graph_legend.py:2,5,9,10,12 - all five unused imports |
| P1-48 | #34 | covered | 34\|graph_legend.py:93 - 3 commented-out lines |
| P1-49 | #34 | covered | 34\|graph_cat.py:135 - broad except |
| P1-50 | #1 | covered | 1\|graph_cat.py:15 - opaque **kwargs |
| P1-51 | #11 | detector-miss | the vertical/horizontal bar pair repeats verbatim across the axis_obj and plt arms; #11 fired at :80/:92 in the same method but not here |
| P1-52 | #11 | detector-miss | four copies of the per-tick target loop in one method; the clone detector clustered smaller blocks in this file but not these |
| P1-53 | #32 | detector-miss | #32 covers imports, params and symbols but not unread local bindings |
| P1-54 | #8 | detector-miss | #8 produced no findings; which_graph is a string encoding two facts, re-parsed at nine sites across two modules |
| P1-55 | #34 | covered | 34\|graph_cat.py:203 - 6 commented-out lines |
| P1-56 | none | inventory-gap | copy-paste variable mix-up that is also a provable no-op - no rule covers it |
| P1-57 | #34 | detector-miss | no #34 finding anywhere in graph_numeric.py, yet :227-251 is the single largest commented-out block in the repo (25 lines) - the detector caught a 3-line block in utils.py and missed this |
| P1-58 | none | inventory-gap | a filterwarnings call written as a restore that does not restore - no rule covers global-state save/restore |
| P1-59 | #18 | covered | 18\|graph_numeric.py:58 - 3 labeled phases |
| P1-60 | #32 | covered | 32\|graph_associations.py:425 - filter_best_corr unreferenced |
| P1-61 | #19 | covered | 19\|graph_associations.py:124,144,160 - three of the four .index() sites |
| P1-62 | #11 | covered | 11\|graph_associations.py:139 and :195 - the repeated fill loop |
| P1-63 | #1 | covered | 1\|graph_associations.py:230 - opaque **kwargs |
| P1-64 | #32 | detector-miss | kwargs_pass_on and marker are unread locals; same gap as P1-53 |
| P1-65 | #32 | detector-miss | dead store overwritten before any read; same gap as P1-53 |
| P1-66 | #41 | threshold-miss | family P silent by provenance |
| P1-67 | #12 | covered | 12\|graph_associations.py:304,306,313,315,335,337,344,345 - nine identity-comprehension findings |
| P1-68 | #32 | covered | 32\|graph_associations.py:10 - unused wrap import |
| P1-69 | none | inventory-gap | DataFrame.append removed in pandas 2.0; the same deprecation was fixed in utils.py but not here - no rule covers a partially-applied fix |
| P1-70 | none | inventory-gap | three of four branches set an index name the consumer never melts on - no rule covers cross-branch contract mismatch |
| P1-71 | #37 | covered | 32\|graph_associations.py:437 names 'size_scale' never read - a different rule than the #37 I chose, but the same site and defect (marker was not named) |
| P1-72 | #39 | covered | 39\|graph_associations.py:437 - prose outweighs the function |
| P1-73 | #32 | detector-miss | a debug value installed into a dict (jinja2_env.globals['hello']) is not a name binding, so name-level liveness cannot see it |
| P1-74 | #32 | covered | 32\|sv_html.py:474 and :483 - both unreferenced |
| P1-75 | #39 | detector-miss | six lines of a thrice-repeated '#UNUSED yet:' marker were not matched by any comment arm |
| P1-76 | #34 | detector-miss | a computed value overwritten on the next line, making three of four loop-body lines dead - not in the noop-code arm |
| P1-77 | #11 | covered | 11\|sv_html.py:176 and :415 - the x2 16-statement clone pair |
| P1-78 | #6 | detector-miss | #6 produced no findings; a string-returning 'generate_html_*' that truncates and appends to its argument is the rule's central case |
| P1-79 | #32 | detector-miss | unread local; same gap as P1-53 |
| P1-80 | #12 | detector-miss | '== False' is not in the idiom catalog |
| P1-81 | #1 | covered | 1\|sv_html.py:99,131,169,175,232,240,265,315,414 - all nine bare-dict signatures |
| P1-82 | #9 | detector-miss | #9 produced no findings; jinja2_env is a module-level mutable rewritten from another module four times |
| P1-83 | #26 | detector-miss | #26 produced no findings; the layout globals are assembled by iterating a config section |
| P1-84 | #11 | threshold-miss | #11 caught the small fmt_percent/fmt_percent1d pair but not the three ~27-line magnitude ladders - their branch bodies differ in format string, so AST normalisation did not match |
| P1-85 | none | inventory-gap | a parameter shadowing a builtin - no rule covers shadowing |
| P1-86 | #32 | detector-miss | the import is used, so liveness passes; the second identical import statement is invisible to the rule |
| P1-87 | #32 | detector-miss | unread local; same gap as P1-53 |
| P1-88 | #34 | covered | 2\|dataframe_report.py:129 - flagged under a different rule (always-True condition), same site and same defect |
| P1-89 | #32 | covered | 2\|dataframe_report.py:338 and 1\|:337 name the site; #32 did not report that the method itself is unreferenced |
| P1-90 | #33 | detector-miss | '-> bool' returning an unconstrained input on an unreachable identity-against-a-type branch was not matched |
| P1-91 | #33 | covered | 33\|dataframe_report.py:358 and :363 |
| P1-92 | #12 | detector-miss | zip(seq, range(len(seq))) for enumerate is not in the catalog - a canonical entry |
| P1-93 | #12 | detector-miss | equality-filter comprehension standing in for a membership test is not in the catalog |
| P1-94 | #12 | detector-miss | df.items() to collect column names (materialising every column) is not in the catalog |
| P1-95 | #11 | covered | 11\|dataframe_report.py:531,540,547,580,588,616,623 - thirteen overlapping views of the same four-way duplication |
| P1-96 | #12 | detector-miss | open/write/close without a with-block is not in the catalog |
| P1-97 | #12 | detector-miss | a hand-rolled null sink standing in for io.StringIO/os.devnull is not in the catalog |
| P1-98 | #41 | threshold-miss | family P silent by provenance; this is the repo's own declared quadratic path, with the halving skip commented out at :444 |
| P1-99 | #19 | detector-miss | list membership inside a loop; same gap as P1-23/P1-29 |
| P1-100 | #19 | detector-miss | Index membership inside a loop; same gap |
| P1-101 | #22 | detector-miss | #22 fired on four weaker velcro cases but missed the two methods that touch no self at all |
| P1-102 | #21 | covered | 21\|dataframe_report.py:23 - self._target['type'] recurs in 3 methods |
| P1-103 | #18 | detector-miss | #18 fired at :458 (2 phases) but not on __init__'s thirteen banner-labeled phases across 294 lines - the largest instance in the repo |
| P1-104 | #29 | covered | 29\|dataframe_report.py:1 - no top-loading docstring |
| P1-105 | #27 | covered | 27\|dataframe_report.py:23 - fan-in 4 in a 653-line module |
| P1-106 | #39 | detector-miss | no #39 finding in dataframe_report.py; two dated 'UPDATE 2021-02-05' entries and a 'NEW (12-14-2020)' entry are pure history |
| P1-107 | #34 | covered | 34\|dataframe_report.py:652 - broad except |
| P1-108 | none | inventory-gap | one attribute carrying a bool flag and an HTML payload - no rule covers attribute type drift |
| P1-109 | none | inventory-gap | f-string with no placeholders - no rule covers it |
| P1-110 | #1 | covered | 1\|dataframe_report.py:319 - opaque **kwargs |
| P1-111 | #35 | covered | 35\|graph.py:11 - the exact 3-module cycle |
| P1-112 | #35 | detector-miss | the single #35 finding names the cycle but not this edge: sv_html reads sweetviz.__version__ without importing sweetviz, binding the name only as a side effect of a submodule import |
| P1-113 | #35 | detector-miss | sv_public imports the module but calls the package attribute, so the real edge runs back through __init__ - not named |
| P1-114 | #1 | covered | 1\|sv_public.py:9,10,19,20,30,31 - all six implicit-Optional params |
| P1-115 | #29 | threshold-miss | #29 did not fire on sv_public.py: 50 lines and short functions put the package's entire public surface under both the module-size and entry-point-size bars |
| P1-116 | none | inventory-gap | a parameter shadowing its own function name - no rule covers shadowing |
| P1-117 | #11 | threshold-miss | analyze and compare share a 2-statement body, under the 3-statement clone bar |
| P1-118 | #12 | detector-miss | type(x) == T for isinstance is not in the catalog, despite six occurrences across three modules |
| P1-119 | #1 | covered | 1\|feature_config.py:7,8,9 - the implicit-Optional half of the site (the unparameterised bare Tuple was not named) |
| P1-120 | #11 | threshold-miss | the four-line repeats are single statements each, under the clone bar |
| P1-121 | #26 | detector-miss | #26 silent; the script's version constants and its import-time side effect were not seen |
| P1-122 | #34 | covered | 34\|update_jquery.py:40 - broad except |
| P1-123 | #29 | covered | 29\|graph_associations.py:1 - no top-loading docstring |
| P1-124 | #29 | covered | 29\|sv_html.py:1 - no top-loading docstring |
