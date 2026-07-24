//! JVM target: `maca build --target jvm` emits Java that compiles and runs.
//! Skips when there's no JDK (`javac`/`java`).

use std::path::PathBuf;
use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn maca() -> &'static str {
    env!("CARGO_BIN_EXE_maca")
}

#[test]
fn program_runs_on_jvm() {
    if !have("javac") || !have("java") {
        eprintln!("skipping: no JDK");
        return;
    }
    let dir = std::env::temp_dir().join("maca-jvm-test");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("fibj.maca");
    std::fs::write(&f, "fib(n: int) -> int =>\n    n < 2 ? n : fib(n - 1) + fib(n - 2)\n\nmain() -> int {\n    info(\"{fib(20)}\")\n    0\n}\n").unwrap();
    let out = dir.join("out");

    let build = Command::new(maca())
        .args([
            "build",
            "--target",
            "jvm",
            &f.to_string_lossy(),
            "-o",
            &out.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca");
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new("java")
        .args(["-cp", &out.to_string_lossy(), "Fibj"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("6765"),
        "fib(20) on JVM should be 6765, got: {stdout}"
    );
}

#[test]
fn fabric_mod_implements_modinitializer() {
    if !have("javac") || !have("java") {
        eprintln!("skipping: no JDK");
        return;
    }
    let dir = std::env::temp_dir().join("maca-fabric-test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("stub/net/fabricmc/api")).unwrap();
    // a stub of the Fabric interface, so javac can resolve it
    std::fs::write(
        dir.join("stub/net/fabricmc/api/ModInitializer.java"),
        "package net.fabricmc.api;\npublic interface ModInitializer { void onInitialize(); }\n",
    )
    .unwrap();
    let stub = dir.join("stub");
    assert!(
        Command::new("javac")
            .arg(dir.join("stub/net/fabricmc/api/ModInitializer.java"))
            .arg("-d")
            .arg(&stub)
            .status()
            .unwrap()
            .success()
    );

    // the mod source (mirrors apps/mcmod/mod.maca)
    let modsrc = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/mcmod/mod.maca");
    let out = dir.join("out");
    let build = Command::new(maca())
        .args([
            "build",
            "--target",
            "jvm",
            &modsrc.to_string_lossy(),
            "-o",
            &out.to_string_lossy(),
            "--cp",
            &stub.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca");
    assert!(
        build.status.success(),
        "build: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(out.join("Mod.class").exists(), "Mod.class not produced");

    // invoke onInitialize through the ModInitializer interface
    std::fs::write(
        out.join("Runner.java"),
        "public class Runner { public static void main(String[] a) {\n  net.fabricmc.api.ModInitializer m = new Mod.ExampleMod();\n  m.onInitialize();\n} }\n",
    )
    .unwrap();
    let cp = format!("{}:{}", stub.display(), out.display());
    assert!(
        Command::new("javac")
            .args(["-cp", &cp, "-d", &out.to_string_lossy()])
            .arg(out.join("Runner.java"))
            .status()
            .unwrap()
            .success()
    );
    let run = Command::new("java")
        .args(["-cp", &cp, "Runner"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("Maca-authored Fabric mod"),
        "onInitialize output: {stdout}"
    );
}
