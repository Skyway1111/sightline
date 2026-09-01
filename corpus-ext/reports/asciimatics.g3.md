# asciimatics — wave 1

| rule | fp class | count | example key |
|---|---|---|---|
| 1 | nested closure inside a method treated as a public boundary | 1 | asciimatics/screen.py:1097:1:weak:asciimatics.screen._AbstractCanvas.fill_polygon.sort_edges:edge |
| 1 | opaque varargs list forwarded into a user-supplied Callable | 1 | asciimatics/screen.py:1494:1:weak:asciimatics.screen.Screen.wrapper:arguments |
| 1 | abstract base property over an open set of subclass value types | 1 | asciimatics/widgets/widget.py:367:1:weak:asciimatics.widgets.widget.Widget.value:new_value |
| 2 | guard is live; the param annotation understates the accepted type | 2 | asciimatics/renderers/charts.py:47:2:redundant:isinstance |
| 2 | guard is live; the *callee's* return annotation is the lie (`__eq__ -> bool` returning NotImplemented) | 1 | asciimatics/strings.py:136:2:redundant:comparison |
| 5 | lift over-narrows a numeric helper to the types its call sites happen to pass | 4 | samples/bars.py:22:5:lift:bars.wv:x |
| 5 | lift into samples/, outside mypy.ini's `packages = asciimatics` scope, usually from a single intra-file call site | 15 | samples/maps.py:163:5:lift:maps.Map._get_satellite_tile:x_tile |
| 6 | a `logger.debug` call counted as an effect that makes a predicate dishonest | 1 | asciimatics/widgets/widget.py:209:6:dishonest-accessor:asciimatics.widgets.widget.Widget.is_mouse_over |
| 6 | read-only DAO getter querying its own store | 3 | samples/contact_list.py:40:6:dishonest-accessor:contact_list.ContactModel.get_summary |
| 8 | callback's button-index comparison (`selected == 0`) read as a validation predicate | 1 | samples/experimental.py:125:8:validation:_P_selected == 0 |
| 11 | copies live in separate standalone demo/test scripts that must not share a helper module | 36 | samples/contact_list.py:105:11:clone:ca1f85c766c6 |
| 11 | the sample's copy-paste template is the thing it teaches (tab_demo's four page classes) | 4 | samples/tab_demo.py:43:11:clone:3d6cbf4e968d |
| 11 | a scene block rebuilt per effect inside one demo's linear scene list | 2 | samples/particles.py:60:11:clone-block:0fb01b11e82f |
| 11 | subclass ctor boilerplate: `super().__init__` forward plus one empty field of a different type | 2 | asciimatics/effects.py:578:11:clone:800774854530 |
| 11 | documented forwarding constructor that exists to publish per-class defaults and pydoc | 2 | asciimatics/renderers/charts.py:156:11:clone:bd845ed139c6 |
| 11 | per-effect tuned constants, not duplicated logic | 2 | asciimatics/particles.py:747:11:clone:23663ebc3c4f |
| 11 | one-expression polymorphic override whose whole content is naming its class | 2 | asciimatics/particles.py:758:11:clone:7e3751c671ef |
| 11 | block clone subsumed by the function clone already reported at the same site | 2 | asciimatics/screen.py:1003:11:clone-block:b109c0599a37 |
| 11 | two overlapping clone groups (13-stmt x2 and 11-stmt x3) over the same region | 2 | asciimatics/utilities.py:95:11:clone-block:d1f2db7e0309 |
| 11 | expr-clone group inflated by a demo's copy and by sub-expressions | 1 | asciimatics/renderers/players.py:101:11:expr-clone:c127b4d42258 |
| 11 | a library private helper duplicated by a standalone demo that cannot import it | 1 | asciimatics/renderers/players.py:177:11:clone:787332b55c59 |
| 13 | documented deprecated public alias whose job is to be a thin forward | 1 | asciimatics/screen.py:1615:13:shallow:asciimatics.screen.Screen.getch |
| 13 | the "forwarded call" carries the SQL statement, which is the method's whole content | 2 | samples/contact_list.py:40:13:shallow:contact_list.ContactModel.get_summary |
| 14 | the library's universal drawing vocabulary (x, y, w, h, colour, attr, bg) read as a data clump | 6 | asciimatics/screen.py:628:14:clump:attr,bg,colour,text,transparent,x,y |
| 14 | trailing args a demo's drawing helpers forward to one transform function | 2 | samples/maps.py:143:14:clump:extent,xo,yo |
| 15 | callback bound by a fixed protocol signature (`move(particle) -> (int, int)`) | 1 | asciimatics/particles.py:713:15:wallet:asciimatics.particles.Splash._splash:particle |
| 16 | compute-then-write *is* the method's definition (constructor, clear, widget update, short scan) | 4 | asciimatics/screen.py:77:16:mutation-tail:asciimatics.screen._DoubleBuffer.clear |
| 17 | neck inside a constructor's flat run of independent `self.<field> = ...` assignments | 21 | asciimatics/screen.py:1367:17:liveness-neck:asciimatics.screen.Screen.__init__:1367 |
| 17 | neck at a nested-def preamble, or on a single local assignment mid-render | 2 | asciimatics/effects.py:903:17:liveness-neck:asciimatics.effects.Clock._update:903 |
| 17 | neck inside a flat field-reset run (`reset()`) | 1 | asciimatics/renderers/players.py:41:17:liveness-neck:asciimatics.renderers.players.AbstractScreenPlayer.reset:41 |
| 18 | a numbered "Notes:" caveat list read as labeled phases | 1 | asciimatics/widgets/filebrowser.py:71:18:sections:asciimatics.widgets.filebrowser.FileBrowser.clone |
| 20 | a one-expression lambda written exactly twice | 5 | asciimatics/widgets/dropdownlist.py:40:20:lambda:asciimatics.widgets.dropdownlist:95276714 |
| 20 | throwaway data-source lambda in a chart demo | 1 | samples/bars.py:48:20:lambda:bars:6d0c0b99 |
| 21 | a repeated sub-expression (index, `len`/`range`/`enumerate`, helper call) counted as an invariant | 11 | asciimatics/widgets/layout.py:19:21:invariant:asciimatics.widgets.layout.Layout:0b0110eb |
| 22 | bound-method UI callback (on_click / on_change) that must live on the instance | 9 | samples/forms.py:160:22:velcro:forms.DemoFrame._set_default |
| 22 | public API facade, deprecated alias, or signal handler composed from its own class's public methods | 6 | asciimatics/screen.py:1571:22:velcro:asciimatics.screen.Screen.get_key |
| 23 | a nested closure's complexity attributed to its enclosing def as well | 1 | tests/test_screen.py:260:23:cognitive-complexity:tests.test_screen.TestScreen.test_draw |
| 25 | public verb delegating to a private primitive (an abstraction boundary, not a rename) | 2 | asciimatics/paths.py:128:25:rename-delegation:asciimatics.paths.Path.jump_to |
| 25 | documented deprecated alias whose old name is the point | 2 | asciimatics/screen.py:1625:25:rename-delegation:asciimatics.screen.Screen.putch |
| 25 | signal handler named for the signal it serves | 1 | asciimatics/screen.py:2532:25:rename-delegation:asciimatics.screen._CursesScreen._continue_handler |
| 27 | module is already the smallest unit of its concept (one class, or an ABC plus its implementations) | 4 | asciimatics/widgets/layout.py:1:27:price:asciimatics.widgets.layout |
| 27 | the one-concept file of an already-split package (renderers/) | 1 | asciimatics/renderers/charts.py:1:27:price:asciimatics.renderers.charts |
| 27 | fan-out measured at a package's composition root | 1 | asciimatics/widgets/frame.py:1:27:fan-out:asciimatics.widgets.frame |
| 29 | line count read as runtime cost on per-keystroke / per-frame TUI handlers | 11 | asciimatics/widgets/textbox.py:165:29:cost-docstring:asciimatics.widgets.textbox.TextBox.process_event |
| 29 | filename-announced demo in a samples/ tree where no file carries a module docstring | 10 | samples/basics.py:1:29:top-loading:basics |
| 29 | sphinx-quickstart's generated `conf.py` | 1 | doc/source/conf.py:1:29:top-loading:conf |
| 32 | Sphinx `conf.py` module globals, which the tool reads by name | 17 | doc/source/conf.py:32:32:dead-symbol:conf.extensions |
| 32 | published API accessor whose callers live downstream | 1 | asciimatics/screen.py:1847:32:dead-symbol:asciimatics.screen.Screen.current_scene |
| 33 | a `@property` getter and its `@x.setter` folded into one symbol, then compared to each other | 25 | asciimatics/widgets/widget.py:127:33:mixed-returns:asciimatics.widgets.widget.Widget.disabled |
| 33 | `Optional[Event]` handler where falling through is the protocol's "consumed" value | 2 | samples/interactive.py:19:33:mixed-returns:interactive.KeyboardController.process_event |
| 37 | public library option judged by its in-repo (sample) call sites only | 2 | asciimatics/screen.py:1492:37:unused-default:asciimatics.screen.Screen.wrapper:height |
| 39 | published API docstring on a one-line accessor — doc/source runs automodule with `:members:`, so the ratio is structurally >1 | 35 | asciimatics/effects.py:117:39:comment-ratio:asciimatics.effects.Effect.screen |
| 39 | private helper whose docstring states what the body cannot (algorithm, why the signature swallows its args) | 12 | asciimatics/screen.py:2523:39:comment-ratio:asciimatics.screen._CursesScreen._resize_handler |
| 39 | unittest test docstring, which the runner prints as the test's description | 7 | tests/renderers/test_base.py:7:39:comment-ratio:tests.renderers.test_base.TestRenderers.test_height |
| 39 | `_clear` docstrings naming which surface is cleared across three identically-named methods | 3 | asciimatics/renderers/base.py:223:39:comment-ratio:asciimatics.renderers.base.DynamicRenderer._clear |
| 39 | demo-script prose carrying record shape or a chosen refresh rate | 3 | samples/quick_model.py:12:39:comment-ratio:quick_model.ContactModel.__init__ |
| 39 | one arm's label in a uniform ANSI-escape dispatch chain | 1 | asciimatics/parsers.py:356:39:comment-restates:asciimatics.parsers:356 |
| 39 | "previous"/"no longer" describing runtime lifecycle read as repo history | 1 | asciimatics/screen.py:2749:39:comment-history:asciimatics.screen:2749 |
| 42 | nested `test_`-prefixed callback (a PopUpDialog on_click handler) matched as a test | 4 | tests/test_widgets.py:1695:42:assertion-free:tests.test_widgets.TestWidgets.test_pop_up_widget.test_on_click |
| 45 | patched name re-used only to make the double delegate to the real object | 1 | tests/test_effects.py:352:45:testing-the-double:tests.test_effects.TestEffects.test_clock |
| 48 | named formula or named step where the name is the documentation | 5 | asciimatics/paths.py:14:48:fold:asciimatics.paths._spline |
| 50 | demo script's `demo(screen)` entry point, called only by Screen.wrapper below it | 29 | samples/256colour.py:11:50:unannotated:256colour.demo |
| 50 | local helper of a self-contained demo, outside mypy.ini's `packages = asciimatics` scope | 25 | samples/bars.py:15:50:unannotated:bars.fn |
| 50 | override of an asciimatics base class whose signature is already published there | 25 | samples/forms.py:141:50:unannotated:forms.DemoFrame.process_event |
| 55 | published API of a library whose (text, x, y, colour, attr, bg) order is the terminal-graphics convention | 12 | asciimatics/screen.py:628:55:positional-width:asciimatics.screen._AbstractCanvas.print_at |
| 55 | private drawing helper inside a standalone demo script | 5 | samples/maps.py:143:55:positional-width:maps.Map._scale_coords |
| 55 | private twin deliberately mirroring a public signature | 2 | asciimatics/screen.py:50:55:positional-width:asciimatics.screen._DoubleBuffer.clear |
| 55 | maths helper whose parameter order is the formula (`_spline(t, p0..p3)`, a nested closure) | 2 | asciimatics/paths.py:14:55:positional-width:asciimatics.paths._spline |
| 56 | published API whose only in-repo caller is its test because the intended callers are downstream | 11 | asciimatics/screen.py:1919:56:test-only:asciimatics.screen.ManagedScreen |

## wave 2

| rule | fp class | count | example key |
|---|---|---|---|
| 1 | abstract base property over an open set of subclass value types (getter and setter halves) | 2 | asciimatics/widgets/widget.py:360:1:weak:asciimatics.widgets.widget.Widget.value:return |
| 50 | `@x.setter` stub on a widget that has no value: the getter returns `None` and the stored field is never read, so only the base's `Any` is available | 3 | asciimatics/widgets/divider.py:66:50:unannotated:asciimatics.widgets.divider.Divider.value.setter |
