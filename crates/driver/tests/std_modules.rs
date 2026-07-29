//! `std/` — the Maca-source standard library, executed.
//!
//! `std/` was a README describing builtins. It is now importable Maca: `text`,
//! `list`, `path`, `json`, `csv`, `fs`. Each has a suite of `test_…` functions
//! under `std/tests/`, and this runs them all through `maca test` — a library
//! nothing runs is a claim rather than a fact, and the suites are written in
//! the language they ship with, so they also gate the compiler.
//!
//! The assertions live in the Maca; this file only reports the exit code,
//! which `maca test` sets to the number of failed assertions.

use std::process::Command;

fn have_cc() -> bool {
    Command::new("cc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn have_wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn repo() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run every `test_…` function in `std/tests/<name>.maca`. The suite runs from
/// the repository root so `import std/…` resolves.
fn suite(name: &str) {
    if have_wsl() || !have_cc() {
        eprintln!("skipping std/{name}: needs a host cc and no wsl");
        return;
    }

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
        .args(["test", &format!("std/tests/{name}.maca")])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "std/{name}:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn text_module() {
    suite("text");
}

#[test]
fn list_module() {
    suite("list");
}

#[test]
fn path_module() {
    suite("path");
}

#[test]
fn json_module() {
    suite("json");
}

#[test]
fn csv_module() {
    suite("csv");
}

#[test]
fn fs_module() {
    suite("fs");
}
