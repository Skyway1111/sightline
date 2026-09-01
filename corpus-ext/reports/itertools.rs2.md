# itertools (rs2) - blind judge report

Repo: `../gauntlet-corpus/itertools` (crate `itertools` 0.15.0, edition 2018,
lib-only, ~9.1k lines prod in `src/`, ~15k lines test in `tests/`+`benches/`+`examples/`).
Read cold. No sightline output seen; no cargo/rustc run.

## Phase 1 - blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | src/lazy_buffer.rs:36 | #6 | `get_next` is named like the pure `get_at`/`get_array` beside it but advances the fused source and pushes into `buffer`, returning a success flag. Every caller (`permutations.rs:76`, `combinations.rs`) reads a mutation as a read. Name it `fill_next`/`try_extend_one`. | `pub fn get_next(&mut self) -> bool {` / `if let Some(x) = self.it.next() { self.buffer.push(x); true }` |
| P1-2 | src/lib.rs:3266 | #11 | Seven `sorted*` methods carry the same three-statement body `let mut v = Vec::from_iter(self); v.sort_X(..); v.into_iter()` at 3273, 3309, 3346, 3378, 3414, 3451, 3489. One private `fn sort_into_iter(self, f)` would own it. | `let mut v = Vec::from_iter(self);` / `v.sort_unstable_by(cmp);` / `v.into_iter()` |
| P1-3 | src/flatten_ok.rs:41 | #11 | `next` (41-71) and `next_back` (123-153) are a 30-line mirror clone: identical loop, identical two "necessary for FusedIterator" comments, differing only in front/back field and `next`/`next_back`. Any fix to the fuse logic has to land twice. | `if let Some(inner) = &mut self.inner_front {` / `if let Some(item) = inner.next() { return Some(Ok(item)); }` |
| P1-4 | src/either_or_both.rs:384 | #11 | `insert_left` (384-408) and `insert_right` (426-450) are a 24-line mirror clone including the `ptr::read`/`ptr::write`/`unreachable_unchecked` unsafe block and all four SAFETY comments. Duplicated `unsafe` is the worst class of copy to leave. | `let right = std::ptr::read(right as *mut _);` / `std::ptr::write(self as *mut _, Both(val, right));` |
| P1-5 | src/merge_join.rs:171 | #11 | `impl OrderingOrBool<T,T> for F` (171-190) and `impl ... for MergeLte` (192-211) are 19-line clones: identical `left`, `right`, `size_hint` (comment included) and a `merge` differing only in `self(&left,&right)` vs `left <= right`. | `fn size_hint(left: SizeHint, right: SizeHint) -> SizeHint {` / `// Not ExactSizeIterator because size may be larger than usize` / `size_hint::add(left, right)` |
| P1-6 | src/peeking_take_while.rs:58 | #11 | `PeekingNext for PutBack<I>` (58-71) and `PeekingNext for PutBackN<I>` (79-92) have byte-identical 13-line `peeking_next` bodies. | `if let Some(r) = self.next() {` / `if !accept(&r) { self.put_back(r); return None; }` |
| P1-7 | src/process_results_impl.rs:45 | #11 | `fold` (45-60) and `rfold` (73-88) are 14-line clones differing only in `try_fold` vs `try_rfold`; the error-capture closure is written twice. | `.try_fold(init, |acc, opt| match opt {` / `Err(e) => { *error = Err(e); Err(acc) }` |
| P1-8 | src/groupbylazy.rs:340 | #11 | `new` (340-359) and `new_chunks` (504-523) repeat the whole ten-field `GroupInner` initializer verbatim; only `key` and the wrapper type differ. Adding a field to `GroupInner` means editing two literals. | `current_key: None, current_elt: None, done: false,` / `top_group: 0, oldest_buffered_group: 0, bottom_group: 0,` |
| P1-9 | src/format.rs:55 | #11 | `FormatWith::fmt` (55-70) and `Format::format` (88-105) are 16-line clones: same `inner.take()` panic guard, same first-element-then-separator loop, differing only in `format(fst, &mut |..|)` vs `cb(&fst, f)`. | `let mut iter = match self.inner.take() { Some(t) => t,` / `None => panic!("Format: was already formatted once"), };` |
| P1-10 | src/size_hint.rs:22 | #11 | `add_scalar` (22), `sub_scalar` (31) and `mul_scalar` (52) are three copies of the same five-line destructure/saturate/map shape; only the arithmetic op differs. | `let (mut low, mut hi) = sh;` / `low = low.saturating_add(x);` / `hi = hi.and_then(|elt| elt.checked_add(x));` |
| P1-11 | src/grouping_map.rs:334 | #11 | `max_by` (334) and `min_by` (415) are the same five-line `self.reduce(|acc,key,val| match compare(..))` with the two arms swapped; the `max`/`min` (308/389) and `max_by_key`/`min_by_key` (363/444) pairs mirror the same way. | `self.reduce(|acc, key, val| match compare(key, &acc, &val) {` / `Ordering::Less \| Ordering::Equal => val,` |
| P1-12 | src/lib.rs:3612 | #11 | `k_smallest_by_key` (3612), `k_smallest_relaxed_by_key` (3711), `k_largest_by_key` (3801) and `k_largest_relaxed_by_key` (3889) are four copies of the same signature, where-clause and one-line `key_to_cmp` delegation, each with a copy-pasted doc block. | `fn k_largest_by_key<F, K>(self, k: usize, key: F) -> VecIntoIter<Self::Item>` / `self.k_largest_by(k, k_smallest::key_to_cmp(key))` |
| P1-13 | src/array_impl.rs:134 | #18 | `CircularArrayWindows::next` narrates two phases in prose: `// Initialisation code, when next() is called for the first time` (136) and `// Normal case. Read the next item ...` (190). Those labels are a function boundary; the init arm is 50 lines. | `// Initialisation code, when next() is called for the first time` / `// Normal case. Read the next item in the logical` |
| P1-14 | src/flatten_ok.rs:41 | #18 | `next` labels `// Handle the front inner iterator.` (43) and `// Handle the back inner iterator.` (58); `next_back` repeats the pair at 125/140. Two named phases inside one 30-line loop. | `// Handle the front inner iterator.` / `// Handle the back inner iterator.` |
| P1-15 | src/flatten_ok.rs:75 | #18 | `fold` is split by bare `// Front` (80) and `// Back` (91) labels around three folds; `rfold` repeats the pair at 162/173. | `// Front` / `// Back` |
| P1-16 | src/k_smallest.rs:5 | #18 | `k_smallest_general` ends with a three-item numbered narration `1) "pop" ... 2) shrink ... 3) restore` (76-78) describing the drain loop that follows: a phase list spelled in prose inside an 85-line function. | `// 1) "pop" the largest item off the heap into the tail slot of the underlying storage,` / `// 2) shrink the logical size of the heap by 1,` |
| P1-17 | src/flatten_ok.rs:82 | #20 | `\|a, o\| f(a, Ok(o))` is written six times in this module (82, 87, 93, 164, 169, 175). Name it once. | `Some(x) => x.fold(init, \|a, o\| f(a, Ok(o))),` |
| P1-18 | src/grouping_map.rs:312 | #20 | `\|_, v1, v2\| V::cmp(v1, v2)` appears at 312, 393 and 483 (`max`, `min`, `minmax`). | `self.max_by(\|_, v1, v2\| V::cmp(v1, v2))` |
| P1-19 | src/grouping_map.rs:368 | #20 | `\|key, v1, v2\| f(key, v1).cmp(&f(key, v2))` appears at 368, 449 and 565 (`max_by_key`, `min_by_key`, `minmax_by_key`). | `self.max_by(\|key, v1, v2\| f(key, v1).cmp(&f(key, v2)))` |
| P1-20 | src/adaptors/mod.rs:924 | #20 | `\|v\| v.as_ref().map(&mut f).unwrap_or(true)` is repeated at 924, 934 and 958 (`FilterOk::fold`, `::collect`, `::rfold`). | `.filter(\|v\| v.as_ref().map(&mut f).unwrap_or(true))` |
| P1-21 | src/adaptors/mod.rs:1029 | #20 | `\|v\| transpose_result(v.map(&mut f))` is repeated at 1029, 1039 and 1063 (`FilterMapOk::fold`, `::collect`, `::rfold`). | `.filter_map(\|v\| transpose_result(v.map(&mut f)))` |
| P1-22 | src/array_impl.rs:134 | #23 | `CircularArrayWindows::next` is 80 lines of `match`/`match`/`if`/`for`/`if`/`for` nested six deep, mixing one-shot buffer construction with the steady-state ring read, and carrying a `TODO: can this be improved?` mid-body. | `for j in i..N { items[j] = items[j - i].clone(); }` / `prefix_pos = N - i;` |
| P1-23 | src/minmax.rs:48 | #23 | `minmax_impl` is a 65-line function whose `loop` holds a nested `match`/`match` prologue plus two symmetric three-way `if`/`else if` ladders over `(min, min_key, max, max_key)`; the "3 comparisons for 2 elements" trick is only explained in a comment. | `if !lt(&second, &first, &second_key, &first_key) {` / `if lt(&first, &min, &first_key, &min_key) { min = first; min_key = first_key; }` |
| P1-24 | src/permutations.rs:66 | #23 | `Permutations::next` is a five-arm state-machine `match` of ~48 lines; the `Buffered` arm alone nests an `if/else` over a `for` loop with two early `return None` state transitions. | `PermutationState::Buffered { ref k, min_n } => {` / `for _ in 0..prev_iteration_count { if advance(&mut indices, &mut cycles) {` |
| P1-25 | src/lib.rs:3055 | #23 | `tree_reduce`, a default trait method, inlines a local `type State<T>` plus two mutually recursive generic `fn`s (`inner0`, `inner`) in 60 lines, ending in `match ... { _ => unreachable!() }`. The algorithm deserves its own module beside `k_smallest`. | `fn inner<T, II, FF>(stop: usize, it: &mut II, f: &mut FF) -> State<T>` / `match inner(usize::MAX, &mut self, &mut f) { Err(x) => x, _ => unreachable!() }` |
| P1-26 | src/k_smallest.rs:5 | #23 | `k_smallest_general` is 85 lines holding two nested `fn` definitions (`sift_down`, `children_of`), two early-return special cases (k==0, k==1), a heapify loop, a streaming loop and a heapsort drain. Five separable concerns in one body. | `fn sift_down<T, F>(heap: &mut [T], is_less_than: &mut F, mut origin: usize)` / `while heap.len() > 1 { let last_idx = heap.len() - 1;` |
| P1-27 | src/lib.rs:1 | #27 | `lib.rs` is 5343 lines and holds the crate's hot symbol, the `Itertools` trait (457-5200): every task touching any adaptor loads the whole file. The per-method doc blocks are the bulk; the method bodies mostly delegate to the module that already exists. | `pub trait Itertools: Iterator {` |
| P1-28 | src/adaptors/mod.rs:1 | #27 | `adaptors/mod.rs` is 1265 lines carrying ten unrelated adaptors (`Interleave`, `PutBack`, `Batching`, `MapSpecialCase` re-exports, `TupleCombinations`, `FilterOk`, `FilterMapOk`, `Positions`, `Update`, `WhileSome`) plus `checked_binomial`. Its siblings `map.rs`/`coalesce.rs` show the split it never finished, and its `//!` header is the license text, so the first screen says nothing about what the module is. | `//! Licensed under the Apache License, Version 2.0` |
| P1-29 | src/groupbylazy.rs:1 | #29 | 673-line module, no `//!` header. It holds the crate's trickiest state machine (`GroupInner` with `bottom_group`/`oldest_buffered_group`/`top_group`/`dropped_group` and a shared `RefCell`), and the first screen is `use alloc::vec::{self, Vec};`. | `use alloc::vec::{self, Vec};` |
| P1-30 | src/grouping_map.rs:1 | #29 | 619-line module, no `//!` header; a reader has to reach line 40 to learn `GroupingMap` is a lazy fold-by-key builder. | `use crate::{` / `    adaptors::map::{MapSpecialCase, MapSpecialCaseFn},` |
| P1-31 | src/either_or_both.rs:1 | #29 | 514-line module, no `//!` header, defining the crate's second public enum (`EitherOrBoth`) with ~40 methods including two `unsafe` ones. | `use core::ops::{Deref, DerefMut};` |
| P1-32 | src/merge_join.rs:1 | #29 | 348-line module, no `//!` header; it introduces four traits/types (`FuncLR`, `OrderingOrBool`, `MergeFuncLR`, `MergeLte`) whose relationship is never stated anywhere. | `use std::cmp::Ordering;` |
| P1-33 | src/combinations.rs:1 | #29 | 318-line module, no `//!` header; the `PoolIndex` trait and its three impls (`Box<[usize]>`, `Vec<usize>`, `[usize; K]`) are the module's whole design and go unexplained. | `use alloc::boxed::Box;` |
| P1-34 | src/adaptors/coalesce.rs:1 | #29 | 288-line module, no `//!` header; it is the shared engine behind `coalesce`, `dedup`, `dedup_by`, `dedup_with_count` and `dedup_by_with_count` via three adapter traits, and nothing on the first screen says so. | `use std::fmt;` |
| P1-35 | src/peeking_take_while.rs:1 | #29 | 274-line module, no `//!` header; ~30 of its lines are `peeking_next_by_clone!` invocations over std iterator types and the rule for which types qualify lives only in a trailing comment at 272. | `use crate::PutBack;` |
| P1-36 | src/next_array.rs:1 | #29 | 269-line module, no `//!` header. It is the crate's only `MaybeUninit` code, with a hand-written drop-safety argument in scattered comments and no top-level statement of the invariant. | `use core::mem::{self, MaybeUninit};` |
| P1-37 | src/adaptors/mod.rs:813 | #34 | An eleven-line commented-out Rust program that generated the twelve `impl_tuple_combination!` lines below it, left inline. Git remembers it; the comment even admits "It could probably be replaced by a bit more macro cleverness." | `//    for i in 2..=12 {` / `//        println!("impl_tuple_combination!(Tuple{arity}Combination ...` |
| P1-38 | src/unziptuple.rs:52 | #34 | A four-line commented-out `extend_reserve` block (52-55) plus a commented `extend_one` call at 59, both parked behind "Still unstable #72631". The tracking issue is the record; the dead code is not. | `// let (lower_bound, _) = self.size_hint();` / `//     $($FromT.extend_reserve(lower_bound);)*` |
| P1-39 | src/groupbylazy.rs:110 | #34 | A six-line block-commented debug `println!` at the head of `GroupInner::step`, the module's hottest function. | `/*` / `println!("client={}, bottom_group={}, oldest_buffered_group={}, top_group={}, buffers=[{}]",` |
| P1-40 | src/sources.rs:3 | #36 | A module-wide inner `#![allow(deprecated)]` over the whole 153-line module. It is there so the module can define its own deprecated items, but it also silences every *future* deprecation any code in the module touches. Put `#[allow(deprecated)]` on the items instead. | `#![allow(deprecated)]` |
| P1-41 | src/lib.rs:175 | #36 | `#[allow(deprecated)]` on `pub use crate::structs::*;` blankets a glob re-export of the entire `structs` module, so any item deprecated there in future is silently re-exported without warning. The neighbouring 114 and 173 name the deprecated item explicitly; this one does not. | `#[allow(deprecated)]` / `pub use crate::structs::*;` |
| P1-42 | src/ziptuple.rs:57 | #36 | `#[allow(unused_assignments)]` sits on the whole generated `Iterator` impl (both `next` and `size_hint`) and is re-applied for all thirteen `impl_zip_iter!` arities, so any dead store added to either method later is invisible in every expansion. Five `#[allow]` in 137 lines is the crate's densest module. | `#[allow(non_snake_case)]` / `#[allow(unused_assignments)]` / `impl<$($B),*> Iterator for Zip<($($B,)*)>` |
| P1-43 | src/merge_join.rs:104 | #37 | `FuncLR` is a crate-private trait (its module is private and it is not re-exported) with exactly one impl, a blanket one at 108, and exactly one use: naming `F::T` inside the `MergeJoinBy` type alias at 99. An associated-type projection dressed as an interface. | `pub trait FuncLR<L, R> { type T; }` / `impl<L, R, T, F: FnMut(&L, &R) -> T> FuncLR<L, R> for F { type T = T; }` |
| P1-44 | src/adaptors/mod.rs:27 | #38 | The literal `"iterator adaptors are lazy and do nothing unless consumed"` is written 44 times across 28 modules. It has already drifted: `sources.rs:66` says `"iterators are lazy and do nothing unless consumed"` and `grouping_map.rs` says `"GroupingMap is lazy and do nothing unless consumed"` (ungrammatical). One `macro_rules! lazy_adaptor` or a `const` would keep the sentence in one place. | `#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]` |
| P1-45 | tests/test_std.rs:1643 | #42 | `into_group_map_with_hasher` runs an `empty::<(u8,u8)>()` iterator into a `HashMap` and asserts nothing. It passes whatever the function returns; only the type annotation is checked, and that is the compiler's job. | `let _: HashMap<_, _, TestHasher> =` / `empty::<(u8, u8)>().into_group_map_with_hasher(TestHasher::new());` |
| P1-46 | tests/test_std.rs:1649 | #42 | `into_group_map_by_with_hasher`: same shape, empty input, no verdict. | `let _: HashMap<_, _, TestHasher> =` / `empty::<(u8, u8)>().into_group_map_by_with_hasher(\|x\| *x, TestHasher::new());` |
| P1-47 | tests/test_std.rs:1655 | #42 | `into_grouping_map_with_hasher`: `.collect()` on an empty source, no assertion. | `let _: HashMap<_, Vec<_>, TestHasher> = empty::<(u8, u8)>()` / `.into_grouping_map_with_hasher(TestHasher::new()).collect();` |
| P1-48 | tests/test_std.rs:1662 | #42 | `into_grouping_map_by_with_hasher`: same, no assertion. | `let _: HashMap<_, Vec<_>, TestHasher> = empty::<(u8, u8)>()` / `.into_grouping_map_by_with_hasher(\|x\| *x, TestHasher::new()).collect();` |
| P1-49 | tests/test_std.rs:1669 | #42 | `counts_with_hasher`: one binding, no assertion. | `let _: HashMap<_, _, TestHasher> = empty::<u8>().counts_with_hasher(TestHasher::new());` |
| P1-50 | tests/test_std.rs:1674 | #42 | `counts_by_with_hasher`: one binding, no assertion. All six `*_with_hasher` tests could be one test that actually feeds non-empty data and asserts the grouping. | `let _: HashMap<_, _, TestHasher> =` / `empty::<u8>().counts_by_with_hasher(\|x\| x, TestHasher::new());` |
| P1-51 | tests/test_core.rs:86 | #42 | `product_temporary` iterates a 27-element `iproduct!` with an empty loop body (`// ok`) and asserts nothing. It would pass if `iproduct!` yielded nothing at all. | `for (_x, _y, _z) in iproduct!(` / `) { // ok }` |
| P1-52 | tests/test_core.rs:114 | #42 | `izip2` binds two `izip!` results to `iter::Zip<_, _>` and asserts nothing; the neighbouring `izip_macro` (line 96) already covers the values. | `let _zip1: iter::Zip<_, _> = izip!(1.., 2..);` / `let _zip2: iter::Zip<_, _> = izip!(1.., 2..,);` |
| P1-53 | tests/test_core.rs:162 | #42 | `chain2` calls `chain!` twice, binds to `_`, and asserts nothing. | `let _ = chain!(1.., 2..);` / `let _ = chain!(1.., 2..,);` |
| P1-54 | tests/macros_hygiene.rs:7 | #42 | `iproduct_hygiene` (7), `izip_hygiene` (15) and `chain_hygiene` (22) each bind four macro expansions to `_` and assert nothing. The intended oracle is "it compiles", which a `tests/compile/` or trybuild file states honestly; as `#[test]` bodies they pass whatever the macros do at runtime. | `let _ = itertools::iproduct!(0..6, 0..9);` |
| P1-55 | src/k_smallest.rs:18 | #48 | `children_of` is a private nested `fn` with a one-expression body and exactly one call site, eleven lines below it at 23. The hop and the `#[inline]` buy nothing; inline it. | `fn children_of(n: usize) -> (usize, usize) { (2 * n + 1, 2 * n + 2) }` |
| P1-56 | src/either_or_both.rs:378 | none | `insert_right`'s doc example is copy-pasted from `insert_left` and never calls `insert_right` in its "Overwriting a pre-existing value" case: it asserts on `either.insert_left(69)` under `insert_right`'s own docs. A doctest that exercises the neighbouring function. | `/// let mut either: EitherOrBoth<_, ()> = Left(0_u32);` / `/// assert_eq!(*either.insert_left(69), 69);` |
| P1-57 | src/lib.rs:760 | none | Panic contracts are documented two ways: nine sites use bold prose `**Panics**` (lib.rs 545, 629, 760, 2793, 2821, 5240; groupbylazy 266; rciter_impl 43; sources 134) and three use the rustdoc `# Panics` heading (lib.rs 3193, 3222; next_array 28). Only the heading form is machine-visible and `clippy::missing_panics_doc`-checkable. One convention, one home. | `/// **Panics** if `size` is 0.` |
| P1-58 | src/format.rs:58 | none | `FormatWith`'s `Display` impl panics on a second format and the panic is documented only on the `Itertools::format_with` method (lib.rs:2821), not on the public `FormatWith`/`Format` structs a user names directly. No `# Panics` heading anywhere in the module. | `None => panic!("FormatWith: was already formatted once"),` |
| P1-59 | src/groupbylazy.rs:339 | none | Both crate-internal constructors carry the stub doc `/// Create a new` with the sentence unfinished (339 and 503). Duplicated placeholder prose reads as a doc the author meant to come back to. | `/// Create a new` / `pub fn new<K, J, F>(iter: J, f: F) -> ChunkBy<K, J::IntoIter, F>` |
| P1-60 | src/with_position.rs:69 | none | `Position` exposes `pub is_first`/`pub is_last` fields and also `pub fn is_first(self)`/`pub fn is_last(self)` that return them. Two public spellings of one fact, and `p.is_first` vs `p.is_first()` is a silent foot-gun (a method value is not a bool). Neither accessor has a caller in `src/` or `tests/`. | `pub fn is_first(self) -> bool { self.is_first }` |
| P1-61 | src/grouping_map.rs:527 | none | `minmax_by`'s `MinMax(min, max)` arm ends with `else { MinMaxResult::MinMax(min, max) }`, reconstructing exactly what it destructured. The no-op branch is a shape every reader still has to check. | `} else { MinMaxResult::MinMax(min, max) }` |
| P1-62 | tests/quick.rs:354 | none | A sixteen-line block-commented-out set of three `#[quickcheck]` property tests, headed `NOTE: Range<i8> is broken! (all signed ranges are)`. Disabled tests carrying an unactioned bug report: either fix, file, or delete. | `/*` / ` * NOTE: Range<i8> is broken!` / `#[quickcheck] fn size_range_i8(a: Iter<i8>) -> bool { exact_size(a) }` |
| P1-63 | src/next_array.rs:185 | none | `tracked_drop` is an 84-line `#[test]` running five unnamed scenario blocks against one function-scoped `static DROPPED`, resetting it with `swap(0, ..)` between them. A failure in block 3 reports as "tracked_drop failed"; blocks 4 and 5 never run. Five `#[test]`s with a fresh counter each. | `static DROPPED: AtomicU16 = AtomicU16::new(0);` / `assert_eq!(DROPPED.swap(0, Ordering::Relaxed), 2);` |
| P1-64 | src/adaptors/mod.rs:855 | none | `test_checked_binomial` is quadratic in an unexplained `const LIMIT: usize = 500;`: 251,001 `assert_eq!` calls plus 501 `Vec` rebuilds of 501 elements, on every `cargo test`. No comment says why 500 rather than 50, and nothing in the code under test has that bound. | `const LIMIT: usize = 500;` / `for n in 0..=LIMIT { for k in 0..=LIMIT {` |

