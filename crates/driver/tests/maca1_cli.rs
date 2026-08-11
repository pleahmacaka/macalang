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

/// `spec` is the whole language as one document, useful only if it fits a context window.
#[test]
fn maca1_prints_the_specification_within_its_budget() {
    let dir = std::env::temp_dir().join("maca1-cli-spec");
    let Some(bin) = maca1(&dir) else { return };

    let out = Command::new(&bin)
        .current_dir(repo())
        .arg("spec")
        .output()
        .expect("spawn maca1 spec");

    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("## The language") && text.contains("no `fn` keyword"),
        "the language section and the mistakes table are both in it"
    );
    assert!(
        text.contains("`std/text`"),
        "and the standard library is indexed out of the tree"
    );
    let tokens = text.len() / 3;
    assert!(
        tokens > 5_000 && tokens <= 15_000,
        "{tokens} tokens is outside the budget the document is written to"
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

/// With no file named, a command is about the package here: its one `[[bin]]`, or a message naming the choice.
#[test]
fn maca1_takes_its_entry_from_the_manifest() {
    let dir = std::env::temp_dir().join("maca1-cli-manifest");
    let Some(bin) = maca1(&dir) else { return };
    let solo = dir.join("solo");
    let pair = dir.join("pair");
    let bare = dir.join("bare");
    for d in [&solo, &pair, &bare] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(
        solo.join("maca.toml"),
        "[package]\nname = \"solo\"\n\n[[bin]]\nname = \"solo\"\npath = \"app.maca\"\n",
    )
    .unwrap();
    std::fs::write(
        solo.join("app.maca"),
        "main() -> int {\n    info(\"from the manifest\")\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        pair.join("maca.toml"),
        "[package]\nname = \"pair\"\n\n[[bin]]\nname = \"a\"\npath = \"a.maca\"\n\n\
         [[bin]]\nname = \"b\"\npath = \"b.maca\"\n",
    )
    .unwrap();
    for (file, word) in [("a.maca", "A"), ("b.maca", "B")] {
        std::fs::write(
            pair.join(file),
            format!("main() -> int {{\n    info(\"{word}\")\n    0\n}}\n"),
        )
        .unwrap();
    }

    let one = Command::new(&bin)
        .current_dir(&solo)
        .arg("run")
        .output()
        .expect("spawn maca1 run");
    assert_eq!(
        one.status.code(),
        Some(0),
        "the single [[bin]] is the entry:\n{}",
        String::from_utf8_lossy(&one.stderr)
    );
    assert!(String::from_utf8_lossy(&one.stdout).contains("from the manifest"));

    let two = Command::new(&bin)
        .current_dir(&pair)
        .arg("run")
        .output()
        .expect("spawn maca1 run");
    assert_eq!(
        two.status.code(),
        Some(2),
        "two binaries and no --bin is a usage error"
    );
    let said = String::from_utf8_lossy(&two.stderr);
    assert!(said.contains("one of a, b"), "the message names them: {said}");

    let chosen = Command::new(&bin)
        .current_dir(&pair)
        .args(["run", "--bin", "b"])
        .output()
        .expect("spawn maca1 run --bin");
    assert_eq!(chosen.status.code(), Some(0), "--bin names one of them");
    assert!(String::from_utf8_lossy(&chosen.stdout).contains('B'));

    let none = Command::new(&bin)
        .current_dir(&bare)
        .arg("run")
        .output()
        .expect("spawn maca1 run");
    assert_eq!(
        none.status.code(),
        Some(2),
        "no manifest and no file is a usage error"
    );
    assert!(
        String::from_utf8_lossy(&none.stderr).contains("no maca.toml here"),
        "and the message says what is missing"
    );

    std::fs::write(
        solo.join("maca.toml"),
        "[package]\nname = \"solo\"\n\n[build]\ncflags = \"-lnosuchlibrary\"\n\n\
         [[bin]]\nname = \"solo\"\npath = \"app.maca\"\n",
    )
    .unwrap();
    let linked = Command::new(&bin)
        .current_dir(&solo)
        .arg("run")
        .output()
        .expect("spawn maca1 run");
    assert_ne!(
        linked.status.code(),
        Some(0),
        "[build] cflags reach the C compiler, so a library nobody has fails the link"
    );
}

/// Help is what a user reads first, so every spelling of the question prints the same page.
#[test]
fn maca1_prints_its_usage_and_names_an_unknown_command() {
    let dir = std::env::temp_dir().join("maca1-cli-help");
    let Some(bin) = maca1(&dir) else { return };

    for spelling in ["--help", "-h", "help"] {
        let out = Command::new(&bin)
            .arg(spelling)
            .output()
            .expect("spawn maca1 help");
        let said = String::from_utf8_lossy(&out.stdout).into_owned();
        assert_eq!(out.status.code(), Some(0), "asking for help is not an error");
        assert!(
            said.contains("usage: maca <command> [args]") && said.contains("  build "),
            "`{spelling}` prints the command list: {said}"
        );
    }

    for spelling in ["--version", "-V", "version"] {
        let out = Command::new(&bin)
            .arg(spelling)
            .output()
            .expect("spawn maca1 version");
        assert!(
            String::from_utf8_lossy(&out.stdout).starts_with("maca "),
            "`{spelling}` asks the same question as --version"
        );
    }

    let no = Command::new(&bin)
        .current_dir(&dir)
        .arg("nosuchcommand")
        .output()
        .expect("spawn maca1 nosuchcommand");
    assert_eq!(
        no.status.code(),
        Some(2),
        "a command nobody serves is a usage error"
    );
    assert!(
        String::from_utf8_lossy(&no.stderr).contains("unknown command `nosuchcommand`"),
        "it names what it did not understand"
    );
    assert!(
        String::from_utf8_lossy(&no.stdout).contains("usage: maca"),
        "and prints the page that would have told them"
    );
}

/// `dev` writes the flake its own starter describes, so the message and the emitter agree.
#[test]
fn maca1_writes_the_flake_its_own_starter_describes() {
    let dir = std::env::temp_dir().join("maca1-cli-dev");
    let Some(bin) = maca1(&dir) else { return };
    let _ = std::fs::remove_file(dir.join("dev.maca"));
    let _ = std::fs::remove_file(dir.join("flake.nix"));

    let out = Command::new(&bin)
        .current_dir(&dir)
        .arg("dev")
        .output()
        .expect("spawn maca1 dev");
    assert!(!out.status.success(), "no dev.maca here is a failure");

    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    let starter: String = text
        .lines()
        .filter(|l| l.starts_with("    "))
        .map(|l| format!("{}\n", l.trim_start()))
        .collect();
    assert!(starter.contains("dev.packages"), "no starter in:\n{text}");
    std::fs::write(dir.join("dev.maca"), &starter).unwrap();

    let out = Command::new(&bin)
        .current_dir(&dir)
        .arg("dev")
        .output()
        .expect("spawn maca1 dev");
    assert!(
        out.status.success(),
        "the printed starter must compile:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let flake = std::fs::read_to_string(dir.join("flake.nix")).expect("no flake.nix");
    assert!(
        flake.contains("pkgs.mkShell") && flake.contains("pkgs.rustc"),
        "the flake must carry the shell the config declared:\n{flake}"
    );
}

/// `profile` runs the program under callgrind and answers with where its instructions went.
#[test]
fn maca1_profiles_a_run_under_callgrind() {
    if !have("valgrind") {
        eprintln!("skipping maca1 profile: needs valgrind");
        return;
    }
    let dir = std::env::temp_dir().join("maca1-cli-profile");
    let Some(bin) = maca1(&dir) else { return };
    let file = dir.join("hot.maca");
    std::fs::write(
        &file,
        "spin(n: int, acc: int) -> int {\n    n <= 0 ? acc : spin(n - 1, acc + n)\n}\n\nmain() -> int {\n    info(\"{spin(20000, 0)}\")\n    0\n}\n",
    )
    .unwrap();
    let svg = dir.join("hot.svg");
    let _ = std::fs::remove_file(&svg);
    let src = file.to_string_lossy().to_string();
    let out_svg = svg.to_string_lossy().to_string();

    let out = Command::new(&bin)
        .current_dir(&dir)
        .args(["profile", &src, "-o", &out_svg])
        .output()
        .expect("spawn maca1 profile");

    let table = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "profile should succeed:\n{table}");
    assert!(
        table.contains("self%") && table.contains("spin"),
        "the hot function must show in the table:\n{table}"
    );
    let flame = std::fs::read_to_string(&svg).expect("no flame graph was written");
    assert!(
        flame.starts_with("<svg") && flame.contains("Ir"),
        "the flame graph must be an svg counted in instructions"
    );
}



/// A `[scripts]` name in maca.toml is a command of its own, and its exit code is the caller's.
#[test]
fn maca1_runs_a_scripts_alias_from_the_manifest() {
    let dir = std::env::temp_dir().join("maca1-cli-scripts");
    let Some(bin) = maca1(&dir) else { return };
    let project = dir.join("scripted");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("maca.toml"),
        "[package]\nname = \"scripted\"\n\n[scripts]\n\
         greet = \"echo hi-from-scripts\"\nboom = \"exit 3\"\n",
    )
    .unwrap();

    let ran = Command::new(&bin)
        .current_dir(&project)
        .arg("greet")
        .output()
        .expect("spawn maca1 greet");
    assert_eq!(ran.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&ran.stdout).contains("hi-from-scripts"),
        "the table names a command, and the command runs"
    );

    let failed = Command::new(&bin)
        .current_dir(&project)
        .arg("boom")
        .output()
        .expect("spawn maca1 boom");
    assert_eq!(
        failed.status.code(),
        Some(3),
        "a script that fails fails the caller, so a chain of them stops"
    );
}

