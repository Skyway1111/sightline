//! The authoring-time gate for the #12 idiom
//! catalog. Every entry holds a naive/idiom exemplar pair and CrossHair
//! `diffbehavior` must find no counterexample. Run when the catalog changes.
//!
//! Entries equivalent only on a sub-domain declare a projection wrapper
//! applied to BOTH sides: the proven claim is on-domain equivalence. The
//! exemplars are `catalog/idioms/*.py`, one module per entry, kept at the
//! Python bytes they had; the fast matcher-to-exemplar pin is this file's
//! own test module (no z3 in the test suite).
//!
//! `--python` names the interpreter, which needs CrossHair installed on it;
//! without the flag, the first of `python3` and `python` on PATH.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::Result;

use crate::paths::{path_python, workspace_root};

/// One proven pair: the module its two sides live in, the two function names
/// CrossHair compares, the domain the claim holds on, and the near miss a
/// whole-function matcher must refuse (`None` for a node-level idiom, whose
/// matched node is the shape).
pub struct Entry {
    pub name: &'static str,
    pub module: &'static str,
    /// the matched shape, and the idiom it reimplements
    pub naive: &'static str,
    pub idiom: &'static str,
    /// the projected pair CrossHair proves where a domain restricts the claim
    pub check: Option<(&'static str, &'static str)>,
    pub domain: &'static str,
    #[cfg_attr(not(test), expect(dead_code, reason = "the near-miss pin reads it"))]
    pub near_miss: Option<&'static str>,
}

/// Whole-function entries first, then the node-level ones, as `_CATALOG` and
/// `_node_idioms` name them.
pub const ENTRIES: [Entry; 8] = [
    Entry {
        name: "binary-search",
        module: "binary_search",
        naive: "binary_search_naive",
        idiom: "binary_search_idiom",
        check: None,
        domain: "total",
        near_miss: Some("sift_up_near_miss"),
    },
    Entry {
        name: "clamp",
        module: "clamp",
        naive: "clamp_naive",
        idiom: "clamp_idiom",
        check: Some(("clamp_naive_on_domain", "clamp_idiom_on_domain")),
        domain: "lo <= hi",
        near_miss: Some("sign_near_miss"),
    },
    Entry {
        name: "tolower",
        module: "tolower",
        naive: "tolower_naive",
        idiom: "tolower_idiom",
        check: Some(("tolower_naive_on_domain", "tolower_idiom_on_domain")),
        domain: "ascii",
        near_miss: Some("caesar_near_miss"),
    },
    Entry {
        name: "manual-sum",
        module: "manual_sum",
        naive: "manual_sum_naive",
        idiom: "manual_sum_idiom",
        check: None,
        domain: "total",
        near_miss: Some("product_near_miss"),
    },
    Entry {
        name: "identity-comp",
        module: "identity_comp",
        naive: "identity_comp_naive",
        idiom: "identity_comp_idiom",
        check: None,
        domain: "total",
        near_miss: None,
    },
    Entry {
        name: "bool-ternary",
        module: "bool_ternary",
        naive: "bool_ternary_naive",
        idiom: "bool_ternary_idiom",
        check: None,
        domain: "total",
        near_miss: None,
    },
    Entry {
        name: "range-len",
        module: "range_len",
        naive: "range_len_naive",
        idiom: "range_len_idiom",
        check: None,
        domain: "total",
        near_miss: None,
    },
    Entry {
        name: "keys-membership",
        module: "keys_membership",
        naive: "keys_membership_naive",
        idiom: "keys_membership_idiom",
        check: None,
        domain: "total",
        near_miss: None,
    },
];

/// Suggestion branches beyond each entry's primary exemplar (`entry/arm`).
pub const EXTRA_ARMS: [(&str, &str, &str, &str); 3] = [
    (
        "bool-ternary/neg",
        "bool_ternary",
        "bool_ternary_neg_naive",
        "bool_ternary_neg_idiom",
    ),
    (
        "identity-comp/set",
        "identity_comp",
        "identity_setcomp_naive",
        "identity_setcomp_idiom",
    ),
    (
        "keys-membership/notin",
        "keys_membership",
        "keys_notin_naive",
        "keys_notin_idiom",
    ),
];

/// The planted pairs `--self-test` adds; both must be refuted.
const PLANTED: [(&str, &str, &str, &str); 2] = [
    (
        "planted-raw-bug",
        "selftest",
        "selftest_broken_naive",
        "selftest_broken_idiom",
    ),
    (
        "planted-projected-bug",
        "selftest",
        "selftest_projected_naive",
        "selftest_projected_idiom",
    ),
];

pub fn data_dir() -> PathBuf {
    workspace_root().join("catalog").join("idioms")
}

fn domain_of(name: &str) -> &'static str {
    ENTRIES
        .iter()
        .find(|e| e.name == name)
        .map_or("total", |e| e.domain)
}

/// One z3 subprocess per pair (about `timeout` s each when proven).
fn diffbehavior(
    python: &Path,
    dir: &Path,
    module: &str,
    a: &str,
    b: &str,
    timeout: f64,
) -> Result<(bool, String)> {
    let out = Command::new(python)
        .current_dir(dir)
        .args(["-m", "crosshair", "diffbehavior"])
        .arg(format!("{module}.{a}"))
        .arg(format!("{module}.{b}"))
        .arg(format!("--per_condition_timeout={timeout}"))
        .output()?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok((out.status.success(), text))
}

