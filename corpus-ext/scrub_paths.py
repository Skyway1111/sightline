#!/usr/bin/env python3
"""Rewrite this machine's absolute paths out of corpus-ext/, in place.

corpus-ext/ is append-only judged evidence (BRIEF.md): this only substitutes
path spellings, never judgments, counts, verdicts, SHAs or repository names.
Run once per machine that has local paths baked into tracked corpus-ext/
files, before those files are published.

The root is derived from git, never hardcoded, so this script itself holds
no machine-specific path:

  git rev-parse --git-common-dir   ->  <claude-code>/sightline/.git
  .parent.parent                   ->  <claude-code>/

Two roots get scrubbed, both slash spellings of each, longest first so a
gauntlet-corpus path never falls through to the coarser substitution:
  <claude-code>/gauntlet-corpus  -> the placeholder, GAUNTLET_CORPUS_ROOT
  <claude-code>                  -> placeholder + "/.." (a path that sits
                                     beside gauntlet-corpus, not under it)
"""

import shutil
import subprocess
import sys
from pathlib import Path

PLACEHOLDER = "<GAUNTLET_CORPUS_ROOT>"
GIT = shutil.which("git")
if GIT is None:
    sys.exit("git not found on PATH")


def claude_code_root() -> Path:
    common_dir = subprocess.run(  # noqa: S603 (fixed args, no untrusted input)
        [GIT, "rev-parse", "--git-common-dir"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    return Path(common_dir).resolve().parent.parent


def substitutions(root: Path) -> list[tuple[str, str]]:
    gauntlet_bs = str(root / "gauntlet-corpus")
    root_bs = str(root)
    return [
        (gauntlet_bs.replace("\\", "/"), PLACEHOLDER),
        (gauntlet_bs, PLACEHOLDER),
        (root_bs.replace("\\", "/"), f"{PLACEHOLDER}/.."),
        (root_bs, f"{PLACEHOLDER}\\.."),
    ]


def main() -> int:
    subs = substitutions(claude_code_root())
    tracked = subprocess.run(  # noqa: S603 (fixed args, no untrusted input)
        [GIT, "ls-files", "corpus-ext"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.splitlines()

    changed_files = 0
    total_hits = 0
    for rel_path in map(Path, tracked):
        if not rel_path.is_file():
            continue
        text = rel_path.read_text(encoding="utf-8", newline="")
        original, hits = text, 0
        for old, new in subs:
            hits += text.count(old)
            text = text.replace(old, new)
        if text != original:
            rel_path.write_text(text, encoding="utf-8", newline="")
            changed_files += 1
            total_hits += hits
            print(f"{rel_path}: {hits} replacement(s)")  # noqa: T201

    print(f"\n{changed_files} file(s) changed, {total_hits} path(s) replaced.")  # noqa: T201
    return 0


if __name__ == "__main__":
    sys.exit(main())
