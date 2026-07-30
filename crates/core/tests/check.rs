use maca_core::{DiagKind, Mode, check};
use std::path::PathBuf;

/// The source as the compiler sees it: with `import a/b` resolved and inlined.
///
/// Checking a file on its own reports every name it imports as undefined, and
/// this suite is what says an example type-checks — so an example that uses
/// `std/` would have had to avoid imports to stay green.
fn read(rel: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    maca_parser::imports::load_with_imports(&p).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn diags(rel: &str, mode: Mode) -> Vec<maca_core::Diagnostic> {
    let src = read(rel);
    let parsed = maca_parser::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "{rel} parse errors: {:?}",
        parsed.errors
    );
    check(&parsed.module, mode)
}

fn assert_clean(rel: &str, mode: Mode) {
    let d = diags(rel, mode);
    assert!(d.is_empty(), "{rel} should typecheck, got: {d:?}");
}

fn assert_has(rel: &str, mode: Mode, kind: DiagKind) {
    let d = diags(rel, mode);
    assert!(
        d.iter().any(|x| x.kind == kind),
        "{rel} expected {kind:?}, got: {d:?}"
    );
}

// ---- good examples typecheck --------------------------------------------

#[test]
fn hello_ok() {
    assert_clean("examples/hello.maca", Mode::Program);
}
#[test]
fn taskr_ok() {
    assert_clean("examples/taskr.maca", Mode::Program);
}
#[test]
fn counter_ok() {
    assert_clean("examples/counter.maca", Mode::Program);
}
#[test]
fn dot_ok() {
    assert_clean("examples/dot.maca", Mode::Program);
}
#[test]
fn system_ok() {
    assert_clean("examples/system.maca", Mode::Config);
}
#[test]
fn operators_ok() {
    // Operator overloading on a user type (`Vec2 + Vec2` → `add`) type-checks.
    assert_clean("examples/operators.maca", Mode::Program);
}
#[test]
fn generic_ok() {
    // Polymorphic functions instantiate per use: `id(5): int` and
    // `id("hello"): str` must both check with no false-positive mismatch.
    assert_clean("examples/generic.maca", Mode::Program);
}
#[test]
fn keywords_ok() {
    // identifiers that are C keywords are valid Maca and type-check normally.
    assert_clean("examples/keywords.maca", Mode::Program);
}
#[test]
fn indexing_ok() {
    // `xs[i]` (list/string subscript) and lvalue assignment type-check.
    assert_clean("examples/indexing.maca", Mode::Program);
}
#[test]
fn record_update_ok() {
    // `base with { field = value }` type-checks to the base record's type.
    assert_clean("examples/record_update.maca", Mode::Program);
}
#[test]
fn recursive_sum_ok() {
    // Recursive sum types (tree, linked list) type-check; the recursive payload
    // binds at the sum's own type inside match arms.
    assert_clean("examples/tree.maca", Mode::Program);
}
#[test]
fn recursive_record_ok() {
    // A record whose field is a list of its own type (`Tree { kids: Tree[] }`)
    // type-checks; the recursive field resolves at the record's own type.
    assert_clean("examples/recursive_record.maca", Mode::Program);
}
#[test]
fn sum_record_ok() {
    // A sum whose payload is a record declared later in the file type-checks;
    // the payload binds at the record's real type inside the match arm.
    assert_clean("examples/sum_record.maca", Mode::Program);
}

// ---- bad examples are rejected with the right diagnostic -----------------

