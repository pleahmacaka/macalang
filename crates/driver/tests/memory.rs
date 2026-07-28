//! Perceus: the code generator inserts the drops, and the buffer comes back.
//!
//! The runtime has always had a size-tracked allocator with a free-list. What
//! it did not have was anyone calling `drop` — so a run-once program held every
//! buffer it ever allocated until exit. These tests are about the other half:
//! codegen releasing a local's buffer when the local cannot outlive its block,
//! and the next allocation of that size picking it up instead of calling
//! malloc.
//!
//! The valgrind test is the one that matters. Reuse without correctness is a
//! use-after-free, and the failure is silent.

use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn build(name: &str, src: &str) -> Option<std::path::PathBuf> {
    let dir = std::env::temp_dir().join("maca-memory-test");
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join(format!("{name}.maca"));
    let bin = dir.join(name);
    std::fs::write(&f, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["build", &f.to_string_lossy(), "-o", &bin.to_string_lossy()])
        .output()
        .expect("spawn maca build");
    assert!(
        out.status.success(),
        "build failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(bin)
}

fn run(bin: &std::path::Path) -> String {
    let out = Command::new(bin).output().expect("run");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A loop that builds and discards a value reuses one buffer instead of asking
/// the allocator for a new one every time round.
#[test]
fn a_discarded_buffer_is_reused() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let src = "build(n: int) -> int[] {\n\
        \x20   xs = []\n\
        \x20   for i in 1..n {\n\
        \x20       xs = xs.push(i)\n\
        \x20   }\n\
        \x20   xs\n\
        }\n\n\
        main() -> int {\n\
        \x20   total = 0\n\
        \x20   for round in 1..500 {\n\
        \x20       ys = build(200)\n\
        \x20       total = total + ys.length()\n\
        \x20   }\n\
        \x20   info(\"total={total}\")\n\
        \x20   info(\"reused={reuse_count() * 100 / alloc_count()}\")\n\
        \x20   0\n\
        }\n";
    let bin = build("reuse", src).unwrap();
    let out = run(&bin);
    assert!(out.contains("total=100000"), "wrong result:\n{out}");
    // essentially every allocation after the first round comes off the
    // free-list; without codegen-inserted drops this is 0
    let pct: i64 = out
        .lines()
        .find_map(|l| l.strip_prefix("reused="))
        .and_then(|n| n.parse().ok())
        .unwrap_or(-1);
    assert!(pct > 90, "only {pct}% of allocations were reused:\n{out}");
}

/// The correctness half. A value that escapes its block — returned, kept in an
/// outer binding, stored in a longer-lived structure — must not be dropped.
#[test]
fn nothing_live_is_dropped() {
    if wsl() || !have("cc") || !have("valgrind") {
        eprintln!("skipping: needs a host cc, valgrind, and no wsl");
        return;
    }
    let src = "keep(xs: int[]) -> int[] => xs\n\n\
        sum_of(xs: int[]) -> int => xs.reduce(0, (a, b) => a + b)\n\n\
        main() -> int {\n\
        \x20   // aliased: two names, one buffer\n\
        \x20   a = [1, 2, 3]\n\
        \x20   b = a\n\
        \x20   info(\"1: {a.length()} {b.length()} {sum_of(a)} {sum_of(b)}\")\n\
        \x20   // built in a loop, one kept out of it\n\
        \x20   kept = []\n\
        \x20   for i in 1..50 {\n\
        \x20       tmp = []\n\
        \x20       for j in 1..20 {\n\
        \x20           tmp = tmp.push(i * j)\n\
        \x20       }\n\
        \x20       kept = i == 25 ? tmp : kept\n\
        \x20   }\n\
        \x20   info(\"2: {kept.length()} {kept.get(0)} {kept.get(19)}\")\n\
        \x20   // the same for a map\n\
        \x20   held: Map str int = map()\n\
        \x20   for i in 1..100 {\n\
        \x20       m: Map str int = map()\n\
        \x20       m = m.set(\"x\", i).set(\"y\", i * 2)\n\
        \x20       held = i == 50 ? m : held\n\
        \x20   }\n\
        \x20   info(\"3: {held.get(\"x\", 0)} {held.get(\"y\", 0)} {held.length()}\")\n\
        \x20   // handed to a function that gives it back\n\
        \x20   c = keep([9, 8, 7])\n\
        \x20   info(\"4: {c.length()} {c.get(0)}\")\n\
        \x20   // arrays of arrays: the inner buffers outlive their block\n\
        \x20   rows: int[][] = []\n\
        \x20   for i in 1..30 {\n\
        \x20       row = []\n\
        \x20       for j in 1..5 {\n\
        \x20           row = row.push(i + j)\n\
        \x20       }\n\
        \x20       rows = rows.push(row)\n\
        \x20   }\n\
        \x20   info(\"5: {rows.length()} {rows.get(0).length()} {rows.get(29).get(4)}\")\n\
        \x20   0\n\
        }\n";
    let bin = build("live", src).unwrap();
    let out = Command::new("valgrind")
        .args(["--error-exitcode=9", "--leak-check=full", "-q"])
        .arg(&bin)
        .output()
        .expect("valgrind");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // the answers first: a dropped-too-early buffer usually reads as garbage
    // long before valgrind is consulted
    for want in [
        "1: 3 3 6 6",
        "2: 20 25 500",
        "3: 50 100 2",
        "4: 3 9",
        "5: 30 5 35",
    ] {
        assert!(stdout.contains(want), "expected {want:?} in:\n{stdout}");
    }
    assert!(
        out.status.success(),
        "valgrind found a memory error:\n{stderr}"
    );
    assert!(
        stderr.trim().is_empty(),
        "valgrind was not quiet:\n{stderr}"
    );
}

/// Nested array types are emitted in dependency order.
///
/// `IntArrArr`'s element is `IntArr`, so the inner type has to be complete
/// first. The element set is a hash set, so before it was sorted whether
/// `int[][]` compiled depended on the iteration order.
#[test]
fn nested_array_types_are_declared_before_use() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let src = "main() -> int {\n\
        \x20   grid: int[][] = []\n\
        \x20   for i in 1..3 {\n\
        \x20       row = []\n\
        \x20       for j in 1..3 {\n\
        \x20           row = row.push(i * j)\n\
        \x20       }\n\
        \x20       grid = grid.push(row)\n\
        \x20   }\n\
        \x20   info(\"{grid.length()} {grid.get(2).get(2)}\")\n\
        \x20   0\n\
        }\n";
    let bin = build("nested", src).unwrap();
    assert!(run(&bin).contains("3 9"), "nested arrays wrong");
}
