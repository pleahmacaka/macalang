mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

fn maca_test(name: &str) -> (bool, String) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs")
        .join(format!("{name}.maca"));
    let out = Command::new(maca())
        .args(["test", &path.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
fn a_binding_shadows_a_foreign_namespace_of_the_same_name() {
    if !have("cc") {
        eprintln!("skipping: no cc");
        return;
    }
    let (ok, text) = maca_test("ffi_names");
    assert!(ok, "{text}");
    assert!(text.contains("5 tests passed"), "{text}");
}

#[test]
fn a_genuine_foreign_call_still_reaches_the_library() {
    if !have("cc") {
        eprintln!("skipping: no cc");
        return;
    }
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/ffi_sqlite.maca");
    if !example.exists() {
        eprintln!("skipping: no ffi example");
        return;
    }
    let out = Command::new(maca())
        .arg("run")
        .arg(&example)
        .output()
        .expect("spawn maca run");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        eprintln!("skipping: sqlite not linkable here:\n{text}");
        return;
    }
    assert!(
        text.contains("ada is 36"),
        "the result set did not come back:\n{text}"
    );
}
