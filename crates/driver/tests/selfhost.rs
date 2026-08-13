use maca_core::{Mode, check};
use std::fs;
use std::path::PathBuf;

/// Source order: definitions before use.
const COMPILER_FILES: &[&str] = &[
    "ty.maca",
    "token.maca",
    "ast.maca",
    "styles.maca",
    "lexer.maca",
    "parser.maca",
    "check.maca",
    "emit_c.maca",
    "emit_rust.maca",
    "emit_js.maca",
    "emit_nix.maca",
    "emit_embedded.maca",
    "emit_jvm.maca",
    "print.maca",
];

/// The compiler itself, which a program reaches as the `maca` package.
fn compiler_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../modules/maca")
}

/// The binary that drives it.
fn cli_main() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/maca1/main.maca")
}

fn read(name: &str) -> String {
    fs::read_to_string(compiler_dir().join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// The whole front-end as one translation unit (the driver builds a single file; concatenation stands in for cross-file module resolution).
fn concatenated() -> String {
    let mut parts: Vec<String> = COMPILER_FILES.iter().map(|n| read(n)).collect();
    parts.push(fs::read_to_string(cli_main()).expect("read apps/maca1/main.maca"));
    parts.join("\n")
}

#[test]
fn every_selfhost_file_parses() {
    for name in COMPILER_FILES {
        let src = read(name);
        let parsed = maca_parser::parse(&src);
        assert!(
            parsed.errors.is_empty(),
            "maca/{name} parse errors: {:?}",
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
