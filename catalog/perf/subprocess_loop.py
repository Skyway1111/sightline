"""#41 catalog entry `subprocess-in-loop`.

n=20: a process spawn costs about 20 ms wall on Windows, so the default
n=1000 would be a multi-minute bench proving the same per-spawn ratio.
"""

import subprocess
import sys


def subprocess_slow(args):
    for a in args:
        subprocess.run(  # sightline-ok: 41
            [sys.executable, "-c", "pass", a], capture_output=True
        )
    return len(args)


def subprocess_fast(args):
    subprocess.run(
        [sys.executable, "-c", "pass", *args], capture_output=True
    )
    return len(args)
