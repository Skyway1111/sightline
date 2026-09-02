//! For each closed-world escape condition, the closed world fails with the
//! named reason and the effects summary reports unknown rather than clean -
//! except `framework-base`, which opens the caller set and not the body (a
//! library dispatches to the hook; what the hook does is still what it
//! wrote). No test here checks that #4 and #5 emit no findings over the same
//! table.

use sightline_testkit::{ESCAPE_FIXTURES, build};

#[test]
fn escape_named_and_effects_unknown_not_clean() {
    for fixture in &ESCAPE_FIXTURES {
        let (_dir, stack) = build(fixture.files);
        let facts = stack.facts();
        let verdict = stack
            .provers
            .closed_world(facts)
            .verdict(fixture.symbol)
            .clone();
        assert!(!verdict.passed, "{}", fixture.reason);
        assert_eq!(verdict.reason.as_deref(), Some(fixture.reason));

        let eff = &stack.provers.effects(facts)[fixture.symbol];
        // an escaped symbol is unknown unless only its callers are
        assert_eq!(eff.unknown, fixture.unknown, "{}", fixture.reason);
        assert_eq!(eff.clean(), !fixture.unknown, "{}", fixture.reason);
    }
}
