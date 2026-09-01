"""#12 catalog entry `manual-sum`: exemplar pair and near miss."""

from typing import List


def manual_sum_naive(xs: List[int]) -> int:  # sightline-ok: 12, 10
    total = 0
    for x in xs:
        total += x
    return total


def manual_sum_idiom(xs: List[int]) -> int:
    return sum(xs)


def product_near_miss(xs: List[int]) -> int:  # a fold that is not a sum
    total = 1
    for x in xs:
        total *= x
    return total
