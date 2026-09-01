"""#12 catalog entry `clamp`: exemplar pair, its domain projection and the
near miss.

The entry is equivalent only on `lo <= hi`, so the projection wrapper is
applied to BOTH sides: the proven claim is on-domain equivalence.
"""


def clamp_naive(x: int, lo: int, hi: int) -> int:  # sightline-ok: 12, 14
    if x < lo:
        return lo
    elif x > hi:
        return hi
    return x


def clamp_idiom(x: int, lo: int, hi: int) -> int:
    return min(max(x, lo), hi)


def clamp_naive_on_domain(x: int, lo: int, hi: int) -> int:  # sightline-ok: 11
    lo, hi = min(lo, hi), max(lo, hi)
    return clamp_naive(x, lo, hi)


def clamp_idiom_on_domain(x: int, lo: int, hi: int) -> int:  # sightline-ok: 11
    lo, hi = min(lo, hi), max(lo, hi)
    return clamp_idiom(x, lo, hi)


NEG, POS = -1, 1


def sign_near_miss(x: int) -> int:  # names returned, but against constants
    if x < 0:
        return NEG
    elif x > 0:
        return POS
    return x
