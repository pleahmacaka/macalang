//! What the embedded target lowers, and what it refuses by name.
//!
//! A freestanding MCU has no allocator, no libc and no scheduler, so a good
//! part of the language genuinely does not fit. The bug these tests guard is
//! not that it does not fit; it is that not fitting used to be silent: an
//! unhandled expression became the literal `0u`, which C accepts anywhere a
//! `uint32_t` is wanted, so a `match` in firmware compiled to the number zero.

fn emit(src: &str) -> Result<String, Vec<String>> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    maca_backend_embedded::emit_c_checked(&p.module)
}

/// The C for a module that must lower cleanly.
fn ok(src: &str) -> String {
    match emit(src) {
        Ok(c) => c,
        Err(p) => panic!("unexpected refusal: {p:?}"),
    }
}

/// The refusal messages for a module that must not lower.
fn refused(src: &str) -> String {
    match emit(src) {
        Ok(c) => panic!("expected a refusal, got C:\n{c}"),
        Err(p) => p.join("\n"),
    }
}

#[test]
fn a_field_store_is_emitted_not_dropped() {
    // Only the right-hand side used to reach the output, as `v;`, a statement
    // C accepts in silence, so the store simply never happened.
    let c = ok("set(p: int, v: int) {\n    p.reg = v\n}\n");
    assert!(c.contains("p.reg = v;"), "no store in:\n{c}");
}

#[test]
fn an_index_store_is_emitted_not_dropped() {
    let c = ok("set(xs: int, i: int, v: int) {\n    xs[i] = v\n}\n");
    assert!(c.contains("xs[i] = v;"), "no store in:\n{c}");
}

#[test]
fn a_store_is_the_last_thing_on_its_line() {
    // Guards the shape as well as the presence: `v;` also "contains" `v`.
    let c = ok("set(p: int, v: int) {\n    p.reg = v + 1\n}\n");
    assert!(
        c.lines().any(|l| l.trim() == "p.reg = (v + 1u);"),
        "store is not a whole statement:\n{c}"
    );
}

#[test]
fn concat_is_refused_rather_than_becoming_addition() {
    // `++` fell through the operator table's `_ => "+"`, so a concatenation
    // became pointer arithmetic.
    let msg = refused("f(a: int, b: int) -> int {\n    a ++ b\n}\n");
    assert!(msg.contains("++"), "message does not name `++`: {msg}");
    assert!(
        msg.contains("allocator") || msg.contains("heap"),
        "message does not say why: {msg}"
    );
}

#[test]
fn match_is_refused_rather_than_becoming_zero() {
    let msg =
        refused("f(n: int) -> int {\n    match n {\n        1 => 2\n        _ => 3\n    }\n}\n");
    assert!(
        msg.contains("match"),
        "message does not name `match`: {msg}"
    );
}

#[test]
fn a_float_is_refused_rather_than_becoming_zero() {
    let msg = refused("f() -> int {\n    x = 1.5\n    x\n}\n");
    assert!(msg.contains("float"), "message does not name it: {msg}");
}

#[test]
fn a_string_is_refused_rather_than_becoming_zero() {
    let msg = refused("f() -> int {\n    s = \"hi\"\n    0\n}\n");
    assert!(msg.contains("string"), "message does not name it: {msg}");
}

#[test]
fn a_list_is_refused_rather_than_becoming_zero() {
    let msg = refused("f() -> int {\n    xs = [1, 2]\n    0\n}\n");
    assert!(msg.contains("list"), "message does not name it: {msg}");
}

#[test]
fn a_sum_declaration_is_refused_rather_than_becoming_a_const() {
    // `Color = Red | Green` lowered to `static const uint32_t Color = (Red |
    // Green);`, C naming two identifiers that do not exist.
    let msg = refused("Color = Red | Green\n\nf() -> int {\n    0\n}\n");
    assert!(msg.contains("sum type"), "message does not say what: {msg}");
    assert!(msg.contains("Color"), "message does not name it: {msg}");
}

#[test]
fn a_record_declaration_is_refused_rather_than_becoming_a_const() {
    let msg = refused("Reg = { addr: int }\n\nf() -> int {\n    0\n}\n");
    assert!(msg.contains("record"), "message does not say what: {msg}");
    assert!(msg.contains("Reg"), "message does not name it: {msg}");
}

#[test]
fn integer_arithmetic_still_lowers() {
    // The refusals must not have swallowed the subset that does work.
    let c = ok("f(a: int, b: int) -> int {\n    (a << 2) | (b % 3)\n}\n");
    assert!(c.contains("<<") && c.contains('%'), "lost operators:\n{c}");
}

#[test]
fn every_refusal_names_a_construct_rather_than_generated_code() {
    // A message about `0u` or a C type would be about code the author never
    // wrote, the failure mode the whole limitation channel exists to avoid.
    for src in [
        "f(n: int) -> int {\n    match n {\n        1 => 2\n        _ => 3\n    }\n}\n",
        "f() -> int {\n    xs = [1, 2]\n    0\n}\n",
        "f(a: int, b: int) -> int {\n    a ++ b\n}\n",
    ] {
        let msg = refused(src);
        assert!(
            !msg.contains("0u") && !msg.contains("uint32_t"),
            "refusal talks about generated code: {msg}"
        );
    }
}
