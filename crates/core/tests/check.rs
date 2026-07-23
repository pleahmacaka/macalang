use maca_core::{check, DiagKind, Mode};
use std::fs;
use std::path::PathBuf;

fn read(rel: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    fs::read_to_string(p).unwrap()
}

fn diags(rel: &str, mode: Mode) -> Vec<maca_core::Diagnostic> {
    let src = read(rel);
    let parsed = maca_parser::parse(&src);
    assert!(parsed.errors.is_empty(), "{rel} parse errors: {:?}", parsed.errors);
    check(&parsed.module, mode)
}

fn assert_clean(rel: &str, mode: Mode) {
    let d = diags(rel, mode);
    assert!(d.is_empty(), "{rel} should typecheck, got: {d:?}");
}

fn assert_has(rel: &str, mode: Mode, kind: DiagKind) {
    let d = diags(rel, mode);
    assert!(d.iter().any(|x| x.kind == kind), "{rel} expected {kind:?}, got: {d:?}");
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
fn sum_record_ok() {
    // A sum whose payload is a record declared later in the file type-checks;
    // the payload binds at the record's real type inside the match arm.
    assert_clean("examples/sum_record.maca", Mode::Program);
}

// ---- bad examples are rejected with the right diagnostic -----------------

#[test]
fn type_mismatch_rejected() {
    assert_has("examples/bad/type_mismatch.maca", Mode::Program, DiagKind::TypeMismatch);
}
#[test]
fn nonexhaustive_rejected() {
    assert_has("examples/bad/nonexhaustive.maca", Mode::Program, DiagKind::NonExhaustive);
}
#[test]
fn effect_in_config_rejected() {
    assert_has("examples/bad/effect_in_config.maca", Mode::Config, DiagKind::EffectInConfig);
}
#[test]
fn unknown_option_rejected() {
    assert_has("examples/bad/unknown_option.maca", Mode::Config, DiagKind::UnknownOption);
}
#[test]
fn arg_mismatch_rejected() {
    // Calling `double(n: int)` with a string is a concrete argument clash.
    assert_has("examples/bad/arg_mismatch.maca", Mode::Program, DiagKind::TypeMismatch);
}
#[test]
fn arity_rejected() {
    // `greet(name: str)` called with two arguments.
    assert_has("examples/bad/arity.maca", Mode::Program, DiagKind::TypeMismatch);
}
#[test]
fn branch_mismatch_rejected() {
    // Ternary branches with disagreeing types (int vs str).
    assert_has("examples/bad/branch_mismatch.maca", Mode::Program, DiagKind::TypeMismatch);
}

#[test]
fn loops_ok() {
    assert_clean("examples/loops.maca", Mode::Program);
}

#[test]
fn while_cond_must_be_bool() {
    assert_has("examples/bad/while_cond.maca", Mode::Program, DiagKind::TypeMismatch);
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
    assert_has("examples/bad/range_end.maca", Mode::Program, DiagKind::TypeMismatch);
}

#[test]
fn reassign_constant_rejected() {
    // a bare `count = 0` binds a constant; reassigning it is an error.
    assert_has("examples/bad/reassign_const.maca", Mode::Program, DiagKind::Immutable);
}

#[test]
fn mutable_reassign_ok() {
    // a bare lowercase binding is mutable — declare then reassign stays clean.
    assert_clean("examples/loops.maca", Mode::Program);
}

#[test]
fn undefined_call_rejected() {
    // calling a name that is defined nowhere is caught as a clean diagnostic
    // instead of leaking a broken-C link error out of codegen.
    assert_has("examples/bad/undefined_call.maca", Mode::Program, DiagKind::UndefinedName);
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
