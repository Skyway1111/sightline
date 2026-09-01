# termimad (rs2) judge report

Repo: `../gauntlet-corpus/termimad` (crate `termimad` 0.35.2, ~10.4k lines of
Rust across `src/`, `tests/`, `examples/`). Read cold; no checker output seen.

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/skin.rs:1 | #29 | 750-line module, the largest in the crate, with no `//!` header: the first screen is a `use` block, so a reader must scan the whole file to learn that it holds MadSkin plus the whole write/print API | `use {` |
| P1-2 | src/views/input_field_content.rs:1 | #29 | 777-line module with no `//!` header; the only orientation is the doc comment on `InputFieldContent` 22 lines in, after two other types | `use {` |
| P1-3 | src/views/input_field.rs:1 | #29 | 702-line module with no `//!` header | `use {` |
| P1-4 | src/views/list_view.rs:1 | #29 | 445-line module with no `//!` header; declares 5 types before the first doc comment | `use {` |
| P1-5 | src/fit/composite_fit.rs:1 | #29 | 330-line module with no `//!` header, while its sibling `fit/mod.rs` has one | `use {` |
| P1-6 | src/events/event_source.rs:1 | #29 | 310-line module with no `//!` header, holding the crate's only spawned reader thread | `use {` |
| P1-7 | src/parse/mod.rs:73 | #11 | `PushStyleTokens::to_style_tokens_string` body is byte-identical to the free `style_tokens_to_string` at line 102; one should call the other | `let mut s = String::new();` / `for token in tokens {` |
| P1-8 | src/parse/mod.rs:89 | #11 | `write_style_tokens` is the third copy of that same loop and the one that drifted: it writes the leading space on the *first* token instead of on the separators, inverting the intent its two twins express | `if first {` / `write!(w, " {token}")?;` |
| P1-9 | src/skin.rs:419 | #11 | `print_expander`, `write_expander`, `print_owning_expander`, `write_owning_expander`, `print_owning_expander_md`, `write_owning_expander_md` (419-490) are six copies of the same 4-line body: `terminal_size()`, expand, `FmtText::from_text`, print or write | `let (width, _) = terminal_size();` / `let fmt_text = FmtText::from_text(self, text, Some(width as usize));` |
| P1-10 | src/skin.rs:145 | #11 | `default_dark` (145) and `default_light` (160) are the same 6-statement body differing only in gray levels; a new styled field must be added to both | `skin.code_block.set_fgbg(gray(20), gray(5));` / `skin.headers[0].set_fg(gray(22));` |
| P1-11 | src/skin.rs:389 | #11 | the "open stdout, delegate to the `_on` variant, flush" body is copied 7 times: skin.rs:389, 554, 562, views/text_view.rs:94, views/list_view.rs:340, views/input_field.rs:696, views/mad_view.rs:36 | `let mut w = std::io::stdout();` / `self.write_in_area_on(&mut w, markdown, area)?;` / `w.flush()?;` |
| P1-12 | src/views/input_field_content.rs:358 | #11 | `select_word_around` (358) and `select_non_space_around` (377) are identical 14-line bodies differing only in the predicate `is_word_char` vs `is_non_space_char`; one parameterized fn covers both | `while start > 0 && is_word_char(chars[start - 1]) {` |
| P1-13 | src/views/input_field_content.rs:519 | #11 | `move_lines_up` (519) and `move_lines_down` (535) repeat the same col-preserving vertical move (measure col, clamp y, restore x) with the sign flipped | `let cols = self.lines[self.pos.y].char_idx_to_col(self.pos.x);` |
| P1-14 | src/views/input_field.rs:357 | #11 | `apply_click_event` re-derives the (x, y) content position with the same 6 lines `get_pos` (377) already implements; it should call `get_pos` | `let y = ((y - self.area.top) as usize + self.scroll.y)` / `.min(self.content.line_count() - 1);` |
| P1-15 | src/views/list_view.rs:398 | #11 | `select_first_line` (398) and `select_last_line` (409) are the same 9-line scan differing only by `.rev()` | `if self.rows[i].displayed {` / `self.selection = Some(i);` |
| P1-16 | src/views/list_view.rs:357 | #11 | `ListView::try_scroll_lines` and `TextView::try_scroll_lines` (views/text_view.rs:144) are the same negative/positive scroll body in two modules, and they clamp differently (one saturating, one not), so the copies have already drifted | `let lines_count = -lines_count as usize;` / `self.scroll = self.scroll.saturating_sub(lines_count);` |
| P1-17 | src/tbl.rs:208 | #11 | the `TableRule` arm (208-220) and the `TableRow` arm (221-233) of `find_tables` are byte-identical except for `aligns.len()` vs `cells.len()` | `b.height += 1;` / `b.nbcols = b.nbcols.max(aligns.len());` |
| P1-18 | src/tbl.rs:203 | #11 | `find_tables` and `code.rs:42 find_blocks` are the same "accumulate consecutive matching lines into ranges, flush on break, flush at end" algorithm written twice over different line kinds | `if let Some(c) = current.take() {` / `tables.push(c);` |
| P1-19 | src/fit/composite_fit.rs:220 | #11 | the mid-token loop (220-232) and the mid-compound loop (234-248) in `Fitter::fit` are identical 11-line `while excess > 0` blocks differing only in the `Zone` constructor and the min width | `if let Some(zone) = Zone::biggest_token(&fc.compounds, 3) {` / `gain = zone.cut(&mut fc.compounds, excess + 1);` |
| P1-20 | src/fit/composite_fit.rs:108 | #11 | `Zone::biggest_token` (108) and `Zone::biggest_compound` (139) are identical `drain(..).max_by_key(...)` bodies over a different collector | `Zone::token(compounds, min_removable_width)` / `.max_by_key(\|z\| z.removable_width)` |
| P1-21 | src/fit/composite_fit.rs:70 | #11 | inside `Zone::token` the "measure the run, build char infos, push a Zone" block appears twice (70-82 and 91-103), once for whitespace-terminated runs and once for the tail | `let removable_width = zs.width();` / `zones.push(Zone {` |
| P1-22 | src/fit/crop_writer.rs:85 | #11 | `queue_unstyled_g_string` (85) and `queue_g_string` (105) are the same 15-line truncating loop differing only in the final writer call; `queue_unstyled_str`/`queue_str` (47/56) and `repeat`/`repeat_unstyled` (136/146) repeat the same styled/unstyled split | `if len > self.allowed {` / `s.truncate(idx);` / `self.allowed = 0;` |
| P1-23 | src/fit/str_fit.rs:69 | #11 | `make_string` (69) and `make_cow` (85) are the same body, one returning `String` and one `Cow`; `make_string` is exactly `make_cow(..).0.into_owned()` | `let string = (s[0..fit.bytes_count]).replace('\t', TAB_REPLACEMENT);` |
| P1-24 | src/fit/tbl_fit.rs:117 | #11 | `if self.available_sum_width >= sum_widths { return ... }` is a byte-identical copy of the early return at line 80 and is unreachable: line 80 already returned for that condition, so a reader must prove it dead | `if self.available_sum_width >= sum_widths {` / `col_widths: self.cols.iter().map(\|c\| c.width).collect(),` |
| P1-25 | src/compound_style.rs:89 | #11 | the background half of `blend_with` is a copy of the foreground half with the field name not updated: it reads and writes `foreground_color` again, so a skin's background is never blended and `MadSkin::blend_with` silently half-works | `if let Some(bg) = self.object_style.foreground_color.as_mut() {` |
| P1-26 | src/serde/serde_skin.rs:46 | #11 | five identical 4-line arms (`bold`, `italic`, `strikeout`, `inline_code`, `ellipsis`, 46-70) plus three more for `parse_styled_char` (73-87) and three for `parse_line_style` (96-110): 11 copies of "next_value, parse, assign" that a small table or helper collapses | `let value = map.next_value::<String>()?;` / `let cs = parse_compound_style(&value).map_err(de::Error::custom)?;` |
| P1-27 | src/serde/serde_compound_style.rs:11 | #11 | the `Deserialize`+`Serialize` pair is duplicated verbatim in four modules (serde_compound_style.rs:11, serde_line_style.rs:11, serde_styled_char.rs:12, serde_ordered_item_style.rs:12), differing only in the type and the `parse_*` fn | `let s = String::deserialize(deserializer)?;` / `parse_compound_style(&s).map_err(de::Error::custom)` |
| P1-28 | src/styled_char.rs:43 | #11 | `nude_char()` (43) and `get_char()` (54) are two public names for the identical one-line accessor `self.nude_char`; both are used, so callers pick at random | `pub fn nude_char(&self) -> char {` / `self.nude_char` |
| P1-29 | src/styled_char.rs:83 | #11 | `repeated` (83) and `queue_repeat` (90) build the repetition string with the same 4-line loop and differ only in the last line | `for _ in 0..count {` / `s.push(self.nude_char);` |
| P1-30 | src/rect.rs:104 | #11 | in `Rect::draw` the top-border block (104-111) and the bottom-border block (126-133) are the same 5-statement sequence with `top_*` swapped for `bottom_*` | `if area.width > 2 {` / `for _ in 0..area.width - 2 {` / `cs.queue(w, bs.top)?;` |
| P1-31 | src/macros.rs:224 | #11 | `mad_print_inline!` (224) and `mad_write_inline!` (259) have identical 8-line bodies apart from the final `print_composite` vs `write_composite`; the print form can expand to the write form on stdout | `static TEMPLATE: Lazy<InlineTemplate<'static>> = Lazy::new(\|\| {` |
| P1-32 | src/fit/wrap.rs:63 | #11 | the `OrderedListItem` arm (63-69) and the `OrderedListItemFollowUp` arm (70-76) of `composite_kind_widths` are byte-identical and should be one `\|`-joined arm | `let indent = ordered_item_indent(level, index);` |
| P1-33 | src/serde/serde_skin.rs:45 | #18 | `visit_map` narrates seven labeled phases in one 130-line body (45 inline styles, 72 marker chars, 89 scrollbar, 95 line styles, 112 headers, 141 ordered list item style, 152 table border chars): each label is a helper waiting to be named | `// inline styles` / `// marker chars` / `// scrollbar` |
| P1-34 | src/serde/serde_skin.rs:185 | #18 | `Serialize::serialize` repeats the same seven labeled phases (185, 192, 197, 201, 206, 209, 212), so the two halves of the format are narrated twice and can drift apart | `// inline styles` / `// line styles` / `// headers` |
| P1-35 | src/views/list_view.rs:254 | #18 | `write_on` is three prose-labeled phases in one function: `// title line` (254), `// separator line` (277), `// rows, maybe scrolled` (292) | `// title line` / `// separator line` |
| P1-36 | src/fit/composite_fit.rs:221 | #18 | `Fitter::fit` narrates four phases: mid-token cut (221), mid-compound cut (235), left truncation (273), right truncation (301), each a self-contained block over `excess` | `// cutting in the middle of big no space parts` / `// left truncating` |
| P1-37 | src/fit/wrap.rs:105 | #18 | `hard_wrap_composite` labels its two branches `// Strategy 1:` (105) and `// Strategy 2:` (130) in prose instead of naming two functions | `// Strategy 1:` / `// Strategy 2:` |
| P1-38 | src/fit/tbl_fit.rs:127 | #18 | `TblFit::fit` numbers its phases `// Step 1` (127) and `// Step 2` (153), each a distinct reduction pass over `excess` | `// Step 1` / `// Step 2` |
| P1-39 | src/tbl.rs:92 | #18 | `fix_columns` narrates four phases (92 add missing cells, 121 width invariant, 123 resize and insert rows, 168 apply alignment), and the first label is written twice in two different wordings, one line apart | `// we add the missing cells and also prepare the fitter` / `// We also add the missing cells` |
| P1-40 | src/views/input_field.rs:577 | #23 | `display_on` is a 115-line render loop with 5 nesting levels (per-row loop, per-char loop, ellipsis branch, selection branch, trailing fill), a shadowed `width`, and 4 separate assignments of `terminal_cursor_pos` | `for (i, c) in chars.iter().enumerate() {` / `if displayed_width + char_width >= width {` |
| P1-41 | src/views/input_field.rs:499 | #23 | `fix_scroll` is 65 lines of unexplained nested arithmetic branches on `scroll.x`/`scroll.y` with magic thresholds (4, 2, 1) and no named intermediate | `} else if pos.y >= self.scroll.y + height {` / `self.scroll.y = pos.y - height + 1;` |
| P1-42 | src/skin.rs:671 | #23 | `write_fmt_line` is a 79-line match whose `TableRule` arm holds three inline `match rule.position` expressions inside `write!` arguments, a fold, and a nested loop | `self.table.compound_style.apply_to(match rule.position {` |
| P1-43 | src/skin.rs:573 | #23 | `write_fmt_composite` is 90 lines mixing margin arithmetic, a 6-arm `match` with guards on `block`, and two `cfg`-duplicated compound loops | `CompositeKind::OrderedListItemFollowUp { level, index } if block => {` |
| P1-44 | src/fit/composite_fit.rs:200 | #23 | `Fitter::fit` is 130 lines: three special-case early returns, two `while` loops with inner `if let`, an alignment `match` producing two counters, then two more truncation loops with inner loops | `while excess_left > 0 && !compounds.is_empty() {` |
| P1-45 | src/events/event_source.rs:142 | #23 | `with_options` is a 135-line function whose body is one `thread::spawn` closure containing the whole escape-sequence state machine, the mouse double-click detector, and the send/quit protocol | `thread::spawn(move \|\| {` / `let in_seq = current_escape_sequence.is_some();` |
| P1-46 | src/tbl.rs:85 | #23 | `fix_columns` is 115 lines with a `match` on a `Result` whose Ok arm holds a double loop, then a reverse row loop with a nested column loop and a row-insertion loop, then a third loop | `for ir in (self.start..self.start + self.height).rev() {` |
| P1-47 | src/serde/serde_skin.rs:38 | #23 | `visit_map` is a 130-line `while let` over a 15-arm `match`, one arm of which nests a two-variant match with a triple loop over headers | `while let Some(key) = map.next_key::<String>()? {` |
| P1-48 | src/views/list_view.rs:245 | #23 | `write_on` is 93 lines with an unlabelled `loop` inside a `for` used purely to skip filtered rows, plus a nested column loop and a scrollbar branch | `loop {` / `if self.rows[row_idx].displayed {` |
| P1-49 | src/fit/crop_writer.rs:15 | #32 | `CropWriter` and its 16 methods (155 lines) have no reference anywhere in the repo: not in `src/`, not in `tests/`, not in any of the 20 examples; it is context every reader of `fit/` ingests for nothing | `pub struct CropWriter<'w, W>` |
| P1-50 | src/rect.rs:77 | #32 | `Rect`, `RectBorderStyle` and the four `BORDER_STYLE_*` statics (136 lines) are referenced only by each other: nothing in the crate, tests, or examples constructs a `Rect` | `pub struct Rect<'s> {` |
| P1-51 | src/fit/mod.rs:36 | #32 | `fill_bg` has zero references in the repo; the crate fills backgrounds through `Filling::queue_styled` everywhere else | `pub fn fill_bg<W>(w: &mut W, len: usize, bg: Color) -> Result<(), Error>` |
| P1-52 | src/fit/str_fit.rs:69 | #32 | `StrFit::make_string` has zero references; every caller uses `make_cow` (the doc comments of the two are copies of each other) | `pub fn make_string(s: &str, cols_max: usize) -> (String, usize) {` |
| P1-53 | src/macros.rs:22 | #32 | `let mut i: usize = 0;` is dead in both `mad_print_inline!` (22) and `mad_write_inline!` (56): nothing in either expansion reads `i`, and the first copy carries two `#[allow]` attributes whose only job is to hide that | `#[allow(unused_variables)]` / `let mut i: usize = 0;` |
| P1-54 | src/fit/str_fit.rs:59 | #56 | `StrFit::count_fitting` is reached only from `tests/fit.rs` (10 of that file's assertions); no prod path calls it, so the test proves a wrapper nothing ships | `pub fn count_fitting(s: &str, cols_max: usize) -> (usize, usize) {` |
| P1-55 | src/area.rs:117 | #37 | `compute_scrollbar<U1, U2, U3>` carries three type parameters with `TryFrom<usize>` bounds and `unwrap`s the conversions, yet both call sites (area.rs:107, views/list_view.rs:165) instantiate all three as `u16`; the generality buys nothing and costs two `Debug` bounds and two unwraps | `pub fn compute_scrollbar<U1, U2, U3>(` / `<U2 as TryFrom<usize>>::Error: std::fmt::Debug,` |
| P1-56 | src/area.rs:99 | #37 | `Area::scrollbar<U: Into<usize>>` is called only with `u16` (views/text_view.rs:87, views/input_field.rs:589), both sites casting to `u16` at the call | `pub fn scrollbar<U>(` |
| P1-57 | src/views/text_view.rs:155 | #37 | `try_scroll_pages<C: Into<f64>>` is called only with `-1`/`1` integer literals (examples/scrollable/main.rs:45,46,131,132; examples/stderr/main.rs:67,68); the sibling `ListView`/`MadView` versions take a plain `i32` | `pub fn try_scroll_pages<C: Into<f64>>(&mut self, pages_count: C) {` |
| P1-58 | src/fit/wrap.rs:1 | #36 | a bare `#[allow(unused_imports)]` sits over the module's whole `use` block, including a `crate::*` glob: it permanently blinds the compiler to dead imports in a 177-line module rather than naming what is needed | `#[allow(unused_imports)]` / `use {` |
| P1-59 | src/fit/mod.rs:32 | #38 | `DEFAULT_TAB_REPLACEMENT: &str = "  "` duplicates `TAB_REPLACEMENT: &str = "  "` (fit/str_fit.rs:6): the same fact under two names in two modules of one crate, each with its own readers (crop_writer.rs:36 vs input_field_content.rs:678), so changing the tab width fixes half the crate | `pub static DEFAULT_TAB_REPLACEMENT: &str = "  ";` |
| P1-60 | src/events/event_source.rs:111 | #48 | `is_seq_start` is a private one-line predicate with exactly one call site (line 214) and no other reference | `fn is_seq_start(key: KeyEvent) -> bool {` / `key.code == KeyCode::Char('_') && key.modifiers == KeyModifiers::ALT` |
| P1-61 | src/events/event_source.rs:114 | #48 | `is_seq_end` is a private one-line predicate with exactly one call site (line 181) | `fn is_seq_end(key: KeyEvent) -> bool {` |
| P1-62 | src/views/input_field_content.rs:695 | #48 | `is_non_space_char` is a private one-line predicate wrapping `!c.is_whitespace()` with one call site (line 380) | `fn is_non_space_char(c: char) -> bool {` / `!c.is_whitespace()` |
| P1-63 | tests/tables.rs:7 | #42 | `test_fix_issue_77` has no assertion: it discards the render with `let _ =` and relies entirely on an unstated "must not panic" contract that no reader can see from the body | `let _ = skin.text(md, Some(20)).to_string(); // Panics with Termimad 0.35.0` |
| P1-64 | tests/wrap.rs:117 | #42 | `check_issue_23` loops 20 widths building texts into `_text` and never asserts anything; it passes whatever the wrapper produces as long as it does not panic | `let _text = FmtText::from(&skin, md, Some(w));` |
| P1-65 | src/views/input_field_content.rs:763 | #42 | `test_select_clear_del_selection` drives select/clear/del and then ends: no assertion, no `check(...)`, so it cannot fail on a wrong result | `con.clear();` / `con.del_selection();` |
| P1-66 | src/views/input_field_content.rs:771 | #42 | `test_select_del_char_left_del_selection` is the same assertion-free shape as P1-65 with one call swapped | `con.del_char_left();` / `con.del_selection();` |
| P1-67 | tests/fit.rs:55 | #44 | six assertions in `test_count_fitting` (55, 56, 61, 62, 65, 66) check `str::len` and `chars().count()` of literals the test itself wrote: they assert properties of the Rust stdlib and constants, not of `StrFit`, and cannot fail | `assert_eq!(c12.len(), 14);` / `assert_eq!(c12.chars().count(), 12);` |
| P1-68 | src/serde/serde_skin.rs:166 | none | a library deserializer writes to stdout on every unknown key; in a TUI crate whose whole purpose is controlling the terminal, this corrupts the caller's screen and cannot be silenced | `println!("unknown key: {key}");` |
| P1-69 | src/views/input_field.rs:330 | none | `fix_scroll` runs twice per keystroke on the replace-selection path: `put_char` (216) already calls it, and line 331 calls it again | `self.put_char(c);` / `self.fix_scroll();` |
| P1-70 | src/views/list_view.rs:363 | none | `try_scroll_lines` subtracts before clamping: with fewer displayed rows than `tbody_height` (an empty or filtered list) `displayed_rows_count - tbody_height()` underflows and panics, while the negative branch two lines up is correctly `saturating_sub` | `.min(self.displayed_rows_count - self.tbody_height() as usize + 1);` |
| P1-71 | src/events/event_source.rs:282 | none | `unblock` unwraps a channel send, so a caller that has dropped or ended the source thread gets a panic instead of an error, on a method the doc calls mandatory | `self.tx_quit.send(quit).unwrap();` |
| P1-72 | src/events/event_source.rs:308 | none | `Drop::drop` unwraps `disable_raw_mode`: a failure during unwinding aborts the process | `terminal::disable_raw_mode().unwrap();` |
| P1-73 | src/fit/wrap.rs:56 | none | `composite_kind_widths` returns `(0, 0)` for `ListItemFollowUp` in `FirstLineOnly` mode but `(indent, 0)` for `OrderedListItemFollowUp` (line 70) in the same mode: two follow-up kinds disagree with no comment saying why | `ListItemsIndentationMode::FirstLineOnly => (0, 0),` |
| P1-74 | src/fit/composite_fit.rs:40 | none | a leftover author note in French (`virer`, "get rid of") on a struct field, with no issue reference and no way for a reader to act on it | `width: usize, // virer` |
| P1-75 | src/skin.rs:450 | none | three `write_*` methods (450, 463, 477) carry the doc comment "do a `print!` of the given owning expander", copied from their `print_*` twins: the contract text names the wrong sink on a writer-taking method | ``/// do a `print!` of the given owning expander`` |
| P1-76 | src/views/input_field.rs:177 | none | `copy_selection` takes `&mut self` but mutates nothing (it forwards to `selection_string(&self)`), needlessly forcing callers to hold an exclusive borrow to read | `pub fn copy_selection(&mut self) -> String {` / `self.content.selection_string()` |
| P1-77 | src/views/input_field_content.rs:135 | none | `line_saturating` evaluates its fallback eagerly with `unwrap_or`, indexing `self.lines[len - 1]` on every call including the hit path | `.unwrap_or(&self.lines[self.lines.len() - 1])` |

