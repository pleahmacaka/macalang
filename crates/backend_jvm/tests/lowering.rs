//! What the jvm backend lowers, and what it refuses by name.
//!
//! `jexpr` ended in `_ => "null"`, and `null` is assignable to every Java
//! reference type — so an unlowered construct type-checked and the program ran
//! with a hole in it. The worst case was `Expr::Lambda`: a closure handed to a
//! Java API became `null`, and registering a callback is what this target is
//! for.
//!
//! Where a JDK is present the emitted Java is compiled and run, because
//! compiling is not answering.

use std::process::Command;

fn emit(src: &str) -> Result<String, Vec<String>> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    maca_backend_jvm::emit_checked(&p.module, "M", None)
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

fn have_jdk() -> bool {
    Command::new("javac")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Compile the emitted Java and run `M.main`, returning stdout.
fn run(src: &str) -> Option<String> {
    if !have_jdk() {
        eprintln!("skipping: no JDK");
        return None;
    }
    let java = ok(src);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&src, &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-jvm-{}-{key:x}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("M.java"), &java).unwrap();

    let c = Command::new("javac")
        .arg("M.java")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        c.status.success(),
        "javac failed\n{}\n--- java ---\n{java}",
        String::from_utf8_lossy(&c.stderr)
    );
    let o = Command::new("java")
        .arg("M")
        .current_dir(&dir)
        .output()
        .unwrap();
    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

#[test]
fn a_lambda_is_a_java_lambda_not_null() {
    // A closure handed to a Java API is what this target is for — a Fabric mod
    // registers callbacks — and it used to emit `null`, which is assignable to
    // any functional interface, so it compiled and the callback did nothing.
    //
    // Only the emission is asserted: calling one through an unannotated Maca
    // parameter needs that parameter to be a functional interface rather than
    // `Object`, which this backend does not yet do.
    let java =
        ok("register(cb) {\n    cb(1)\n}\n\nmain() -> int {\n    register(v => v + 1)\n    0\n}\n");
    assert!(
        java.contains("(v) -> (v + 1L)"),
        "lambda not lowered:\n{java}"
    );
    assert!(
        !java.contains("register(null)"),
        "still passing null:\n{java}"
    );
}

#[test]
fn break_and_continue_are_lowered() {
    let src = "\
count_to(n: int) -> int {
    i = 0
    while true {
        i = i + 1
        if i >= n { break }
    }
    i
}

main() -> int {
    info(str(count_to(4)))
    0
}
";
    let Some(out) = run(src) else { return };
    assert_eq!(out, "4");
}

#[test]
fn a_string_pattern_is_a_case_label_not_a_second_default() {
    // Both `Str` and `Bool` used to fall to `default`, so two such arms were a
    // `duplicate default label` error from javac.
    let src = "\
grade(s: str) -> int {
    match s {
        \"a\" => 1
        \"b\" => 2
        _ => 0
    }
}

main() -> int {
    info(str(grade(\"b\")))
    0
}
";
    let java = ok(src);
    assert!(java.contains("case \"b\""), "no string label in:\n{java}");
    assert_eq!(
        java.matches("default").count(),
        1,
        "more than one default:\n{java}"
    );
    let Some(out) = run(src) else { return };
    assert_eq!(out, "2");
}

#[test]
fn a_bool_pattern_is_refused_because_java_cannot_switch_on_one() {
    let msg = refused(
        "f(b: bool) -> int {\n    match b {\n        true => 1\n        false => 2\n    }\n}\n",
    );
    assert!(msg.contains("bool"), "{msg}");
}

#[test]
fn a_payload_pattern_is_refused_rather_than_dropping_its_binding() {
    // `Circle(r) => r * r` emitted `case Circle ->` and javac said
    // "cannot find symbol: r".
    let msg = refused(
        "Shape = Circle(int) | Rect(int, int)\n\narea(s: Shape) -> int {\n    match s {\n        Circle(r) => r * r\n        Rect(w, h) => w * h\n    }\n}\n",
    );
    assert!(msg.contains("payload"), "{msg}");
    assert!(msg.contains("Circle"), "message does not name it: {msg}");
}

#[test]
fn a_literal_matching_no_declared_record_is_refused_rather_than_becoming_null() {
    // A literal whose shape belongs to a declared record now takes that record's
    // name, since the checker accepts it there. One that matches nothing still
    // has no type for a Java `new`, and the refusal names the fields written so
    // the author can see what it looked for.
    let msg = refused("f() -> int {\n    p = { x = 1, y = 2 }\n    p.x\n}\n");
    assert!(msg.contains("no declared record"), "{msg}");
    assert!(
        msg.contains('x') && msg.contains('y'),
        "does not name the fields: {msg}"
    );
}

#[test]
fn spawn_and_await_are_refused_by_name() {
    let msg = refused("f() -> int {\n    fut = spawn g()\n    await fut\n}\n\ng() -> int => 1\n");
    assert!(msg.contains("spawn") || msg.contains("await"), "{msg}");
}

#[test]
fn a_refusal_names_a_construct_not_generated_java() {
    for src in [
        "f() -> int {\n    p = { x = 1 }\n    p.x\n}\n",
        "f(b: bool) -> int {\n    match b {\n        true => 1\n        false => 2\n    }\n}\n",
    ] {
        let msg = refused(src);
        assert!(!msg.contains("null"), "refusal talks about output: {msg}");
    }
}

#[test]
fn enums_records_and_arithmetic_still_lower_and_run() {
    // The refusals must not have swallowed the subset that already worked.
    let src = "\
Status = Todo | Done

label(s: Status) -> str {
    match s {
        Todo => \"todo\"
        Done => \"done\"
    }
}

main() -> int {
    info(label(Done))
    info(str(6 * 7))
    0
}
";
    let Some(out) = run(src) else { return };
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["done", "42"]);
}
