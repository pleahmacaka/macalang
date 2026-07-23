# Benchmarks — Maca vs C / Rust / Go / JS / Python

Median execution time in **ms** (lower is better); `×N` is the slowdown
vs C. Maca is `maca build` (Maca → C → `cc -O2`); C is `cc -O2`; Rust is
`rustc -O`; Go is `go build`; JS is Node; Python is CPython 3. Every
column computes the same verified result. Reproduce with `python3
bench/run.py`.

### Recursion-bound (call overhead)

Tree/nested recursion — dominated by function-call cost.

| kernel | params | Maca | C | Rust | Go | JS (node) | Python |
|---|---|---|---|---|---|---|---|
| fib | fib(40) | 201.7 (×1.0) | 202.0 (×1.0) | 302.1 (×1.5) | 577.9 (×2.9) | 1175.8 (×5.8) | 13378.2 (×66.2) |
| tak | tak(32,16,8) | 70.2 (×1.0) | 70.6 (×1.0) | 79.7 (×1.1) | 96.2 (×1.4) | 201.9 (×2.9) | 2007.8 (×28.5) |
| ackermann | ack(3,10) | 37.3 (×1.0) | 37.7 (×1.0) | 164.5 (×4.4) | 252.4 (×6.7) | 412.1 (×10.9) | 5336.0 (×141.6) |

### Compute-bound (loops, arrays, float)

Tight loops over arrays and floating point — dominated by the inner loop.

| kernel | params | Maca | C | Rust | Go | JS (node) | Python |
|---|---|---|---|---|---|---|---|
| sieve | primes ≤ 10⁷ | 338.0 (×5.0) | 67.6 (×1.0) | 67.7 (×1.0) | 69.8 (×1.0) | 118.2 (×1.7) | 547.5 (×8.1) |
| mandel | 800×800, ≤1000 it | 605.2 (×1.0) | 605.0 (×1.0) | 584.8 (×1.0) | 602.4 (×1.0) | 663.8 (×1.1) | 16376.1 (×27.1) |
| matmul | 400×400 int | 72.2 (×1.2) | 59.8 (×1.0) | 68.3 (×1.1) | 110.7 (×1.9) | 196.6 (×3.3) | 5512.4 (×92.2) |

On recursion and float/loop work Maca lands right on C (it *is* C, at
`-O2`). `sieve` is the one gap: Maca's only array type is 64-bit `int[]`,
so a sieve over 10⁷ touches ~80 MB where C's `char` array touches ~10 MB —
it's memory-bound, not a codegen deficit (a compact byte array would close
it).

_Host: Linux x86_64, 6.18.5. Times are wall-clock medians including process startup._
