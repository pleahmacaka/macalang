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
            if rest[..b].contains(';') {
                None
            } else {
                Some(&rest[b..])
            }
        })
    });
    sig.unwrap_or("").to_string()
}

#[test]
fn value_position_if_declares_then_assigns() {
    // regression: `let x = if …` must not silently become `x = 0`
    let body = func(
        "pick(c: bool) -> int {\n    b = if c { 100 } else { 200 }\n    b\n}\n",
        "pick",
    );
    assert!(body.contains("int64_t b;"), "temp not declared:\n{body}");
    assert!(
        body.contains("b = 100;") && body.contains("b = 200;"),
        "branches don't assign:\n{body}"
    );
    assert!(!body.contains("unsupported"), "still unsupported:\n{body}");
}

#[test]
fn value_position_ternary() {
    let body = func("f(c: bool) -> int {\n    a = c ? 10 : 20\n    a\n}\n", "f");
    assert!(body.contains("int64_t a = (c ? 10 : 20);"), "{body}");
}

#[test]
fn enum_match_is_a_tag_test() {
    // regression: nullary variant patterns must compare tags, not bind + `else`
    let src = "Color = Red | Green | Blue\n\nscore(x: Color) -> int {\n    match x {\n        Red => 1\n        Green => 2\n        Blue => 3\n    }\n}\n";
    let body = func(src, "score");
    assert!(body.contains("== Color_Red"), "no tag test:\n{body}");
    assert!(
        body.contains("== Color_Green") && body.contains("== Color_Blue"),
        "{body}"
    );
    // first arm must be a real `if`, not a bare `else`
    assert!(body.contains("if ("), "{body}");
    assert!(
        !body.contains("Color Red = "),
        "variant bound as variable:\n{body}"
    );
}

#[test]
fn sum_and_record_types() {
    let out = c("Status = Todo | Done\nPoint = {\n    x: int\n    y: int\n}\n");
    assert!(
        out.contains("enum") || out.contains("Status_Todo"),
        "no enum for sum:\n{out}"
    );
    assert!(
        out.contains("Point") && out.contains("x") && out.contains("y"),
        "no struct for record:\n{out}"
    );
}

#[test]
fn string_interpolation_builds_a_string() {
    let out = c("main() -> int {\n    n = 5\n    info(\"n is {n}\")\n    0\n}\n");
    // interpolation lowers through the maca_str / fmt runtime, not a bare literal
    assert!(out.contains("maca_") && out.contains("n is"), "{out}");
}

#[test]
fn needs_async_detection() {
    let plain = c("main() -> int { 0 }");
    assert!(
        !maca_backend_c::needs_async(&plain),
        "plain program should not need async"
    );
}

#[test]
fn while_loop_and_reassignment() {
    let body = func(
        "sum_to(n: int) -> int {\n    acc = 0\n    i = 1\n    while i <= n {\n        acc = acc + i\n        i = i + 1\n    }\n    acc\n}\n",
        "sum_to",
    );
    assert!(body.contains("while ((i <= n))"), "no while:\n{body}");
    assert!(
        body.contains("acc = (acc + i);"),
        "no reassignment:\n{body}"
    );
    assert!(
        body.contains("i = (i + 1);"),
        "counter not updated:\n{body}"
    );
}

#[test]
fn break_and_continue() {
    let body = func(
        "f() -> int {\n    i = 0\n    while i < 10 {\n        i = i + 1\n        if i < 3 { continue }\n        break\n    }\n    i\n}\n",
        "f",
    );
    assert!(body.contains("break;"), "no break:\n{body}");
    assert!(body.contains("continue;"), "no continue:\n{body}");
}

#[test]
fn modulo_and_shift_operators() {
    let body = func(
        "f(n: int) -> int {\n    a = n % 3\n    b = n << 2\n    c = n >> 1\n    a + b + c\n}\n",
        "f",
    );
    assert!(body.contains("(n % 3)"), "no modulo:\n{body}");
    assert!(body.contains("(n << 2)"), "no shl:\n{body}");
    assert!(body.contains("(n >> 1)"), "no shr:\n{body}");
}