### Ideals with no site found

- **#9** shared static written by three functions of its own module: none in prod.
  The only `static` in `src/` is `next_array.rs:189`, function-scoped inside a
  `#[cfg(test)]` fn (see P1-63).
- **#32 / #56**: this is a lib crate whose public API *is* the root, so no
  `pub` item is unreachable in the #32 sense and none is reached only from
  tests. `Position::is_exactly_one`/`is_middle` and several `EitherOrBoth`
  methods have zero references anywhere, but they are shipped API, not dead
  code (see P1-60 for the accessor pair that is genuinely redundant).
- **#44**: no assertion of a call-free expression against itself and no
  `assert!(true/false)` anywhere in `tests/`, `benches/`, `examples/` or
  `#[cfg(test)]`.
- **#47**: no `sleep`, no `Duration`, anywhere in the repo.
- **#53**: the crate has no `# Errors` section at all; the absent-section half
  belongs to `clippy::missing_errors_doc`, not to #53.
- **#59**: prod has no entry point that spends off the machine (no I/O, no
  process, no network, no deletion). `examples/iris.rs` reads an embedded
  `include_str!`.

## Phase 2 - audit finding verdicts

Sheet: `corpus-ext/sheets/itertools.rs2.wave1.tsv`, 4 rows, all `rs:48`
(fold-candidate, `fold` arm, indexed tier). 1 `real`, 3 `fp`.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| src/groupbylazy.rs:376 | rs:48 | indexed | fp | `ChunkBy::step` (367) is the same one-line `borrow_mut()` shape with the same single caller and was not flagged: `drop_group` is one half of the two-method facade that keeps every `borrow_mut()` on the private `inner` RefCell inside one impl block, and folding it alone leaks the cell into a `Drop` impl and breaks the symmetry the ChunkBy/IntoChunks pair is built on. |
| src/groupbylazy.rs:575 | rs:48 | indexed | fp | Same site shape as `ChunkBy::drop_group`: the twin `IntoChunks::step` (570) is identical and unflagged, so folding this one produces three different spellings of one access pattern across two parallel type families and moves a RefCell `borrow_mut()` into `Chunk::drop`. |
| src/k_smallest.rs:18 | rs:48 | indexed | real | `children_of` is a doubly nested private fn whose whole body is `(2 * n + 1, 2 * n + 2)`, with its single call site five lines below at 23; the name adds a hop and an `#[inline]` and no reuse, and inlining the tuple into the destructuring `let` reads better. |
| src/next_array.rs:128 | rs:48 | indexed | fp | This is an `unsafe fn` carrying its own `# Safety` doc section, and it is the crate's polyfill for the unstable std `MaybeUninit::slice_assume_init_mut` it is named after; folding it merges two independent safety obligations (the caller's validity invariant and the cast's layout argument) into one unsafe block, deletes the documented contract, and orphans the SAFETY comment at 96 that names it. |

