# cr-sqlite (Rust) judge report

Repo: `../gauntlet-corpus/cr-sqlite`, Rust tree only (`core/rs/`).
Prod crates: `core`, `fractindex-core`, `bundle`, `bundle_static`.
Test code: `core/rs/integration_check/**` (a test-runner crate: every `run_suite`
is a test entry point), plus `#[cfg(test)] mod tests` blocks and the `#[test]`
bindgen layout functions in `core/src/c.rs`.

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | core/rs/core/src/tableinfo.rs:228 | #11 | 16 `get_*_stmt` lazy accessors (228-560, plus 657-713 on `ColumnInfo`) share one body shape: `if self.X.try_borrow()?.is_none() { let sql = format!(..); let ret = db.prepare_v3(&sql, PREPARE_PERSISTENT)?; *self.X.try_borrow_mut()? = Some(ret); } Ok(self.X.try_borrow()?)`. Only the field name and the SQL differ. The author flags it himself on line 227. | `    // TODO: macro-ify all these` |
| P1-2 | core/rs/core/src/tableinfo.rs:106 | #11 | `get_or_create_key_via_raw_values` is `get_or_create_key` (78-104) with the `bind_package_to_stmt` line swapped for a bind loop; the three-arm `match stmt.step()` and every `reset_cached_stmt` path are identical copies. | `    pub fn get_or_create_key_via_raw_values(` |
| P1-3 | core/rs/core/src/tableinfo.rs:203 | #11 | `create_key_via_raw_values` (203-225) duplicates `create_key` (181-201) whole; same divergence as P1-2, so the pk-binding decision now lives in four places. | `    fn create_key_via_raw_values(` |
| P1-4 | core/rs/core/src/tableinfo.rs:589 | #11 | `clear_stmts` hand-writes `let mut stmt = self.<field>.try_borrow_mut()?; stmt.take();` 15 times. Adding a 16th cached stmt field means editing the struct, an accessor, and this list; nothing catches a miss. | `        let mut stmt = self.set_winner_clock_stmt.try_borrow_mut()?;` |
| P1-5 | core/rs/core/src/lib.rs:127 | #11 | The `let rc = db.create_function_v2(name, nargs, flags, udata, Some(fn), None, None, None).unwrap_or(ERROR); if rc != OK { unsafe { crsql_freeExtData(ext_data) }; return null_mut(); }` block is repeated 19 times in `sqlite3_crsqlcore_init` (111-508). A table of registrations plus one loop is the one home. | `    let rc = db` |
| P1-6 | core/rs/core/src/changes_vtab.rs:374 | #11 | The table-info lookup block (374-388) is a verbatim copy of changes_vtab_write.rs:488-505, down to both TODO comments (`will this work given insert_tbl is null termed?`, `technically safe since we checked is_none`). Two homes for one unsafe idiom. | `    let tbl_infos = mem::ManuallyDrop::new(Box::from_raw(` |
| P1-7 | core/rs/core/src/changes_vtab_write.rs:653 | #11 | The set-sync-bit / step / clear-sync-bit / reconcile-two-Results sequence appears three times: 260-288 (`merge_sentinel_only_insert`), 339-361 (`merge_delete`), 653-679 (`merge_insert`). Line 649 admits it. | `    // TODO: this is all almost identical between all three merge cases!` |
| P1-8 | core/rs/core/src/local_writes/mod.rs:79 | #11 | Six copies of one shape (get stmt ref, `or_else(Err(msg))`, `as_ref().ok_or("Failed to deref sentinel stmt")`, chained `bind_*`, `step_trigger_stmt`): mod.rs:79 and :111, after_update.rs:126 and :151, after_insert.rs:64, after_delete.rs:48 and :64. | `fn mark_new_pk_row_created(` |
| P1-9 | core/rs/core/src/local_writes/after_delete.rs:17 | #11 | `x_crsql_after_delete` (17-34) and `x_crsql_after_insert` (after_insert.rs:17-34) are byte-identical apart from the inner call name. | `pub unsafe extern "C" fn x_crsql_after_delete(` |
| P1-10 | core/rs/core/src/triggers.rs:78 | #11 | `create_delete_trigger` (78-98) is `create_insert_trigger` (22-38) with AFTER INSERT/NEW swapped for AFTER DELETE/OLD; the trigger-name suffix convention is now spelled in three places in this file. | `fn create_delete_trigger(` |
| P1-11 | core/rs/fractindex-core/src/util.rs:70 | #11 | `extract_columns` (70-82) duplicates `extract_pk_columns` (54-66) exactly except for the SQL string; both also carry the same wrong doc comment (see P1-63). | `pub fn extract_columns(` |
| P1-12 | core/rs/fractindex-core/src/util.rs:39 | #11 | `collection_max_select` (39-50) is `collection_min_select` (26-37) with `MIN` replaced by `MAX`; the shared `AND "{order_col}" != -1 AND "{order_col}" != 1` sentinel filter is duplicated with it. | `pub fn collection_max_select(` |
| P1-13 | core/rs/fractindex-core/src/util.rs:84 | #11 | `escape_ident` here is character-for-character `core/src/util.rs:117`, and `escape_arg` (89) is `core/src/util.rs:121`'s `escape_ident_as_value`. Identifier quoting is the security-relevant fact in this repo and it has two homes under two names. | `pub fn escape_ident(ident: &str) -> String {` |
| P1-14 | core/rs/fractindex-core/src/fractindex_view.rs:113 | #11 | `create_instead_of_update_trigger` (113-169) repeats the entire `CASE (SELECT count(*) ...) WHEN 1 THEN crsql_fract_key_between(...) WHEN 0 THEN -1 ELSE crsql_fract_fix_conflict_return_old_key(...)` expression from `create_instead_of_insert_trigger` (79-92). The conflict-resolution policy is written twice and has already drifted (see P1-59). | `fn create_instead_of_update_trigger(` |
| P1-15 | core/rs/fractindex-core/src/as_ordered.rs:136 | #11 | `create_simple_move_trigger` (136-159) is `create_pend_trigger` (113-134) with a different trigger name and `AFTER UPDATE OF` for `AFTER INSERT`; the identical `CASE ... WHEN -1 ... WHEN 1 ...` body is copied. | `fn create_simple_move_trigger(` |
| P1-16 | core/rs/fractindex-core/src/fractindex.rs:257 | #11 | `decrement_integer` (257-302) mirrors `increment_integer` (212-255) statement for statement with `+1`/`-1` and head-bound swaps. A single `step_integer(x, digits, dir)` is the one home for the carry/borrow loop. | `fn decrement_integer(x: &str, digits: &str) -> Result<Option<String>, &'static str> {` |
| P1-17 | core/rs/core/src/create_cl_set_vtab.rs:117 | #11 | The vtab dealloc-on-error block (117-123) is a verbatim copy of 37-43. | `                if *vtab != core::ptr::null_mut() {` |
| P1-18 | core/rs/integration_check/src/t/pk_only_tables.rs:90 | #11 | `setup_schema` is defined four times: module level (72-77) and nested inside `insert_pkonly_row` (90-95), `modify_pkonly_row` (132-137), `junction_table` (177-180). The three nested copies shadow the module-level one that already exists. | `    fn setup_schema(db: &ManagedConnection) {` |
| P1-19 | core/rs/integration_check/src/t/automigrate.rs:618 | #11 | `expect_tables` (618-633), `expect_indices` (635-652) and `expect_columns` (594-610) are one function: prepare, optional bind, walk rows, `if !expected.contains(..) { return Ok(false) }`, `Ok(len == expected.len())`. | `fn expect_tables(db: &ManagedConnection, expected: Vec<&str>) -> Result<bool, ResultCode> {` |
| P1-20 | core/rs/integration_check/src/t/automigrate.rs:213 | #11 | The ~130-line schema literal bound at 213-346 is a copy of the one at 60-189 with `width`/`height` added to three tables. A base string plus the delta would make the thing under test readable; as written the diff is invisible. | `      r#"` |
| P1-21 | core/rs/integration_check/src/t/tableinfo.rs:28 | #11 | `make_site` (28-31) is duplicated in test_db_version.rs:8-11, same body, same magic `"0000000000000000"`. | `fn make_site() -> *mut c_char {` |
| P1-22 | core/rs/core/src/lib.rs:104 | #23 | `sqlite3_crsqlcore_init` runs 111-511: 19 sequential registration blocks each with its own `if rc != OK` early return, interleaved with raw malloc, ext-data creation and two differently shaped error-cleanup paths (some free `site_id_buffer`, some free `ext_data`, some neither). Nesting is shallow but the branch count is far past the comprehension bar. | `pub extern "C" fn sqlite3_crsqlcore_init(` |
| P1-23 | core/rs/core/src/changes_vtab_write.rs:439 | #23 | `merge_insert` is 263 lines with four length-checked error exits, five early `Ok(OK)` returns, three `match merge_result` blocks and a five-way boolean state derivation (`is_delete`, `needs_resurrect`, `row_exists_locally`, `is_sentinel_only`, `does_cid_win`). The delete / sentinel / resurrect / value paths are four separate merges wearing one function. | `unsafe fn merge_insert(` |
| P1-24 | core/rs/core/src/changes_vtab_write.rs:28 | #23 | `did_cid_win` nests a `match step()` inside an `if ret == 0 && mergeEqualValues` inside a `match step()`, with a `reset_cached_stmt` on all nine exits. The version compare and the site-id tiebreak are two decisions in one body. | `fn did_cid_win(` |
| P1-25 | core/rs/core/src/changes_vtab.rs:61 | #23 | `changes_best_index` builds the WHERE clause, the bitmask, the ORDER BY clause and the cost estimate in one 130-line body, with a four-deep `for`/`if`/`if let`/`if let`/`if` nest at 73-100 and a four-branch cost ladder at 150-176. | `fn changes_best_index(` |
| P1-26 | core/rs/fractindex-core/src/fractindex.rs:79 | #23 | `midpoint` mixes a recursive common-prefix walk, an unchecked `while` scan over `b_bytes[n]`, and a four-deep `if let`/`if let`/`if`/`else` digit ladder with returns at every level. | `fn midpoint(a: &str, b: Option<&str>, digits: &str) -> Result<String, &'static str> {` |
| P1-27 | core/rs/fractindex-core/src/fractindex.rs:20 | #23 | `key_between`'s three non-trivial match arms each re-derive integer part, fractional part and increment/decrement with their own early returns; the `(None,None)`/`(Some,Some)`/`(None,Some)`/`(Some,None)` cases share no helper. | `pub fn key_between(a: Option<&str>, b: Option<&str>) -> Result<Option<String>, &'static str> {` |
| P1-28 | core/rs/core/src/lib.rs:289 | #34 | A 16-line commented-out `create_function_v2` registration for `crsql_version`, complete with its error branch. Git holds it; here it reads as an intent nobody can act on, and it strands `x_crsql_version` (782) as dead code. | `    // let rc = db` |
| P1-29 | core/rs/core/src/bootstrap.rs:160 | #34 | Two commented-out migration gates (`update_to_0_13_0`, `update_to_0_15_0`) left inside `maybe_update_db_inner`, where a reader cannot tell whether the 0.13/0.15 migrations are pending, dropped, or superseded by the hard error above them. | `    // if recorded_version < consts::CRSQLITE_VERSION_0_13_0 {` |
| P1-30 | core/rs/core/src/changes_vtab_write.rs:559 | #34 | An eight-line commented-out `|| crsql_columnExists(...) == 0` disjunct sitting immediately above the `if is_sentinel_only` it used to belong to. It reads as a condition someone meant to restore. | `    /*` |
| P1-31 | core/rs/core/src/consts.rs:3 | #34 | Two commented-out version consts, one of which (`CRSQLITE_VERSION_0_15_0`) is live nine lines below at 16. A reader has to diff a comment against a declaration to learn which is authoritative. | `// pub const CRSQLITE_VERSION_0_15_0: i32 = 15_00_00;` |
| P1-32 | core/rs/integration_check/src/t/pk_only_tables.rs:34 | #34 | A 37-line commented-out `print_changes` function (34-70), fully formed Rust, kept as a debugging aid. It is the largest dead block in the tree and the `ColumnType` import at line 2 exists partly to keep its ghost plausible. | `// fn print_changes(` |
| P1-33 | core/rs/core/src/local_writes/after_update.rs:93 | #34 | A commented-out `if db.changes64() == 0 {` guard whose orphaned closing `// }` sits at line 97, so lines 95-96 are live code visually indented inside a comment. | `        // if db.changes64() == 0 { <-- an optimization if we can get to it. we'd need to know to increment causal length.` |
| P1-34 | core/rs/core/src/tableinfo.rs:914 | #18 | `is_table_compatible` narrates six checks as labeled phases (`// No unique indices besides primary key`, `// Must have a primary key`, `// All primary keys have to be non-nullable`, `// No auto-increment primary keys`, `// No checked foreign key constraints`, `// Check for default value or nullable`). Each phase is a named predicate spelled in prose plus a copy of the `db.count(...) != 0 { err.set(..); return Ok(false) }` block. | `    // No unique indices besides primary key` |
| P1-35 | core/rs/fractindex-core/src/as_ordered.rs:20 | #18 | `as_ordered` narrates numbered phases 0, 1, 2 and 4 in comments (20, 38, 60, 74). Phase 3 is missing, which is the tell: the numbering is a function boundary written in prose, and one step of it has already gone. | `    // 0. we should drop all triggers and views if they already exist` |
| P1-36 | core/rs/core/src/tableinfo.rs:30 | #27 | `TableInfo` is the hot symbol of the whole extension (imported by lib, changes_vtab, changes_vtab_write, local_writes, backfill, bootstrap, triggers, stmt_cache, alter, create_crr) and lives in a 1001-line module. Every task that touches a cached statement pays for all 1001 lines. | `pub struct TableInfo {` |
| P1-37 | core/rs/core/src/c.rs:114 | #27 | 353 of c.rs's 466 lines are generated bindgen layout tests (114-466), so the module that owns the hot `crsql_ExtData` FFI struct costs four times what its 113 lines of declarations do. The generated tests belong in their own module. | `fn bindgen_test_layout_crsql_Changes_vtab() {` |
| P1-38 | core/rs/core/src/c.rs:49 | #36 | Six `#[allow(non_snake_case, non_camel_case_types)]` / `#[allow(non_snake_case)]` attributes in one 466-line module (49, 75, 84, 115, 162, 260). The crate-wide FFI naming exemption is being re-declared per item instead of once at the module boundary. | `#[allow(non_snake_case, non_camel_case_types)]` |
| P1-39 | core/rs/core/src/util.rs:125 | #37 | `Countable` is a crate-private trait with exactly one impl (`for *mut sqlite::sqlite3`, line 129) and one consumer (tableinfo.rs `is_table_compatible`, five call sites). A free `fn count(db, sql)` says the same thing without the extension-trait indirection. | `pub trait Countable {` |
| P1-40 | core/rs/core/src/c.rs:10 | #38 | `INSERT_SENTINEL` and `DELETE_SENTINEL` are both `"-1"`. Two names for one value, distinguished only by which `col_version` parity the reader remembers; every comparison against either is really a comparison against `"-1"`. | `pub static INSERT_SENTINEL: &str = "-1";` |
| P1-41 | core/rs/core/src/alter.rs:91 | #38 | The compaction DELETE hardcodes `col_name != '-1'` and `col_name = '-1'` while the same function interpolates `crate::c::DELETE_SENTINEL` eight lines earlier (line 83). The sentinel value now has two homes inside one function. | `              "DELETE FROM \"{tbl_name}__crsql_clock\" WHERE (col_name != '-1' OR (col_name = '-1' AND col_version % 2 != 0))` |
| P1-42 | core/rs/core/src/tableinfo.rs:1 | #29 | 1001 lines, no `//!` header. Nothing on the first screen says this module owns the CRR schema model, the per-table prepared-statement cache and the compatibility rules. No file in the Rust tree has a `//!` header. | `use crate::alloc::string::ToString;` |
| P1-43 | core/rs/core/src/lib.rs:1 | #29 | 869 lines, no `//!` header. The crate root is the extension entry point (function registry, FFI surface, module wiring) and opens on a raw `#![cfg_attr]` plus a TODO about pub mods. | `#![cfg_attr(not(test), no_std)]` |
| P1-44 | core/rs/core/src/changes_vtab_write.rs:1 | #29 | 702 lines, no `//!` header, and the file holds the merge algorithm (causal length, resurrect, sentinel, tiebreak) that a reader most needs oriented. | `use alloc::boxed::Box;` |
| P1-45 | core/rs/core/src/changes_vtab.rs:1 | #29 | 574 lines, no `//!` header; the file implements the whole `crsql_changes` vtab (best_index, filter, next, column, update) with no statement of what the vtab is or what its column order means. | `extern crate alloc;` |
| P1-46 | core/rs/fractindex-core/src/fractindex.rs:1 | #29 | 447 lines, no `//!` header, and the module implements a base-95 fractional-index scheme whose key format (head char encodes integer length) is documented nowhere except inside `get_integer_len`. | `extern crate alloc;` |
| P1-47 | core/rs/integration_check/src/t/automigrate.rs:25 | #42 | Thirteen empty test functions (25-49: `change_index_col_order`, `add_many_cols`, `remove_many_cols`, `remove_indexed_cols`, `add_crr`, `add_table`, `remove_table`, `remove_crr`, `primary_key_change`, `with_default_value`, `not_null`, `nullable`, `no_default_value`), all called from `run_suite` at 667-679. They pass whatever automigrate does. | `fn change_index_col_order() {}` |
| P1-48 | core/rs/integration_check/src/t/fract.rs:5 | #42 | `sort_no_list_col` inserts five rows with colliding order keys and repositions one, then ends. No assertion on the resulting order, which is the only thing the test is named for. | `fn sort_no_list_col() {` |
| P1-49 | core/rs/integration_check/src/t/backfill.rs:8 | #42 | `new_empty_table` has no verdict; the body comment concedes it ("Just testing that we can execute these statements without error"). It cannot distinguish a correct backfill from a no-op one. | `    // Just testing that we can execute these statements without error` |
| P1-50 | core/rs/integration_check/src/t/backfill.rs:63 | #42 | `reapplied_empty_table` runs `crsql_as_crr` twice and selects the clock table twice with no assertion. Idempotence, the property it exists to pin, is unchecked. | `fn reapplied_empty_table() -> Result<(), ResultCode> {` |
| P1-51 | core/rs/integration_check/src/t/tableinfo.rs:290 | #42 | `test_create_clock_table_from_table_info` creates four clock tables and asserts nothing; line 324 says so. | `    // todo: Check that clock tables have expected schema(s)` |
| P1-52 | core/rs/integration_check/src/t/tableinfo.rs:327 | #42 | `test_leak_condition` drives two connections through schema changes and inserts with no assertion of any kind. Named for a leak it never observes; only a hard crash would fail it. | `fn test_leak_condition() {` |
| P1-53 | core/rs/integration_check/src/t/automigrate.rs:511 | #42 | `change_index_to_unique` migrates a plain index to a UNIQUE index and then asserts only that the index names are unchanged (524 concedes: `// TODO: test index uniqueness`). The verdict cannot fail if uniqueness is dropped. Same shape at 533 and 556 for index composition. | `    // TODO: test index uniqueness` |
| P1-54 | core/rs/integration_check/src/t/pack_columns.rs:69 | #44 | `assert!("unexpected type" == "")` used as a "fail here" marker in six else-branches (69, 74, 79, 101, 106, 111). The assertion is a comparison of two literals, so it specifies nothing; `panic!("unexpected type")` or a match on the expected variant does. | `        assert!("unexpected type" == "");` |
| P1-55 | core/rs/core/src/pack_columns.rs:95 | none | `val * 0x000000FF != 0` should be `val & 0x000000FF`. Multiplication here happens to be non-zero for the same inputs `&` would be, but it overflows in debug for `val > 0x0084_0000` and the intent is plainly the mask used on the three lines above. One-character bug in the pack encoder's length ladder. | `    } else if val * 0x000000FF != 0 {` |
| P1-56 | core/rs/core/src/automigrate.rs:428 | none | `mem_result` and `local_result` are bound once at 426-427 and never reassigned, so the `while` condition is loop-invariant: with two ROW results the loop only terminates through the `return` at 434, and `mem_result != local_result` at 440 can never see the loop's own stepping. Index-column comparison is effectively broken. | `    while mem_result == ResultCode::ROW && local_result == ResultCode::ROW {` |
| P1-57 | core/rs/core/src/automigrate.rs:423 | none | `IDX_COLS_SQL` takes a `?` (index name) but neither `fetch_idx_cols_mem` nor `fetch_idx_cols_local` binds it, so both statements run `pragma_index_info(NULL)` and return no rows. Every index-composition comparison silently passes. | `    let fetch_idx_cols_mem = mem_db.prepare_v2(IDX_COLS_SQL)?;` |
| P1-58 | core/rs/fractindex-core/src/fractindex_view.rs:227 | none | `.join(", AND")` emits `"a" = ?1, AND"b" = ?2` for composite primary keys: a stray comma and no space before AND. Multi-column primary keys cannot reach `fix_conflict_return_old_key` at all. Should be `" AND "`. | `        .join(", AND");` |
| P1-59 | core/rs/fractindex-core/src/fractindex_view.rs:155 | none | The update trigger interpolates `table = table` and `order_col = order_by_column.text()` raw, while the sibling insert trigger (95, 98) passes both through `escape_ident`. A table or column name containing a double quote breaks the trigger here and not there. | `        table = table,` |
| P1-60 | core/rs/fractindex-core/src/as_ordered.rs:26 | none | Both drop guards set an error on the context and then fall through to the next statement (26-28, 34-36), unlike every later guard in the function which returns. A failed drop is reported and then overwritten by the eventual success result. | `    if rc.is_err() {` |
| P1-61 | core/rs/core/src/lib.rs:782 | none | `x_crsql_version` is unreachable: its only registration is the commented-out block at 289-304. `consts::CRSQLITE_VERSION_STR` (consts.rs:15) is likewise referenced nowhere in the Rust tree. Both are dead. | `unsafe extern "C" fn x_crsql_version(` |
| P1-62 | core/rs/core/src/create_cl_set_vtab.rs:208 | none | `eof` unconditionally returns `ResultCode::OK as c_int`, which is 0, meaning "not at end of file". Any `SELECT` against a `clset` vtab loops forever on a cursor that `next` never advances. Eight stub methods (194-234) all return OK and are all wired into `MODULE`. | `extern "C" fn eof(_cursor: *mut sqlite::vtab_cursor) -> c_int {` |
| P1-63 | core/rs/fractindex-core/src/util.rs:52 | none | The doc comment "Stmt is returned to the caller since all values become invalid as soon as the statement is dropped" sits on `extract_pk_columns`, which returns `Vec<String>` and drops its statement. Repeated verbatim on `extract_columns` at 68. The comment describes a signature that no longer exists. | `/// Stmt is returned to the caller since all values become invalid as soon as the` |
| P1-64 | core/rs/core/src/automigrate.rs:382 | none | `if let Err(e) = local_db.exec_safe(&sql) { return Err(e); }` is `local_db.exec_safe(&sql)?;` written in four lines. The handler adds no strategy, only the appearance of one. | `        if let Err(e) = local_db.exec_safe(&sql) {` |
| P1-65 | core/rs/integration_check/src/t/pk_update.rs:1 | none | The whole module is a 26-line block comment describing tests that were never written, yet it is wired in at t/mod.rs:6 as `pub mod pk_update` and never called from `crsql_integration_check`. The pk-change replication case it describes has no coverage anywhere. | `/*` |
| P1-66 | core/rs/integration_check/src/t/pk_only_tables.rs:173 | none | `junction_table` (173-228) is 56 lines of test whose only call site is commented out at 266. Composite-primary-key sync is untested and the code that would test it is rotting in place. | `    // junction_table()?;` |
| P1-67 | core/rs/integration_check/src/t/automigrate.rs:453 | none | `remove_col_fract_table` is never called from `run_suite` (654-681), unlike the 13 empty stubs which are. Dropping a column from a fractionally-indexed table is the one migration path with a known view/trigger interaction, and it never runs. | `fn remove_col_fract_table() {` |
| P1-68 | core/rs/integration_check/src/t/tableinfo.rs:284 | none | The `ydoc` case creates a composite-primary-key STRICT table and then checks `atable2` again, repeating the assertion from 271-273 verbatim. The table under test is never passed to `is_table_compatible`; copy-paste left the case inert. | `    let is_compatible = test_exports::tableinfo::is_table_compatible(raw_db, "atable2", err)` |
| P1-69 | core/rs/core/src/local_writes/after_update.rs:77 | none | The message `"failed geteting or creating lookaside key"` is copied with its typo into four sites (after_update.rs:77 and :84, after_insert.rs:45, after_delete.rs:46). One `const` would have one spelling. | `            .or_else(\|_\| Err("failed geteting or creating lookaside key"))?;` |
| P1-70 | core/rs/core/src/changes_vtab.rs:207 | none | `get_clock_table_col_name` and `get_operator_string` (222) allocate a `String` per call for values that are all `&'static str` literals, on the best-index path that runs for every query against `crsql_changes`. Returning `Option<&'static str>` costs nothing and removes the `alloc` dependency from both. | `fn get_clock_table_col_name(col: &Option<CrsqlChangesColumn>) -> Option<String> {` |

