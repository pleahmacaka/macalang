//! `maca fmt` must be safe: idempotent, comment-preserving, no reflow. It
//! normalizes indentation only, so the golden examples (already 4-space) are
//! left byte-for-byte unchanged.

use std::path::PathBuf;
use std::process::Command;

fn maca() -> &'static str {
    env!("CARGO_BIN_EXE_maca")
}

fn examples() -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    ["hello", "taskr", "generic", "system", "counter", "dot"]
        .iter()
        .map(|n| base.join(format!("{n}.maca")))
        .collect()
}

#[test]
fn examples_are_already_formatted() {
    // Default style is 4-space; the examples are 4-space, so --check is clean.
    let mut args = vec!["fmt".to_string(), "--check".to_string()];
    for p in examples() {
        args.push(p.to_string_lossy().into_owned());
    }
    let out = Command::new(maca()).args(&args).output().expect("spawn maca");
    assert!(
        out.status.success(),
        "fmt --check should pass on the golden examples\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn fmt_preserves_comments_and_is_idempotent() {
    let dir = std::env::temp_dir().join("maca-fmt-test");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("c.maca");
    let src = "// a leading comment\nfoo(x: int) -> int =>\n    x > 0\n        ? x   // inline note\n        : 0\n";
    std::fs::write(&f, src).unwrap();

    // format twice
    for _ in 0..2 {
        let out = Command::new(maca()).args(["fmt", &f.to_string_lossy()]).output().unwrap();
        assert!(out.status.success(), "fmt failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let after = std::fs::read_to_string(&f).unwrap();
    assert!(after.contains("// a leading comment"), "leading comment dropped:\n{after}");
    assert!(after.contains("// inline note"), "inline comment dropped:\n{after}");
    // the ternary continuation indentation is preserved (not flattened)
    assert!(after.contains("        ? x"), "continuation indent lost:\n{after}");
    // idempotent: a --check now passes
    let chk = Command::new(maca()).args(["fmt", "--check", &f.to_string_lossy()]).output().unwrap();
    assert!(chk.status.success(), "fmt not idempotent");
}
