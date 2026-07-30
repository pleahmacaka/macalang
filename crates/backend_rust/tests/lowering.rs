//! What the rust backend lowers, and what it refuses by name.
//!
//! Two catch-alls made wrong answers look like right ones. `expr` ended in
//! `Default::default()`, which type-checks wherever a value is wanted — so
//! `break` inside a `while` became a value and the loop could not terminate.
//! `pat_match` ended in `_`, so a float, record or list pattern became a
//! wildcard; rustc reports an *unreachable pattern warning* for the arms it
//! swallows, and the build succeeds with the first arm always winning.
//!
//! Where rustc is on PATH the emitted program is compiled and run, because
//! compiling is not answering.

use std::process::Command;

fn emit(src: &str) -> Result<String, Vec<String>> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    maca_backend_rust::emit_checked(&p.module)
}

fn ok(src: &str) -> String {
    match emit(src) {
        Ok(s) => s,
        Err(e) => panic!("unexpected refusal: {e:?}"),
    }
}

fn refused(src: &str) -> String {
    match emit(src) {
        Ok(s) => panic!("expected a refusal, got:\n{s}"),
        Err(e) => e.join("\n"),
    }
}

fn have_rustc() -> bool {
    Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Compile and run the emitted Rust, returning its stdout.
fn run(src: &str) -> Option<String> {
    if !have_rustc() {
        eprintln!("skipping: no rustc");
        return None;
    }
    let rs = ok(src);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&src, &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-rs-{}-{key:x}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("m.rs");
    std::fs::write(&path, &rs).unwrap();
    let bin = dir.join("m");
    let c = Command::new("rustc")
        .arg(&path)
        .args(["--edition", "2021", "-o"])
        .arg(&bin)
        .output()
        .unwrap();
    assert!(
        c.status.success(),
        "rustc failed\n{}\n--- rs ---\n{rs}",
        String::from_utf8_lossy(&c.stderr)
    );
    let o = Command::new(&bin).output().unwrap();
    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

const BREAKS: &str = "\
count_to(n: int) -> int {
    i = 0
    while true {
        i = i + 1
        if i >= n { break }
    }
    i
}

evens_below(n: int) -> int {
    total = 0
    for i in 1..n {
        if i % 2 == 1 { continue }
        total = total + i
    }
    total
}

main() -> int {
    info(str(count_to(5)))
    info(str(evens_below(6)))
    0
}
";

#[test]
fn break_and_continue_are_lowered_not_defaulted() {
    let rs = ok(BREAKS);
    assert!(rs.contains("break"), "no break in:\n{rs}");
    assert!(rs.contains("continue"), "no continue in:\n{rs}");
    assert!(
        !rs.contains("Default::default()"),
        "still defaulting:\n{rs}"
    );
}

#[test]
fn a_loop_with_break_terminates_and_answers() {
    // The whole point: as `Default::default()` this program hangs.
    let Some(out) = run(BREAKS) else { return };
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["5", "12"]);
}

#[test]
fn a_record_update_is_lowered() {
    let src = "\
Point = { x: int, y: int }

moved(p: Point) -> int {
    q = p with { x = 9 }
    q.x + q.y
}

main() -> int {
    info(str(moved(Point { x = 1, y = 2 })))
    0
}
";
    let rs = ok(src);
    assert!(!rs.contains("Default::default()"), "{rs}");
    let Some(out) = run(src) else { return };
    assert_eq!(out, "11");
}

#[test]
fn a_float_pattern_is_refused_rather_than_becoming_a_wildcard() {
    // As two `_` arms this compiles with only a warning and always answers "a".
    let msg = refused(
        "pick(x: float) -> str {\n    match x {\n        1.5 => \"a\"\n        2.5 => \"b\"\n        _ => \"c\"\n    }\n}\n",
    );
    assert!(msg.contains("1.5"), "message does not name it: {msg}");
    assert!(
        msg.contains("guard"),
        "message does not say what to do: {msg}"
    );
}

#[test]
fn a_list_pattern_is_refused_rather_than_becoming_a_wildcard() {
    let msg = refused(
        "first(xs: int[]) -> int {\n    match xs {\n        [] => 0\n        x, ..rest => x\n    }\n}\n",
    );
    assert!(msg.contains("list pattern"), "{msg}");
}

#[test]
fn a_refusal_message_has_no_stray_indentation() {
    // A wrapped message read as one run-on line with a gap in the middle.
    let msg = refused(
        "pick(x: float) -> str {\n    match x {\n        1.5 => \"a\"\n        _ => \"c\"\n    }\n}\n",
    );
    assert!(!msg.contains("  "), "double space in: {msg:?}");
}

#[test]
fn sum_variants_and_guards_still_lower() {
    // The refusals must not have swallowed the patterns that do work.
    let src = "\
Shape = Circle(int) | Rect(int, int)

area(s: Shape) -> int {
    match s {
        Circle(r) => r * r
        Rect(w, h) => w * h
    }
}

main() -> int {
    info(str(area(Rect(3, 4))))
    0
}
";
    let Some(out) = run(src) else { return };
    assert_eq!(out, "12");
}
