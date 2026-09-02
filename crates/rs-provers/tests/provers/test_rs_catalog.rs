//! The class of a listed path, the
//! lookup rule (longest `::` prefix, then the method any receiver spells),
//! and the one resolver both readers share - a callee's spelling through the
//! module's `use` bindings before the table is asked.

use std::collections::BTreeMap;

use sightline_core::catalog::{
    BLOCKS, ClassSet, DELETES, LOGS, MUTATES, PROCESS, READS, REMOTE, SPAWNS, WRITES,
};
use sightline_rs_provers::catalog::{classes, effects_of};
use sightline_testkit::build_rs;

fn of(path: &str) -> &'static ClassSet {
    classes(Some(path))
}

#[test]
fn a_listed_path_answers_its_classes() {
    assert_eq!(*of("std::fs::read_to_string"), ClassSet::from([READS]));
    assert_eq!(*of("std::fs::write"), ClassSet::from([WRITES]));
    // a subprocess is a process of its own, where every other spawn is a
    // thread of this one: #59 reads the difference
    assert_eq!(
        *of("std::process::Command::new"),
        ClassSet::from([SPAWNS, PROCESS])
    );
    assert_eq!(*of("std::process::exit"), ClassSet::from([PROCESS]));
    assert_eq!(
        *of("std::net::TcpStream::connect"),
        ClassSet::from([REMOTE, READS, WRITES])
    );
    assert_eq!(*of("std::io::stdin"), ClassSet::from([READS]));
    assert_eq!(*of("std::io::stderr"), ClassSet::from([WRITES]));
    assert_eq!(*of("println"), ClassSet::from([LOGS]));
    assert_eq!(*of("eprintln"), ClassSet::from([LOGS]));
    assert_eq!(*of("dbg"), ClassSet::from([LOGS]));
    assert_eq!(*of("log::warn"), ClassSet::from([LOGS]));
    assert_eq!(*of("tracing::info"), ClassSet::from([LOGS]));
    assert_eq!(*of("tokio::spawn"), ClassSet::from([SPAWNS]));
    assert_eq!(*of("std::thread::spawn"), ClassSet::from([SPAWNS]));
    assert_eq!(*of("std::thread::sleep"), ClassSet::from([BLOCKS]));
    assert_eq!(*of("tokio::time::sleep"), ClassSet::from([BLOCKS]));
    // a process-global setting is both: the process it writes is the one its
    // reader is holding, which is how #59 tells it from a spend
    assert_eq!(*of("std::env::set_var"), ClassSet::from([PROCESS, MUTATES]));
    assert_eq!(*of("reqwest::get"), ClassSet::from([REMOTE]));
}

/// `tokio::fs` is `std::fs` on the runtime's pool, member for member, so a
/// repo whose file work is async is not read as pure (bob's
/// `get_downloads_directory` creates the tree it reports).
#[test]
fn a_mirrored_module_resolves_to_the_one_it_mirrors() {
    assert_eq!(*of("tokio::fs::create_dir_all"), ClassSet::from([WRITES]));
    assert_eq!(
        *of("tokio::fs::remove_dir_all"),
        ClassSet::from([WRITES, DELETES])
    );
    assert_eq!(*of("tokio::fs::read_to_string"), ClassSet::from([READS]));
    // not mirrored
    assert_eq!(
        *of("tokio::net::TcpStream::connect"),
        ClassSet::from([REMOTE])
    );
}

#[test]
fn a_member_overrides_its_root() {
    assert_eq!(*of("std::fs::metadata"), ClassSet::from([READS]));
    assert_eq!(
        *of("std::fs::remove_dir_all"),
        ClassSet::from([WRITES, DELETES])
    );
}

#[test]
fn a_method_on_any_receiver_is_keyed_by_its_name() {
    // a lock is how a shared read is spelled, not a wait its caller feels
    assert_eq!(*of("state.inner.lock"), ClassSet::from([MUTATES]));
    assert_eq!(*of("rx.recv"), ClassSet::from([BLOCKS]));
    // `[T]::join` on most receivers
    assert!(of("parts.join").is_empty());
}

#[test]
fn an_unlisted_path_is_empty_and_never_an_error() {
    assert!(of("crate::util::helper").is_empty());
    assert!(of("").is_empty());
    assert!(classes(None).is_empty());
}

#[test]
fn effects_of_resolves_a_spelling_through_the_module_bindings() {
    let (_dir, stack) = build_rs(&[(
        "src/lib.rs",
        "use std::fs;\n\
         use std::fs::remove_file;\n\
         use tokio::time::sleep;\n\
         pub fn go(p: &str) {\n\
         \x20   fs::read_to_string(p);\n\
         \x20   remove_file(p);\n\
         \x20   sleep(1);\n\
         \x20   helper(p);\n\
         \x20   println!(\"hi\");\n\
         }\n\
         fn helper(p: &str) {}\n",
    )]);
    let facts = stack.facts();
    let provers = stack.provers();
    let module = &facts.modules["demo_crate"];
    let body = provers.body("demo_crate::go");

    let found: BTreeMap<&str, ClassSet> = body
        .calls
        .iter()
        .chain(body.macros.iter())
        .map(|c| (c.path.as_str(), effects_of(module, c).clone()))
        .collect();
    assert_eq!(
        found,
        BTreeMap::from([
            ("fs::read_to_string", ClassSet::from([READS])),
            ("remove_file", ClassSet::from([WRITES, DELETES])),
            ("sleep", ClassSet::from([BLOCKS])),
            ("helper", ClassSet::new()),
            ("println", ClassSet::from([LOGS])),
        ])
    );
}
