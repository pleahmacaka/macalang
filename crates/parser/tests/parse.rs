use maca_parser::{Expr, FnBody, Stmt, Type, parse, print_module};
use std::fs;
use std::path::PathBuf;

fn example(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name);
    fs::read_to_string(p).unwrap()
}

/// Parse an example, then check parse→print→parse is AST-stable.
fn roundtrip(name: &str) {
    let src = example(name);
    let first = parse(&src);
    assert!(
        first.errors.is_empty(),
        "{name} parse errors: {:?}",
        first.errors
    );

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
    let Some(maca_parser::FnBody::Block(b)) = &f.body else {
        panic!()
    };
    let Stmt::Expr(Expr::For { iter, .. }) = &b[0] else {
        panic!("expected for, got {:?}", b[0])
    };
    assert!(
        matches!(&**iter, Expr::Ident(n) if n == "xs"),
        "iter should be plain xs"
    );
}

#[test]
fn roundtrip_range() {
    roundtrip("range.maca");
}

#[test]
fn roundtrip_tour() {
    roundtrip("tour.maca");
}

#[test]
fn range_binds_looser_than_arithmetic() {
    use maca_parser::{BinOp, Expr, Stmt};
    // `0..n - 1` must read as `0 .. (n - 1)`, not `(0..n) - 1`.
    let m = clean("f(n: int) => 0..n - 1");
    let Stmt::Fn(f) = &m.items[0] else { panic!() };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else {
        panic!()
    };
    let Expr::Range { lo, hi } = &**e else {
        panic!("expected range, got {e:?}")
    };
    assert!(matches!(&**lo, Expr::Int(0)));
    assert!(
        matches!(&**hi, Expr::Binary { op: BinOp::Sub, .. }),
        "hi should be n - 1"
    );
}

/// `(T, U) -> R` is a type. A function passed as an argument still needs no
/// annotation — an unannotated parameter that is called in the body is one —
/// but a function *kept* in a record field is declared before anything calls
/// it, so there has to be a way to write it down.
#[test]
fn a_function_type_is_surface_syntax() {
    let p = parse("f(pred: (str) -> bool) -> int => 0\n");
    assert!(p.errors.is_empty(), "unexpected errors: {:?}", p.errors);
    let Some(Stmt::Fn(f)) = p.module.items.first() else {
        panic!("expected a function")
    };
    let Some(Type::Fn(ps, r)) = &f.params[0].ty else {
        panic!("expected a function type, got {:?}", f.params[0].ty)
    };
    assert_eq!(ps.len(), 1);
    assert!(matches!(&**r, Type::Name(n) if n == &["bool".to_string()]));

    // and it round-trips through the printer
    let printed = maca_parser::print_module(&p.module);
    assert!(printed.contains("(str) -> bool"), "{printed}");
}

/// The parentheses are not optional and a list of types is not a type. Both
/// have to be refusals rather than silence, because the shape they are nearest
/// to — a grouped type — is one the parser accepts.
#[test]
fn a_type_list_without_an_arrow_is_refused() {
    assert!(!parse("f(x: (str, int)) -> int => 0\n").errors.is_empty());
    assert!(!parse("f(x: ()) -> int => 0\n").errors.is_empty());
    assert!(parse("f(x: (str)) -> int => 0\n").errors.is_empty());
    assert!(parse("f(x: () -> int) -> int => 0\n").errors.is_empty());
}

#[test]
fn malformed_params_terminate() {
    // The parser must report errors and *terminate* rather than spin forever
    // (regression: the param loop used to make no progress and OOM). The
    // playground parses arbitrary user input, so robustness here matters.
    let _ = parse("g(a: , , ->) {}\n");
    let _ = parse("h(/ <io,,, > ) => 0\n");
    let _ = parse("k(f: (,) -> ) => 0\n");
    let _ = parse("m(f: (str) -> ) => 0\n");
}

#[test]
fn ternary_is_not_propagate() {
    use maca_parser::{Expr, Stmt};
    let m = clean("x = c ? a : b");
    let Stmt::Bind(bind) = &m.items[0] else {
        panic!()
    };
    assert!(matches!(bind.value, Expr::Ternary { .. }));

    let m = clean("y = f()?");
    let Stmt::Bind(bind) = &m.items[0] else {
        panic!()
    };
    assert!(matches!(bind.value, Expr::Try(_)));
}

