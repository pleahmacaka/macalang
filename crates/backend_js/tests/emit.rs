//! Hermetic tests for the JS backend: assert the generated JS directly (no
//! Node/browser needed; the driver's WSL-gated tests cover actual execution).

fn js(src: &str) -> String {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    maca_backend_js::emit(&p.module).js
}

#[test]
fn function_becomes_js_and_is_exported() {
    let out = maca_backend_js::emit(&maca_parser::parse("add(a: int, b: int) -> int => a + b\n").module);
    assert!(out.js.contains("function add"), "no function:\n{}", out.js);
    assert!(out.js.contains("(a + b)"), "no body:\n{}", out.js);
    assert!(out.exports.contains(&"add".to_string()), "not exported: {:?}", out.exports);
}

#[test]
fn arithmetic_and_ternary() {
    let out = js("f(c: bool) -> int => c ? 1 + 2 : 3\n");
    assert!(out.contains("(1 + 2)"), "{out}");
    assert!(out.contains("? ") && out.contains(" : "), "no ternary:\n{out}");
}

#[test]
fn while_break_continue() {
    let out = js(
        "f(n: int) -> int {\n    let i = 0\n    while i < n {\n        i = i + 1\n        if i < 2 { continue }\n        break\n    }\n    i\n}\n",
    );
    assert!(out.contains("while ("), "no while:\n{out}");
    assert!(out.contains("break;"), "no break:\n{out}");
    assert!(out.contains("continue;"), "no continue:\n{out}");
    // `let i` declares, bare `i =` reassigns (no second `let`)
    assert!(out.contains("let i = 0"), "no let:\n{out}");
    assert!(out.contains("i = (i + 1)") && !out.contains("let i = (i + 1)"), "reassignment wrong:\n{out}");
}

#[test]
fn string_concat_uses_plus_or_concat() {
    let out = js("greet(n: str) -> str => \"hi {n}\"\n");
    assert!(out.contains("hi"), "interpolation dropped:\n{out}");
}
