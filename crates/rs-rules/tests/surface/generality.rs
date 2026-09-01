//! #37 over Rust: the trait with one implementation, and the type parameter
//! every spelled use names the same.

use sightline_core::findings::Tier;
use sightline_testkit::run_rs_rule;

use crate::{causes, krate};

const TYPES: &str = "pub struct A;\npub struct B;\n";
const HOLDER: &str = "struct Holder<T>(T);\npub struct Bar;\npub struct Other;\n";

#[test]
fn a_private_trait_with_one_impl_is_speculative() {
    let src =
        format!("trait Step {{ fn go(&self); }}\n{TYPES}impl Step for A {{ fn go(&self) {{}} }}\n");
    let found = run_rs_rule("37", &krate(&src));

    assert_eq!(found.len(), 1);
    assert_eq!(&*found[0].site.symbol, "demo_crate::Step");
    assert_eq!(
        found[0].message,
        "trait demo_crate::Step has exactly one implementation (demo_crate::A) - speculative \
         abstraction"
    );
    assert_eq!(causes(&found), ["single-impl:demo_crate::Step"]);
    assert_eq!(found[0].tier(), Tier::Indexed);
}

#[test]
fn a_second_impl_exercises_the_trait() {
    let src = format!(
        "trait Step {{ fn go(&self); }}\n{TYPES}impl Step for A {{ fn go(&self) {{}} }}\n\
         impl Step for B {{ fn go(&self) {{}} }}\n"
    );

    assert!(run_rs_rule("37", &krate(&src)).is_empty());
}

#[test]
fn a_trait_the_crate_exports_is_answered_downstream() {
    let src = format!(
        "pub trait Step {{ fn go(&self); }}\n{TYPES}impl Step for A {{ fn go(&self) {{}} }}\n"
    );

    assert!(run_rs_rule("37", &krate(&src)).is_empty());
}

#[test]
fn a_trait_no_impl_names_stays_silent() {
    assert!(run_rs_rule("37", &krate("trait Marker {}\npub struct A;\n")).is_empty());
}

#[test]
fn one_impl_on_a_type_from_outside_the_repo_is_the_orphan_rule() {
    let src = "trait Step { fn go(&self); }\npub struct A;\n\
               impl Step for Vec<A> { fn go(&self) {} }\n";

    assert!(run_rs_rule("37", &krate(src)).is_empty());
}

#[test]
fn an_impl_for_the_parameter_itself_is_one_implementation() {
    // gigatoken's `PretokenCountable for T` and itertools' `FuncLR for F`:
    // one body offered to whatever meets the bound, judged real both times
    let src = format!(
        "trait Step {{ fn go(&self); }}\n{TYPES}impl<T: Clone> Step for T {{ fn go(&self) {{}} }}\n"
    );
    let found = run_rs_rule("37", &krate(&src));

    assert_eq!(found.len(), 1);
    assert_eq!(&*found[0].site.symbol, "demo_crate::Step");
}

#[test]
fn an_impl_on_a_reference_to_the_parameter_reads_the_same() {
    let src = format!(
        "trait Step {{ fn go(&self); }}\n{TYPES}impl<T> Step for &T {{ fn go(&self) {{}} }}\n"
    );

    assert_eq!(run_rs_rule("37", &krate(&src)).len(), 1);
}

#[test]
fn an_impl_on_a_type_it_parameterizes_is_a_family() {
    // log4rs's `ErasedDeserialize for DeserializeEraser<T>`: one implementor
    // for every T anyone erases, third-party types included
    let src = "trait Step { fn go(&self); }\npub struct Eraser<T>(T);\n\
               impl<T> Step for Eraser<T> { fn go(&self) {} }\n";

    assert!(run_rs_rule("37", &krate(src)).is_empty());
}

#[test]
fn a_trait_a_macro_implements_has_no_countable_impls() {
    let src = format!(
        "macro_rules! wire {{ ($t:ty) => {{ impl Step for $t {{ fn go(&self) {{}} }} }}; }}\n\
         trait Step {{ fn go(&self); }}\n{TYPES}impl Step for A {{ fn go(&self) {{}} }}\n"
    );

    assert!(run_rs_rule("37", &krate(&src)).is_empty());
}

#[test]
fn a_parameter_every_use_spells_the_same_is_monomorphic() {
    let src = format!(
        "{HOLDER}fn a(h: Holder<Bar>) {{}}\nfn b(h: Holder<Bar>) {{}}\n\
         fn c() -> Holder<Bar> {{ todo!() }}\n"
    );
    let found = run_rs_rule("37", &krate(&src));

    assert_eq!(causes(&found), ["monomorphic:demo_crate::Holder:T"]);
    assert_eq!(
        found[0].message,
        "type parameter `T` of demo_crate::Holder is Bar at all 3 instantiations the repo \
         spells - name the type"
    );
    assert_eq!(found[0].salience, 3.0);
    assert_eq!(found[0].tier(), Tier::Indexed);
}

#[test]
fn two_spelled_instantiations_are_under_the_floor() {
    let src = format!("{HOLDER}fn a(h: Holder<Bar>) {{}}\nfn b(h: Holder<Bar>) {{}}\n");

    assert!(run_rs_rule("37", &krate(&src)).is_empty());
}

#[test]
fn a_second_type_argument_exercises_the_parameter() {
    let src = format!(
        "{HOLDER}fn a(h: Holder<Bar>) {{}}\nfn b(h: Holder<Bar>) {{}}\n\
         fn c() -> Holder<Other> {{ todo!() }}\n"
    );

    assert!(run_rs_rule("37", &krate(&src)).is_empty());
}