#[test]
fn fail_lowers_to_maca_fail_not_abort() {
    let out = c("g(n: int) -> int {\n    if n < 0 { fail \"bad\" }\n    n\n}\n");
    assert!(
        out.contains("maca_fail("),
        "fail should call maca_fail:\n{out}"
    );
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
    assert!(
        body.contains("maca_str_eq"),
        "string pattern not compared:\n{body}"
    );
    assert!(
        body.contains("if ("),
        "first arm must be a real if, not else:\n{body}"
    );
}

#[test]
fn or_patterns_combine_with_logical_or() {
    let out = c(
        "C = A | B | D\nf(c: C) -> int {\n    match c {\n        A | B => 1\n        D => 2\n    }\n}\n",
    );
    // an or-pattern's alternatives are OR'd (each a tag test)
    assert!(
        out.contains("== C_A") && out.contains("== C_B"),
        "or alts missing:\n{out}"
    );
    assert!(
        out.contains("||"),
        "alternatives not combined with ||:\n{out}"
    );
}

#[test]
fn float_literal_pattern() {
    let body = func(
        "f(x: float) -> int {\n    match x {\n        1.5 => 1\n        _ => 0\n    }\n}\n",
        "f",
    );
    assert!(
        body.contains("== 1.5"),
        "float pattern not compared:\n{body}"
    );
}

#[test]
fn payload_sum_is_a_tagged_union() {
    let out = c(
        "Shape = Circle(int) | Rect(int, int)\narea(s: Shape) -> int {\n    match s {\n        Circle(r) => r * r\n        Rect(w, h) => w * h\n    }\n}\n",
    );
    // tagged struct + tag enum + per-variant constructor
    assert!(out.contains("Shape_tag"), "no tag enum:\n{out}");
    assert!(out.contains("union"), "no union:\n{out}");
    assert!(
        out.contains("static Shape Shape_Circle(int64_t _0)"),
        "no ctor:\n{out}"
    );
    // match extracts payload from the union and tag-tests
    assert!(
        out.contains(".tag == Shape_tag_Circle"),
        "no tag test:\n{out}"
    );
    assert!(
        out.contains(".as.Circle._0"),
        "no payload extraction:\n{out}"
    );
}

#[test]
fn recursive_sum_boxes_self_referential_payload() {
    // In `Tree = Leaf(int) | Node(Tree, Tree)` the recursive payload must be a
    // pointer (`Tree*`), heap-allocated in the constructor, and dereferenced
    // when a match binds it. Otherwise the struct is infinitely sized.
    let out = c(
        "Tree = Leaf(int) | Node(Tree, Tree)\ntotal(t: Tree) -> int {\n    match t {\n        Leaf(n) => n\n        Node(l, r) => total(l) + total(r)\n    }\n}\n",
    );
    // named forward-declared struct (so a self-pointer is legal)
    assert!(
        out.contains("typedef struct Tree Tree;"),
        "no forward decl:\n{out}"
    );
    assert!(out.contains("struct Tree {"), "not a named struct:\n{out}");
    // the payload slot is a pointer, allocated in the constructor
    assert!(
        out.contains("Tree* _0;") && out.contains("Tree* _1;"),
        "payload not boxed:\n{out}"
    );
    assert!(
        out.contains("maca_alloc(sizeof(Tree))"),
        "box not heap-allocated:\n{out}"
    );
    // a bound recursive payload is dereferenced
    assert!(out.contains("= *"), "boxed bind not dereferenced:\n{out}");
    // a non-recursive int payload stays by value
    assert!(
        out.contains("int64_t _0;"),
        "int payload should be by value:\n{out}"
    );
}

