//! Tooling: fmt (idempotent, 4-space, block breaks), lint, and
//! `[scripts]` aliases. Pure — no WSL/toolchain needed.

use std::process::Command;

fn maca() -> &'static str {
    env!("CARGO_BIN_EXE_maca")
}
fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn fmt_is_idempotent_and_indented() {
    let src = std::fs::read_to_string(example("taskr.maca")).unwrap();
    let tmp = std::env::temp_dir().join("maca-fmt-test.maca");
    std::fs::write(&tmp, &src).unwrap();

    let run = || {
        Command::new(maca())
            .args(["fmt", &tmp.to_string_lossy()])
            .status()
            .unwrap();
        std::fs::read_to_string(&tmp).unwrap()
    };
    let once = run();
    let twice = run();
    assert_eq!(once, twice, "fmt must be idempotent");
    assert!(once.contains("\n    "), "expected 4-space indentation");
    // forced block breaks: no `{ ... }` statement block on one line
    assert!(
        once.lines().any(|l| l.trim_end().ends_with('{')),
        "block-opening braces should end a line (forced break)"
    );
}

#[test]
fn lint_flags_long_line() {
    // wide *code* — the rule is about what a reader has to scan
    let long = format!("total = {}\n", vec!["value"; 20].join(" + "));
    let (ok, err) = lint(&long, "wide");
    assert!(!ok, "lint should exit nonzero on a seeded issue: {err}");
    assert!(err.contains("80 columns"), "expected an 80-column diagnostic");
}

/// A long string literal is like a long comment — a C template, a URL, a test
/// program — and rewrapping it would change or disfigure it. So the line is
/// measured with its literals collapsed, which is what `tools/lint.maca` does;
/// the two linters have to agree or one of them is lying about the codebase.
#[test]
fn lint_does_not_flag_a_long_string_literal() {
    let long = format!("x = \"{}\"\n", "a".repeat(200));
    let (ok, err) = lint(&long, "literal");
    assert!(ok, "a long string literal is not a long line: {err}");
}

/// `} else if cond {` is one line of a guard chain, not a single-line block.
/// The rule used to fire on every one of them.
#[test]
fn lint_does_not_flag_an_else_if_chain() {
    let src = "pick(n: int) -> str {\n\
               \x20   if n > 1 {\n\
               \x20       \"many\"\n\
               \x20   } else if n == 1 {\n\
               \x20       \"one\"\n\
               \x20   } else {\n\
               \x20       \"none\"\n\
               \x20   }\n\
               }\n";
    let (ok, err) = lint(src, "chain");
    assert!(ok, "an else-if chain is the style, not a violation: {err}");
}

/// Lint a source string; returns whether it was clean, and what it complained
/// about.
fn lint(src: &str, name: &str) -> (bool, String) {
    let tmp = std::env::temp_dir().join(format!("maca-lint-{name}.maca"));
    std::fs::write(&tmp, src).unwrap();
    let out = Command::new(maca())
        .args(["lint", &tmp.to_string_lossy()])
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn script_alias_runs() {
    let dir = std::env::temp_dir().join("maca-script-test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("maca.toml"),
        "[scripts]\ngo = \"echo maca-script-ok\"\n",
    )
    .unwrap();
    let out = Command::new(maca())
        .current_dir(&dir)
        .arg("go")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("maca-script-ok"),
        "script alias should run its command; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// `maca dev` with no `dev.maca` prints one you can paste.
///
/// `maca init` does not scaffold the file, because a Nix dev shell is
/// optional — so "cannot read dev.maca" was the whole of the guidance, and
/// finding out what belongs in it meant leaving the terminal. The starter it
/// prints is compiled here, so it cannot drift from what the command accepts.
#[test]
fn dev_without_a_dev_maca_shows_a_working_starter() {
    let dir = std::env::temp_dir().join("maca-dev-starter");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");

    let out = Command::new(maca())
        .current_dir(&dir)
        .arg("dev")
        .output()
        .expect("spawn maca dev");
    let text = String::from_utf8_lossy(&out.stdout).to_string()
        + &String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "should not succeed:\n{text}");

    // Everything indented in the message is the starter file.
    let starter: String = text
        .lines()
        .filter(|l| l.starts_with("    "))
        .map(|l| format!("{}\n", l.trim_start()))
        .collect();
    assert!(
        starter.contains("dev.packages"),
        "no starter in the message:\n{text}"
    );

    std::fs::write(dir.join("dev.maca"), &starter).expect("write starter");
    let out = Command::new(maca())
        .current_dir(&dir)
        .arg("dev")
        .output()
        .expect("spawn maca dev");
    assert!(
        out.status.success(),
        "the printed starter does not compile:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(dir.join("flake.nix").exists(), "no flake.nix was written");
}
