mod common;
use common::*;

use std::process::Command;

/// Run the linter over `target`, returning (exit ok, stdout).
fn lint(target: &str) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "run",
            &repo().join("apps/lint/lint.maca").to_string_lossy(),
            target,
        ])
        .output()
        .expect("spawn maca run apps/lint/lint.maca");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr),
    )
}

#[test]
fn every_rule_fires_and_a_clean_file_is_clean() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping lint port test: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-lint-port");
    let _ = std::fs::create_dir_all(&dir);

    let bad = dir.join("bad.maca");
    let wide = "+ 1 ".repeat(25) + "+ 1";
    std::fs::write(
        &bad,
        format!(
            "main() -> int {{\n\
             \x20   if x > 0 {{ x = 2 }}\n\
             \ty = 3\n\
             \x20   z = 4   \n\
             \x20   w = 0 {wide}\n\
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

    let good = dir.join("good.maca");
    std::fs::write(&good, "main() -> int {\n    0\n}\n").unwrap();
    let (ok, report) = lint(&good.to_string_lossy());
    assert!(ok, "a clean file must exit zero:\n{report}");
    assert!(report.contains("clean"), "expected `clean`, got:\n{report}");
}

/// Ternaries and prose are exempt.
#[test]
fn the_single_line_if_rule_does_not_fire_on_ternaries_or_comments() {
    if have_wsl() || !have("cc") {
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

/// Maca linting Maca.
#[test]
fn the_repository_passes_its_own_linter() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping lint dogfood test: needs a host cc and no wsl");
        return;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &repo().join("apps/lint/lint.maca").to_string_lossy()])
        .current_dir(repo())
        .output()
        .expect("spawn maca run apps/lint/lint.maca");
    assert!(
        out.status.success(),
        "the repository does not pass apps/lint/lint.maca:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// The width rule measures code, not text.
#[test]
fn a_long_string_literal_is_not_a_long_line() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping lint width test: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-lint-width");
    let _ = std::fs::create_dir_all(&dir);
    let f = dir.join("wide.maca");
    let long = "x".repeat(150);
    std::fs::write(
        &f,
        format!("main() -> int {{\n    info(\"{long}\")\n    0\n}}\n"),
    )
    .unwrap();
    let (ok, report) = lint(&f.to_string_lossy());
    assert!(ok, "a long string literal should be exempt:\n{report}");

    let g = dir.join("wide2.maca");
    let pad = " ".repeat(70);
    std::fs::write(
        &g,
        format!("main() -> int {{\n    a = 1{pad}+ 2 + 3 + 4 + \"s\"\n    0\n}}\n"),
    )
    .unwrap();
    let (ok, report) = lint(&g.to_string_lossy());
    assert!(!ok, "wide code should still be flagged:\n{report}");
}
