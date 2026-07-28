//! The standard library surface that appendix C documents, executed.
//!
//! Appendix C used to carry a "What is missing" list — no hash map, no file
//! metadata, no stdin, no time, no assertions, no string `slice`. Each of those
//! is now a real builtin, and each is exercised here, because a documented
//! library that nothing runs is a claim rather than a fact.

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

fn run_with(name: &str, src: &str, stdin: &str) -> (bool, String) {
    use std::io::Write;
    use std::process::Stdio;
    let dir = std::env::temp_dir().join("maca-stdlib-test");
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join(format!("{name}.maca"));
    std::fs::write(&f, src).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn maca run");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
    )
}

fn run(name: &str, src: &str) -> (bool, String) {
    run_with(name, src, "")
}

/// `Map str V` — a real hash map, monomorphized on its value type the way an
/// array is on its element type.
///
/// Keys are `str` and only `str`: one key type means one hash and one
/// comparison, and an integer key is `str(n)` away.
#[test]
fn maps_work() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "maps",
        "main() -> int {\n\
        \x20   counts: Map str int = map()\n\
        \x20   counts = counts.set(\"apple\", 3).set(\"pear\", 1).set(\"apple\", 5)\n\
        \x20   info(\"a: {counts.get(\"apple\", 0)} {counts.get(\"kiwi\", -1)} {counts.length()}\")\n\
        \x20   info(\"b: {counts.has(\"pear\")} {counts.has(\"kiwi\")}\")\n\
        \x20   // sorted, so walking a map twice writes the same file twice\n\
        \x20   info(\"c: {counts.keys().join(\",\")}\")\n\
        \x20   counts = counts.remove(\"pear\")\n\
        \x20   info(\"d: {counts.length()} {counts.has(\"pear\")}\")\n\
        \x20   // a value type other than int\n\
        \x20   names: Map str str = map()\n\
        \x20   names = names.set(\"ko\", \"한국어\")\n\
        \x20   info(\"e: {names.get(\"ko\", \"?\")}\")\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "maps don't work:\n{out}");
    for want in [
        "a: 5 -1 2", // set overwrites, get takes a default, length counts keys
        "b: true false",
        "c: apple,pear", // keys come back sorted
        "d: 1 false",    // remove
        "e: 한국어",     // a str-valued map
    ] {
        assert!(out.contains(want), "expected {want:?} in:\n{out}");
    }
}

/// Enough entries to force several grows and to exercise deletion's
/// backward-shift: a probe that stopped at a hole would lose keys.
#[test]
fn a_map_survives_growth_and_deletion() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "map_grow",
        "main() -> int {\n\
        \x20   m: Map str int = map()\n\
        \x20   for i in 1..200 {\n\
        \x20       m = m.set(\"k{i}\", i)\n\
        \x20   }\n\
        \x20   info(\"n={m.length()} first={m.get(\"k1\", 0)} last={m.get(\"k200\", 0)}\")\n\
        \x20   for i in 1..100 {\n\
        \x20       m = m.remove(\"k{i}\")\n\
        \x20   }\n\
        \x20   info(\"after={m.length()} gone={m.has(\"k50\")} kept={m.get(\"k150\", 0)}\")\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("n=200 first=1 last=200"),
        "growth lost entries:\n{out}"
    );
    assert!(
        out.contains("after=100 gone=false kept=150"),
        "deletion broke the probe chain:\n{out}"
    );
}

/// `slice` takes an exclusive end on a string exactly as it does on a list.
#[test]
fn strings_have_slice() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "slice",
        "main() -> int {\n\
        \x20   s = \"abcdef\"\n\
        \x20   info(\"{s.slice(1, 3)} {s.substr(1, 3)} {s.slice(0, 99)} {s.slice(4, 2)}\")\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    // slice(1,3) is two characters; substr(1,3) is three — the two names keep
    // their own conventions, which is the point of having both
    assert!(out.contains("bc bcd abcdef "), "slice/substr wrong:\n{out}");
}

