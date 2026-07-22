#!/usr/bin/env python3
"""Benchmark harness: Maca vs C vs Python vs JS on recursion-heavy kernels.

Each program computes the same result; we compile the compiled languages once,
then time execution (median of N runs). Maca is built through the `maca` CLI
(→ C → native cc -O2), so it should land in C's ballpark — the point of the
"C-tier" claim — while staying far ahead of interpreted Python and JIT'd JS on
call-bound code.

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
MACA = os.path.join(ROOT, "target", "release", "maca")

ALGOS = ["fib", "tak", "ackermann"]
EXPECTED = {"fib": "102334155", "tak": "9", "ackermann": "8189"}

# node needs a bigger stack for the deep ackermann recursion
NODE = ["node", "--stack-size=8000"]


def sh(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def time_cmd(cmd, runs):
    """Return (median_ms, stdout) over `runs` timed executions (1 warmup)."""
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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=5)
    args = ap.parse_args()
    os.makedirs(TMP, exist_ok=True)

    have_node = shutil.which("node") is not None
    have_py = shutil.which("python3") is not None

    results = {}
    for algo in ALGOS:
        langs = {
            "Maca": build_maca(algo),
            "C": build_c(algo),
        }
        if have_py:
            langs["Python"] = ["python3", os.path.join(REF, f"{algo}.py")]
        if have_node:
            langs["JS (node)"] = NODE + [os.path.join(REF, f"{algo}.js")]

        results[algo] = {}
        for lang, cmd in langs.items():
            ms, out = time_cmd(cmd, args.runs)
            ok = out == EXPECTED[algo]
            results[algo][lang] = {"ms": round(ms, 3), "output": out, "ok": ok}
            print(f"{algo:10} {lang:10} {ms:9.2f} ms  out={out} {'ok' if ok else 'MISMATCH'}")

    with open(os.path.join(BENCH, "results.json"), "w") as f:
        json.dump(results, f, indent=2)
    write_markdown(results)
    print(f"\nwrote {BENCH}/results.json and results.md")


def write_markdown(results):
    langs = ["Maca", "C", "JS (node)", "Python"]
    lines = [
        "# Benchmarks — Maca vs C vs JS vs Python",
        "",
        "Recursion-heavy integer kernels. Median execution time (ms); lower is",
        "better. `×C` is the slowdown relative to C. Maca is built through the",
        "`maca` CLI (Maca → C → `cc -O2`); C is `cc -O2`; Python is CPython 3;",
        "JS is Node. See `bench/run.py`.",
        "",
        "| kernel | " + " | ".join(langs) + " |",
        "|" + "---|" * (len(langs) + 1),
    ]
    for algo in ALGOS:
        row = [algo]
        base = results[algo].get("C", {}).get("ms")
        for lang in langs:
            cell = results[algo].get(lang)
            if not cell:
                row.append("—")
            else:
                ms = cell["ms"]
                mult = f" (×{ms / base:.1f})" if base else ""
                row.append(f"{ms:.1f} ms{mult}")
        lines.append("| " + " | ".join(row) + " |")
    lines += [
        "",
        f"_Params: fib(40), tak(32,16,8), ackermann(3,10). "
        f"Host: {os.uname().sysname} {os.uname().machine}._",
        "",
    ]
    with open(os.path.join(BENCH, "results.md"), "w") as f:
        f.write("\n".join(lines))


if __name__ == "__main__":
    main()
