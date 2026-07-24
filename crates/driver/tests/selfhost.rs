//! Gate for the Maca-in-Maca sources under `selfhost/`.
//!
//! Two levels of assurance:
//!  1. Every `selfhost/*.maca` must parse with no errors, and the whole thing —
//!     concatenated so cross-file references resolve — must type-/effect-check
//!     clean under the stage-0 front-end.
//!  2. Where a native toolchain is present, the concatenated front-end is
//!     actually *compiled and run*: the Maca-written lexer → recursive-descent
//!     parser → AST pretty-printer must produce the expected output. That is the
//!     real self-hosting milestone — the compiler's own front-end, written in
//!     Maca, executing as a native binary.

use maca_core::{Mode, check};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Source order: definitions before use.
const SELFHOST_FILES: &[&str] = &[
    "token.maca",
    "ast.maca",
    "lexer.maca",
    "parser.maca",
    "check.maca",
    "emit_c.maca",
    "main.maca",
];

fn selfhost_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../selfhost")
}

fn read(name: &str) -> String {
    fs::read_to_string(selfhost_dir().join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// The whole front-end as one translation unit (the driver builds a single
/// file; concatenation stands in for cross-file module resolution).
fn concatenated() -> String {
    SELFHOST_FILES
        .iter()
        .map(|n| read(n))
        .collect::<Vec<_>>()
        .join("\n")
}

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

#[test]
fn every_selfhost_file_parses() {
    for name in SELFHOST_FILES {
        let src = read(name);
        let parsed = maca_parser::parse(&src);
        assert!(
            parsed.errors.is_empty(),
            "selfhost/{name} parse errors: {:?}",
            parsed.errors
        );
    }
}

#[test]
fn selfhost_module_typechecks() {
    let src = concatenated();
    let parsed = maca_parser::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "selfhost parse errors: {:?}",
        parsed.errors
    );

    let diags = check(&parsed.module, Mode::Program);
    assert!(
        diags.is_empty(),
        "selfhost should type-check clean, got: {diags:?}"
    );
}

#[test]
fn selfhost_frontend_compiles_and_runs() {
    // Needs the host-cc native path (the C backend links with `cc` when there's
    // no WSL/Nix). Skip cleanly where neither is available.
    if wsl() || !have("cc") {
        eprintln!("skipping selfhost native run: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-selfhost-run");
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join("frontend.maca");
    std::fs::write(&src, concatenated()).unwrap();
    let bin = dir.join("frontend");

    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &src.to_string_lossy(),
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        build.status.success(),
        "selfhost front-end failed to build:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&bin).output().expect("run selfhost front-end");
    let stdout = String::from_utf8_lossy(&run.stdout);
    // lexer: `add(1) + 2 - 3` scans to 9 tokens (8 + Eof) over 14 chars.
    assert!(
        stdout.contains("scanned 9 tokens from 14 chars"),
        "lexer output wrong: {stdout}"
    );
    // parser + AST printer: left-associative, call folded — the recursive
    // `Expr` type round-trips through native codegen.
    assert!(
        stdout.contains("parsed: ((add(1) + 2) - 3)"),
        "parser/AST output wrong: {stdout}"
    );
    // checker (check.maca): the well-typed tree infers `int` with no errors,
    // and a `str + int` clash is reported as a type error.
    assert!(
        stdout.contains("checked: type int, 0 errors"),
        "checker output wrong (good tree): {stdout}"
    );
    assert!(
        stdout.contains("type error, 1 errors"),
        "checker didn't flag the str+int clash: {stdout}"
    );
    // emitter (emit_c.maca): the AST lowers to a C translation unit.
    assert!(
        stdout.contains("emitted: int main(void) { return ((add(1) + 2) - 3); }"),
        "emitter output wrong: {stdout}"
    );
}
