mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch workspace, keyed by name so concurrent cases do not share one.
fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maca-ws-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
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

/// A root that gathers two members, a library and an application, the way this repository does.
fn monorepo(name: &str) -> PathBuf {
    let dir = workspace(name);
    write(
        &dir,
        "maca.toml",
        "[package]\nname = \"repo\"\nversion = \"9.9.9\"\n\n\
         [workspace]\nmembers = [\"modules/greet\", \"apps/hello\"]\n\n\
         [format]\nindent_style = \"space\"\nindent_size = 8\n",
    );
    write(
        &dir,
        "modules/greet/maca.toml",
        "[package]\nname = \"greet\"\n",
    );
    write(
        &dir,
        "modules/greet/say.maca",
        "greeting(who: str) -> str => \"hi \" ++ who\n",
    );
    write(
        &dir,
        "modules/greet/tests/say.maca",
        "import greet/say\n\n\
         test_greeting_names_who_it_greets() {\n\
         \x20   assert_eq(greeting(\"ana\"), \"hi ana\", \"the name is in it\")\n}\n",
    );
    write(
        &dir,
        "apps/hello/maca.toml",
        "[package]\nname = \"hello\"\n\n[[bin]]\nname = \"hello\"\npath = \"main.maca\"\n",
    );
    write(
        &dir,
        "apps/hello/main.maca",
        "import greet/say\n\nmain() -> int {\n\
         \x20   info(greeting(\"world\"))\n    0\n}\n",
    );
    dir
}

#[test]
fn a_package_builds_from_its_own_directory() {
    let dir = monorepo("build-here");
    let o = maca_in(&dir.join("apps/hello"), &["build"]);
    assert!(o.status.success(), "bare build in a package:\n{}", text(&o));
    assert!(
        dir.join("apps/hello/main").exists() || dir.join("apps/hello/hello").exists(),
        "the [[bin]] should have produced a binary:\n{}",
        text(&o)
    );
}

/// A member's imports have to keep reaching the workspace's source roots, which sit above its own manifest.
#[test]
fn a_member_manifest_does_not_cut_the_import_search_short() {
    if !have("cc") {
        eprintln!("skipping: needs a host cc");
        return;
    }
    let dir = monorepo("resolve");
    let o = maca_in(&dir.join("apps/hello"), &["run"]);
    assert!(o.status.success(), "run from the package:\n{}", text(&o));
    assert!(
        text(&o).contains("hi world"),
        "the imported module should have run:\n{}",
        text(&o)
    );
}

#[test]
fn a_bare_test_runs_the_packages_own_suites() {
    if !have("cc") {
        eprintln!("skipping: needs a host cc");
        return;
    }
    let dir = monorepo("suites");
    let o = maca_in(&dir.join("modules/greet"), &["test"]);
    let out = text(&o);
    assert!(o.status.success(), "bare test in a package:\n{out}");
    assert!(
        out.contains("greet 9.9.9"),
        "the heading should name the package and the version it inherits:\n{out}"
    );
    assert!(
        out.contains("1 of 1 suites passed"),
        "the suite under tests/ should have run:\n{out}"
    );
}

#[test]
fn a_library_with_no_bin_is_told_so_rather_than_building_the_roots() {
    let dir = monorepo("no-bin");
    let o = maca_in(&dir.join("modules/greet"), &["build"]);
    let out = text(&o);
    assert!(!o.status.success(), "there is nothing to build:\n{out}");
    assert!(
        out.contains("greet") && out.contains("[[bin]]"),
        "the message should name the package and what it lacks:\n{out}"
    );
}

#[test]
fn a_member_key_beats_the_root_key_it_repeats() {
    let dir = monorepo("override");
    write(
        &dir,
        "apps/hello/maca.toml",
        "[package]\nname = \"hello\"\n\n[format]\nindent_size = 2\n\n\
         [[bin]]\nname = \"hello\"\npath = \"main.maca\"\n",
    );
    write(
        &dir,
        "apps/hello/wide.maca",
        "main() -> int {\n        0\n}\n",
    );
    let o = maca_in(&dir, &["fmt", "apps/hello/wide.maca"]);
    assert!(o.status.success(), "fmt:\n{}", text(&o));
    let got = std::fs::read_to_string(dir.join("apps/hello/wide.maca")).unwrap();
    assert!(
        got.contains("\n  0\n"),
        "the member's indent_size should have won over the root's 8:\n{got:?}"
    );
}

#[test]
fn a_root_key_the_member_is_silent_about_is_inherited() {
    let dir = monorepo("inherit");
    write(&dir, "apps/hello/wide.maca", "main() -> int {\n  0\n}\n");
    let o = maca_in(&dir, &["fmt", "apps/hello/wide.maca"]);
    assert!(o.status.success(), "fmt:\n{}", text(&o));
    let got = std::fs::read_to_string(dir.join("apps/hello/wide.maca")).unwrap();
    assert!(
        got.contains("\n        0\n"),
        "the root's indent_size should reach a member that states none:\n{got:?}"
    );
}

#[test]
fn a_member_that_is_listed_but_absent_is_an_error_naming_it() {
    let dir = monorepo("missing-member");
    write(
        &dir,
        "maca.toml",
        "[package]\nname = \"repo\"\n\n\
         [workspace]\nmembers = [\"modules/greet\", \"apps/hello\", \"apps/ghost\"]\n",
    );
    let o = maca_in(&dir.join("apps/hello"), &["build"]);
    let out = text(&o);
    assert!(!o.status.success(), "a listed member must exist:\n{out}");
    assert!(
        out.contains("apps/ghost"),
        "the message should name the member:\n{out}"
    );
}

