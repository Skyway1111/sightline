"""The planted non-win `--self-test` adds: both sides are the fast shape, so
the ratio cannot clear 2x. A planted pair that proves is a machinery gap."""

from strconcat import _parts  # noqa: F401  (the bench setup names it)


def planted_slow(parts):
    return "".join(parts)


def planted_fast(parts):
    return "".join(parts)
