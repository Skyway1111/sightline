"""#41 catalog entry `deepcopy-in-loop`."""

import copy


def deepcopy_slow(rows, template):
    out = 0
    for _r in rows:
        cfg = copy.deepcopy(template)  # sightline-ok: 41
        out += cfg["k"]
    return out


def deepcopy_fast(rows, template):
    cfg = copy.deepcopy(template)
    out = 0
    for _r in rows:
        out += cfg["k"]
    return out
