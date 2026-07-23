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
fn index_read_and_write() {
    let out = js("f(xs: int[]) -> int {\n    xs[0] = 9\n    xs[1]\n}\n");
    assert!(out.contains("xs[0] = 9"), "index write missing:\n{out}");
    assert!(out.contains("xs[1]"), "index read missing:\n{out}");
}

#[test]
fn record_update_is_object_spread() {
    let out = js("P = {\n    x: int\n    y: int\n}\nf(p: P) -> P => p with { x = 9 }\n");
    // the update lowers to an object spread; the base is copied, `x` overwritten
    assert!(out.contains("...p"), "base not spread:\n{out}");
    assert!(out.contains("x: 9"), "field not overwritten:\n{out}");
    // the function body itself must not be the `null` unsupported fallback
    let body = out.split("function f(p)").nth(1).unwrap_or("");
    let body = &body[..body.find('}').map(|i| i + 1).unwrap_or(body.len())];
    assert!(!body.contains("null"), "function body is unsupported null:\n{body}");
}

#[test]
fn string_concat_uses_plus_or_concat() {
    let out = js("greet(n: str) -> str => \"hi {n}\"\n");
    assert!(out.contains("hi"), "interpolation dropped:\n{out}");
}

#[test]
fn reactive_ui_binds_state_and_calls() {
    // a UI where a text child reads state and a text-returning function call is
    // used as a child (not an element), plus a state-mutating handler.
    let out = js(
        "count = 0\n\
         shown(n: int) -> str => \"n={n}\"\n\
         bump() { count = count + 1  update() }\n\
         main() -> Element =>\n\
             div(\n\
                 button(on:click=bump, \"+\")\n\
                 span(shown(count))\n\
             )\n",
    );
    // state reference resolves to state.count everywhere
    assert!(out.contains("state.count = (state.count + 1)"), "handler not state-aware:\n{out}");
    // a text-returning call is a reactive text node, not a <shown> element
    assert!(out.contains("createTextNode(shown(state.count))"), "call child not text:\n{out}");
    assert!(!out.contains("createElement(\"shown\")"), "text call became an element:\n{out}");
    assert!(out.contains("_binds.push(() => { "), "no reactive updater registered:\n{out}");
    // the click handler is the bare function reference
    assert!(out.contains("addEventListener(\"click\", bump)"), "handler not wired:\n{out}");
}

#[test]
fn html_attribute_sets_inner_html() {
    let out = js(
        "svg = \"<svg></svg>\"\n\
         main() -> Element => div(html=svg)\n",
    );
    assert!(out.contains(".innerHTML = state.svg"), "html= not lowered to innerHTML:\n{out}");
    assert!(out.contains("_binds.push(() => { "), "innerHTML not reactive:\n{out}");
}

#[test]
fn foreign_import_blocks_embed_js_and_css() {
    // `import js`/`import css` with a raw triple-quoted block embed verbatim:
    // the JS is prepended (so its helpers exist before the app mounts), the CSS
    // appended to the stylesheet.
    let out = maca_backend_js::emit(&maca_parser::parse(
        "import css \"\"\"\n.x { color: red }\n\"\"\"\nimport js \"\"\"\nwindow.hi = () => 1;\n\"\"\"\ng = \"\"\nmain() -> Element => div(class=\"x\", g)\n",
    ).module);
    assert!(out.js.contains("window.hi = () => 1;"), "js not embedded:\n{}", out.js);
    // embedded js comes before the app's state/mount
    assert!(out.js.find("window.hi").unwrap() < out.js.find("const state").unwrap(), "js not prepended:\n{}", out.js);
    assert!(out.css.contains(".x { color: red }"), "css not embedded:\n{}", out.css);
}
