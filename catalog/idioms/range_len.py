"""#12 node idiom `range-len`: the exemplar the matched node sits in."""

from typing import List


def range_len_naive(xs: List[int]) -> List[int]:  # sightline-ok: 10
    out = []
    for i in range(len(xs)):  # sightline-ok: 12
        out.append(xs[i] + 1)
    return out


def range_len_idiom(xs: List[int]) -> List[int]:
    out = []
    for _i, x in enumerate(xs):
        out.append(x + 1)
    return out
