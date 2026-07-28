//! `std/` — the Maca-source standard library, executed.
//!
//! `std/` was a README describing builtins. It is now importable Maca: `text`,
//! `list`, `path`, `json`, `csv`, `fs`. Each has a program under `std/tests/`
//! that exercises it, and this runs them all through `maca run` — a library
//! nothing runs is a claim rather than a fact, and these are written in the
//! language they ship with, so they also gate the compiler.

use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn repo() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run `std/tests/<name>.maca` from the repository root (so `import std/…`
/// resolves) and return its stdout.
fn run(name: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
        .args(["run", &format!("std/tests/{name}.maca")])
        .output()
        .expect("spawn maca run");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "std/{name} failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

fn check(name: &str, wants: &[&str]) {
    if wsl() || !have("cc") {
        eprintln!("skipping std/{name}: needs a host cc and no wsl");
        return;
    }
    let out = run(name);
    for w in wants {
        assert!(out.contains(w), "std/{name}: expected {w:?} in:\n{out}");
    }
}

#[test]
fn text_module() {
    check(
        "text",
        &[
            // a trailing newline doesn't invent a final empty line
            "lines: 3 2",
            "words: a|b|c",
            "once: a|b=c / x|",
            "strip: bar foo bar",
            "count: 2 2",
            "title: Hello Wide World",
        ],
    );
}

#[test]
fn list_module() {
    check(
        "list",
        &[
            "any/all: true true false",
            "find: 4 3",
            "take/drop: 3,1,4 | 2,6 | 8",
            "chunk: 3 2,6",
            "zip_add: 11,22",
            "flatten: 1,2,3,4,5",
            "unique: 3,1,4,5,9,2,6",
            "range: 2,3,4,5,6 0",
        ],
    );
}

#[test]
fn path_module() {
    check(
        "path",
        &[
            // one separator, whatever each side brought
            "join: a/b a/b b a",
            "dir: /x/y . /",
            // a leading dot is a hidden file, not an extension
            "ext: [gz] [] []",
            "with: a/b.html b.txt",
            "norm: a/c / x",
        ],
    );
}

#[test]
fn json_module() {
    check(
        "json",
        &[
            "obj: {\"name\":\"Ada\",\"age\":36,\"tags\":[\"math\",\"code\"]}",
            "get: [Ada] [36] []",
            "int: 36 -1",
            "items: math|code",
            // a comma inside a string, and a nested container, are not separators
            "nested: 4 / 1 ~ [2, 3] ~ a,b ~ {\"k\": 4}",
        ],
    );
}

#[test]
fn csv_module() {
    check(
        "csv",
        &[
            "field: [plain] [\"a,b\"] [\"say \"\"hi\"\"\"]",
            "r1: [Ada] [likes, commas]",
            "r2: [Bob] [says \"hi\"]",
            // a newline inside a quoted field keeps the record together
            "multi: 1 3 [line1/line2]",
        ],
    );
}

#[test]
fn fs_module() {
    check(
        "fs",
        &[
            "walk: 3",
            "dirs: 2",
            "find: 2 1",
            "lines: one|two",
            // rewriting a file with its own contents changes nothing
            "changed1: false",
            "changed2: true",
            "copied: 3 3",
            "cleaned: false",
        ],
    );
}
