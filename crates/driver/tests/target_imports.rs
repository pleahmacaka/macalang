//! Every build target resolves `import a/b`.
//!
//! `load_with_imports` ran on exactly two paths — rust and native. `build_nix`,
//! `build_jvm`, `build_embedded` and `build_js` each read one file and parsed
//! it, so an imported module's definitions never reached the output.
//!
//! The JS case was a silent success: the driver printed `built x-web/` and
//! exited 0, and the page threw `ReferenceError` in the browser, far from the
//! compiler that caused it. `modules/signal/dom.maca` documented the symptom in
//! prose and told readers to deliver from a native build instead.

mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch project directory, keyed by name so concurrent tests do not share.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maca-imports-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("lib")).unwrap();
    dir
}

fn write(dir: &Path, rel: &str, src: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, src).unwrap();
}

fn build(dir: &Path, target: &str, out: &str, extra: &[&str]) -> std::process::Output {
    Command::new(maca())
        .args(["build", "--target", target])
        .args(extra)
        .arg(dir.join("app.maca"))
        .arg("-o")
        .arg(dir.join(out))
        .output()
        .expect("spawn maca")
}

fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The two-file program every target below is given.
fn two_files(dir: &Path) {
    write(dir, "lib/greet.maca", "greeting(name: str) -> str => \"hello \" ++ name\n");
    write(
        dir,
        "app.maca",
        "import lib/greet\n\nhello() -> str => greeting(\"world\")\n\nmain() -> int => 0\n",
    );
}

#[test]
fn js_inlines_the_imported_module_and_the_result_runs() {
    if !have("node") {
        eprintln!("skipping: no node");
        return;
    }
    let dir = project("js");
    two_files(&dir);
    let o = build(&dir, "js", "out", &[]);
    assert!(
        o.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );

    let js = dir.join("out/app.js");
    let src = std::fs::read_to_string(&js).unwrap();
    assert!(
        src.contains("function greeting"),
        "imported definition missing:\n{src}"
    );

    // Reading the file is not enough — the definition could be emitted after
    // its use, or the module could fail to load at all.
    let run = Command::new("node")
        .arg("-e")
        .arg(format!(
            "console.log(require({:?}).hello())",
            js.to_string_lossy()
        ))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "hello world",
        "wrong answer from the emitted bundle"
    );
}

