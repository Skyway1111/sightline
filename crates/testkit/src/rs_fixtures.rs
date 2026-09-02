//! The fixture crates, one home for every test that drives the toolchain
//! (`build_rs_oracle`): dependency-free, so no run touches the network.

pub const MANIFEST: &str =
    "[workspace]\n\n[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";

/// `LIB`: the three dispatch shapes, a call beside a plain reference, a
/// definition outside the root, a `#[cfg(test)]`-only reader.
pub const LIB: &str = r#"pub trait Greet {
    fn hello(&self) -> u32;
    fn twice(&self) -> u32 {
        self.hello() + self.hello()
    }
}

pub struct Loud;

impl Greet for Loud {
    fn hello(&self) -> u32 {
        7
    }
}

pub const LIMIT: u32 = 3;

pub fn concrete(x: &Loud) -> u32 {
    x.hello()
}

pub fn generic<T: Greet>(x: &T) -> u32 {
    x.hello()
}

pub fn dynamic(x: &dyn Greet) -> u32 {
    x.hello()
}

pub fn defaulted(x: &Loud) -> u32 {
    x.twice()
}

pub fn limited() -> u32 {
    LIMIT
}

pub fn outside() -> String {
    String::new()
}

pub fn caller() -> u32 {
    concrete(&Loud) + limited()
}

pub fn helper() -> u32 {
    1
}

pub fn apply(f: fn() -> u32) -> u32 {
    f()
}

pub fn both() -> u32 {
    apply(helper) + helper()
}

fn only_asserted() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asserts() {
        assert_eq!(only_asserted(), 3);
    }
}
"#;

/// The `crate` fixture: one package, `LIB` at `src/lib.rs`.
pub const CRATE: &[(&str, &str)] = &[("Cargo.toml", MANIFEST), ("src/lib.rs", LIB)];

const WORKSPACE_MANIFEST: &str =
    "[workspace]\nmembers = [\"good\", \"broken\", \"dependent\"]\nresolver = \"2\"\n";

/// `member(name, extra)`: a package manifest, `extra` appended (dependencies,
/// features, `publish = false`; `""` for none).
pub fn member(name: &str, extra: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{extra}")
}

/// The `workspace` fixture: a clean member, a broken one, and one cargo
/// never reaches because its dependency failed (no artifact, no error of
/// its own).
pub fn workspace() -> Vec<(&'static str, String)> {
    vec![
        ("Cargo.toml", WORKSPACE_MANIFEST.to_string()),
        ("good/Cargo.toml", member("good", "")),
        (
            "good/src/lib.rs",
            "pub fn ok() -> u32 { 1 }\npub fn also() -> u32 { ok() }\n".to_string(),
        ),
        ("broken/Cargo.toml", member("broken", "")),
        (
            "broken/src/lib.rs",
            "use std::nope::Missing;\npub fn bad() -> u32 { 1 }\n".to_string(),
        ),
        (
            "dependent/Cargo.toml",
            member(
                "dependent",
                "\n[dependencies]\nbroken = { path = \"../broken\" }\n",
            ),
        ),
        (
            "dependent/src/lib.rs",
            "pub fn lone() -> u32 { 2 }\npub fn calls_lone() -> u32 { lone() }\n".to_string(),
        ),
    ]
}

/// The `siblings` fixture: two packages under a root with no manifest of
/// its own, one a path dependency of the other, so the oracle runs a
/// project per package.
pub fn siblings() -> Vec<(&'static str, String)> {
    vec![
        ("lower/Cargo.toml", member("lower", "")),
        (
            "lower/src/lib.rs",
            "pub fn shared() -> u32 { 4 }\n".to_string(),
        ),
        (
            "upper/Cargo.toml",
            member(
                "upper",
                "\n[dependencies]\nlower = { path = \"../lower\" }\n",
            ),
        ),
        (
            "upper/src/lib.rs",
            "pub fn uses_shared() -> u32 { lower::shared() }\n".to_string(),
        ),
    ]
}

/// An owned fixture as `build_rs_oracle` takes it.
pub fn borrowed<'a>(files: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    files
        .iter()
        .map(|(rel, src)| (*rel, src.as_str()))
        .collect()
}

/// `blanked`: the source with one item's lines emptied, as a splice overlay
/// is; the lines stay, so every diagnostic below keeps its number.
pub fn blanked(source: &str, head: &str, span: usize) -> String {
    let mut lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with(head))
        .expect("the item head is in the source");
    for line in lines.iter_mut().skip(at).take(span) {
        *line = "";
    }
    lines.join("\n") + "\n"
}
