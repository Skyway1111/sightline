// file-length-ok: one test file per facts layer, sharing one fixture builder
//! The pass-A half of the build: modules, qnames, symbols, classes, the
//! node index and what the tree declares about itself. The tests whose
//! whole subject is refs and call sites live in `resolve.rs`.

use sightline_core::config::Config;
use sightline_py_facts::kinds::Kind;
use sightline_py_facts::model::{RepoFacts, Resolution};
use sightline_py_facts::module::Module;
use sightline_testkit::{build, build_with};

/// Enclosing symbols of the call sites facts alone resolves to `qname`.
fn resolved_callers<'a>(facts: &'a RepoFacts<'_>, qname: &str) -> Vec<&'a str> {
    facts
        .call_sites
        .iter()
        .filter(|c| c.resolution == Resolution::Resolved && c.target.as_deref() == Some(qname))
        .map(|c| &*c.enclosing)
        .collect()
}

/// The refs pointing at `target`, in the order pass B recorded them.
fn refs_to<'a>(facts: &'a RepoFacts<'_>, target: &str) -> Vec<&'a sightline_py_facts::model::Ref> {
    facts
        .refs_to
        .get(target)
        .map(|ids| ids.iter().map(|i| &facts.refs[*i as usize]).collect())
        .unwrap_or_default()
}

fn lines_of(facts: &RepoFacts<'_>, module: &str, kinds: &[Kind], scope: Option<&str>) -> Vec<u32> {
    let m = &facts.modules[module];
    m.nodes(kinds, scope, false)
        .into_iter()
        .map(|n| m.line_of(n))
        .collect()
}

fn qnames<'a>(facts: &'a RepoFacts<'_>) -> Vec<&'a str> {
    facts.modules.keys().map(|q| &**q).collect()
}

fn module<'a, 't>(facts: &'a RepoFacts<'t>, qname: &str) -> &'a Module<'t> {
    &facts.modules[qname]
}

#[test]
fn module_qnames_use_import_names() {
    let (_dir, stack) = build(&[
        ("src/pkg/__init__.py", ""),
        ("src/pkg/mod.py", "X = 1\n"),
        ("tool.py", "Y = 2\n"),
    ]);
    let facts = stack.facts();
    for q in ["pkg.mod", "pkg", "tool"] {
        assert!(
            qnames(facts).contains(&q),
            "missing {q} in {:?}",
            qnames(facts)
        );
    }
    assert_eq!(&*module(facts, "pkg.mod").rel, "src/pkg/mod.py");
}

#[test]
fn namespace_package_dirs_count_above_a_regular_package() {
    // PEP 420: a bare dir contributes its name only above a regular
    // package, never the root or a `src` directly under it; a bare dir
    // with no package below it still yields the stem. The import graph over
    // these modules is `py-provers`' to assert, not this test's.
    let (_dir, stack) = build(&[
        ("src/ns/pkg/__init__.py", ""),
        ("src/ns/pkg/mod.py", "def f():\n    pass\n"),
        (
            "scripts/tool.py",
            "from ns.pkg import mod\n\n\ndef run():\n    mod.f()\n",
        ),
    ]);
    let facts = stack.facts();
    for q in ["ns.pkg.mod", "ns.pkg", "tool"] {
        assert!(qnames(facts).contains(&q), "missing {q}");
    }
    assert_eq!(resolved_callers(facts, "ns.pkg.mod.f"), ["tool.run"]);
    let modules: Vec<&str> = refs_to(facts, "ns.pkg.mod.f")
        .iter()
        .map(|r| &*r.module)
        .collect();
    assert_eq!(modules, ["tool"]);
}

