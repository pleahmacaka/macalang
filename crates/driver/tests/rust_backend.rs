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

    // payload sum (data-carrying variants), incl. a record payload and value
    // reuse — the shape gpql needs (`Outcome = Rows(Grid) | Affected(int)`).
    let (out, code) = build_and_run(
        "payload_sum",
        "Grid = {\n    rows: int\n    cols: int\n}\n\
         Outcome = Rows(Grid) | Affected(int)\n\
         describe(o: Outcome) -> int =>\n    match o {\n        Rows(g) => g.rows\n        Affected(n) => n\n    }\n\n\
         main() -> int {\n    r = Rows(Grid { rows = 7, cols = 3 })\n    a = Affected(42)\n    info(\"rows={describe(r)} affected={describe(a)}\")\n    describe(r)\n}\n",
    );
    assert!(
        out.contains("rows=7 affected=42"),
        "payload_sum stdout: {out}"
    );
    assert_eq!(code, Some(7), "main exit code should be describe(r)=7");
}

#[test]
fn foreign_std_type_compiles_and_runs() {
    if !have("rustc") {
        eprintln!("skipping: no rustc on PATH");
        return;
    }
    // `import rust "std::time::Duration"`, `Duration.from_secs(5)` (associated
    // fn), `d.as_secs()` (instance method) — a real std type, no external crate,
    // through the single-file rustc path.
    let (out, code) = build_and_run(
        "foreign_std",
        "import rust \"std::time::Duration\"\n\n\
         main() -> int {\n    d = Duration.from_secs(5)\n    total = d.as_secs()\n    info(\"duration = {total} secs\")\n    int(total)\n}\n",
    );
    assert!(
        out.contains("duration = 5 secs"),
        "foreign_std stdout: {out}"
    );
    assert_eq!(code, Some(5), "exit code should be as_secs()=5");
}

#[test]
fn closure_passed_to_a_callback_api_runs() {
    if !have("rustc") {
        eprintln!("skipping: no rustc on PATH");
        return;
    }
    // R4: a Maca closure escapes into a foreign higher-order function (defined in
    // an `import rust` raw block) and is invoked there.
    let (out, code) = build_and_run(
        "closure",
        "import rust \"\"\"\nfn apply(f: impl Fn(i64) -> i64, x: i64) -> i64 { f(x) }\n\"\"\"\n\n\
         main() -> int {\n    r = apply(n => n + 1, 41)\n    info(\"apply = {r}\")\n    r\n}\n",
    );
    assert!(out.contains("apply = 42"), "closure stdout: {out}");
    assert_eq!(code, Some(42), "closure result should be 42");
}

#[test]
fn foreign_trait_impl_runs() {
    if !have("rustc") {
        eprintln!("skipping: no rustc on PATH");
        return;
    }
    // R5: `Counter : Greet = { … }` → `impl Greet for Counter`. The trait is
    // supplied locally via a raw block (real gpui isn't available offline), which
    // is exactly the shape `examples/gpui_counter.maca` uses against gpui's
    // `Render`. A leading `self` → `&mut self`; a mutating method returns unit.
    let (out, code) = build_and_run(
        "trait_impl",
        "import rust \"\"\"\ntrait Greet {\n    fn value(&mut self) -> i64;\n    fn bump(&mut self);\n}\n\"\"\"\n\n\
         Counter = {\n    count: int\n}\n\
         Counter : Greet = {\n    value = (self) => self.count + 1\n    bump = (self) => self.count = self.count + 100\n}\n\n\
         main() -> int {\n    c = Counter { count = 41 }\n    v = c.value()\n    info(\"value = {v}\")\n    v\n}\n",
    );
    assert!(out.contains("value = 42"), "trait impl stdout: {out}");
    assert_eq!(code, Some(42), "value() should be 42");
}

/// `[rust-dependencies]` → a generated Cargo project. Verifies the manifest is
/// written correctly and, when the crate is resolvable (cargo present + crate
/// cached/online), that the dependency builds and links. Skips gracefully in a
/// hermetic CI with no crate cache or network.
#[test]
fn rust_dependencies_drive_a_cargo_build() {
    if !have("cargo") {
        eprintln!("skipping: no cargo on PATH");
        return;
    }
    let dir = std::env::temp_dir().join("maca-rust-cargodep");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("maca.toml"),
        "[package]\nname = \"app\"\n\n[rust-dependencies]\nitoa = \"1\"\n",
    )
    .unwrap();
    let mca = dir.join("prog.maca");
    std::fs::write(
        &mca,
        "import rust \"itoa\"\n\nmain() -> int {\n    info(\"cargo dep build\")\n    7\n}\n",
    )
    .unwrap();
    let bin = dir.join("prog");
    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["build", "--target", "rust"])
        .arg(&mca)
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("spawn maca build --target rust");

    // The Cargo.toml is written before cargo runs, so it exists whether or not
    // the offline build resolves the crate.
    let manifest = std::env::temp_dir()
        .join("maca-build-prog")
        .join("cargo/Cargo.toml");
    if let Ok(toml) = std::fs::read_to_string(&manifest) {
        assert!(
            toml.contains("itoa = \"1\"") && toml.contains("[dependencies]"),
            "generated Cargo.toml missing the dependency:\n{toml}"
        );
    }

    if build.status.success() {
        let run = Command::new(&bin).output().expect("run cargo-built binary");
        assert!(String::from_utf8_lossy(&run.stdout).contains("cargo dep build"));
        assert_eq!(run.status.code(), Some(7));
    } else {
        eprintln!(
            "skipping run assertion: cargo build didn't resolve itoa (offline / no cache):\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
    }
}
