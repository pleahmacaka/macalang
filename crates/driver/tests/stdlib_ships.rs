mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A project in a temp directory, which is the only place a build can prove the standard library travelled: anywhere under this repository, `modules/` answers by being on disk.
struct Outside(PathBuf);

impl Outside {
    fn new(tag: &str) -> Outside {
        let dir = std::env::temp_dir().join(format!(
            "maca-ships-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the project");
        let here = Outside(dir);
        assert!(
            !here.dir().starts_with(real(&repo())),
            "{} is inside the repository, so `modules/` would answer for it",
            here.dir().display()
        );
        here.write("maca.toml", "[package]\nname = \"outside\"\n");
        here
    }

    fn dir(&self) -> PathBuf {
        real(&self.0)
    }

    fn write(&self, rel: &str, body: &str) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&path, body).expect("write");
        path
    }

    fn run(&self, rel: &str) -> (bool, String) {
        self.run_with(rel, &[])
    }

    fn run_with(&self, rel: &str, env: &[(&str, &Path)]) -> (bool, String) {
        let mut cmd = Command::new(maca());
        cmd.current_dir(&self.0).env("NO_COLOR", "1");
        for (key, value) in env {
            cmd.env(key, value);
        }
        let out = cmd.args(["run", rel]).output().expect("spawn maca run");
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        (out.status.success(), text)
    }
}

impl Drop for Outside {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn real(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// What `import <path>` names from a file, as a path.
fn resolve(importer: &Path, path: &str) -> Option<PathBuf> {
    let segs: Vec<String> = path.split('/').map(str::to_string).collect();
    maca_parser::modules::resolve_module_path(&segs, importer)
}

const PROGRAM: &str = r#"import { lines } from std/text
import std/json

main() -> int {
    n = lines("a\nb\nc").length()
    info("lines={n}")
    0
}
"#;

/// The whole point: a machine with the compiler on it and none of this repository can `import std/…`.
#[test]
fn a_project_outside_the_repository_can_import_the_standard_library() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();
    let p = Outside::new("plain");
    p.write("main.maca", PROGRAM);

    let (ok, out) = p.run("main.maca");
    assert!(ok, "a project outside the repository should build:\n{out}");
    assert!(
        out.contains("lines=3"),
        "and run what std/text does:\n{out}"
    );
}

/// A vendored fix is only a fix if it is the copy that gets used, including by the packages the compiler carries.
#[test]
fn a_projects_own_file_beats_the_one_the_compiler_carries() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();
    let p = Outside::new("override");
    p.write("main.maca", PROGRAM);
    p.write(
        "modules/std/text.maca",
        "lines(s: str) -> str[] => [s, s]\n",
    );

    let (ok, out) = p.run("main.maca");
    assert!(ok, "the project's own std/text should build:\n{out}");
    assert!(
        out.contains("lines=2"),
        "the project's `lines` should be the one that ran:\n{out}"
    );
}

/// A checkout can be put in front of the carried copy, which is what makes the compiler's own source workable against an installed compiler.
#[test]
fn a_named_directory_replaces_what_the_compiler_carries() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let _lock = BuildLock::acquire();
    let p = Outside::new("named");
    p.write("main.maca", PROGRAM);
    let named = p.write("elsewhere/std/text.maca", "lines(s: str) -> str[] => [s]\n");
    p.write("elsewhere/std/json.maca", "encode(s: str) -> str => s\n");
    let named = named.parent().expect("std/").parent().expect("elsewhere/");

    let (ok, out) = p.run_with("main.maca", &[(maca_stdlib::OVERRIDE, named)]);
    assert!(ok, "the named directory should build:\n{out}");
    assert!(
        out.contains("lines=1"),
        "and be where every std/… came from:\n{out}"
    );
}