## Phase 2 - audit finding verdicts

4 sheet rows (rs:37 x1, rs:48 x3): 2 `real`, 2 `fp`. Verdicts and one-line
reasons are written into
`corpus-ext/sheets/termimad.rs2.wave1.tsv` by
`sightline-rs2/judge-tmp/fill-termimad.py`; repeated here.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| src/errors.rs:13 | rs:37 | indexed | fp | `pub(crate) type Result<T> = std::result::Result<T, Error>` is the standard Rust crate-error alias (the shape of std's own `io::Result`); the parameter is passed straight through to `std::result::Result`, so the alias adds no abstraction of its own to collapse, and naming the type would leave every `Result`-returning pub method spelling the error type by hand for no reader gain |
| src/events/event_source.rs:111 | rs:48 | indexed | real | one-line private predicate with exactly one call site (line 214) and no other reference: the shape #48 names, though the sole caller is the 135-line `with_options` closure, so the fold is worth doing only as part of extracting that state machine |
| src/events/event_source.rs:114 | rs:48 | indexed | real | one-line private predicate with exactly one call site (line 181) and no other reference; same reading as its `is_seq_start` twin |
| src/views/input_field_content.rs:186 | rs:48 | indexed | fp | `fix_pos` is the pos half of a symmetric pair whose sole caller `fix_selection` is nothing but `self.fix_pos(); self.fix_selection_tail();`; folding only the one-line half yields an asymmetric two-line body that reads worse |

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| rs:37 | idiomatic pass-through error alias: a `type Result<T> = std::result::Result<T, E>` whose parameter is forwarded unchanged to a std generic is monomorphic by convention, not by unexercised flexibility | 1 | src/errors.rs:13:37:monomorphic:termimad::errors::Result:T |
| rs:48 | symmetric-pair half: the sole caller's body consists entirely of sibling calls at the same level, so folding one member destroys the parallelism the pair encodes | 1 | src/views/input_field_content.rs:186:48:fold:termimad::views::input_field_content::InputFieldContent::fix_pos |

## Phase 3 - reconciliation

77 phase-1 sites: 28 covered (1 of them withdrawn, see P1-62), 49 misses
(14 threshold-miss, 25 detector-miss, 10 inventory-gap). Reconciled against
every rule of the audit JSON (72 findings: #11 x32, #18 x1, #23 x16, #29 x15,
#37 x1, #42 x4, #48 x3; zero findings for #6, #9, #20, #27, #32, #34, #36,
#38, #44, #47, #53, #56, #59).

A finding counts as covering a site only when it names the same defect at that
construct. A rule landing on the enclosing function for a different reason
(#23 on `Zone::token` while my site is a duplicated block inside it) is not
coverage; that inflates recall and hides the real gap.

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #29 | covered | #29 src/skin.rs:1 (750 lines, 2 top-level items) |
| P1-2 | #29 | covered | #29 src/views/input_field_content.rs:1 |
| P1-3 | #29 | covered | #29 src/views/input_field.rs:1 |
| P1-4 | #29 | covered | #29 src/views/list_view.rs:1 |
| P1-5 | #29 | covered | #29 src/fit/composite_fit.rs:1 |
| P1-6 | #29 | covered | #29 src/events/event_source.rs:1 |
| P1-7 | #11 | threshold-miss | the shared region of `to_style_tokens_string` and `style_tokens_to_string` is the 3-statement build-and-join tail (`let mut s`, `for`, `s`), under the >=5-statement block cutoff, and the whole-fn digests differ by the two leading `push_style_tokens` lines |
| P1-8 | #11 | threshold-miss | same 3-statement block as P1-7; the drift that makes this one a bug (space on the first token, not the separator) is invisible to a rule that only reports exact groups |
| P1-9 | #11 | covered | #11 src/skin.rs:469 + 484 (`print_owning_expander_md`/`write_owning_expander_md`, 5-stmt block clone); 2 of the 6 members of the family reached, the four `print_expander`/`write_expander`/`print_owning_expander`/`write_owning_expander` copies at 419-461 were not |
| P1-10 | #11 | covered | #11 src/skin.rs:145 + 160 (`default_dark`/`default_light`) |
| P1-11 | #11 | covered | #11 src/skin.rs:554 + 562 and #11 src/views/list_view.rs:340 + src/views/text_view.rs:94; the 7-copy family was split into two x2 groups and the skin.rs:389, input_field.rs:696, mad_view.rs:36 members were not reached |
| P1-12 | #11 | covered | #11 src/views/input_field_content.rs:358 + 377 |
| P1-13 | #11 | detector-miss | `move_lines_up`/`move_lines_down` are the same 6-line col-preserving move; the blind digest separates them on `-=` vs `+=` and on the guard shape, which no cutoff explains |
| P1-14 | #11 | threshold-miss | the position-derivation `apply_click_event` copies out of `get_pos` is 3 statements, under the >=5-statement block cutoff |
| P1-15 | #11 | detector-miss | `select_first_line`/`select_last_line` differ only by `.rev()` on the range; the digest treats the iterator expression as structure |
| P1-16 | #11 | threshold-miss | `ListView::try_scroll_lines` and `TextView::try_scroll_lines` share a verbatim 2-statement negative branch, under the block cutoff; the positive branches have already drifted (saturating vs not) |
| P1-17 | #11 | detector-miss | two byte-identical `match` arms inside `find_tables`; #11 rs reads `fn` bodies and top-level statement blocks and has no arm for repeated match arms |
| P1-18 | #11 | detector-miss | `tbl.rs:find_tables` and `code.rs:find_blocks` are the same run-accumulation algorithm; the digest splits them on `match` vs `if let` |
| P1-19 | #11 | threshold-miss | each `while excess > 0` block is 1 top-level statement (4 nested), under the >=5-statement cutoff, though the two are 11 identical lines |
| P1-20 | #11 | covered | #11 src/fit/composite_fit.rs:108 + 139 (`biggest_token`/`biggest_compound`) |
| P1-21 | #11 | threshold-miss | the twice-written push block in `Zone::token` is 3 top-level statements; #23 landed on the same function (cc 33) but names complexity, not the duplication |
| P1-22 | #11 | detector-miss | `queue_unstyled_g_string`/`queue_g_string` are 15-line twins differing only in the writer call; no cutoff explains the split, the digest does |
| P1-23 | #11 | detector-miss | `make_string`/`make_cow` differ only by `Cow` wrapping of the same two branches |
| P1-24 | #11 | threshold-miss | the unreachable copy of the early return is 2 statements, under the block cutoff; nothing in the inventory reads unreachability, so the dead block is invisible from both sides |
| P1-25 | #11 | threshold-miss | the fg/bg halves of `CompoundStyle::blend_with` are 2 statements each inside an `if let`, under the block cutoff, so the copy-paste bug they carry is unreachable by this rule |
| P1-26 | #11 | threshold-miss | the 11 `next_value/parse/assign` arms in `visit_map` are 3 statements each and are match arms, so they fail the cutoff and the construct both |
| P1-27 | #11 | covered | #11 src/serde/serde_compound_style.rs:15 + serde_line_style.rs:15 and serde_ordered_item_style.rs:15 + serde_styled_char.rs:15; the four `deserialize` halves are reached as two x2 groups, the four identical `serialize` halves are not |
| P1-28 | #11 | threshold-miss | `nude_char`/`get_char` are one-line accessors with identical bodies, presumably under a trivial-body exemption; the rule did report the four 2-statement setters in the same impl |
| P1-29 | #11 | detector-miss | `repeated`/`queue_repeat` share a 4-line build loop and differ only in the final line |
| P1-30 | #11 | threshold-miss | the top-border and bottom-border blocks in `Rect::draw` are 4 statements each, under the >=5-statement cutoff; rect.rs drew no finding of any rule |
| P1-31 | #11 | detector-miss | `mad_print_inline!`/`mad_write_inline!` are 8-line identical `macro_rules!` bodies; #11 rs reads `fn` bodies, so macro bodies are outside its reading entirely |
| P1-32 | #11 | detector-miss | identical `OrderedListItem`/`OrderedListItemFollowUp` match arms; same construct gap as P1-17 |
| P1-33 | #18 | detector-miss | `visit_map` narrates 7 phases (`// inline styles`, `// marker chars`, `// scrollbar`, `// line styles`, `// headers`, `// ordered list item style`, `// table border chars`); the only #18 finding in the repo was the `// Step 1`/`// Step 2` pair, so the rule appears to key on numbered markers rather than on a run of parallel labels |
| P1-34 | #18 | detector-miss | the same 7 labels repeated in `Serialize::serialize`, unrecognised for the same reason |
| P1-35 | #18 | detector-miss | `// title line` / `// separator line` / `// rows, maybe scrolled` in `ListView::write_on`, 3 phases, unrecognised |
| P1-36 | #18 | detector-miss | 4 phases in `Fitter::fit` (`// cutting in the middle of ...` x2, `// left truncating`, `// right truncating`), unrecognised; #23 fired on the same function but that is the complexity reading, not the phase reading |
| P1-37 | #18 | detector-miss | `// Strategy 1:` / `// Strategy 2:` in `hard_wrap_composite` is the closest shape to the one that did fire (numbered, colon-terminated) and still missed: worth checking whether the marker set is literally `Step` |
| P1-38 | #18 | covered | #18 src/fit/tbl_fit.rs:127, the repo's only #18 finding |
| P1-39 | #18 | detector-miss | 4 phases in `fix_columns`, one of them written twice in two wordings one line apart, unrecognised |
| P1-40 | #23 | covered | #23 src/views/input_field.rs:577, cc 69 |
| P1-41 | #23 | covered | #23 src/views/input_field.rs:499, cc 47 |
| P1-42 | #23 | covered | #23 src/skin.rs:671, cc 18 |
| P1-43 | #23 | covered | #23 src/skin.rs:573, cc 17 |
| P1-44 | #23 | covered | #23 src/fit/composite_fit.rs:200, cc 57 |
| P1-45 | #23 | covered | #23 src/events/event_source.rs:142, cc 102, the repo's highest |
| P1-46 | #23 | covered | #23 src/tbl.rs:85, cc 65 |
| P1-47 | #23 | threshold-miss | `visit_map` is 130 lines but its shape is one `while let` over a flat 15-arm `match`, which cognitive complexity scores near zero (arms do not nest): the metric and the reader disagree here, and the same file's #29 finding is the only signal |
| P1-48 | #23 | covered | #23 src/views/list_view.rs:245, cc 51 |
| P1-49 | #32 | detector-miss | `CropWriter` (155 lines, 16 methods) has no reference in src, tests or the 20 examples; the audit carries zero #32 findings, consistent with a lib crate making every re-exported `pub` item root-reachable by definition, which makes the rule structurally silent on library dead weight |
| P1-50 | #32 | detector-miss | `Rect` + `RectBorderStyle` + four `BORDER_STYLE_*` statics, 136 lines, no in-repo user; same structural silence |
| P1-51 | #32 | detector-miss | `fill_bg`, zero references; same |
| P1-52 | #32 | detector-miss | `StrFit::make_string`, zero references; same |
| P1-53 | #32 | detector-miss | the dead `let mut i: usize = 0;` in both macros is a local inside a `macro_rules!` body, outside #32's pub-item reading, and the first copy carries the two `#[allow]`s that hide it |
| P1-54 | #56 | detector-miss | `StrFit::count_fitting` is reached only from tests/fit.rs; the audit carries zero #56 findings, same root cause as P1-49 (a re-exported `pub` item in a lib is root-reachable, so the test-only condition never evaluates) |
| P1-55 | #37 | detector-miss | `compute_scrollbar<U1, U2, U3>` is `u16` at both call sites, with two `TryFrom` bounds and two unwraps bought by the generality; the rule's one #37 finding in this repo was the idiomatic `Result` alias it should not have flagged, and it missed this |
| P1-56 | #37 | detector-miss | `Area::scrollbar<U: Into<usize>>` is `u16` at both call sites |
| P1-57 | #37 | detector-miss | `TextView::try_scroll_pages<C: Into<f64>>` takes integer literals at all 6 call sites while its two siblings take a plain `i32` |
| P1-58 | #36 | threshold-miss | one blanket `#[allow(unused_imports)]` over a whole `use` block in a 177-line module is 1 allow, under any per-module density cutoff; density misses the blast radius of a single allow that covers a glob import |
| P1-59 | #38 | threshold-miss | `DEFAULT_TAB_REPLACEMENT` and `TAB_REPLACEMENT` are the same literal at module level in 2 modules, under the >=3-module cutoff, though both have live readers and drifting one breaks half the crate |
| P1-60 | #48 | covered | #48 src/events/event_source.rs:111, judged `real` in phase 2 |
| P1-61 | #48 | covered | #48 src/events/event_source.rs:114, judged `real` in phase 2 |
| P1-62 | #48 | covered | withdrawn, not a miss: my phase-1 claim of a single call site is false. `is_non_space_char` is called three times (380, 383, 387), so #48 correctly did not fire. Recorded here rather than in the committed phase-1 row |
| P1-63 | #42 | covered | #42 tests/tables.rs:7 |
| P1-64 | #42 | covered | #42 tests/wrap.rs:117 |
| P1-65 | #42 | covered | #42 src/views/input_field_content.rs:763 |
| P1-66 | #42 | covered | #42 src/views/input_field_content.rs:771 |
| P1-67 | #44 | detector-miss | `assert_eq!(c12.len(), 14)` compares a call result to a literal the test itself wrote, so it falls outside #44's call-free reading; the ideal (an assertion that can only fail if the stdlib is wrong) needs an arm for an assertion whose non-constant side never touches the crate under test |
| P1-68 | none | inventory-gap | a library deserializer `println!`s to stdout on every unknown key; no rule reads output side effects in a lib |
| P1-69 | none | inventory-gap | `fix_scroll` called twice per keystroke (once inside `put_char`, once after it); no rule reads redundant effect calls |
| P1-70 | none | inventory-gap | `try_scroll_lines` underflows and panics on an empty or fully-filtered list while its sibling branch is `saturating_sub`; no rule reads arithmetic-safety asymmetry |
| P1-71 | none | inventory-gap | `unblock` unwraps a channel send on a method its own doc calls mandatory; no rule reads panic surface |
| P1-72 | none | inventory-gap | `Drop::drop` unwraps, aborting on failure during unwinding; no rule reads panic-in-drop |
| P1-73 | none | inventory-gap | two follow-up composite kinds disagree on `FirstLineOnly` indentation with no comment saying why; no rule reads sibling-arm inconsistency |
| P1-74 | none | inventory-gap | leftover author note in French on a struct field, no issue reference; #34's Rust reading is commented-out code, which this is not |
| P1-75 | none | inventory-gap | three `write_*` methods carry their `print_*` twins' doc text, naming the wrong sink; no rule reads doc/signature disagreement (#53 owns only the `# Errors` section, and this crate has none) |
| P1-76 | none | inventory-gap | `copy_selection` takes `&mut self` and mutates nothing, forcing an exclusive borrow to read; no rule reads over-broad receiver mutability |
| P1-77 | none | inventory-gap | `line_saturating` evaluates its `unwrap_or` fallback eagerly on every call; no rule reads eager-fallback cost |
