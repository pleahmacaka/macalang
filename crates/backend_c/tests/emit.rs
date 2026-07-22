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

#[test]
fn fail_lowers_to_maca_fail_not_abort() {
    let out = c("g(n: int) -> int {\n    if n < 0 { fail \"bad\" }\n    n\n}\n");
    assert!(out.contains("maca_fail("), "fail should call maca_fail:\n{out}");
    assert!(!out.contains("abort()"), "fail must not abort:\n{out}");
}

#[test]
fn match_guard_and_int_patterns() {
    let body = func(
        "classify(n: int) -> str {\n    match n {\n        x if x < 0 => \"neg\"\n        0 => \"zero\"\n        _ => \"other\"\n    }\n}\n",
        "classify",
    );
    // the guard condition is emitted (not dropped)
    assert!(body.contains("< 0)"), "guard condition missing:\n{body}");
    // an integer-literal pattern lowers to an equality test, not a catch-all
    assert!(body.contains("== 0"), "int pattern not tested:\n{body}");
    // guarded matches fall through via goto
    assert!(body.contains("goto"), "no fall-through for guards:\n{body}");
}

#[test]
fn string_literal_match_uses_str_eq() {
    let body = func(
        "route(cmd: str) -> int {\n    match cmd {\n        \"add\" => 1\n        _ => 0\n    }\n}\n",
        "route",
    );
    assert!(body.contains("maca_str_eq"), "string pattern not compared:\n{body}");
    assert!(body.contains("if ("), "first arm must be a real if, not else:\n{body}");
}

#[test]
fn or_patterns_combine_with_logical_or() {
    let out = c("C = A | B | D\nf(c: C) -> int {\n    match c {\n        A | B => 1\n        D => 2\n    }\n}\n");
    // an or-pattern's alternatives are OR'd (each a tag test)
    assert!(out.contains("== C_A") && out.contains("== C_B"), "or alts missing:\n{out}");
    assert!(out.contains("||"), "alternatives not combined with ||:\n{out}");
}

#[test]
fn float_literal_pattern() {
    let body = func("f(x: float) -> int {\n    match x {\n        1.5 => 1\n        _ => 0\n    }\n}\n", "f");
    assert!(body.contains("== 1.5"), "float pattern not compared:\n{body}");
}

#[test]
fn payload_sum_is_a_tagged_union() {
    let out = c("Shape = Circle(int) | Rect(int, int)\narea(s: Shape) -> int {\n    match s {\n        Circle(r) => r * r\n        Rect(w, h) => w * h\n    }\n}\n");
    // tagged struct + tag enum + per-variant constructor
    assert!(out.contains("Shape_tag"), "no tag enum:\n{out}");
    assert!(out.contains("union"), "no union:\n{out}");
    assert!(out.contains("static Shape Shape_Circle(int64_t _0)"), "no ctor:\n{out}");
    // match extracts payload from the union and tag-tests
    assert!(out.contains(".tag == Shape_tag_Circle"), "no tag test:\n{out}");
    assert!(out.contains(".as.Circle._0"), "no payload extraction:\n{out}");
}

#[test]
fn tagged_sum_with_record_payload_orders_record_first() {
    // A sum carrying a record payload must have the record's struct defined
    // *before* the tagged-sum struct, even when the sum is declared first in
    // source (regression: combined records+sums topo order).
    let out = c("Shape = Dot | At(P)\nP = {\n    x: int\n    y: int\n}\nf(s: Shape) -> int {\n    match s {\n        At(p) => p.x\n        Dot => 0\n    }\n}\n");
    let p_at = out.find("} P;").expect("no P struct");
    let shape_at = out.find("} Shape;").expect("no Shape struct");
    assert!(p_at < shape_at, "record P must be emitted before Shape:\n{out}");
    // the payload field is the record by value, not int64_t
    assert!(out.contains("P _0;"), "payload not typed as record P:\n{out}");
}

#[test]
fn record_with_tagged_sum_field_orders_sum_first() {
    // The reverse dependency: a record field whose type is a tagged sum must
    // have the sum struct defined before the record struct.
    let out = c("Holder = {\n    shape: Shape\n}\nShape = Dot | At(int)\n");
    let shape_at = out.find("} Shape;").expect("no Shape struct");
    let holder_at = out.find("} Holder;").expect("no Holder struct");
    assert!(shape_at < holder_at, "sum Shape must be emitted before Holder:\n{out}");
    assert!(out.contains("Shape shape;"), "field not typed as sum Shape:\n{out}");
}

#[test]
fn reify_installs_a_handler() {
    let out = c("boom() -> int {\n    fail \"x\"\n    0\n}\nmain() -> int {\n    try boom()\n    0\n}\n");
    assert!(out.contains("maca_try_push("), "no handler push:\n{out}");
    assert!(out.contains("setjmp("), "no setjmp:\n{out}");
    assert!(out.contains("maca_last_fail()"), "no caught-message read:\n{out}");
}

