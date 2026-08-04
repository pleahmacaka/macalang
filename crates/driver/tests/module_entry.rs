mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Project(PathBuf);

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("maca-m-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create project");
        std::fs::write(dir.join("maca.toml"), "[package]\nname = \"p\"\n").expect("manifest");
        Project(dir)
    }

    fn write(&self, rel: &str, body: &str) -> &Project {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&p, body).expect("write");
        self
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(maca())
            .current_dir(&self.0)
            .arg("-m")
            .args(args)
            .output()
            .expect("spawn maca -m")
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn skip() -> bool {
    if !have("cc") && !have_wsl() {
        eprintln!("skipping: needs a host cc or wsl");
        return true;
    }
    false
}

/// The shape the whole feature exists for.
#[test]
fn a_package_function_runs_from_the_command_line() {
    if skip() {
        return;
    }
    let p = Project::new("serve");
    p.write(
        "modules/http/server.maca",
        "listen(port: int) -> str =>\n    \"listening on {port}\"\n",
    )
    .write(
        "modules/http/serve.maca",
        "import { listen } from http/server\n\n\
         serve() -> int {\n    info(listen(8080))\n    0\n}\n",
    );

    for spec in ["http.serve", "http/serve"] {
        let out = p.run(&[spec]);
        assert!(out.status.success(), "{spec}: {}", stderr(&out));
        assert!(
            stdout(&out).contains("listening on 8080"),
            "{spec}: {}",
            stdout(&out)
        );
    }
}

/// With no function named, `main` runs: a package that can be run says so.
#[test]
fn a_bare_module_runs_its_main() {
    if skip() {
        return;
    }
    let p = Project::new("bare");
    p.write(
        "modules/greet.maca",
        "main() -> int {\n    info(\"hello from the module\")\n    0\n}\n",
    );

    let out = p.run(&["greet"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("hello from the module"));
}

/// Failing that, the function named after the module.
#[test]
fn a_module_without_a_main_runs_its_namesake() {
    if skip() {
        return;
    }
    let p = Project::new("namesake");
    p.write(
        "modules/greet.maca",
        "greet() -> int {\n    info(\"named after the module\")\n    0\n}\n",
    );

    let out = p.run(&["greet"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("named after the module"));
}

/// The rest of the command line reaches a function that asks for it.
#[test]
fn the_leftover_command_line_reaches_the_function() {
    if skip() {
        return;
    }
    let p = Project::new("args");
    p.write(
        "modules/echo.maca",
        "say(args: str[]) -> int {\n    \
         joined = args.join(\" \")\n    \
         info(\"got {args.length()}: {joined}\")\n    0\n}\n",
    );

    let out = p.run(&["echo.say", "alpha", "beta"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("got 2: alpha beta"),
        "{}",
        stdout(&out)
    );
}

/// The declared return type is what the shell sees.
#[test]
fn the_return_type_becomes_the_exit_status() {
    if skip() {
        return;
    }
    let p = Project::new("status");
    p.write(
        "modules/status.maca",
        "code() -> int => 3\n\nok() -> bool => false\n\nnote() -> str => \"nothing to report\"\n",
    );

    assert_eq!(
        p.run(&["status.code"]).status.code(),
        Some(3),
        "an int is the code"
    );
    assert_eq!(
        p.run(&["status.ok"]).status.code(),
        Some(1),
        "false is failure"
    );
    assert_eq!(
        p.run(&["status.note"]).status.code(),
        Some(0),
        "a value with no meaning to the shell still succeeds"
    );
}

/// Every way of getting it wrong says what was wrong, before anything compiles.
#[test]
fn the_refusals_name_what_is_missing() {
    if skip() {
        return;
    }
    let p = Project::new("errors");
    p.write(
        "modules/status.maca",
        "code() -> int => 0\n\nneeds(port: int) -> int => port\n",
    )
    .write("modules/quiet.maca", "helper() -> int => 0\n");

    let e = stderr(&p.run(&["nosuch.thing"]));
    assert!(e.contains("no module `nosuch`"), "{e}");
    assert!(
        e.contains("nosuch.maca") && e.contains("nosuch/thing.maca"),
        "says both places it looked: {e}"
    );

    let e = stderr(&p.run(&["status.missing"]));
    assert!(e.contains("no function `missing`"), "{e}");

    let e = stderr(&p.run(&["status.needs"]));
    assert!(e.contains("port: int"), "names the signature: {e}");
    assert!(e.contains("one `str[]`"), "says what would work: {e}");

    let e = stderr(&p.run(&["quiet"]));
    assert!(e.contains("no `main`"), "{e}");
    assert!(
        e.contains("maca -m quiet.something"),
        "suggests the fix: {e}"
    );
}

/// The generated entry module is an implementation detail and must not survive the run, nor be importable while it exists.
#[test]
fn the_generated_entry_leaves_nothing_behind() {
    if skip() {
        return;
    }
    let p = Project::new("clean");
    p.write("modules/http.maca", "serve() -> int => 0\n");

    assert!(p.run(&["http.serve"]).status.success());

    let run_dir = p.path().join(".maca/run");
    let left: Vec<String> = std::fs::read_dir(&run_dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert!(left.is_empty(), "left behind: {left:?}");
}
