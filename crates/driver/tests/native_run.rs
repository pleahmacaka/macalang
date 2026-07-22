//! Native-toolchain smoke test: with no WSL but a host `cc`, `maca run` must
//! compile and execute a program end to end. Skips if neither is usable.

use std::path::PathBuf;
use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn hello_runs_natively() {
    // Only meaningful on a plain Linux host (no WSL) with a C compiler.
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
    assert!(stdout.contains("6765"), "fib(20) should be 6765, got: {stdout}");
}

#[test]
fn operator_overloading_runs_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
fn fail_exits_cleanly_with_message() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
    assert_eq!(out.status.code(), Some(1), "fail should exit 1, got {:?}", out.status.code());
}

#[test]
fn match_guards_run_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
    assert_eq!(lines, vec!["negative", "zero", "small", "big", "two"], "stdout: {stdout}");
}

#[test]
fn string_match_runs_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-strmatch-test");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("m.maca");
    std::fs::write(&f, "route(c: str) -> int {\n    match c {\n        \"add\" => 1\n        \"del\" => 2\n        _ => 0\n    }\n}\n\nmain() -> int {\n    let a = route(\"add\")\n    let b = route(\"del\")\n    let z = route(\"x\")\n    info(\"{a} {b} {z}\")\n    0\n}\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca")).args(["run", &f.to_string_lossy()]).output().expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1 2 0"), "stdout: {stdout}");
}

#[test]
fn or_patterns_run_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example("or_patterns.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["warm", "cold", "low", "high"], "stdout: {stdout}");
}

#[test]
fn payload_sums_run_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example("payload_sum.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["100", "12"], "stdout: {stdout}");
}

#[test]
fn try_catches_failure_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
    assert_eq!(out.status.code(), Some(0), "try should catch and exit 0, got {:?}", out.status.code());
    assert!(String::from_utf8_lossy(&out.stderr).is_empty(), "no error should be printed");
}

#[test]
fn lambda_runs_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example("lambda.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["1", "4", "9", "16"], "stdout: {stdout}");
}

#[test]
fn generics_monomorphize_and_run_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["hello", "42", "7"], "stdout: {stdout}");
}

#[test]
fn record_patterns_and_ops_run_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
    if wsl || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &example("record_pattern.maca")])
        .output()
        .expect("spawn maca");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["4", "true", "side: right"], "stdout: {stdout}");
}

#[test]
fn indexing_runs_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
        vec!["10 30", "m", "99", "42 2"],
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn record_update_runs_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
        vec!["localhost:80 tls=false", "localhost:443 tls=true", "localhost:8080 tls=false"],
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn sum_with_record_payload_runs_natively() {
    let wsl = Command::new("wsl").arg("true").output().map(|o| o.status.success()).unwrap_or(false);
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
