"""#12 node idiom `bool-ternary`: the exemplar the matched node sits in, and
the negated arm's own pair."""


def bool_ternary_naive(x: int) -> bool:
    return True if x else False  # sightline-ok: 12


def bool_ternary_idiom(x: int) -> bool:
    return bool(x)


def bool_ternary_neg_naive(x: int) -> bool:
    return False if x else True  # sightline-ok: 12


def bool_ternary_neg_idiom(x: int) -> bool:
    return not x
