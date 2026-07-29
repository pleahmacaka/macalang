//! Perceus: the code generator inserts the drops, and the buffer comes back.
//!
//! The runtime has always had a size-tracked allocator with a free-list. What
//! it did not have was anyone calling `drop` — so a run-once program held
//! every buffer it ever allocated until exit. These tests are about the other
//! half: codegen releasing a local's buffer when the local cannot outlive its
//! block, and the next allocation of that size picking it up instead of
//! calling malloc.
//!
//! The valgrind test is the one that matters. Reuse without correctness is a
//! use-after-free, and the failure is silent.
//!
//! The programs live beside this file, in `tests/programs/`, and assert in
//! Maca: each returns `failures()`, so a non-zero exit is the whole verdict.

mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Compile `tests/programs/<name>.maca` to a native binary.
fn build(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("maca-memory-test");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let bin = dir.join(name);

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &program(name).to_string_lossy(),
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");

    assert!(
        out.status.success(),
        "{name} failed to build:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin
}

/// Run a built program and assert it reported no failed assertions.
fn assert_passes(bin: &Path) {
    let out = Command::new(bin).output().expect("run");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// A loop that builds and discards a value reuses one buffer instead of asking
/// the allocator for a new one every time round.
#[test]
fn a_discarded_buffer_is_reused() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    assert_passes(&build("reuse"));
}

/// The correctness half, under valgrind: nothing still reachable is freed.
#[test]
fn nothing_live_is_dropped() {
    if have_wsl() || !have("cc") || !have("valgrind") {
        eprintln!("skipping: needs a host cc, valgrind, and no wsl");
        return;
    }

    let bin = build("live");
    let out = Command::new("valgrind")
        .args(["--error-exitcode=9", "--leak-check=full", "-q"])
        .arg(&bin)
        .output()
        .expect("valgrind");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "{}\n{stderr}",
        String::from_utf8_lossy(&out.stdout),
    );
    assert!(
        stderr.trim().is_empty(),
        "valgrind was not quiet:\n{stderr}"
    );
}

/// Nested array types are declared before the types that hold them.
#[test]
fn nested_array_types_are_declared_before_use() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    assert_passes(&build("nested_arrays"));
}
