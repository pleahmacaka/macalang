mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

/// Every Maca-written suite, run by the compiler written in Maca.
///
/// Naming the directories rather than the files is what makes a suite added in Maca gate itself,
/// which is what lets a Rust test with a Maca twin be deleted. It runs under **stage-1**, not
/// stage-0, because a suite may use a builtin stage-1 has and the frozen bootstrap does not:
/// `capture_err` is the first, and the suites that read a diagnostic all need it.
fn suites() -> Vec<PathBuf> {
    let roots = [
        repo().join("modules").join("maca").join("tests"),
        repo().join("tests"),
    ];
    let mut found: Vec<PathBuf> = roots
        .iter()
        .filter_map(|r| std::fs::read_dir(r).ok())
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "maca"))
        .collect();
    found.sort();
    found
}

#[test]
fn every_maca_suite_holds_under_the_compiler_written_in_maca() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping the Maca suites: needs a host cc and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();

    let dir = std::env::temp_dir().join("maca-suites");
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

    let all = suites();
    assert!(
        all.len() >= 20,
        "expected the Maca suites, found {}",
        all.len()
    );

    let mut failed: Vec<String> = Vec::new();
    for suite in &all {
        let rel = suite.strip_prefix(repo()).unwrap_or(suite);
        let out = Command::new(&stage1)
            .current_dir(repo())
            .env("MACA", &stage1)
            .env("NO_COLOR", "1")
            .args(["test", &rel.to_string_lossy()])
            .output()
            .expect("spawn maca test");
        if !out.status.success() {
            failed.push(format!(
                "{}\n{}\n{}",
                rel.display(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}