#[test]
fn jvm_inlines_the_imported_module() {
    let dir = project("jvm");
    two_files(&dir);
    let o = build(&dir, "jvm", "out", &[]);
    assert!(
        o.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let java = std::fs::read_to_string(dir.join("out/App.java")).unwrap();
    assert!(
        java.contains("String greeting("),
        "imported definition missing:\n{java}"
    );
}

#[test]
fn embedded_inlines_the_imported_module() {
    let dir = project("embedded");
    write(&dir, "lib/reg.maca", "mask(n: int) -> int => 1 << n\n");
    write(
        &dir,
        "app.maca",
        "import lib/reg\n\nmain() {\n    mmio_write(0x40020C14, mask(12))\n}\n",
    );
    let o = build(&dir, "embedded", "out", &["--mcu", "cortex-m4"]);
    assert!(
        o.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let c = std::fs::read_to_string(dir.join("out/firmware.c")).unwrap();
    assert!(c.contains("mask("), "imported definition missing:\n{c}");
}

#[test]
fn nix_reads_an_imported_config_fragment() {
    let dir = project("nix");
    write(&dir, "lib/ports.maca", "services.ssh.port = 2222\n");
    write(&dir, "app.maca", "import lib/ports\n\nservices.ssh.enable = true\n");
    let o = build(&dir, "nix", "out.nix", &[]);
    assert!(
        o.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let nix = std::fs::read_to_string(dir.join("out.nix")).unwrap();
    assert!(nix.contains("2222"), "imported setting missing:\n{nix}");
}

#[test]
fn a_transitive_import_is_inlined() {
    if !have("node") {
        eprintln!("skipping: no node");
        return;
    }
    let dir = project("transitive");
    write(&dir, "lib/c.maca", "base() -> int => 7\n");
    write(&dir, "lib/b.maca", "import lib/c\nmid() -> int => base() + 1\n");
    write(&dir, "lib/a.maca", "import lib/b\ntop() -> int => mid() + 1\n");
    write(
        &dir,
        "app.maca",
        "import lib/a\n\nanswer() -> int => top()\n\nmain() -> int => 0\n",
    );
    let o = build(&dir, "js", "out", &[]);
    assert!(
        o.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let run = Command::new("node")
        .arg("-e")
        .arg(format!(
            "console.log(require({:?}).answer())",
            dir.join("out/app.js").to_string_lossy()
        ))
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "9");
}

#[test]
fn a_diamond_inlines_the_shared_module_once() {
    if !have("node") {
        eprintln!("skipping: no node");
        return;
    }
    // Two copies is a redeclaration in JS and a duplicate symbol in C.
    let dir = project("diamond");
    write(&dir, "lib/base.maca", "base_val() -> int => 7\n");
    write(&dir, "lib/a.maca", "import lib/base\nfrom_a() -> int => base_val() + 1\n");
    write(&dir, "lib/b.maca", "import lib/base\nfrom_b() -> int => base_val() + 2\n");
    write(
        &dir,
        "app.maca",
        "import lib/a\nimport lib/b\n\ntotal() -> int => from_a() + from_b()\n\nmain() -> int => 0\n",
    );
    let o = build(&dir, "js", "out", &[]);
    assert!(
        o.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let src = std::fs::read_to_string(dir.join("out/app.js")).unwrap();
    assert_eq!(
        src.matches("function base_val").count(),
        1,
        "shared module inlined more than once:\n{src}"
    );
    let run = Command::new("node")
        .arg("-e")
        .arg(format!(
            "console.log(require({:?}).total())",
            dir.join("out/app.js").to_string_lossy()
        ))
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "17");
}

#[test]
fn a_selective_import_brings_only_what_it_names() {
    let dir = project("selective");
    write(
        &dir,
        "lib/many.maca",
        "wanted() -> int => 1\n\nunwanted() -> int => 2\n",
    );
    write(
        &dir,
        "app.maca",
        "import { wanted } from lib/many\n\nuse_it() -> int => wanted()\n\nmain() -> int => 0\n",
    );
    let o = build(&dir, "js", "out", &[]);
    assert!(
        o.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let src = std::fs::read_to_string(dir.join("out/app.js")).unwrap();
    assert!(src.contains("function wanted"), "{src}");
    assert!(
        !src.contains("function unwanted"),
        "brought a definition it did not name:\n{src}"
    );
}

#[test]
fn an_import_that_names_no_file_is_an_error_on_every_target() {
    for (target, extra) in [
        ("js", &[][..]),
        ("jvm", &[]),
        ("nix", &[]),
        ("embedded", &["--mcu", "cortex-m4"]),
    ] {
        let dir = project(&format!("missing-{target}"));
        write(
            &dir,
            "app.maca",
            "import lib/nope\n\nmain() -> int => 0\n",
        );
        let o = build(&dir, target, "out", extra);
        assert!(
            !o.status.success(),
            "{target} accepted an import that resolves to no file"
        );
    }
}

#[test]
fn an_import_cycle_terminates_rather_than_hanging() {
    let dir = project("cycle");
    write(&dir, "lib/x.maca", "import lib/y\nfrom_x() -> int => 1\n");
    write(&dir, "lib/y.maca", "import lib/x\nfrom_y() -> int => 2\n");
    write(&dir, "app.maca", "import lib/x\n\nmain() -> int => 0\n");
    // The assertion is that this returns at all; a cycle used to be reachable
    // only on two of the seven targets, so it was never exercised on the rest.
    let o = build(&dir, "js", "out", &[]);
    let _ = o.status;
}

#[test]
fn a_program_with_no_imports_still_builds_on_every_target() {
    for (target, extra, src) in [
        ("js", &[][..], "main() -> int => 0\n"),
        ("jvm", &[], "main() -> int => 0\n"),
        ("nix", &[], "services.ssh.enable = true\n"),
        (
            "embedded",
            &["--mcu", "cortex-m4"],
            "main() {\n    mmio_write(0x40020C14, 1)\n}\n",
        ),
    ] {
        let dir = project(&format!("plain-{target}"));
        write(&dir, "app.maca", src);
        let o = build(&dir, target, "out", extra);
        assert!(
            o.status.success(),
            "{target} regressed on a program with no imports: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
}
