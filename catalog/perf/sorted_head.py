"""#41 catalog entry `sorted-head`.

n-element groups, ten of them (the loop is the shape's hot condition): the
sort pays n log n per group for one extreme, min pays n.
"""


def sorted_head_slow(groups):
    out = []
    for xs in groups:
        out.append(sorted(xs)[0])  # sightline-ok: 41
    return out


def sorted_head_fast(groups):
    out = []
    for xs in groups:
        out.append(min(xs))
    return out
