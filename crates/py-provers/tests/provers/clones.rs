//! `provers/clones.py`: the three clone populations, and the shape
//! differential the `clones` layer rests on.

use serde_json::{Value, json};
use sightline_py_provers::clones::*;
use sightline_testkit::build;

fn mined(files: &[(&str, &str)]) -> (tempfile::TempDir, Value) {
    let (dir, stack) = build(files);
    let doc = dump(stack.facts(), &stack.provers).expect("the layer answers");
    (dir, doc)
}

/// Two bodies differing only in names are one shape (#11's function
/// groups), and a body under the node floor is not worth a name.
#[test]
fn a_renamed_copy_of_a_body_is_one_function_group() {
    let body = |args: &str, names: &str| {
        format!(
            "def {args}(a, b):\n    total = 0\n    for x in a:\n        total += x\n    for y in b:\n        total -= y\n    if total > 10:\n        total = 10\n    return total, {names}\n"
        )
    };
    let (_dir, doc) = mined(&[(
        "m.py",
        &format!("{}{}", body("first", "a"), body("second", "b")),
    )]);
    let groups = doc["functions"].as_array().expect("a list of groups");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].as_array().expect("members").len(), 2);
}

#[test]
fn a_short_body_is_no_group() {
    let (_dir, doc) = mined(&[("m.py", "def f():\n    return 1\ndef g():\n    return 2\n")]);
    assert_eq!(doc["functions"], json!([]));
    assert_eq!(doc["blocks"], json!([]));
}

#[test]
fn foreign_roots_hold_the_names_a_third_party_import_binds() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/util.py", "X = 1\n"),
        (
            "m.py",
            "import os.path\nimport numpy as np\nfrom pkg import util\n",
        ),
    ]);
    let facts = stack.facts();
    let roots = foreign_roots(facts, &facts.modules["m"]);
    assert!(roots.contains("os") && roots.contains("np"));
    assert!(roots.contains("self") && roots.contains("cls"));
    // an internal import is repo knowledge, not a foreign root
    assert!(!roots.contains("util"));
}

/// The blind normalization and the node count of every function body
/// statement of a corpus tree, one TSV row each, against
/// `sightline-phase3/scratch/py-provers-b/py_shapes.py`'s rows from the
/// Python tool. The `clones` layer shows equality classes; this shows the
/// text they were hashed from.
///
/// `SIGHTLINE_SHAPES_ROOT=<tree> SIGHTLINE_SHAPES_CONFIG=<toml>
/// SIGHTLINE_SHAPES_OUT=<tsv> cargo test -p sightline-py-provers --test
/// clones -- --ignored shapes_differential`
#[test]
#[ignore = "reads a corpus tree named by SIGHTLINE_SHAPES_ROOT"]
fn shapes_differential() {
    use camino::{Utf8Path, Utf8PathBuf};
    use sightline_core::config::load_config;
    use sightline_core::walk;
    use sightline_py_facts::astutil::fn_body;
    use sightline_py_facts::cn::Cn;
    use sightline_py_provers::comments::body_of;

    // a probe a reader parameterises, so `check --slow`'s `--ignored` stage
    // passes over it rather than failing on an environment it never sets
    let Ok(named) = std::env::var("SIGHTLINE_SHAPES_ROOT") else {
        eprintln!("skipped: SIGHTLINE_SHAPES_ROOT is unset");
        return;
    };
    let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is unset"));
    let root = Utf8PathBuf::from(named);
    let config_path = std::env::var("SIGHTLINE_SHAPES_CONFIG").ok();
    let config = load_config(&root, config_path.as_deref().map(Utf8Path::new));
    let listing = walk::discover(&root, &config);
    let built = sightline_py_facts::build::build_facts(&root, &config, &listing, None);
    let facts = built.borrow_dependent();

    let shapes = Shapes::default();
    let mut rows: Vec<String> = Vec::new();
    for sym in iter_functions(facts) {
        let module = &facts.modules[&sym.module];
        let Some(body) = body_of(module, sym.node).map(fn_body) else {
            continue;
        };
        for (at, st) in body.iter().enumerate() {
            let node = Cn::Stmt(st);
            rows.push(format!(
                "{}\t{}\t{at}\t{}\t{}",
                module.rel,
                sym.qname,
                shapes.size(node, module),
                shapes.dump(node, module)
            ));
        }
    }
    assert!(!rows.is_empty(), "no function body under {root}");
    std::fs::write(var("SIGHTLINE_SHAPES_OUT"), rows.join("\n") + "\n")
        .expect("the differential output path");
    println!("{} statements", rows.len());
}
