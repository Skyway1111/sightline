"""Times one #41 exemplar pair and prints its two walls.

`cargo xtask perf-catalog` spawns this once per entry:

    python _bench.py <module> <slow> <fast> <setup-expr> <n> <repeats>

The setup expression is evaluated in the module's own namespace with `n`
bound, and yields the argument tuple both sides are called with. Output is
one line, `<slow secs> <fast secs> <True|False>`, the last field saying
whether the two sides returned equal results.
"""

import importlib
import sys
import timeit


def main(argv):
    module_name, slow_name, fast_name, setup_expr, n, repeats = argv
    module = importlib.import_module(module_name)
    args = eval(setup_expr, vars(module), {"n": int(n)})  # noqa: S307
    slow = getattr(module, slow_name)
    fast = getattr(module, fast_name)
    equal = slow(*args) == fast(*args)
    repeat = int(repeats)
    slow_s = min(timeit.repeat(lambda: slow(*args), number=1, repeat=repeat))
    fast_s = min(timeit.repeat(lambda: fast(*args), number=1, repeat=repeat))
    print(f"{slow_s!r} {fast_s!r} {equal}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
