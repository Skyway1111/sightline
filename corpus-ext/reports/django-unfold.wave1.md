# django-unfold — judge report (wave 1)

Repo: `<GAUNTLET_CORPUS_ROOT>\django-unfold`
Prod tree judged: `src/unfold/**` (66 files, 7952 lines). Read cold; no
checker output consulted.

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/unfold/widgets.py:332 | #11 | The same `__init__` body — `super().__init__(attrs={**(attrs or {}), "class": " ".join([*INPUT_CLASSES, ...])})` — is copied at 332, 346, 371, 401, 641, 653, 665, 871, 988, with class-list variants at 358, 452, 479, 499, 526, 546, 564. One `_merged_attrs(attrs, CLASSES)` helper replaces ~15 copies. | `"class": " ".join(` / `[*INPUT_CLASSES, attrs.get("class", "") if attrs else ""]` |
| P1-2 | src/unfold/widgets.py:337 | #3 | `attrs.get("class", "") if attrs else ""` is an emptiness guard the callee's contract already discharges: `{}.get("class", "")` is `""`. ~20 occurrences in this file plus contrib/forms/widgets.py:122. | `[*INPUT_CLASSES, attrs.get("class", "") if attrs else ""]` |
| P1-3 | src/unfold/widgets.py:476 | #11 | `UnfoldAdminSingleDateWidget` is byte-identical to `UnfoldAdminDateWidget` (449) except the missing `Media`; `UnfoldAdminSingleTimeWidget` (523) vs `UnfoldAdminTimeWidget` (496) likewise. | `class UnfoldAdminSingleDateWidget(AdminDateWidget):` / `template_name = "unfold/widgets/date.html"` |
| P1-4 | src/unfold/widgets.py:652 | #11 | `UnfoldAdminDecimalFieldWidget` has the same base class *and* a byte-identical body as `UnfoldAdminIntegerFieldWidget` (640); `UnfoldAdminBigIntegerFieldWidget` (664) differs only in base. | `class UnfoldAdminDecimalFieldWidget(AdminIntegerFieldWidget):` |
| P1-5 | src/unfold/widgets.py:441 | #11 | `UnfoldAdminFileFieldWidget` and `UnfoldAdminImageSmallFieldWidget` (445) are identical class bodies — same bases, same `template_name`. | `class UnfoldAdminFileFieldWidget(FileFieldMixin, AdminFileWidget):` / `template_name = "unfold/widgets/clearable_file_input_small.html"` |
| P1-6 | src/unfold/widgets.py:715 | #11 | The select2 `class Media` block (extra/js/css) is copied verbatim at 715, 759, 912 and again at contrib/filters/forms.py:69 and :153 — five copies of one fact. | `extra = "" if settings.DEBUG else ".min"` / `js = (f"admin/js/vendor/jquery/jquery{extra}.js", ...)` |
| P1-7 | src/unfold/widgets.py:468 | #38 | The `["admin/js/core.js", "admin/js/calendar.js", "admin/js/admin/DateTimeShortcuts.js"]` asset list is declared in three `Media` classes here (468, 515, 594) and two more in contrib/filters/forms.py (232, 265). | `js = ["admin/js/core.js", "admin/js/calendar.js", "admin/js/admin/DateTimeShortcuts.js"]` |
| P1-8 | src/unfold/widgets.py:396 | none | `decompress(value: str \| None)` returns the *bound methods* `value.lower`/`value.upper`, not values; the `str` annotation and the body (which wants a psycopg `Range`) contradict each other, and the declared `tuple[Callable \| None, ...]` documents the bug rather than the intent. | `def decompress(self, value: str \| None) -> tuple[Callable \| None, ...]:` / `return (value.lower, value.upper) if value else (None, None)` |
| P1-9 | src/unfold/widgets.py:289 | #1 | `get_context` publishes `attrs: dict[str, Any] \| None -> dict[str, Any]` — the widget-context contract is untyped at every override (289, 413, 622, 790, contrib/forms/widgets.py:62). | `def get_context(self, name: str, value: Any, attrs: dict[str, Any] \| None) -> dict[str, Any]:` |
| P1-10 | src/unfold/widgets.py:780 | #1 | Opaque `*args: Any, **kwargs: Any` on public widget constructors (780, 800, 961, 980, 1003) — callers cannot see what a widget accepts. | `def __init__(self, radio_style: int \| None = None, *args: Any, **kwargs: Any) -> None:` |
| P1-11 | src/unfold/widgets.py:420 | none | `*["form-check-input"]` unpacks a one-element list literal into a list literal. | `"class": " ".join([*CHECKBOX_CLASSES, *["form-check-input"]]),` |
| P1-12 | src/unfold/templatetags/unfold.py:176 | #11 | `has_nav_item_active` (176) and `has_active_item` (185) have byte-identical bodies under two names, registered as a tag and a filter. | `for item in items:` / `if "active" in item and item["active"]: return True` |
| P1-13 | src/unfold/templatetags/unfold.py:177 | #12 | Both are a hand-rolled `any(...)`; the next function in the same module (194) already writes it as `any(...)`. | `return any(action.get("display_in_dropdown", True) for action in actions)` |
| P1-14 | src/unfold/templatetags/unfold.py:59 | #11 | `_count_errors_in_inline` is the inner loop of `_count_errors_in_general` (50-54) lifted out; the same three lines appear a third time in `tabs_primary_active` (869-873). | `for error in inline.formset.errors:` / `if isinstance(error, dict) and len(error) > 0: count += 1` |
| P1-15 | src/unfold/templatetags/unfold.py:170 | #24 | `import_string(section_class)` resolves a class from a template-supplied string — the section class cannot be found by grep from its use site. | `section_class: type[BaseSection] = import_string(section_class)` |
| P1-16 | src/unfold/templatetags/unfold.py:323 | #2 | `classes` is annotated `list \| tuple`, so `type(classes) in (list, tuple)` can never be False (and uses `type() in` where `isinstance` is meant). | `def add_css_class(field: BoundField, classes: list \| tuple) -> BoundField:` / `if type(classes) in (list, tuple):` |
| P1-17 | src/unfold/templatetags/unfold.py:391 | none | `" ".join(set(classes))` renders CSS classes in hash-randomized order, so the emitted HTML differs between processes; also at 406, 429, 463, 532. `dict.fromkeys` dedupes deterministically. | `return " ".join(set(classes))` |
| P1-18 | src/unfold/templatetags/unfold.py:623 | #23 | `header_title` is a 138-line tag with a four-arm branch and eight near-identical `parts.append({"link": reverse_lazy(...), "title": ...})` blocks. | `def header_title(context: RequestContext) -> str:` |
| P1-19 | src/unfold/templatetags/unfold.py:744 | none | `username = user.get_username()` at 744 is a dead store: line 747 re-executes the identical call inside the guard, then 750 overwrites it again. | `username = user.get_username()` / `if hasattr(user, "get_short_name") and callable(user.get_short_name):` / `username = user.get_username()` |
| P1-20 | src/unfold/templatetags/unfold.py:553 | #30 | Five-link Demeter chain `field.field.field.widget.widgets_names[index]`, repeated at 583. The function is handed an `AdminForm` and reaches four levels past it. | `f"{field.field.name}{field.field.field.widget.widgets_names[index]}"` |
| P1-21 | src/unfold/templatetags/unfold.py:570 | #11 | The ~120-char `x-init` JS f-string is duplicated verbatim in two adjacent branches (570-572 and 575-577) that differ only in which `.widget` they touch. | `f"const $ = django.jQuery; $(function () {{ const select = $('#{field.field.auto_id}'); ..." ` |
| P1-22 | src/unfold/templatetags/unfold.py:302 | none | Missing `f` prefix: the message renders the literal `{bits[0]}` instead of the tag name, unlike its two sibling messages at 283 and 307. | `'"with" in {bits[0]} tag needs at least one keyword argument.'` |
| P1-23 | src/unfold/templatetags/unfold.py:599 | #33 | Declared `-> Iterable[int \| str] \| None`, returns a value on the true branch and falls off the end otherwise — mixed explicit/implicit return. | `if paginator and number:` / `return paginator.get_elided_page_range(number=number)` |
| P1-24 | src/unfold/templatetags/unfold.py:594 | #1 | The only unannotated tag in a fully annotated module; `cl` and `i` carry no contract at all. | `def infinite_paginator_url(cl, i):` |
| P1-25 | src/unfold/templatetags/unfold.py:594 | #15 | Takes the whole `ChangeList` to call exactly one method, and never forwards it. | `return cl.get_query_string({PAGE_VAR: i})` |
| P1-26 | src/unfold/templatetags/unfold.py:361 | none | The local `element_classes` shadows the enclosing function's own name and is then ignored: lines 364-367 re-read `context["element_classes"][key]` three more times. | `element_classes = context.get("element_classes") or {}` / `if key in element_classes:` / `return context["element_classes"][key]` |
| P1-27 | src/unfold/sites.py:453 | #32 | `_replace_values` is defined and referenced nowhere — not in `src/`, `tests/`, or any template. Dead method carrying its own untyped signature. | `def _replace_values(self, target: dict, source: dict, request: HttpRequest):` |
| P1-28 | src/unfold/sites.py:210 | #19 | `pks` is a list; `item.pk in pks` is a linear scan inside the per-result loop that also appends to it — quadratic in the number of search hits. | `for item in search_results:` / `if item.pk in pks: continue` / `pks.append(item.pk)` |
| P1-29 | src/unfold/sites.py:191 | #41 | `[m.lower() for m in allowed_models]` is rebuilt for every model of every app on every command-search request; it is loop-invariant and belongs above the loops (or as a set). | `if model["model"]._meta.label.lower() not in [m.lower() for m in allowed_models]:` |
| P1-30 | src/unfold/sites.py:334 | #34 | Badge callbacks whose import fails are swallowed with a bare `pass`, leaving no badge and no signal; the same swallow appears at 376, 443, 592 and utils.py:256. | `except (ImportError, ValueError):` / `pass` |
| P1-31 | src/unfold/sites.py:330 | #11 | The badge-callback block at 330-335 is identical to 372-377 (group vs item). | `if "badge" in group and isinstance(group["badge"], str):` / `callback = import_string(group["badge"])` |
| P1-32 | src/unfold/sites.py:512 | #33 | `_get_config` returns a value on the true branch and falls off the end otherwise; the `-> Any` annotation hides the None path from every one of its ~25 callers. | `if key in config and config[key]:` / `return self._get_value(config[key], *args)` |
| P1-33 | src/unfold/utils.py:248 | #11 | `resolve_setting_value` is a byte-identical copy of `UnfoldAdminSite._get_value` (sites.py:584-600), and `get_setting_value` (utils.py:240) mirrors `_get_config` (sites.py:512). Two homes for the settings-resolution rule. | `def resolve_setting_value(value: str \| Callable \| None, *args: Any) -> Any:` |
| P1-34 | src/unfold/sites.py:540 | #9 | `_get_colors` mutates the dict returned by `get_config` in place — that dict shares its nested `COLORS` sub-dicts with the module-level `CONFIG_DEFAULTS` (settings.py:44), so a request rewrites the process-wide defaults. `get_tabs_list` (405) deep-copies for exactly this reason. | `colors[name] = color_weights` / `colors[name][weight] = convert_color(value)` |
| P1-35 | src/unfold/sites.py:446 | none | If `import_string` failed at 442 the callback is still a `str`; the `isinstance(callback, str)` arm then calls a string via `lazy(callback)(request)`. The arm can only produce a TypeError. | `if isinstance(callback, str) or isinstance(callback, Callable):` / `if lazy(callback)(request) == True:  # noqa: E712` |
| P1-36 | src/unfold/sites.py:36 | #24 | Eight runtime name resolutions in one module (36, 109, 136, 332, 374, 442, 590, 619): login form, global callback, dashboard callback, badges, permissions, site views. None is greppable from its definition. | `self.login_form = import_string(custom_login_form)` |
| P1-37 | src/unfold/sites.py:512 | #1 | `_get_config(self, key: str, *args: Any) -> Any` and the five helpers built on it (518, 532, 544, 552, 568) publish `*args: Any` as their whole contract. | `def _get_config(self, key: str, *args: Any) -> Any:` |
| P1-38 | src/unfold/sites.py:518 | #37 | That `*args` is monomorphic: every call site in `each_context` and elsewhere passes exactly `(request,)`. The variadic exists for no exercised caller. | `def _get_theme_images(self, key: str, *args: Any) -> dict[str, str] \| str \| None:` |
| P1-39 | src/unfold/utils.py:230 | #2 | `value` is annotated `str`, so `isinstance(value, str)` is discharged by the signature; two occurrences (230, 232). | `def convert_color(value: str) -> str:` / `elif isinstance(value, str) and all(part.isdigit() for part in value.split()):` |
| P1-40 | src/unfold/utils.py:32 | #32 | `empty_value_display` is accepted and never read by `display_for_header` (32), `display_for_dropdown` (46) and `display_for_label` (59) — three dead parameters on the same family. | `def display_for_header(value: Iterable, empty_value_display: str) -> SafeText:` |
| P1-41 | src/unfold/utils.py:195 | #11 | `parse_date_str` and `parse_datetime_str` (203) differ only in the settings key and the `.date()` call. | `for format in settings.DATE_INPUT_FORMATS:` / `return datetime.datetime.strptime(value, format).date()` |
| P1-42 | src/unfold/utils.py:195 | #33 | Both parsers are annotated `-> ... \| None` and reach that None only by falling off the end of the loop — the contract is never written down. | `except (ValueError, TypeError):` / `continue` |
| P1-43 | src/unfold/utils.py:155 | #11 | `prettify_json.format_response` (155-162) and `prettify_traceback.format_response` (180-187) are identical except the lexer, and the two-`div` `mark_safe` tail (166-169 / 189-192) is duplicated with it. | `formatter = HtmlFormatter(style=theme, noclasses=True, nobackground=True, ...)` / `return highlight(response, JsonLexer(), formatter)` |
| P1-44 | src/unfold/utils.py:24 | none | The fallback annotates the name with itself. On Python < 3.14 module-level annotations are evaluated eagerly, so importing `unfold.utils` without djmoney raises `NameError: MoneyField` — and the project claims 3.12/3.13 support. | `except ImportError:` / `MoneyField: type[MoneyField] \| None = None` |
| P1-45 | src/unfold/utils.py:240 | #1 | The real parameter is smuggled through `**kwargs` and popped, so the signature says nothing about it. | `def get_setting_value(key: str, *args: Any, **kwargs: Any) -> Any:` / `settings_name = kwargs.pop("settings_name", "UNFOLD")` |
| P1-46 | src/unfold/utils.py:241 | #37 | That `settings_name` knob has exactly one call site in the prod tree (mixins/formfield_model_admin.py:136) and it never passes it — a default nobody overrides. | `get_setting_value("SHOW_UI_WARNINGS", request) is True` |
| P1-47 | src/unfold/utils.py:113 | #23 | `display_for_field` is an 11-arm elif chain silenced with `# noqa: PLR0911, PLR0912`; `display_for_value` (88) is a second such chain with the same shape. | `def display_for_field(value: Any, field: Any, empty_value_display: str) -> str:  # noqa: PLR0911, PLR0912` |
| P1-48 | src/unfold/admin.py:223 | #32 | `default_choices` is overwritten on the first line of the body — the parameter is dead and the override is invisible from the signature. | `default_choices: list[tuple[str, str]] = BLANK_CHOICE_DASH,` / `default_choices = [("", _("Select action"))]` |
| P1-49 | src/unfold/admin.py:223 | #9 | The default value is `BLANK_CHOICE_DASH`, a module-level mutable list from `django.db.models`, bound as a default argument. | `default_choices: list[tuple[str, str]] = BLANK_CHOICE_DASH,` |
| P1-50 | src/unfold/admin.py:232 | #32 | `get_changelist` ignores both `request` and `**kwargs` and returns a constant. | `def get_changelist(self, request: HttpRequest, **kwargs: Any) -> type[ChangeList]:` / `return ChangeList` |
| P1-51 | src/unfold/admin.py:125 | #9 | Every changeform request rebinds two globals in `django.contrib.admin.helpers`; nothing restores them, and the effect is invisible to any reader of `helpers`. | `helpers.AdminForm = AdminForm  # ty:ignore` / `helpers.Fieldline = Fieldline  # ty:ignore` |
| P1-52 | src/unfold/admin.py:125 | #36 | Two checker-silencing pragmas on the two most surprising lines of the module; the tree carries 14 `type: ignore`/`ty:ignore` pragmas overall, clustered in mixins/action_model_admin.py (4) and contrib. | `helpers.AdminForm = AdminForm  # ty:ignore` |
| P1-53 | src/unfold/admin.py:177 | #11 | Three list comprehensions (177, 186, 195) differ only in the `_get_base_actions_*` call and the path prefix. | `path(f"{action.path.removesuffix('/')}/", wrap(action.method), name=action.action_name)` |
| P1-54 | src/unfold/admin.py:78 | #9 | `readonly_preprocess_fields = {}` (78, and again at 258) and `list_filter_options: dict[...] = {}` (68) are mutable class attributes shared by every subclass and instance. | `list_filter_options: dict[str, ListFilterOptionsItem] = {}` / `readonly_preprocess_fields = {}` |
| P1-55 | src/unfold/admin.py:82 | #1 | The `media` property has no return annotation, and the nested `wrap`/`wrapper` (165-166) are fully untyped in an otherwise annotated module. | `@property` / `def media(self):` |
| P1-56 | src/unfold/forms.py:231 | #11 | The GET block (232-234) and the POST block (236-238) are identical modulo `request.GET`/`request.POST`. | `page = self.request.GET.get(self.get_pagination_key())` / `if page and page.isnumeric() and page > "0": return int(page)` |
| P1-57 | src/unfold/forms.py:233 | none | `page > "0"` compares strings lexicographically: `"00"` passes the guard and yields `paginator.page(0)`, which raises. The intent is `int(page) > 0`. | `if page and page.isnumeric() and page > "0":` |
| P1-58 | src/unfold/forms.py:47 | #37 | A `pass`-only subclass that adds nothing over `ReadOnlyPasswordHashWidget`; the only use (125) could name the base directly. | `class UnfoldReadOnlyPasswordHashWidget(ReadOnlyPasswordHashWidget):` / `pass` |
| P1-59 | src/unfold/forms.py:286 | #32 | `get_before_template_context` and `get_after_template_context` (291) both return `{}` and read neither `request` nor `object_id` — two dead parameters each. | `def get_before_template_context(self, request: HttpRequest, object_id: int \| str \| None = None) -> dict[str, Any]:` / `return {}` |
| P1-60 | src/unfold/forms.py:296 | #11 | `render_before_template` and `render_after_template` (305) are identical modulo `before`/`after`. | `if self.form_before_template:` / `return render_to_string(self.form_before_template, self.get_before_template_context(...))` |
| P1-61 | src/unfold/forms.py:259 | #1 | A required argument is hidden in `**kwargs` and popped, so the signature advertises none of it. | `def __init__(self, *args: Any, **kwargs: Any) -> None:` / `search_var = kwargs.pop("search_var")` |
| P1-62 | src/unfold/forms.py:278 | #8 | `object_id: int \| str \| None` recurs as a raw primitive across forms.py:278/287/292, mixins/action_model_admin.py:152/170/325 and dataclasses.py:39 — a concept re-validated (`if object_id:`) at each site instead of being a type. | `object_id: int \| str \| None = None,` |
| P1-63 | src/unfold/fields.py:65 | #21 | `is_json` (65), `is_image` (74), `is_file` (83) and `wrapper_class` (91) all open with the same self-rooted guard plus tuple unpack — the invariant "resolved_field is a triple or False" is enforced at four call sites instead of in the type. | `if isinstance(self.resolved_field, bool) or not self.resolved_field: return False` / `f, attr, value = self.resolved_field` |
| P1-64 | src/unfold/fields.py:69 | #32 | Those unpacks bind `attr` and `value` and never read them (69, 78, 87); `_get_contents` (122) binds `_obj` and `_model_admin` and reads neither. | `f, attr, value = self.resolved_field` / `return isinstance(f, JSONField)` |
| P1-65 | src/unfold/fields.py:43 | none | `LABEL_CLASSES` already contains `"mb-2"` (widgets.py:59), so the concatenation emits the class twice. | `"class": " ".join(LABEL_CLASSES + ["mb-2"]),` |
| P1-66 | src/unfold/fields.py:188 | none | `False` is used as the failure sentinel of a tuple-returning accessor, so every consumer must first discriminate on `isinstance(..., bool)`; the `url` property (51) does the same with `str \| bool`. `None` (or a raised error) is the honest shape. | `def resolved_field(self) -> bool \| tuple[Field \| None, str \| None, Any]:` / `return False` |
| P1-67 | src/unfold/fields.py:107 | #1 | The only unannotated method in an otherwise fully annotated class. | `def get_admin_url(self, remote_field, remote_obj):` |
| P1-68 | src/unfold/fields.py:230 | #32 | `widget_attrs` ignores its `widget` argument and discards the base class's attrs instead of merging. | `def widget_attrs(self, widget: Widget) -> dict[str, Any]:` / `return {"data-ajax--url": reverse_lazy(self.url_path)}` |
| P1-69 | src/unfold/fields.py:120 | none | `unfold.utils` is already imported at module top (line 26); this function-local re-import breaks no cycle and hides the dependency from the import block. | `from unfold.utils import _boolean_icon` |
| P1-70 | src/unfold/views.py:22 | #13 | `ChangeList.__init__` forwards to `super().__init__` and adds nothing — a subclass that exists only to be named. | `def __init__(self, request: HttpRequest, *args: Any, **kwargs: Any) -> None:` / `super().__init__(request, *args, **kwargs)` |
| P1-71 | src/unfold/views.py:66 | #11 | `UnfoldSiteViewMixin` (66-94) and `UnfoldModelAdminViewMixin` (97-126) are the same class — same `__init__` shape, same two `UnfoldException` guards, same `current_app` write, same context update — with `admin_site` renamed to `model_admin`. | `raise UnfoldException("UnfoldSiteViewMixin was not provided with 'admin_site' argument")` |
| P1-72 | src/unfold/components.py:7 | #9 | `_registry` is a class-level mutable dict written by `register_component` from any importing module and read by the `{% component %}` tag — import-order-dependent global state. | `class ComponentRegistry:` / `_registry: dict[str, type] = {}` |
| P1-73 | src/unfold/components.py:37 | #13 | `register_component` (37) and `get_class` (24) are pure forwarders — one call, nothing added. | `def register_component(cls: type) -> type:` / `ComponentRegistry.register_class(cls)` / `return cls` |
| P1-74 | src/unfold/components.py:46 | #1 | The component extension point — the thing third parties override — is completely untyped and returns its kwargs unchanged. | `def get_context_data(self, **kwargs):` / `return kwargs` |
| P1-75 | src/unfold/sections.py:65 | #2 | `verbose_name` is declared as a class attribute at line 25, so `hasattr(self, "verbose_name")` is provably always True; identical dead guard for `height` at 68 (declared at 26). | `if hasattr(self, "verbose_name") and self.verbose_name:` |
| P1-76 | src/unfold/sections.py:23 | #9 | `fields = []` is a mutable class attribute; any subclass that mutates it instead of rebinding writes into the shared list. | `class TableSection(BaseSection):` / `fields = []` |
| P1-77 | src/unfold/sections.py:80 | #32 | `get_context_data` returns `{}` and ignores both `request` and `instance`. | `def get_context_data(self, request: HttpRequest, instance: Model) -> dict[str, Any]:` / `return {}` |
| P1-78 | src/unfold/sections.py:37 | #24 | Column resolution goes through `getattr(self, field_name)` with names drawn from `self.fields` (37-50), so a section's own methods are unfindable by grep from the field list. | `if hasattr(self, field_name):` / `row.append(getattr(self, field_name)(result))` |
| P1-79 | src/unfold/layout.py:33 | #11 | `FieldsetSubheader` (33-52) and `Hr` (55-74) are identical classes except the `template` string. | `def render(self, form: Form, context: RequestContext, template_pack: SimpleLazyObject = TEMPLATE_PACK, **kwargs: Any) -> str:` / `return render_to_string(self.template, {"title": self.title})` |
| P1-80 | src/unfold/layout.py:40 | #32 | `render` accepts `form`, `context`, `template_pack` and `**kwargs` and reads none of them — four dead parameters, twice (40 and 62). | `form: Form,` / `context: RequestContext,` / `template_pack: SimpleLazyObject = TEMPLATE_PACK,` |
| P1-81 | src/unfold/overrides.py:41 | #26 | The formfield table is a literal dict then mutated by two `try/except ImportError` `.update()` blocks (41, 57) and a `deepcopy`+`update` (68-73); a reader cannot read the mapping off the page. | `FORMFIELD_OVERRIDES.update({ArrayField: {"widget": widgets.UnfoldAdminTextareaWidget}, ...})` |
| P1-82 | src/unfold/overrides.py:9 | #9 | That same module-level dict is imported by mixins/formfield_model_admin.py:24 and mutated at import time by whichever optional packages happen to be installed. | `FORMFIELD_OVERRIDES: dict[Any, Any] = {` |
| P1-83 | src/unfold/settings.py:35 | #26 | `CONFIG_DEFAULTS["FORMS"]["classes"]` is computed by seven `" ".join(...)` calls at import time, which also drags all 1004 lines of widgets.py into every `unfold.settings` import. | `"prose": " ".join(PROSE_CLASSES),` / `"text_input": " ".join(INPUT_CLASSES),` |
| P1-84 | src/unfold/paginator.py:6 | #1 | The whole module (6, 17, 20) is unannotated, including `_get_page(*args, **kwargs)`, in a tree that otherwise annotates. | `def has_next(self):` / `def _get_page(self, *args, **kwargs):` |
| P1-85 | src/unfold/widgets.py:1 | #29 | No module in `src/unfold/` carries a module docstring — including widgets.py (1004 lines), templatetags/unfold.py (890) and sites.py (629). Nothing tells a reader what a module is before they read it. | `import json` |
| P1-86 | src/unfold/widgets.py:42 | #27 | 1004 lines that every consumer must ingest to reach one constant: `INPUT_CLASSES` alone is imported by settings.py, forms.py, fields.py, layout.py, contrib/filters/forms.py and contrib/forms/widgets.py. The class-name constants want their own module. | `BUTTON_CLASSES = [` |
| P1-87 | src/unfold/mixins/action_model_admin.py:193 | #11 | `_get_base_actions_list` (193), `_get_base_actions_detail` (202), `_get_base_actions_row` (211), `_get_base_actions_submit_line` (220) are four identical bodies differing only in the attribute read; `get_actions_list` (143), `get_actions_detail` (151), `get_actions_row` (161), `get_actions_submit_line` (169) are four more. | `return [self.get_unfold_action(action) for action in self._extract_action_names(self.actions_list)]` |
| P1-88 | src/unfold/mixins/action_model_admin.py:208 | #3 | `_extract_action_names` is annotated `-> list[str]` and already normalizes with `for action in actions or []` (185), so the trailing `or []` is a guard the callee's contract discharges — three of the four copies carry it, `_get_base_actions_list` (199) does not. | `for action in self._extract_action_names(self.actions_detail) or []` |
| P1-89 | src/unfold/mixins/action_model_admin.py:257 | #33 | `get_action_by_name` is annotated `-> UnfoldAction \| None` but reaches None only by falling off the loop; `build_dropdown` (293) does the same for `dict \| None`. | `def get_action_by_name(name: str) -> UnfoldAction \| None:` / `if action.action_name == full_action_name: return action` |
| P1-90 | src/unfold/mixins/action_model_admin.py:134 | #12 | Five `method.X if hasattr(method, "X") else Y` ternaries reimplement `getattr(method, "X", Y)` — which the line immediately above (133) already uses. | `attrs=method.attrs if hasattr(method, "attrs") else None,` / `icon=method.icon if hasattr(method, "icon") else None,` |
| P1-91 | src/unfold/mixins/action_model_admin.py:344 | #24 | Permission methods are reached by constructed name here, at decorators.py:51 and at checks.py:55 — three sites building `has_<x>_permission`, none greppable from the method definitions they target. | `permission_rules.append(getattr(self, f"has_{permission}_permission"))` |
| P1-92 | src/unfold/mixins/action_model_admin.py:321 | #11 | `_filter_unfold_actions_by_permissions` (321-359) is the same permission-rule/permission-check double loop as decorators.py:40-77, down to the `"." in permission` split and the `all(permission_checks)` test. | `for permission_rule in permission_rules:` / `if isinstance(permission_rule, str) and "." in permission_rule:` |
| P1-93 | src/unfold/mixins/formfield_model_admin.py:30 | #9 | `_autocomplete_fields_missing` is a *class* attribute, appended at 185 and removed at 152 through `self.`, so every `ModelAdmin` subclass in the process shares one warning list. | `_autocomplete_fields_missing: list[str] = []` / `self._autocomplete_fields_missing.append(field_name)` |
| P1-94 | src/unfold/mixins/formfield_model_admin.py:157 | #15 | The only use of `formfield` is `formfield.widget` (174) and it is never forwarded — the function demands the whole field to read one attribute. | `formfield: ModelChoiceField \| ModelMultipleChoiceField \| None,` / `if formfield is not None and isinstance(formfield.widget, FilteredSelectMultiple):` |
| P1-95 | src/unfold/mixins/nested_inlines_model_admin.py:61 | #9 | `django.contrib.admin.options.all_valid` is rebound on every changeform request and never restored; the comment concedes the design. | `# Monkey patch all_valid to do nested formsets validation. ...` / `options.all_valid = nested_all_valid` |
| P1-96 | src/unfold/mixins/nested_inlines_model_admin.py:96 | #11 | The "build nested formsets for each inline_class" loop appears twice in one method (96-117 and 120-140), identical but for the form it starts from. | `inline_formset.inline_type = "stacked"` / `if issubclass(inline_class, TabularInline): inline_formset.inline_type = "tabular"` |
| P1-97 | src/unfold/mixins/nested_inlines_model_admin.py:198 | #12 | `if not (...): return False` / `return True` is `return bool(...)` written the long way. | `if not (inline.has_view_or_change_permission(request, obj) or ...): return False` / `return True` |
| P1-98 | src/unfold/mixins/nested_inlines_model_admin.py:92 | #35 | Function-local import hiding a real cycle: `unfold.admin` → `unfold.mixins` → `nested_inlines_model_admin` → `unfold.admin`. The lazy import breaks the load order, not the dependency. | `from unfold.admin import TabularInline` |
| P1-99 | src/unfold/mixins/nested_inlines_model_admin.py:74 | none | The loop variable shadows the `form` parameter and the shadowed value is then passed to the recursive call (82); the fix was suppressed rather than made (`# TODO: fix linting error`). Same shape at 176. | `# TODO: fix linting error` / `for form in formset.forms:  # noqa: PLR1704` |
| P1-100 | src/unfold/mixins/dataset_model_admin.py:34 | #9 | `django.contrib.admin.views.main.IGNORED_PARAMS` is rebound per changeform request from the module-level tuple. | `main.IGNORED_PARAMS = (*IGNORED_PARAMS, *ignored_params)` |
| P1-101 | src/unfold/mixins/dataset_model_admin.py:32 | none | `"_changelist_filters"` is appended once per dataset although it does not vary with the dataset — the list gets N duplicates. | `for dataset in datasets:` / `ignored_params.append("_changelist_filters")` |
| P1-102 | src/unfold/contrib/filters/admin/mixins.py:56 | #11 | Ten `choices()` implementations share the same `add_facets`/`facet_counts` preamble and the same `yield {"form": self.form_class(label=_(" By %(filter_title)s ") % ..., name=..., data={...})}` tail: mixins.py:56, choice_filters.py:20/72/107/135, dropdown_filters.py:25/79/126, text_filters.py:24/54, datetime_filters.py:77/180, numeric_filters.py:72/147. | `add_facets = getattr(changelist, "add_facets", False)` / `facet_counts = self.get_facet_queryset(changelist) if add_facets else None` |
| P1-103 | src/unfold/contrib/filters/admin/numeric_filters.py:28 | #14 | The six-parameter group `(field, request, params, model, model_admin, field_path)` is repeated on seven `__init__` signatures: numeric_filters.py:28/103/133, datetime_filters.py:22/101, dropdown_filters.py:107, text_filters.py:38. | `field: Field, request: HttpRequest, params: dict[str, str], model: type[Model], model_admin: ModelAdmin, field_path: str,` |
| P1-104 | src/unfold/contrib/filters/admin/datetime_filters.py:120 | #11 | Four identical four-line blocks for `_from_0`, `_from_1`, `_to_0`, `_to_1` (120-138); the same shape appears twice more at 41-53. | `if self.parameter_name + "_from_0" in params:` / `value = params.pop(self.field_path + "_from_0")` |
| P1-105 | src/unfold/contrib/filters/admin/datetime_filters.py:41 | none | The membership test uses `self.parameter_name` but the pop uses `self.field_path`; the class explicitly allows `parameter_name` to be overridden (38-39), and the two keys then disagree, giving a KeyError. Same mismatch at 48 and 120-138. | `if self.parameter_name + "_from" in params:` / `value = params.pop(self.field_path + "_from")` |
| P1-106 | src/unfold/contrib/filters/admin/numeric_filters.py:39 | #11 | The same `isinstance(...)`/`raise TypeError(f"Class {type(self.field)} is not supported for ...")` guard appears four times: numeric_filters.py:39, 113 and datetime_filters.py:32, 111. | `if not isinstance(field, DecimalField \| IntegerField \| FloatField \| AutoField):` / `raise TypeError(f"Class {type(self.field)} is not supported for {self.__class__.__name__}.")` |
| P1-107 | src/unfold/contrib/filters/admin/numeric_filters.py:56 | #33 | `queryset` is `-> QuerySet \| None` and `value` (65) is `-> Any`; both return on the guarded path and fall off the end otherwise, so "no filter" and "error" both surface as an implicit None. | `if self.value() and self.parameter_name:` / `return queryset.filter(**{self.parameter_name: self.value()})` |
| P1-108 | src/unfold/contrib/filters/admin/numeric_filters.py:148 | #41 | Rendering one slider filter issues three database round trips over the same queryset — `count()`, `aggregate(Min)`, `aggregate(Max)` — where a single `aggregate(min=Min(...), max=Max(...), total=Count(...))` gives all three. This runs on every changelist page that declares the filter. | `total = self.q.all().count()` / `min_value = self.q.all().aggregate(min=Min(self.parameter_name)).get("min", 0)` |
| P1-109 | src/unfold/contrib/filters/admin/numeric_filters.py:193 | #12 | A format-string round trip to compute `10 ** -precision`: build `"{:.6f}"`, format `0`, concatenate `"1"`, parse back to float. | `result_format = f"{{:.{precision - 1}f}}"` / `return float(result_format.format(0) + "1")` |
| P1-110 | src/unfold/contrib/filters/admin/dropdown_filters.py:63 | #2 | `parameter_name` is declared `str \| None` (mixins.py:83), so `is not None` already narrows it to `str` and the `isinstance` is dead. | `self.parameter_name is not None` / `and isinstance(self.parameter_name, str)` |
| P1-111 | src/unfold/contrib/filters/admin/dropdown_filters.py:43 | #12 | `self.multiple if hasattr(self, "multiple") else False` is `getattr(self, "multiple", False)`; repeated at 97, 146 and (with an extra isinstance) mixins.py:179. | `multiple=self.multiple if hasattr(self, "multiple") else False,` |
| P1-112 | src/unfold/contrib/filters/admin/dropdown_filters.py:73 | #11 | `ChoicesDropdownFilter.queryset` (73-77) and `RelatedDropdownFilter.queryset` (120-124) are byte-identical. | `if self.value() not in EMPTY_VALUES: return super().queryset(request, queryset)` / `return queryset` |
| P1-113 | src/unfold/contrib/filters/admin/mixins.py:160 | #32 | `field_choices` ignores all three parameters and returns a constant; `text_filters.py:21 lookups` ignores both of its own. | `def field_choices(self, field: RelatedField, request: HttpRequest, model_admin: ModelAdmin) -> list[tuple]:` / `return [("", BLANK_CHOICE_DASH)]` |
| P1-114 | src/unfold/contrib/filters/admin/mixins.py:106 | none | `value_from` already holds the value the guard tested; the dict then re-reads the same key from `used_parameters`. Same at 116 for `value_to`. | `value_from = self.used_parameters.get(f"{self.parameter_name}_from", None)` / `f"{self.parameter_name}__gte": self.used_parameters.get(f"{self.parameter_name}_from", None),` |
| P1-115 | src/unfold/contrib/filters/admin/choice_filters.py:147 | #32 | The `enumerate` index is bound and never used. | `choices = [[val, val] for _i, val in enumerate(self.lookup_choices)]` |
| P1-116 | src/unfold/contrib/filters/admin/choice_filters.py:28 | none | This branch tests only `add_facets` while every sibling (72-76, 108-111, 136-139, and mixins.py:62) tests `add_facets and facet_counts`; when `get_facet_queryset` returns None the `.get` on line 31 raises. | `if add_facets:` / `choices.append((lookup, f"{title} ({facet_counts.get(f'{i}__c', '-')})"))` |
| P1-117 | src/unfold/contrib/filters/admin/__init__.py:40 | none | `"MultipleRelatedCheckboxFilter"` is exported but defined nowhere in the repo — `from ...filters.admin import *` raises `AttributeError`, and the name is a false promise to every reader of the public surface. | `"MultipleRelatedCheckboxFilter",` |
| P1-118 | src/unfold/contrib/filters/admin/__init__.py:51 | none | `"RangeDateFilter"` is listed twice in the same `__all__`. | `"RangeDateFilter",` / `"RangeDateFilter",` |
| P1-119 | src/unfold/contrib/filters/forms.py:118 | #9 | `DropdownForm.widget` (118) and `HorizontalRadioForm.widget` (114) hold widget *instances* as class attributes. Django mutates `widget.attrs` during rendering, so one form's classes leak into every other instance in the process; the sibling classes (87, 109) correctly hold the class. | `class DropdownForm(forms.Form):` / `widget = UnfoldAdminSelectWidget(attrs={"data-theme": "admin-autocomplete", ...})` |
| P1-120 | src/unfold/contrib/filters/forms.py:226 | #37 | A `pass`-only subclass with no behavior of its own; its sole consumer (numeric_filters.py:131) could name `RangeNumericForm`. | `class SliderNumericForm(RangeNumericForm):` / `pass` |
| P1-121 | src/unfold/contrib/filters/forms.py:230 | #11 | `RangeDateForm` (230-260) and `RangeDateTimeForm` (263-305) are the same `_from`/`_to` field-pair construction twice, and within each the two field blocks are themselves copies. | `self.fields[self.name + "_from"] = forms.DateField(` / `self.fields[self.name + "_to"] = forms.DateField(` |
| P1-122 | src/unfold/contrib/filters/forms.py:26 | #1 | Every public form constructor in the module (26, 49-50, 94-95, 132-133, 170, 189-190, 237, 270) ends in bare unannotated `*args, **kwargs`. | `def __init__(self, name: str, label: str, *args, **kwargs) -> None:` |
| P1-123 | src/unfold/contrib/filters/admin/mixins.py:45 | #38 | The literal `"unfold/filters/filters_field.html"` is declared as a `template` class attribute in eight classes across four modules (mixins.py:45/51, choice_filters.py:16/68/104/132, dropdown_filters.py:21, text_filters.py:15/35) — one fact, eight homes. | `template = "unfold/filters/filters_field.html"` |
| P1-124 | src/unfold/contrib/inlines/admin.py:68 | #34 | Commented-out code left inside a live dict literal. | `# "fk_name": self.fk_name,` |
| P1-125 | src/unfold/contrib/inlines/admin.py:29 | #1 | Neither `get_formset` (29) nor `_get_formset_defaults` (48) declares a return type, though both return structures other code destructures. | `def get_formset(self, request: HttpRequest, obj: Model \| None = None, **kwargs: Any):` |
| P1-126 | src/unfold/contrib/inlines/checks.py:8 | #32 | Both check overrides return `[]` and read neither parameter — four dead parameters in a 16-line module. | `def _check_exclude_of_parent_model(self, obj: InlineModelAdmin, parent_model: Model) -> list[CheckMessage]:` / `return []` |
| P1-127 | src/unfold/contrib/inlines/forms.py:40 | #33 | Annotated `-> BaseModelFormSet` but returns the *class* `modelformset_factory` produced, i.e. `type[BaseModelFormSet]`; the same confusion appears on `cls: BaseModelFormSet` at line 21. | `def nonrelated_inline_formset_factory(...) -> BaseModelFormSet:` / `inline_formset = modelformset_factory(model, formset=formset, **kwargs)` |
| P1-128 | src/unfold/contrib/inlines/forms.py:14 | #32 | `save_as_new` is accepted and never read or forwarded to `super().__init__`. | `save_as_new: bool = False,` |
| P1-129 | src/unfold/contrib/forms/widgets.py:103 | #34 | Both arms of the conditional expression are `w`, so the comprehension rebuilds the identical list — a statement with no effect. | `self.widgets = [w if isinstance(w, type) else w for w in self.widgets]` |
| P1-130 | src/unfold/contrib/forms/widgets.py:54 | #2 | `widget_class` and `choices` are both assigned unconditionally in `__init__` (47-48), so both `hasattr` guards are provably always True. | `if hasattr(self, "widget_class") and self.widget_class is not None:` / `if hasattr(self, "choices") and self.choices is not None:` |
| P1-131 | src/unfold/contrib/forms/widgets.py:70 | #32 | `files` is accepted and unused in `value_from_datadict` (70) and `value_omitted_from_data` (81). | `def value_from_datadict(self, data: QueryDict, files: MultiValueDict, name: str) -> list:` |
| P1-132 | src/unfold/contrib/import_export/forms.py:36 | #12 | `True if X else False` where `X` is already the boolean. | `"readonly": True if len(format_choices) == 1 else False,` |
| P1-133 | src/unfold/contrib/import_export/forms.py:18 | #1 | The whole module (18, 42, 57) is unannotated, including the public `SelectableFieldsExportForm(formats, resources, **kwargs)` constructor. | `def __init__(self, formats, resources, **kwargs):` |
| P1-134 | src/unfold/contrib/import_export/forms.py:23 | #36 | Checker-silencing pragmas cluster at the third-party boundary: 23 and 26 here, contrib/forms/widgets.py:72 and 83, templatetags/unfold_list.py:10 — each one blinds the oracle at exactly the seam where the types are least known. | `choices=self.fields["resource"].choices  # ty:ignore[unresolved-attribute]` |
| P1-135 | src/unfold/templatetags/unfold_list.py:315 | none | The `if`/`else` yield byte-identical expressions — the branch cannot change the output. | `if field_index != 0:` / `yield format_html("<td{}>{}</td>", row_class, result_repr)` / `else: yield format_html("<td{}>{}</td>", row_class, result_repr)` |
| P1-136 | src/unfold/templatetags/unfold_list.py:196 | #41 | Three loop-invariant lookups (one of them a method call) re-evaluated once per column per row: a 100-row × 10-column changelist runs `get_empty_value_display()` 1000 times instead of once. This is the changelist render path. | `empty_value_display = cl.model_admin.get_empty_value_display()` / `ordering_field = getattr(cl.model_admin, "ordering_field", None)` |
| P1-137 | src/unfold/templatetags/unfold_list.py:111 | #2 | Every `property` object has `fget`, so the `hasattr` conjunct is discharged by the `isinstance` that precedes it. | `if isinstance(attr, property) and hasattr(attr, "fget"):` |
| P1-138 | src/unfold/templatetags/unfold_list.py:182 | #23 | `items_for_result` is 148 lines with five nesting levels, silenced by `# noqa: PLR0915, PLR0912`; `result_headers` (75-179) is a second 105-line generator with the same shape. | `def items_for_result(  # noqa: PLR0915, PLR0912` |
| P1-139 | src/unfold/templatetags/unfold_list.py:328 | #34 | Code the authors cannot show is reachable, kept with a TODO and excluded from coverage — git remembers it; the module should not. | `# TODO: find out when this line of code is executed` / `if form and not form[cl.model._meta.pk.name].is_hidden:  # pragma: no cover` |
| P1-140 | src/unfold/templatetags/unfold_list.py:375 | #11 | Three `if DJANGO_VERSION >= (6, 1): return InclusionAdminNode(...)` / `return InclusionAdminNode(...)` pairs (375-390, 503-520, 523-538), each duplicating all of its kwargs across the two branches. | `if DJANGO_VERSION >= (6, 1):` / `return InclusionAdminNode("unfold_result_list", parser, token, func=result_list, template_name="change_list_results.html")` |
| P1-141 | src/unfold/templatetags/unfold_list.py:464 | #11 | `unfold_horizontal_filters` (464-480) and `unfold_vertical_filters` (483-500) are the same walk over `cl.filter_specs` with complementary predicates; the `field_path` preamble is a third copy of lines 442-445. | `for spec in cl.filter_specs:` / `if hasattr(spec, "field_path"): field_path = spec.field_path` |
| P1-142 | src/unfold/templatetags/unfold_list.py:449 | none | `field_path` is bound only inside two `hasattr` branches with no `else`, so a filter spec carrying neither attribute reaches line 449 with the name unbound (`UnboundLocalError`). Same hole at 474 and 495. | `elif hasattr(spec, "parameter_name"): field_path = spec.parameter_name` / `if field_path:` |
| P1-143 | src/unfold/templatetags/unfold_list.py:75 | #1 | Five public functions in an otherwise annotated module carry no annotations at all: `result_headers` (75), `results` (344), `unfold_search_form` (424), `unfold_search_form_tag` (504), `unfold_admin_actions_tag` (524). | `def result_headers(cl):` |
| P1-144 | src/unfold/contrib/filters/apps.py:4 | #11 | Ten `contrib/*/apps.py` AppConfig classes are the same three lines with a different `name`/`label` pair (constance, filters, forms, guardian, hijack, import_export, inlines, location_field, simple_history, waffle). | `class DefaultAppConfig(AppConfig):` / `name = "unfold.contrib.filters"` / `label = "unfold_filters"` |
| P1-145 | src/unfold/apps.py:15 | #9 | `ready()` rebinds two django globals so that every later `from django.contrib import admin; admin.site` resolves to the Unfold site — invisible at both definition sites. | `admin.site = site` / `sites.site = site` |
| P1-146 | src/unfold/decorators.py:32 | #33 | `decorator` is annotated `-> Action` but returns `inner`, a plain function with attributes stapled on (102-128). `dataclasses.py:35` propagates the lie as `method: Action`, and four `# type: ignore` in action_model_admin.py pay for it. | `def decorator(func: Callable) -> Action:` / `return inner` |
| P1-147 | src/unfold/decorators.py:117 | #12 | `if x is not None: y = x else: y = DEFAULT` is `y = x or DEFAULT`; the `extra_options` pair at 122-125 is the same shape inverted. | `if variant is not None: inner.variant = variant` / `else: inner.variant = ActionVariant.DEFAULT` |
| P1-148 | src/unfold/decorators.py:47 | #7 | A three-line comment narrates the permission-method contract (`has_<some>_permission(self, request, obj=None)`) that line 51 then builds by string; the protocol is prose plus a `getattr`, never a type. | `# Permissions methods have following syntax: has_<some>_permission(self, request, obj=None):` |
| P1-149 | src/unfold/checks.py:13 | #1 | `**kwargs` is entirely unannotated on a public check entry point, and `_check_unfold_action_permission_methods` (19) takes `obj: Any` though it is always a `BaseModelAdmin`. | `def check(self, admin_obj: BaseModelAdmin, **kwargs) -> list[CheckMessage]:` |