#[test]
fn a_loose_module_beside_a_package_shares_its_namespace() {
    // ComfyUI's src/models/dit_3b/ has no __init__.py: nadit.py was named
    // bare `nadit` while nablocks/ walked up to the dotted home, so the
    // relative import resolved against nothing
    let (_dir, stack) = build(&[
        ("src/__init__.py", ""),
        (
            "src/models/dit_3b/nablocks/__init__.py",
            "def get_nablock():\n    return 1\n",
        ),
        (
            "src/models/dit_3b/nadit.py",
            "from .nablocks import get_nablock\n\ndef run():\n    return get_nablock()\n",
        ),
        ("scripts/tool.py", "X = 1\n"),
    ]);
    let facts = stack.facts();
    assert!(qnames(facts).contains(&"src.models.dit_3b.nadit"));
    assert!(!qnames(facts).contains(&"nadit"));
    // a loose script dir stays loose
    assert!(qnames(facts).contains(&"tool"));
    assert_eq!(
        module(facts, "src.models.dit_3b.nadit")
            .bindings
            .get("get_nablock")
            .map(|q| &**q),
        Some("src.models.dit_3b.nablocks.get_nablock"),
    );
    assert_eq!(
        resolved_callers(facts, "src.models.dit_3b.nablocks.get_nablock"),
        ["src.models.dit_3b.nadit.run"]
    );
}

#[test]
fn symbols_qualified_and_visibility() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/m.py",
            concat!(
                "__all__ = ['f']\n",
                "def f():\n    pass\n",
                "def g():\n    pass\n",
                "def _h():\n    pass\n",
                "class C:\n",
                "    def meth(self):\n        pass\n",
            ),
        ),
    ]);
    let s = &stack.facts().symbols;
    assert!(s["pkg.m.f"].is_public);
    // `__all__` is present and g is not in it
    assert!(!s["pkg.m.g"].is_public);
    assert!(!s["pkg.m._h"].is_public);
    assert_eq!(s["pkg.m.C.meth"].kind, "method");
    assert_eq!(s["pkg.m.C.meth"].parent.as_deref(), Some("pkg.m.C"));
}

#[test]
fn class_hierarchy() {
    let (_dir, stack) = build(&[(
        "m.py",
        "class Base:\n    pass\nclass Child(Base):\n    pass\n",
    )]);
    let facts = stack.facts();
    assert_eq!(
        facts.classes["m.Child"]
            .bases
            .iter()
            .map(|b| &**b)
            .collect::<Vec<_>>(),
        ["m.Base"]
    );
    assert_eq!(
        facts.classes["m.Base"]
            .subclasses
            .iter()
            .map(|b| &**b)
            .collect::<Vec<_>>(),
        ["m.Child"]
    );
}

#[test]
fn an_external_base_keeps_the_text_the_module_spells() {
    let (_dir, stack) = build(&[(
        "m.py",
        "import enum\nclass A(enum.Enum):\n    pass\nclass B(Unbound[int]):\n    pass\n",
    )]);
    let facts = stack.facts();
    assert_eq!(facts.classes["m.A"].external_bases, ["enum.Enum"]);
    // an unbound root falls back to the unparsed base expression
    assert_eq!(facts.classes["m.B"].external_bases, ["Unbound[int]"]);
}

#[test]
fn excludes_and_syntax_errors() {
    let config = Config {
        excludes: vec!["vendor".to_string()],
        ..Config::new()
    };
    let (_dir, stack) = build_with(
        &[
            ("keep.py", "X = 1\n"),
            ("vendor/skip.py", "Y = 2\n"),
            (".venv/lib/junk.py", "Z = 3\n"),
            ("broken.py", "def (\n"),
        ],
        config,
    );
    let facts = stack.facts();
    assert!(qnames(facts).contains(&"keep"));
    assert!(!qnames(facts).contains(&"vendor.skip"));
    assert!(!qnames(facts).iter().any(|q| q.contains("junk")));
    assert!(!qnames(facts).contains(&"broken"));
    assert!(facts.errors.iter().any(|e| e.contains("broken.py")));
}

#[test]
fn enclosing_symbol_lookup() {
    let (_dir, stack) = build(&[("m.py", "def f():\n    x = 1\n    return x\n")]);
    let facts = stack.facts();
    let m = module(facts, "m");
    let assign = m.nodes(&[Kind::Assign], None, false)[0];
    let def = m.nodes(&[Kind::FunctionDef], None, false)[0];
    assert_eq!(&*facts.enclosing(m, assign), "m.f");
    assert_eq!(&*facts.enclosing(m, def), "m.f");
    assert_eq!(
        facts.enclosing_symbol(m, assign).map(|s| &*s.qname),
        Some("m.f")
    );
}

