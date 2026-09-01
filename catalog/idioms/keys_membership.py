"""#12 node idiom `keys-membership`: the exemplar the matched node sits in,
and the `notin` arm's own pair."""

from typing import Dict


def keys_membership_naive(d: Dict[int, int], k: int) -> bool:  # sightline-ok: 10, 14
    return k in d.keys()  # sightline-ok: 12


def keys_membership_idiom(d: Dict[int, int], k: int) -> bool:  # sightline-ok: 10
    return k in d


def keys_notin_naive(d: Dict[int, int], k: int) -> bool:  # sightline-ok: 10
    return k not in d.keys()  # sightline-ok: 12


def keys_notin_idiom(d: Dict[int, int], k: int) -> bool:  # sightline-ok: 10
    return k not in d
