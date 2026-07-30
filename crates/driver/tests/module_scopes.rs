//! Modules that never imported each other, sharing one namespace.
//!
//! Inlining flattens every module into a single translation unit, so a name
//! written in one file could be answered by a definition in another. What the
//! resolver does about that is checked in `maca-parser`'s own suites, on the
//! combined source; what is here is the part only a real build can answer,
//! because both failures happened after resolution and before anything ran.
//!
//! The assertions are in Maca, in `tests/programs/scopes/suite.maca`. This is
//! the runner that checks the exit code.

mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

/// Two modules with the same private helper, and a lambda capturing a parameter
/// another module defines at top level.
///
/// Before the repair, neither half reached the assertions. The first was a
/// `TypeMismatch` in `alpha_reading` against beta's signature; the second was
/// `incompatible type for argument 2 of 'prefixed'` out of the C compiler, the
/// captured `column` having been lowered as a function value.
#[test]
fn modules_do_not_answer_for_each_others_names() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let suite = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/scopes/suite.maca");
    let _lock = BuildLock::acquire();
    let out = Command::new(maca())
        .args(["test", &suite.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "the suite did not pass:\n{text}");
    assert!(
        text.contains("3 tests passed"),
        "every test should have run:\n{text}"
    );
}