## Phase 2 — audit finding verdicts

Per-row verdicts live in `corpus-ext/sheets/cr-sqlite.rs1.wave1.tsv` (146 rows, keyed
`file:line:rule:arm:digest`). Summary:

| rule | rows | real | fp |
|------|------|------|-----|
| rs:11 structural-clones (clone) | 38 | 32 | 6 |
| rs:11 structural-clones (clone-block) | 72 | 41 | 31 |
| rs:18 section-comments | 1 | 1 | 0 |
| rs:20 repeated-lambda | 1 | 0 | 1 |
| rs:23 cognitive-complexity | 9 | 8 | 1 |
| rs:27 purchase-price | 1 | 1 | 0 |
| rs:29 top-loading | 21 | 14 | 7 |
| rs:34 noop-code | 3 | 3 | 0 |
| **total** | **146** | **100** | **46** |

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| rs:11 | window re-tiling: one self-similar statement run yields a disjoint (or overlapping) tiling at every window size, so a single block produces up to five digests and sixteen findings | 26 | core/rs/core/src/tableinfo.rs:591:11:clone-block:79246e50f5aa |
| rs:11 | shape-only match: the matched block is a run of `exec_safe(<literal>)?` / `prepare_v2(<literal>)?` calls whose literals are all of its content and all differ | 5 | core/rs/integration_check/src/t/automigrate.rs:367:11:clone-block:c4b5e15542db |
| rs:11 | test-case scaffolding: shared arrange/act frame around assertions that are the point of each case | 4 | core/rs/integration_check/src/t/automigrate.rs:494:11:clone:b8a5b65af64b |
| rs:11 | minimum-body floor: two-statement function wrapping one already-parameterised library call | 2 | core/rs/core/src/create_cl_set_vtab.rs:264:11:clone:4b2029fe5935 |
| rs:29 | no size floor on "big": 150 to 182 line modules fire, and this repo has 30 Rust files with zero `//!` | 4 | core/rs/core/src/db_version.rs:1:29:top-loading:crsql_core::db_version |
| rs:29 | scope: prod-only rule read the `integration_check` test-runner crate | 3 | core/rs/integration_check/src/t/automigrate.rs:1:29:top-loading:crsql_integration_check::t::automigrate |
| rs:20 | triviality floor: the repeated closure is a single method call on its parameter, so it cannot drift and its name is the method | 1 | core/rs/core/src/unpack_columns_vtab.rs:81:20:closure:crsql_core::unpack_columns_vtab:f6fb5e5a |
| rs:23 | guard weighting: nesting-weighted `if remaining() < n { return Err }` guards inflate a flat table-driven decoder whose arms are independently readable | 1 | core/rs/core/src/pack_columns.rs:125:23:cognitive-complexity:crsql_core::pack_columns::unpack_columns |
| | **total** | **46** | |

