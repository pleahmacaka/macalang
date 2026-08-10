mod common;
use common::*;

use std::process::Command;

/// Build the self-hosted compiler once, so each check below runs the binary a clean checkout would.
fn maca1(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    if have_wsl() || !have("cc") {
        eprintln!("skipping maca1 cli: needs a host cc and no wsl");
        return None;
    }
    let _lock = BuildLock::acquire();
    std::fs::create_dir_all(dir).unwrap();
    let bin = dir.join("maca1");
    let out = Command::new(maca())
        .current_dir(repo())
        .args([
            "build",
            "apps/maca1/main.maca",
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        out.status.success(),
        "the self-hosted compiler must build:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(bin)
}

/// `run` is `build` plus the exit code of what it built, which is what a script wants from it.
#[test]
fn maca1_runs_a_program_and_hands_back_its_exit_code() {
    let dir = std::env::temp_dir().join("maca1-cli-run");
    let Some(bin) = maca1(&dir) else { return };
    let file = dir.join("r.maca");
    std::fs::write(&file, "main() -> int {\n    info(\"ran\")\n    7\n}\n").unwrap();

    let out = Command::new(&bin)
        .args(["run", &file.to_string_lossy()])
        .output()
        .expect("spawn maca1 run");

    assert_eq!(out.status.code(), Some(7), "the program's own code, not 0");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("ran"),
        "and its output reaches the caller"
    );
}

/// `test` finds every `test_…` function, runs each, and exits with how many assertions failed.
#[test]
fn maca1_runs_every_test_function_and_counts_the_failures() {
    let dir = std::env::temp_dir().join("maca1-cli-test");
    let Some(bin) = maca1(&dir) else { return };
    let file = dir.join("t.maca");
    std::fs::write(
        &file,
        "test_one_holds() {\n    assert_eq(\"a\", \"a\", \"same\")\n}\n\n\
         test_one_does_not() {\n    assert_eq(\"a\", \"b\", \"different\")\n}\n",
    )
    .unwrap();

    let out = Command::new(&bin)
        .args(["test", &file.to_string_lossy()])
        .output()
        .expect("spawn maca1 test");

    assert_eq!(
        out.status.code(),
        Some(1),
        "one of the two failed, so the count is one:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("test_one_holds"),
        "each test names itself as it runs"
    );
}

/// `check` reports without building, and its exit code is how many things it found.
#[test]
fn maca1_checks_without_building() {
    let dir = std::env::temp_dir().join("maca1-cli-check");
    let Some(bin) = maca1(&dir) else { return };
    let good = dir.join("good.maca");
    let bad = dir.join("bad.maca");
    std::fs::write(&good, "f(n: int) -> int => n + 1\n").unwrap();
    std::fs::write(&bad, "f() -> int => \"x\"\n").unwrap();

    let ok = Command::new(&bin)
        .args(["check", &good.to_string_lossy()])
        .output()
        .expect("spawn maca1 check");
    assert_eq!(ok.status.code(), Some(0), "nothing to say about a clean file");

    let no = Command::new(&bin)
        .args(["check", &bad.to_string_lossy()])
        .output()
        .expect("spawn maca1 check");
    assert_eq!(no.status.code(), Some(1), "one mismatch is one error");
    assert!(
        String::from_utf8_lossy(&no.stdout).contains("expected int, found str"),
        "and the message names both types"
    );
}

/// The toolchain answers for its own version, because an installer verifies with it.
#[test]
fn maca1_names_its_own_version() {
    let dir = std::env::temp_dir().join("maca1-cli-version");
    let Some(bin) = maca1(&dir) else { return };

    let out = Command::new(&bin)
        .arg("--version")
        .output()
        .expect("spawn maca1 --version");

    assert!(
        String::from_utf8_lossy(&out.stdout).starts_with("maca "),
        "the shape `maca-install` reads back after installing"
    );
}

/// `fmt` rewrites in place and settles: running it twice must not move the file again.
#[test]
fn maca1_formats_in_place_and_settles() {
    let dir = std::env::temp_dir().join("maca1-cli-fmt");
    let Some(bin) = maca1(&dir) else { return };
    let file = dir.join("f.maca");
    std::fs::write(&file, "add(a: int,b: int)->int=>a+b\n").unwrap();

    let flagged = Command::new(&bin)
        .args(["fmt", &file.to_string_lossy(), "--check"])
        .output()
        .expect("spawn maca1 fmt --check");
    assert_eq!(flagged.status.code(), Some(1), "--check writes nothing, it reports");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "add(a: int,b: int)->int=>a+b\n",
        "and leaves the file alone"
    );

    let done = Command::new(&bin)
        .args(["fmt", &file.to_string_lossy()])
        .output()
        .expect("spawn maca1 fmt");
    assert_eq!(done.status.code(), Some(0), "formatting succeeds");
    let once = std::fs::read_to_string(&file).unwrap();
    assert!(once.contains("add(a: int, b: int) -> int"), "{once}");

    let again = Command::new(&bin)
        .args(["fmt", &file.to_string_lossy(), "--check"])
        .output()
        .expect("spawn maca1 fmt --check");
    assert_eq!(
        again.status.code(),
        Some(0),
        "a formatter that moves on the second run is not canonical"
    );
}

