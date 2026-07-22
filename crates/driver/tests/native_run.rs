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
