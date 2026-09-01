//! Port of `provers/shipping.py`'s contract (REF has no test file of its own
//! for it: #35's assertions live in the rules tests phase 5 ports). Expected
//! values from a probe through REF's own `shipped_subsets`:
//! `sightline-phase3/scratch/py-provers-a/probe_shipping_spend.py`.

use sightline_py_provers::shipping::shipped_subsets;
use sightline_testkit::build;

/// A prod module-scope collection of file names, two or more of which name a
/// module by its path tail, stages a runtime elsewhere. A list with one such
/// name stages nothing, a list of bare names names no file, and a test's list
/// asserts over the tree.
#[test]
fn a_prod_list_of_module_files_is_a_shipped_subset() {
    let (_dir, stack) = build(&[
        ("rofl/__init__.py", ""),
        ("rofl/metadata.py", "X = 1\n"),
        ("rofl/data.py", "Y = 2\n"),
        (
            "rofl/pack.py",
            concat!(
                "_CONTAINER = ['rofl/metadata.py', 'rofl/__init__.py']\n",
                "_ONE = ['rofl/metadata.py', 'notes.txt']\n",
                "_NAMES = ['a', 'b']\n",
            ),
        ),
        (
            "tests/test_pack.py",
            "FILES = ['rofl/metadata.py', 'rofl/data.py']\n",
        ),
    ]);
    let subsets: Vec<Vec<String>> = shipped_subsets(stack.facts())
        .into_iter()
        .map(|s| s.into_iter().map(|q| q.to_string()).collect())
        .collect();

    assert_eq!(subsets, [["rofl", "rofl.metadata"]]);
}
