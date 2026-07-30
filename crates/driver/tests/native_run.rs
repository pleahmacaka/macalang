//! Native-toolchain smoke test: with no WSL but a host `cc`, `maca run` must
//! compile and execute a program end to end. Skips if neither is usable.

mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

#[test]
fn hello_runs_natively() {
    // Only meaningful on a plain Linux host (no WSL) with a C compiler.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("hello.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Hello, World"),
        "expected greeting.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `apps/cli_tool/` — the `cli` package used the way a program uses it.
///
/// An application, not a fixture, so it lives under `apps/` with the other
/// programs and is named by its written path: `apps/` is deliberately not an
/// import search root.
///
/// Run three ways, because a command line has three outcomes and only one of
/// them is the happy path: help printed and nothing done, a refusal naming what
/// was wrong, and the work itself.
#[test]
fn the_cli_example_helps_refuses_and_runs() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let bin = std::env::temp_dir().join("maca-cli-example");
    let src = repo().join("apps/cli_tool/cli_tool.maca");
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
        "cli_tool failed to build:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let say = |args: &[&str]| {
        let o = Command::new(&bin)
            .args(args)
            .env("NO_COLOR", "1")
            .current_dir(repo())
            .output()
            .expect("run cli_tool");
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };

    let (ok, out, _) = say(&["--help"]);
    assert!(ok, "help exits cleanly");
    assert!(out.contains("-s, --sort"), "help lists the options:\n{out}");
    assert!(
        out.contains("(default: name)"),
        "and their defaults:\n{out}"
    );

    let (ok, _, e) = say(&["modules/cli", "--srot", "lines"]);
    assert!(!ok, "a misspelt option is a failure");
    assert!(
        e.contains("did you mean `--sort`"),
        "and names the one meant:\n{e}"
    );

    let (ok, out, e) = say(&["modules/cli", "--top", "3", "--totals"]);
    assert!(ok, "the real run succeeds:\n{e}");
    assert!(out.contains("parse.maca"), "lists files:\n{out}");
    assert!(
        out.contains("3 file(s) counted"),
        "and honours --top:\n{out}"
    );
    assert!(out.contains("bytes"), "and --totals:\n{out}");
}

/// The other four package demos under `apps/`, each run to its last line.
///
/// Nothing compiled these while they sat in `examples/`, and under `apps/` they
/// need it: `apps/bench_demo` imports `bench/…` from a directory whose sibling
/// is `apps/bench`, which is the shape that hides a package. That collision is
/// refused rather than resolved silently, but only for a build that happens, so
/// this is the build that happens. The needle is the program's own last line, so
/// a demo that compiles and then stops halfway is not counted as passing.
#[test]
fn the_package_demos_run_natively() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    for (app, last) in [
        ("bench_demo", "5 cases:"),
        ("profile_demo", "the first four moments"),
        ("signal_demo", "nothing to patch"),
        ("tambo_demo", "unanswered route names: 0"),
    ] {
        let src = repo().join(format!("apps/{app}/{app}.maca"));
        let out = Command::new(env!("CARGO_BIN_EXE_maca"))
            .args(["run", &src.to_string_lossy()])
            .current_dir(repo())
            .output()
            .expect("spawn maca run");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains(last),
            "{app} did not reach its last line ({last:?}).\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn recursion_and_arithmetic_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-native-test");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("fib.maca");
    std::fs::write(&f, "fib(n: int) -> int =>\n    n < 2 ? n : fib(n - 1) + fib(n - 2)\n\nmain() -> int {\n    info(\"{fib(20)}\")\n    0\n}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = PathBuf::from(&f);
    assert!(
        stdout.contains("6765"),
        "fib(20) should be 6765, got: {stdout}"
    );
}

