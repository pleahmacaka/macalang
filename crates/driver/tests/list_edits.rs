mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

/// Run `tests/programs/<name>.maca` under `maca test`, twice, so a released block that is read again is a wrong answer rather than a lucky one.
fn suite(name: &str) {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    for poison in ["0", "1"] {
        let out = Command::new(maca())
            .args(["test", &program(name).to_string_lossy()])
            .env("MACA_POISON", poison)
            .output()
            .expect("spawn maca test");
        assert!(
            out.status.success(),
            "MACA_POISON={poison}:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// `set`, `insert` and `remove` answer what they say, and edit in place exactly where `push` does.
#[test]
fn a_list_is_edited_in_place_only_where_nobody_else_holds_it() {
    suite("list_edits");
}

/// `encode`/`decode` round-trip a record with a nested list and a sum, and name the field when the text does not match.
#[test]
fn json_is_written_and_read_from_the_declared_type() {
    suite("json_typed");
}

/// An index the list does not have has to be *skipped*, not written: no answer changes when it is written, so valgrind is what sees it.
#[test]
fn an_edit_out_of_range_writes_nothing() {
    if have_wsl() || !have("cc") || !have("valgrind") {
        eprintln!("skipping: needs a host cc, valgrind, and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-list-edit-range");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let src = dir.join("range.maca");
    std::fs::write(
        &src,
        "main() -> int {\n\
        \x20   xs = [1, 2, 3]\n\
        \x20   ys = xs.set(7, 9).insert(-4, 8).remove(99).set(-1, 7)\n\
        \x20   info(\"{ys.length()}\")\n\
        \x20   0\n\
         }\n",
    )
    .expect("write source");

    let bin = dir.join("range");
    let built = Command::new(maca())
        .args([
            "build",
            &src.to_string_lossy(),
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert_valgrind_quiet(&bin);
}

fn assert_valgrind_quiet(bin: &PathBuf) {
    let out = Command::new("valgrind")
        .args(["--error-exitcode=9", "-q"])
        .arg(bin)
        .output()
        .expect("valgrind");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stderr.trim().is_empty(),
        "valgrind was not quiet:\n{stderr}\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
}