#[test]
fn tagged_sum_with_record_payload_orders_record_first() {
    // A sum carrying a record payload must have the record's struct defined
    // *before* the tagged-sum struct, even when the sum is declared first in
    // source (regression: combined records+sums topo order).
    let out = c(
        "Shape = Dot | At(P)\nP = {\n    x: int\n    y: int\n}\nf(s: Shape) -> int {\n    match s {\n        At(p) => p.x\n        Dot => 0\n    }\n}\n",
    );
    let p_at = out.find("} P;").expect("no P struct");
    let shape_at = out.find("} Shape;").expect("no Shape struct");
    assert!(
        p_at < shape_at,
        "record P must be emitted before Shape:\n{out}"
    );
    // the payload field is the record by value, not int64_t
    assert!(
        out.contains("P _0;"),
        "payload not typed as record P:\n{out}"
    );
}

#[test]
fn record_with_tagged_sum_field_orders_sum_first() {
    // The reverse dependency: a record field whose type is a tagged sum must
    // have the sum struct defined before the record struct.
    let out = c("Holder = {\n    shape: Shape\n}\nShape = Dot | At(int)\n");
    let shape_at = out.find("} Shape;").expect("no Shape struct");
    let holder_at = out.find("} Holder;").expect("no Holder struct");
    assert!(
        shape_at < holder_at,
        "sum Shape must be emitted before Holder:\n{out}"
    );
    assert!(
        out.contains("Shape shape;"),
        "field not typed as sum Shape:\n{out}"
    );
}

#[test]
fn reify_installs_a_handler() {
    let out =
        c("boom() -> int {\n    fail \"x\"\n    0\n}\nmain() -> int {\n    try boom()\n    0\n}\n");
    assert!(out.contains("maca_try_push("), "no handler push:\n{out}");
    assert!(out.contains("setjmp("), "no setjmp:\n{out}");
    assert!(
        out.contains("maca_last_fail()"),
        "no caught-message read:\n{out}"
    );
}

#[test]
fn non_capturing_lambda_is_a_closure() {
    let out = c("main() -> int {\n    xs = 1, 2, 3\n    ys = xs.parallel(v => v + 1)\n    0\n}\n");
    // a lambda lowers to a hoisted fn taking the closure env + a boxed arg
    assert!(
        out.contains("static int64_t _lam0(void* _envp, int64_t _a0)"),
        "lambda not a closure:\n{out}"
    );
    assert!(out.contains("(v + 1)"), "lambda body wrong:\n{out}");
    // non-capturing → a NULL environment
    assert!(
        out.contains("NULL }"),
        "non-capturing closure should have a null env:\n{out}"
    );
    assert!(!out.contains("unsupported"), "should be supported:\n{out}");
}

#[test]
fn capturing_lambda_builds_an_environment() {
    // a captured outer variable `k` is stored in a heap env, not miscompiled.
    let out = c(
        "main() -> int {\n    k = 3\n    xs = 1, 2\n    ys = xs.parallel(v => v * k)\n    0\n}\n",
    );
    assert!(
        !out.contains("unsupported"),
        "capture wrongly flagged unsupported:\n{out}"
    );
    assert!(out.contains("_lam0_env"), "no capture env struct:\n{out}");
    assert!(
        out.contains("_e->k = k;"),
        "capture not stored into the env:\n{out}"
    );
    assert!(
        out.contains("maca_alloc(sizeof(_lam0_env))"),
        "env not heap-allocated:\n{out}"
    );
}

#[test]
fn generic_fn_is_monomorphized() {
    let out = c(
        "id(x: a) -> a => x\nBox = {\n    v: int\n}\nmain() -> int {\n    n: int = id(42)\n    b: Box = id(Box { v = 7 })\n    s: str = id(\"hi\")\n    0\n}\n",
    );
    // one specialized copy per distinct instantiation, each with the right C type
    assert!(
        out.contains("int64_t id__int(int64_t x)"),
        "no int specialization:\n{out}"
    );
    assert!(
        out.contains("maca_str id__str(maca_str x)"),
        "no str specialization:\n{out}"
    );
    assert!(
        out.contains("Box id__Box(Box x)"),
        "no record specialization:\n{out}"
    );
    // calls are rewritten to the mangled names
    assert!(
        out.contains("id__int(42)"),
        "int call not rewritten:\n{out}"
    );
    // the generic template itself is not emitted as a single int64_t function
    assert!(
        !out.contains("int64_t id(int64_t x)"),
        "generic emitted monomorphically:\n{out}"
    );
}

