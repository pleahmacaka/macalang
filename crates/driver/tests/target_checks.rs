mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

const PROGRAM_TARGETS: [&str; 5] = ["c", "js", "jvm", "rust", "embedded"];

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maca-checks-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(dir: &std::path::Path, target: &str, src: &str) -> String {
    std::fs::write(dir.join("app.maca"), src).unwrap();
    let out = Command::new(maca())
        .args(["build", "--target", target])
        .arg(dir.join("app.maca"))
        .arg("-o")
        .arg(dir.join(format!("out-{target}")))
        .output()
        .expect("spawn maca");
    assert!(
        !out.status.success(),
        "{target} accepted a program the checker rejects"
    );
    String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr)
}

/// Each target runs the checker from its own call site, so a target that stops asking is a silent wrong answer on that target alone.
#[test]
fn every_program_target_refuses_the_same_type_error() {
    let dir = scratch("mismatch");
    for target in PROGRAM_TARGETS {
        let said = build(
            &dir,
            target,
            "add(a: int, b: int) -> int => a + b\n\n\
             main() -> int {\n    add(\"not an int\", 2)\n}\n",
        );
        assert!(
            said.contains("TypeMismatch") && said.contains("expected int, found str"),
            "{target} failed without naming the mismatch:\n{said}"
        );
    }
}

/// The same for the diagnostics a back end could otherwise emit code for: a match that misses a variant, and a write to a constant.
#[test]
fn every_program_target_refuses_a_non_exhaustive_match_and_a_written_constant() {
    let dir = scratch("kinds");
    for target in PROGRAM_TARGETS {
        let said = build(
            &dir,
            target,
            "Color = Red | Green | Blue\n\n\
             name(c: Color) -> str => match c {\n    Red => \"red\"\n    Green => \"green\"\n}\n\n\
             main() -> int {\n    info(name(Red))\n    0\n}\n",
        );
        assert!(
            said.contains("NonExhaustive"),
            "{target} failed without naming the missing variant:\n{said}"
        );

        let said = build(
            &dir,
            target,
            "main() -> int {\n    const limit = 3\n    limit = 4\n    limit\n}\n",
        );
        assert!(
            said.contains("Immutable"),
            "{target} failed without saying the name is a constant:\n{said}"
        );
    }
}
