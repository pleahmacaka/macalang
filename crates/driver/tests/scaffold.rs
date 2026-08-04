mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

fn scaffold(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maca-init-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = Command::new(maca())
        .arg("init")
        .arg(&dir)
        .output()
        .expect("spawn maca");
    assert!(
        out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    dir
}

fn maca_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(maca())
        .current_dir(dir)
        .args(args)
        .output()
        .expect("spawn maca")
}

fn text(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr)
}

#[test]
fn init_writes_a_manifest_that_says_what_the_project_is_and_the_program_it_names() {
    let dir = scaffold("shape");
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let toml = std::fs::read_to_string(dir.join("maca.toml")).expect("maca.toml");
    let main = std::fs::read_to_string(dir.join("main.maca")).expect("main.maca");

    assert!(
        toml.contains("[package]") && toml.contains(&format!("name = \"{name}\"")),
        "the project should say what it is called:\n{toml}"
    );
    assert!(
        toml.contains("[[bin]]") && toml.contains("path = \"main.maca\""),
        "and what it builds:\n{toml}"
    );
    for absent in [
        "[dependencies]",
        "[format]",
        "[scripts]",
        "[page]",
        "version",
    ] {
        assert!(
            !toml.contains(absent),
            "`{absent}` is a key the project does not need:\n{toml}"
        );
    }
    assert!(
        !dir.join(".gitignore").exists(),
        "a scaffold writes the project, not an opinion about version control"
    );

    let parsed = maca_parser::parse(&main);
    assert!(
        parsed.errors.is_empty(),
        "scaffold parse errors: {:?}",
        parsed.errors
    );
    let diags = maca_core::check(&parsed.module, maca_core::Mode::Program);
    assert!(
        diags.is_empty(),
        "scaffold should type-check, got: {diags:?}"
    );
}

#[test]
fn what_init_writes_carries_no_commentary() {
    let dir = scaffold("comments");
    for rel in ["maca.toml", "main.maca"] {
        let text = std::fs::read_to_string(dir.join(rel)).expect(rel);
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            assert!(
                !t.starts_with('#') && !t.starts_with("//"),
                "{rel}:{}: a scaffold explains itself by working, not by commenting: {line}",
                i + 1
            );
        }
    }
}

#[test]
fn a_scaffolded_project_builds_and_runs_with_no_flags_and_no_edits() {
    if !have("cc") {
        eprintln!("skipping: needs a host cc");
        return;
    }
    let _lock = BuildLock::acquire();
    let dir = scaffold("build");

    let o = maca_in(&dir, &["build"]);
    assert!(
        o.status.success(),
        "bare build of a scaffold:\n{}",
        text(&o)
    );

    let o = maca_in(&dir, &["run"]);
    let out = text(&o);
    assert!(o.status.success(), "bare run of a scaffold:\n{out}");
    assert!(out.contains("hello"), "the program should have run:\n{out}");
}

#[test]
fn a_scaffolded_project_is_testable_with_no_edits() {
    if !have("cc") {
        eprintln!("skipping: needs a host cc");
        return;
    }
    let _lock = BuildLock::acquire();
    let dir = scaffold("test");
    let o = maca_in(&dir, &["test", "main.maca"]);
    let out = text(&o);
    assert!(o.status.success(), "maca test on a scaffold:\n{out}");
    assert!(
        out.contains("no tests found"),
        "and it should say there are none yet rather than inventing one:\n{out}"
    );
}
