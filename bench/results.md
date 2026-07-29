# Benchmarks — Maca vs C / Rust / Go / JS / Python

Median execution time in **ms** (lower is better); `×N` is the slowdown
vs C. Maca is `maca build` (Maca → C → `cc -O2`); C is `cc -O2`; Rust is
`rustc -O`; Go is `go build`; JS is Node; Python is CPython 3. Every
column computes the same verified result. Reproduce with
`maca run bench/run.maca`.

### Recursion-bound (call overhead)

Tree/nested recursion — dominated by function-call cost.

| kernel | params | Maca | C | Rust | Go | JS (node) | Python |
|---|---|---|---|---|---|---|---|
| fib | fib(40) | 168 (×1.0) | 172 (×1.0) | 257 (×1.5) | 488 (×2.8) | 994 (×5.8) | 11622 (×67.6) |
| tak | tak(32,16,8) | 69 (×1.0) | 66 (×1.0) | 59 (×0.9) | 75 (×1.1) | 196 (×3.0) | 1735 (×26.3) |
| ackermann | ack(3,10) | 18 (×0.9) | 21 (×1.0) | 35 (×1.7) | 70 (×3.3) | 234 (×11.1) | 4634 (×220.7) |

### Compute-bound (loops, arrays, float)

Tight loops over arrays and floating point — dominated by the inner loop.

| kernel | params | Maca | C | Rust | Go | JS (node) | Python |
|---|---|---|---|---|---|---|---|
| sieve | primes ≤ 10⁷ | 259 (×2.7) | 97 (×1.0) | 105 (×1.1) | 101 (×1.0) | 199 (×2.1) | 505 (×5.2) |
| mandel | 800×800, ≤1000 it | 408 (×1.0) | 393 (×1.0) | 400 (×1.0) | 419 (×1.1) | 463 (×1.2) | 13341 (×33.9) |
| matmul | 400×400 int | 61 (×1.1) | 57 (×1.0) | 57 (×1.0) | 125 (×2.2) | 163 (×2.9) | 4788 (×84.0) |

On recursion and float/loop work Maca lands right on C (it *is* C, at
`-O2`). `sieve` is the one gap: Maca's only array type is 64-bit `int[]`,
so a sieve over 10⁷ touches ~80 MB where C's `char` array touches ~10 MB —
it's memory-bound, not a codegen deficit (a compact byte array would close
it).

_Host: Linux 6.18.5 x86_64. Times are wall-clock medians including process startup._
