//! Variadic parameters: `f(...rest: T)` takes any number of trailing arguments
//! and sees them as a `T[]`.
//!
//! The behaviour is asserted in Maca, in `tests/programs/variadic.maca`, and run
//! by `maca test` — this file is the runner plus the two things that are about
//! the *process* rather than the values: a poisoned run, and the programs that
//! must fail to compile.

mod common;
use common::*;

use std::process::Command;

/// The whole suite, plain and with released blocks poisoned.
///
/// A variadic call builds a list at the call site and the callee may grow it in
/// place, so a stale handle to a freed buffer is exactly the failure this
/// feature could introduce. `MACA_POISON=1` fills a released block with `0xDD`,
/// which turns reading one into a wrong answer instead of a lucky one.
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

/// Too few fixed arguments. A variadic relaxes the *upper* bound only.
#[test]
fn too_few_fixed_arguments_is_rejected() {
    rejected("bad_variadic_arity", "expects at least 1 argument");
}

/// A variadic that is not the last parameter: nothing says where the collection
/// stops.
#[test]
fn a_variadic_before_another_parameter_is_rejected() {
    rejected("bad_variadic_not_last", "must be last");
}

/// Passed by name rather than called. There is no arity to pass.
#[test]
fn a_variadic_as_a_function_value_is_rejected() {
    rejected("bad_variadic_value", "cannot be used as a function value");
}

/// `spawn f(…)` bypasses the call site that would do the collecting, and a task
/// slot could not carry the list anyway. It used to print a pointer.
#[test]
fn spawning_a_variadic_is_rejected() {
    rejected("bad_variadic_spawn", "cannot take a variadic");
}

/// A variadic of function type. `arr_name` puts a closure element on the same
/// array name as an integer one, so declaring one redefined `IntArr` as an array
/// of closures and every `int[]` in the program stopped compiling.
#[test]
fn a_variadic_of_function_type_is_rejected() {
    rejected("bad_variadic_fn_type", "variadic of function type");
}

/// No annotation, so nothing says what the collected list holds. The back end
/// picks the integer array: `firstf(1.5, 2.5)` compiled clean and answered
/// `1.0`, which is the reason this is a rule and not a default.
#[test]
fn an_unannotated_variadic_is_rejected() {
    rejected("bad_variadic_unannotated", "needs its element type");
}

/// A variadic `main`. Nothing calls it, so nothing collects; the process hands
/// an entry point its command line as a `str[]`.
#[test]
fn a_variadic_main_is_rejected() {
    rejected("bad_variadic_main", "`main` cannot be variadic");
}