pub fn main(args: &[&str]) -> Result<u8> {
    let mut timeout = 10.0f64;
    let mut python: Option<PathBuf> = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match *arg {
            "--timeout" => {
                timeout = rest.next().and_then(|v| v.parse().ok()).unwrap_or(timeout);
            }
            "--python" => python = rest.next().map(PathBuf::from),
            _ => {}
        }
    }
    let python = match python {
        Some(path) => path,
        None => path_python()?,
    };
    let dir = data_dir();
    let self_test = args.contains(&"--self-test");

    let mut pairs: Vec<(&str, &str, &str, &str)> = ENTRIES
        .iter()
        .map(|e| {
            let (a, b) = e.check.unwrap_or((e.naive, e.idiom));
            (e.name, e.module, a, b)
        })
        .collect();
    pairs.extend(EXTRA_ARMS);
    if self_test {
        pairs.extend(PLANTED);
    }

    let mut failures = 0;
    for (name, module, a, b) in &pairs {
        let planted = PLANTED.iter().any(|(p, _, _, _)| p == name);
        let start = Instant::now();
        let (ok, output) = diffbehavior(&python, &dir, module, a, b, timeout)?;
        let wall = start.elapsed().as_secs_f64();
        failures += usize::from(planted || !ok);
        println!(
            "{name:<22} {:<18} domain={:<10} {wall:5.1}s",
            crate::text::label(planted, ok, "refuted"),
            domain_of(name)
        );
        if !ok && !planted {
            println!("  {}", output.trim().replace('\n', "\n  "));
        }
    }
    println!(
        "catalog_check: {}/{} proven",
        pairs.len() - failures,
        pairs.len()
    );
    Ok(u8::from(failures > 0))
}

/// One exemplar's own source out of its data file, as `inspect.getsource`
/// hands it to the Python pins. A top-level def or class needs no dedent.
#[cfg(test)]
pub fn source_of(dir: &Path, module: &str, name: &str) -> String {
    let text = std::fs::read_to_string(dir.join(format!("{module}.py")))
        .unwrap_or_else(|e| panic!("reading {module}.py: {e}"));
    let parsed = ruff_python_parser::parse_module(&text).expect("the exemplar parses");
    for stmt in &parsed.syntax().body {
        let (found, range) = match stmt {
            ruff_python_ast::Stmt::FunctionDef(f) => (f.name.as_str(), f.range),
            ruff_python_ast::Stmt::ClassDef(c) => (c.name.as_str(), c.range),
            _ => continue,
        };
        if found == name {
            return text[usize::from(range.start())..usize::from(range.end())].to_string();
        }
    }
    panic!("{module}.py holds no {name}");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every finding rule #12 reports over one exemplar, by idiom name.
    fn idioms_fired(source: &str) -> Vec<String> {
        sightline_testkit::run_rule("12", &[("m.py", source)])
            .into_iter()
            .filter_map(|f| {
                f.cause
                    .strip_prefix("idiom:")
                    .and_then(|rest| rest.split(':').next().map(str::to_string))
            })
            .collect()
    }

    #[test]
    fn every_exemplar_fires_its_own_idiom() {
        let dir = data_dir();
        for entry in &ENTRIES {
            let source = source_of(&dir, entry.module, entry.naive);
            let fired = idioms_fired(&source);
            assert!(
                fired.iter().any(|n| n == entry.name),
                "{} misses its exemplar (fired {fired:?})",
                entry.name
            );
        }
    }

    #[test]
    fn every_extra_arm_fires_its_entry() {
        let dir = data_dir();
        for (arm, module, naive, _idiom) in &EXTRA_ARMS {
            let base = arm.split('/').next().unwrap_or(arm);
            let fired = idioms_fired(&source_of(&dir, module, naive));
            assert!(
                fired.iter().any(|n| n == base),
                "{arm} exemplar misses its idiom (fired {fired:?})"
            );
        }
    }

    /// CrossHair proves naive == idiom; only this pin proves the matcher
    /// stops at the shape (sign is not clamp, a heap sift is not bisect, a
    /// Caesar shift is not tolower).
    #[test]
    fn whole_function_matchers_reject_their_near_miss() {
        let dir = data_dir();
        for entry in &ENTRIES {
            let Some(near) = entry.near_miss else {
                continue;
            };
            let fired = idioms_fired(&source_of(&dir, entry.module, near));
            assert!(
                !fired.iter().any(|n| n == entry.name),
                "{} matcher fires on its near miss",
                entry.name
            );
        }
    }

    /// A whole-function entry must pin the neighbour its matcher rejects.
    #[test]
    fn a_projection_entry_carries_a_near_miss() {
        for entry in ENTRIES.iter().filter(|e| e.check.is_some()) {
            assert!(entry.near_miss.is_some(), "{} has no near miss", entry.name);
        }
    }

    /// A projection restricts the domain, never patches a side: the two
    /// wrappers are the same shape once the side they call is normalized.
    #[test]
    fn projections_wrap_both_sides() {
        let dir = data_dir();
        for entry in &ENTRIES {
            let Some((naive, idiom)) = entry.check else {
                continue;
            };
            let shape = |name: &str| {
                source_of(&dir, entry.module, name)
                    .replace(name, "F")
                    .replace(entry.naive, "F")
                    .replace(entry.idiom, "F")
            };
            assert_eq!(
                shape(naive),
                shape(idiom),
                "{} projection asymmetric",
                entry.name
            );
        }
    }

    #[test]
    fn every_entry_names_a_data_file_that_parses() {
        let dir = data_dir();
        for entry in &ENTRIES {
            for name in [Some(entry.naive), Some(entry.idiom), entry.near_miss]
                .into_iter()
                .flatten()
            {
                assert!(!source_of(&dir, entry.module, name).is_empty());
            }
        }
    }
}
