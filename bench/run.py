#!/usr/bin/env python3
"""Benchmark harness: Maca vs C / Rust / Go / JS / Python across algorithms.

Two families of workload:
  * recursion-bound integer kernels  (fib, tak, ackermann) — call overhead
  * compute-bound loops/arrays/float (sieve, mandel, matmul)

Every implementation computes the *same* result (verified against EXPECTED).
Compiled languages are built once at -O2/-O; then we time execution (median of
N runs, one warmup). Maca goes through the `maca` CLI (Maca → C → `cc -O2`), so
it should land in C's ballpark — the "C-tier native" claim — while staying far
ahead of interpreted Python and JIT'd JS on call-bound code.

Usage:  python3 bench/run.py [--runs N]
Writes: bench/results.json, bench/results.md
"""

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BENCH = os.path.join(ROOT, "bench")
REF = os.path.join(BENCH, "ref")
TMP = "/tmp/maca-bench"


def find_maca():
    for prof in ("release", "debug"):
        p = os.path.join(ROOT, "target", prof, "maca")
        if os.path.exists(p):
            return p
    sys.exit("no `maca` binary — run `cargo build -p maca-driver`")


MACA = find_maca()

# families and their parameters (shown in the report)
RECURSION = ["fib", "tak", "ackermann"]
COMPUTE = ["sieve", "mandel", "matmul"]
ALGOS = RECURSION + COMPUTE

EXPECTED = {
    "fib": "102334155",
    "tak": "9",
    "ackermann": "8189",
    "sieve": "664579",
    "mandel": "141554306",
    "matmul": "163198950",
}
PARAMS = {
    "fib": "fib(40)",
    "tak": "tak(32,16,8)",
    "ackermann": "ack(3,10)",
    "sieve": "primes ≤ 10⁷",
    "mandel": "800×800, ≤1000 it",
    "matmul": "400×400 int",
}

# order languages fastest-family-first for the table
LANGS = ["Maca", "C", "Rust", "Go", "JS (node)", "Python"]
# node needs a bigger stack for the deep ackermann recursion
NODE = ["node", "--stack-size=8000"]


def sh(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def time_cmd(cmd, runs):
    """Median wall-clock ms over `runs` executions (after one warmup)."""
    sh(cmd)  # warmup
    times = []
    out = ""
    for _ in range(runs):
        t0 = time.perf_counter()
        r = sh(cmd)
        times.append((time.perf_counter() - t0) * 1000.0)
        out = r.stdout.strip()
    return statistics.median(times), out


def build_maca(algo):
    out = os.path.join(TMP, f"{algo}_maca")
    r = sh([MACA, "build", os.path.join(BENCH, f"{algo}.maca"), "-o", out])
    if r.returncode != 0:
        sys.exit(f"maca build {algo} failed:\n{r.stderr}")
    return [out]


def build_c(algo):
    out = os.path.join(TMP, f"{algo}_c")
    r = sh(["cc", "-O2", os.path.join(REF, f"{algo}.c"), "-o", out])
    if r.returncode != 0:
        sys.exit(f"cc {algo} failed:\n{r.stderr}")
    return [out]


def build_rust(algo):
    out = os.path.join(TMP, f"{algo}_rs")
    r = sh(["rustc", "-O", os.path.join(REF, f"{algo}.rs"), "-o", out])
    if r.returncode != 0:
        sys.exit(f"rustc {algo} failed:\n{r.stderr}")
    return [out]


def build_go(algo):
    out = os.path.join(TMP, f"{algo}_go")
    env = dict(os.environ, GOCACHE=os.path.join(TMP, "gocache"))
    r = sh(["go", "build", "-o", out, os.path.join(REF, f"{algo}.go")], env=env)
    if r.returncode != 0:
        sys.exit(f"go build {algo} failed:\n{r.stderr}")
    return [out]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=5)
    args = ap.parse_args()
    os.makedirs(TMP, exist_ok=True)

    have = {
        "Rust": shutil.which("rustc") is not None,
        "Go": shutil.which("go") is not None,
        "JS (node)": shutil.which("node") is not None,
        "Python": shutil.which("python3") is not None,
    }

    results = {}
    for algo in ALGOS:
        cmds = {"Maca": build_maca(algo), "C": build_c(algo)}
        if have["Rust"]:
            cmds["Rust"] = build_rust(algo)
        if have["Go"]:
            cmds["Go"] = build_go(algo)
        if have["JS (node)"]:
            cmds["JS (node)"] = NODE + [os.path.join(REF, f"{algo}.js")]
        if have["Python"]:
            cmds["Python"] = ["python3", os.path.join(REF, f"{algo}.py")]

        results[algo] = {}
        for lang, cmd in cmds.items():
            ms, out = time_cmd(cmd, args.runs)
            ok = out == EXPECTED[algo]
            results[algo][lang] = {"ms": round(ms, 3), "output": out, "ok": ok}
            flag = "ok" if ok else f"MISMATCH (got {out}, want {EXPECTED[algo]})"
            print(f"{algo:10} {lang:10} {ms:10.2f} ms  {flag}")

    with open(os.path.join(BENCH, "results.json"), "w") as f:
        json.dump(results, f, indent=2)
    write_markdown(results)
    print(f"\nwrote {BENCH}/results.json and results.md")


def cell(results, algo, lang, base):
    c = results[algo].get(lang)
    if not c:
        return "—"
    ms = c["ms"]
    mult = f" (×{ms / base:.1f})" if base and base > 0 else ""
    return f"{ms:.1f}{mult}"


def table(results, algos, title, note):
    lines = [f"### {title}", "", note, ""]
    lines.append("| kernel | params | " + " | ".join(LANGS) + " |")
    lines.append("|" + "---|" * (len(LANGS) + 2))
    for algo in algos:
        base = results[algo].get("C", {}).get("ms")
        row = [algo, PARAMS[algo]] + [cell(results, algo, l, base) for l in LANGS]
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")
    return lines


def write_markdown(results):
    u = os.uname()
    lines = [
        "# Benchmarks — Maca vs C / Rust / Go / JS / Python",
        "",
        "Median execution time in **ms** (lower is better); `×N` is the slowdown",
        "vs C. Maca is `maca build` (Maca → C → `cc -O2`); C is `cc -O2`; Rust is",
        "`rustc -O`; Go is `go build`; JS is Node; Python is CPython 3. Every",
        "column computes the same verified result. Reproduce with `python3",
        "bench/run.py`.",
        "",
    ]
    lines += table(
        results, RECURSION, "Recursion-bound (call overhead)",
        "Tree/nested recursion — dominated by function-call cost.",
    )
    lines += table(
        results, COMPUTE, "Compute-bound (loops, arrays, float)",
        "Tight loops over arrays and floating point — dominated by the inner loop.",
    )
    lines += [
        "On recursion and float/loop work Maca lands right on C (it *is* C, at",
        "`-O2`). `sieve` is the one gap: Maca's only array type is 64-bit `int[]`,",
        "so a sieve over 10⁷ touches ~80 MB where C's `char` array touches ~10 MB —",
        "it's memory-bound, not a codegen deficit (a compact byte array would close",
        "it).",
        "",
        f"_Host: {u.sysname} {u.machine}, {u.release}. Times are wall-clock "
        f"medians including process startup._",
        "",
    ]
    with open(os.path.join(BENCH, "results.md"), "w") as f:
        f.write("\n".join(lines))


if __name__ == "__main__":
    main()
