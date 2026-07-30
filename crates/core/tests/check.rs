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

/// A keyword Maca doesn't have gets told what Maca does instead.
///
/// `return x` parses as the identifier `return` beside `x`, reaches the C
/// backend, and comes out as `'return_mc' undeclared` — a message about a
/// mangled name in a file the programmer never wrote. These are the words
/// people actually reach for on their first day; each earns a real answer.
#[test]
fn phantom_keywords_explain_themselves() {
    for (src, want) in [
        ("f(n: int) -> int {\n    return n\n}\n", "no `return`"),
        ("f() -> int {\n    let x = 1\n    x\n}\n", "no `let`/`var`"),
        ("f() -> int {\n    var x = 1\n    x\n}\n", "no `let`/`var`"),
        ("f() -> int {\n    type T = 1\n    2\n}\n", "no `type`"),
        ("f() -> int {\n    null\n}\n", "no null"),
    ] {
        let parsed = maca_parser::parse(src);
        let d = check(&parsed.module, Mode::Program);
        let hit = d
            .iter()
            .find(|x| x.kind == DiagKind::UndefinedName && x.msg.contains(want));
        assert!(hit.is_some(), "no hint for {want:?} in {src:?}: {d:?}");
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
