//! The standard library surface that appendix C documents, executed.
//!
//! Appendix C used to carry a "What is missing" list — no hash map, no file
//! metadata, no stdin, no time, no assertions, no string `slice`. Each of
//! those is now a real builtin, and each is exercised here, because a
//! documented library that nothing runs is a claim rather than a fact.
//!
//! Most of it is asserted in Maca, in `modules/std/tests/builtins.maca`, and run by
//! `maca test`. What stays here is the two cases that are about the *process*
//! rather than the values: reading piped stdin, and what a failing assertion
//! writes and returns.

mod common;
use common::*;

use std::io::Write;
use std::process::{Command, Stdio};

/// Write `src` to a scratch file and `maca run` it with `stdin` on its input.
/// Returns the exit status and stdout+stderr together.
fn run_with(name: &str, src: &str, stdin: &str) -> (bool, String) {
    let dir = std::env::temp_dir().join("maca-stdlib-test");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join(format!("{name}.maca"));
    std::fs::write(&file, src).expect("write source");

    let mut child = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &file.to_string_lossy()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn maca run");

    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");

    let out = child.wait_with_output().expect("wait");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

/// Maps, string `slice`, file metadata and time — asserted in Maca.
#[test]
fn the_documented_builtins_work() {
    if unsupported_host() {
        return;
    }

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
        .args(["test", "modules/std/tests/builtins.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Standard input, line by line, until it runs out.
#[test]
fn stdin_can_be_read() {
    if unsupported_host() {
        return;
    }

    let (ok, out) = run_with(
        "stdin",
        r#"main() -> int {
    n = 0
    while !at_eof() {
        line = read_line()
        n = n + 1
        info("{n}: {line.upper()}")
    }
    info("lines: {n}")
    0
}
"#,
        "alpha\nbeta\ngamma\n",
    );

    assert!(ok, "{out}");
    for want in ["1: ALPHA", "3: GAMMA", "lines: 3"] {
        assert!(out.contains(want), "expected {want:?} in:\n{out}");
    }
}

/// Assertions report and keep going, and `failures()` is what a test returns.
///
/// Aborting on the first failure means fixing a suite takes as many runs as it
/// has bugs; counting them means one run tells you everything.
#[test]
fn assertions_count_rather_than_abort() {
    if unsupported_host() {
        return;
    }

    let (ok, out) = run_with(
        "assert",
        r#"test_arithmetic() -> int {
    assert(1 + 1 == 2, "one plus one")
    assert_eq("{2 * 21}", "42", "the answer")
    failures()
}

main() -> int {
    info("clean: {test_arithmetic()}")
    assert(false, "deliberate")
    assert_eq("got", "want", "also deliberate")
    info("failures: {failures()}")
    0
}
"#,
        "",
    );

    assert!(ok, "{out}");
    assert!(
        out.contains("clean: 0"),
        "a passing test should count 0:\n{out}"
    );
    // Both failures ran — the first didn't stop the second.
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