/// `init` scaffolds a project, and what it scaffolds has to run.
#[test]
fn maca1_scaffolds_a_project_that_runs() {
    let dir = std::env::temp_dir().join("maca1-cli-init");
    let Some(bin) = maca1(&dir) else { return };
    let project = dir.join("fresh");
    let _ = std::fs::remove_dir_all(&project);

    let made = Command::new(&bin)
        .args(["init", &project.to_string_lossy()])
        .output()
        .expect("spawn maca1 init");
    assert_eq!(made.status.code(), Some(0));

    let toml = std::fs::read_to_string(project.join("maca.toml")).unwrap();
    assert!(toml.contains("name = \"fresh\""), "named for its directory: {toml}");

    let ran = Command::new(&bin)
        .current_dir(&project)
        .args(["run", "main.maca"])
        .output()
        .expect("spawn maca1 run");
    assert_eq!(
        ran.status.code(),
        Some(0),
        "a scaffold that does not run is not a scaffold:\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(String::from_utf8_lossy(&ran.stdout).contains("hello"));
}

/// `--target` picks the back end, and `-o` names a binary for the native one and the source for the rest.
#[test]
fn maca1_builds_for_the_target_it_is_given() {
    let dir = std::env::temp_dir().join("maca1-cli-target");
    let Some(bin) = maca1(&dir) else { return };
    let file = dir.join("t.maca");
    std::fs::write(&file, "main() -> int {\n    info(\"hi\")\n    0\n}\n").unwrap();

    let rs = dir.join("t.rs");
    let made = Command::new(&bin)
        .args([
            "build",
            &file.to_string_lossy(),
            "-o",
            &rs.to_string_lossy(),
            "--target",
            "rust",
        ])
        .output()
        .expect("spawn maca1 build --target rust");
    assert_eq!(made.status.code(), Some(0));
    assert!(
        std::fs::read_to_string(&rs).unwrap().starts_with("use std::rc::Rc;"),
        "the Rust target writes Rust"
    );

    let no = Command::new(&bin)
        .args([
            "build",
            &file.to_string_lossy(),
            "-o",
            &dir.join("x").to_string_lossy(),
            "--target",
            "nowhere",
        ])
        .output()
        .expect("spawn maca1 build --target nowhere");
    assert_eq!(no.status.code(), Some(2), "a target nobody serves is a usage error");
}

/// Two packages may each name a function `count`: a name a file defines is that file's, whoever else claimed it.
#[test]
fn maca1_lets_two_packages_name_one_function() {
    let dir = std::env::temp_dir().join("maca1-cli-scope");
    let Some(bin) = maca1(&dir) else { return };
    let project = dir.join("p");
    for sub in ["pa", "pb"] {
        std::fs::create_dir_all(project.join(sub)).unwrap();
    }
    std::fs::write(
        project.join("pa/one.maca"),
        "count(s: str) -> int => s.length()\n",
    )
    .unwrap();
    std::fs::write(
        project.join("pb/two.maca"),
        "count(n: int) -> int => n + 1\ntwice(n: int) -> int => count(n) + count(n)\n",
    )
    .unwrap();
    std::fs::write(
        project.join("main.maca"),
        "import pa/one\nimport pb/two\n\nmain() -> int {\n    \
         info(str(count(\"abc\")))\n    info(str(twice(1)))\n    0\n}\n",
    )
    .unwrap();

    let out = Command::new(&bin)
        .current_dir(&project)
        .args(["run", "main.maca"])
        .output()
        .expect("spawn maca1 run");

    let said = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "two packages naming one function is not a clash:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        said.contains('3') && said.contains('4'),
        "each call reached its own package's function, not the other's: {said}"
    );
}

/// A suite that imports resolves from where its own file is, not from wherever a scratch file landed.
#[test]
fn maca1_runs_a_suite_that_imports_a_module() {
    let dir = std::env::temp_dir().join("maca1-cli-import");
    let Some(bin) = maca1(&dir) else { return };

    let out = Command::new(&bin)
        .current_dir(repo())
        .args(["test", "modules/std/tests/json.maca"])
        .output()
        .expect("spawn maca1 test");

    assert_eq!(
        out.status.code(),
        Some(0),
        "the import graph is walked from the suite's own directory:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
