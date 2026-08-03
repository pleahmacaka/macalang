//! `std/`: the Maca-source standard library, executed.
//!
//! `std/` was a README describing builtins. It is now importable Maca: `text`,
//! `list`, `path`, `json`, `csv`, `fs`, `proc`. Each has a suite of `test_…`
//! functions under `modules/std/tests/`, and this runs them all through
//! `maca test`. A library nothing runs is a claim rather than a fact, and the
//! suites are written in the language they ship with, so they also gate the
//! compiler.
//!
//! The assertions live in the Maca; this file only reports the exit code,
//! which `maca test` sets to the number of failed assertions.

mod common;
use common::*;

use std::process::Command;

/// Run every `test_…` function in `modules/std/tests/<name>.maca`. The suite
/// runs from
/// the repository root so `import std/…` resolves.
fn suite(name: &str) {
    package_suite(&format!("std/tests/{name}"));
}

/// The same, for any package: `modules/<path>.maca`.
///
/// Colour is forced off, because a suite that renders terminal output has to
/// assert on the text and the escape codes are decided by whether the test
/// runner happens to have a terminal attached.
fn package_suite(path: &str) {
    if have_wsl() || !have("cc") {
        eprintln!("skipping modules/{path}: needs a host cc and no wsl");
        return;
    }

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .args(["test", &format!("modules/{path}.maca")])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "modules/{path}:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn text_module() {
    suite("text");
}

/// `cli`, the command-line package: what a command accepts, read off one
/// value, and what it prints.
#[test]
fn cli_module() {
    package_suite("cli/tests/cli");
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
/// a name that drifted out of it (renamed, moved, removed) sends a reader to
/// a function that isn't there. The suites above prove the modules work; this
/// proves the index of them is honest.
#[test]
fn the_std_readme_names_functions_that_exist() {
    let readme = std::fs::read_to_string(repo().join("modules/std/README.md"))
        .expect("modules/std/README.md");

    let mut checked = 0;
    for line in readme.lines() {
        let Some(rest) = line.strip_prefix("| `std/") else {
            continue;
        };
        let Some((module, body)) = rest.split_once("` | ") else {
            continue;
        };

        let src = std::fs::read_to_string(repo().join(format!("modules/std/{module}.maca")))
            .unwrap_or_else(|_| {
                panic!("modules/std/README.md lists std/{module}, which has no source")
            });

        for name in backticked(body) {
            // `str_`-prefixed twins` and the like are prose, not a call.
            if name.ends_with('_') {
                continue;
            }
            assert!(
                src.lines().any(|l| l.starts_with(&format!("{name}("))),
                "modules/std/README.md lists `{name}` under std/{module}, which does not define it"
            );
            checked += 1;
        }
    }
    assert!(
        checked > 40,
        "the table stopped being parsed: {checked} names"
    );
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