#[test]
fn c_keyword_identifiers_are_escaped() {
    // a fn/param/var/field named like a C keyword must not emit invalid C
    let out = c(
        "Config = {\n    default: int\n}\ndouble(n: int) -> int => n * 2\nmain() -> int {\n    new = 5\n    c = Config { default = double(new) }\n    info(\"{c.default}\")\n    0\n}\n",
    );
    // the keyword `double` is escaped at definition and call sites
    assert!(out.contains("double_mc("), "fn name not escaped:\n{out}");
    assert!(
        !out.contains("int64_t double("),
        "raw C keyword fn emitted:\n{out}"
    );
    // the field `default` and the var `new` are escaped
    assert!(out.contains("default_mc"), "field/var not escaped:\n{out}");
    assert!(out.contains("new_mc"), "var not escaped:\n{out}");
    // ordinary names are untouched
    assert!(
        out.contains("Config"),
        "type name should be unchanged:\n{out}"
    );
}

#[test]
fn ufcs_call_resolves_return_type() {
    // `x.f()` where `f` returns int must be usable as an int (interpolation
    // wraps it), not fall back to an unknown type
    let out = c(
        "twice(n: int) -> int => n * 2\nmain() -> int {\n    x = 3\n    info(\"{x.twice()}\")\n    0\n}\n",
    );
    // the int return flows into maca_from_int (string interpolation), proving the
    // UFCS call resolved to int rather than an unknown type
    assert!(
        out.contains("maca_from_int(twice(x))"),
        "UFCS int result not formatted:\n{out}"
    );
}

#[test]
fn len_lowers_to_array_len_or_strlen() {
    let al = func("f(xs: int[]) -> int => len(xs)\n", "f");
    assert!(al.contains("(xs).len"), "array len not lowered:\n{al}");
    let sl = func("g(s: str) -> int => len(s)\n", "g");
    assert!(sl.contains("strlen(s)"), "string len not lowered:\n{sl}");
    assert!(
        !c("f(xs: int[]) -> int => len(xs)\n").contains("undefined"),
        "len must resolve"
    );
}

#[test]
fn list_index_reads_backing_buffer() {
    let body = func("f(xs: int[]) -> int => xs[1]\n", "f");
    assert!(
        body.contains(".data[1]"),
        "index not lowered to buffer access:\n{body}"
    );
    assert!(!body.contains("unsupported"), "still unsupported:\n{body}");
}

#[test]
fn string_index_uses_str_at() {
    let body = func("f(s: str) -> str => s[0]\n", "f");
    assert!(
        body.contains("maca_str_at("),
        "string index not lowered:\n{body}"
    );
}

#[test]
fn index_and_field_assignment_are_lvalues() {
    // `xs[i] = v` and `p.f = v` must write through the lvalue, not no-op
    let body = func(
        "P = {\n    x: int\n}\nf(xs: int[], p: P) -> int {\n    xs[0] = 9\n    p.x = 5\n    0\n}\n",
        "f",
    );
    assert!(
        body.contains(".data[0] = 9;"),
        "element assign missing:\n{body}"
    );
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
    assert!(
        !body.contains(".y ="),
        "untouched field must not be assigned:\n{body}"
    );
    assert!(!body.contains("unsupported"), "still unsupported:\n{body}");
}

#[test]
fn record_pattern_binds_fields() {
    let body = func(
        "P = {\n    x: int\n    y: int\n}\nf(p: P) -> int {\n    match p {\n        { x, y } => x + y\n    }\n}\n",
        "f",
    );
    assert!(
        body.contains(".x;") && body.contains(".y;"),
        "fields not extracted:\n{body}"
    );
    assert!(
        body.contains("if (1)"),
        "irrefutable first arm must be if(1), not else:\n{body}"
    );
}

