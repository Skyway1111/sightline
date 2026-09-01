"""#41 catalog entry `filter-scan`.

n rows (a dict keyed by id, scanned by f) times n probes: the re-keyed index
pays once, the scan pays per probe.
"""

from types import SimpleNamespace  # noqa: F401  (the bench setup names it)


def filter_scan_slow(rows, keys):
    out = []
    for key in keys:
        for r in rows.values():
            if r.f != key:  # sightline-ok: 41
                continue
            out.append(r)
    return out


def filter_scan_fast(rows, keys):
    by_f = {}
    for r in rows.values():
        by_f.setdefault(r.f, []).append(r)
    out = []
    for key in keys:
        out.extend(by_f.get(key, ()))
    return out
