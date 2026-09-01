"""#41 catalog entry `open-in-loop`, and the temp file its bench opens."""

import tempfile
from pathlib import Path


def open_slow(path, n):
    total = 0
    for _ in range(n):
        with open(path) as f:  # sightline-ok: 41
            total += len(f.readline())
    return total


def open_fast(path, n):
    total = 0
    with open(path) as f:
        for _ in range(n):
            f.seek(0)
            total += len(f.readline())
    return total


def _file_setup(n):
    path = Path(tempfile.mkdtemp()) / "lines.txt"
    path.write_text("hello line\n" * 5)
    return (path, n)