#[test]
fn the_parent_map_walks_a_chain_prefix() {
    let (_dir, stack) = build(&[
        ("state.py", "cache = {}\n"),
        (
            "user.py",
            "import state\ndef f():\n    state.cache.update({1: 2})\n",
        ),
    ]);
    let facts = stack.facts();
    let m = module(facts, "user");
    let name = m
        .nodes(&[Kind::Name], None, false)
        .into_iter()
        .find(|n| m.line_of(*n) == 3)
        .expect("the `state` name of the chain");
    let attr = m.parent_of(name).expect("the `cache` attribute");
    assert_eq!(m.nodes[attr as usize].kind(), Kind::Attribute);
    let outer = m.parent_of(attr).expect("the `update` attribute");
    assert_eq!(m.nodes[outer as usize].kind(), Kind::Attribute);
    assert_eq!(
        m.nodes[m.parent_of(outer).unwrap() as usize].kind(),
        Kind::Call
    );
}

#[test]
fn star_imports_and_dynamic_all_recorded() {
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        ("pkg/a.py", "def f():\n    pass\n"),
        ("m.py", "from pkg.a import *\n__all__ = ['x'] + ['y']\n"),
    ]);
    let facts = stack.facts();
    assert!(module(facts, "m").dynamic_all);
    assert!(!module(facts, "pkg.a").dynamic_all);
    // `*` binds no name; the `__all__` assignment binds its own
    assert!(!module(facts, "m").bindings.contains_key("f"));
    assert!(module(facts, "m").bindings.contains_key("__all__"));
}

#[test]
fn module_node_index_by_type_and_scope() {
    // one traversal feeds every reader: nodes by exact class, document
    // order, optionally restricted to one symbol's own scope
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import os\n",
            "def f(x):\n",
            "    for i in x:\n",
            "        pass\n",
            "    def inner():\n",
            "        for j in x:\n",
            "            pass\n",
            "    return [y for y in x]\n",
            "class C:\n",
            "    def m(self):\n",
            "        while True:\n",
            "            break\n",
        ),
    )]);
    let facts = stack.facts();
    let m = module(facts, "m");
    let kinds: Vec<Kind> = m
        .nodes(&[Kind::For, Kind::While], None, false)
        .into_iter()
        .map(|n| m.nodes[n as usize].kind())
        .collect();
    assert_eq!(kinds, [Kind::For, Kind::For, Kind::While]);
    assert_eq!(lines_of(facts, "m", &[Kind::For], None), [3, 6]);
    assert_eq!(lines_of(facts, "m", &[Kind::For], Some("m.f")), [3]);
    assert_eq!(lines_of(facts, "m", &[Kind::For], Some("m.f.inner")), [6]);
    assert!(m.nodes(&[Kind::For], Some("m.C.m"), false).is_empty());
    assert_eq!(lines_of(facts, "m", &[Kind::Import], Some("m")), [1]);
    assert_eq!(m.nodes(&[Kind::Module], None, false), [0]);
    let while_node = m.nodes(&[Kind::While], None, false)[0];
    assert_eq!(&*facts.enclosing(m, while_node), "m.C.m");
    // `nested` adds the descendant scopes, in sorted-key order
    assert_eq!(
        m.nodes(&[Kind::For], Some("m.f"), true)
            .into_iter()
            .map(|n| m.line_of(n))
            .collect::<Vec<_>>(),
        [3, 6]
    );
}

#[test]
fn module_comments_are_tokenized_once() {
    let (_dir, stack) = build(&[("m.py", "x = 1  # a\n# b\ny = 2\n")]);
    let m = module(stack.facts(), "m");
    assert_eq!(
        m.comments.iter().map(|c| &*c.text).collect::<Vec<_>>(),
        ["# a", "# b"]
    );
    assert_eq!(m.comments[0].line, 1);
    assert_eq!(m.comments[0].col, 7);
    // only the second comment owns its line
    assert_eq!(
        m.standalone_comments.iter().copied().collect::<Vec<_>>(),
        [2]
    );
}

#[test]
fn lines_count_newlines_alone_like_the_ast() {
    // a form feed and U+2028 are line breaks to `str.splitlines`, not to
    // the tokenizer or the AST: every line index after one still agrees
    let src = "def a():\n    return 1\n\x0c\ndef f(a):\n    # note \n    return a\n";
    let (_dir, stack) = build(&[("m.py", src)]);
    let facts = stack.facts();
    let m = module(facts, "m");
    assert_eq!(m.lines.len(), 6);
    assert_eq!(m.lines[3], "def f(a):");
    assert_eq!(lines_of(facts, "m", &[Kind::FunctionDef], None), [1, 4]);
}

