//! Hermetic tests for the C backend: assert the emitted C source directly, with
//! no compiler/WSL required (the `driver` run tests cover actual execution).

fn c(src: &str) -> String {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    maca_backend_c::emit(&p.module)
}

/// Return the body of function `name` from emitted C (between its `{` and the
/// matching top-level `}`), for focused assertions.
fn func(src: &str, name: &str) -> String {
    let out = c(src);
    let sig = out.match_indices(&format!("{name}(")).find_map(|(i, _)| {
        let rest = &out[i..];
        // the definition, not the forward declaration (which ends in `;`)
        rest.find('{').and_then(|b| {
            if rest[..b].contains(';') { None } else { Some(&rest[b..]) }
        })
    });
    sig.unwrap_or("").to_string()
}

#[test]
fn value_position_if_declares_then_assigns() {
    // regression: `let x = if …` must not silently become `x = 0`
    let body = func(
        "pick(c: bool) -> int {\n    let b = if c { 100 } else { 200 }\n    b\n}\n",
        "pick",
    );
    assert!(body.contains("int64_t b;"), "temp not declared:\n{body}");
    assert!(body.contains("b = 100;") && body.contains("b = 200;"), "branches don't assign:\n{body}");
    assert!(!body.contains("unsupported"), "still unsupported:\n{body}");
}

#[test]
fn value_position_ternary() {
    let body = func("f(c: bool) -> int {\n    let a = c ? 10 : 20\n    a\n}\n", "f");
    assert!(body.contains("int64_t a = (c ? 10 : 20);"), "{body}");
}

#[test]
fn enum_match_is_a_tag_test() {
    // regression: nullary variant patterns must compare tags, not bind + `else`
    let src = "Color = Red | Green | Blue\n\nscore(x: Color) -> int {\n    match x {\n        Red => 1\n        Green => 2\n        Blue => 3\n    }\n}\n";
    let body = func(src, "score");
    assert!(body.contains("== Color_Red"), "no tag test:\n{body}");
    assert!(body.contains("== Color_Green") && body.contains("== Color_Blue"), "{body}");
    // first arm must be a real `if`, not a bare `else`
    assert!(body.contains("if (") , "{body}");
    assert!(!body.contains("Color Red = "), "variant bound as variable:\n{body}");
}

#[test]
fn sum_and_record_types() {
    let out = c("Status = Todo | Done\nPoint = {\n    x: int\n    y: int\n}\n");
    assert!(out.contains("enum") || out.contains("Status_Todo"), "no enum for sum:\n{out}");
    assert!(out.contains("Point") && out.contains("x") && out.contains("y"), "no struct for record:\n{out}");
}

#[test]
fn string_interpolation_builds_a_string() {
    let out = c("main() -> int {\n    let n = 5\n    info(\"n is {n}\")\n    0\n}\n");
    // interpolation lowers through the maca_str / fmt runtime, not a bare literal
    assert!(out.contains("maca_") && out.contains("n is"), "{out}");
}

#[test]
fn needs_async_detection() {
    let plain = c("main() -> int { 0 }");
    assert!(!maca_backend_c::needs_async(&plain), "plain program should not need async");
}

#[test]
fn while_loop_and_reassignment() {
    let body = func(
        "sum_to(n: int) -> int {\n    let acc = 0\n    let i = 1\n    while i <= n {\n        acc = acc + i\n        i = i + 1\n    }\n    acc\n}\n",
        "sum_to",
    );
    assert!(body.contains("while ((i <= n))"), "no while:\n{body}");
    assert!(body.contains("acc = (acc + i);"), "no reassignment:\n{body}");
    assert!(body.contains("i = (i + 1);"), "counter not updated:\n{body}");
}

#[test]
fn break_and_continue() {
    let body = func(
        "f() -> int {\n    let i = 0\n    while i < 10 {\n        i = i + 1\n        if i < 3 { continue }\n        break\n    }\n    i\n}\n",
        "f",
    );
    assert!(body.contains("break;"), "no break:\n{body}");
    assert!(body.contains("continue;"), "no continue:\n{body}");
}

#[test]
fn modulo_and_shift_operators() {
    let body = func("f(n: int) -> int {\n    let a = n % 3\n    let b = n << 2\n    let c = n >> 1\n    a + b + c\n}\n", "f");
    assert!(body.contains("(n % 3)"), "no modulo:\n{body}");
    assert!(body.contains("(n << 2)"), "no shl:\n{body}");
    assert!(body.contains("(n >> 1)"), "no shr:\n{body}");
}