/// File metadata and deletion.
#[test]
fn file_metadata_and_deletion_work() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "files",
        "main() -> int {\n\
        \x20   d = \"/tmp/maca-stdlib-files\"\n\
        \x20   make_dir(d ++ \"/nested\")\n\
        \x20   p = d ++ \"/f.txt\"\n\
        \x20   write_file(p, \"hello\")\n\
        \x20   info(\"a: {file_size(p)} {is_dir(p)} {is_dir(d)}\")\n\
        \x20   info(\"b: {file_size(d ++ \"/none\")} {modified_ms(p) > 0}\")\n\
        \x20   remove_file(p)\n\
        \x20   info(\"c: {file_exists(p)}\")\n\
        \x20   // recursive: the directory still holds `nested`\n\
        \x20   remove_dir(d)\n\
        \x20   info(\"d: {file_exists(d)}\")\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    assert!(out.contains("a: 5 false true"), "metadata wrong:\n{out}");
    // a missing file is -1, not 0 — an empty file and an absent one differ
    assert!(
        out.contains("b: -1 true"),
        "missing-file size wrong:\n{out}"
    );
    assert!(out.contains("c: false"), "remove_file wrong:\n{out}");
    assert!(
        out.contains("d: false"),
        "remove_dir wasn't recursive:\n{out}"
    );
}

/// Standard input, line by line and whole.
#[test]
fn stdin_can_be_read() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run_with(
        "stdin",
        "main() -> int {\n\
        \x20   n = 0\n\
        \x20   while !at_eof() {\n\
        \x20       line = read_line()\n\
        \x20       n = n + 1\n\
        \x20       info(\"{n}: {line.upper()}\")\n\
        \x20   }\n\
        \x20   info(\"lines: {n}\")\n\
        \x20   0\n\
        }\n",
        "alpha\nbeta\ngamma\n",
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("1: ALPHA") && out.contains("3: GAMMA") && out.contains("lines: 3"),
        "line reading wrong:\n{out}"
    );
}

/// Time, in UTC.
#[test]
fn time_is_available() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "time",
        "main() -> int {\n\
        \x20   // a fixed instant, so the assertion is about formatting\n\
        \x20   info(\"a: {format_time(0, \"%Y-%m-%dT%H:%M:%SZ\")}\")\n\
        \x20   info(\"b: {format_time(86400000, \"%Y-%m-%d\")}\")\n\
        \x20   info(\"c: {now_ms() > 1700000000000}\")\n\
        \x20   info(\"d: {now_iso().length()}\")\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("a: 1970-01-01T00:00:00Z"),
        "epoch wrong:\n{out}"
    );
    assert!(out.contains("b: 1970-01-02"), "a day later wrong:\n{out}");
    assert!(out.contains("c: true"), "now_ms wrong:\n{out}");
    assert!(out.contains("d: 20"), "now_iso length wrong:\n{out}");
}

/// Assertions report and keep going, and `failures()` is what a test returns.
///
/// Aborting on the first failure means fixing a suite takes as many runs as it
/// has bugs; counting them means one run tells you everything.
#[test]
fn assertions_count_rather_than_abort() {
    if wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let (ok, out) = run(
        "assert",
        "test_arithmetic() -> int {\n\
        \x20   assert(1 + 1 == 2, \"one plus one\")\n\
        \x20   assert_eq(\"{2 * 21}\", \"42\", \"the answer\")\n\
        \x20   failures()\n\
        }\n\n\
         main() -> int {\n\
        \x20   info(\"clean: {test_arithmetic()}\")\n\
        \x20   assert(false, \"deliberate\")\n\
        \x20   assert_eq(\"got\", \"want\", \"also deliberate\")\n\
        \x20   info(\"failures: {failures()}\")\n\
        \x20   0\n\
        }\n",
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("clean: 0"),
        "a passing test should count 0:\n{out}"
    );
    // both failures ran — the first didn't stop the second
    assert!(
        out.contains("assertion failed: deliberate"),
        "no report:\n{out}"
    );
    assert!(
        out.contains("got:  got") && out.contains("want: want"),
        "assert_eq should show both sides:\n{out}"
    );
    assert!(out.contains("failures: 2"), "count wrong:\n{out}");
}
