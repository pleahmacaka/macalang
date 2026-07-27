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
