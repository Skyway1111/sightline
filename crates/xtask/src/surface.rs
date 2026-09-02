//! This tree's own line budget: non-test lines under
//! `crates/` against `BOUND`. The bound moves only by an argument in the
//! commit that needs it.

use std::path::Path;

use anyhow::Result;

use crate::paths::workspace_root;

/// The implementation surface one agent context holds.
pub const BOUND: usize = 49_000;

/// Lines outside `#[cfg(test)]`. ceiling: the first `#[cfg(test)]` ends the
/// count for that file, so a test module that is not last hides the code
/// after it. Every crate here puts its tests last.
fn non_test_lines(text: &str) -> usize {
    text.lines()
        .take_while(|l| l.trim() != "#[cfg(test)]")
        .count()
}

fn walk(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            if name != "tests" && name != "target" {
                walk(&path, files)?;
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

/// Non-test lines under `crates/`.
pub fn count() -> Result<usize> {
    let mut files = Vec::new();
    walk(&workspace_root().join("crates"), &mut files)?;
    let mut total = 0;
    for path in files {
        total += non_test_lines(&std::fs::read_to_string(&path)?);
    }
    Ok(total)
}

pub fn main(_args: &[&str]) -> Result<u8> {
    let total = count()?;
    println!("surface: {total} non-test lines under crates/ (bound {BOUND})");
    Ok(u8::from(total > BOUND))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_module_is_not_counted() {
        let text = "fn a() {}\nfn b() {}\n#[cfg(test)]\nmod tests {\n    fn c() {}\n}\n";
        assert_eq!(non_test_lines(text), 2);
    }

    #[test]
    fn the_surface_is_under_the_bound() {
        assert!(count().unwrap() < BOUND);
    }
}
