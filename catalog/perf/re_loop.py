"""#41 catalog entry `re-in-loop`."""

import re


def re_slow(lines):
    hits = 0
    for s in lines:
        if re.search(r"[a-z]+\d{2,}", s):  # sightline-ok: 41
            hits += 1
    return hits


def re_fast(lines):
    pat = re.compile(r"[a-z]+\d{2,}")
    hits = 0
    for s in lines:
        if pat.search(s):
            hits += 1
    return hits
