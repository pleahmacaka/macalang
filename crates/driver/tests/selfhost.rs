//! Gate for the Maca-in-Maca sources under `selfhost/`.
//!
//! Two levels of assurance:
//!  1. Every `selfhost/*.maca` must parse with no errors, and the whole thing —
//!     concatenated so cross-file references resolve — must type-/effect-check
//!     clean under the stage-0 front-end.
//!  2. Where a native toolchain is present, the compiler is actually *compiled
//!     and run*: the Maca-written lexer → parser (with precedence) → checker →
//!     two emitters (C and Rust) process expressions, functions, and whole
//!     modules. As a capstone, the complete program it emits is compiled two
//!     ways — the C by the host cc, the Rust by rustc — and both are executed;
//!     the matching exit codes prove the self-hosted compiler produced a working
//!     executable through each back end, not just a plausible-looking string.

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
    "emit_rust.maca",
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
    let bin = dir.join("frontend");

    // Build straight from the real entry point: the driver resolves the
    // `import selfhost/…` statements to the sibling modules and inlines them in
    // dependency order — no manual concatenation.
    let entry = selfhost_dir().join("main.maca");
    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &entry.to_string_lossy(),
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
    // precedence climbing: `*` binds tighter than `+`.
    assert!(
        stdout.contains("precedence: (2 + (3 * 4))"),
        "operator precedence wrong: {stdout}"
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
    // a whole function definition through lex → parse → emit: the two-char
    // operator lexer (`->`, `=>`), parameter parsing, and function emission all
    // exercise together, producing compilable C.
    assert!(
        stdout.contains("fn: int add(int x, int y) { return (x + y);"),
        "function emission wrong: {stdout}"
    );
    // a whole module (several functions) lowers to a complete C translation unit.
    assert!(
        stdout.contains("module (2 fns):")
            && stdout.contains("int inc(int n) { return (n + 1);")
            && stdout.contains("int dbl(int n) { return (n * 2);"),
        "module emission wrong: {stdout}"
    );
    // whole-module checking: the lexer scans string literals, and the checker
    // finds the single `int + str` clash across the module's two functions.
    assert!(
        stdout.contains("module check: 1 type errors"),
        "module type-check wrong: {stdout}"
    );
    // a block-bodied function: a local binding lowers to a C local, the trailing
    // expression to the `return`.
    assert!(
        stdout.contains("block fn: int sq(int n) { int t = (n * n); return t;"),
        "block-body emission wrong: {stdout}"
    );

    // multi-argument call: `add(1, 2)` parses to a two-argument call node.
    assert!(
        stdout.contains("multi-arg: add(1, 2)"),
        "multi-argument call parse wrong: {stdout}"
    );

    // ternary: `a > b ? a : b` parses to a conditional node (comparison inside).
    assert!(
        stdout.contains("ternary: ((a > b) ? a : b)"),
        "ternary parse wrong: {stdout}"
    );

    // unary minus: `-n` parses to a prefix negation that binds tighter than `-`.
    assert!(
        stdout.contains("unary: (0 - (-n))"),
        "unary negation parse wrong: {stdout}"
    );

    // modulo: `a % b` binds at precedence 5 (like `*`/`/`), tighter than `+`.
    assert!(
        stdout.contains("modulo: ((a % b) + 1)"),
        "modulo precedence wrong: {stdout}"
    );

    // boolean operators: comparison > `&&` > `||`, so the whole thing groups as
    // `((a < b && c) || d)`.
    assert!(
        stdout.contains("logic: (((a < b) && c) || d)"),
        "boolean operator precedence wrong: {stdout}"
    );

    // Capstone: compile and run the *complete program* the self-hosted compiler
    // emitted. Extract the C between the markers, compile it with the host cc,
    // run it, and check its exit code is `add(40, 2) == 42` — the Maca-written
    // compiler produced a working executable (multi-arg params + call args).
    let c_src = stdout
        .split_once("=== emitted program ===")
        .and_then(|(_, rest)| rest.split_once("=== end program ==="))
        .map(|(prog, _)| prog.trim().to_string())
        .expect("emitted-program markers present");
    assert!(
        c_src.contains("int grade(int x)") && c_src.contains("int main()"),
        "emitted program missing functions:\n{c_src}"
    );
    let cfile = dir.join("emitted.c");
    std::fs::write(&cfile, &c_src).unwrap();
    let ebin = dir.join("emitted");
    let cc = Command::new("cc")
        .args([
            &cfile.to_string_lossy().to_string(),
            "-o",
            &ebin.to_string_lossy(),
        ])
        .output()
        .expect("spawn cc");
    assert!(
        cc.status.success(),
        "self-host-emitted C failed to compile:\n{}\n--- source ---\n{c_src}",
        String::from_utf8_lossy(&cc.stderr)
    );
    let code = Command::new(&ebin)
        .status()
        .expect("run emitted program")
        .code();
    assert_eq!(
        code,
        Some(42),
        "self-host-emitted program returned {code:?}, expected grade(95) == 42"
    );

    // Capstone #2: the *same* program through the Maca-written Rust back end
    // (emit_rust.maca). Extract the emitted Rust, compile it with `rustc`, run
    // it, and check the exit code is again `sq(9) == 81` — proving the Rust
    // backend written in Maca produces a working executable, not just C.
    if !have("rustc") {
        eprintln!("skipping selfhost rust capstone: no rustc on PATH");
        return;
    }
    let rs_src = stdout
        .split_once("=== emitted rust ===")
        .and_then(|(_, rest)| rest.split_once("=== end rust ==="))
        .map(|(prog, _)| prog.trim().to_string())
        .expect("emitted-rust markers present");
    assert!(
        rs_src.contains("fn grade(x: i64) -> i64") && rs_src.contains("fn __maca_main() -> i64"),
        "emitted Rust missing functions:\n{rs_src}"
    );
    let rsfile = dir.join("emitted.rs");
    std::fs::write(&rsfile, &rs_src).unwrap();
    let rbin = dir.join("emitted_rs");
    let rustc = Command::new("rustc")
        .args([
            &rsfile.to_string_lossy().to_string(),
            "--edition",
            "2021",
            "-o",
            &rbin.to_string_lossy(),
        ])
        .output()
        .expect("spawn rustc");
    assert!(
        rustc.status.success(),
        "self-host-emitted Rust failed to compile:\n{}\n--- source ---\n{rs_src}",
        String::from_utf8_lossy(&rustc.stderr)
    );
    let rcode = Command::new(&rbin)
        .status()
        .expect("run emitted rust program")
        .code();
    assert_eq!(
        rcode,
        Some(42),
        "self-host-emitted Rust program returned {rcode:?}, expected grade(95) == 42"
    );
}
