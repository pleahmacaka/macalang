//! `std/` — the Maca-source standard library, executed.
//!
//! `std/` was a README describing builtins. It is now importable Maca: `text`,
//! `list`, `path`, `json`, `csv`, `fs`, `proc`. Each has a suite of `test_…`
//! functions under `std/tests/`, and this runs them all through `maca test` —
//! a library nothing runs is a claim rather than a fact, and the suites are
//! written in the language they ship with, so they also gate the compiler.
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

#[test]
fn proc_module() {
    suite("proc");
}

/// Every function `std/README.md` advertises is defined by the module it names.
///
/// The table is the first thing anyone reads to find out what `std/` holds, so
/// a name that drifted out of it — renamed, moved, removed — sends a reader to
/// a function that isn't there. The suites above prove the modules work; this
/// proves the index of them is honest.
#[test]
fn the_std_readme_names_functions_that_exist() {
    let readme = std::fs::read_to_string(repo().join("std/README.md")).expect("std/README.md");

    let mut checked = 0;
    for line in readme.lines() {
        let Some(rest) = line.strip_prefix("| `std/") else {
            continue;
        };
        let Some((module, body)) = rest.split_once("` | ") else {
            continue;
        };

        let src = std::fs::read_to_string(repo().join(format!("std/{module}.maca")))
            .unwrap_or_else(|_| panic!("std/README.md lists std/{module}, which has no source"));

        for name in backticked(body) {
            // `str_`-prefixed twins` and the like are prose, not a call.
            if name.ends_with('_') {
                continue;
            }
            assert!(
                src.lines().any(|l| l.starts_with(&format!("{name}("))),
                "std/README.md lists `{name}` under std/{module}, which does not define it"
            );
            checked += 1;
        }
    }
    assert!(checked > 40, "the table stopped being parsed: {checked} names");
}

/// The `` `name` `` spans in a README table cell.
fn backticked(cell: &str) -> Vec<String> {
    cell.split('`')
        .skip(1)
        .step_by(2)
        .filter(|s| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
        .map(str::to_string)
        .collect()
}
