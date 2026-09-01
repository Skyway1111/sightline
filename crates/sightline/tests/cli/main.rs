//! The verbs end to end on mini repos (port of `tests/test_cli.py`,
//! `test_gate.py`, `test_lang.py` and `tests/rs/test_rs_gate.py`): the built
//! binary is driven as a reader drives it, and the oracle is off by config
//! for speed. The oracle-on end to end is the corpus gate.

mod audit;
mod gate;
mod rs_gate;

use std::process::Command;

use tempfile::TempDir;

/// `oracle = false`, as every fixture in the reference's CLI tests sets it.
pub const NO_ORACLE: &str = "[tool.sightline]\noracle = false\n";

/// One run of the binary: the exit code and both streams.
pub struct Out {
    pub code: i32,
    pub out: String,
    pub err: String,
}

pub fn root(dir: &TempDir) -> String {
    dir.path().to_string_lossy().replace('\\', "/")
}

pub fn run(args: &[&str]) -> Out {
    let done = Command::new(env!("CARGO_BIN_EXE_sightline"))
        .args(args)
        .output()
        .expect("the binary runs");
    Out {
        code: done.status.code().expect("the binary exits with a code"),
        out: String::from_utf8_lossy(&done.stdout).into_owned(),
        err: String::from_utf8_lossy(&done.stderr).into_owned(),
    }
}

/// The findings of a `--json` run as `(rule, file, symbol)` triples.
pub fn findings(out: &str) -> Vec<(String, String, String)> {
    let doc: serde_json::Value = serde_json::from_str(out).expect("the JSON output parses");
    doc["findings"]
        .as_array()
        .expect("findings is a list")
        .iter()
        .map(|f| {
            let text = |key: &str| f[key].as_str().unwrap_or_default().to_string();
            (text("rule"), text("file"), text("symbol"))
        })
        .collect()
}

pub fn provenance(out: &str) -> serde_json::Value {
    let doc: serde_json::Value = serde_json::from_str(out).expect("the JSON output parses");
    doc["provenance"].clone()
}
