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
fn roundtrip_operators() {
    roundtrip("operators.maca");
}
#[test]
fn roundtrip_generic() {
    roundtrip("generic.maca");
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
fn roundtrip_range() {
    roundtrip("range.maca");
}

#[test]
fn range_binds_looser_than_arithmetic() {
    use maca_parser::{BinOp, Expr, Stmt};
    // `0..n - 1` must read as `0 .. (n - 1)`, not `(0..n) - 1`.
    let m = clean("f(n: int) => 0..n - 1");
    let Stmt::Fn(f) = &m.items[0] else { panic!() };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else { panic!() };
    let Expr::Range { lo, hi } = &**e else { panic!("expected range, got {e:?}") };
    assert!(matches!(&**lo, Expr::Int(0)));
    assert!(matches!(&**hi, Expr::Binary { op: BinOp::Sub, .. }), "hi should be n - 1");
}

#[test]
fn malformed_params_terminate() {
    // A function-type annotation isn't surface syntax; the parser must report
    // errors and *terminate* rather than spin forever (regression: the param
    // loop used to make no progress and OOM). The playground parses arbitrary
    // user input, so robustness here matters.
    let p = parse("f(pred: (str) -> bool) -> int => 0\n");
    assert!(!p.errors.is_empty(), "expected parse errors");

    // an unclosed / stray param list must also terminate
    let _ = parse("g(a: , , ->) {}\n");
    let _ = parse("h(/ <io,,, > ) => 0\n");
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
fn match_guard_arm() {
    use maca_parser::{Expr, Stmt};
    // `_ if cond => body` — the guard must not swallow the arm's `=>` as a
    // lambda arrow.
    let m = clean("f(x: int) -> int =>\n    match true {\n        _ if x > 0 => 1\n        _ => 0\n    }\n");
    let Stmt::Fn(f) = &m.items[0] else { panic!() };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else { panic!() };
    let Expr::Match { arms, .. } = &**e else { panic!("expected match, got {e:?}") };
    assert_eq!(arms.len(), 2);
    assert!(arms[0].guard.is_some(), "first arm should carry a guard");
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

#[test]
fn roundtrip_loops() {
    roundtrip("loops.maca");
}

#[test]
fn roundtrip_fizzbuzz() {
    roundtrip("fizzbuzz.maca");
}

#[test]
fn roundtrip_match_guard() {
    roundtrip("match_guard.maca");
}

#[test]
fn roundtrip_or_patterns() {
    roundtrip("or_patterns.maca");
}

#[test]
fn roundtrip_payload_sum() {
    roundtrip("payload_sum.maca");
}

#[test]
fn roundtrip_catch() {
    roundtrip("catch.maca");
}

#[test]
fn roundtrip_lambda() {
    roundtrip("lambda.maca");
}

#[test]
fn roundtrip_generic_record() {
    roundtrip("generic_record.maca");
}

#[test]
fn malformed_input_does_not_hang() {
    // a reserved word where a field name is expected used to loop forever
    for src in [
        "f(p: P) -> int { match p { { from } => 1 } }\n",
        "R = {\n    from: int\n}\n",
        "main() -> int {\n    let x = {\n    0\n}\n",
        "x = ) ( } {\n",
    ] {
        let p = parse(src);
        // it must terminate and report errors, not run out of memory
        assert!(!p.errors.is_empty(), "expected parse errors for {src:?}");
    }
}

#[test]
fn roundtrip_record_pattern() {
    roundtrip("record_pattern.maca");
}

#[test]
fn roundtrip_record_update() {
    roundtrip("record_update.maca");
}

#[test]
fn roundtrip_sum_record() {
    roundtrip("sum_record.maca");
}

#[test]
fn roundtrip_indexing() {
    roundtrip("indexing.maca");
}

#[test]
fn roundtrip_tree() {
    roundtrip("tree.maca");
}

#[test]
fn roundtrip_keywords() {
    roundtrip("keywords.maca");
}

#[test]
fn arrow_body_can_be_a_comma_list() {
    // `make() -> int[] => 1, 2, 3` — a bracketless list is a valid arrow body
    let m = clean("make() -> int[] => 1, 2, 3\n");
    let maca_parser::Stmt::Fn(f) = &m.items[0] else { panic!("not a fn") };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else { panic!("no expr body") };
    let maca_parser::Expr::List(es) = &**e else { panic!("body is not a list: {e:?}") };
    assert_eq!(es.len(), 3, "expected 3 elements");
}

#[test]
fn index_is_postfix_not_a_list() {
    // `xs[i]` after an expression is a subscript; a leading `[..]` is a list
    let m = clean("f(xs: int[]) -> int => xs[0]\n");
    let maca_parser::Stmt::Fn(f) = &m.items[0] else { panic!("not a fn") };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else { panic!("no expr body") };
    assert!(matches!(&**e, maca_parser::Expr::Index { .. }), "not an Index: {e:?}");
}
