mod common;
use common::*;

use std::process::Command;

/// The whole suite, plain and with released blocks poisoned.
#[test]
fn variadic_arguments_collect_into_a_list() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let program = program("variadic");
    for poison in ["0", "1"] {
        let out = Command::new(maca())
            .args(["test", &program.to_string_lossy()])
            .env("MACA_POISON", poison)
            .output()
            .expect("spawn maca test");
        assert!(
            out.status.success(),
            "MACA_POISON={poison}:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// A program that must not compile, and the words the diagnostic owes the reader.
fn rejected(name: &str, expect: &str) {
    let program = program(name);
    let out = Command::new(maca())
        .args(["build", &program.to_string_lossy()])
        .output()
        .expect("spawn maca build");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success(),
        "{name} compiled, and should not:\n{said}"
    );
    assert!(
        said.contains(expect),
        "{name} was rejected for the wrong reason (wanted {expect:?}):\n{said}"
    );
}

/// Too few fixed arguments.
#[test]
fn too_few_fixed_arguments_is_rejected() {
    rejected("bad_variadic_arity", "expects at least 1 argument");
}

/// A variadic that is not the last parameter: nothing says where the collection stops.
#[test]
fn a_variadic_before_another_parameter_is_rejected() {
    rejected("bad_variadic_not_last", "must be last");
}

/// Passed by name rather than called.
#[test]
fn a_variadic_as_a_function_value_is_rejected() {
    rejected("bad_variadic_value", "cannot be used as a function value");
}

/// `spawn f(…)` bypasses the call site that would do the collecting, and a task slot could not carry the list anyway.
#[test]
fn spawning_a_variadic_is_rejected() {
    rejected("bad_variadic_spawn", "cannot take a variadic");
}

/// A variadic of function type.
#[test]
fn a_variadic_of_function_type_is_rejected() {
    rejected("bad_variadic_fn_type", "variadic of function type");
}

/// No annotation, so nothing says what the collected list holds.
#[test]
fn an_unannotated_variadic_is_rejected() {
    rejected("bad_variadic_unannotated", "needs its element type");
}

/// A variadic `main`.
#[test]
fn a_variadic_main_is_rejected() {
    rejected("bad_variadic_main", "`main` cannot be variadic");
}