#[test]
fn non_capturing_lambda_is_hoisted() {
    let out = c("main() -> int {\n    let xs = 1, 2, 3\n    let ys = xs.parallel(v => v + 1)\n    0\n}\n");
    assert!(out.contains("static int64_t _lam0(int64_t v)"), "lambda not hoisted:\n{out}");
    assert!(out.contains("return (v + 1);"), "lambda body wrong:\n{out}");
    assert!(out.contains("_lam0, 4)") || out.contains("_lam0,4)"), "lambda not passed to parallel:\n{out}");
    assert!(!out.contains("unsupported"), "should be supported:\n{out}");
}

#[test]
fn capturing_lambda_is_flagged_not_miscompiled() {
    let out = c("main() -> int {\n    let k = 3\n    let xs = 1, 2\n    let ys = xs.parallel(v => v * k)\n    0\n}\n");
    assert!(out.contains("unsupported: capturing lambda"), "capture not flagged:\n{out}");
}

#[test]
fn generic_fn_is_monomorphized() {
    let out = c("id(x: a) -> a => x\nBox = {\n    v: int\n}\nmain() -> int {\n    let n: int = id(42)\n    let b: Box = id(Box { v = 7 })\n    let s: str = id(\"hi\")\n    0\n}\n");
    // one specialized copy per distinct instantiation, each with the right C type
    assert!(out.contains("int64_t id__int(int64_t x)"), "no int specialization:\n{out}");
    assert!(out.contains("maca_str id__str(maca_str x)"), "no str specialization:\n{out}");
    assert!(out.contains("Box id__Box(Box x)"), "no record specialization:\n{out}");
    // calls are rewritten to the mangled names
    assert!(out.contains("id__int(42)"), "int call not rewritten:\n{out}");
    // the generic template itself is not emitted as a single int64_t function
    assert!(!out.contains("int64_t id(int64_t x)"), "generic emitted monomorphically:\n{out}");
}

#[test]
fn list_index_reads_backing_buffer() {
    let body = func("f(xs: int[]) -> int => xs[1]\n", "f");
    assert!(body.contains(".data[1]"), "index not lowered to buffer access:\n{body}");
    assert!(!body.contains("unsupported"), "still unsupported:\n{body}");
}

#[test]
fn string_index_uses_str_at() {
    let body = func("f(s: str) -> str => s[0]\n", "f");
    assert!(body.contains("maca_str_at("), "string index not lowered:\n{body}");
}

#[test]
fn index_and_field_assignment_are_lvalues() {
    // `xs[i] = v` and `p.f = v` must write through the lvalue, not no-op
    let body = func(
        "P = {\n    x: int\n}\nf(xs: int[], p: P) -> int {\n    xs[0] = 9\n    p.x = 5\n    0\n}\n",
        "f",
    );
    assert!(body.contains(".data[0] = 9;"), "element assign missing:\n{body}");
    assert!(body.contains(".x = 5;"), "field assign missing:\n{body}");
}

#[test]
fn record_update_copies_and_overwrites() {
    // `base with { … }` must copy the struct and assign only the named fields,
    // not miscompile to `0 /* unsupported */`.
    let body = func(
        "P = {\n    x: int\n    y: int\n}\nf(p: P) -> P => p with { x = 9 }\n",
        "f",
    );
    assert!(body.contains("P _t"), "no struct copy temp:\n{body}");
    assert!(body.contains(".x = 9;"), "field not overwritten:\n{body}");
    assert!(!body.contains(".y ="), "untouched field must not be assigned:\n{body}");
    assert!(!body.contains("unsupported"), "still unsupported:\n{body}");
}

#[test]
fn record_pattern_binds_fields() {
    let body = func("P = {\n    x: int\n    y: int\n}\nf(p: P) -> int {\n    match p {\n        { x, y } => x + y\n    }\n}\n", "f");
    assert!(body.contains(".x;") && body.contains(".y;"), "fields not extracted:\n{body}");
    assert!(body.contains("if (1)"), "irrefutable first arm must be if(1), not else:\n{body}");
}

#[test]
fn string_concat_uses_maca_concat() {
    let out = c("g(n: str) -> str => \"hi \" ++ n\n");
    assert!(out.contains("maca_concat("), "string ++ should use maca_concat:\n{out}");
    assert!(!out.contains("IntArr_concat"), "must not be array concat:\n{out}");
}

#[test]
fn unary_not_and_forward_record() {
    let out = c("A = {\n    b: B\n}\nB = {\n    v: int\n}\nf(x: bool) -> bool => !x\nmain() -> int {\n    let a = A { b = B { v = 1 } }\n    0\n}\n");
    assert!(out.contains("(!x)"), "no unary not:\n{out}");
    assert!(out.contains("B b;"), "forward record field not resolved to struct:\n{out}");
}
