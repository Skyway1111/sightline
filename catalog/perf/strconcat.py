"""#41 catalog entry `str-concat-in-loop`, and the parts setup it and the
planted pair share."""


def strconcat_slow(parts):
    s = ""
    for p in parts:
        s += p  # sightline-ok: 41
    return s


def strconcat_fast(parts):
    return "".join(parts)


def _parts(n):
    return ([f"part{i};" for i in range(n)],)
