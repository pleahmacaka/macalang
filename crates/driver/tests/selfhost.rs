//! Gate for the Maca-in-Maca sources under `selfhost/`.
//!
//! We can't run them natively in CI (no zig/WSL here), but the stage-0
//! front-end is exactly the tool that must accept them: every `selfhost/*.maca`
//! must parse with no errors, and the whole thing — concatenated so cross-file
//! references resolve — must type-/effect-check clean. That keeps the
//! self-hosted compiler honest as it grows.

use maca_core::{check, Mode};
use std::fs;
use std::path::PathBuf;

/// Source order: definitions before use.
const SELFHOST_FILES: &[&str] =
    &["token.maca", "ast.maca", "lexer.maca", "parser.maca", "main.maca"];

fn selfhost_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../selfhost")
}

fn read(name: &str) -> String {
    fs::read_to_string(selfhost_dir().join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

#[test]
fn every_selfhost_file_parses() {
    for name in SELFHOST_FILES {
        let src = read(name);
        let parsed = maca_parser::parse(&src);
        assert!(parsed.errors.is_empty(), "selfhost/{name} parse errors: {:?}", parsed.errors);
    }
}

#[test]
fn selfhost_module_typechecks() {
    // Concatenate in dependency order so `lexer.maca`/`main.maca` see the
    // `token.maca` declarations they reference.
    let src: String = SELFHOST_FILES.iter().map(|n| read(n)).collect::<Vec<_>>().join("\n");
    let parsed = maca_parser::parse(&src);
    assert!(parsed.errors.is_empty(), "selfhost parse errors: {:?}", parsed.errors);

    let diags = check(&parsed.module, Mode::Program);
    assert!(diags.is_empty(), "selfhost should type-check clean, got: {diags:?}");
}
