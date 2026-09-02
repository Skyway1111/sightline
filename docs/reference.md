# Reference

Every verb, flag, config key and exit code of the `sightline` binary.
[README.md](../README.md) is the how-to. [architecture.md](../architecture.md)
explains how a finding is produced.

## Verbs

| Verb | What it does | Facts | Exits |
| --- | --- | --- | --- |
| `audit ROOT` | Ranked report with a provenance header | Repo-wide | 0 |
| `gate ROOT` | Blocking check over the files a change touched | One file per changed file | 0, or 1 on a block |
| `gate ROOT --full` | Blocking check over the whole tree, for CI | Repo-wide | 0, or 1 on a block |
| `baseline ROOT` | Writes `.sightline-baseline.json` | Repo-wide | 0 |
| `fix ROOT` | Unified diff of verified fixes. It never writes to the tree | Repo-wide | 0 |
| `facts ROOT QNAME` | What the provers hold about one symbol or module | Repo-wide | 0 |
| `explain RULE` | One rule's record and its measured precision, or why a retired id was cut | None | 0 |
| `explain` | Every rule the binary runs, one line each, with its slug, language, family, posture, tier and judged precision | None | 0 |
| `debug dump ROOT` | One JSON document per pipeline layer, for reading what a stage holds | Repo-wide | 0 |

`ROOT` is a directory. `RULE` is an id (`23`) or a slug (`dead-symbols`).

## Flags

`--threads N` is global and sets the worker pool. The default is every core.

`--quiet` is global and drops the oracle's pass lines from stderr, for a hook
or a CI job. Findings, notes and errors print either way.

`--config PATH` takes the config table from `PATH` instead of the tree. Use it
for a checkout you cannot write to. Every verb that reads a repository accepts
it, except `explain`.

| Verb | Flag | Effect |
| --- | --- | --- |
| `audit` | `--json` | The report as one JSON document |
| `audit` | `--sarif` | SARIF 2.1.0, which GitHub code scanning accepts. It conflicts with `--json` |
| `audit` | `--all` | Ignore the baseline and report every finding |
| `audit` | `--paths PATH...` | Report only findings under these paths. Facts stay repo-wide |
| `audit` | `--rules SPEC` | Run only these rules. Ids or slugs, comma-separated |
| `audit` | `--profile JSON` | Write this audit's per-pass walls to `JSON` |
| `gate` | `--files FILE...` | Gate these files. The default is the working-tree diff against HEAD |
| `gate` | `--since REF` | Also gate files changed since the merge base with `REF` |
| `gate` | `--full` | Run the whole audit pipeline. It conflicts with `--files` and `--since` |
| `fix` | `--out PATH` | Write the diff to `PATH` instead of stdout |
| `fix` | `--rules SPEC` | Emit fixes for these rules only |
| `baseline` | `--prune` | Drop baseline keys no finding claims any more |

`gate` takes no `--rules`: a gate run with fewer rules is a false pass.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The run finished. For `gate`, nothing blocked |
| 1 | `gate` found a blocking finding |
| 2 | The command line or the repository is wrong, or the run failed |
| 3 | Sightline hit a bug and stopped. The panic prints above a line naming the verb and where to report it |

## Config keys