#[test]
fn a_member_without_a_package_name_is_an_error_naming_its_manifest() {
    let dir = monorepo("nameless");
    write(
        &dir,
        "apps/hello/maca.toml",
        "[[bin]]\npath = \"main.maca\"\n",
    );
    let o = maca_in(&dir, &["build", "apps/hello/main.maca"]);
    let out = text(&o);
    assert!(!o.status.success(), "a member says what it is:\n{out}");
    assert!(
        out.contains("apps/hello") && out.contains("name"),
        "the message should name the manifest and the key:\n{out}"
    );
}

/// A directory beside a member is only a package when it says so, so a scratch directory is not one.
#[test]
fn a_stray_directory_does_not_become_a_package() {
    let dir = monorepo("stray");
    write(&dir, "apps/scratch/notes.maca", "main() -> int => 0\n");
    let o = maca_in(&dir.join("apps/hello"), &["build"]);
    assert!(
        o.status.success(),
        "a directory with no maca.toml is not a member:\n{}",
        text(&o)
    );
}

#[test]
fn a_directory_that_declares_a_package_and_is_not_listed_is_an_error_naming_it() {
    let dir = monorepo("undeclared");
    write(
        &dir,
        "apps/scratch/maca.toml",
        "[package]\nname = \"scratch\"\n",
    );
    let o = maca_in(&dir.join("apps/hello"), &["build"]);
    let out = text(&o);
    assert!(
        !o.status.success(),
        "the list and the tree must agree:\n{out}"
    );
    assert!(
        out.contains("apps/scratch"),
        "the message should name the directory:\n{out}"
    );
}

#[test]
fn a_member_may_not_declare_a_workspace_of_its_own() {
    let dir = monorepo("nested");
    write(
        &dir,
        "apps/hello/maca.toml",
        "[package]\nname = \"hello\"\n\n[workspace]\nmembers = []\n\n\
         [[bin]]\nname = \"hello\"\npath = \"main.maca\"\n",
    );
    let o = maca_in(&dir.join("apps/hello"), &["build"]);
    let out = text(&o);
    assert!(!o.status.success(), "one workspace per tree:\n{out}");
    assert!(
        out.contains("[workspace]"),
        "the message should say what is wrong:\n{out}"
    );
}

#[test]
fn two_binaries_need_the_one_to_build_naming() {
    let dir = monorepo("two-bins");
    write(
        &dir,
        "apps/hello/maca.toml",
        "[package]\nname = \"hello\"\n\n\
         [[bin]]\nname = \"hello\"\npath = \"main.maca\"\n\n\
         [[bin]]\nname = \"other\"\npath = \"other.maca\"\n",
    );
    write(&dir, "apps/hello/other.maca", "main() -> int => 3\n");

    let o = maca_in(&dir.join("apps/hello"), &["build"]);
    let out = text(&o);
    assert!(!o.status.success(), "which one was meant:\n{out}");
    assert!(
        out.contains("hello") && out.contains("other"),
        "the message should list the names to choose from:\n{out}"
    );

    let o = maca_in(&dir.join("apps/hello"), &["build", "--bin", "other"]);
    assert!(o.status.success(), "--bin picks one:\n{}", text(&o));
    assert!(
        dir.join("apps/hello/other").exists(),
        "the named binary should be the one built"
    );
}

#[test]
fn the_build_target_can_be_declared_rather_than_passed() {
    let dir = monorepo("target");
    write(
        &dir,
        "apps/hello/maca.toml",
        "[package]\nname = \"hello\"\n\n[build]\ntarget = \"rust\"\n\n\
         [[bin]]\nname = \"hello\"\npath = \"main.maca\"\n",
    );
    let o = maca_in(&dir.join("apps/hello"), &["build"]);
    let out = text(&o);
    assert!(o.status.success(), "declared target:\n{out}");
    assert!(
        dir.join("apps/hello/main.rs").exists(),
        "the rust backend should have emitted its source:\n{out}"
    );
}

/// The repository is its own witness: every directory this workspace lists is a package that says what it is.
#[test]
fn every_member_this_repository_lists_is_a_package() {
    let root = repo();
    let toml = std::fs::read_to_string(root.join("maca.toml")).expect("the root manifest");
    let members: Vec<String> = toml
        .lines()
        .map(str::trim)
        .skip_while(|l| !l.starts_with("members"))
        .skip(1)
        .take_while(|l| !l.starts_with(']'))
        .map(|l| l.trim_end_matches(',').trim_matches('"').to_string())
        .filter(|l| !l.is_empty())
        .collect();
    assert!(members.len() >= 21, "found only {} members", members.len());
    for m in &members {
        let file = root.join(m).join("maca.toml");
        let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{m}: {e}"));
        assert!(
            text.contains("[package]") && text.contains("name ="),
            "{m} does not say what it is"
        );
        assert!(
            !text.contains("version ="),
            "{m} states a version the workspace already states, which is one more copy to keep in step"
        );
        for line in text.lines().map(str::trim) {
            let Some(rel) = line.strip_prefix("path =") else {
                continue;
            };
            let rel = rel.trim().trim_matches('"');
            assert!(
                root.join(m).join(rel).is_file(),
                "{m}: [[bin]] path `{rel}` names no file"
            );
        }
    }
}
