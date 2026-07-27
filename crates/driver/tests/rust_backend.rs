//! End-to-end test for `maca build --target rust`: emit Rust, compile it with
//! `rustc`, run the binary, and check its output / exit code. Skips if `rustc`
//! isn't on PATH (it always is in this workspace).

use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn build_and_run(name: &str, src: &str) -> (String, Option<i32>) {
    let dir = std::env::temp_dir().join(format!("maca-rust-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let mca = dir.join("prog.maca");
    std::fs::write(&mca, src).unwrap();
    let bin = dir.join("prog");
    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            "--target",
            "rust",
            &mca.to_string_lossy(),
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build --target rust");
    assert!(
        build.status.success(),
        "rust build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&bin).output().expect("run emitted binary");
    (
        String::from_utf8_lossy(&run.stdout).to_string(),
        run.status.code(),
    )
}

#[test]
fn hello_and_recursion_and_loops_run_via_rust() {
    if !have("rustc") {
        eprintln!("skipping: no rustc on PATH");
        return;
    }
    // hello: exit code is the returned int.
    let (out, code) = build_and_run(
        "hello",
        "main() -> int {\n    info(\"Hello, World\")\n    0\n}\n",
    );
    assert!(out.contains("Hello, World"), "hello stdout: {out}");
    assert_eq!(code, Some(0));

    // recursion + interpolation.
    let (out, _) = build_and_run(
        "fib",
        "fib(n: int) -> int =>\n    n < 2 ? n : fib(n - 1) + fib(n - 2)\n\nmain() -> int {\n    info(\"fib(10)={fib(10)}\")\n    0\n}\n",
    );
    assert!(out.contains("fib(10)=55"), "fib stdout: {out}");

    // records + nullary sum + match + while + for + list + reassignment.
    let (out, code) = build_and_run(
        "core",
        "Color = Red | Green | Blue\n\
         Point = {\n    x: int\n    y: int\n}\n\
         warmth(c: Color) -> int =>\n    match c {\n        Red => 2\n        Green => 1\n        Blue => 0\n    }\n\
         sum_to(n: int) -> int {\n    acc = 0\n    i = 1\n    while i <= n {\n        acc = acc + i\n        i = i + 1\n    }\n    acc\n}\n\
         main() -> int {\n    p = Point { x = 3, y = 4 }\n    info(\"warmth={warmth(Green)} sum={sum_to(5)} px={p.x}\")\n    sum_to(5)\n}\n",
    );
    assert!(out.contains("warmth=1 sum=15 px=3"), "core stdout: {out}");
    assert_eq!(code, Some(15), "main exit code should be sum_to(5)=15");
}
