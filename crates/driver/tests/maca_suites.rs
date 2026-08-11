mod common;
use common::*;

use std::process::Command;

/// The suites that stand in for a Rust test file, run by the compiler written in Maca.
///
/// These are named rather than globbed, because a suite that imports `maca/*` recompiles the
/// whole compiler and a run of every suite costs more CI time than the job has. Each one here
/// imports only `std/*`, so the seven together take about fifteen seconds. The compiler's own
/// suites are gated one at a time by `selfhost.rs`, which is where an expensive one belongs.
const TWINS: &[&str] = &[
    "tests/version.maca",
    "tests/house_style.maca",
    "modules/maca/tests/corpus.maca",
    "modules/maca/tests/module_entry.maca",
    "modules/maca/tests/handbook.maca",
    "modules/maca/tests/proc.maca",
    "modules/maca/tests/tooling.maca",
];

/// Run under stage-1, not stage-0: these suites use `capture_err`, which the frozen bootstrap
/// has no lowering for, and a diagnostic is read off stderr.
#[test]
fn the_suites_that_replace_a_rust_test_hold() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping the Maca twins: needs a host cc and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();

    let dir = std::env::temp_dir().join("maca-twins");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let stage1 = dir.join("maca1");
    let entry = repo().join("apps").join("maca1").join("main.maca");
    let build = Command::new(maca())
        .current_dir(repo())
        .args([
            "build",
            &entry.to_string_lossy(),
            "-o",
            &stage1.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        build.status.success(),
        "stage-0 could not build stage-1:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let mut failed: Vec<String> = Vec::new();
    for suite in TWINS {
        let out = Command::new(&stage1)
            .current_dir(repo())
            .env("MACA", &stage1)
            .env("NO_COLOR", "1")
            .args(["test", suite])
            .output()
            .expect("spawn maca test");
        if !out.status.success() {
            failed.push(format!(
                "{}\n{}\n{}",
                suite,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}
