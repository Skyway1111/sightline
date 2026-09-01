"""#12 catalog entry `tolower`: exemplar pair, its ascii projection and the
near miss.

Equivalent on ascii alone, so the projection wrapper is applied to BOTH
sides: the proven claim is on-domain equivalence.
"""


def tolower_naive(s: str) -> str:  # sightline-ok: 12
    out = ""
    for c in s:
        if "A" <= c <= "Z":
            out += chr(ord(c) + 32)
        else:
            out += c
    return out


def tolower_idiom(s: str) -> str:
    return s.lower()


def _ascii(s: str) -> str:
    return "".join(chr(ord(c) % 128) for c in s)


def tolower_naive_on_domain(s: str) -> str:
    return tolower_naive(_ascii(s))


def tolower_idiom_on_domain(s: str) -> str:
    return tolower_idiom(_ascii(s))


def caesar_near_miss(s: str, k: int) -> str:  # shift by k, no case guard
    out = []
    for c in s:
        out.append(chr(ord(c) + k))
    return "".join(out)
