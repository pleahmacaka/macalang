//! Records on the JVM: naming, constructor order, and field reads.
//!
//! A Java `record` keeps its components private and its constructor is
//! positional, and neither of those matched what the emitter did. The values
//! were passed in the order the literal wrote them, so `{ y = 2, x = 1 }` became
//! `Point(2, 1)` with the fields swapped and nothing saying so, and a field read
//! emitted `p.x` where a record exposes `p.x()`.
//!
//! The third case came from the checker learning to unify an anonymous literal
//! with a named record of the same shape: the emitter's advice to "declare a
//! record type" was impossible to follow on a program where the record was
//! declared and the literal written straight into it.

use std::process::Command;

fn emit(src: &str, class: &str) -> Result<String, Vec<String>> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    maca_backend_jvm::emit_checked(&p.module, class, None)
}

fn ok(src: &str, class: &str) -> String {
    match emit(src, class) {
        Ok(s) => s,
        Err(e) => panic!("unexpected refusal: {e:?}"),
    }
}

fn refused(src: &str, class: &str) -> String {
    match emit(src, class) {
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

/// Compile the emitted Java and run its `main`, returning stdout lines.
fn run(src: &str, class: &str) -> Option<Vec<String>> {
    if !have_jdk() {
        eprintln!("skipping: no JDK");
        return None;
    }
    let java = ok(src, class);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&src, &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-jvm-rec-{}-{key:x}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{class}.java")), &java).unwrap();
    let c = Command::new("javac")
        .arg(format!("{class}.java"))
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        c.status.success(),
        "javac failed\n{}\n--- java ---\n{java}",
        String::from_utf8_lossy(&c.stderr)
    );
    let o = Command::new("java")
        .arg(class)
        .current_dir(&dir)
        .output()
        .unwrap();
    Some(
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.to_string())
            .collect(),
    )
}

#[test]
fn a_record_constructor_follows_the_declaration_not_the_literal() {
    // A Java record's constructor is positional. Passing the values in the order
    // they were written put `{ y = 2, x = 1 }` into `Point(2, 1)`, so the fields
    // were swapped and nothing said so.
    let src = "\
Point = { x: int, y: int }

flipped() -> Point => Point { y = 2, x = 1 }

main() -> int {
    p = flipped()
    info(\"x={p.x} y={p.y}\")
    0
}
";
    let java = ok(src, "J");
    assert!(java.contains("new Point(1L, 2L)"), "wrong order:\n{java}");
    let Some(out) = run(src, "J") else { return };
    assert_eq!(out, vec!["x=1 y=2"]);
}

#[test]
fn a_record_field_is_read_through_its_accessor() {
    // A Java record keeps its components private, so `p.x` does not compile.
    let src = "\
Point = { x: int, y: int }

main() -> int {
    p = Point { x = 3, y = 4 }
    info(str(p.y))
    0
}
";
    let java = ok(src, "K");
    assert!(java.contains(".y()"), "not an accessor:\n{java}");
    let Some(out) = run(src, "K") else { return };
    assert_eq!(out, vec!["4"]);
}

#[test]
fn an_anonymous_literal_takes_the_declared_records_name() {
    let src = "\
Point = { x: int, y: int }

corner() -> Point {
    { x = 1, y = 2 }
}

main() -> int {
    info(str(corner().x))
    0
}
";
    let java = ok(src, "L");
    assert!(
        java.contains("new Point("),
        "did not name the record:\n{java}"
    );
    let Some(out) = run(src, "L") else { return };
    assert_eq!(out, vec!["1"]);
}

#[test]
fn a_literal_missing_a_declared_field_is_refused() {
    // There is no value to pass for the component the literal never wrote.
    let msg = refused(
        "Point = { x: int, y: int }\n\nhalf() -> Point => Point { x = 1 }\n\nmain() -> int => 0\n",
        "M",
    );
    assert!(msg.contains('y'), "does not name the field: {msg}");
}