#[test]
fn a_non_utf8_module_is_read_lossily_and_marked() {
    let dir = sightline_testkit::make_repo(&[]);
    std::fs::write(dir.path().join("m.py"), b"# caf\xe9\nx = 1\n").unwrap();
    std::fs::write(dir.path().join("n.py"), b"y = 2\n").unwrap();
    let root = camino::Utf8Path::from_path(dir.path()).unwrap();
    let config = Config::new();
    let listing = sightline_core::walk::discover(root, &config);
    let built = sightline_py_facts::build::build_facts(root, &config, &listing, None);
    let facts = built.borrow_dependent();
    assert!(facts.modules["m"].lossy);
    assert!(!facts.modules["n"].lossy);
    assert_eq!(facts.modules["m"].lines[0], "# caf\u{fffd}");
}

#[test]
fn property_setter_is_its_own_symbol() {
    // `@x.setter` is a second body under the getter's name: a symbol of
    // its own, never folded into the getter
    let (_dir, stack) = build(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/m.py",
            concat!(
                "class C:\n",
                "    @property\n",
                "    def x(self):\n        return self._x\n",
                "    @x.setter\n",
                "    def x(self, value):\n        self._x = value\n",
            ),
        ),
    ]);
    let facts = stack.facts();
    assert_eq!(facts.symbols["pkg.m.C.x"].lineno, 3);
    assert_eq!(facts.symbols["pkg.m.C.x.setter"].lineno, 6);
    assert_eq!(&*facts.symbols["pkg.m.C.x.setter"].name, "x");
    assert_eq!(&*facts.classes["pkg.m.C"].methods["x"], "pkg.m.C.x");
    let m = module(facts, "pkg.m");
    assert_eq!(m.nodes(&[Kind::Return], Some("pkg.m.C.x"), false).len(), 1);
    assert!(
        m.nodes(&[Kind::Return], Some("pkg.m.C.x.setter"), false)
            .is_empty()
    );
}

#[test]
fn a_src_package_keeps_its_own_name() {
    // `src/__init__.py` makes `src` a package: the repo spells its imports
    // `src.pkg.m`, and the import root stays the tree root
    let (_dir, stack) = build(&[
        ("src/__init__.py", ""),
        ("src/pkg/__init__.py", ""),
        ("src/pkg/m.py", "X = 1\n"),
    ]);
    let facts = stack.facts();
    assert!(qnames(facts).contains(&"src.pkg.m"));
    assert_eq!(facts.import_roots.len(), 1);
    assert_eq!(facts.import_roots[0], facts.root);
}

#[test]
fn a_def_under_a_module_level_block_binds() {
    // `if sys.platform == "win32": def _w(): ...` is the module's symbol
    // already; its loads must resolve too, or a def installed by
    // `other.download = _w` reads as referenced in no other place
    let (_dir, stack) = build(&[(
        "m.py",
        concat!(
            "import sys\nimport other\n",
            "if sys.platform == 'win32':\n",
            "    def _w(url):\n        return url\n",
            "    other.download = _w\n",
            "try:\n    import fast\nexcept ImportError:\n    fast = None\n",
            "def use():\n    return fast\n",
        ),
    )]);
    let facts = stack.facts();
    let m = module(facts, "m");
    assert_eq!(m.bindings.get("_w").map(|q| &**q), Some("m._w"));
    assert!(matches!(
        m.bindings.get("fast").map(|q| &**q),
        Some("fast") | Some("m.fast")
    ));
    assert!(facts.symbols.contains_key("m._w"));
    let kinds: Vec<&str> = refs_to(facts, "m._w")
        .iter()
        .map(|r| r.kind.value())
        .collect();
    assert_eq!(kinds, ["load"]);
}

#[test]
fn a_type_checking_body_binds_nothing() {
    let (_dir, stack) = build(&[(
        "m.py",
        "from typing import TYPE_CHECKING\nif TYPE_CHECKING:\n    import heavy\n",
    )]);
    let m = module(stack.facts(), "m");
    assert!(m.bindings.contains_key("TYPE_CHECKING"));
    assert!(!m.bindings.contains_key("heavy"));
}

