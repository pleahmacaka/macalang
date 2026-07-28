//! `tools/lint.maca` — the Maca style linter, written in Maca.
//!
//! Like `tools/bindgen.maca`, this is compiler tooling ported into the language
//! it compiles. The gate checks the four rules actually fire, that a clean file
//! reports clean, that the exit code distinguishes the two — and that the tool
//! finds `tools/` itself clean, so the linter and the code it lints stay in
//! agreement.
//!
//! The single-line-`if` rule earns its own assertion. It silently matched
//! *nothing* for a while: the rule tested `line.contains("{")`, and a bare `{`
//! in a Maca string opens an interpolation, so the literal never held a brace.
//! The lexer now rejects that (see `crates/lexer/tests/lex.rs`); this test makes
//! sure the rule it broke stays fixed.

use std::path::PathBuf;
use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run the linter over `target`, returning (exit ok, stdout).
fn lint(target: &str) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "run",
            &repo().join("tools/lint.maca").to_string_lossy(),
            target,
        ])
        .output()
        .expect("spawn maca run tools/lint.maca");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr),
    )
}

#[test]
fn every_rule_fires_and_a_clean_file_is_clean() {
    if wsl() || !have("cc") {
        eprintln!("skipping lint port test: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-lint-port");
    let _ = std::fs::create_dir_all(&dir);

    // one file, one violation of each rule, in rule order
    let bad = dir.join("bad.maca");
    let wide = "w".repeat(90);
    std::fs::write(
        &bad,
        format!(
            "main() -> int {{\n\
             \x20   if x > 0 {{ x = 2 }}\n\
             \ty = 3\n\
             \x20   z = 4   \n\
             \x20   info(\"{wide}\")\n\
             \x20   0\n\
             }}\n"
        ),
    )
    .unwrap();

    let (ok, report) = lint(&bad.to_string_lossy());
    assert!(!ok, "a file with violations must exit non-zero:\n{report}");
    for (line, rule) in [
        (2, "single-line `if` block"),
        (3, "hard tab"),
        (4, "trailing whitespace"),
        (5, "line exceeds 80 columns"),
    ] {
        let want = format!("bad.maca:{line}: {rule}");
        assert!(
            report.contains(&want),
            "missing `{want}` in report:\n{report}"
        );
    }
    assert!(report.contains("4 issues"), "wrong count:\n{report}");

    // a file that breaks no rule
    let good = dir.join("good.maca");
    std::fs::write(&good, "main() -> int {\n    0\n}\n").unwrap();
    let (ok, report) = lint(&good.to_string_lossy());
    assert!(ok, "a clean file must exit zero:\n{report}");
    assert!(report.contains("clean"), "expected `clean`, got:\n{report}");
}

/// Ternaries and prose are exempt — a linter that cries wolf gets turned off.
#[test]
fn the_single_line_if_rule_does_not_fire_on_ternaries_or_comments() {
    if wsl() || !have("cc") {
        eprintln!("skipping lint exemption test: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-lint-exempt");
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join("exempt.maca");
    std::fs::write(
        &f,
        "// a comment mentioning `if x { y }` inline\n\
         pick(c: bool) -> int => c ? 1 : 2\n\
         has(s: str) -> bool => s.contains(\"if \")\n",
    )
    .unwrap();
    let (ok, report) = lint(&f.to_string_lossy());
    assert!(ok, "no rule should fire here:\n{report}");
}

/// Maca linting Maca: the tools directory must pass its own linter.
#[test]
fn the_tools_directory_passes_its_own_linter() {
    if wsl() || !have("cc") {
        eprintln!("skipping lint dogfood test: needs a host cc and no wsl");
        return;
    }
    let (ok, report) = lint(&repo().join("tools").to_string_lossy());
    assert!(ok, "tools/ does not pass tools/lint.maca:\n{report}");
}
