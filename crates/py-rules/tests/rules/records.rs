//! #57 dead-key: a key every closed producer writes that no closed sink
//! reads. Each pair: the positive fires, its
//! near-miss twin stays silent.

use std::collections::BTreeSet;

use sightline_core::findings::{Finding, Tier};
use sightline_testkit::run_rule;

const PRODUCER: &str = "def build():\n    return {'id': 1, 'name': 'x', 'size': 3}\n";
const CONSUMER: &str = "def show(rec):\n    return rec['id'] + rec['name']\n";

fn causes(found: &[Finding]) -> Vec<&str> {
    found.iter().map(|f| f.cause.as_str()).collect()
}

fn module(tail: &str) -> String {
    format!("{PRODUCER}{CONSUMER}{tail}")
}

#[test]
fn fires_when_every_sink_is_closed() {
    let src = module("def main():\n    r = build()\n    return show(r), r.get('id')\n");
    let found = run_rule("57", &[("m.py", &src)]);
    assert_eq!(causes(&found), ["dead-key:m.build:size"]);
    let f = &found[0];
    assert_eq!(&*f.site.symbol, "m.build");
    assert_eq!(f.tier(), Tier::Indexed);
    assert!(f.message.contains("'size'"));
    assert!(f.message.contains("{id, name, size}"));
    assert!(f.message.contains("read: {id, name}"));
    let premises = match &f.evidence {
        sightline_core::findings::Evidence::Wp { premises } => {
            premises.iter().cloned().collect::<BTreeSet<_>>()
        }
        other => panic!("#57 holds whole-program evidence, not {other:?}"),
    };
    assert_eq!(
        premises,
        BTreeSet::from([
            "producer m.build".to_string(),
            "sink m.main.r".to_string(),
            "sink m.show.rec".to_string(),
        ])
    );
}

/// One returned result reopens the contract: every key may be read.
#[test]
fn silent_on_any_open_sink() {
    let src = module("def main():\n    return show(build())\ndef leak():\n    return build()\n");
    assert!(run_rule("57", &[("m.py", &src)]).is_empty());
}

/// The producer is reached by value: a caller the graph never sees.
#[test]
fn silent_when_the_world_is_open() {
    let src = module("def main():\n    return show(build())\nHOOK = build\n");
    assert!(run_rule("57", &[("m.py", &src)]).is_empty());
}

/// The PoC's two false positives were fixture builders mirroring a real
/// schema; a prod producer read only by a test still fires.
#[test]
fn silent_for_a_producer_on_a_test_path() {
    let inside = module("def test_it():\n    assert show(build())\n");
    assert!(run_rule("57", &[("tests/test_m.py", &inside)]).is_empty());
    let test_side =
        format!("from m import build\n{CONSUMER}def test_it():\n    assert show(build())\n");
    let found = run_rule("57", &[("m.py", PRODUCER), ("tests/test_m.py", &test_side)]);
    assert_eq!(causes(&found), ["dead-key:m.build:size"]);
}

#[test]
fn a_key_read_by_any_sink_is_live() {
    let src = module(concat!(
        "def other(rec):\n    return 'size' in rec\n",
        "def main():\n    return show(build()), other(build())\n",
    ));
    assert!(run_rule("57", &[("m.py", &src)]).is_empty());
}

#[test]
fn only_keys_on_every_path_are_reported() {
    let found = run_rule(
        "57",
        &[(
            "m.py",
            concat!(
                "def build(flag):\n",
                "    if flag:\n",
                "        return {'id': 1, 'name': 'x', 'size': 3}\n",
                "    return {'id': 1, 'name': 'x', 'kind': 'y', 'extra': 0}\n",
                "def show(rec):\n    return rec['id']\n",
                "def main():\n    return show(build(True))\n",
            ),
        )],
    );
    assert_eq!(causes(&found), ["dead-key:m.build:name"]);
}