/// `++` on strings is string concatenation, not the array kind. One call
/// carries the whole chain: `a ++ b ++ c` allocates once, where a nested pair
/// of `maca_concat`s built a string for the first `++` and abandoned it.
#[test]
fn string_concat_is_one_call_for_the_whole_chain() {
    let out = c("g(n: str) -> str => \"hi \" ++ n\n");
    assert!(
        out.contains("maca_concat_n(2, \"hi \", n)"),
        "string ++ should concatenate in one call:\n{out}"
    );
    let chain = c("g(a: str, b: str, c: str) -> str => a ++ b ++ c\n");
    assert!(
        chain.contains("maca_concat_n(3, a, b, c)"),
        "a three-way chain is still one call:\n{chain}"
    );
    assert!(
        !out.contains("IntArr_concat"),
        "must not be array concat:\n{out}"
    );
}

#[test]
fn unary_not_and_forward_record() {
    let out = c(
        "A = {\n    b: B\n}\nB = {\n    v: int\n}\nf(x: bool) -> bool => !x\nmain() -> int {\n    a = A { b = B { v = 1 } }\n    0\n}\n",
    );
    assert!(out.contains("(!x)"), "no unary not:\n{out}");
    assert!(
        out.contains("B b;"),
        "forward record field not resolved to struct:\n{out}"
    );
}

#[test]
fn float_and_int_coercions_lower_to_casts() {
    // `float(x)` / `int(x)` are builtin coercions, not calls to a user function
    // (a missing `float` builtin used to emit an undefined `float_mc` reference).
    let out = c("f(px: int, n: int) -> float => float(px) / float(n)\n");
    assert!(out.contains("(double)"), "float() not a cast:\n{out}");
    assert!(
        !out.contains("float_mc"),
        "float() mangled to a call:\n{out}"
    );
    let out2 = c("g(x: float) -> int => int(x)\n");
    assert!(out2.contains("(int64_t)"), "int() not a cast:\n{out2}");
}

#[test]
fn string_stdlib_lowers_to_runtime_calls() {
    // UFCS string methods lower to the maca_* runtime helpers; `split` builds a
    // StrArr from the returned buffer and registers the StrArr typedef.
    let out = c(
        "main() -> int {\n    s = \"a,b\"\n    parts = s.split(\",\")\n    x = s.trim().upper()\n    y = s.contains(\"a\")\n    z = s.replace(\"a\", \"b\")\n    w = s.substr(0, 1)\n    i = s.index_of(\"b\")\n    0\n}\n",
    );
    assert!(out.contains("maca_split("), "split not lowered:\n{out}");
    assert!(
        out.contains("MACA_DEFINE_ARRAY(StrArr, maca_str)"),
        "StrArr not defined:\n{out}"
    );
    assert!(
        out.contains("maca_trim(") && out.contains("maca_upper("),
        "trim/upper not lowered:\n{out}"
    );
    assert!(
        out.contains("maca_contains("),
        "contains not lowered:\n{out}"
    );
    assert!(out.contains("maca_replace("), "replace not lowered:\n{out}");
    assert!(out.contains("maca_substr("), "substr not lowered:\n{out}");
    assert!(
        out.contains("maca_index_of("),
        "index_of not lowered:\n{out}"
    );
}

#[test]
fn spawn_and_await_lower_to_runtime_futures() {
    // colorblind async: `spawn f(x)` -> maca_spawn, `await` -> maca_await, and
    // `sleep_ms` -> maca_sleep_ms. No `async` keyword anywhere.
    let out = c(
        "work(n: int) -> int {\n    sleep_ms(1)\n    n * 2\n}\nmain() -> int {\n    a = spawn work(21)\n    x = await a\n    info(\"{x}\")\n    0\n}\n",
    );
    assert!(
        out.contains("maca_spawn((maca_task_fn)work, (int64_t)(21))"),
        "spawn not lowered:\n{out}"
    );
    assert!(out.contains("maca_await("), "await not lowered:\n{out}");
    assert!(
        out.contains("maca_sleep_ms(1)"),
        "sleep_ms not lowered:\n{out}"
    );
    assert!(
        out.contains("maca_future*"),
        "future type not emitted:\n{out}"
    );
    assert!(
        !out.contains("unsupported"),
        "async miscompiled to unsupported:\n{out}"
    );
}