Sightline reads `[tool.sightline]` from `pyproject.toml` in the root, and from
`sightline.toml` when the root has no `pyproject.toml`. A missing file is an
empty table, which is a valid config.

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `excludes` | list of strings | `[]` | Directories the walk never enters, on top of the built-in list below |
| `rules-off` | list of ids or slugs | `[]` | Rules this repository does not run |
| `oracle` | bool | `true` | Setting it to `false` turns off the type checker. Every oracle-backed finding then goes silent, and the header says so |
| `python-env` | string | unset | The Python environment whose packages the type checker resolves. Auto-detected when unset |
| `hot-roots` | list of qnames | `[]` | Seeds for the hot set family P (#41) prices. A `*` matches one path segment |
| `published` | bool | unset | Overrides what the packaging metadata says this repository publishes. `true` means a library, whose callers are downstream, so no in-repo caller set is complete |

`[tool.sightline.rust-toolchain]` holds one key, `cargo`, naming the cargo
version the Rust oracle expects on PATH. A version off the pin is a note in the
provenance header, never a failure.

The walk always skips dot-directories and these names: `__pycache__`, `venv`,
`node_modules`, `site-packages`, `build`, `dist`, `target`. A config may exclude
vendored, environment and generated directories. Excluding source to move a
number is out of contract.

Sightline matches an `excludes` entry against the whole relative path with
Python's `fnmatch`, which lowercases both the entry and the path on Windows and
neither on any other platform. An entry whose case differs from the paths it
targets therefore excludes them on Windows and not on Linux or macOS. Spell an
entry in the case its paths use.

## Postures

A rule's posture is the axis `gate` blocks on. It is declared on the rule and
read in no other place. `sightline explain <id>` prints it.

| Posture | `gate` behavior | Baseline |
| --- | --- | --- |
| GATE | Blocks wherever it runs | Never written |
| RATCHET | Blocks what is new against the baseline | Written |
| REPORT | Never blocks | Never written |

No rule holds GATE. It is what a rule earns from a judged round, and the two
that held it were measured instead: #44 moved to RATCHET at 0.585 and #31 was
retired at 0.073 (`data/retired.toml`). So what `gate` blocks on today is
RATCHET against the baseline, and a tree with a committed baseline blocks only
on what a change adds.

Performance (#41) and complexity (#23) are REPORT. Both are a receipt, never a
gate.

## Suppression

A marker names rule ids or slugs, comma-separated. In source, spell it in the
file's comment syntax:

```
# sightline-ok: 11 - an enum's match table is its own name
// sightline-ok: dead-symbols
```

In a `.md` or `.rst` file, spell it in HTML:

```
<!-- sightline-ok: 32 -->
```

A marker on a line of its own applies to the next line. A marker after code
applies to its own line. Ids are numeric and permanent, and a retired id stays
reserved; `sightline explain <id>` prints why a retired id was cut.

## Where Sightline writes

Inside a repository, Sightline writes only where you ask it to. `baseline ROOT`
writes `.sightline-baseline.json` at the root. `fix --out PATH` and `audit
--profile JSON` write the path you name. No other verb writes into a tree.

Outside a repository, the Rust oracle builds the audited tree and points cargo
at a build directory of its own. The base is
`%LOCALAPPDATA%\sightline\cargo-target` on Windows and
`~/.cache/sightline/cargo-target` elsewhere. Under the base sits one directory
per audited root, named by the first 12 hex digits of the SHA-1 of that root's
absolute path. A root whose crates live under sub-roots gets one subdirectory
per sub-root under that, named by the sub-root's relative path with each `/`
replaced by `-`.

Nothing prunes those directories. Each one holds a full cargo build of one
tree, and the base keeps one per root you have audited until you delete it.

To find the directory for one root, hash the root's absolute path. On Linux:

```
printf %s "$(cd ROOT && pwd)" | sha1sum | cut -c1-12
```

macOS spells that command `shasum -a 1`. On Windows the hashed string is the path in the platform's own spelling, with
backslashes:

```
$p = (Resolve-Path ROOT).Path
$h = [System.Security.Cryptography.SHA1]::HashData([Text.Encoding]::UTF8.GetBytes($p))
(($h | ForEach-Object { $_.ToString('x2') }) -join '').Substring(0, 12)
```

To clear every build Sightline has made, delete the base directory. The next
audit of any root builds it again.

`SIGHTLINE_CARGO_TARGET` replaces the whole path, the per-root directory
included. Every root audited while it is set builds into the single directory
it names. That is how two checkouts of one repository share a warm build, and
it is why two unrelated roots sharing it make cargo rebuild between audits.

While the Rust index loads, Sightline points `TMP` and `TEMP` at a temporary
directory of its own, because the proc-macro server copies every proc-macro
dynamic library into that directory and holds the copies open. Sightline sweeps
the directory when the load closes.

## Portability

**One platform is deterministic.** Two audits of one tree on one machine are
identical byte for byte, at one thread and at every core.

**A baseline written on one platform holds on another.**
`.sightline-baseline.json` keys every count as `<rule>|<symbol qname>`. Neither
half of that key names a path or a platform, so one tree ratchets the same way
wherever it is audited.

**The report's order is the same on every platform.** The total order that
sorts findings ends on the path relative to the root, then the line, the
column, and the rule id. The relative path is spelled with `/` on every
platform. A rule that reports one of several sites picks that site by a total
order too, the lowest qualified name or the first position in document order,
and neither reads the order a directory was listed in.

**The walk order itself is per platform.** The walk sorts each directory's
children case-insensitively on Windows and by byte everywhere else, which is
what `sorted(Path.iterdir())` gives in Python. That sets the order the facts
are built in. It does not set which findings exist, what they say, or the key
they ratchet under.

## Known limitations

**The corpus gate does not run in CI.** `cargo xtask check` reads the six
corpus repositories as clones under `../sightline-corpus/`, or the directory
`SIGHTLINE_CORPUS_ROOT` names, of the urls `crates/xtask/corpus.toml` lists,
at the pins it records, with a `.venv` in each Python root. CI does not clone
or provision them; it runs format, clippy, the unit tests and a build of the
binary, so run the gate locally before you push.
