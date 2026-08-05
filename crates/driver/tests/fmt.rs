mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

fn examples() -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/examples");
    ["hello", "taskr", "generic", "system", "counter", "dot"]
        .iter()
        .map(|n| base.join(format!("{n}.maca")))
        .collect()
}

#[test]
fn examples_are_already_formatted() {
    let mut args = vec!["fmt".to_string(), "--check".to_string()];
    for p in examples() {
        args.push(p.to_string_lossy().into_owned());
    }
    let out = Command::new(maca())
        .args(&args)
        .output()
        .expect("spawn maca");
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

    for _ in 0..2 {
        let out = Command::new(maca())
            .args(["fmt", &f.to_string_lossy()])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "fmt failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let after = std::fs::read_to_string(&f).unwrap();
    assert!(
        after.contains("// a leading comment"),
        "leading comment dropped:\n{after}"
    );
    assert!(
        after.contains("// inline note"),
        "inline comment dropped:\n{after}"
    );
    assert!(
        after.contains("        ? x"),
        "continuation indent lost:\n{after}"
    );
    let chk = Command::new(maca())
        .args(["fmt", "--check", &f.to_string_lossy()])
        .output()
        .unwrap();
    assert!(chk.status.success(), "fmt not idempotent");
}

/// `fmt` used to infer the file's indent step as the gcd of every leading width, which one continuation line aligned under an open paren destroys.
#[test]
fn an_aligned_continuation_does_not_double_the_indent() {
    let src = "f(t: int) -> str =>\n\
               \x20   pair([\"id\", \"name\"],\n\
               \x20         [str(t), \"x\"])\n\
               \n\
               g() -> int {\n\
               \x20   n = 1\n\
               \x20   n + 1\n\
               }\n";
    assert_eq!(
        formatted(src, "indent-gcd"),
        src,
        "fmt changed a formatted file"
    );
}

/// And it must not re-indent the aligned line itself.
#[test]
fn alignment_survives_formatting() {
    let src = "f() -> str =>\n\
               \x20   pair(one,\n\
               \x20        two)\n";
    assert_eq!(formatted(src, "align"), src, "the aligned argument moved");
}

/// A raw `"""…"""` block holds foreign source with its own indentation.
#[test]
fn a_raw_block_is_neither_measured_nor_reindented() {
    let src = "Css = \"\"\"\n\
               body {\n\
               \x20 color: red;\n\
               }\n\
               \"\"\"\n\
               \n\
               go() -> int {\n\
               \x20   n = 1\n\
               \x20   n\n\
               }\n";
    let out = formatted(src, "raw");
    assert_eq!(out, src, "a raw block changed the file's indentation");
    assert!(out.contains("\n  color: red;"), "the CSS was re-indented");
}

/// Format `src` and hand back the result.
fn formatted(src: &str, name: &str) -> String {
    let tmp = std::env::temp_dir().join(format!("maca-fmt-{name}.maca"));
    std::fs::write(&tmp, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["fmt", &tmp.to_string_lossy()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "fmt failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(&tmp).unwrap()
}