#[test]
fn closures_capture_and_list_methods_lower() {
    // a capturing lambda becomes a heap env + maca_closure; map/filter/reduce
    // lower to closure calls; sort/sum use the runtime/inline helpers.
    let out = c(
        "main() -> int {\n    xs = 1, 2, 3\n    k = 10\n    a = xs.map(v => v + k)\n    b = xs.filter(v => v > 1)\n    t = xs.reduce(0, (acc, x) => acc + x)\n    s = xs.sort()\n    info(\"{a[0]} {len(b)} {t} {s[0]} {xs.sum()}\")\n    0\n}\n",
    );
    assert!(out.contains("maca_closure"), "no closure type:\n{out}");
    assert!(out.contains("_env"), "no capture env struct:\n{out}");
    assert!(
        out.contains("maca_call1("),
        "map/filter don't call the closure:\n{out}"
    );
    assert!(
        out.contains("maca_call2("),
        "reduce doesn't use a 2-arg closure:\n{out}"
    );
    assert!(out.contains("maca_sort_i64"), "sort not lowered:\n{out}");
    assert!(
        !out.contains("unsupported"),
        "closure/list method miscompiled:\n{out}"
    );
}

#[test]
fn emit_checked_flags_unsupported_instead_of_silent_zero() {
    // `with` on a non-record can't lower; emit_checked must report it, not emit
    // a silently-wrong `0`.
    let src = "main() -> int {\n    x = 5\n    y = x with { a = 1 }\n    0\n}\n";
    let m = maca_parser::parse(src).module;
    let res = maca_backend_c::emit_checked(&m);
    assert!(
        res.is_err(),
        "unsupported `with` should be an error, got Ok"
    );
    assert!(
        res.unwrap_err().iter().any(|p| p.contains("with")),
        "wrong problem message"
    );

    // a normal program still succeeds
    let ok = maca_parser::parse(
        "main() -> int {\n    xs = 1, 2, 3\n    info(\"{xs.sum()}\")\n    0\n}\n",
    )
    .module;
    assert!(
        maca_backend_c::emit_checked(&ok).is_ok(),
        "valid program wrongly rejected"
    );
}

/// A value with no text form is a diagnostic, not a pointer dereference.
///
/// The two-operand `maca_concat` took declared parameters, so handing it a
/// record was a C type error naming both types. One variadic call takes
/// whatever it is given, so the refusal has to be made here.
#[test]
fn concatenating_a_value_with_no_text_form_is_refused() {
    for (src, want) in [
        (
            "Point = { x: int }\n\nmain() -> int {\n                 p = Point { x = 1 }\n    info(\"p: \" ++ p)\n    0\n}\n",
            "Point",
        ),
        (
            "main() -> int {\n    xs = [1, 2]\n    info(\"xs: \" ++ xs)\n    0\n}\n",
            "IntArr",
        ),
    ] {
        let m = maca_parser::parse(src).module;
        let err = maca_backend_c::emit_checked(&m).expect_err("must be refused");
        assert!(
            err.iter()
                .any(|p| p.contains(want) && p.contains("text form")),
            "wrong or missing diagnostic for {want}: {err:?}"
        );
    }

    // an unannotated parameter is still a string as far as `++` is concerned;
    // refusing it would reject `greet(n) => "hi " ++ n`
    let ok = maca_parser::parse("greet(n) -> str => \"hi \" ++ n\n").module;
    assert!(
        maca_backend_c::emit_checked(&ok).is_ok(),
        "an unknown type is not a refusal"
    );
}

