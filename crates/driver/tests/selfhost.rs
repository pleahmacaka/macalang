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

    // type threading: parameter and return types flow from the signature into
    // the emitted C (`str` → `const char*`) and Rust (`str` → `String`).
    assert!(
        stdout.contains("typed sig C:    int tag(const char* s, int n)"),
        "C parameter typing wrong: {stdout}"
    );
    assert!(
        stdout.contains("typed sig Rust: fn tag(s: String, n: i64) -> i64"),
        "Rust parameter typing wrong: {stdout}"
    );

    // bool: a `-> bool` function returning a literal — `true`/`false` lower to
    // `1`/`0` in C but stay keywords in Rust, and the return type differs.
    assert!(
        stdout.contains("bool fn C:    int flag() { return 1;"),
        "C bool lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("bool fn Rust: fn flag() -> bool { true"),
        "Rust bool typing wrong: {stdout}"
    );

    // float: the lexer scans `2.0` as one token; the type threads to `double`
    // (C) / `f64` (Rust), and the literal keeps its spelling (Rust suffixes it).
    assert!(
        stdout.contains("float fn C:    double scale(double x) { return (x * 2.0);"),
        "C float lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("float fn Rust: fn scale(x: f64) -> f64 { (x * 2.0_f64)"),
        "Rust float lowering wrong: {stdout}"
    );

    // records: a type declaration → a struct, a field access → member access.
    assert!(
        stdout.contains("typedef struct { int x; int y;")
            && stdout.contains("int sum(Point p) { return (p.x + p.y);"),
        "C record lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("struct Point { x: i64, y: i64 }")
            && stdout.contains("fn sum(p: Point) -> i64 { (p.x + p.y)"),
        "Rust record lowering wrong: {stdout}"
    );

    // sum types: a `A | B | C` declaration → a C/Rust enum; a bare variant
    // reference stays bare (Rust glob-imports the variants via `use Color::*`).
    assert!(
        stdout.contains("typedef enum { Red, Green, Blue } Color;")
            && stdout.contains("rank(Color c) { return ((c == Green) ? 42 : 0);"),
        "C sum lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("enum Color { Red, Green, Blue }")
            && stdout.contains("use Color::*;"),
        "Rust sum lowering wrong: {stdout}"
    );

    // match: a nullary-variant match lowers to a right-nested ternary chain in
    // C and a native `match` in Rust.
    assert!(
        stdout.contains("match C:    int code(Color c) { return (c == Red ? 1 : (c == Green ? 42 : (c == Blue ? 3 : 0)));"),
        "C match lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("match Rust: fn code(c: Color) -> i64 { match c { Red => 1i64, Green => 42i64, Blue => 3i64,"),
        "Rust match lowering wrong: {stdout}"
    );

    // string equality: `w == "let"` → `strcmp(w, "let") == 0` in C (with a
    // <string.h> preamble), a native `==` in Rust.
    assert!(
        stdout.contains("streq C:    int kw(const char* w) { return ((strcmp(w, \"let\") == 0) ? 1 : 0);"),
        "C string-equality lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("streq Rust: fn kw(w: String) -> i64 { (if (w == \"let\".to_string())"),
        "Rust string-equality lowering wrong: {stdout}"
    );

    // string concatenation: `a ++ b` → the `maca_cat` heap helper in C, a
    // `format!` in Rust.
    assert!(
        stdout.contains("concat C:    const char* wrap(const char* s) { return maca_cat(maca_cat(\"[\", s), \"]\");"),
        "C string-concat lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("concat Rust: fn wrap(s: String) -> String { format!(\"{}{}\", format!(\"{}{}\", \"[\".to_string(), s), \"]\".to_string())"),
        "Rust string-concat lowering wrong: {stdout}"
    );

    // a realistic file shape: leading `import` lines are skipped, leaving the
    // type/function definitions — 2 items here (a sum + a function).
    assert!(
        stdout.contains("real file: 2 items after skipping imports"),
        "import skipping wrong: {stdout}"
    );

    // Capstone: compile and run the *complete program* the self-hosted compiler
    // emitted. Extract the C between the markers, compile it with the host cc,
    // run it, and check its exit code is `sum(Point{40,2}) == 42` — the
    // Maca-written compiler produced a working executable using a record type,
    // a record literal, field access, and a record-typed parameter.
    let c_src = stdout
        .split_once("=== emitted program ===")
        .and_then(|(_, rest)| rest.split_once("=== end program ==="))
        .map(|(prog, _)| prog.trim().to_string())
        .expect("emitted-program markers present");
    assert!(
        c_src.contains("typedef struct { int x; int y;  } Point;")
            && c_src.contains("typedef enum { Red, Green, Blue } Color;")
            && c_src.contains("#include <string.h>")
            && c_src.contains("int fld(Point p)")
            && c_src.contains("int code(Color c)")
            && c_src.contains("static char* maca_cat(")
            && c_src.contains("int hi(const char* a, const char* b)")
            && c_src.contains("int main()"),
        "emitted program missing record/sum/functions:\n{c_src}"
    );
    assert!(
        c_src.contains("(Point){ .x = 40, .y = 9 }"),
        "C record literal (designated init) wrong:\n{c_src}"
    );
    assert!(
        c_src.contains("return p.x;")
            && c_src.contains("(c == Green ? 2 :")
            && c_src.contains("(strcmp(maca_cat(a, b), \"hello\") == 0)"),
        "C field access / match / concat wrong:\n{c_src}"
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
    let run = Command::new(&ebin).output().expect("run emitted program");
    assert_eq!(
        run.status.code(),
        Some(42),
        "self-host-emitted program returned {:?}, expected 42",
        run.status.code()
    );
    // the `info` builtin printed a line via the emitted `printf`.
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("self-hosted!"),
        "emitted program didn't print via info: {:?}",
        String::from_utf8_lossy(&run.stdout)
    );

    // Capstone #2: the *same* program through the Maca-written Rust back end
    // (emit_rust.maca). Extract the emitted Rust, compile it with `rustc`, run
    // it, and check the exit code is again `42` — proving the Rust
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
        rs_src.contains("struct Point { x: i64, y: i64 }")
            && rs_src.contains("enum Color { Red, Green, Blue }")
            && rs_src.contains("fn fld(p: Point) -> i64")
            && rs_src.contains("fn code(c: Color) -> i64")
            && rs_src.contains("fn hi(a: String, b: String) -> i64")
            && rs_src.contains("fn __maca_main() -> i64"),
        "emitted Rust missing record/sum/functions:\n{rs_src}"
    );
    assert!(
        rs_src.contains("Point { x: 40i64, y: 9i64 }") && rs_src.contains("match c { Red =>"),
        "Rust record literal / match wrong:\n{rs_src}"
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
    let rrun = Command::new(&rbin).output().expect("run emitted rust program");
    assert_eq!(
        rrun.status.code(),
        Some(42),
        "self-host-emitted Rust program returned {:?}, expected 42",
        rrun.status.code()
    );
    // the `info` builtin printed a line via the emitted `println!`.
    assert!(
        String::from_utf8_lossy(&rrun.stdout).contains("self-hosted!"),
        "emitted Rust program didn't print via info: {:?}",
        String::from_utf8_lossy(&rrun.stdout)
    );
}
