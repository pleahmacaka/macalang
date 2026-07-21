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
fn generic_ok() {
    // Polymorphic functions instantiate per use: `id(5): int` and
    // `id("hello"): str` must both check with no false-positive mismatch.
    assert_clean("examples/generic.maca", Mode::Program);
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