/// A `++` chain runs left to right, whatever the ownership analysis decides to
/// name. Naming only the pieces to release left the order to the C compiler for
/// the rest, which is neither the order the source is written in nor a stable
/// one.
#[test]
fn a_concat_chain_evaluates_in_source_order() {
    let out = c(
        "a() -> str => \"A\"\n\nb(s: str) -> str => s\n\n                 c() -> str => \"C\"\n\n                 main() -> int {\n    info(a() ++ b(\"B\") ++ c())\n    0\n}\n",
    );
    let body = out
        .split("int main(")
        .nth(1)
        .expect("a main")
        .split('\n')
        .find(|l| l.contains("maca_concat_n"))
        .expect("the concatenation");
    let at = |needle: &str| body.find(needle).unwrap_or(usize::MAX);
    assert!(
        at("a()") < at("b(\"B\")") && at("b(\"B\")") < at("c()"),
        "operands are not evaluated in source order:\n{body}"
    );
}

/// Several Maca types share one C array: a closure, a future and a value of
/// unknown type all cross as `int64_t`, so all three are `IntArr`. Emitting the
/// definitions keyed on the Maca type wrote the same `typedef` twice.
#[test]
fn one_array_type_is_defined_once_however_many_types_share_it() {
    let out = c(
        "Box = { v: int }\n\n                 f(xs: int[], g) -> int {\n                     ys = [g]\n    xs.length() + ys.length()\n}\n\n                 main() -> int => f([1], 2)\n",
    );
    let defs = out.matches("MACA_DEFINE_ARRAY(IntArr,").count();
    assert_eq!(defs, 1, "IntArr defined {defs} times:\n{out}");
}

/// `assert_eq` compares what it is given as text, so a number is rendered on
/// the way in. Passing one through unchanged handed an `int64_t` to a
/// `const char*` parameter, which `strcmp` then dereferenced.
#[test]
fn assert_eq_renders_what_it_is_given() {
    let out = c("main() -> int {\n    assert_eq(3, 3, \"same\")\n    failures()\n}\n");
    assert!(
        out.contains("maca_from_int(3)"),
        "the number is rendered:\n{out}"
    );
    let strs = c("main() -> int {\n    assert_eq(\"a\", \"a\", \"same\")\n    failures()\n}\n");
    assert!(
        !strs.contains("maca_from_int"),
        "and a string is left alone:\n{strs}"
    );
}

/// A method the checker accepts on a receiver, that this back end cannot lower
/// for that element type, is a diagnostic here rather than a C error naming the
/// generated call. `xs.sort()` on a `str[][]` compiled to `sort(rows)`.
#[test]
fn a_method_that_cannot_be_lowered_says_so() {
    let m = maca_parser::parse(
        "main() -> int {\n    rows: str[][] = [[\"a\"]]\n             rows.sort().length()\n}\n",
    )
    .module;
    let err = maca_backend_c::emit_checked(&m).expect_err("must be refused");
    assert!(
        err.iter().any(|p| p.contains("sort")),
        "names the method: {err:?}"
    );
}

/// An absent element answers with its type's empty value, and a sum whose
/// variants carry payloads is a struct, so `0` in the other arm of the bounds
/// check stopped every program indexing a list of them from compiling.
#[test]
fn a_list_of_payload_variants_can_be_indexed() {
    let out = c(
        "Shape = Circle(float) | Square(int)\n\n                 main() -> int {\n    ss = [Circle(1.0), Square(2)]\n                     ss.get(0)\n    ss.first()\n    ss.last()\n    0\n}\n",
    );
    assert!(
        !out.contains(": 0; })") || out.contains("memset"),
        "a struct element gets a zeroed value, not `0`:\n{out}"
    );
    assert_eq!(
        out.matches("memset(&_z, 0, sizeof _z)").count(),
        3,
        "one per accessor:\n{out}"
    );
}

