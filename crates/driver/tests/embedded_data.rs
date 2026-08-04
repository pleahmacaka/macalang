mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch directory inside the workspace, so the program it holds can still reach `modules/`.
fn scratch(name: &str) -> PathBuf {
    let dir = repo()
        .join("target")
        .join(format!("maca-data-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn errors(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr)
}

/// A program that reads one file into one record, built and run.
fn run_in(dir: &Path, source: &str) -> std::process::Output {
    write(dir, "app.maca", source);
    let _lock = BuildLock::acquire();
    Command::new(maca())
        .arg("run")
        .arg(dir.join("app.maca"))
        .output()
        .expect("spawn maca")
}

const READER: &str = "import { decode } from std/json\n\n\
     Entry = { title: str, count: int }\n\n\
     main() -> int {\n\
     \x20   e: Entry = data(\"conf/app.json\")\n\
     \x20   info(\"{e.title}/{e.count}\")\n\
     \x20   0\n\
     }\n";

#[test]
fn the_suite_that_reads_files_while_building_passes() {
    let _lock = BuildLock::acquire();
    let o = Command::new(maca())
        .arg("test")
        .arg(program("embedded"))
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn maca test");
    assert!(
        o.status.success(),
        "a file read while building is resolved against the source, not the working directory:\n{}",
        errors(&o)
    );
}

#[test]
fn a_missing_file_fails_the_build_naming_it() {
    let dir = scratch("missing");
    let o = run_in(&dir, READER);
    assert!(!o.status.success(), "a missing file should fail the build");
    let text = errors(&o);
    assert!(
        text.contains("conf/app.json"),
        "the message should name the file\n{text}"
    );
}

#[test]
fn reading_a_file_without_the_reader_that_types_it_says_which_import_is_missing() {
    let dir = scratch("no-decode");
    write(
        &dir,
        "conf/app.json",
        "{ \"title\": \"a\", \"count\": 1 }\n",
    );
    let o = run_in(
        &dir,
        "Entry = { title: str, count: int }\n\n\
         main() -> int {\n\
         \x20   e: Entry = data(\"conf/app.json\")\n\
         \x20   info(e.title)\n\
         \x20   0\n\
         }\n",
    );
    assert!(!o.status.success(), "nothing types the text without decode");
    let text = errors(&o);
    assert!(
        text.contains("std/json") && text.contains("decode"),
        "the message should name the import to add\n{text}"
    );
}

#[test]
fn a_path_that_is_not_written_down_fails_the_build_saying_so() {
    let dir = scratch("computed");
    write(
        &dir,
        "conf/app.json",
        "{ \"title\": \"a\", \"count\": 1 }\n",
    );
    let o = run_in(
        &dir,
        "import { decode } from std/json\n\n\
         Entry = { title: str, count: int }\n\n\
         main() -> int {\n\
         \x20   name = \"conf/\" ++ \"app.json\"\n\
         \x20   e: Entry = data(name)\n\
         \x20   info(e.title)\n\
         \x20   0\n\
         }\n",
    );
    assert!(
        !o.status.success(),
        "a build-time read needs a build-time path"
    );
    let text = errors(&o);
    assert!(
        text.contains("data"),
        "the message should say which call it was about\n{text}"
    );
}

#[test]
fn changing_the_file_changes_the_program_the_next_build_produces() {
    let dir = scratch("cache");
    write(
        &dir,
        "conf/app.json",
        "{ \"title\": \"first\", \"count\": 1 }\n",
    );
    let first = run_in(&dir, READER);
    assert!(first.status.success(), "{}", errors(&first));
    assert!(
        errors(&first).contains("first/1"),
        "the file as it was\n{}",
        errors(&first)
    );

    write(
        &dir,
        "conf/app.json",
        "{ \"title\": \"second\", \"count\": 2 }\n",
    );
    let second = run_in(&dir, READER);
    assert!(second.status.success(), "{}", errors(&second));
    assert!(
        errors(&second).contains("second/2"),
        "a build cache keyed only on the source text would answer `first/1` here\n{}",
        errors(&second)
    );
}

#[test]
fn a_local_copy_appearing_beside_a_file_takes_over_from_it() {
    let dir = scratch("shadow");
    write(
        &dir,
        "conf/app.json",
        "{ \"title\": \"public\", \"count\": 1 }\n",
    );
    let before = run_in(&dir, READER);
    assert!(errors(&before).contains("public/1"), "{}", errors(&before));

    write(
        &dir,
        "conf/app.local.json",
        "{ \"title\": \"private\", \"count\": 9 }\n",
    );
    let after = run_in(&dir, READER);
    assert!(
        errors(&after).contains("private/9"),
        "the .local copy shadows the committed one without the source naming it\n{}",
        errors(&after)
    );

    std::fs::remove_file(dir.join("conf/app.local.json")).unwrap();
    let again = run_in(&dir, READER);
    assert!(
        errors(&again).contains("public/1"),
        "and removing it hands the file back\n{}",
        errors(&again)
    );
}
