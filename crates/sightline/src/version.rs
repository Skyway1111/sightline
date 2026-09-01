//! `--version`: the crate version, the fork rev and the ra_ap version
//! (decision 15). Each pin has one home in a manifest and one copy here; the
//! tests below read the manifests and pin the two equal.

/// The `rev` every fork dependency of the workspace `Cargo.toml` declares.
const FORK_REV: &str = "284831cb43bb167d149b23f0e49bcae015c4d183";

/// The version every `ra_ap_*` dependency of `crates/rs-provers` declares.
const RA_AP_VERSION: &str = "0.0.328";

/// The one line `sightline --version` prints after the binary name. clap takes
/// a `&'static str`, so the formatted line lives in a `OnceLock`.
pub fn long() -> &'static str {
    static LONG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    LONG.get_or_init(|| {
        format!(
            "{} (ty-unnecessary {FORK_REV}, ra_ap {RA_AP_VERSION})",
            env!("CARGO_PKG_VERSION")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{FORK_REV, RA_AP_VERSION, long};

    fn manifest(rel: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// Values between `field = "` and the next quote, one per matching line.
    fn pins<'a>(text: &'a str, marker: &str, field: &str) -> Vec<&'a str> {
        text.lines()
            .filter(|line| line.contains(marker))
            .map(|line| {
                line.split(field)
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .unwrap_or("")
            })
            .collect()
    }

    #[test]
    fn fork_rev_matches_the_workspace_manifest() {
        let text = manifest("../../Cargo.toml");
        let revs = pins(
            &text,
            "git = \"https://github.com/Skyway1111/ty-unnecessary\"",
            "rev = \"",
        );
        assert_eq!(revs.len(), 12, "fork dependencies: {revs:?}");
        assert!(revs.iter().all(|rev| *rev == FORK_REV), "{revs:?}");
    }

    #[test]
    fn ra_ap_version_matches_the_rs_provers_manifest() {
        let text = manifest("../rs-provers/Cargo.toml");
        let versions = pins(&text, "package = \"ra_ap", "version = \"=");
        assert_eq!(versions.len(), 6, "ra_ap dependencies: {versions:?}");
        assert!(versions.iter().all(|v| *v == RA_AP_VERSION), "{versions:?}");
    }

    #[test]
    fn long_names_all_three_pins() {
        let line = long();
        assert!(line.starts_with(env!("CARGO_PKG_VERSION")), "{line}");
        assert!(line.contains(FORK_REV), "{line}");
        assert!(line.contains(RA_AP_VERSION), "{line}");
    }
}
