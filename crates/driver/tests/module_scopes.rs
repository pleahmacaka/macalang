mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

/// Two modules with the same private helper, a lambda capturing a parameter another module defines at top level, and a third module written against one of two same-named definitions it can reach.
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
        text.contains("4 tests passed"),
        "every test should have run:\n{text}"
    );
}
