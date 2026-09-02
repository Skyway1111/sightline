//! For every finding-emitting rule, its fixture and a twin that differs only
//! by an unrelated extraction (a helper split out in a side module, away from
//! the flagged structure) yield identical findings. The tool never rewards or
//! punishes extraction count.

use camino::Utf8Path;
use sightline_py_provers::oracle::Oracle;
use sightline_testkit::{build, run_rule, run_rule_on};

/// The unrelated module: identical meaning, the twin has a helper
/// extracted. Its docstring keeps #29 quiet and its shapes trigger nothing.
const UNRELATED_PLAIN: &str =
    "\"\"\"Unrelated helper module.\"\"\"\ndef util(x):\n    return (x * 3) + 1\n";
const UNRELATED_SPLIT: &str = "\"\"\"Unrelated helper module.\"\"\"\ndef util(x):\n    return _mul(x) + 1\ndef _mul(x):\n    return x * 3\n";

/// Rules whose fixture needs the checker to fire.
const ORACLE_RULES: [&str; 3] = ["2", "5", "10"];

/// rule id -> the fixture files whose findings must survive an unrelated split.
const POS: [(&str, &[(&str, &str)]); 35] = [
    (
        "1",
        &[(
            "api.py",
            "from typing import Any\ndef load(cfg: Any):\n    return cfg\n",
        )],
    ),
    (
        "2",
        &[(
            "m.py",
            "def g(x: str) -> bool:\n    return isinstance(x, str)\n",
        )],
    ),
    (
        "3",
        &[(
            "m.py",
            "def f(items: list):\n    if items:\n        items.sort()\n",
        )],
    ),
    (
        "5",
        &[(
            "m.py",
            "def _scale(nums):\n    out = []\n    for n in nums:\n        out.append(n * 2)\n    return out\ndef use1() -> list:\n    return _scale([1, 2])\ndef use2() -> list:\n    return _scale([3])\n",
        )],
    ),
    (
        "6",
        &[
            ("state.py", "cache = {}\n"),
            (
                "m.py",
                "from state import cache\ndef get_price(k):\n    cache[k] = 1\n    return cache[k]\n",
            ),
        ],
    ),
    (
        "7",
        &[(
            "m.py",
            "def read(conn):\n    \"\"\"The caller must call connect() before this.\"\"\"\n    return conn.recv()\n",
        )],
    ),
    (
        "9",
        &[(
            "state.py",
            "active = False\ndef start():\n    global active\n    active = True\ndef stop():\n    global active\n    active = False\ndef reset():\n    global active\n    active = False\n",
        )],
    ),
    (
        "10",
        &[(
            "m.py",
            "def total(xs: list[int]) -> int:\n    acc = 0\n    for x in xs:\n        acc += x\n    return acc\ndef use() -> int:\n    return total([1])\n",
        )],
    ),
    (
        "11",
        &[
            (
                "a.py",
                "def score_users(rows):\n    total = 0\n    for row in rows:\n        if row.active:\n            total += row.value * row.weight\n    return total\n",
            ),
            (
                "b.py",
                "def score_items(entries):\n    acc = 0\n    for item in entries:\n        if item.active:\n            acc += item.value * item.weight\n    return acc\n",
            ),
        ],
    ),
    (
        "12",
        &[(
            "m.py",
            "def clamp(x, lo, hi):\n    if x < lo:\n        return lo\n    elif x > hi:\n        return hi\n    return x\n",
        )],
    ),
    (
        "14",
        &[(
            "m.py",
            "def connect(host: str, port: int, timeout: int):\n    pass\ndef ping(host: str, port: int, timeout: int):\n    pass\ndef trace(host: str, port: int, timeout: int):\n    pass\n",
        )],
    ),
    (
        "18",
        &[(
            "m.py",
            "def run(data):\n    # Step 1: load\n    x = []\n    for row in data:\n        x.append(row)\n    # Step 2: emit\n    return [i for i in x]\n",
        )],
    ),
    (
        "20",
        &[(
            "m.py",
            "def a(rows):\n    return sorted(rows, key=lambda r: (r.date, r.priority))\ndef b(rows):\n    return max(rows, key=lambda x: (x.date, x.priority))\ndef c(rows):\n    return min(rows, key=lambda q: (q.date, q.priority))\n",
        )],
    ),
    (
        "21",
        &[(
            "m.py",
            "class Job:\n    def run(self):\n        return self._state.get('phase') == 'ready'\n    def stop(self):\n        return not self._state.get('phase') == 'ready'\n    def poll(self):\n        return 2 if self._state.get('phase') == 'ready' else 3\n",
        )],
    ),
    (
        "23",
        &[(
            "m.py",
            "def f(xs):\n    for a in xs:\n        if a:\n            for b in a:\n                if b:\n                    if b > 1:\n                        return b\n    return 0\n",
        )],
    ),
    (
        "24",
        &[(
            "m.py",
            "def dispatch(obj, kind):\n    return getattr(obj, f'handle_{kind}')\n",
        )],
    ),
    (
        "26",
        &[(
            "m.py",
            "RAW = ['x', 'y']\nFEATURES = sorted(p.upper() for p in RAW)\n",
        )],
    ),
    (
        "27",
        &[
            (
                "big.py",
                "def hot():\n    return 1\ndef filler_0():\n    return 0\ndef filler_1():\n    return 1\ndef filler_2():\n    return 2\ndef filler_3():\n    return 3\ndef filler_4():\n    return 4\ndef filler_5():\n    return 5\ndef filler_6():\n    return 6\ndef filler_7():\n    return 7\ndef filler_8():\n    return 8\ndef filler_9():\n    return 9\ndef filler_10():\n    return 10\ndef filler_11():\n    return 11\ndef filler_12():\n    return 12\ndef filler_13():\n    return 13\ndef filler_14():\n    return 14\ndef filler_15():\n    return 15\ndef filler_16():\n    return 16\ndef filler_17():\n    return 17\ndef filler_18():\n    return 18\ndef filler_19():\n    return 19\ndef filler_20():\n    return 20\ndef filler_21():\n    return 21\ndef filler_22():\n    return 22\ndef filler_23():\n    return 23\ndef filler_24():\n    return 24\ndef filler_25():\n    return 25\ndef filler_26():\n    return 26\ndef filler_27():\n    return 27\ndef filler_28():\n    return 28\ndef filler_29():\n    return 29\ndef filler_30():\n    return 30\ndef filler_31():\n    return 31\ndef filler_32():\n    return 32\ndef filler_33():\n    return 33\ndef filler_34():\n    return 34\ndef filler_35():\n    return 35\ndef filler_36():\n    return 36\ndef filler_37():\n    return 37\ndef filler_38():\n    return 38\ndef filler_39():\n    return 39\ndef filler_40():\n    return 40\ndef filler_41():\n    return 41\ndef filler_42():\n    return 42\ndef filler_43():\n    return 43\ndef filler_44():\n    return 44\ndef filler_45():\n    return 45\ndef filler_46():\n    return 46\ndef filler_47():\n    return 47\ndef filler_48():\n    return 48\ndef filler_49():\n    return 49\ndef filler_50():\n    return 50\ndef filler_51():\n    return 51\ndef filler_52():\n    return 52\ndef filler_53():\n    return 53\ndef filler_54():\n    return 54\ndef filler_55():\n    return 55\ndef filler_56():\n    return 56\ndef filler_57():\n    return 57\ndef filler_58():\n    return 58\ndef filler_59():\n    return 59\ndef filler_60():\n    return 60\ndef filler_61():\n    return 61\ndef filler_62():\n    return 62\ndef filler_63():\n    return 63\ndef filler_64():\n    return 64\ndef filler_65():\n    return 65\ndef filler_66():\n    return 66\ndef filler_67():\n    return 67\ndef filler_68():\n    return 68\ndef filler_69():\n    return 69\ndef filler_70():\n    return 70\ndef filler_71():\n    return 71\ndef filler_72():\n    return 72\ndef filler_73():\n    return 73\ndef filler_74():\n    return 74\ndef filler_75():\n    return 75\ndef filler_76():\n    return 76\ndef filler_77():\n    return 77\ndef filler_78():\n    return 78\ndef filler_79():\n    return 79\ndef filler_80():\n    return 80\ndef filler_81():\n    return 81\ndef filler_82():\n    return 82\ndef filler_83():\n    return 83\ndef filler_84():\n    return 84\ndef filler_85():\n    return 85\ndef filler_86():\n    return 86\ndef filler_87():\n    return 87\ndef filler_88():\n    return 88\ndef filler_89():\n    return 89\ndef filler_90():\n    return 90\ndef filler_91():\n    return 91\ndef filler_92():\n    return 92\ndef filler_93():\n    return 93\ndef filler_94():\n    return 94\ndef filler_95():\n    return 95\ndef filler_96():\n    return 96\ndef filler_97():\n    return 97\ndef filler_98():\n    return 98\ndef filler_99():\n    return 99\ndef filler_100():\n    return 100\ndef filler_101():\n    return 101\ndef filler_102():\n    return 102\ndef filler_103():\n    return 103\ndef filler_104():\n    return 104\ndef filler_105():\n    return 105\ndef filler_106():\n    return 106\ndef filler_107():\n    return 107\ndef filler_108():\n    return 108\ndef filler_109():\n    return 109\ndef filler_110():\n    return 110\ndef filler_111():\n    return 111\ndef filler_112():\n    return 112\ndef filler_113():\n    return 113\ndef filler_114():\n    return 114\ndef filler_115():\n    return 115\ndef filler_116():\n    return 116\ndef filler_117():\n    return 117\ndef filler_118():\n    return 118\ndef filler_119():\n    return 119\ndef filler_120():\n    return 120\ndef filler_121():\n    return 121\ndef filler_122():\n    return 122\ndef filler_123():\n    return 123\ndef filler_124():\n    return 124\ndef filler_125():\n    return 125\ndef filler_126():\n    return 126\ndef filler_127():\n    return 127\ndef filler_128():\n    return 128\ndef filler_129():\n    return 129\ndef filler_130():\n    return 130\ndef filler_131():\n    return 131\ndef filler_132():\n    return 132\ndef filler_133():\n    return 133\ndef filler_134():\n    return 134\ndef filler_135():\n    return 135\ndef filler_136():\n    return 136\ndef filler_137():\n    return 137\ndef filler_138():\n    return 138\ndef filler_139():\n    return 139\ndef filler_140():\n    return 140\ndef filler_141():\n    return 141\ndef filler_142():\n    return 142\ndef filler_143():\n    return 143\ndef filler_144():\n    return 144\ndef filler_145():\n    return 145\ndef filler_146():\n    return 146\ndef filler_147():\n    return 147\ndef filler_148():\n    return 148\ndef filler_149():\n    return 149\ndef filler_150():\n    return 150\ndef filler_151():\n    return 151\ndef filler_152():\n    return 152\ndef filler_153():\n    return 153\ndef filler_154():\n    return 154\ndef filler_155():\n    return 155\ndef filler_156():\n    return 156\ndef filler_157():\n    return 157\ndef filler_158():\n    return 158\ndef filler_159():\n    return 159\ndef filler_160():\n    return 160\ndef filler_161():\n    return 161\ndef filler_162():\n    return 162\ndef filler_163():\n    return 163\ndef filler_164():\n    return 164\ndef filler_165():\n    return 165\ndef filler_166():\n    return 166\ndef filler_167():\n    return 167\ndef filler_168():\n    return 168\ndef filler_169():\n    return 169\ndef filler_170():\n    return 170\ndef filler_171():\n    return 171\ndef filler_172():\n    return 172\ndef filler_173():\n    return 173\ndef filler_174():\n    return 174\ndef filler_175():\n    return 175\ndef filler_176():\n    return 176\ndef filler_177():\n    return 177\ndef filler_178():\n    return 178\ndef filler_179():\n    return 179\ndef filler_180():\n    return 180\ndef filler_181():\n    return 181\ndef filler_182():\n    return 182\ndef filler_183():\n    return 183\ndef filler_184():\n    return 184\ndef filler_185():\n    return 185\ndef filler_186():\n    return 186\ndef filler_187():\n    return 187\ndef filler_188():\n    return 188\ndef filler_189():\n    return 189\ndef filler_190():\n    return 190\ndef filler_191():\n    return 191\ndef filler_192():\n    return 192\ndef filler_193():\n    return 193\ndef filler_194():\n    return 194\ndef filler_195():\n    return 195\ndef filler_196():\n    return 196\ndef filler_197():\n    return 197\ndef filler_198():\n    return 198\ndef filler_199():\n    return 199\ndef filler_200():\n    return 200\ndef filler_201():\n    return 201\ndef filler_202():\n    return 202\ndef filler_203():\n    return 203\ndef filler_204():\n    return 204\ndef filler_205():\n    return 205\ndef filler_206():\n    return 206\ndef filler_207():\n    return 207\ndef filler_208():\n    return 208\ndef filler_209():\n    return 209\ndef filler_210():\n    return 210\ndef filler_211():\n    return 211\ndef filler_212():\n    return 212\ndef filler_213():\n    return 213\ndef filler_214():\n    return 214\ndef filler_215():\n    return 215\ndef filler_216():\n    return 216\ndef filler_217():\n    return 217\ndef filler_218():\n    return 218\ndef filler_219():\n    return 219\ndef filler_220():\n    return 220\ndef filler_221():\n    return 221\ndef filler_222():\n    return 222\ndef filler_223():\n    return 223\ndef filler_224():\n    return 224\ndef filler_225():\n    return 225\ndef filler_226():\n    return 226\ndef filler_227():\n    return 227\ndef filler_228():\n    return 228\ndef filler_229():\n    return 229\ndef filler_230():\n    return 230\ndef filler_231():\n    return 231\ndef filler_232():\n    return 232\ndef filler_233():\n    return 233\ndef filler_234():\n    return 234\ndef filler_235():\n    return 235\ndef filler_236():\n    return 236\ndef filler_237():\n    return 237\ndef filler_238():\n    return 238\ndef filler_239():\n    return 239\ndef filler_240():\n    return 240\ndef filler_241():\n    return 241\ndef filler_242():\n    return 242\ndef filler_243():\n    return 243\ndef filler_244():\n    return 244\ndef filler_245():\n    return 245\ndef filler_246():\n    return 246\ndef filler_247():\n    return 247\ndef filler_248():\n    return 248\ndef filler_249():\n    return 249\n",
            ),
            (
                "user_0.py",
                "from big import hot\ndef u0():\n    return hot()\n",
            ),
            (
                "user_1.py",
                "from big import hot\ndef u1():\n    return hot()\n",
            ),
            (
                "user_2.py",
                "from big import hot\ndef u2():\n    return hot()\n",
            ),
        ],
    ),
    (
        "29",
        &[(
            "m.py",
            "def f0():\n    return 0\ndef f1():\n    return 1\ndef f2():\n    return 2\ndef f3():\n    return 3\ndef f4():\n    return 4\ndef f5():\n    return 5\ndef f6():\n    return 6\ndef f7():\n    return 7\ndef f8():\n    return 8\ndef f9():\n    return 9\ndef f10():\n    return 10\ndef f11():\n    return 11\ndef f12():\n    return 12\ndef f13():\n    return 13\ndef f14():\n    return 14\ndef f15():\n    return 15\ndef f16():\n    return 16\ndef f17():\n    return 17\ndef f18():\n    return 18\ndef f19():\n    return 19\ndef f20():\n    return 20\ndef f21():\n    return 21\ndef f22():\n    return 22\ndef f23():\n    return 23\ndef f24():\n    return 24\ndef f25():\n    return 25\ndef f26():\n    return 26\ndef f27():\n    return 27\ndef f28():\n    return 28\ndef f29():\n    return 29\ndef f30():\n    return 30\ndef f31():\n    return 31\ndef f32():\n    return 32\ndef f33():\n    return 33\ndef f34():\n    return 34\ndef f35():\n    return 35\ndef f36():\n    return 36\ndef f37():\n    return 37\ndef f38():\n    return 38\ndef f39():\n    return 39\ndef f40():\n    return 40\ndef f41():\n    return 41\ndef f42():\n    return 42\ndef f43():\n    return 43\ndef f44():\n    return 44\ndef f45():\n    return 45\ndef f46():\n    return 46\ndef f47():\n    return 47\ndef f48():\n    return 48\ndef f49():\n    return 49\ndef f50():\n    return 50\ndef f51():\n    return 51\ndef f52():\n    return 52\ndef f53():\n    return 53\ndef f54():\n    return 54\ndef f55():\n    return 55\ndef f56():\n    return 56\ndef f57():\n    return 57\ndef f58():\n    return 58\ndef f59():\n    return 59\ndef f60():\n    return 60\ndef f61():\n    return 61\ndef f62():\n    return 62\ndef f63():\n    return 63\ndef f64():\n    return 64\ndef f65():\n    return 65\ndef f66():\n    return 66\ndef f67():\n    return 67\ndef f68():\n    return 68\ndef f69():\n    return 69\ndef f70():\n    return 70\ndef f71():\n    return 71\ndef f72():\n    return 72\ndef f73():\n    return 73\ndef f74():\n    return 74\n",
        )],
    ),
    (
        "32",
        &[
            (
                "m.py",
                "def _dead():\n    return 1\ndef used():\n    return 2\n",
            ),
            ("n.py", "from m import used\nused()\n"),
        ],
    ),
    (
        "33",
        &[("m.py", "def f(x) -> int:\n    if x:\n        return 1\n")],
    ),
    (
        "34",
        &[(
            "m.py",
            "def run(x):\n    # y = x * 2\n    # if y > 3:\n    #     return y\n    return x\n",
        )],
    ),
    (
        "35",
        &[
            ("a.py", "import b\ndef fa():\n    return b\n"),
            ("b.py", "import a\ndef fb():\n    return a\n"),
        ],
    ),
    (
        "36",
        &[(
            "m.py",
            "def f(x):\n    a = x.go()  # type: ignore\n    b = x.run()  # type: ignore\n    return a + b  # pyright: ignore\n",
        )],
    ),
    (
        "37",
        &[(
            "m.py",
            "def render(data, mode):\n    return (data, mode)\ndef a(d):\n    return render(d, 'fast')\ndef b(d):\n    return render(d, 'fast')\ndef c(d):\n    return render(d, 'fast')\n",
        )],
    ),
    (
        "38",
        &[
            ("da.py", "ENDPOINT = 'api/v2/rows'\n"),
            ("db.py", "ROWS_URL = 'api/v2/rows'\n"),
            ("dc.py", "PATH = 'api/v2/rows'\n"),
        ],
    ),
    (
        "39",
        &[(
            "m.py",
            "def handle(user):\n    # user rows\n    user_rows = user\n    return user_rows\n",
        )],
    ),
    ("40", &[("m.py", "def is_ready(x) -> str:\n    return x\n")]),
    (
        "41",
        &[(
            "m.py",
            "import copy\ndef apply(rows, template):\n    \"\"\"Hot path: called per request.\"\"\"\n    for row in rows:\n        cfg = copy.deepcopy(template)\n        row.use(cfg)\n",
        )],
    ),
    ("42", &[("tests/test_t.py", "def test_bare():\n    pass\n")]),
    (
        "44",
        &[("tests/test_t.py", "def test_const():\n    assert True\n")],
    ),
    (
        "47",
        &[(
            "tests/test_t.py",
            "import time\ndef test_slow():\n    time.sleep(1)\n    assert 1 + 1 == 2\n",
        )],
    ),
    (
        "48",
        &[(
            "m.py",
            "def _tidy(rows):\n    return [r for r in rows if r]\ndef load(rows):\n    return _tidy(rows)\n",
        )],
    ),
    (
        "49",
        &[(
            "m.py",
            "def f(x, acc=[]):\n    acc.append(x)\n    return acc\n",
        )],
    ),
    (
        "56",
        &[
            ("m.py", "def _only_tested():\n    return 4\n"),
            (
                "tests/test_m.py",
                "from m import _only_tested\ndef test_it():\n    assert _only_tested()\n",
            ),
        ],
    ),
];

