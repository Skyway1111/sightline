"""#12 catalog entry `binary-search`: exemplar pair and near miss.

Proof input for `cargo xtask catalog`.
"""

import bisect
from typing import List


def binary_search_naive(xs: List[int], x: int) -> int:  # sightline-ok: 12, 10
    lo = 0
    hi = len(xs)
    while lo < hi:
        mid = (lo + hi) // 2
        if xs[mid] < x:
            lo = mid + 1
        else:
            hi = mid
    return lo


def binary_search_idiom(xs: List[int], x: int) -> int:
    return bisect.bisect_left(xs, x)


def sift_up_near_miss(heap: List[int], pos: int) -> None:  # one bound in the while
    while pos > 0:
        parent = (pos - 1) // 2
        if heap[pos] < heap[parent]:
            heap[pos], heap[parent] = heap[parent], heap[pos]
            pos = parent
        else:
            break
