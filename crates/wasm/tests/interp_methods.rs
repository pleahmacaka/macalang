//! The playground interpreter is a third implementation of Maca's semantics,
//! alongside the C backend and the JS backend, and nothing compared it to
//! either. It is also the only place most people ever run the language, so a
//! wrong answer here teaches the wrong thing.
//!
//! Two defects these cover. `xs.parallel(f)` is in `maca_core::LIST_METHODS` and
//! the C backend lowers it, but the interpreter had no case for it, so it fell
//! through to the end of the call path. And that end was `Ok(Value::Unit)`, so
//! every unimplemented or misspelt call evaluated quietly to unit rather than
//! failing.
//!
//! Expected values are the answers the native backend gives for the same
//! expression. Native is the reference; a second implementation that disagrees
//! is wrong even when its own answer looks reasonable.

use maca_wasm::interp;

/// Run `body` as the whole of `main`, returning what it printed.
fn output(body: &str) -> String {
    let src = format!("main() -> int {{\n{body}\n    0\n}}\n");
    let p = maca_parser::parse(&src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let r = interp::run(&p.module);
    assert!(r.error.is_none(), "interpreter failed: {:?}", r.error);
    r.output.trim().to_string()
}

/// Run `body` expecting the interpreter to report an error.
fn failure(body: &str) -> String {
    let src = format!("main() -> int {{\n{body}\n    0\n}}\n");
    let p = maca_parser::parse(&src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let r = interp::run(&p.module);
    match r.error {
        Some(e) => e,
        None => panic!("expected an error, got output: {:?}", r.output),
    }
}

#[test]
fn parallel_maps_rather_than_answering_unit() {
    // There is no second thread here, so `parallel` and `map` agree; what must
    // not happen is the list vanishing.
    let out = output("    info(str([1, 2, 3].parallel(v => v * 2).sum()))");
    assert_eq!(out, "12");
}

#[test]
fn parallel_and_map_agree() {
    let out = output(
        "    a = [1, 2, 3].map(v => v + 1).sum()\n    b = [1, 2, 3].parallel(v => v + 1).sum()\n    info(\"{a}|{b}\")",
    );
    assert_eq!(out, "9|9");
}

#[test]
fn a_call_the_interpreter_does_not_know_is_an_error_not_unit() {
    // Returning unit made a typo answer quietly in the one place people try the
    // language out.
    let msg = failure("    info(str(no_such_helper(1)))");
    assert!(
        msg.contains("no_such_helper"),
        "error does not name the call: {msg}"
    );
}

#[test]
fn a_user_function_is_still_reached() {
    // The refusal must not have swallowed the lookup it comes after.
    let src =
        "double(n: int) -> int => n * 2\n\nmain() -> int {\n    info(str(double(21)))\n    0\n}\n";
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let r = interp::run(&p.module);
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.output.trim(), "42");
}

#[test]
fn a_sum_constructor_is_still_reached() {
    let src = "\
Shape = Circle(int) | Square(int)

area(s: Shape) -> int {
    match s {
        Circle(r) => r * r
        Square(w) => w * w
    }
}

main() -> int {
    info(str(area(Square(5))))
    0
}
";
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let r = interp::run(&p.module);
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.output.trim(), "25");
}

/// Every method in the two closed sets, with the answer the native backend
/// gives. A name the interpreter lacks now fails loudly, so this list is what
/// keeps the three implementations agreeing.
const CASES: &[(&str, &str)] = &[
    ("\"hello\".length()", "5"),
    ("\"aB\".upper()", "AB"),
    ("\"aB\".lower()", "ab"),
    ("\"  x  \".trim()", "x"),
    ("\"hello\".contains(\"ell\")", "true"),
    ("\"hello\".starts_with(\"he\")", "true"),
    ("\"hello\".ends_with(\"lo\")", "true"),
    // every occurrence, not just the first
    ("\"aXbXc\".replace(\"X\", \"-\")", "a-b-c"),
    // substr takes a length, slice takes an end
    ("\"hello\".substr(1, 3)", "ell"),
    ("\"hello\".slice(1, 3)", "el"),
    ("\"hello\".index_of(\"l\")", "2"),
    ("\"hello\".index_of(\"z\")", "-1"),
    ("\"ab\".repeat(3)", "ababab"),
    ("\"7\".pad_start(3, \"0\")", "007"),
    ("\"7\".pad_end(3, \"0\")", "700"),
    // the shortfall splits left-biased
    ("\"x\".pad_center(6, \".\")", "..x..."),
    ("\"a,b,c\".split(\",\").length()", "3"),
    ("\"abc\".chars().length()", "3"),
    ("\"abc\".at(1)", "b"),
    ("\" \".is_whitespace()", "true"),
    ("\"5\".is_ascii_digit()", "true"),
    ("\"z\".is_alpha()", "true"),
    ("[1, 2, 3].length()", "3"),
    ("[1, 2, 3].map(v => v * 2).sum()", "12"),
    ("[1, 2, 3, 4].filter(v => v > 2).length()", "2"),
    ("[1, 2, 3].reduce(0, (a, b) => a + b)", "6"),
    ("[1, 2, 3].fold(10, (a, b) => a + b)", "16"),
    // numeric, not the string ordering that would put 10 before 9. `join` is a
    // `str[]` method, so an int list is read through `get` instead.
    ("[10, 9, 2].sort().get(0)", "2"),
    ("[10, 9, 2].sort().get(2)", "10"),
    ("[\"b\", \"a\", \"c\"].sort().join(\"|\")", "a|b|c"),
    ("[1, 2, 3].reverse().get(0)", "3"),
    // push returns the list, not its new length
    ("[1, 2].push(3).get(2)", "3"),
    ("[1, 2].push(3).length()", "3"),
    ("[1, 2, 3].pop().length()", "2"),
    ("[1, 2, 3, 4].slice(1, 3).get(0)", "2"),
    ("[1, 2, 3, 4].slice(1, 3).length()", "2"),
    ("[1, 2].contains(2)", "true"),
    ("[5, 6].index_of(6)", "1"),
    ("[5, 6].index_of(9)", "-1"),
    ("[1, 2, 3].sum()", "6"),
    ("[3, 1, 2].min()", "1"),
    ("[3, 1, 2].max()", "3"),
    ("[7, 8].first()", "7"),
    ("[7, 8].last()", "8"),
    ("[7, 8].get(1)", "8"),
    ("[\"a\", \"b\"].join(\"-\")", "a-b"),
    ("[1, 2, 3].parallel(v => v * 2).sum()", "12"),
];

#[test]
fn every_method_answers_what_the_native_backend_answers() {
    let mut wrong = Vec::new();
    for (expr, want) in CASES {
        let got = output(&format!("    info(str({expr}))"));
        if &got != want {
            wrong.push(format!("{expr}: want {want}, got {got}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_name_in_both_closed_sets_is_exercised() {
    // Without this, adding a method to maca-core leaves the interpreter falling
    // through, which now fails loudly but only if something calls it.
    let joined: String = CASES.iter().map(|(e, _)| *e).collect::<Vec<_>>().join(" ");
    let mut missing = Vec::new();
    for m in maca_core::STR_METHODS.iter().chain(maca_core::LIST_METHODS) {
        if !joined.contains(&format!(".{m}(")) {
            missing.push(*m);
        }
    }
    assert!(missing.is_empty(), "not exercised: {missing:?}");
}
