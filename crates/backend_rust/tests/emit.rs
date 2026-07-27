//! Hermetic tests for the Rust backend: assert the emitted Rust source (no
//! rustc needed; the driver's `rust_backend` test covers real compilation).

fn rs(src: &str) -> String {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    maca_backend_rust::emit(&p.module)
}

#[test]
fn main_exits_with_the_returned_code() {
    let out = rs("main() -> int {\n    info(\"hi\")\n    0\n}\n");
    assert!(out.contains("fn __maca_main() -> i64"), "{out}");
    assert!(
        out.contains("std::process::exit(__maca_main() as i32)"),
        "{out}"
    );
    assert!(out.contains("println!"), "{out}");
}

#[test]
fn functions_types_and_recursion() {
    let out = rs("fib(n: int) -> int =>\n    n < 2 ? n : fib(n - 1) + fib(n - 2)\n");
    assert!(out.contains("fn fib(mut n: i64) -> i64"), "{out}");
    assert!(out.contains("if (n < 2i64)"), "{out}");
}

#[test]
fn records_become_structs_sums_become_enums() {
    let out = rs("Color = Red | Green | Blue\nPoint = {\n    x: int\n    y: int\n}\n");
    assert!(out.contains("enum Color { Red, Green, Blue }"), "{out}");
    assert!(out.contains("struct Point {"), "{out}");
    assert!(out.contains("x: i64,") && out.contains("y: i64,"), "{out}");
}

#[test]
fn payload_sum_becomes_a_data_enum() {
    // A variant with a payload — `Circle(int)` — must become `Circle(i64)`, not
    // be mis-parsed as an addition of function calls. Construction and match
    // arms are both qualified `Enum::Variant`.
    let out = rs("Shape = Circle(int) | Rect(int, int)\n\
         area(s: Shape) -> int =>\n    match s {\n        Circle(r) => r * r\n        Rect(w, h) => w * h\n    }\n\n\
         main() -> int { area(Circle(5)) }\n");
    assert!(
        out.contains("enum Shape { Circle(i64), Rect(i64, i64) }"),
        "payload not lowered to a data enum: {out}"
    );
    assert!(
        !out.contains("static SHAPE"),
        "type declaration mis-emitted as a constant: {out}"
    );
    assert!(
        out.contains("Shape::Circle(5i64)"),
        "ctor not qualified: {out}"
    );
    assert!(
        out.contains("Shape::Rect(w, h) =>"),
        "pattern not qualified: {out}"
    );
}

#[test]
fn record_payload_sum_and_value_reuse() {
    // A record-carrying variant (gpql's `Outcome = Rows(Grid) | Affected(int)`)
    // needs the struct to derive `PartialEq` too, and a value used twice must be
    // cloned rather than moved.
    let out = rs("Grid = {\n    rows: int\n}\n\
         Outcome = Rows(Grid) | Affected(int)\n\
         rows_of(o: Outcome) -> int =>\n    match o {\n        Rows(g) => g.rows\n        Affected(n) => n\n    }\n\n\
         main() -> int {\n    r = Rows(Grid { rows = 7 })\n    info(\"{rows_of(r)}\")\n    rows_of(r)\n}\n");
    assert!(
        out.contains("#[derive(Clone, Debug, PartialEq)]\n#[allow(dead_code)]\nstruct Grid"),
        "struct must derive PartialEq for an enum payload: {out}"
    );
    assert!(out.contains("Outcome::Rows("), "ctor not qualified: {out}");
    assert!(
        out.contains("rows_of(r.clone())"),
        "reused local not cloned (would move): {out}"
    );
}

#[test]
fn foreign_type_calls_use_associated_path() {
    // Maca has no `::` surface syntax: a call on a foreign (capitalized,
    // non-local) type is its constructor, and `Type.assoc(a)` is an associated
    // function. Integer literals in foreign-call position drop the `i64` suffix
    // so Rust infers the parameter type (u64 here).
    let out = rs("import rust \"std::time::Duration\"\n\n\
         main() -> int {\n    d = Duration.from_secs(5)\n    b = Buffer()\n    0\n}\n");
    assert!(out.contains("use std::time::Duration;"), "no use: {out}");
    assert!(
        out.contains("Duration::from_secs(5)") && !out.contains("Duration::from_secs(5i64)"),
        "associated fn / literal suffix: {out}"
    );
    assert!(
        out.contains("Buffer::new()"),
        "ctor not mapped to ::new: {out}"
    );
}

#[test]
fn instance_method_stays_dotted() {
    // a call on a *value* is still an instance method, not an associated path.
    let out = rs("main() -> int {\n    xs = [1, 2, 3]\n    len(xs)\n}\n");
    assert!(
        !out.contains("xs::"),
        "instance receiver mis-qualified: {out}"
    );
}

#[test]
fn variant_reference_is_qualified() {
    // a bare `Green` must emit `Color::Green`, and a match arm likewise.
    let out = rs(
        "Color = Red | Green | Blue\nrank(c: Color) -> int =>\n    match c {\n        Red => 0\n        _ => 1\n    }\n\nmain() -> int { rank(Green) }\n",
    );
    assert!(out.contains("Color::Green"), "unqualified variant: {out}");
    assert!(out.contains("Color::Red =>"), "unqualified pattern: {out}");
}

#[test]
fn reassignment_is_not_a_new_binding() {
    // the first `acc = 0` is a `let`, the later `acc = acc + i` a reassignment,
    // else the loop variable never changes (an infinite loop).
    let out = rs(
        "sum_to(n: int) -> int {\n    acc = 0\n    i = 1\n    while i <= n {\n        acc = acc + i\n        i = i + 1\n    }\n    acc\n}\n",
    );
    assert!(out.contains("let mut acc = 0i64;"), "no initial let: {out}");
    assert!(
        out.contains("acc = (acc + i);") && !out.contains("let mut acc = (acc + i)"),
        "reassignment emitted as a new binding: {out}"
    );
}

#[test]
fn list_and_len() {
    let out = rs("main() -> int {\n    xs = [1, 2, 3]\n    len(xs)\n}\n");
    assert!(out.contains("vec![1i64, 2i64, 3i64]"), "{out}");
    assert!(out.contains("maca_len(&xs)"), "{out}");
}