#[test]
fn multi_file_imports_resolve_and_run() {
    // `maca build main.maca` inlines local `import` modules in dependency order.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-multifile");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mathlib.maca"), "twice(n: int) -> int => n * 2\n").unwrap();
    std::fs::write(
        dir.join("main.maca"),
        "import mathlib\n\nmain() -> int {\n    info(\"{twice(21)}\")\n    0\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &dir.join("main.maca").to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("42"),
        "import didn't resolve: stdout {stdout}\nstderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn selective_import_runs_and_drops_unused() {
    // `import { name } from module` inlines only the named definition and its
    // same-module dependency closure; everything else in the module is dropped.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-selective-import");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mathutil.maca"),
        "square(x: int) -> int => x * x\n\
         cube(x: int) -> int => square(x) * x\n\
         boom(x: int) -> int => x / 0\n", // referencing this would divide by zero
    )
    .unwrap();
    std::fs::write(
        dir.join("main.maca"),
        "import { cube } from mathutil\n\nmain() -> int {\n    info(\"cube(4)={cube(4)}\")\n    cube(4)\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &dir.join("main.maca").to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("cube(4)=64"),
        "selective import didn't resolve cube/square: stdout {stdout}\nstderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // `cube` exits with 64; had `boom` been inlined and reached it'd trap, but
    // it's dropped entirely — the closure pulled in only cube + square.
    assert_eq!(out.status.code(), Some(64), "exit code should be cube(4)");
}

#[test]
fn higher_order_params_run_natively() {
    // A function passed by name to an unannotated `pred` parameter, then called
    // inside the callee — the C backend wraps the fn in a closure and lowers the
    // param call through the closure ABI.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-higher-order");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("ho.maca");
    std::fs::write(
        &f,
        "even(n: int) -> bool => n % 2 == 0\n\
         count_if(xs: int[], i: int, pred) -> int =>\n\
             i >= xs.length() ? 0 : (pred(xs.get(i)) ? 1 : 0) + count_if(xs, i + 1, pred)\n\n\
         main() -> int {\n\
             xs = 1, 2, 3, 4, 5, 6\n\
             info(\"evens={count_if(xs, 0, even)}\")\n\
             0\n\
         }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("evens=3"),
        "higher-order param wrong: stdout {stdout}\nstderr {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn recursive_record_runs_natively() {
    // A recursive record (`Tree { kids: Tree[] }`) compiles through the C
    // backend's forward-declaration path and walks correctly at run time.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("recursive_record.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("tree has 5 nodes"),
        "recursive record walk wrong.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn operator_overloading_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    // (1,2)+(3,4) = (4,6); *2 = (8,12)
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("operators.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("8,12"),
        "operator overloading wrong.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn sqlite_ffi_runs_natively_with_system_sqlite() {
    // Real C FFI: on a plain Linux host the driver links system sqlite with the
    // host cc. Needs libsqlite3 headers; skip where they're absent.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let has_sqlite = std::path::Path::new("/usr/include/sqlite3.h").exists();
    if wsl || !have("cc") || !has_sqlite {
        eprintln!("skipping: needs a native cc + system sqlite3 and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("ffi_sqlite.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // rows come back ordered by age: ada (36) then alan (41)
    for want in ["ada is 36", "alan is 41"] {
        assert!(
            stdout.contains(want),
            "missing {want:?}.\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn math_prelude_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("math.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for want in [
        "sqrt(144)=12.0",
        "pow(2,10)=1024.0",
        "abs=7",
        "clamp=10",
        "gcd(54,24)=6",
    ] {
        assert!(stdout.contains(want), "missing {want:?}: {stdout}");
    }
}

#[test]
fn closures_and_collections_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("collections.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for want in [
        "shifted0=15",
        "total=15",
        "names=alice, bob, eve",
        "inc(41)=42",
        "has4=true",
    ] {
        assert!(
            stdout.contains(want),
            "missing {want:?}.\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn async_spawn_await_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    // two spawned tasks resolve to 20 and 40; awaiting both sums to 60.
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("async.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("20 + 40 = 60"),
        "async result wrong.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn string_stdlib_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("strings.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // split → 3 cols; trim/lower normalize; upper/contains/replace/substr/index_of
    for want in [
        "cols: 3",
        "name",
        "age",
        "city",
        "EMPLOYEES TABLE",
        "has Table: true",
        "prefix true suffix true",
        "Employees View",
        "Employees",
        "index 10",
        "---------",
        "007",
        "ab...",
        "    hi",
    ] {
        assert!(
            stdout.contains(want),
            "missing {want:?}.\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn fail_exits_cleanly_with_message() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-fail-test");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("fail.maca");
    // `fail msg` must print "error: <msg>" to stderr and exit 1 (not SIGABRT)
    std::fs::write(
        &f,
        "check(n: int) -> int {\n    if n < 0 {\n        fail \"negative input\"\n    }\n    n\n}\n\nmain() -> int {\n    info(\"{check(0 - 1)}\")\n    0\n}\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("error: negative input"), "stderr: {stderr}");
    assert_eq!(
        out.status.code(),
        Some(1),
        "fail should exit 1, got {:?}",
        out.status.code()
    );
}

#[test]
fn match_guards_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("match_guard.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        vec!["negative", "zero", "small", "big", "two"],
        "stdout: {stdout}"
    );
}

#[test]
fn string_match_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-strmatch-test");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("m.maca");
    std::fs::write(&f, "route(c: str) -> int {\n    match c {\n        \"add\" => 1\n        \"del\" => 2\n        _ => 0\n    }\n}\n\nmain() -> int {\n    a = route(\"add\")\n    b = route(\"del\")\n    z = route(\"x\")\n    info(\"{a} {b} {z}\")\n    0\n}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1 2 0"), "stdout: {stdout}");
}

#[test]
fn or_patterns_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("or_patterns.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["warm", "cold", "low", "high"],
        "stdout: {stdout}"
    );
}

#[test]
fn payload_sums_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("payload_sum.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["100", "12"],
        "stdout: {stdout}"
    );
}

#[test]
fn try_catches_failure_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("catch.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // the failure is caught: execution continues and exits cleanly
    assert!(stdout.contains("recovered"), "stdout: {stdout}");
    assert_eq!(
        out.status.code(),
        Some(0),
        "try should catch and exit 0, got {:?}",
        out.status.code()
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).is_empty(),
        "no error should be printed"
    );
}

#[test]
fn lambda_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("lambda.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["1", "4", "9", "16"],
        "stdout: {stdout}"
    );
}

#[test]
fn generics_monomorphize_and_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    // record instantiation is the case a single-int64_t stamp cannot compile
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("generic_record.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["hello", "42", "7"],
        "stdout: {stdout}"
    );
}

#[test]
fn record_patterns_and_ops_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("record_pattern.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["4", "true", "side: right"],
        "stdout: {stdout}"
    );
}

#[test]
fn c_keyword_identifiers_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("keywords.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["10", "20", "7 hi", "99"],
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn indexing_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("indexing.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["10 30", "3", "m", "4", "99", "42 2"],
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn record_update_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("record_update.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec![
            "localhost:80 tls=false",
            "localhost:443 tls=true",
            "localhost:8080 tls=false"
        ],
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn recursive_sum_types_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    // boxed recursive payloads: tree fold + list length
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("tree.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["6", "3"],
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn sum_with_record_payload_runs_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    // sum declared before the record it carries: the combined topo order must
    // define the record struct first, or the C won't compile.
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("sum_record.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["12", "0"],
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn microkernel_boots_natively() {
    // the capstone stress test: a whole microkernel simulation must compile and
    // run end to end (and exercises the build cache on the second CI build).
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let src = format!(
        "{}/../../apps/microkernel/kernel.maca",
        env!("CARGO_MANIFEST_DIR")
    );
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &src])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("maca microkernel — boot"),
        "no boot banner:\n{stdout}"
    );
    assert!(
        stdout.contains("cap fault"),
        "capability check missing:\n{stdout}"
    );
    assert!(
        stdout.contains("tasks retired  : 5/6"),
        "unexpected scheduler outcome:\n{stdout}"
    );
    assert_eq!(out.status.code(), Some(0), "kernel should halt cleanly");
}

#[test]
fn file_io_builtins_run_natively() {
    // read_file / write_file / file_exists / make_dir / list_dir — the
    // filesystem primitives a build tool needs.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    // start from a clean directory so `list_dir` counts are deterministic
    let _ = std::fs::remove_dir_all("/tmp/maca_fileio_example");
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("fileio.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for want in [
        "read 4 lines",    // read_file round-trips what write_file wrote
        "one.md exists",   // file_exists is true for a written file
        "nope.md missing", // ...and false for an absent one
        "pages: 2",        // make_dir + list_dir see both files
        "first: one.md",   // list_dir is sorted, so builds are reproducible
        "second: two.md",
    ] {
        assert!(
            stdout.contains(want),
            "missing {want:?}.\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// `maca test` — chapter 12 of the handbook, executed.
///
/// The suites live in `tests/programs/testsuite/` rather than in a Rust string
/// literal, because they are the same Maca the chapter prints.
#[test]
fn maca_test_runs_test_prefixed_functions() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }

    // Passing: two tests, plus a `main` that must be replaced rather than run.
    let (ok, out, _) = maca_test("passing");
    assert!(ok, "passing tests should exit 0:\n{out}");
    assert!(out.contains("running 2 tests"), "count wrong:\n{out}");
    assert!(out.contains("2 tests passed"), "summary wrong:\n{out}");
    assert!(
        !out.contains("main must not run"),
        "the file's own main was executed:\n{out}"
    );

    // Failing: every test still runs, each is marked, and the exit code is the
    // number of failed assertions — two, from three tests.
    let (ok, out, code) = maca_test("failing");
    assert!(!ok, "a failing suite must exit non-zero");
    assert_eq!(code, Some(2), "the exit code is the failure count:\n{out}");
    assert!(out.contains("running 3 tests"), "count wrong:\n{out}");
    assert!(
        out.contains("2 assertion(s) failed"),
        "summary wrong:\n{out}"
    );
    for want in ["got:  got", "want: want", "one is not greater than two"] {
        assert!(out.contains(want), "expected {want:?} in:\n{out}");
    }
    // The passing test after two failures still ran and still reported.
    assert!(
        out.contains("test_a_passing_one_still_runs\n    ok"),
        "a later test was skipped:\n{out}"
    );

    // `fail` is not `assert`: it ends the program where it happened.
    let (ok, out, _) = maca_test("aborting");
    assert!(!ok, "`fail` must exit non-zero");
    assert!(
        out.contains("test_broken") && !out.contains("test_after"),
        "output should stop at the aborting test:\n{out}"
    );
    assert!(out.contains("deliberate"), "failure message lost:\n{out}");

    // A file with no tests is not an error.
    let (ok, out, _) = maca_test("no_tests");
    assert!(ok, "no tests should not be a failure:\n{out}");
    assert!(out.contains("no tests found"), "should say so:\n{out}");
}

/// Run `tests/programs/testsuite/<name>.maca` through `maca test`. Returns
/// success, stdout+stderr together, and the exit code.
fn maca_test(name: &str) -> (bool, String, Option<i32>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs/testsuite")
        .join(format!("{name}.maca"));
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["test", &path.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text, out.status.code())
}

#[test]
fn functions_without_a_declared_return_type_return_their_value() {
    // An arrow body *is* the function's value. Before this was fixed, a
    // function with no `-> T` discarded its body and fell off the end of a
    // non-void C function, returning garbage (and often segfaulting).
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-inferred-ret");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("ret.maca");
    std::fs::write(
        &f,
        // no `-> T` anywhere: a plain arithmetic body, a comparison (bool), a
        // string concat, and a call through a higher-order parameter
        "inc(x) => x + 1\n\
         big(x) => x > 10\n\
         greet(n) => \"hi \" ++ n\n\
         twice(g, x) => g(g(x))\n\
         main() -> int {\n\
             name = \"maca\"\n\
             info(\"inc={inc(41)}\")\n\
             info(\"big={big(50)}\")\n\
             info(\"greet={greet(name)}\")\n\
             info(\"twice={twice(n => n + 1, 40)}\")\n\
             0\n\
         }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for want in ["inc=42", "big=true", "greet=hi maca", "twice=42"] {
        assert!(
            stdout.contains(want),
            "missing {want:?} — an inferred return type was mishandled.\n\
             stdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert_eq!(out.status.code(), Some(0), "program should exit cleanly");
}

#[test]
fn list_methods_accept_a_named_function() {
    // `xs.filter(is_even)` — a top-level function passed where a lambda is
    // expected. Previously only literal lambdas were accepted and this emitted
    // a call to an undeclared C function.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-named-fn-methods");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("hof.maca");
    std::fs::write(
        &f,
        "is_even(n: int) -> bool => n % 2 == 0\n\
         double(n: int) -> int => n * 2\n\
         add(a: int, b: int) -> int => a + b\n\
         main() -> int {\n\
             info(\"filter={[1, 2, 3, 4].filter(is_even).length()}\")\n\
             info(\"map={[1, 2, 3].map(double).get(2)}\")\n\
             info(\"reduce={[1, 2, 3, 4].reduce(0, add)}\")\n\
             info(\"lambda={[1, 2, 3, 4].filter(n => n > 2).length()}\")\n\
             0\n\
         }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for want in ["filter=2", "map=6", "reduce=10", "lambda=2"] {
        assert!(
            stdout.contains(want),
            "missing {want:?}.\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn handbook_examples_all_run() {
    // `examples/handbook.maca` collects every runnable claim The Maca Handbook
    // makes. Writing the book found five real compiler bugs, so its examples
    // are executed here — documentation that isn't run is a claim, not a fact.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example_str("handbook.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for want in [
        "record: 3 4 -> 5", // ctor, field access, `with`
        "bindings: 1 100",  // mutable vs const
        "sides: 4",         // sum type + match
        "list: 10 3 60",    // first/length/reduce
        "named fn: 2",      // a named fn passed to .filter
        "inferred: 42 42",  // undeclared return types
        "sum_to: 55",       // `for` over an inclusive range
        "ternary: pass",
        "patterns: empty / one: 7 / head 1, then 2 more", // list patterns
        "propagate: 42",                                  // `?`
        "try: [division by zero] []",                     // `try` gives the message, not the value
        "strings: ababab 007 2",                          // repeat/pad_start/split
        "braces: {} {}",                                  // both literal-brace escapes
        "fmt: [3.14] [    42] [ok  ] [  ok  ] [007]",     // interpolation format specs
        // UI elements on the native target: attributes, children, and the
        // three forms an identifier can't express on its own
        "ui: <article class=\"prose\"><h1>Hi</h1><span>Body</span></article>",
        "attrs: <div data-kind=\"note\">seen</div>", // hyphenated name, false flag absent
        "flag: <details open><summary>more</summary>text</details>",
        "dyn: <h2 id=\"s\">Deep</h2>", // a tag chosen at run time
        "styles: true false",          // the sheet is tree-shaken
    ] {
        assert!(
            stdout.contains(want),
            "handbook claim broken — missing {want:?}.\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn concurrent_runs_of_the_same_program_dont_collide() {
    // Concurrent `maca run`s of the same source used to collide twice over.
    //
    // Warm cache: each process installs the cached binary at the same path, and
    // copying over a file another process is *executing* fails with ETXTBSY
    // ("Text file busy"). The cache now installs via a temp file + rename.
    //
    // Cold cache: each process runs `cc -o` into that same path, and one that
    // execs while another is still linking gets the same ETXTBSY. `run` now
    // names its throwaway binary per process. This is the case that actually
    // broke CI — the warm path was covered, the cold one wasn't, and CI starts
    // cold every time.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-concurrent-run");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("spin.maca");
    // a little work, so the runs genuinely overlap
    std::fs::write(
        &f,
        "sum_to(n: int) -> int {\n    t = 0\n    for i in 1..n {\n        t = t + i\n    }\n    t\n}\n\
         main() -> int {\n    info(\"{sum_to(200000)}\")\n    0\n}\n",
    )
    .unwrap();

    // Both races, in order: cold (no cache at all, every process compiles and
    // links), then warm (the cache is primed and every process installs).
    for cold in [true, false] {
        if !cold {
            let _ = Command::new(env!("CARGO_BIN_EXE_maca"))
                .args(["run", &f.to_string_lossy()])
                .output();
        }
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let path = f.to_string_lossy().to_string();
                std::thread::spawn(move || {
                    let mut c = Command::new(env!("CARGO_BIN_EXE_maca"));
                    c.args(["run", &path]);
                    if cold {
                        c.env("MACA_NO_CACHE", "1");
                    }
                    c.output().expect("spawn maca run")
                })
            })
            .collect();
        check_racers(handles, cold);
    }
}

fn check_racers(handles: Vec<std::thread::JoinHandle<std::process::Output>>, cold: bool) {
    let phase = if cold { "cold cache" } else { "warm cache" };
    for h in handles {
        let out = h.join().expect("thread panicked");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "a concurrent run failed ({phase}): {stderr}"
        );
        assert!(
            !stderr.contains("Text file busy") && !stderr.contains("cache copy failed"),
            "concurrent runs raced ({phase}): {stderr}"
        );
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("20000100000"),
            "wrong result from a concurrent run ({phase})"
        );
    }
}

/// Lambdas at every position: a top-level one is a function, a local one is a
/// closure, and both may carry types.
#[test]
fn lambdas_run_natively() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let program = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/lambdas.maca");
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["test", &program.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// A function kept in a record field — the route table, the reducer, the
/// builder. `(T, U) -> R` is the type that makes it writable; the assertions
/// are in Maca, in `tests/programs/function_fields.maca`.
#[test]
fn a_function_can_be_kept_in_a_record_field() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let program =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/function_fields.maca");
    for poison in ["0", "1"] {
        let out = Command::new(env!("CARGO_BIN_EXE_maca"))
            .args(["test", &program.to_string_lossy()])
            .env("MACA_POISON", poison)
            .output()
            .expect("spawn maca test");
        assert!(
            out.status.success(),
            "MACA_POISON={poison}:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// A generic function that names its own element type — a type variable bound
/// from inside a parameter's type, and a local declared with it. Assertions in
/// `tests/programs/generics.maca`.
#[test]
fn a_generic_can_name_its_own_element_type() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let program = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/generics.maca");
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["test", &program.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// `xs = xs.push(v)` is an append where nothing else holds the list, and a copy
/// everywhere else. Assertions in `tests/programs/accumulate.maca`, run plain
/// and poisoned — a buffer moved out from under a second holder is exactly what
/// a released-memory pattern would show.
#[test]
fn a_list_accumulates_without_copying_itself() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let program = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/accumulate.maca");
    for poison in ["0", "1"] {
        let out = Command::new(env!("CARGO_BIN_EXE_maca"))
            .args(["test", &program.to_string_lossy()])
            .env("MACA_POISON", poison)
            .output()
            .expect("spawn maca test");
        assert!(
            out.status.success(),
            "MACA_POISON={poison}:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

#[test]
fn native_backend_regressions_still_hold() {
    // List patterns, `chars()` registering its array type, `++` converting a
    // non-string operand, and anonymous record literals. Each one is a bug
    // that *compiled*, so each is asserted in Maca and run by `maca test`.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let program = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/regressions.maca");
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["test", &program.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
