mod common;
use common::*;

use std::process::Command;

/// A module value is one value, computed once, with the type its initialiser answered.
///
/// A value nothing writes used to be emitted as a function, so every read ran
/// the initialiser again: `Stamp = now_ms()` was a different number each time
/// it was read. The declared type was guessed from the initialiser's shape
/// rather than taken from the lowering, so an intrinsic no signature table
/// holds was declared `const char*` and given an integer.
#[test]
fn a_module_value_is_computed_once_and_typed_by_its_initialiser() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(maca())
        .args(["test", &program("module_values").to_string_lossy()])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