## Phase 3 — reconciliation

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #11 | covered | the 16 accessors land as five separate digest groups rather than one family, but every member is reported |
| P1-2 | #11 | detector-miss | gapped clone: `bind_package_to_stmt(..)` swapped for a bind loop, so the blind digest of an otherwise identical body diverges |
| P1-3 | #11 | detector-miss | same one-statement substitution between `create_key` and `create_key_via_raw_values` |
| P1-4 | #11 | covered | 53b028cb7063 names it, and correctly links `ColumnInfo::clear_stmts` |
| P1-5 | #11 | covered | ca70e2204d5a catches 2 of the 19 registration blocks; the other 17 differ by a `Some(ptr)` argument |
| P1-6 | #11 | detector-miss | cross-file block clone (changes_vtab.rs:374 / changes_vtab_write.rs:488); the block arm only grouped within-file blocks here |
| P1-7 | #11 | detector-miss | the sync-bit merge block occurs 3x across three functions of one file and is not grouped, though the author's own TODO names it |
| P1-8 | #11 | detector-miss | six copies of the get-stmt / bind-chain / step wrapper across `local_writes`; each is a whole function, and the bind chains differ in arity |
| P1-9 | #11 | covered | b07f1b580a2c |
| P1-10 | #11 | detector-miss | triggers.rs drew zero findings; insert/delete trigger builders differ by NEW/OLD and one extra `let` |
| P1-11 | #11 | covered | 320f5aa049bf |
| P1-12 | #11 | covered | ff95ada4f457 |
| P1-13 | #11 | detector-miss | `escape_ident` is duplicated across the `core` and `fractindex-core` crates; the merged audit indexes each Cargo root separately, so no cross-crate group can form |
| P1-14 | #11 | detector-miss | the two INSTEAD OF trigger builders share a large SQL `CASE` expression but diverge in surrounding statements |
| P1-15 | #11 | covered | 059484befd06 |
| P1-16 | #11 | covered | via the block arm (1471549f2dc6); the whole-fn clone arm did not group increment/decrement |
| P1-17 | #11 | threshold-miss | the duplicated dealloc block is one `if` wrapping three statements, under the 5-statement floor |
| P1-18 | #11 | covered | fdd1c711fbc8 groups three of the four `setup_schema` copies; the fourth is inside dead `junction_table` |
| P1-19 | #11 | detector-miss | `expect_tables` / `expect_indices` / `expect_columns` differ by a `bind_text` line, enough to break the digest |
| P1-20 | #11 | detector-miss | the duplication is a 130-line string literal, which a statement-structure digest cannot see |
| P1-21 | #11 | covered | 020c425dcc09 |
| P1-22 | #23 | covered | cc 26 |
| P1-23 | #23 | covered | cc 31 |
| P1-24 | #23 | covered | cc 20 |
| P1-25 | #23 | covered | cc 41, the highest in the repo |
| P1-26 | #23 | covered | cc 32 |
| P1-27 | #23 | covered | cc 21 |
| P1-28 | #34 | covered | 16 lines |
| P1-29 | #34 | covered | reported twice, once per 3-line run |
| P1-30 | #34 | detector-miss | `/* ... */` block comment holding an expression fragment (`|| crsql_columnExists(..) == 0`), which does not parse as an item or statement |
| P1-31 | #34 | threshold-miss | 2-line run of commented-out consts, under the 3-line floor, and one of the two is live nine lines below |
| P1-32 | #34 | detector-miss | the largest commented-out block in the tree (37 lines, a complete `fn`) drew nothing; #29 did read this crate, so scope alone does not explain it |
| P1-33 | #34 | threshold-miss | 2-line run with its orphaned closing `// }` four lines further down |
| P1-34 | #18 | detector-miss | six labeled phases in `is_table_compatible`, but labeled by noun phrase rather than by number; only the numbered `as_ordered` case fired |
| P1-35 | #18 | covered | the sole #18 finding |
| P1-36 | #27 | covered | the sole #27 finding, and it names the same three hot symbols |
| P1-37 | #27 | threshold-miss | c.rs is 466 lines and 76% generated tests around the hot `crsql_ExtData`; it sits under the price cutoff that tableinfo.rs clears |
| P1-38 | #36 | threshold-miss | 6 `#[allow]` in 466 lines (1.3%) is under the density bar; #36 produced zero rows repo-wide |
| P1-39 | #37 | detector-miss | `Countable` has exactly one impl and one consumer, but it is written `pub trait` inside a private `mod util`, so a syntactic non-public check misses it; #37 produced zero rows repo-wide |
| P1-40 | #38 | threshold-miss | two aliased consts with the same value in one module, against a bar of the same literal in three modules |
| P1-41 | #38 | detector-miss | the drifted `'-1'` copies sit inside a `format!` in a function body, and #38 only reads module-level declarations |
| P1-42 | #29 | covered | 1001 lines |
| P1-43 | #29 | covered | 869 lines |
| P1-44 | #29 | covered | 702 lines |
| P1-45 | #29 | covered | 574 lines |
| P1-46 | #29 | covered | 447 lines |
| P1-47 | #42 | detector-miss | 13 empty test functions, all called from `run_suite`; #42 produced zero rows repo-wide |
| P1-48 | #42 | detector-miss | as P1-47: this crate carries no `#[test]` attribute, so a test-function detector keyed on it sees no tests at all |
| P1-49 | #42 | detector-miss | as P1-48 |
| P1-50 | #42 | detector-miss | as P1-48 |
| P1-51 | #42 | detector-miss | as P1-48 |
| P1-52 | #42 | detector-miss | as P1-48 |
| P1-53 | #42 | detector-miss | the three index tests were grouped as a #11 clone but never judged for a missing verdict |
| P1-54 | #44 | detector-miss | six `assert!("unexpected type" == "")` constant assertions; #44 produced zero rows, same `#[test]`-attribute cause |
| P1-55 | none | inventory-gap | `val * 0x000000FF` where `&` was meant, in the pack encoder's length ladder |
| P1-56 | none | inventory-gap | loop-invariant `while` in `maybe_recreate_index` |
| P1-57 | none | inventory-gap | `IDX_COLS_SQL`'s `?` parameter never bound, so index-composition comparison always passes |
| P1-58 | none | inventory-gap | `.join(", AND")` emits malformed SQL for composite primary keys |
| P1-59 | none | inventory-gap | update trigger interpolates identifiers raw where the sibling insert trigger escapes them |
| P1-60 | none | inventory-gap | error set on the context and then fallen through, twice |
| P1-61 | none | inventory-gap | `x_crsql_version` and `CRSQLITE_VERSION_STR` unreachable |
| P1-62 | none | inventory-gap | `clset` vtab `eof` always returns 0, plus eight OK-returning stub methods wired into MODULE |
| P1-63 | none | inventory-gap | doc comment describes a signature the function no longer has, twice |
| P1-64 | none | inventory-gap | `if let Err(e) = .. { return Err(e) }` in place of `?` |
| P1-65 | none | inventory-gap | `pk_update` module is a 26-line comment, wired in and never run |
| P1-66 | none | inventory-gap | `junction_table` dead, its call commented out |
| P1-67 | none | inventory-gap | `remove_col_fract_table` never called from `run_suite` |
| P1-68 | none | inventory-gap | copy-paste leaves the `ydoc` case asserting `atable2` again |
| P1-69 | none | inventory-gap | a typoed error message copied to four sites |
| P1-70 | none | inventory-gap | per-call `String` allocation for `&'static str` literals on the best-index path |

Totals: 25 covered, 23 detector-miss, 6 threshold-miss, 16 inventory-gap (70 sites).
