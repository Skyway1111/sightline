//! The closed-world verdict (port of `provers/closed_world.py`'s
//! `CwVerdict`), shared because both languages' closed-world provers answer
//! in it.

use indexmap::IndexSet;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CwVerdict {
    pub passed: bool,
    /// the first escape in the prover's own order, when not passed
    pub reason: Option<String>,
    /// every escape that holds
    pub reasons: IndexSet<String>,
}

impl CwVerdict {
    pub fn passed() -> CwVerdict {
        CwVerdict {
            passed: true,
            reason: None,
            reasons: IndexSet::new(),
        }
    }

    /// The escapes in the order the prover found them; the first is the
    /// reason a report names.
    pub fn escaped<I, S>(reasons: I) -> CwVerdict
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let reasons: IndexSet<String> = reasons.into_iter().map(Into::into).collect();
        CwVerdict {
            passed: false,
            reason: reasons.first().cloned(),
            reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reason_is_the_first_escape_and_the_set_keeps_its_order() {
        let v = CwVerdict::escaped(["reflection", "star-import", "reflection"]);
        assert!(!v.passed);
        assert_eq!(v.reason.as_deref(), Some("reflection"));
        assert_eq!(
            v.reasons.iter().collect::<Vec<_>>(),
            ["reflection", "star-import"]
        );

        let ok = CwVerdict::passed();
        assert!(ok.passed && ok.reason.is_none() && ok.reasons.is_empty());
    }
}