mod published {
    //! `facts.published`: the module qnames a distribution in the tree
    //! ships. Empty means an application, every caller of every def of
    //! which is in this tree.

    use super::*;

    const LIB: [(&str, &str); 3] = [
        ("src/mypkg/__init__.py", ""),
        ("src/mypkg/api.py", "def f():\n    return 1\n"),
        ("tests/test_api.py", "def test_f():\n    pass\n"),
    ];
    const DIST: &str =
        "[project]\nname = \"mypkg\"\n\n[build-system]\nrequires = [\"setuptools\"]\n";

    fn lib_with(extra: &[(&str, &str)]) -> Vec<(&'static str, String)> {
        let mut files: Vec<(&'static str, String)> =
            LIB.iter().map(|(r, s)| (*r, s.to_string())).collect();
        for (rel, src) in extra {
            let rel: &'static str = Box::leak(rel.to_string().into_boxed_str());
            files.push((rel, src.to_string()));
        }
        files
    }

    fn published_of(files: &[(&str, String)]) -> Vec<String> {
        let rows: Vec<(&str, &str)> = files.iter().map(|(r, s)| (*r, s.as_str())).collect();
        let (_dir, stack) = build(&rows);
        let mut out: Vec<String> = stack
            .facts()
            .published
            .iter()
            .map(|q| q.to_string())
            .collect();
        out.sort();
        out
    }

    #[test]
    fn setuptools_src_layout() {
        let toml = DIST.to_string()
            + "\n[tool.setuptools]\npackage-dir = {\"\" = \"src\"}\npackages = [\"mypkg\"]\n";
        let files = lib_with(&[("pyproject.toml", &toml)]);
        assert_eq!(published_of(&files), ["mypkg", "mypkg.api"]);
    }

    #[test]
    fn project_metadata_without_a_build_publishes_nothing() {
        // the near-miss twin: `[project]` alone is an app manifest, not a
        // distribution
        let files = lib_with(&[("pyproject.toml", "[project]\nname = \"mypkg\"\n")]);
        assert!(published_of(&files).is_empty());
    }

    #[test]
    fn the_name_is_the_fallback_when_no_backend_declares_packages() {
        let files = lib_with(&[("pyproject.toml", DIST)]);
        assert_eq!(published_of(&files), ["mypkg", "mypkg.api"]);
    }

    #[test]
    fn a_py_typed_marker_is_a_distribution() {
        let files = lib_with(&[
            ("pyproject.toml", "[project]\nname = \"mypkg\"\n"),
            ("src/mypkg/py.typed", ""),
        ]);
        assert_eq!(published_of(&files), ["mypkg", "mypkg.api"]);
    }

    #[test]
    fn hatch_packages() {
        let toml = concat!(
            "[project]\nname = \"other\"\n\n",
            "[build-system]\nrequires = [\"hatchling\"]\n\n",
            "[tool.hatch.build.targets.wheel]\npackages = [\"src/mypkg\"]\n",
        );
        let files = lib_with(&[("pyproject.toml", toml)]);
        assert_eq!(published_of(&files), ["mypkg", "mypkg.api"]);
    }

    /// A poetry tree keeps its metadata under `[tool.poetry]` and packages
    /// what `packages` includes, its own name when it lists none.
    #[test]
    fn poetry_packages_and_the_poetry_name() {
        let listed = concat!(
            "[tool.poetry]\nname = \"other\"\n",
            "packages = [{ include = \"mypkg\", from = \"src\" }]\n\n",
            "[build-system]\nrequires = [\"poetry-core\"]\n",
        );
        let files = lib_with(&[("pyproject.toml", listed)]);
        assert_eq!(published_of(&files), ["mypkg", "mypkg.api"]);

        let named =
            "[tool.poetry]\nname = \"mypkg\"\n\n[build-system]\nrequires = [\"poetry-core\"]\n";
        let files = lib_with(&[("pyproject.toml", named)]);
        assert_eq!(published_of(&files), ["mypkg", "mypkg.api"]);
    }

    #[test]
    fn autodoc_with_members_publishes_the_module() {
        // autoclass names a class, so its module is published; the
        // near-miss twin is the same directive with no `:members:`
        let (_dir, stack) = build(&[
            ("plugins/__init__.py", ""),
            (
                "plugins/extra.py",
                "class Thing:\n    def go(self):\n        return 1\n",
            ),
            ("plugins/hidden.py", "def g():\n    return 2\n"),
            (
                "docs/api.rst",
                concat!(
                    "API\n===\n\n",
                    ".. autoclass:: plugins.extra.Thing\n    :members:\n\n",
                    ".. automodule:: plugins.hidden\n    :undoc-members:\n",
                ),
            ),
            ("pyproject.toml", "[project]\nname = \"app\"\n"),
        ]);
        let facts = stack.facts();
        let mut published: Vec<&str> = facts.published.iter().map(|q| &**q).collect();
        published.sort_unstable();
        assert_eq!(published, ["plugins.extra"]);
        // a .rst is a doc file
        assert_eq!(facts.doc_files["docs/api.rst"][0], "API");
    }

    #[test]
    fn the_config_bit_overrides_the_read_both_ways() {
        let files = lib_with(&[("pyproject.toml", DIST)]);
        let rows: Vec<(&str, &str)> = files.iter().map(|(r, s)| (*r, s.as_str())).collect();
        for (bit, expected) in [(false, Vec::new()), (true, vec!["mypkg", "mypkg.api"])] {
            let config = Config {
                published: Some(bit),
                ..Config::new()
            };
            let (_dir, stack) = build_with(&rows, config);
            let mut got: Vec<&str> = stack.facts().published.iter().map(|q| &**q).collect();
            got.sort_unstable();
            // a test module is no one's API
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn a_private_do_not_upload_classifier_publishes_nothing() {
        // the standard "never leaves this repo" declaration: no index
        // accepts it, so its packages ship to no one
        let private = concat!(
            "[project]\nname = \"mypkg\"\nclassifiers = [\"Private :: Do Not Upload\"]\n\n",
            "[build-system]\nrequires = [\"setuptools\"]\n",
        );
        let (_dir, stack) = build(&[("src/mypkg/__init__.py", ""), ("pyproject.toml", private)]);
        assert!(stack.facts().published.is_empty());

        let open = private.replace("classifiers = [\"Private :: Do Not Upload\"]\n", "");
        let (_dir, stack) = build(&[("src/mypkg/__init__.py", ""), ("pyproject.toml", &open)]);
        let published: Vec<&str> = stack.facts().published.iter().map(|q| &**q).collect();
        assert_eq!(published, ["mypkg"]);
    }
}

mod workspace_members {
    //! A member's own `pyproject.toml` makes its `src/` an import root, so
    //! the qname map stops there and the oracle resolves there.

    use super::*;

    #[test]
    fn a_members_src_is_an_import_root() {
        let (_dir, stack) = build(&[
            (
                "member/pyproject.toml",
                "[project]\nname = \"member\"\n\n[build-system]\nrequires = [\"hatchling\"]\n",
            ),
            ("member/src/member/__init__.py", ""),
            ("member/src/member/core.py", "def run():\n    return 1\n"),
            (
                "app.py",
                "from member.core import run\n\n\ndef main():\n    return run()\n",
            ),
        ]);
        let facts = stack.facts();
        // not member.src.member.core
        assert!(qnames(facts).contains(&"member.core"));
        let mut published: Vec<&str> = facts.published.iter().map(|q| &**q).collect();
        published.sort_unstable();
        assert_eq!(published, ["member", "member.core"]);
        let roots: Vec<&str> = facts
            .import_roots
            .iter()
            .map(|p| p.file_name().unwrap_or(""))
            .collect();
        assert_eq!(roots, ["src", facts.root.file_name().unwrap()]);
        assert_eq!(resolved_callers(facts, "member.core.run"), ["app.main"]);
    }

    #[test]
    fn a_flat_member_is_its_own_root() {
        let (_dir, stack) = build(&[
            ("member/pyproject.toml", "[project]\nname = \"member\"\n"),
            ("member/thing/__init__.py", ""),
            ("member/thing/m.py", "X = 1\n"),
        ]);
        let facts = stack.facts();
        assert!(qnames(facts).contains(&"thing.m"));
        let roots: Vec<&str> = facts
            .import_roots
            .iter()
            .map(|p| p.file_name().unwrap_or(""))
            .collect();
        assert_eq!(roots, ["member", facts.root.file_name().unwrap()]);
    }
}
