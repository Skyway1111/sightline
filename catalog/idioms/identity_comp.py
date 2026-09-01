"""#12 node idiom `identity-comp`: the exemplar the matched node sits in,
and the `set` arm's own pair.

A node-level entry has no near miss: the matched node is the shape.
"""

from typing import List


def identity_comp_naive(xs: List[int]) -> List[int]:  # sightline-ok: 10
    return [x for x in xs]  # sightline-ok: 12


def identity_comp_idiom(xs: List[int]) -> List[int]:
    return list(xs)


def identity_setcomp_naive(xs: List[int]) -> "set[int]":  # sightline-ok: 10
    return {x for x in xs}  # sightline-ok: 12


def identity_setcomp_idiom(xs: List[int]) -> "set[int]":  # sightline-ok: 10, 13
    return set(xs)