#[test]
fn recursive_record_forward_declares_to_break_the_array_cycle() {
    // `Node { kids: Node[] }` is a definition cycle: the struct body needs the
    // element-array type, the array's ops need the struct's size. The backend
    // forward-declares the record and splits the array into struct-then-ops.
    let out = c("Node = {\n    name: str\n    kids: Node[]\n}\n\nmain() -> int { 0 }\n");
    assert!(
        out.contains("typedef struct Node Node;"),
        "no forward decl for recursive record:\n{out}"
    );
    assert!(
        out.contains("MACA_ARRAY_STRUCT(NodeArr, Node)"),
        "element array struct not declared before the body:\n{out}"
    );
    assert!(
        out.contains("struct Node {"),
        "recursive record body should be a named struct:\n{out}"
    );
    assert!(
        out.contains("MACA_ARRAY_OPS(NodeArr, Node)"),
        "element array ops not emitted after the body:\n{out}"
    );
    // ordering: forward decl < array struct < body < array ops
    let fwd = out.find("typedef struct Node Node;").unwrap();
    let arr_s = out.find("MACA_ARRAY_STRUCT(NodeArr").unwrap();
    let body = out.find("struct Node {").unwrap();
    let arr_o = out.find("MACA_ARRAY_OPS(NodeArr").unwrap();
    assert!(
        fwd < arr_s && arr_s < body && body < arr_o,
        "recursive-record emission out of order: fwd={fwd} arr_struct={arr_s} body={body} arr_ops={arr_o}"
    );
}

#[test]
fn str_and_array_scan_primitives_lower() {
    // the methods the self-hosted lexer scans source with.
    let body = func(
        "scan(s: str) -> int {\n    cs = s.chars()\n    n = cs.length()\n    c = cs.get(0)\n    ws = c.is_whitespace()\n    d = c.is_ascii_digit()\n    a = c.is_alpha()\n    l = s.length()\n    sub = cs.slice(0, 2)\n    n + l\n}\n",
        "scan",
    );
    assert!(body.contains("maca_str_at"), "chars() not lowered:\n{body}");
    assert!(
        body.contains("maca_strlen"),
        "length() not lowered:\n{body}"
    );
    assert!(
        body.contains("maca_is_space"),
        "is_whitespace not lowered:\n{body}"
    );
    assert!(
        body.contains("maca_is_digit"),
        "is_ascii_digit not lowered:\n{body}"
    );
    assert!(
        body.contains("maca_is_alpha"),
        "is_alpha not lowered:\n{body}"
    );
    assert!(
        !body.contains("unsupported"),
        "a scan primitive is unsupported:\n{body}"
    );
}

#[test]
fn higher_order_param_and_fn_value_lower_to_closures() {
    // an unannotated param that is called is typed `maca_closure`; a fn passed
    // by name is wrapped in a closure with a boxing thunk.
    let src = "even(n: int) -> bool => n % 2 == 0\n\n\
        apply(pred, x: int) -> bool => pred(x)\n\n\
        main() -> int {\n    apply(even, 4) ? 0 : 1\n}\n";
    let out = c(src);
    // the callee param is a closure in `apply`'s signature
    assert!(
        out.contains("maca_closure") && out.contains("apply("),
        "higher-order param not typed as a closure:\n{out}"
    );
    // `even` passed by name gets a boxing thunk and a closure literal
    assert!(
        out.contains("even__fnval"),
        "fn value not wrapped in a thunk:\n{out}"
    );
    // and the param call goes through the closure ABI
    let body = func(src, "apply");
    assert!(
        body.contains("maca_call1"),
        "param call didn't use the closure ABI:\n{body}"
    );
}

#[test]
fn empty_list_argument_takes_its_element_type_from_the_callee() {
    // `seed([])` where `seed(xs: str[])` must build a StrArr, not the default
    // IntArr: the call threads the parameter type as the literal's expected.
    let body = func(
        "seed(xs: str[]) -> int => xs.length()\n\nmain() -> int {\n    seed([])\n}\n",
        "main",
    );
    assert!(
        body.contains("StrArr_new()"),
        "empty-list arg didn't take the parameter's element type:\n{body}"
    );
}
