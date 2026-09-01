//! What a run says on stderr while it works: the oracle pass lines, and
//! nothing else. `--quiet` turns them off, so a hook or a CI job reads only
//! the report. A finding, a note and an error are not progress and print
//! whatever this holds.

use std::sync::atomic::{AtomicBool, Ordering};

static QUIET: AtomicBool = AtomicBool::new(false);

/// `--quiet`, set from the command line before the first pass runs.
pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

/// One progress line, unless the run is quiet.
pub fn progress(line: &str) {
    if !QUIET.load(Ordering::Relaxed) {
        eprintln!("{line}");
    }
}