## Phase 2 — audit finding verdicts

531 findings judged. Large homogeneous families grouped into one row with an
instance count + named spot-checks (counts reconcile to the audit total).
Verdict `real` = a competent reviewer would flag the site; `fp` = the site is
not a genuine shortfall (framework-dictated signature, oracle over-narrowing,
idiomatic, or a legitimate design).

### Proved tier (15 — judged individually; the tier claims soundness)

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| src/unfold/fields.py:66 (isinstance) | 2 | proved | fp | Oracle typed `self.resolved_field` as its `@cached_property` **function** (`() -> bool\|tuple`), not the descriptor value; at runtime it returns `False` on lookup failure, so `isinstance(..., bool)` is live. Unsound. |
| src/unfold/fields.py:66 (cond-always-true) | 2 | proved | fp | Same root cause: `not self.resolved_field` is "always True" only because the property is read as a function object; the value is genuinely falsy when `lookup_field` raises. |
| src/unfold/fields.py:75 (isinstance) | 2 | proved | fp | Same cached_property misresolution (`is_image`). |
| src/unfold/fields.py:75 (cond-always-true) | 2 | proved | fp | Same. |
| src/unfold/fields.py:84 (isinstance) | 2 | proved | fp | Same (`is_file`). |
| src/unfold/fields.py:84 (cond-always-true) | 2 | proved | fp | Same. |
| src/unfold/fields.py:92 (isinstance) | 2 | proved | fp | Same (`wrapper_class`). |
| src/unfold/fields.py:92 (cond-always-true) | 2 | proved | fp | Same. |
| src/unfold/fields.py:128 (isinstance) | 2 | proved | fp | Same, in `_get_contents`. The guard `isinstance(self.resolved_field, bool)` is the real discriminator between the `False` sentinel and the tuple. |
| src/unfold/contrib/filters/admin/mixins.py:31 | 2 | proved | fp | `ValueMixin.lookup_val = None` is a class default; subclasses set `self.lookup_val = params.get(...)` (text_filters.py:48) to a list at runtime. Oracle narrows to `None` from the default and calls the `isinstance(list)` guard dead — unsound. |
| src/unfold/mixins/action_model_admin.py:315 | 2 | proved | real | `nav_item: str \| dict`; after `if isinstance(nav_item, str)` (312), the `elif isinstance(nav_item, dict)` is provably redundant per the annotation. Sound. |
| src/unfold/utils.py:230 | 2 | proved | real | `value: str` param, so `isinstance(value, str)` is discharged by the signature (my P1-39). |
| src/unfold/utils.py:232 | 2 | proved | real | Same, second occurrence in `convert_color`. |
| src/unfold/sites.py:597 | 2 | proved | real | After `is None` (585) and the `str` branch return (588), `value` is `Callable`; `isinstance(value, Callable)` is redundant. Sound. |
| src/unfold/utils.py:261 | 2 | proved | real | Same pattern in the `resolve_setting_value` duplicate. Sound. |

