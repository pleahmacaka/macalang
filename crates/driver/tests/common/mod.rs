#![allow(dead_code, unused_imports)]

use std::path::PathBuf;

pub use maca_testsupport::{BuildLock, have, have_jdk, have_wsl, to_wsl, unsupported_host};

/// The repository root, from this crate's manifest directory.
pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `maca` binary this `cargo test` built.
pub fn maca() -> &'static str {
    env!("CARGO_BIN_EXE_maca")
}

/// A golden example by file name.
pub fn example(name: &str) -> PathBuf {
    repo().join("examples").join(name)
}

/// A golden example as a path string, for the suites that pass it to a `Command` rather than opening it.
pub fn example_str(name: &str) -> String {
    example(name).display().to_string()
}

/// A test program from `tests/programs`.
pub fn program(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs")
        .join(format!("{name}.maca"))
}
