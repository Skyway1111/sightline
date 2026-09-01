"""#41 catalog entry `materialized-short-circuit`.

n-element groups, ten of them: the loop is the shape's hot condition.
"""


def shortcircuit_slow(groups):
    hits = 0
    for xs in groups:
        if any([x % 3 == 0 for x in xs]):  # sightline-ok: 41
            hits += 1
    return hits


def shortcircuit_fast(groups):
    hits = 0
    for xs in groups:
        if any(x % 3 == 0 for x in xs):
            hits += 1
    return hits
