//! `provers/records.py`: which producers close, and where their results flow.

use std::collections::BTreeSet;

use sightline_py_provers::records::{Edge, Records};
use sightline_testkit::build;

const PRODUCER: &str = "def build():\n    return {'id': 1, 'name': 'x', 'size': 3}\n";
const KEYS: [&str; 3] = ["id", "name", "size"];

fn shapes(records: &Records) -> Vec<(&str, Vec<Vec<&str>>)> {
    records
        .produced
        .iter()
        .map(|(q, shapes)| {
            let mut rows: Vec<Vec<&str>> = shapes
                .iter()
                .map(|shape| shape.iter().map(|k| &**k).collect())
                .collect();
            rows.sort();
            (&**q, rows)
        })
        .collect()
}

fn producers(records: &Records) -> Vec<&str> {
    let mut names: Vec<&str> = records.produced.keys().map(|q| &**q).collect();
    names.sort();
    names
}

/// `(sink, name, reads)` as the Python test spells an edge, sorted.
fn sinks(records: &Records) -> Vec<(&str, &str, Option<Vec<&str>>)> {
    let mut rows: Vec<(&str, &str, Option<Vec<&str>>)> = records
        .edges
        .iter()
        .map(|e: &Edge| {
            (
                &*e.sink,
                e.name.as_str(),
                e.reads
                    .as_ref()
                    .map(|r| r.iter().map(|k| &**k).collect::<Vec<&str>>()),
            )
        })
        .collect();
    rows.sort();
    rows
}

fn keys<'a>(names: &[&'a str]) -> Option<Vec<&'a str>> {
    let sorted: BTreeSet<&str> = names.iter().copied().collect();
    Some(sorted.into_iter().collect())
}

#[test]
fn closed_producers_are_literal_on_every_return_path() {
    let source = format!(
        "{PRODUCER}{}",
        concat!(
            "def bound():\n",
            "    rec = {'id': 1, 'name': 'x', 'size': 3}\n    return rec\n",
            "def shapes(flag):\n",
            "    if flag:\n        return {'id': 1, 'name': 'x', 'size': 3}\n",
            "    return {'id': 1, 'name': 'x', 'kind': 'y'}\n",
            "def small():\n    return {'id': 1, 'name': 'x'}\n",
            "def mixed(flag):\n",
            "    if flag:\n        return {'id': 1, 'name': 'x', 'size': 3}\n",
            "    return build()\n",
            "def gen():\n    yield {'id': 1, 'name': 'x', 'size': 3}\n",
        )
    );
    let (_dir, stack) = build(&[("m.py", &source)]);
    let rec = stack.provers.records(stack.facts());
    assert_eq!(
        shapes(rec),
        vec![
            ("m.build", vec![KEYS.to_vec()]),
            ("m.bound", vec![KEYS.to_vec()]),
            ("m.shapes", vec![vec!["id", "kind", "name"], KEYS.to_vec()]),
        ]
    );
}

/// Hole 1643c54: pop/setdefault on the producer's own local change the key
/// set it returns; on a consumer's param they are reads.
#[test]
fn reshaping_the_own_record_opens_the_producer() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def popped():\n",
            "    rec = {'id': 1, 'name': 'x', 'size': 3}\n    rec.pop('size')\n    return rec\n",
            "def defaulted():\n",
            "    rec = {'id': 1, 'name': 'x', 'size': 3}\n",
            "    rec.setdefault('tags', [])\n    return rec\n",
            "def peeked():\n",
            "    rec = {'id': 1, 'name': 'x', 'size': 3}\n",
            "    if rec.get('size'):\n        pass\n    return rec\n",
            "def show(rec):\n    return rec.pop('size'), rec.setdefault('id', 0)\n",
            "def main():\n    return show(peeked())\n",
        ),
    )]);
    let rec = stack.provers.records(stack.facts());
    assert_eq!(producers(rec), vec!["m.peeked"]);
    assert_eq!(sinks(rec), vec![("m.show", "rec", keys(&["size", "id"]))]);
}

/// Hole 1643c54: an implicit `return None` is a return path with no keys.
#[test]
fn a_body_that_can_fall_off_the_end_is_open() {
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "def falls(flag):\n",
            "    if flag:\n        return {'id': 1, 'name': 'x', 'size': 3}\n",
            "def raises(flag):\n",
            "    if flag:\n        return {'id': 1, 'name': 'x', 'size': 3}\n",
            "    raise ValueError(flag)\n",
            "def guarded(flag):\n",
            "    try:\n        return {'id': 1, 'name': 'x', 'size': 3}\n",
            "    except KeyError:\n        return {'id': 0, 'name': '', 'size': 0}\n",
            "def looped(flags):\n",
            "    for flag in flags:\n        return {'id': 1, 'name': 'x', 'size': 3}\n",
        ),
    )]);
    let rec = stack.provers.records(stack.facts());
    assert_eq!(producers(rec), vec!["m.guarded", "m.raises"]);
}

#[test]
fn edges_follow_binders_and_params_and_open_on_escape() {
    let source = format!(
        "{PRODUCER}{}",
        concat!(
            "def show(rec):\n    return rec['id'] + rec.get('name')\n",
            "def main():\n    r = build()\n    show(r)\n    return r['size']\n",
            "def leak():\n    return build()\n",
            "def whole(rec):\n    return list(rec)\n",
            "def forwarded():\n    return whole(build())\n",
            "TOP = build()\n",
        )
    );
    let (_dir, stack) = build(&[("m.py", &source)]);
    let rec = stack.provers.records(stack.facts());
    assert_eq!(
        sinks(rec),
        vec![
            ("m", "", None),
            ("m.leak", "", None),
            ("m.main", "r", keys(&["size"])),
            ("m.show", "rec", keys(&["id", "name"])),
            ("m.whole", "rec", None),
        ]
    );
    let producers: BTreeSet<&str> = rec.edges.iter().map(|e| &*e.producer).collect();
    assert_eq!(producers, BTreeSet::from(["m.build"]));
}