### False-positive shapes

| rule | fp class | count | example key |
|------|----------|-------|-------------|
| rs:48 | private-field-facade: the fn's body is the only sanctioned access to a private `RefCell`/`Cell` field, and it is one of >=2 same-shape sibling methods forming that facade (its twin `step` has the identical single-caller property and was not flagged) | 2 | `src/groupbylazy.rs:376:48:fold:itertools::groupbylazy::ChunkBy::drop_group` |
| rs:48 | contract-carrying fn: an `unsafe fn` (or any fn with a `# Safety` doc section); the name is a promise of a proof obligation, not of reuse, and folding merges two independent safety arguments into one block | 1 | `src/next_array.rs:128:48:fold:itertools::next_array::slice_assume_init_mut` |

## Phase 3 - reconciliation

Audit: 126 findings over rules 11 (54), 42 (36), 29 (18), 23 (8), 48 (4),
20 (3), 27 (3). Rules 6, 9, 18, 32, 34, 36, 37, 38, 44, 47, 53, 56, 59 fired
zero times on this repo.

Of 64 phase-1 sites: 28 covered, 9 detector-miss, 18 threshold-miss,
9 inventory-gap.

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #6 | detector-miss | `lazy_buffer.rs:36 get_next` mutates the fused source and the buffer behind a `get_` name; #6 fired nowhere in the repo. The rs reading is "accessor-named functions whose callee graph has effects", and `Iterator::next` plus `Vec::push` on `&mut self` is that graph. |
| P1-2 | #11 | threshold-miss | The seven `sorted*` bodies are three statements each, under the whole-fn arm's size floor. #11 fired 54 times elsewhere; no finding anywhere in `lib.rs`. |
| P1-3 | #11 | covered | `11 src/flatten_ok.rs:41` (clone x2 with `next_back`). Also `23 src/flatten_ok.rs:41`. |
| P1-4 | #11 | detector-miss | `insert_left`/`insert_right` (24 lines each) are mirror clones, but the mirroring moves the wildcard across the tuple (`Both(left, _)` vs `Both(_, right)`), so an identifier-normalised token digest sees two different sequences. #11 did fire on the `as_ref`/`as_deref` pairs in the same file (151/160/169/182), which do not mirror. |
| P1-5 | #11 | detector-miss | The clone unit is a whole `impl OrderingOrBool` block (171-190 vs 192-211), not a fn or a >=5-statement block: each constituent fn (`left`, `right`, `size_hint`) is 1-3 lines, and `merge` differs by one expression. An impl block is not in either arm's vocabulary. |
| P1-6 | #11 | covered | `11 src/peeking_take_while.rs:58` (clone x2 with `PutBackN::peeking_next` at 79). |
| P1-7 | #11 | covered | `11 src/process_results_impl.rs:45` (clone x2 with `rfold` at 73). |
| P1-8 | #11 | detector-miss | `new` (340) and `new_chunks` (504) repeat the ten-field `GroupInner` literal, but the one differing field is `key: f` vs `key: ChunkIndex::new(size)`: an identifier substituted by a call. The digest normalises identifiers, not identifier-for-call. #11 did fire on `Group::next`/`Chunk::next` in the same file (493/667). |
| P1-9 | #11 | detector-miss | Same class as P1-8: `FormatWith::fmt` (55) and `Format::format` (88) differ only by `format(fst, &mut \|disp\| ...)` vs `cb(&fst, f)`, a closure argument where the twin has a plain one. No #11 anywhere in `format.rs`. |
| P1-10 | #11 | covered | `11 src/size_hint.rs:22` (clone x3 with `sub_scalar` 31, `mul_scalar` 52), exactly the group named. |
| P1-11 | #11 | covered | `11 src/grouping_map.rs:334` (x2 with `min_by` 415); the `*_by_key` trio is also reported at 363/444/560. |
| P1-12 | #11 | threshold-miss | The four `k_*_by_key` bodies are one line each, under the size floor; only the copy-pasted doc blocks make the group large. |
| P1-13 | #18 | covered | Site matched by `23 src/array_impl.rs:134` (cc 28). #18 itself fired nowhere: the two phase labels (`// Initialisation code...` 136, `// Normal case.` 190) sit on `match` arms rather than at statement level. |
| P1-14 | #18 | covered | Site matched by `11 src/flatten_ok.rs:41` and `23 src/flatten_ok.rs:41`. The `// Handle the front/back inner iterator.` pair went unread by #18. |
| P1-15 | #18 | covered | Site matched by `11 src/flatten_ok.rs:75`. The bare `// Front` / `// Back` labels went unread by #18. |
| P1-16 | #18 | threshold-miss | The three numbered steps are one contiguous comment run (76-78), not two separately placed labels, so the "phase" count is 1. Nothing reported at `k_smallest.rs:5`. |
| P1-17 | #20 | threshold-miss | `\|a, o\| f(a, Ok(o))` occurs 6x in `flatten_ok`, well over the 3x count, so the cutoff that excluded it is the nontriviality floor: the body is one call. The three closures #20 did report are all two calls or a method chain. |
| P1-18 | #20 | threshold-miss | Same cutoff: `\|_, v1, v2\| V::cmp(v1, v2)` is one call and occurs 3x (312, 393, 483); its two-call sibling in the same module was reported. |
| P1-19 | #20 | covered | `20 src/grouping_map.rs:368`, the exact closure and count. |
| P1-20 | #20 | covered | `20 src/adaptors/mod.rs:924`. |
| P1-21 | #20 | covered | `20 src/adaptors/mod.rs:1029`. |
| P1-22 | #23 | covered | `23 src/array_impl.rs:134`, cc 28, the repo's second highest. |
| P1-23 | #23 | covered | `23 src/minmax.rs:48`, cc 31, the repo's highest. |
| P1-24 | #23 | covered | `23 src/permutations.rs:66`, cc 15, exactly at the threshold. |
| P1-25 | #23 | threshold-miss | `tree_reduce`'s two nested `fn`s (`inner0`, `inner`) are scored as separate functions, so the outer body scores under 15. The reader still ingests all 60 lines as one method. |
| P1-26 | #23 | threshold-miss | Same nested-fn split: `sift_down` and `children_of` are scored apart from `k_smallest_general`, leaving the 85-line outer body under 15. |
| P1-27 | #27 | covered | `27 src/lib.rs:1` (5343 lines, 5 hot symbols). |
| P1-28 | #27 | covered | `27 src/adaptors/mod.rs:1` (1265 lines, 3 hot symbols). |
| P1-29 | #29 | covered | `29 src/groupbylazy.rs:1` (673 lines, 12 top-level items). |
| P1-30 | #29 | covered | `29 src/grouping_map.rs:1`; also `27 src/grouping_map.rs:1`. |
| P1-31 | #29 | covered | `29 src/either_or_both.rs:1`. |
| P1-32 | #29 | covered | `29 src/merge_join.rs:1`. |
| P1-33 | #29 | covered | `29 src/combinations.rs:1`. |
| P1-34 | #29 | covered | `29 src/adaptors/coalesce.rs:1`. |
| P1-35 | #29 | covered | `29 src/peeking_take_while.rs:1`. |
| P1-36 | #29 | covered | `29 src/next_array.rs:1`. |
| P1-37 | #34 | detector-miss | The comment run at 813-824 is prose, then eleven lines of Rust, then prose; the run as a whole does not parse, so a whole-run parse test rejects it. The parseable code block inside it is the site. |
| P1-38 | #34 | detector-miss | Two causes: the run is headed by the prose line `// Still unstable #72631`, and the commented code carries macro-repetition syntax (`$($FromT.extend_reserve(lower_bound);)*`) that only parses inside a `macro_rules!` body. |
| P1-39 | #34 | detector-miss | The commented-out `println!` at 110-115 is a `/* */` block, not a `//` run; the rs arm reads `//` runs only. |
| P1-40 | #36 | threshold-miss | One `#![allow]` over 153 lines is far under any density bar, yet its scope is the whole module. Density misses the scope axis: an inner `#![allow]` blinds every line under it. |
| P1-41 | #36 | threshold-miss | Same scope-vs-density gap: 4 allows over 5343 lines, but the one at 175 blankets a glob re-export of the entire `structs` module. |
| P1-42 | #36 | threshold-miss | 5 allows in 137 lines (3.6%) is the crate's densest module and still under the bar; #36 fired nowhere in the repo. |
| P1-43 | #37 | threshold-miss | Excluded by the rule's own restriction: `FuncLR`'s single impl is a blanket `impl<..., F: FnMut(&L, &R) -> T> FuncLR<L, R> for F`, i.e. for a type parameter, not "a type the repo owns". #37 fired nowhere. |
| P1-44 | #38 | detector-miss | The 44 copies live in `#[must_use = "..."]` attributes, not in module-level `const`/`static` declarations, so the literal index never sees them. The drift is already visible (`"iterators are lazy..."`, `"GroupingMap is lazy and do nothing..."`). |
| P1-45 | #42 | threshold-miss | Not reported. The body is a single `let _: HashMap<_, _, TestHasher> = ...`; comparing against `chain2` (unannotated `let _`, reported at test_core.rs:162) the exemption is the type annotation, read as a compile-time verdict. Defensible for a type-level test, but the input here is `empty::<(u8,u8)>()`, so nothing about the grouping is checked. |
| P1-46 | #42 | threshold-miss | Same annotated-binding exemption. |
| P1-47 | #42 | threshold-miss | Same annotated-binding exemption. |
| P1-48 | #42 | threshold-miss | Same annotated-binding exemption. |
| P1-49 | #42 | threshold-miss | Same annotated-binding exemption. |
| P1-50 | #42 | threshold-miss | Same annotated-binding exemption. |
| P1-51 | #42 | covered | `42 tests/test_core.rs:86` (`product_temporary`). |
| P1-52 | #42 | threshold-miss | Same annotated-binding exemption (`let _zip1: iter::Zip<_, _> = ...`), and it confirms the pattern: its unannotated twin `chain2` was reported. |
| P1-53 | #42 | covered | `42 tests/test_core.rs:162` (`chain2`). |
| P1-54 | #42 | covered | `42 tests/macros_hygiene.rs:7`, plus 15 and 22 for the other two. |
| P1-55 | #48 | covered | `48 src/k_smallest.rs:18`, judged `real` in phase 2. |
| P1-56 | none | inventory-gap | A doctest under `insert_right` that exercises `insert_left`. No rule reads doc-example bodies for the symbol they are attached to. |
| P1-57 | none | inventory-gap | Two spellings of the panic contract (`**Panics**` prose 9x vs the `# Panics` heading 3x). No rule owns doc-section convention drift; #53 is `# Errors` only. |
| P1-58 | none | inventory-gap | A public `Display` impl that panics, with the panic documented only on the `Itertools` method, not on the struct a user names. Same gap as P1-57. |
| P1-59 | none | inventory-gap | Duplicated stub doc `/// Create a new` on both crate-internal constructors. No rule reads doc-comment content for completeness. |
| P1-60 | none | inventory-gap | `Position` ships `pub is_first` fields and `pub fn is_first()` accessors returning them, with no caller for either accessor. Neither #32 (the API is root-reachable in a lib crate) nor #37 (no trait, no type parameter) covers a redundant accessor over a public field. |
| P1-61 | none | inventory-gap | The `else { MinMaxResult::MinMax(min, max) }` arm reconstructs what it destructured. #34's rs arm needs *every* arm of a match to re-return its input; a single identity branch inside a transforming match is unread. |
| P1-62 | none | inventory-gap | 16 lines of block-commented-out `#[quickcheck]` tests carrying an unactioned bug note. #34 reads prod only, and would need the block-comment form (P1-39) anyway. |
| P1-63 | none | inventory-gap | An 84-line `#[test]` running five scenarios against one function-scoped `static`, where a failure in scenario 3 hides 4 and 5. The test-family inventory has no "one test, many scenarios" reading, and #23 reads prod only. |
| P1-64 | none | inventory-gap | A quadratic test (`LIMIT = 500`, 251k assertions) on every `cargo test`. #47 owns wall-clock sleeps only; nothing prices a test's own cost. |

### What the reconciliation says

- The four rules that carried this repo (#11, #29, #23, #27) landed on their
  sites precisely: 15 of 15 sites I raised for them that fell inside their
  stated arms were covered, and the four #23 numbers (28, 31, 15, 19) rank the
  functions the way a reader would.
- The largest single miss class is #11's blindness to *mirrored* clones
  (P1-4, P1-8, P1-9): three of the crate's most expensive duplications differ
  from their twin by one substitution the identifier-normalising digest cannot
  absorb, while the unmirrored pairs beside them in the same files were all
  caught. This is the one change with real yield here.
- #18, #34, #36 and #38 each found zero sites in a repo that has them, and in
  every case the cause is a form the arm does not read: comment runs that mix
  prose with code, `/* */` blocks, `#![allow]` scope rather than density, and
  literals inside attributes.
- Nine sites map to no rule at all, and seven of those nine are documentation
  defects (a wrong doctest, two panic-contract gaps, a stub doc, a redundant
  accessor pair). Doc honesty is the inventory's thinnest coverage on Rust.
