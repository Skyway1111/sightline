//! What a call to a callable outside the repo does (port of
//! `rs/catalog.py`), on the effect-class axis `core::catalog` names for both
//! languages. A callable is keyed by its absolute path (`std::fs`, a root
//! standing for its whole module, a member overriding it), by the bare name
//! a macro spells (`println`), or - with a leading dot - by the method it
//! spells on any receiver (`.lock`). The longest `::` prefix wins.
//!
//! An unlisted callable is assumed pure: it computes and returns. The
//! receiver keys are name-level and say nothing about the type they land on,
//! so `.write` answers for a file and a lock alike.

use std::sync::LazyLock;

use sightline_core::catalog::{
    BLOCKS, Catalog, ClassSet, DELETES, LOGS, MUTATES, PROCESS, READS, REMOTE, SPAWNS, WRITES,
};
use sightline_core::pytext;
use sightline_rs_facts::model::RsModule;

use crate::RsCall;

/// `_BY_CLASS`, verbatim.
const BY_CLASS: &[(&str, &str)] = &[
    (
        READS,
        "std::fs std::io::stdin std::env::var std::env::vars std::env::args \
         std::net .read_to_string .read_to_end .read_line .read_dir",
    ),
    (
        WRITES,
        "std::io::stdout std::io::stderr std::fs::write std::fs::copy \
         std::fs::rename std::fs::create_dir std::fs::create_dir_all \
         std::fs::set_permissions std::fs::File::create std::fs::hard_link \
         std::fs::remove_file std::fs::remove_dir std::fs::remove_dir_all \
         std::net .write_all .flush",
    ),
    (
        DELETES,
        "std::fs::remove_file std::fs::remove_dir std::fs::remove_dir_all",
    ),
    (LOGS, "println eprintln print eprint dbg log tracing slog"),
    // off the machine: a socket, and the clients a Rust repo reaches for
    (
        REMOTE,
        "std::net reqwest hyper ureq isahc surf awc tonic tokio::net \
         tokio_postgres sqlx redis lapin",
    ),
    (
        SPAWNS,
        "std::process::Command std::thread::spawn std::thread::Builder \
         tokio::spawn tokio::task::spawn tokio::task::spawn_blocking \
         rayon::spawn rayon::join",
    ),
    // a wait on something the caller cannot see. No `.join`: spelled on any
    // receiver it is `[T]::join` far more often than `JoinHandle::join` (doxx
    // `parts.join(".")` was #6's one fp there). No `.lock` either: it is how
    // a shared read is spelled in Rust, and it stays MUTATES for what the
    // guard is then used for (tower `Handle::get_error_on_closed`, rs2 fp)
    (
        BLOCKS,
        "std::thread::sleep tokio::time::sleep tokio::time::timeout \
         .recv .blocking_recv .blocking_send .wait",
    ),
    // the process world, which no reader of this one walks back:
    // `Command` is here as well as under SPAWNS because the thing it
    // spawns is a process, where every other spawn is a thread of this one
    (
        PROCESS,
        "std::env::set_var std::env::remove_var std::env::set_current_dir \
         std::process::exit std::process::abort std::panic::set_hook \
         std::process::Command",
    ),
    // shared state one call reorders for every other holder - the
    // process-global settings among them, which is what tells them apart
    // from the `std::process` entries beside them in PROCESS: a reader
    // inside the process still sees a hook or an env var (#59)
    (
        MUTATES,
        ".lock .write .borrow_mut .get_mut .insert .push .remove .clear \
         std::env::set_var std::env::remove_var std::env::set_current_dir \
         std::panic::set_hook",
    ),
];

/// A module that mirrors another member for member, so the table names the
/// members once: `tokio::fs::create_dir_all` is `std::fs::create_dir_all`
/// run on the runtime's pool.
const MIRRORS: [(&str, &str); 1] = [("tokio::fs", "std::fs")];

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| Catalog::new("::", BY_CLASS));

/// The classes a spelling names: its longest `::` prefix in the table, else
/// the method it spells on whatever receiver. A mirrored module resolves to
/// the one it mirrors first. An unlisted spelling is the empty set, never an
/// error.
pub fn classes(path: Option<&str>) -> &'static ClassSet {
    let raw = path.unwrap_or("");
    let mut node = raw.to_string();
    for (spelled, mirrored) in MIRRORS {
        if node == spelled || node.starts_with(&format!("{spelled}::")) {
            node = format!("{mirrored}{}", &node[spelled.len()..]);
            break;
        }
    }
    let tail = pytext::rpartition(&raw.replace("::", "."), ".")
        .2
        .to_string();
    CATALOG.classes_of(Some(&node), Some(&tail))
}

/// What one call site of this module does: the callee's spelling resolves
/// through the module's `use` bindings to an absolute path before the table
/// is asked, so `fs::read_to_string` under `use std::fs` and the path
/// written out answer alike.
pub fn effects_of(module: &RsModule<'_>, call: &RsCall<'_>) -> &'static ClassSet {
    let (head, _, rest) = pytext::partition(&call.path, "::");
    let Some(base) = module.bindings.get(head) else {
        return classes(Some(&call.path));
    };
    let spelled = if rest.is_empty() {
        base.clone()
    } else {
        format!("{base}::{rest}")
    };
    classes(Some(&spelled))
}