/// The same, without a compiler: what each search rule resolves to, one written import at a time.
#[test]
fn the_compilers_copy_is_the_last_thing_asked() {
    let p = Outside::new("order");
    let main = p.write("main.maca", PROGRAM);

    let carried = resolve(&main, "std/text").expect("the compiler carries std/text");
    assert!(
        carried.starts_with(real(&maca_stdlib::holder())),
        "with nothing of its own the project gets the carried copy, not {}",
        carried.display()
    );

    let installed = p.write(
        "maca_modules/std/text.maca",
        "lines(s: str) -> str[] => [s]\n",
    );
    assert_eq!(
        resolve(&main, "std/text").map(|p| real(&p)),
        Some(real(&installed)),
        "an installed dependency beats the carried copy"
    );

    let own = p.write("modules/std/text.maca", "lines(s: str) -> str[] => [s]\n");
    assert_eq!(
        resolve(&main, "std/text").map(|p| real(&p)),
        Some(real(&own)),
        "and the project's own source beats both"
    );
}

/// A carried package resolves its own imports against the project that asked for it, or a replaced file would be replaced for the program and not for the package that reads it.
#[test]
fn a_carried_package_reads_the_projects_replacement_too() {
    let p = Outside::new("through");
    let main = p.write("main.maca", PROGRAM);
    let own = p.write("modules/std/text.maca", "lines(s: str) -> str[] => [s]\n");

    let json = resolve(&main, "std/json").expect("the compiler carries std/json");
    assert!(
        json.starts_with(real(&maca_stdlib::holder())),
        "std/json is the carried one: {}",
        json.display()
    );
    assert_eq!(
        resolve(&json, "std/text").map(|p| real(&p)),
        Some(real(&own)),
        "and what it imports is the project's replacement"
    );
}

/// A name no package answers for is still an error, so the fallback cannot turn a typo into a silent success.
#[test]
fn a_module_nobody_defines_is_still_missing() {
    let p = Outside::new("missing");
    let main = p.write("main.maca", "import std/nosuch\n\nmain() -> int => 0\n");
    assert_eq!(resolve(&main, "std/nosuch"), None);
    assert_eq!(resolve(&main, "std"), None, "a directory is not a module");
}

/// The copy in the binary and the copy in the tree are one standard library, and a snapshot nobody compares is one that goes stale.
#[test]
fn what_the_compiler_carries_is_what_the_tree_holds() {
    let root = repo().join("modules");
    let mut want: Vec<(String, String)> = Vec::new();
    walk(&root, &root, &mut want);
    want.sort();

    let mut got: Vec<(String, String)> = maca_stdlib::FILES
        .iter()
        .map(|(rel, text)| ((*rel).to_string(), (*text).to_string()))
        .collect();
    got.sort();

    let names =
        |v: &[(String, String)]| -> Vec<String> { v.iter().map(|(n, _)| n.clone()).collect() };
    assert_eq!(
        names(&got),
        names(&want),
        "the compiler carries different files from the ones under modules/"
    );
    let changed: Vec<&str> = got
        .iter()
        .zip(&want)
        .filter(|((_, a), (_, b))| a != b)
        .map(|((n, _), _)| n.as_str())
        .collect();
    assert!(
        changed.is_empty(),
        "carried and edited out of step: {changed:?}"
    );
}

/// Every package the repository documents ships, because one that did not would work here and fail everywhere else.
#[test]
fn every_package_ships() {
    let mut want: Vec<String> = std::fs::read_dir(repo().join("modules"))
        .expect("modules/")
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    want.sort();

    let mut got: Vec<String> = maca_stdlib::packages()
        .iter()
        .map(|p| (*p).to_string())
        .collect();
    got.sort();
    assert_eq!(got, want);
}

/// The files a released compiler needs: a package's source and the manifest naming it, and not the suite the repository runs over it.
fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                walk(root, &path, out);
            }
        } else if name.ends_with(".maca") || name == "maca.toml" {
            let rel = path
                .strip_prefix(root)
                .expect("under modules/")
                .display()
                .to_string()
                .replace('\\', "/");
            out.push((rel, std::fs::read_to_string(&path).expect("read")));
        }
    }
}
