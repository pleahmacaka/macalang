use maca_parser::{parse, print_module};
use std::fs;
use std::path::PathBuf;

fn example(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(name);
    fs::read_to_string(p).unwrap()
}

/// Parse an example, then check parse→print→parse is AST-stable.
fn roundtrip(name: &str) {
    let src = example(name);
    let first = parse(&src);
    assert!(first.errors.is_empty(), "{name} parse errors: {:?}", first.errors);

    let printed = print_module(&first.module);
    let second = parse(&printed);
    assert!(
        second.errors.is_empty(),
        "{name} reparse errors: {:?}\n--- printed ---\n{printed}",
        second.errors
    );
    assert_eq!(
        second.module, first.module,
        "{name} roundtrip changed the AST\n--- printed ---\n{printed}"
    );
}

#[test]
fn roundtrip_hello() {
    roundtrip("hello.maca");
}
#[test]
fn roundtrip_taskr() {
    roundtrip("taskr.maca");
}
#[test]
fn roundtrip_system() {
    roundtrip("system.maca");
}
#[test]
fn roundtrip_counter() {
    roundtrip("counter.maca");
}
#[test]
fn roundtrip_dot() {
    roundtrip("dot.maca");
}

// ---- targeted grammar cases ---------------------------------------------

fn clean(src: &str) -> maca_parser::Module {
    let p = parse(src);
    assert!(p.errors.is_empty(), "unexpected errors: {:?}", p.errors);
    p.module
}

#[test]
fn ctor_vs_for_header_brace() {
    use maca_parser::{Expr, Stmt};
    // `for t in xs { .. }` — the `{` is the body, not `xs { .. }` ctor.
    let m = clean("f() {\n    for t in xs {\n        g(t)\n    }\n}");
    let Stmt::Fn(f) = &m.items[0] else { panic!() };
    let Some(maca_parser::FnBody::Block(b)) = &f.body else { panic!() };
    let Stmt::Expr(Expr::For { iter, .. }) = &b[0] else { panic!("expected for, got {:?}", b[0]) };
    assert!(matches!(&**iter, Expr::Ident(n) if n == "xs"), "iter should be plain xs");
}

#[test]
fn ternary_is_not_propagate() {
    use maca_parser::{Expr, Stmt};
    let m = clean("x = c ? a : b");
    let Stmt::Bind(bind) = &m.items[0] else { panic!() };
    assert!(matches!(bind.value, Expr::Ternary { .. }));

    let m = clean("y = f()?");
    let Stmt::Bind(bind) = &m.items[0] else { panic!() };
    assert!(matches!(bind.value, Expr::Try(_)));
}

#[test]
fn bracketless_list_binding() {
    use maca_parser::{Expr, Stmt};
    let m = clean("pkgs = a, b, c");
    let Stmt::Bind(b) = &m.items[0] else { panic!() };
    let Expr::List(es) = &b.value else { panic!("expected list, got {:?}", b.value) };
    assert_eq!(es.len(), 3);
}

#[test]
fn ui_directive_arg() {
    use maca_parser::{Arg, Dir, Expr, Stmt};
    let m = clean("v() -> Element => input(bind:value=name)");
    let Stmt::Fn(f) = &m.items[0] else { panic!() };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else { panic!() };
    let Expr::Call { args, .. } = &**e else { panic!() };
    let Arg::Directive { kind, prop, .. } = &args[0] else { panic!("expected directive") };
    assert_eq!(*kind, Dir::Bind);
    assert_eq!(prop, "value");
}