Proved tier: **5 real, 10 fp.** The 10 FPs share two unsound-narrowing roots — a `@cached_property` descriptor read as its underlying function, and a `= None` class default over-narrowed against runtime subclass assignment — both aggravated by the 434 unresolved imports (density 4.0) reported in provenance. The soundness claim does not hold on this repo.

### Heuristic / indexed families

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| src/unfold/admin.py:160 | 2 | heuristic | fp | `self.custom_urls is None` flagged always-False from the `()` class default, but `custom_urls` is an overridable class attribute a subclass may set to `None`; the guard is defensive. Same over-narrowing class as the proved FPs. |
| #1 own weak boundary — 34 findings (rep. src/unfold/utils.py:113) | 1 | heuristic | real | unfold's own functions publishing `Any`/bare containers a narrower type would fix. Spot-checks: utils.py:113 `display_for_field(value: Any, field: Any)`; utils.py:89 `display_for_value`; utils.py:59 `display_for_label`; decorators.py:26/30 `action` attrs/extra_options; settings.py:112/116 `get_config`/`merge_dicts` dict[str,Any]; sites.py:319/404 sidebar/tabs returns; templatetags/unfold.py:176 `has_nav_item_active(items: list)`; unfold.py:775 `has_nested_tables(table: dict)`; components.py:28/46 `create_instance`/`BaseComponent.get_context_data`; sections.py:80 `TemplateSection.get_context_data`; contrib/forms/widgets.py:53 `get_widget_instance`. |
| #1 framework-override boundary — 38 findings (rep. src/unfold/widgets.py:290) | 1 | heuristic | fp | `Any`/`**kwargs`/bare return matching the Django (or inclusion-tag) superclass signature — not narrowable without breaking LSP. Spot-checks: widgets get_context 290/414/415/623/624/790 (Django Widget); each_context 49, index 120, changeform_view admin.py:121/122 + mixins 21/77/78/57, changelist_view 28, save_model 108, password_change 302; get_context_data views.py:73/104 (ContextMixin); check checks.py:13, get_urls wrapper admin.py:166; result_list unfold_list.py:353; lookups text_filters.py:21; widget_attrs fields.py:230; get_formset_kwargs admin.py:237; queryset numeric_filters.py:57; ArrayWidget value_from_datadict:72/decompress:86/get_context:62. |
| src/unfold/templatetags/unfold.py:199 / :204 / :209 | 1 | heuristic | fp | `class_name`/`is_list`/`is_dict(value: Any)` are type-testing/repr filters that must accept `Any` by purpose (3 findings). |
| #11 prod structural clones — 55 findings (rep. src/unfold/widgets.py:332) | 11 | indexed | real | Genuine normalized-AST clone groups in prod. Spot-checks: widgets.py:332/346/371/401/641/653/665 (input __init__ x7); widgets.py:452/479/499/526 (date/time __init__ x4); widgets.py:691/732 and 703/744 (select / select2 __init__); templatetags/unfold.py:176/185; utils.py:155/180 (prettify_json/traceback); layout.py:36/58; fields.py:65/74 (is_json/is_image); datetime_filters.py:121/126/131/136; numeric_filters.py:39/113 + datetime_filters.py:32/111 (type-guard); dropdown_filters.py:73/120; mixins.py:57 + dropdown_filters.py:26/80; unfold_list.py:376/524. All match my P1 clone sites. |
| #11 test/example clones — 129 findings (rep. tests/test_actions.py:69) | 11 | indexed | real | Genuine clones, but in test/example scaffolding (repeated fixture setup + assert blocks). Spot-checks: server/example/admin.py:545/564/582/601/623/645/663/682 (action handlers x8); test_ui_warnings.py:20/46/53/79/92/105; test_actions.py:124/148/191/210; test_colors.py:31/75/118/161; test_fields.py:101/113/129/145; test_views.py:7/19/29/36/48/58; test_tags.py:558/572/586; test_site_branding.py:18/90. Lower salience (test duplication is common) but the detector is correct that they are clones. |
| #27 purchase-price — 43 findings (rep. src/unfold/widgets.py:107) | 27 | indexed | real | Accurate context-economics metric: widgets.py (1004 lines; INPUT_CLASSES fan-in 18), sites.py (629; UnfoldAdminSite fan-in 101), example/admin.py (1140). Matches P1-86/27. Genuine large-module hot symbols. |
| #29 top-loading — 38 findings (rep. src/unfold/widgets.py:1) | 29 | heuristic | real | 19 modules with no top-loading docstring (widgets 1004, unfold.py 890, sites 629 — my P1-85) + 19 heavy entry points declaring no cost (items_for_result 148, header_title 136, action 114). Accurate; REPORT-tier presence exception. |
| #23 cognitive-complexity — 24 findings (rep. src/unfold/templatetags/unfold_list.py:182) | 23 | heuristic | real | Genuine high-complexity functions: items_for_result 73, display_for_field 72, action 58, display_for_value 46, _get_contents 43, result_headers 34, _get_tabs_list 33 … checks.py:19 at exactly 15. Accurate measurements. |
| #24 dynamic-identifiers — 14 findings (rep. src/unfold/sections.py:37) | 24 | heuristic | real | All genuine getattr/hasattr on constructed/dynamic names: decorators.py:51 + action_model_admin.py:344 + checks.py:56 `has_{permission}_permission`; sections.py:32/37/38/39/49/50 `getattr(self, field_name)`; fields.py:56/57 dynamic field; action_model_admin.py:235 `getattr(self, method_name)`; unfold_list.py:236 `getattr(result, f.name)`; settings.py:131 `getattr(settings, settings_name)`. |
| src/unfold/templatetags/unfold.py:569 | 30 | heuristic | real | 5-hop `field.field.field.widget.widget.attrs` through unfold's own AdminField/wrapper nesting. |
| src/unfold/fields.py:212 | 30 | heuristic | real | `self.field.field.required` reach through unfold's AdminField wrapper. |
| #30 Demeter through framework API — 15 findings (rep. src/unfold/templatetags/unfold_list.py:329) | 30 | heuristic | fp | Deep reaches the Django ORM/field API forces: `.model._meta.*` (unfold_list 329/407, get_unfold_action, get_admin_url, datasets 94/98, get_default_prefix), `.remote_field.model` (AutocompleteDropdownForm), `.instance._state` (has_changed), `field.field.widget.attrs` (add_css_class), inlines hand_clean_DELETE `self._meta` x2, get_context_data admin_site chain. `_meta` is the API — not narrowable. |
| #14 data-clump (genuine) — 2 findings | 14 | indexed | real | datetime_filters.py:22 `(model, model_admin, params, request)` recurs in 9 filter `__init__`s (my P1-103); filters/forms.py:40 `(choices, label, name)` in 3 filter forms. |
| #14 data-clump (framework/fixture) — 11 findings (rep. tests/test_filters.py:42) | 14 | indexed | fp | Groups dictated by pytest fixtures (test_filters/test_inlines/test_nonrelated/test_tags — `admin_client, ..._factory`) or Django method signatures (`request, obj, ...` in save_model/changeform_view/get_formset_kwargs/get_context, example admin action handlers) — not a concept wanting a type. |
| #26 declaration-literalness — 9 findings (rep. src/unfold/widgets.py:100) | 26 | heuristic | real | Class-list constants assembled by `[*BASE, ...]` splat (widgets BASE_INPUT/INPUT/DATETIME/COLOR/READONLY/TEXTAREA/SELECT, forms WYSIWYG) + `conftest.py:14` star import. A reader must execute the splat to know members. |
| #40 naming-proxies — 8 findings (rep. src/unfold/templatetags/unfold.py:395) | 40 | heuristic | fp | `*_classes`/`querystring_params`/`hex_to_values`/`contents` return a pre-joined string for template `class="..."`/query-string/HTML output — the plural denotes joined content, not a collection contract; no call site misreads. Idiomatic template-tag returns. |
| src/unfold/paginator.py:20 | 13 | indexed | fp | `_get_page` overrides Django's `Paginator._get_page` to substitute `InfinitePage` — a purposeful override, not a forward-only wrapper. |
| src/unfold/contrib/waffle/admin.py:22 | 13 | indexed | fp | `formfield_for_dbfield` deliberately skips waffle's impl via `super(ModelAdmin, self)` (MRO surgery) — adds real behavior. |
| #13 thin forwarders — 3 findings (rep. tests/server/example/admin.py:174) | 13 | indexed | real | Single-forward bodies in example/test code: ProjectNonrelatedInline.get_count, UserAdmin.display_datetime, test InlineWithoutSaveNewInstance.get_form_queryset. Low value (example code) but correct detection. |
| #32 dead-symbols — 4 findings (rep. src/unfold/utils.py:32) | 32 | indexed | real | widgets.py:113 INPUT_CLASSES_READONLY never referenced (verified: only its definition exists); display_for_header/label/dropdown `empty_value_display` never read (my P1-40). |
| #21 distributed-invariant — 4 findings (rep. src/unfold/fields.py:38) | 21 | heuristic | real | `isinstance(self.resolved_field, bool)...` in 4 methods + `self.field['field']` in 3 (my P1-63); `self.get_unfold_action(action)` in 4 (P1-87); `self._get_config(key,*args)` in 4 (P1-37). Genuine repeated self-rooted patterns. |
| #6 dishonest-accessor — 3 findings (rep. src/unfold/views.py:45) | 6 | indexed | fp | `get_results`/`get_queryset` (Django ChangeList overrides) and `get_context` (Django Widget) — the "get_" name and the mutation are inherited from Django's contract, not unfold's naming choice. |
| #28 doc-path-integrity — 2 findings | 28 | indexed | real | docs/.../django-waffle.md:9 names `unfold.model.ModelAdmin` (typo for `unfold.admin`); django-import-export.md:28 names `unfold.contrib.import_export.admin` — no such module exists (only forms.py/apps.py). Both genuinely unresolvable. |
| #16 mutation-tail — 2 findings | 16 | heuristic | real | nested_inlines `_get_nested_formset` (8 pure then mutate) and formfield `_check_autocomplete_field` (6 pure then append) — accurate structural shape. |
| src/unfold/sites.py:366 | 39 | heuristic | real | Comment `# Permission callback` restates `item["has_permission"] = self._call_permission_callback(...)`. |
| src/unfold/admin.py:153 | 39 | heuristic | fp | `get_custom_urls`'s 6 doc lines document the `custom_urls` tuple format (a useful contract), not restatement/history. |
| #36 type-lie-density — 2 findings | 36 | heuristic | real | action_model_admin silences the checker 3x (P1-52) and inlines `get_formset` returns Unknown into typed callers (P1-125) — genuine Any-laundering / pragma density. |
| #22 velcro-method — 2 findings | 22 | heuristic | fp | `create_instance` is a cohesive factory classmethod that belongs on the registry; `save_formset` is a required Django override hook — neither is a free function hiding in a class. |
| #12 idiom-catalog — 2 findings | 12 | heuristic | real | fields.py:176 `... in dict.keys()` (drop `.keys()`); import_export/forms.py:36 `True if X else False` → `bool(X)` (my P1-132). |
| src/unfold/mixins/action_model_admin.py:293 | 10 | indexed | real | `build_dropdown` demands concrete `dict` for `nav_item` where a `Mapping`/Protocol suffices (reads only `["title"]`, `.get`, `["items"]`). |
| src/unfold/contrib/inlines/admin.py:135 | 17 | heuristic | real | Live-variable neck at line 135 in `_get_formset_defaults` — accurate structural split point (REPORT-tier). |
| src/unfold/sites.py:210 | 19 | heuristic | real | Linear `item.pk in pks` scan inside the search-results loop — O(n·m) (my P1-28). |
| #15 pytest-fixture params — 53 findings (rep. tests/test_inlines.py:24) | 15 | heuristic | fp | Test params are pytest-django fixtures (`admin_client`/`client` used for `.force_login`/`.get`/`.post`) — the shape is fixture injection, not a narrowable wallet. |
| #15 framework-signature params — 4 findings (rep. src/unfold/templatetags/unfold.py:624) | 15 | heuristic | fp | `header_title(context)` (takes_context tag), `do_capture(parser)` (Django tag-compile signature), `unfold_horizontal/vertical_filters(cl)` (template filters receiving the ChangeList) — signatures fixed by the framework. |
| src/unfold/templatetags/unfold.py:43 | 15 | heuristic | real | `_count_errors_in_general` takes `admin_form` for only `.errors`/`.non_field_errors`, never forwarded — a genuine demand-narrowing candidate. |
| src/unfold/forms.py:242 | 15 | heuristic | real | `get_page(paginator, page)` uses only `.num_pages`/`.page` of the paginator — narrowable helper. |