/// `_run_rule`: the rule over the fixture plus the unrelated module, its
/// own findings sorted by (rule, cause). The unrelated module's findings
/// are its own shape's (#48 flags the split's `_mul` by design).
fn own_findings(rule_id: &str, files: &[(&str, &str)], unrelated: &str) -> Vec<(String, String)> {
    let mut all: Vec<(&str, &str)> = files.to_vec();
    all.push(("zz_unrelated.py", unrelated));
    let mut out = if ORACLE_RULES.contains(&rule_id) {
        let (dir, mut stack) = build(&all);
        let root = Utf8Path::from_path(dir.path()).expect("a utf-8 temp path");
        let import_roots = stack.facts().import_roots.clone();
        stack.provers.oracle =
            Some(Oracle::new(root, &[], &import_roots, None).expect("an in-process checker"));
        let found = run_rule_on(rule_id, &stack);
        stack.provers.close();
        found
    } else {
        run_rule(rule_id, &all)
    }
    .into_iter()
    .filter(|f| &*f.site.rel != "zz_unrelated.py")
    .map(|f| (f.rule.to_string(), f.cause))
    .collect::<Vec<_>>();
    out.sort();
    out
}

#[test]
fn an_unrelated_extraction_changes_nothing() {
    for (rule_id, files) in POS {
        let pos = own_findings(rule_id, files, UNRELATED_PLAIN);
        let twin = own_findings(rule_id, files, UNRELATED_SPLIT);
        assert!(
            !pos.is_empty(),
            "#{rule_id}: the positive fixture must fire"
        );
        assert_eq!(
            pos, twin,
            "#{rule_id}: an unrelated extraction changed the findings"
        );
    }
}

// --- split-fix rules: the intended split discharges, an unrelated one not ---

const SPLIT_FIX_18: [(&str, &str); 1] = [(
    "m.py",
    "def load(data):\n    return list(data)\ndef run(data):\n    x = load(data)\n    return x\n",
)];

const SPLIT_BADFIX_18: [(&str, &str); 1] = [(
    "m.py",
    "def _identity(x):\n    return x\ndef run(data):\n    # Step 1: load\n    x = []\n    for row in _identity(data):\n        x.append(row)\n    # Step 2: emit\n    return [i for i in x]\n",
)];

#[test]
fn the_intended_split_discharges_rule_18() {
    assert!(run_rule("18", &SPLIT_FIX_18).is_empty());
}

#[test]
fn an_unrelated_split_keeps_the_rule_18_finding() {
    assert!(!run_rule("18", &SPLIT_BADFIX_18).is_empty());
}
