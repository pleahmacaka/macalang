mod common;
use common::*;

use std::process::Command;

/// The installer is a Maca program, so the rules it decides an install by are checked the way every other suite is.
#[test]
fn the_installer_rules_hold() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let out = Command::new(maca())
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .args(["test", "apps/install/tests/install.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// A name a C header already defines must not reach the emitted C, because the preprocessor wins that argument.
#[test]
fn a_local_named_after_a_c_macro_is_renamed() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-c-macro-name");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("m.maca");
    std::fs::write(
        &file,
        "main() -> int {\n    unix = 1\n    linux = 2\n    unix + linux - 3\n}\n",
    )
    .unwrap();
    let out = Command::new(maca())
        .current_dir(repo())
        .args(["run", &file.to_string_lossy()])
        .output()
        .expect("spawn maca run");

    assert!(
        out.status.success(),
        "`unix` and `linux` are macros every C compiler predefines:\n{}",
        String::from_utf8_lossy(&out.stderr),
    );
}
