mod common;
use common::*;

use maca_core::{Mode, check};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Source order: definitions before use.
const SELFHOST_FILES: &[&str] = &[
    "ty.maca",
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/selfhost")
}

fn read(name: &str) -> String {
    fs::read_to_string(selfhost_dir().join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// The whole front-end as one translation unit (the driver builds a single file; concatenation stands in for cross-file module resolution).
fn concatenated() -> String {
    SELFHOST_FILES
        .iter()
        .map(|n| read(n))
        .collect::<Vec<_>>()
        .join("\n")
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
    if have_wsl() || !have("cc") {
        eprintln!("skipping selfhost native run: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-selfhost-run");
    let _ = std::fs::create_dir_all(&dir);
    let bin = dir.join("frontend");

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
    assert!(
        stdout.contains("scanned 9 tokens from 14 chars"),
        "lexer output wrong: {stdout}"
    );
    assert!(
        stdout.contains("parsed: ((add(1) + 2) - 3)"),
        "parser/AST output wrong: {stdout}"
    );
    assert!(
        stdout.contains("precedence: (2 + (3 * 4))"),
        "operator precedence wrong: {stdout}"
    );
    assert!(
        stdout.contains("checked: type int, 0 errors"),
        "checker output wrong (good tree): {stdout}"
    );
    assert!(
        stdout.contains("type error, 1 errors"),
        "checker didn't flag the str+int clash: {stdout}"
    );
    assert!(
        stdout.contains("emitted: int main(void) { return ((add(1) + 2) - 3); }"),
        "emitter output wrong: {stdout}"
    );
    assert!(
        stdout.contains("fn: int add(int x, int y) { return (x + y);"),
        "function emission wrong: {stdout}"
    );
    assert!(
        stdout.contains("module (2 fns):")
            && stdout.contains("int inc(int n) { return (n + 1);")
            && stdout.contains("int dbl(int n) { return (n * 2);"),
        "module emission wrong: {stdout}"
    );
    assert!(
        stdout.contains("module check: 1 type errors"),
        "module type-check wrong: {stdout}"
    );
    assert!(
        stdout.contains("block fn: int sq(int n) { int t = (n * n); return t;"),
        "block-body emission wrong: {stdout}"
    );

    assert!(
        stdout.contains("multi-arg: add(1, 2)"),
        "multi-argument call parse wrong: {stdout}"
    );

    assert!(
        stdout.contains("ternary: ((a > b) ? a : b)"),
        "ternary parse wrong: {stdout}"
    );

    assert!(
        stdout.contains("unary: (0 - (-n))"),
        "unary negation parse wrong: {stdout}"
    );

    assert!(
        stdout.contains("modulo: ((a % b) + 1)"),
        "modulo precedence wrong: {stdout}"
    );

    assert!(
        stdout.contains("logic: (((a < b) && c) || d)"),
        "boolean operator precedence wrong: {stdout}"
    );

    assert!(
        stdout.contains("typed sig C:    int tag(const char* s, int n)"),
        "C parameter typing wrong: {stdout}"
    );
    assert!(
        stdout.contains("typed sig Rust: fn tag(s: String, n: i64) -> i64"),
        "Rust parameter typing wrong: {stdout}"
    );

    assert!(
        stdout.contains("bool fn C:    int flag() { return 1;"),
        "C bool lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("bool fn Rust: fn flag() -> bool { true"),
        "Rust bool typing wrong: {stdout}"
    );

    assert!(
        stdout.contains("float fn C:    double scale(double x) { return (x * 2.0);"),
        "C float lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("float fn Rust: fn scale(x: f64) -> f64 { (x * 2.0_f64)"),
        "Rust float lowering wrong: {stdout}"
    );

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

    assert!(
        stdout.contains("typedef enum { Red, Green, Blue } Color;")
            && stdout.contains("rank(Color c) { return ((c == Green) ? 42 : 0);"),
        "C sum lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("enum Color { Red, Green, Blue }") && stdout.contains("use Color::*;"),
        "Rust sum lowering wrong: {stdout}"
    );

    assert!(
        stdout.contains("match C:    int code(Color c) { return (c == Red ? 1 : (c == Green ? 42 : (c == Blue ? 3 : 0)));"),
        "C match lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("match Rust: fn code(c: Color) -> i64 { match c { Red => 1i64, Green => 42i64, Blue => 3i64,"),
        "Rust match lowering wrong: {stdout}"
    );

    assert!(
        stdout.contains(
            "streq C:    int kw(const char* w) { return ((strcmp(w, \"let\") == 0) ? 1 : 0);"
        ),
        "C string-equality lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("streq Rust: fn kw(w: String) -> i64 { (if (w == \"let\".to_string())"),
        "Rust string-equality lowering wrong: {stdout}"
    );

    assert!(
        stdout.contains("concat C:    const char* wrap(const char* s) { return maca_cat(maca_cat(\"[\", s), \"]\");"),
        "C string-concat lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("concat Rust: fn wrap(s: String) -> String { format!(\"{}{}\", format!(\"{}{}\", \"[\".to_string(), s), \"]\".to_string())"),
        "Rust string-concat lowering wrong: {stdout}"
    );

    assert!(
        stdout.contains(
            "str C:    const char* numbered(int n) { return maca_cat(\"#\", maca_int_to_str(n));"
        ),
        "C int-to-string lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("str Rust: fn numbered(n: i64) -> String { format!(\"{}{}\", \"#\".to_string(), format!(\"{}\", n))"),
        "Rust int-to-string lowering wrong: {stdout}"
    );

    assert!(
        stdout.contains("method C:    int slen(const char* s) { return ((int)strlen(s));"),
        "C method-call lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("method Rust: fn slen(s: String) -> i64 { ((s).len() as i64)"),
        "Rust method-call lowering wrong: {stdout}"
    );

    assert!(
        stdout.contains("at C:    int first(const char* s) { return ((int)s[0]);"),
        "C .at lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains(
            "at Rust: fn first(s: String) -> i64 { ((s).as_bytes()[(0i64) as usize] as i64)"
        ),
        "Rust .at lowering wrong: {stdout}"
    );

    assert!(
        stdout.contains("check arity: 1"),
        "a call with the wrong argument count should be an error: {stdout}"
    );
    assert!(
        stdout.contains("check return: 1"),
        "a body disagreeing with its declared return should be an error: {stdout}"
    );
    assert!(
        stdout.contains("check calls: 1"),
        "a call's declared return type should reach the arithmetic: {stdout}"
    );
    assert!(
        stdout.contains("check clean: 0"),
        "a correct module should check clean: {stdout}"
    );

    assert!(
        stdout.contains("typedef enum { Circle_tag, Rect_tag } Shape_tag;"),
        "C payload tags wrong: {stdout}"
    );
    assert!(
        stdout.contains("static inline Shape Rect(long _0, long _1) { Shape _v; _v.tag = Rect_tag; _v._0 = _0; _v._1 = _1; return _v; }"),
        "C payload constructor wrong: {stdout}"
    );
    assert!(
        stdout.contains("enum Shape { Circle(i64), Rect(i64, i64) }"),
        "Rust payload enum wrong: {stdout}"
    );
    assert!(
        stdout.contains("bind C:    int area(Shape s) { return (s.tag == Circle_tag ? ({ long r = s._0; (r * r); }) : (s.tag == Rect_tag ? ({ long w = s._0; long h = s._1; (w * h); }) : 0));"),
        "C payload binding wrong: {stdout}"
    );
    assert!(
        stdout.contains("bind Rust: fn area(s: Shape) -> i64 { match s { Circle(r) => (r * r), Rect(w, h) => (w * h),"),
        "Rust payload binding wrong: {stdout}"
    );

    assert!(
        stdout.contains(
            "interp C:    const char* label(int n) { return maca_cat(maca_cat(\"n = \", maca_int_to_str(n)), \"!\");"
        ),
        "C interpolation lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("interp Rust: fn label(n: i64) -> String { format!(\"{}{}\", format!(\"{}{}\", \"n = \".to_string(), format!(\"{}\", n)), \"!\".to_string())"),
        "Rust interpolation lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("interp expr: ((sum  ++ str((a + b))) ++  done)"),
        "interpolated expression wrong: {stdout}"
    );

    assert!(
        stdout.contains("array C:    int total(MacaList xs) { return ((((int)xs.data[0]) + ((int)xs.data[1])) + (xs.len));"),
        "C dynamic-array lowering wrong: {stdout}"
    );
    assert!(
        stdout.contains("array Rust: fn total(xs: Vec<i64>) -> i64 { ((xs[(0i64) as usize] + xs[(1i64) as usize]) + ((xs).len() as i64))"),
        "Rust dynamic-array lowering wrong: {stdout}"
    );

    assert!(
        stdout.contains("real file: 2 items after skipping imports"),
        "import skipping wrong: {stdout}"
    );

    let c_src = stdout
        .split_once("=== emitted program ===")
        .and_then(|(_, rest)| rest.split_once("=== end program ==="))
        .map(|(prog, _)| prog.trim().to_string())
        .expect("emitted-program markers present");
    assert!(
        c_src.contains("typedef struct { int x; int y; } Point;")
            && c_src.contains("typedef enum { Red, Green, Blue } Color;")
            && c_src.contains("#include <string.h>")
            && c_src.contains("int fld(Point p)")
            && c_src.contains("int code(Color c)")
            && c_src.contains("static char* maca_cat(")
            && c_src.contains("int hi(const char* a, const char* b)")
            && c_src.contains("static char* maca_int_to_str(")
            && c_src.contains("int chk(int n)")
            && c_src.contains("int slen(const char* s)")
            && c_src.contains("typedef struct { long* data; int len; } MacaList;")
            && c_src.contains("int head(MacaList xs)")
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
            cfile.to_string_lossy().as_ref(),
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
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("self-hosted!"),
        "emitted program didn't print via info: {:?}",
        String::from_utf8_lossy(&run.stdout)
    );

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
            && rs_src.contains("fn chk(n: i64) -> i64")
            && rs_src.contains("fn slen(s: String) -> i64")
            && rs_src.contains("fn head(xs: Vec<i64>) -> i64")
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
            rsfile.to_string_lossy().as_ref(),
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
    let rrun = Command::new(&rbin)
        .output()
        .expect("run emitted rust program");
    assert_eq!(
        rrun.status.code(),
        Some(42),
        "self-host-emitted Rust program returned {:?}, expected 42",
        rrun.status.code()
    );
    assert!(
        String::from_utf8_lossy(&rrun.stdout).contains("self-hosted!"),
        "emitted Rust program didn't print via info: {:?}",
        String::from_utf8_lossy(&rrun.stdout)
    );
}

/// The same run, watched.
#[test]
fn the_selfhost_front_end_reads_nothing_it_does_not_own() {
    if !have("cc") || have_wsl() || !have("valgrind") {
        eprintln!("skipping: needs a host cc, valgrind, and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();
    let dir = std::env::temp_dir().join("maca-selfhost-valgrind");
    fs::create_dir_all(&dir).expect("scratch dir");
    let bin = dir.join("frontend");

    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &selfhost_dir().join("main.maca").to_string_lossy(),
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

    let out = Command::new("valgrind")
        .args(["--error-exitcode=9", "-q"])
        .arg(&bin)
        .output()
        .expect("valgrind");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stderr.trim().is_empty(),
        "valgrind was not quiet:\n{stderr}"
    );
}

/// The differential gate: one source, two compilers, the same program.
#[test]
fn stage0_and_stage1_compile_the_same_program_the_same_way() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping differential: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-two-stage");
    let _ = std::fs::create_dir_all(&dir);

    let maca1 = dir.join("maca1");
    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &selfhost_dir().join("main.maca").to_string_lossy(),
            "-o",
            &maca1.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        build.status.success(),
        "stage-0 could not build stage-1:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let src = "Shape = Circle(int) | Rect(int, int)\n\
        area(s: Shape) -> int => match s { Circle(r) => r * r Rect(w, h) => w * h }\n\
        label(n: int) -> str => \"area = {n}\"\n\
        main() -> int {\n\
        \x20   a = area(Rect(6, 7))\n\
        \x20   info(label(a))\n\
        \x20   a\n\
        }\n";
    let prog = dir.join("prog.maca");
    std::fs::write(&prog, src).unwrap();

    let s0_bin = dir.join("s0");
    let b0 = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &prog.to_string_lossy(),
            "-o",
            &s0_bin.to_string_lossy(),
        ])
        .output()
        .expect("stage-0 build");
    assert!(
        b0.status.success(),
        "stage-0 build failed:\n{}",
        String::from_utf8_lossy(&b0.stderr)
    );
    let r0 = Command::new(&s0_bin).output().expect("run stage-0 output");

    let c_path = dir.join("prog1.c");
    let emit = Command::new(&maca1)
        .arg(&prog)
        .arg(&c_path)
        .output()
        .expect("stage-1 emit");
    assert!(
        emit.status.success(),
        "stage-1 reported {} check errors:\n{}",
        emit.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&emit.stderr)
    );
    let s1_bin = dir.join("s1");
    let cc = Command::new("cc")
        .arg(&c_path)
        .arg("-o")
        .arg(&s1_bin)
        .output()
        .expect("cc");
    assert!(
        cc.status.success(),
        "the C stage-1 emitted does not compile:\n{}\n--- source ---\n{}",
        String::from_utf8_lossy(&cc.stderr),
        std::fs::read_to_string(&c_path).unwrap_or_default()
    );
    let r1 = Command::new(&s1_bin).output().expect("run stage-1 output");

    assert_eq!(
        String::from_utf8_lossy(&r0.stdout),
        String::from_utf8_lossy(&r1.stdout),
        "stage-0 and stage-1 printed different things"
    );
    assert_eq!(
        r0.status.code(),
        r1.status.code(),
        "stage-0 and stage-1 exited differently"
    );
    assert_eq!(r1.status.code(), Some(42), "the program should exit 42");

    if have("rustc") {
        let rs_path = dir.join("prog1.rs");
        let emit_rs = Command::new(&maca1)
            .arg(&prog)
            .arg(&rs_path)
            .arg("rust")
            .output()
            .expect("stage-1 emit rust");
        assert!(emit_rs.status.success(), "stage-1 rust emit failed");
        let rs_bin = dir.join("s1rs");
        let rc = Command::new("rustc")
            .args(["-A", "warnings", "-o"])
            .arg(&rs_bin)
            .arg(&rs_path)
            .output()
            .expect("rustc");
        assert!(
            rc.status.success(),
            "the Rust stage-1 emitted does not compile:\n{}",
            String::from_utf8_lossy(&rc.stderr)
        );
        let r2 = Command::new(&rs_bin).output().expect("run rust output");
        assert_eq!(
            String::from_utf8_lossy(&r2.stdout),
            String::from_utf8_lossy(&r0.stdout),
            "the Rust back end disagreed with stage-0"
        );
        assert_eq!(r2.status.code(), Some(42));
    }
}

/// What the Maca-written parser reads and the two Maca-written back ends emit for an `if`, asserted in Maca.
#[test]
fn the_parser_written_in_maca_reads_an_if() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping selfhost parse suite: needs a host cc and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .args(["test", "apps/selfhost/tests/parse.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "apps/selfhost/tests/parse.maca:
{}
{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// What the Maca-written checker infers, rejects and leaves gradual, asserted in Maca.
#[test]
fn the_checker_written_in_maca_holds() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping selfhost check suite: needs a host cc and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .args(["test", "apps/selfhost/tests/check.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "apps/selfhost/tests/check.maca:
{}
{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// The type representation, unification, row unification and schemes, asserted in Maca.
#[test]
fn the_type_system_written_in_maca_holds() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping selfhost ty suite: needs a host cc and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .args(["test", "apps/selfhost/tests/ty.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "apps/selfhost/tests/ty.maca:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
