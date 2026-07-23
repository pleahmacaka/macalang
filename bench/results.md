# Benchmarks — Maca vs C vs JS vs Python

Recursion-heavy integer kernels. Median execution time (ms); lower is
better. `×C` is the slowdown relative to C. Maca is built through the
`maca` CLI (Maca → C → `cc -O2`); C is `cc -O2`; Python is CPython 3;
JS is Node. See `bench/run.py`.

| kernel | Maca | C | JS (node) | Python |
|---|---|---|---|---|
| fib | 168.0 ms (×1.0) | 166.6 ms (×1.0) | 975.7 ms (×5.9) | 11194.1 ms (×67.2) |
| tak | 65.1 ms (×1.1) | 58.6 ms (×1.0) | 193.4 ms (×3.3) | 1680.1 ms (×28.6) |
| ackermann | 18.6 ms (×0.9) | 19.7 ms (×1.0) | 234.0 ms (×11.9) | 4476.6 ms (×227.8) |

_Params: fib(40), tak(32,16,8), ackermann(3,10). Host: Linux x86_64._
