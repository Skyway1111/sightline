//! `tests/rules/test_trust.py:test_method_tail_identity_under_cross_file_override`:
//! #33's tail verdict must not depend on which build mode ran.

use camino::Utf8Path;
use indexmap::IndexSet;
use sightline_core::config::Config;
use sightline_core::findings::{Rel, Sink};
use sightline_core::walk;
use sightline_py_facts::build::build_facts;
use sightline_py_provers::Provers;
use sightline_py_rules::RULES;
use sightline_testkit::make_repo;

/// A method tail CHA-resolves in a single-file build but goes ambiguous when
/// another file overrides it (gate identity).
#[test]
fn method_tail_identity_under_cross_file_override() {
    let dir = make_repo(&[
        (
            "m.py",
            concat!(
                "from typing import NoReturn\n",
                "class A:\n",
                "    def die(self) -> NoReturn:\n        raise SystemExit(1)\n",
                "    def get(self, x) -> int:\n",
                "        if x:\n            return 1\n        self.die()\n",
            ),
        ),
        (
            "other.py",
            concat!(
                "from typing import NoReturn\n",
                "from m import A\n",
                "class B(A):\n",
                "    def die(self) -> NoReturn:\n        raise SystemExit(2)\n",
            ),
        ),
    ]);
    let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
    let config = Config::new();
    let listing = walk::discover(root, &config);

    let m_findings = |only: Option<&IndexSet<Rel>>| {
        let built = build_facts(root, &config, &listing, only);
        let facts = built.borrow_dependent();
        let provers = Provers::bare(facts);
        let rule = RULES
            .iter()
            .find(|r| r.record.id == "33")
            .expect("#33 is registered");
        let mut sink = Sink::new();
        (rule.run)(facts, &provers, &mut sink);
        let mut rows: Vec<(u32, String)> = sink
            .0
            .into_iter()
            .filter(|f| &*f.site.rel == "m.py")
            .map(|f| (f.site.line, f.cause))
            .collect();
        rows.sort();
        rows
    };

    let only: IndexSet<Rel> = [Rel::from("m.py")].into_iter().collect();
    assert_eq!(m_findings(Some(&only)), m_findings(None));
}