#[test]
fn a_use_that_spells_no_argument_neither_counts_nor_silences() {
    let src = format!(
        "{HOLDER}fn a(h: Holder<Bar>) {{}}\nfn b(h: Holder<Bar>) {{}}\n\
         fn c() -> Holder<Bar> {{ todo!() }}\nfn d() {{ let _h = Holder(1); }}\n"
    );
    let found = run_rs_rule("37", &krate(&src));

    assert_eq!(causes(&found), ["monomorphic:demo_crate::Holder:T"]);
    assert!(found[0].message.contains("at all 3"));
}

#[test]
fn a_wildcard_argument_is_an_inferred_instantiation() {
    let src = format!(
        "{HOLDER}fn a(h: Holder<Bar>) {{}}\nfn b(h: Holder<Bar>) {{}}\n\
         fn c() -> Holder<Bar> {{ todo!() }}\nfn d(h: Holder<_>) {{}}\n"
    );

    assert!(run_rs_rule("37", &krate(&src)).is_empty());
}

#[test]
fn a_generic_impl_of_the_type_is_the_declaration_side() {
    // `impl<T> Holder<T>` names the parameter, not a type anyone instantiated
    let src = format!(
        "{HOLDER}impl<T> Holder<T> {{ fn get(&self) {{}} }}\nfn a(h: Holder<Bar>) {{}}\n\
         fn b(h: Holder<Bar>) {{}}\nfn c() -> Holder<Bar> {{ todo!() }}\n"
    );
    let found = run_rs_rule("37", &krate(&src));

    assert_eq!(causes(&found), ["monomorphic:demo_crate::Holder:T"]);
}

#[test]
fn a_published_generic_is_instantiated_downstream() {
    let src = format!(
        "pub {HOLDER}fn a(h: Holder<Bar>) {{}}\nfn b(h: Holder<Bar>) {{}}\n\
         fn c() -> Holder<Bar> {{ todo!() }}\n"
    );

    assert!(run_rs_rule("37", &krate(&src)).is_empty());
}

#[test]
fn a_turbofish_spells_a_generic_fn_instantiation() {
    let src = "pub struct Bar;\nfn pick<T>() -> u32 { 1 }\n\
               fn a() -> u32 { pick::<Bar>() }\nfn b() -> u32 { pick::<Bar>() }\n\
               fn c() -> u32 { pick::<Bar>() }\n";
    let found = run_rs_rule("37", &krate(src));

    assert_eq!(causes(&found), ["monomorphic:demo_crate::pick:T"]);
}

#[test]
fn a_call_with_no_turbofish_is_not_a_spelled_site() {
    let src = "pub struct Bar;\nfn pick<T>() -> u32 { 1 }\n\
               fn a() -> u32 { pick::<Bar>() }\nfn b() -> u32 { pick::<Bar>() }\n\
               fn c() -> u32 { pick::<Bar>() }\nfn d() -> u32 { pick() }\n";
    let found = run_rs_rule("37", &krate(src));

    assert_eq!(causes(&found), ["monomorphic:demo_crate::pick:T"]);
}

#[test]
fn a_wildcard_turbofish_still_silences_the_reading() {
    // wl-screenrec's `TypedObjectId<_>`: the source writes the argument and
    // leaves it unknown, so the spelled ones do not speak for every use
    let src = "pub struct Bar;\nfn pick<T>() -> u32 { 1 }\n\
               fn a() -> u32 { pick::<Bar>() }\nfn b() -> u32 { pick::<Bar>() }\n\
               fn c() -> u32 { pick::<Bar>() }\nfn d() -> u32 { pick::<_>() }\n";

    assert!(run_rs_rule("37", &krate(src)).is_empty());
}

#[test]
fn an_associated_type_argument_is_one_type_per_implementor() {
    // itertools `VecIntoIter<Self::Item>` in 19 trait signatures: `Self::Item`
    // is a type each implementor picks, not a type the repo names
    let src = "struct Holder<T>(T);\npub trait Sorted {\n    type Item;\n    \
               fn a(self) -> Holder<Self::Item>;\n    fn b(self) -> Holder<Self::Item>;\n    \
               fn c(self) -> Holder<Self::Item>;\n}\n";

    assert!(run_rs_rule("37", &krate(src)).is_empty());
}

#[test]
fn an_alias_passing_its_parameter_through_abstracts_nothing() {
    // termimad `pub(crate) type Result<T> = std::result::Result<T, Error>`
    let src = "pub struct Error;\ntype Res<T> = std::result::Result<T, Error>;\n\
               fn a() -> Res<()> { todo!() }\nfn b() -> Res<()> { todo!() }\n\
               fn c() -> Res<()> { todo!() }\n";

    assert!(run_rs_rule("37", &krate(src)).is_empty());
}

#[test]
fn an_alias_that_wraps_its_parameter_still_names_one_type() {
    let src = "pub struct Bar;\nstruct Holder<T>(T);\ntype Res<T> = (Holder<T>, u32);\n\
               fn a(r: Res<Bar>) {}\nfn b(r: Res<Bar>) {}\nfn c(r: Res<Bar>) {}\n";
    let found = run_rs_rule("37", &krate(src));

    assert_eq!(causes(&found), ["monomorphic:demo_crate::Res:T"]);
}