**Phase 2 totals: 531 judged — 380 real, 151 fp.**

## Phase 3 — reconciliation

Each of my 149 phase-1 sites classed against the audit. `covered` = an audit
finding matches the site (same rule, or the same issue under another rule).
Misses: `detector-miss` (rule in inventory, fired elsewhere or nowhere, but not
here), `threshold-miss` (rule fired but a per-module threshold excluded this
site), `inventory-gap` (no rule covers it — all my `none` sites).

Systemic causes behind the misses:
- **#9, #33, #34, #35, #37, #8, #7, #3 produced zero findings repo-wide** — every P1 site mapped to them is a detector-miss. Notably #9 (shared-mutable-state) fired on none of ~10 module-level mutable class attrs / global monkeypatches; #33 (return-honesty) on none of the fall-off-the-end `-> X | None` functions; #34 (noop-code) on none of the commented-out code / dead branches.
- **#41 is silent by design** (provenance: "family P silent: no hot-roots config and no cost-declaring docstrings") — P1-29/108/136 are a principled coverage gap, not a detector bug.
- **#11** misses small cross-file/attribute clones (Media-class tuples, 3-line AppConfigs, one-line list-comprehension methods, in-line f-string dups) — its unit is the function/`__init__` body.
- **#32** fires only where a sibling shares the signature; it misses most unused params, unused unpack targets, and the dead method `_replace_values`.
- **#1** keys on `Any`/bare-container/opaque-`**kwargs`, so fully un-annotated params/returns (paginator.py, get_admin_url, infinite_paginator_url) slip through.
- **#24** covers getattr/hasattr but not `import_string(...)` dynamic resolution (8 sites in sites.py, plus section-class loading).
- **#12/#2** miss idioms/redundancies outside their catalogued shapes (`type(x) in (...)`, hasattr-on-declared-attr, manual `any()` loop, `if not: return False/return True`).

