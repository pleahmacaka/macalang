//! The JS target runs the type checker, like every other target does.

mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

fn built(program: &str, name: &str) -> (bool, String, PathBuf) {
    let dir = std::env::temp_dir().join(format!("maca-jschecked-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("main.maca");
    std::fs::write(&src, program).unwrap();
    let out = dir.join("site");

    let o = Command::new(maca())
        .args(["build", "--target", "js"])
        .arg(&src)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("spawn maca build");
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);

    (o.status.success(), text, out)
}

/// A build that is refused must not also leave a page behind, or the next step
/// in a pipeline reads a stale one and calls it success.
fn refused(program: &str, name: &str, wanted: &str) {
    let (ok, text, out) = built(program, name);

    assert!(!ok, "the build succeeded:\n{text}");
    assert!(text.contains(wanted), "wanted `{wanted}` in:\n{text}");
    assert!(
        !out.join("index.html").exists(),
        "a refused build still wrote index.html"
    );
}

#[test]
fn a_call_to_a_name_nothing_defines_is_refused() {
    refused(
        "main() -> int {\n  nowhere()\n  0\n}\n",
        "undefined",
        "UndefinedName",
    );
}

#[test]
fn a_type_mismatch_is_refused() {
    refused(
        "take(n: int) -> int => n\n\nmain() -> int => take(\"text\")\n",
        "mismatch",
        "TypeMismatch",
    );
}

#[test]
fn a_match_that_misses_a_variant_is_refused() {
    refused(
        "Colour = Red | Green\n\nname(c: Colour) -> str =>\n  match c {\n    Red => \"red\"\n  }\n\nmain() -> int {\n  info(name(Red))\n  0\n}\n",
        "nonexhaustive",
        "NonExhaustive",
    );
}

#[test]
fn writing_a_constant_is_refused() {
    refused(
        "main() -> int {\n  const n = 1\n  n = 2\n  0\n}\n",
        "immutable",
        "Immutable",
    );
}

/// The checker running is not worth much if it also rejects what the target
/// legitimately builds, so a real page has to keep compiling.
#[test]
fn a_program_the_checker_accepts_still_builds() {
    let (ok, text, out) = built(
        "greet(who: str) -> str => \"hello {who}\"\n\nmain() -> Element =>\n  div(class=\"p-4\", greet(\"world\"))\n",
        "accepted",
    );

    assert!(ok, "{text}");
    assert!(out.join("index.html").exists(), "no page was written");
}

/// `--target tauri` reaches the same emitter through `build_js`, so it must not
/// be a way around the checker.
#[test]
fn the_tauri_target_is_checked_too() {
    let dir = std::env::temp_dir().join(format!("maca-jschecked-tauri-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("main.maca");
    std::fs::write(&src, "main() -> int {\n  nowhere()\n  0\n}\n").unwrap();

    let o = Command::new(maca())
        .args(["build", "--target", "tauri"])
        .arg(&src)
        .arg("-o")
        .arg(dir.join("app"))
        .output()
        .expect("spawn maca build");
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);

    assert!(!o.status.success(), "the build succeeded:\n{text}");
    assert!(text.contains("UndefinedName"), "{text}");
}
