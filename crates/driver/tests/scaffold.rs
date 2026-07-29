//! `maca init` scaffolds a working project (hermetic — no toolchain needed).

mod common;
use common::*;

use std::process::Command;

#[test]
fn init_scaffolds_a_valid_project() {
    let dir = std::env::temp_dir().join("maca-init-test-proj");
    let _ = std::fs::remove_dir_all(&dir);

    let out = Command::new(maca())
        .arg("init")
        .arg(&dir)
        .output()
        .expect("spawn maca");
    assert!(
        out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // the three scaffolded files exist
    let toml = std::fs::read_to_string(dir.join("maca.toml")).expect("maca.toml");
    let main = std::fs::read_to_string(dir.join("main.maca")).expect("main.maca");
    assert!(dir.join(".gitignore").exists(), ".gitignore missing");

    // maca.toml carries the expected sections
    assert!(toml.contains("[package]"), "no [package]:\n{toml}");
    assert!(toml.contains("[format]"), "no [format]:\n{toml}");
    assert!(toml.contains("[scripts]"), "no [scripts]:\n{toml}");

    // the generated program actually parses and type-checks
    let parsed = maca_parser::parse(&main);
    assert!(
        parsed.errors.is_empty(),
        "scaffold parse errors: {:?}",
        parsed.errors
    );
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    assert!(
        diags.is_empty(),
        "scaffold should type-check, got: {diags:?}"
    );
}
