mod common;
use common::*;

use std::process::Command;

/// The whole Maca suite.
#[test]
fn an_arrow_block_is_a_block_and_a_literal_meets_its_record_type() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let program = program("arrow_records");
    let out = Command::new(maca())
        .args(["test", &program.to_string_lossy()])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// A program that must not compile, and the words the diagnostic owes the reader.
fn rejected(name: &str, expect: &str) {
    let program = program(name);
    let out = Command::new(maca())
        .args(["build", &program.to_string_lossy()])
        .output()
        .expect("spawn maca build");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success(),
        "{name} compiled, and should not:\n{said}"
    );
    assert!(
        said.contains(expect),
        "{name} was rejected for the wrong reason (wanted {expect:?}):\n{said}"
    );
}

/// Every entry a distinct `name = value` with only newlines between them reads as a record literal and as a block at the same time.
#[test]
fn an_arrow_body_that_reads_both_ways_is_refused() {
    for bad in ["bad_arrow_ambiguous", "bad_arrow_one_field"] {
        rejected(bad, "reads as a record literal and as a block");
        rejected(bad, "Write `Name { … }` for the record");
        rejected(bad, "drop the `=>` for the block");
    }
}

/// Meeting a named record type is also what checks the literal against it.
#[test]
fn a_literal_written_into_a_record_type_must_name_its_fields() {
    rejected("bad_record_missing_field", "missing field `y`");
    rejected("bad_record_extra_field", "unexpected field `z`");
}

/// The one position the checker does not cover, because `Expr::List` unifies its elements and ignores the error so a gradual list can be heterogeneous.
#[test]
fn a_literal_of_another_shape_where_a_record_is_wanted_is_refused() {
    rejected(
        "bad_record_shape_in_list",
        "`Point` has no field `host`, so this `{ … }` is not the `Point`",
    );
    rejected(
        "bad_record_missing_in_list",
        "this `{ … }` never writes `Point`'s field `y`, so it is not the `Point`",
    );
}

/// The JS backend emitted `return { a: 1, a: a };` for the arrow-block form.
#[test]
fn the_js_backend_emits_a_block_and_not_a_record() {
    if !have("node") {
        eprintln!("skipping: needs node");
        return;
    }
    let out_dir = std::env::temp_dir().join("maca-arrow-records-js");
    let _ = std::fs::remove_dir_all(&out_dir);
    let program = program("arrow_records");
    let built = Command::new(maca())
        .args([
            "build",
            &program.to_string_lossy(),
            "--target",
            "js",
            "-o",
            &out_dir.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build --target js");
    assert!(
        built.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr),
    );
    let js = std::fs::read_to_string(out_dir.join("app.js")).expect("emitted app.js");

    let body = js
        .split("function several(")
        .nth(1)
        .expect("`several` was emitted")
        .split("\nfunction ")
        .next()
        .expect("its body");
    assert!(
        body.contains("var a = 1") && body.contains("return (a + (b * 20))"),
        "expected statements, got:\n{body}"
    );
    assert!(
        !body.contains("a: 1"),
        "the block came out as a record again:\n{body}"
    );

    let ran = Command::new("node")
        .arg("-e")
        .arg(format!(
            "const m = require({:?}); \
             if (m.several() !== 41) throw new Error('several: ' + m.several()); \
             if (m.one() !== 42) throw new Error('one: ' + m.one());",
            out_dir.join("app.js").to_string_lossy()
        ))
        .output()
        .expect("spawn node");
    assert!(
        ran.status.success(),
        "node refused the emitted module:\n{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let _ = std::fs::remove_dir_all(&out_dir);
}
