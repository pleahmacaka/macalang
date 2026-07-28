//! Native-toolchain smoke test: with no WSL but a host `cc`, `maca run` must
//! compile and execute a program end to end. Skips if neither is usable.

use std::path::PathBuf;
use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

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
        .args(["run", &example("hello.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Hello, World"),
        "expected greeting.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
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
        .args(["run", &example("recursive_record.maca")])
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
        .args(["run", &example("operators.maca")])
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
        .args(["run", &example("ffi_sqlite.maca")])
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
        .args(["run", &example("math.maca")])
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
        .args(["run", &example("collections.maca")])
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
        .args(["run", &example("async.maca")])
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
        .args(["run", &example("strings.maca")])
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
        .args(["run", &example("match_guard.maca")])
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
        .args(["run", &example("or_patterns.maca")])
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
        .args(["run", &example("payload_sum.maca")])
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
        .args(["run", &example("catch.maca")])
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
        .args(["run", &example("lambda.maca")])
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
        .args(["run", &example("generic_record.maca")])
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
        .args(["run", &example("record_pattern.maca")])
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
        .args(["run", &example("keywords.maca")])
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
        .args(["run", &example("indexing.maca")])
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
        .args(["run", &example("record_update.maca")])
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
        .args(["run", &example("tree.maca")])
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
        .args(["run", &example("sum_record.maca")])
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
        .args(["run", &example("fileio.maca")])
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
    let dir = std::env::temp_dir().join("maca-test-cmd");
    std::fs::create_dir_all(&dir).unwrap();

    // passing: two tests, plus a `main` that must be replaced rather than run
    let pass = dir.join("pass.maca");
    std::fs::write(
        &pass,
        "add(a: int, b: int) -> int => a + b\n\
         main() -> int {\n    info(\"main must not run\")\n    99\n}\n\
         test_commutes() -> int => add(2, 3) == add(3, 2) ? 0 : fail \"should commute\"\n\
         test_identity() -> int => add(7, 0) == 7 ? 0 : fail \"identity\"\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["test", &pass.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "passing tests should exit 0: {stdout}"
    );
    assert!(stdout.contains("running 2 tests"), "count wrong: {stdout}");
    assert!(stdout.contains("2 tests passed"), "summary wrong: {stdout}");
    assert!(
        !stdout.contains("main must not run"),
        "the file's own main was executed: {stdout}"
    );

    // failing: a non-zero exit, and the failing test is the last one announced
    let fail = dir.join("fail.maca");
    std::fs::write(
        &fail,
        "test_ok() -> int => 0\n\
         test_broken() -> int => fail \"deliberate\"\n\
         test_after() -> int => 0\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["test", &fail.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a failing test must exit non-zero");
    assert!(
        stdout.contains("test_broken") && !stdout.contains("test_after"),
        "output should stop at the failing test: {stdout}"
    );
    assert!(
        stderr.contains("deliberate"),
        "failure message lost: {stderr}"
    );

    // a file with no tests is not an error
    let none = dir.join("none.maca");
    std::fs::write(&none, "main() -> int => 0\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["test", &none.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    assert!(out.status.success(), "no tests should not be a failure");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("no tests found"),
        "should say there are no tests"
    );
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
fn bracketed_list_patterns_match() {
    // `[]`, `[x]`, `[x, y]`, `[x, ..rest]` — brackets alongside the bracketless
    // spelling. Before this, an empty list had no pattern at all.
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-list-patterns");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("lp.maca");
    std::fs::write(
        &f,
        "kind(xs: int[]) -> int =>\n\
         \x20   match xs {\n\
         \x20       []     => 0\n\
         \x20       [x]    => 1\n\
         \x20       [x, y] => 2\n\
         \x20       _      => 9\n\
         \x20   }\n\
         tail_len(xs: int[]) -> int =>\n\
         \x20   match xs {\n\
         \x20       [x, ..rest] => rest.length()\n\
         \x20       _           => 0\n\
         \x20   }\n\
         bare(xs: int[]) -> int =>\n\
         \x20   match xs {\n\
         \x20       x, ..rest => rest.length()\n\
         \x20       _         => 0\n\
         \x20   }\n\
         main() -> int {\n\
         \x20   info(\"kinds={kind([])} {kind([5])} {kind([5, 6])} {kind([1, 2, 3])}\")\n\
         \x20   info(\"tail={tail_len([1, 2, 3])}\")\n\
         \x20   info(\"bare={bare([1, 2, 3])}\")\n\
         \x20   0\n\
         }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for want in ["kinds=0 1 2 9", "tail=2", "bare=2"] {
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
        .args(["run", &example("handbook.maca")])
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
        "propagate: 42",
        "try: [division by zero] []", // `try` gives the message, not the value                                  // `?`
        // `try`
        "strings: ababab 007 2", // repeat/pad_start/split
        "braces: {} {}",         // both literal-brace escapes
        "fmt: [3.14] [    42] [ok  ] [  ok  ] [007]", // interpolation format specs
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

/// `s.chars()` on its own, not passed to a `str[]` parameter.
///
/// The C backend registers array types in a prepass, and `chars()` was not one
/// of the expressions that registered `StrArr`. It compiled only when the
/// result went straight into a parameter declared `str[]`, which registered the
/// type as a side effect; standalone it emitted a reference to an undeclared
/// `StrArr` and failed in the C compiler. Found while chasing a Korean-heading
/// bug in the handbook generator.
#[test]
fn chars_registers_its_array_type() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-chars-prepass");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("chars.maca");
    std::fs::write(
        &f,
        "main() -> int {\n\
        \x20   n = \"abc\".chars().length()\n\
        \x20   cs = \"hello\".chars()\n\
        \x20   info(\"{n} {cs.length()} {cs.get(1)} {\"xy\".chars().length()}\")\n\
        \x20   0\n\
        }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("3 5 e 2"),
        "chars() prepass regression.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `++` converts a non-string operand instead of passing it through.
///
/// `"h" ++ level` used to emit `maca_concat("h", level)` — an `int64_t` handed
/// to a `maca_str` parameter. C accepts it with a warning nobody reads, and the
/// program segfaults dereferencing address 3. It compiled, so it looked like a
/// language feature; it just crashed.
#[test]
fn concat_converts_a_non_string_operand() {
    let wsl = Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-concat-coerce");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("concat.maca");
    std::fs::write(
        &f,
        "main() -> int {\n\
        \x20   level = 3\n\
        \x20   info(\"h\" ++ level)\n\
        \x20   info(\"n=\" ++ 4 ++ \" f=\" ++ 1.5 ++ \" b=\" ++ true)\n\
        \x20   info(7 ++ \" trailing\")\n\
        \x20   0\n\
        }\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &f.to_string_lossy()])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("h3")
            && stdout.contains("n=4 f=1.5 b=true")
            && stdout.contains("7 trailing"),
        "`++` mis-lowered a non-string operand.\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