#[test]
fn type_mismatch_rejected() {
    assert_has(
        "examples/bad/type_mismatch.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}
#[test]
fn nonexhaustive_rejected() {
    assert_has(
        "examples/bad/nonexhaustive.maca",
        Mode::Program,
        DiagKind::NonExhaustive,
    );
}
#[test]
fn effect_in_config_rejected() {
    assert_has(
        "examples/bad/effect_in_config.maca",
        Mode::Config,
        DiagKind::EffectInConfig,
    );
}
/// An effect in *statement* position, not only in a binding's value. The check
/// looked at what a binding was set to and nothing else, so a bare `info(…)`
/// was accepted and then dropped from the emitted Nix — the build succeeded
/// and the line was gone.
#[test]
fn effect_statement_in_config_rejected() {
    assert_has(
        "examples/bad/effect_statement_in_config.maca",
        Mode::Config,
        DiagKind::EffectInConfig,
    );
}
#[test]
fn unknown_option_rejected() {
    assert_has(
        "examples/bad/unknown_option.maca",
        Mode::Config,
        DiagKind::UnknownOption,
    );
}
#[test]
fn arg_mismatch_rejected() {
    // Calling `double(n: int)` with a string is a concrete argument clash.
    assert_has(
        "examples/bad/arg_mismatch.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}
#[test]
fn arity_rejected() {
    // `greet(name: str)` called with two arguments.
    assert_has(
        "examples/bad/arity.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}
/// A variadic parameter is a lowered declaration now, not a rejected one: the
/// call site collects its trailing arguments into the `T[]` the body sees.
///
/// The rules it does have — last, annotated, not `main`, not a function value —
/// and the arity relaxation are asserted end to end by
/// `crates/driver/tests/variadic.rs`, which runs them through the real compiler.
/// What this says is the part the checker owes: a program that uses one is
/// clean.
#[test]
fn variadic_typechecks() {
    assert_clean("crates/driver/tests/programs/variadic.maca", Mode::Program);
}

/// The rules it does have belong to *this* layer, so they are asserted here and
/// not only through the back end that would also refuse them. A variadic has no
/// arity as a value; a call that skips a fixed parameter is short one argument
/// the collection cannot invent; a parameter after the variadic has no way to be
/// told from one more of them; an unannotated one has no element type, so the
/// back end picks the integer array and a `float` element comes back truncated;
/// and `main` is handed its arguments by the process, not by a call site.
#[test]
fn variadic_misuse_rejected() {
    for bad in [
        "bad_variadic_value",
        "bad_variadic_arity",
        "bad_variadic_not_last",
        "bad_variadic_unannotated",
        "bad_variadic_main",
    ] {
        assert_has(
            &format!("crates/driver/tests/programs/{bad}.maca"),
            Mode::Program,
            DiagKind::TypeMismatch,
        );
    }
}
/// A record literal has to name every field the record declares.
///
/// A missing one was a silent zero, `""` for a `str` and `0` for an `int`, so a
/// value whose whole job is to carry copy could ship half-empty and compile
/// clean. `apps/site/home.maca` keeps a page's text in a 30-field record for
/// exactly the reason that a missing field should be a compile error.
#[test]
fn missing_record_field_rejected() {
    assert_has(
        "examples/bad/missing_field.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

/// And no field it doesn't declare. A misspelt name is worse than a missing
/// one: the value goes nowhere and the field it was meant for stays empty.
#[test]
fn unknown_record_field_rejected() {
    assert_has(
        "examples/bad/unknown_field.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

#[test]
fn branch_mismatch_rejected() {
    // Ternary branches with disagreeing types (int vs str).
    assert_has(
        "examples/bad/branch_mismatch.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

#[test]
fn loops_ok() {
    assert_clean("examples/loops.maca", Mode::Program);
}

#[test]
fn while_cond_must_be_bool() {
    assert_has(
        "examples/bad/while_cond.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

#[test]
fn payload_sum_ok() {
    assert_clean("examples/payload_sum.maca", Mode::Program);
}

#[test]
fn range_ok() {
    assert_clean("examples/range.maca", Mode::Program);
}

#[test]
fn tour_ok() {
    assert_clean("examples/tour.maca", Mode::Program);
}

#[test]
fn range_end_must_be_int() {
    assert_has(
        "examples/bad/range_end.maca",
        Mode::Program,
        DiagKind::TypeMismatch,
    );
}

#[test]
fn reassign_constant_rejected() {
    // a bare `count = 0` binds a constant; reassigning it is an error.
    assert_has(
        "examples/bad/reassign_const.maca",
        Mode::Program,
        DiagKind::Immutable,
    );
}

#[test]
fn mutable_reassign_ok() {
    // a bare lowercase binding is mutable — declare then reassign stays clean.
    assert_clean("examples/loops.maca", Mode::Program);
}

#[test]
fn collections_and_math_examples_typecheck() {
    assert_clean("examples/collections.maca", Mode::Program);
    assert_clean("examples/math.maca", Mode::Program);
    assert_clean("examples/dot.maca", Mode::Program);
}

#[test]
fn async_example_typechecks() {
    // await/spawn/sleep_ms type-check with no `async` keyword.
    assert_clean("examples/async.maca", Mode::Program);
}

#[test]
fn async_effect_is_inferred_and_banned_in_config() {
    // The async effect is inferred (never written); in config mode any effect is
    // rejected, which is how we observe that `sleep_ms`/`spawn`/`await` carry it.
    let src = "svc.value = spawn work(1)\n";
    let parsed = maca_parser::parse(src);
    let d = check(&parsed.module, Mode::Config);
    let eff = d.iter().find(|x| x.kind == DiagKind::EffectInConfig);
    assert!(eff.is_some(), "async effect not flagged in config: {d:?}");
    assert!(
        eff.unwrap().msg.contains("async"),
        "effect name missing: {}",
        eff.unwrap().msg
    );
}

#[test]
fn undefined_call_rejected() {
    // calling a name that is defined nowhere is caught as a clean diagnostic
    // instead of leaking a broken-C link error out of codegen.
    assert_has(
        "examples/bad/undefined_call.maca",
        Mode::Program,
        DiagKind::UndefinedName,
    );
}

#[test]
fn ui_element_tags_are_not_undefined() {
    // the reactive-UI DSL calls open-ended HTML tags (`div`, `input`, …) that
    // are intentionally undefined-looking; they must not be flagged.
    let src = "main() -> Element =>\n    div(\n        button(\"ok\")\n        input(placeholder=\"name\")\n    )\n";
    let parsed = maca_parser::parse(src);
    let d = check(&parsed.module, Mode::Program);
    assert!(
        !d.iter().any(|x| x.kind == DiagKind::UndefinedName),
        "UI tags wrongly flagged: {d:?}"
    );
}

/// A keyword Maca doesn't have gets told what Maca does instead, and told it
/// *first*.
///
/// `return x` parses as the identifier `return` beside `x`, reaches the C
/// backend, and comes out as `'return_mc' undeclared`, a message about a
/// mangled name in a file the programmer never wrote. These are the words
/// people actually reach for on their first day; each earns a real answer.
///
/// The order carries the weight. Phrased as an absence, "Maca has no `return`"
/// is what a reader takes away, and one did: they concluded a function could
/// not hand a value back, when Maca has exactly Rust's rule. So the working
/// form leads and the missing word is the aside, which is what this asserts.
#[test]
fn phantom_keywords_lead_with_what_maca_does() {
    for (word, opens_with) in [
        ("return", "a function's last expression is its value"),
        ("let", "write `x = e`"),
        ("var", "write `x = e`"),
        ("fn", "write the signature straight out"),
        ("func", "write the signature straight out"),
        ("def", "write the signature straight out"),
        ("type", "declare a type by binding it"),
        ("async", "async is an inferred effect"),
        ("null", "a sum type with an empty variant"),
        ("nil", "a sum type with an empty variant"),
    ] {
        let src = format!("f() -> int {{\n    {word}\n    1\n}}\n");
        let parsed = maca_parser::parse(&src);
        let d = check(&parsed.module, Mode::Program);
        let lead = format!("`{word}`: ");
        let hit = d
            .iter()
            .find(|x| x.kind == DiagKind::UndefinedName && x.msg.starts_with(&lead))
            .unwrap_or_else(|| panic!("no hint for `{word}`: {d:?}"));
        let hint = &hit.msg[lead.len()..];
        assert!(
            hint.starts_with(opens_with),
            "`{word}` should open with {opens_with:?}, opens with {hint:?}"
        );
        assert!(
            !hint.starts_with("Maca has no"),
            "`{word}` leads with the denial again: {hint:?}"
        );
    }
}

/// The hint fires only on names that are defined nowhere, so a program that
/// legitimately binds one of these words still compiles.
#[test]
fn a_defined_name_is_never_a_phantom_keyword() {
    let src = "type(n: int) -> int => n + 1\n\nmain() -> int => type(41)\n";
    let parsed = maca_parser::parse(src);
    let d = check(&parsed.module, Mode::Program);
    assert!(
        !d.iter().any(|x| x.kind == DiagKind::UndefinedName),
        "a user-defined `type` was flagged as a phantom keyword: {d:?}"
    );
}

// ---- a named record type and a record literal are one type ---------------

/// Check a source string in program mode.
fn check_src(src: &str) -> Vec<maca_core::Diagnostic> {
    let parsed = maca_parser::parse(src);
    assert!(parsed.errors.is_empty(), "{src}: {:?}", parsed.errors);
    check(&parsed.module, Mode::Program)
}

fn assert_src_clean(src: &str) {
    let d = check_src(src);
    assert!(d.is_empty(), "should typecheck:\n{src}\ngot: {d:?}");
}

/// `Point = { x: int, y: int }` and `{ x = 5, y = 6 }` are the same type.
///
/// They were not: the declared name is nominal, the literal is structural, and
/// unification saw two unrelated types. Naming the constructor worked, so the
/// language had both halves and no way to put them together, in every position
/// where a type is written down.
#[test]
fn a_record_literal_meets_the_record_type_it_is_written_into() {
    let decl = "Point = { x: int, y: int }\nLine = { a: Point, b: Point }\n\n";
    for (what, src) in [
        (
            "a local binding",
            "main() -> int {\n    p: Point = { x = 5, y = 6 }\n    p.x\n}\n",
        ),
        (
            "a top-level binding",
            "Origin: Point = { x = 0, y = 0 }\n\nmain() -> int => Origin.x\n",
        ),
        (
            "return position",
            "mk() -> Point => { x = 1, y = 2 }\n\nmain() -> int => mk().x\n",
        ),
        (
            "a parameter",
            "far(p: Point) -> int => p.x\n\nmain() -> int => far({ x = 1, y = 2 })\n",
        ),
        (
            "a nested field",
            "main() -> int {\n    l: Line = { a = { x = 1, y = 2 }, b = { x = 3, y = 4 } }\n    l.a.x\n}\n",
        ),
        (
            "a list element",
            "main() -> int {\n    ps: Point[] = [{ x = 1, y = 2 }]\n    ps[0].y\n}\n",
        ),
    ] {
        let d = check_src(&format!("{decl}{src}"));
        assert!(d.is_empty(), "{what} should typecheck:\n{src}\ngot: {d:?}");
    }
}

/// Meeting the declaration is also what checks the literal against it. A record
/// literal is open (field access is row-polymorphic), so left open here a field
/// nobody wrote would be silently zero, which is the bug the `Point { … }`
/// spelling already refuses.
#[test]
fn a_literal_written_into_a_record_type_must_name_its_fields() {
    for (src, want) in [
        (
            "main() -> int {\n    p: Point = { x = 5 }\n    p.x\n}\n",
            "missing field `y`",
        ),
        (
            "main() -> int {\n    p: Point = { x = 5, y = 6, z = 7 }\n    p.x\n}\n",
            "unexpected field `z`",
        ),
        (
            "main() -> int {\n    p: Point = { x = 5, y = \"six\" }\n    p.x\n}\n",
            "expected int, found str",
        ),
        // Naming none of them is the same mistake, not an exemption. An empty
        // literal used to be waved through, so `p: Point = {}` compiled and ran
        // with both fields zero, which is the silence the rest of this test is
        // about.
        (
            "main() -> int {\n    p: Point = {}\n    p.x\n}\n",
            "missing field `x`",
        ),
    ] {
        let d = check_src(&format!("Point = {{ x: int, y: int }}\n\n{src}"));
        assert!(
            d.iter()
                .any(|x| x.kind == DiagKind::TypeMismatch && x.msg.contains(want)),
            "expected {want:?} for:\n{src}\ngot: {d:?}"
        );
    }
}

/// Two anonymous literals still meet structurally, and stay open. Closing every
/// literal would have been the easy way to check the one above and would break
/// this.
#[test]
fn two_anonymous_record_literals_still_meet_structurally() {
    assert_src_clean(
        "pick(c: bool) -> int {\n    p = c ? { x = 1 } : { x = 2 }\n    p.x\n}\n\
         \nmain() -> int => pick(true)\n",
    );
}

/// A real mismatch still is one, and is reported the way the author wrote it:
/// the annotation is expected, the value is what was found. The pair used to
/// come out inverted, which is how the unification bug was first noticed.
#[test]
fn expected_names_the_annotation_and_found_names_the_value() {
    for (src, want) in [
        (
            "Point = { x: int, y: int }\n\nmain() -> int {\n    p: Point = 3\n    p.x\n}\n",
            "expected Point, found int",
        ),
        (
            "f() -> str => 42\n\nmain() -> int => 0\n",
            "expected str, found int",
        ),
        (
            "h(n: int) -> int => n\n\nmain() -> int => h(\"s\")\n",
            "expected int, found str",
        ),
        (
            "main() -> int {\n    while 7 {\n        break\n    }\n    0\n}\n",
            "expected bool, found int",
        ),
        (
            "main() -> int {\n    for i in \"a\"..\"b\" {\n        info(\"{i}\")\n    }\n    0\n}\n",
            "expected int, found str",
        ),
    ] {
        let d = check_src(src);
        assert!(
            d.iter()
                .any(|x| x.kind == DiagKind::TypeMismatch && x.msg.contains(want)),
            "expected {want:?} for:\n{src}\ngot: {d:?}"
        );
    }
}
