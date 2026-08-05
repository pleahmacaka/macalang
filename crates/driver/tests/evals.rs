mod common;
use common::*;

use std::process::Command;

/// A model that solves exactly one problem, so the harness's own arithmetic is what is measured.
const STAND_IN: &str = "#!/bin/sh\n\
    if grep -q below_zero \"$1\"; then\n\
    \x20 printf '%s\\n' 'below_zero(operations: int[]) -> bool {' \\\n\
    \x20   '    balance = 0' \\\n\
    \x20   '    for i in 0..operations.length() {' \\\n\
    \x20   '        balance = balance + operations[i]' \\\n\
    \x20   '        if balance < 0 { return true }' \\\n\
    \x20   '    }' \\\n\
    \x20   '    false' \\\n\
    \x20   '}'\n\
    else\n\
    \x20 echo 'nothing()'\n\
    fi\n";

fn stand_in() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("maca-evals-harness");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let script = dir.join("model.sh");
    std::fs::write(&script, STAND_IN).expect("write the stand-in");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("make it runnable");
    }
    script
}

/// The ported problems are committed, so they have to still be Maca the compiler accepts.
#[test]
fn every_ported_problem_compiles_as_a_stub() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = repo().join("apps/evals/problems");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir)
        .expect("the problems are committed")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "maca") {
            continue;
        }
        let out = Command::new(maca())
            .args(["check", &path.to_string_lossy()])
            .output()
            .expect("spawn maca check");
        assert!(
            out.status.success(),
            "{} does not check:\n{}",
            path.display(),
            String::from_utf8_lossy(&out.stdout)
        );
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} problems: the port shrank");
}

/// A stub has to fail its own test, or the harness would score an unwritten answer as a pass.
#[test]
fn a_stub_fails_the_test_it_ships_with() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let problem = repo().join("apps/evals/problems/below_zero.maca");
    let out = Command::new(maca())
        .args(["test", &problem.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    assert!(
        !out.status.success(),
        "the stub passed, so the harness cannot tell a written answer from an empty one"
    );
}

/// The harness's arithmetic: one solved problem out of the set is one, not zero and not all.
///
/// The table goes to a path this test owns. Writing the committed one turned a
/// measured number into whatever ran last, and a `cargo test` quietly replaced
/// a real model's result with the stand-in's.
///
/// `MACA` is the binary this test run built. Pointing it at `target/release`
/// is what made this pass here and fail in CI, which never builds one.
#[test]
fn the_harness_counts_what_the_model_actually_solved() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let baseline = std::env::temp_dir().join("maca-evals-harness/BASELINE.md");
    let out = Command::new(maca())
        .args(["run", "apps/evals/run.maca", &baseline.to_string_lossy()])
        .current_dir(repo())
        .env("MACA_EVAL_MODEL", stand_in())
        .env("MACA", maca())
        .output()
        .expect("spawn the harness");
    assert!(
        out.status.success(),
        "the harness failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let baseline = std::fs::read_to_string(&baseline).expect("the run writes a baseline");
    for condition in ["no spec", "spec", "spec + one check retry"] {
        assert!(
            baseline.contains(condition),
            "`{condition}` is missing from the baseline:\n{baseline}"
        );
    }
    assert!(
        baseline.contains("| 1 |"),
        "the stand-in solves exactly one problem and the baseline does not say so:\n{baseline}"
    );
    assert!(
        !baseline.contains("| 0 |"),
        "a condition scored zero, so grading is matching prose again:\n{baseline}"
    );
}

/// The README's commands are how a person reaches the harness, and `apps/` is not a search root, so a `-m` spelling of them cannot work.
#[test]
fn the_readme_names_a_command_that_resolves() {
    let readme =
        std::fs::read_to_string(repo().join("apps/evals/README.md")).expect("the README is here");

    let mut named = 0;
    for line in readme.lines() {
        let Some(rest) = line.split_once("maca run ").map(|(_, r)| r) else {
            continue;
        };
        let path = rest.split_whitespace().next().expect("a path after `run`");
        assert!(
            repo().join(path).is_file(),
            "the README runs `{path}`, which is not there"
        );
        named += 1;
    }
    assert!(named >= 2, "the README stopped naming how to run it");

    let out = Command::new(maca())
        .args(["-m", "evals"])
        .current_dir(repo())
        .output()
        .expect("spawn maca -m");
    assert!(
        !out.status.success(),
        "`-m evals` resolves after all, so the README's reason for a path is gone"
    );
}

/// Two runs of the harness must not write a problem's answer to the same file, or whichever wrote last grades both.
#[test]
fn each_run_grades_in_a_directory_of_its_own() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let tmp = std::env::temp_dir().join("maca-evals-scratch");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("scratch dir");

    let mut seen = Vec::new();
    for run in 0..2 {
        let baseline = tmp.join(format!("BASELINE-{run}.md"));
        let out = Command::new(maca())
            .args(["run", "apps/evals/run.maca", &baseline.to_string_lossy()])
            .current_dir(repo())
            .env("MACA_EVAL_MODEL", stand_in())
            .env("MACA", maca())
            .env("TMPDIR", &tmp)
            .output()
            .expect("spawn the harness");
        assert!(
            out.status.success(),
            "run {run} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        seen.push(std::fs::read_to_string(&baseline).expect("a baseline per run"));
    }
    assert_eq!(seen[0], seen[1], "the same model scored differently twice");

    let dirs: Vec<String> = std::fs::read_dir(&tmp)
        .expect("read the scratch")
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(dirs.len() >= 2, "both runs shared one directory: {dirs:?}");
}
