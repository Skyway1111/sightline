"""#41 catalog entry `nested-same-collection`."""


def nested_slow(xs):
    dup = 0
    for a in xs:
        for b in xs:  # sightline-ok: 41
            if a == b:
                dup += 1
    return dup


def nested_fast(xs):
    from collections import Counter

    return sum(c * c for c in Counter(xs).values())