| P1 id | rule | class | note |
|-------|------|-------|------|
| P1-1 | 11 | covered | clone finding at widgets.py:332 |
| P1-2 | 3 | detector-miss | #3 produced zero findings repo-wide |
| P1-3 | 11 | covered | clone at 479 (date/time __init__ group) |
| P1-4 | 11 | covered | clone at 653 (Decimal __init__ group) |
| P1-5 | 11 | detector-miss | FileField/ImageSmall class clone too small (only template_name) |
| P1-6 | 11 | detector-miss | select2 `Media` class-tuple clone not a function body |
| P1-7 | 38 | detector-miss | #38 zero findings; js-asset lists live in Media classes, not module scope |
| P1-8 | none | inventory-gap | decompress returns bound methods — logic bug, no rule |
| P1-9 | 1 | covered | #1 get_context at widgets.py:290 |
| P1-10 | 1 | detector-miss | opaque `**kwargs` on widget `__init__` not flagged |
| P1-11 | none | inventory-gap | `*["form-check-input"]` splat — no rule |
| P1-12 | 11 | covered | clone at unfold.py:176 |
| P1-13 | 12 | detector-miss | manual `any()` loop not in the idiom catalog |
| P1-14 | 11 | detector-miss | cross-function partial clone (inner loop lifted) |
| P1-15 | 24 | detector-miss | `import_string(section_class)` not in #24's call set |
| P1-16 | 2 | detector-miss | `type(x) in (list,tuple)` not an isinstance/compare shape |
| P1-17 | none | inventory-gap | `set()` nondeterministic class order — no rule |
| P1-18 | 23 | covered | #23 complexity 24 at header_title:624 |
| P1-19 | none | inventory-gap | dead store (re-assigned) — #32 is name-liveness only |
| P1-20 | 30 | detector-miss | #30 fired on changeform_condition:569, missed changeform_data:553 (same shape) |
| P1-21 | 11 | detector-miss | in-line f-string dup across two branches, not a function clone |
| P1-22 | none | inventory-gap | missing `f` prefix — no rule |
| P1-23 | 33 | detector-miss | #33 zero findings |
| P1-24 | 1 | detector-miss | fully un-annotated params (no `Any`) outside #1's trigger |
| P1-25 | 15 | detector-miss | #15 fired elsewhere but not on infinite_paginator_url(cl) |
| P1-26 | none | inventory-gap | shadowed-then-ignored local — no rule |
| P1-27 | 32 | detector-miss | dead method `_replace_values` not caught by #32 |
| P1-28 | 19 | covered | #19 at sites.py:210 |
| P1-29 | 41 | detector-miss | family P silent by design (no hot-roots config) |
| P1-30 | 34 | detector-miss | #34 zero findings |
| P1-31 | 11 | detector-miss | badge-block dup (330 vs 372) not detected |
| P1-32 | 33 | detector-miss | #33 zero findings |
| P1-33 | 11 | covered | clone at utils.py:248 (resolve_setting_value/_get_value) |
| P1-34 | 9 | detector-miss | #9 zero findings |
| P1-35 | none | inventory-gap | dead isinstance-after-failed-import — subtle, no rule |
| P1-36 | 24 | detector-miss | `import_string` dynamic resolution not caught |
| P1-37 | 1 | detector-miss | `_get_config(*args: Any) -> Any` not flagged |
| P1-38 | 37 | detector-miss | #37 zero findings |
| P1-39 | 2 | covered | proved finding at utils.py:230 |
| P1-40 | 32 | covered | #32 at utils.py:32 (empty_value_display unread) |
| P1-41 | 11 | detector-miss | parse_date_str/parse_datetime_str clone not detected |
| P1-42 | 33 | detector-miss | #33 zero findings |
| P1-43 | 11 | covered | clone at utils.py:155 |
| P1-44 | none | inventory-gap | self-annotation NameError risk — no rule |
| P1-45 | 1 | covered | #1 return-Any at get_setting_value:240 |
| P1-46 | 37 | detector-miss | #37 zero findings |
| P1-47 | 23 | covered | #23 complexity 72 at display_for_field:113 |
| P1-48 | 32 | detector-miss | param overwritten-before-read not caught |
| P1-49 | 9 | detector-miss | #9 zero findings (BLANK_CHOICE_DASH mutable default) |
| P1-50 | 32 | detector-miss | unused `request`/`**kwargs` on get_changelist not caught |
| P1-51 | 9 | detector-miss | #9 zero findings (module-global monkeypatch) |
| P1-52 | 36 | threshold-miss | admin.py has 2 ignore pragmas; #36 threshold is 3/module |
| P1-53 | 11 | detector-miss | three near-identical url comprehensions not grouped |
| P1-54 | 9 | detector-miss | #9 zero findings (mutable class attrs) |
| P1-55 | 1 | detector-miss | missing return annotation on `media` property (not `Any`) |
| P1-56 | 11 | detector-miss | GET/POST block dup inside get_page_num not detected |
| P1-57 | none | inventory-gap | `page > "0"` string-compare bug — no rule |
| P1-58 | 37 | detector-miss | #37 zero findings (pass-only subclass) |
| P1-59 | 32 | detector-miss | dead request/object_id params not caught |
| P1-60 | 11 | detector-miss | render_before/after clone not detected |
| P1-61 | 1 | detector-miss | opaque `**kwargs` (search_var popped) not flagged |
| P1-62 | 8 | detector-miss | #8 zero findings |
| P1-63 | 21 | covered | #21 at fields.py:38 (resolved_field guard recurs in 4 methods) |
| P1-64 | 32 | detector-miss | unused unpack targets (attr/value) not caught |
| P1-65 | none | inventory-gap | duplicate `mb-2` CSS class — no rule |
| P1-66 | none | inventory-gap | `False` sentinel of a tuple return — no rule |
| P1-67 | 1 | detector-miss | fully un-annotated method (no `Any`) |
| P1-68 | 32 | detector-miss | unused `widget` param not caught |
| P1-69 | none | inventory-gap | redundant function-local re-import — no rule |
| P1-70 | 13 | detector-miss | pass-through `__init__` (adds nothing) not caught by #13 |
| P1-71 | 11 | detector-miss | two near-identical view-mixin classes not detected |
| P1-72 | 9 | detector-miss | #9 zero findings (class-level `_registry`) |
| P1-73 | 13 | detector-miss | `register_component` forwarder not caught |
| P1-74 | 1 | covered | #1 opaque `**kwargs` at components.py:46 |
| P1-75 | 2 | detector-miss | hasattr-on-declared-attr always-true not caught |
| P1-76 | 9 | detector-miss | #9 zero findings (`fields = []` class attr) |
| P1-77 | 32 | detector-miss | dead request/instance params not caught |
| P1-78 | 24 | covered | #24 getattr/hasattr in TableSection |
| P1-79 | 11 | covered | clone at layout.py:36 (FieldsetSubheader/Hr) |
| P1-80 | 32 | detector-miss | four dead render params not caught |
| P1-81 | 26 | detector-miss | dict-`.update()` assembly not the splat shape #26 caught |
| P1-82 | 9 | detector-miss | #9 zero findings (mutated module-level dict) |
| P1-83 | 26 | detector-miss | computed CONFIG_DEFAULTS dict not flagged |
| P1-84 | 1 | detector-miss | fully un-annotated module (no `Any`) |
| P1-85 | 29 | covered | #29 module-docstring at widgets.py:1 |
| P1-86 | 27 | covered | #27 at widgets.py:42 (BUTTON_CLASSES fan-in) |
| P1-87 | 11 | detector-miss | four one-line _get_base_actions_* comprehensions below clone threshold |
| P1-88 | 3 | detector-miss | #3 zero findings |
| P1-89 | 33 | detector-miss | #33 zero findings |
| P1-90 | 12 | detector-miss | hasattr-ternary→getattr idiom not catalogued |
| P1-91 | 24 | covered | #24 at action_model_admin.py:344 |
| P1-92 | 11 | detector-miss | cross-file permission-loop clone not detected |
| P1-93 | 9 | detector-miss | #9 zero findings (`_autocomplete_fields_missing` class list) |
| P1-94 | 15 | detector-miss | #15 missed this wallet (`formfield` used only for `.widget`) |
| P1-95 | 9 | detector-miss | #9 zero findings (all_valid monkeypatch) |
| P1-96 | 11 | detector-miss | in-method loop dup not detected |
| P1-97 | 12 | detector-miss | `if not: return False/return True` → bool() not catalogued |
| P1-98 | 35 | detector-miss | #35 zero findings (lazy import cycle) |
| P1-99 | none | inventory-gap | loop-var shadows param — no rule |
| P1-100 | 9 | detector-miss | #9 zero findings (IGNORED_PARAMS rebind) |
| P1-101 | none | inventory-gap | duplicate append in loop — no rule |
| P1-102 | 11 | covered | clone at mixins.py:57 (choices) |
| P1-103 | 14 | covered | #14 group at datetime_filters.py:22 spans the filter __init__ clump |
| P1-104 | 11 | covered | clone at datetime_filters.py:121 (param blocks) |
| P1-105 | none | inventory-gap | parameter_name/field_path key mismatch bug — no rule |
| P1-106 | 11 | covered | clone group numeric/datetime type-guards |
| P1-107 | 33 | detector-miss | #33 zero findings |
| P1-108 | 41 | detector-miss | family P silent by design |
| P1-109 | 12 | detector-miss | format-string 10**-n idiom not catalogued |
| P1-110 | 2 | detector-miss | `is not None and isinstance(str)` redundancy not caught |
| P1-111 | 12 | detector-miss | hasattr-ternary→getattr not catalogued |
| P1-112 | 11 | covered | clone at dropdown_filters.py:73 (queryset) |
| P1-113 | 32 | detector-miss | dead field_choices params not caught |
| P1-114 | none | inventory-gap | redundant re-read of dict key — no rule |
| P1-115 | 32 | detector-miss | unused enumerate index `_i` not caught |
| P1-116 | none | inventory-gap | facet_counts None-deref bug — no rule |
| P1-117 | none | inventory-gap | `__all__` names a nonexistent symbol — no rule |
| P1-118 | none | inventory-gap | duplicate `__all__` entry — no rule |
| P1-119 | 9 | detector-miss | #9 zero findings (widget instance as class attr) |
| P1-120 | 37 | detector-miss | #37 zero findings (pass-only subclass) |
| P1-121 | 11 | detector-miss | RangeDate/RangeDateTime form clone not detected |
| P1-122 | 1 | detector-miss | opaque `*args,**kwargs` on form constructors not flagged |
| P1-123 | 38 | detector-miss | #38 zero findings (repeated template-string class attr) |
| P1-124 | 34 | detector-miss | #34 zero findings (commented-out code) |
| P1-125 | 1 | covered | issue caught under #36 at inlines/admin.py:27 (untyped return → Unknown) |
| P1-126 | 32 | detector-miss | dead check params not caught |
| P1-127 | 33 | detector-miss | #33 zero findings |
| P1-128 | 32 | detector-miss | unused `save_as_new` param not caught |
| P1-129 | 34 | detector-miss | #34 zero findings (identical `w if.. else w`) |
| P1-130 | 2 | detector-miss | hasattr-on-assigned-attr always-true not caught |
| P1-131 | 32 | detector-miss | unused `files` param not caught |
| P1-132 | 12 | covered | #12 bool-ternary at import_export/forms.py:36 |
| P1-133 | 1 | detector-miss | fully un-annotated constructors (no `Any`) |
| P1-134 | 36 | threshold-miss | import_export/forms has 2 pragmas; #36 threshold is 3/module |
| P1-135 | none | inventory-gap | identical if/else branches — no rule |
| P1-136 | 41 | detector-miss | family P silent by design |
| P1-137 | 2 | detector-miss | isinstance(property)+hasattr(fget) redundancy not caught |
| P1-138 | 23 | covered | #23 complexity 73 at items_for_result:182 |
| P1-139 | 34 | detector-miss | #34 zero findings (dead `# pragma: no cover` branch) |
| P1-140 | 11 | covered | clone at unfold_list.py:376 (version-gated tag) |
| P1-141 | 11 | detector-miss | horizontal/vertical filters clone not detected |
| P1-142 | none | inventory-gap | possibly-unbound `field_path` — no rule |
| P1-143 | 1 | detector-miss | fully un-annotated functions (no `Any`) |
| P1-144 | 11 | detector-miss | 10 three-line AppConfig clones across files not detected |
| P1-145 | 9 | detector-miss | #9 zero findings (admin.site global rebind) |
| P1-146 | 33 | detector-miss | #33 zero findings (`-> Action` but returns `inner`) |
| P1-147 | 12 | detector-miss | `if/else`→`or` idiom not catalogued |
| P1-148 | 7 | detector-miss | #7 zero findings (comment-borne protocol) |
| P1-149 | 1 | covered | #1 opaque `**kwargs` at checks.py:13 |

**Phase 3 totals: 149 sites — 30 covered, 96 detector-miss, 2 threshold-miss, 21 inventory-gap.**
