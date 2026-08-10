mod common;
use common::*;

use std::process::Command;

/// Build the self-hosted compiler once, so each check below runs the binary a clean checkout would.
fn maca1(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    if have_wsl() || !have("cc") {
        eprintln!("skipping maca1 cli: needs a host cc and no wsl");
        return None;
    }
    let _lock = BuildLock::acquire();
    std::fs::create_dir_all(dir).unwrap();
    let bin = dir.join("maca1");
    let out = Command::new(maca())
        .current_dir(repo())
        .args([
            "build",
            "apps/maca1/main.maca",
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        out.status.success(),
        "the self-hosted compiler must build:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(bin)
}

/// `run` is `build` plus the exit code of what it built, which is what a script wants from it.
#[test]
fn maca1_runs_a_program_and_hands_back_its_exit_code() {
    let dir = std::env::temp_dir().join("maca1-cli-run");
    let Some(bin) = maca1(&dir) else { return };
    let file = dir.join("r.maca");
    std::fs::write(&file, "main() -> int {\n    info(\"ran\")\n    7\n}\n").unwrap();

    let out = Command::new(&bin)
        .args(["run", &file.to_string_lossy()])
        .output()
        .expect("spawn maca1 run");

    assert_eq!(out.status.code(), Some(7), "the program's own code, not 0");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("ran"),
        "and its output reaches the caller"
    );
}

/// `test` finds every `test_…` function, runs each, and exits with how many assertions failed.
#[test]
fn maca1_runs_every_test_function_and_counts_the_failures() {
    let dir = std::env::temp_dir().join("maca1-cli-test");
    let Some(bin) = maca1(&dir) else { return };
    let file = dir.join("t.maca");
    std::fs::write(
        &file,
        "test_one_holds() {\n    assert_eq(\"a\", \"a\", \"same\")\n}\n\n\
         test_one_does_not() {\n    assert_eq(\"a\", \"b\", \"different\")\n}\n",
    )
    .unwrap();

    let out = Command::new(&bin)
        .args(["test", &file.to_string_lossy()])
        .output()
        .expect("spawn maca1 test");

    assert_eq!(
        out.status.code(),
        Some(1),
        "one of the two failed, so the count is one:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("test_one_holds"),
        "each test names itself as it runs"
    );
}

/// A suite that imports resolves from where its own file is, not from wherever a scratch file landed.
#[test]
fn maca1_runs_a_suite_that_imports_a_module() {
    let dir = std::env::temp_dir().join("maca1-cli-import");
    let Some(bin) = maca1(&dir) else { return };

    let out = Command::new(&bin)
        .current_dir(repo())
        .args(["test", "modules/std/tests/json.maca"])
        .output()
        .expect("spawn maca1 test");

    assert_eq!(
        out.status.code(),
        Some(0),
        "the import graph is walked from the suite's own directory:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
