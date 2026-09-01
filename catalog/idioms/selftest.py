"""Planted bugs for `cargo xtask catalog --self-test`: the machinery must
refute both, the raw pair and the projected one. A planted pair that proves
is a machinery gap, not a pass."""

from clamp import clamp_naive


def selftest_broken_naive(x: int, lo: int, hi: int) -> int:
    return clamp_naive(x, lo, hi)


def selftest_broken_idiom(x: int, lo: int, hi: int) -> int:
    return max(min(x, lo), hi)


def selftest_projected_naive(x: int, lo: int, hi: int) -> int:  # sightline-ok: 11
    lo, hi = min(lo, hi), max(lo, hi)
    return clamp_naive(x, lo, hi)


def selftest_projected_idiom(x: int, lo: int, hi: int) -> int:  # sightline-ok: 11
    lo, hi = min(lo, hi), max(lo, hi)
    return max(min(x, lo), hi)