#[test]
fn match_guard_arm() {
    use maca_parser::{Expr, Stmt};
    // `_ if cond => body` — the guard must not swallow the arm's `=>` as a
    // lambda arrow.
    let m = clean(
        "f(x: int) -> int =>\n    match true {\n        _ if x > 0 => 1\n        _ => 0\n    }\n",
    );
    let Stmt::Fn(f) = &m.items[0] else { panic!() };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else {
        panic!()
    };
    let Expr::Match { arms, .. } = &**e else {
        panic!("expected match, got {e:?}")
    };
    assert_eq!(arms.len(), 2);
    assert!(arms[0].guard.is_some(), "first arm should carry a guard");
}

#[test]
fn bracketless_list_binding() {
    use maca_parser::{Expr, Stmt};
    let m = clean("pkgs = a, b, c");
    let Stmt::Bind(b) = &m.items[0] else { panic!() };
    let Expr::List(es) = &b.value else {
        panic!("expected list, got {:?}", b.value)
    };
    assert_eq!(es.len(), 3);
}

#[test]
fn ui_directive_arg() {
    use maca_parser::{Arg, Dir, Expr, Stmt};
    let m = clean("v() -> Element => input(bind:value=name)");
    let Stmt::Fn(f) = &m.items[0] else { panic!() };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else {
        panic!()
    };
    let Expr::Call { args, .. } = &**e else {
        panic!()
    };
    let Arg::Directive { kind, prop, .. } = &args[0] else {
        panic!("expected directive")
    };
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
    // a reserved word where a field name is expected used to loop forever.
    // (`from` is no longer one — `{ from }` is a perfectly good shorthand
    // field now — so the cases here use words that still are.)
    for src in [
        "f(p: P) -> int { match p { { match } => 1 } }\n",
        "R = {\n    while: int\n}\n",
        "main() -> int {\n    x = {\n    0\n}\n",
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
fn roundtrip_async() {
    roundtrip("async.maca");
}

#[test]
fn roundtrip_collections() {
    roundtrip("collections.maca");
}

#[test]
fn await_and_spawn_bind_tighter_than_binary() {
    // `await a + await b` must parse as `(await a) + (await b)`.
    let m = parse("f() -> int => await a + await b\n").module;
    let maca_parser::Stmt::Fn(f) = &m.items[0] else {
        panic!("not a fn")
    };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else {
        panic!("no expr body")
    };
    let maca_parser::Expr::Binary { lhs, rhs, .. } = &**e else {
        panic!("expected a binary at the top, got {e:?}")
    };
    assert!(
        matches!(&**lhs, maca_parser::Expr::Await(_)),
        "lhs not await: {lhs:?}"
    );
    assert!(
        matches!(&**rhs, maca_parser::Expr::Await(_)),
        "rhs not await: {rhs:?}"
    );
}

#[test]
fn arrow_body_can_be_a_comma_list() {
    // `make() -> int[] => 1, 2, 3` — a bracketless list is a valid arrow body
    let m = clean("make() -> int[] => 1, 2, 3\n");
    let maca_parser::Stmt::Fn(f) = &m.items[0] else {
        panic!("not a fn")
    };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else {
        panic!("no expr body")
    };
    let maca_parser::Expr::List(es) = &**e else {
        panic!("body is not a list: {e:?}")
    };
    assert_eq!(es.len(), 3, "expected 3 elements");
}

#[test]
fn index_is_postfix_not_a_list() {
    // `xs[i]` after an expression is a subscript; a leading `[..]` is a list
    let m = clean("f(xs: int[]) -> int => xs[0]\n");
    let maca_parser::Stmt::Fn(f) = &m.items[0] else {
        panic!("not a fn")
    };
    let Some(maca_parser::FnBody::Expr(e)) = &f.body else {
        panic!("no expr body")
    };
    assert!(
        matches!(&**e, maca_parser::Expr::Index { .. }),
        "not an Index: {e:?}"
    );
}

#[test]
fn a_lambda_can_declare_its_return_type() {
    // `(a, b) -> T => …`. The annotation is what a trait-impl method needs when
    // it has to match a signature the compiler cannot read.
    let m = parsed("use_it() -> int {\n    f = (a, b) -> Element => g(a, b)\n    0\n}\n");
    let Stmt::Fn(fd) = &m.items[0] else { panic!() };
    let Some(FnBody::Block(stmts)) = &fd.body else {
        panic!()
    };
    let Stmt::Bind(b) = &stmts[0] else {
        panic!("{:?}", stmts[0])
    };
    let Expr::Lambda { params, ret, .. } = &b.value else {
        panic!("expected a lambda: {:?}", b.value)
    };
    assert_eq!(params.len(), 2);
    assert!(ret.is_some(), "an annotation was written");

    // and it survives a print / re-parse round trip
    let printed = print_module(&m);
    let again = parse(&printed);
    assert!(again.errors.is_empty(), "{printed}: {:?}", again.errors);
    let Stmt::Fn(fd2) = &again.module.items[0] else {
        panic!("{printed}")
    };
    let Some(FnBody::Block(ss2)) = &fd2.body else {
        panic!("{printed}")
    };
    let Stmt::Bind(b2) = &ss2[0] else {
        panic!("{printed}")
    };
    let Expr::Lambda { ret: ret2, .. } = &b2.value else {
        panic!("{printed}")
    };
    assert_eq!(ret, ret2, "the declared type was lost: {printed}");
}

#[test]
fn a_lambda_parameter_can_declare_its_type() {
    let m = parsed("use_it() -> int {\n    f = (a: Window, b) => g(a, b)\n    0\n}\n");
    let Stmt::Fn(fd) = &m.items[0] else { panic!() };
    let Some(FnBody::Block(stmts)) = &fd.body else {
        panic!()
    };
    let Stmt::Bind(b) = &stmts[0] else { panic!() };
    let Expr::Lambda { params, .. } = &b.value else {
        panic!("{:?}", b.value)
    };
    assert!(params[0].ty.is_some(), "the first parameter is typed");
    assert!(params[1].ty.is_none(), "the second is not");
}

/// A top-level `name = (…) => …` *is* a function definition — there is nothing
/// to capture at module scope, and lowering it as a closure produced a constant
/// no call site could reach.
#[test]
fn a_top_level_lambda_binding_becomes_a_function() {
    let m = parsed("twice = (x) -> int => x * 2\n");
    let Stmt::Fn(fd) = &m.items[0] else {
        panic!("expected a fn def: {:?}", m.items[0])
    };
    assert_eq!(fd.name, "twice");
    assert_eq!(fd.params.len(), 1);
    assert!(fd.ret.is_some(), "the annotation became the return type");
}

/// …but a local one stays a lambda, because a local *can* capture.
#[test]
fn a_local_lambda_binding_stays_a_lambda() {
    let m = parsed("use_it() -> int {\n    step = 10\n    f = (x) => x + step\n    f(1)\n}\n");
    let Stmt::Fn(fd) = &m.items[0] else { panic!() };
    let Some(FnBody::Block(stmts)) = &fd.body else {
        panic!()
    };
    let Stmt::Bind(b) = &stmts[1] else { panic!() };
    assert!(
        matches!(&b.value, Expr::Lambda { .. }),
        "expected a lambda: {:?}",
        b.value
    );
}

#[test]
fn an_arrow_after_a_call_is_still_a_signature_not_a_lambda() {
    // `-> T` only heads a lambda when a `=>` follows the type. A function
    // definition must keep parsing as one.
    let m = parsed("f(a: int) -> int => a\n");
    assert!(
        matches!(&m.items[0], Stmt::Fn(_)),
        "expected a fn def: {:?}",
        m.items[0]
    );
}

/// Parse a source string, requiring no errors.
fn parsed(src: &str) -> maca_parser::Module {
    let p = parse(src);
    assert!(p.errors.is_empty(), "{src}: {:?}", p.errors);
    p.module
}

// ---- a `{` after a function's `=>` --------------------------------------

/// `f() -> int => { a = 1 \n a }` is a block, not a record literal.
///
/// It was read as a record: the fields came out as `a = 1` and a punned `a`, so
/// the JS backend emitted `return { a: 1, a: a };` (a duplicate key and a
/// reference to an `a` that was never declared) and the native path reported a
/// mismatch against `{ a: any }`, a record the author never wrote.
///
/// The arrow form is parsed straight into `FnBody::Block`, because
/// `f() -> T => { … }` and `f() -> T { … }` are the same function; every back
/// end already lowers a block body, and the printer prints back the spelling
/// without the spare `=>`.
#[test]
fn an_arrow_body_that_binds_and_then_reads_a_name_is_a_block() {
    for src in [
        "f() -> int => {\n    a = 1\n    a\n}\n",
        "f() -> int => {\n    a = 1\n    b = a + 1\n    a + b\n}\n",
        // one statement, no binding at all
        "f() -> int => {\n    41 + 1\n}\n",
        // A name bound and then reassigned. Every entry here is a `name =
        // value`, so the repeated name is the only thing that rules the record
        // out, and it is the only shape where that is true: a block whose last
        // statement is an assignment has no value to return, so this case
        // cannot be asserted by running it.
        "f() -> int => {\n    a = 1\n    a = 2\n}\n",
        // a punned `{ x, y }` is a bare identifier per entry, same as above
        "f(x: int, y: int) -> int => {\n    x\n    y\n}\n",
        // nothing at all: `{}` is an empty block, and a record with no fields
        // is not a thing worth having a second spelling for
        "f() -> int => {}\n",
    ] {
        let m = parsed(src);
        let Stmt::Fn(fd) = &m.items[0] else {
            panic!("expected a fn def: {:?}", m.items[0])
        };
        assert!(
            matches!(&fd.body, Some(FnBody::Block(_))),
            "expected a block body for {src:?}, got {:?}",
            fd.body
        );
    }
}

/// The record body the ambiguity is about still works, because a comma
/// separates record fields and never separates statements.
///
/// Only a comma at the brace's *own* depth separates two fields, and only the
/// matching `}` ends the body. A field value carries commas and braces of its
/// own, so both of those are load-bearing: counting a nested comma cuts a field
/// value in half (`tags = [1` then `2]`), and stopping at the first `}` cuts the
/// body off inside a nested literal. Either way this record stops parsing as
/// one, so both shapes are here.
#[test]
fn a_comma_separated_arrow_body_is_a_record_literal() {
    for src in [
        "mk() -> Point => { x = 1, y = 2 }\n",
        "mk() -> Point => {\n    x = 1,\n    y = 2\n}\n",
        // commas inside a field's own brackets and parens
        "mk() -> Point => { x = [1, 2].length(), y = max(3, 4) }\n",
        // a nested record literal, whose `}` is not this body's
        "mk() -> Line => { a = { x = 1, y = 2 }, b = { x = 3, y = 4 } }\n",
    ] {
        let m = parsed(src);
        let Stmt::Fn(fd) = &m.items[0] else { panic!() };
        let Some(FnBody::Expr(e)) = &fd.body else {
            panic!("expected an expression body for {src:?}: {:?}", fd.body)
        };
        assert!(
            matches!(&**e, Expr::Record(fs) if fs.len() == 2),
            "expected a two-field record for {src:?}: {e:?}"
        );
    }
}

/// With every entry a distinct `name = value` and only newlines between them,
/// both readings hold and neither is taken. Refused by name, with both
/// spellings, rather than silently picking one.
#[test]
fn an_arrow_body_that_reads_both_ways_is_refused() {
    for src in [
        // a single field, which is also a single binding statement
        "mk() -> Point => { x = 1 }\n",
        "mk() -> Point => {\n    x = 1\n    y = 2\n}\n",
    ] {
        let p = parse(src);
        let msg = p.errors.join("\n");
        assert!(
            msg.contains("reads as a record literal and as a block"),
            "expected a refusal for {src:?}, got {:?}",
            p.errors
        );
        assert!(
            msg.contains("Name { … }") && msg.contains("drop the `=>`"),
            "the refusal must show both spellings, got {msg:?}"
        );
    }
}

/// An arrow body that is not a brace at all is untouched, including the
/// bracketless comma list.
#[test]
fn arrow_bodies_that_are_not_braces_are_unchanged() {
    for src in [
        "f() -> int => 1\n",
        "f() -> int[] => 1, 2, 3\n",
        "f() -> int => if true { 1 } else { 2 }\n",
        "f(p: Point) -> int => match p.x {\n    0 => 1\n    _ => 2\n}\n",
    ] {
        let m = parsed(src);
        let Stmt::Fn(fd) = &m.items[0] else { panic!() };
        assert!(
            matches!(&fd.body, Some(FnBody::Expr(_))),
            "expected an expression body for {src:?}: {:?}",
            fd.body
        );
    }
}

/// The pretty-printer prints an arrow-block body back as a block body, so the
/// two spellings converge on the one the parser keeps.
///
/// This is `print_module`, which is what an editor's format command gets over
/// the LSP and what `maca.fmt` answers over MCP. `maca fmt` on the command line
/// is a different thing on purpose: it re-indents the original text, because
/// the lexer drops comments and a print-based `fmt` would delete them.
#[test]
fn the_printer_prints_an_arrow_block_body_without_the_arrow() {
    let printed = print_module(&parsed("f() -> int => {\n    a = 1\n    a\n}\n"));
    assert!(
        printed.contains("f() -> int {") && !printed.contains("=>"),
        "expected a block body, got:\n{printed}"
    );
    // and the printed form parses to the same thing
    assert_eq!(
        parse(&printed).module,
        parsed("f() -> int {\n    a = 1\n    a\n}\n")
    );
}