/// `-m` runs one function of a module, found the way an import finds it, and answers with its result.
#[test]
fn maca1_runs_one_function_out_of_a_module() {
    let dir = std::env::temp_dir().join("maca1-cli-module");
    let Some(bin) = maca1(&dir) else { return };
    let project = dir.join("moduled");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("src/greeter.maca"),
        "greet(args: str[]) -> int {\n    info(\"greeted {str(args.length())}\")\n    7\n}\n\
         \ngreeter() -> int => 3\n",
    )
    .unwrap();

    let ran = Command::new(&bin)
        .current_dir(&project)
        .args(["-m", "greeter.greet", "a", "b"])
        .output()
        .expect("spawn maca1 -m");
    assert_eq!(
        ran.status.code(),
        Some(7),
        "the function's own answer is the exit code"
    );
    assert!(
        String::from_utf8_lossy(&ran.stdout).contains("greeted 2"),
        "and what is left of the command line reaches it"
    );

    let bare = Command::new(&bin)
        .current_dir(&project)
        .args(["-m", "greeter"])
        .output()
        .expect("spawn maca1 -m greeter");
    assert_eq!(
        bare.status.code(),
        Some(3),
        "a module with no `main` is run by the function named after it"
    );

    let missing = Command::new(&bin)
        .current_dir(&project)
        .args(["-m", "greeter.nope"])
        .output()
        .expect("spawn maca1 -m greeter.nope");
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("no function `nope`"),
        "and a function that is not there is named rather than guessed at"
    );
}
