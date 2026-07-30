//! A `{` after a function's `=>`, and a record literal meeting the record type
//! it is written into.
//!
//! The behaviour is asserted in Maca, in `tests/programs/arrow_records.maca`,
//! and run by `maca test`. This file is the runner plus the part that is about
//! the *process* rather than the values: the programs that must not compile, and
//! the emitted JS, because the arrow-block defect was visible on both back ends
//! and only one of them was refusing it.

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

/// A program that must not compile, and the words the diagnostic owes the
/// reader.
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

/// Every entry a distinct `name = value` with only newlines between them reads
/// as a record literal and as a block at the same time. Neither is taken, and
/// the refusal shows both spellings, because a compiler that guesses here is
/// back to the silence this fixed.
#[test]
fn an_arrow_body_that_reads_both_ways_is_refused() {
    for bad in ["bad_arrow_ambiguous", "bad_arrow_one_field"] {
        rejected(bad, "reads as a record literal and as a block");
        rejected(bad, "Write `Name { … }` for the record");
        rejected(bad, "drop the `=>` for the block");
    }
}

/// Meeting a named record type is also what checks the literal against it. A
/// record literal is open on purpose, so a field nobody wrote would otherwise be
/// silently zero, which is exactly what the `Point { … }` spelling refuses.
#[test]
fn a_literal_written_into_a_record_type_must_name_its_fields() {
    rejected("bad_record_missing_field", "missing field `y`");
    rejected("bad_record_extra_field", "unexpected field `z`");
}

/// The one position the checker does not cover, because `Expr::List` unifies its
/// elements and ignores the error so a gradual list can be heterogeneous. The
/// element type is settled by the first element, and the native emitter builds a
/// record from the *declaration*, so a literal of another shape would drop its
/// own field and zero the record's. Refused in the back end by name.
///
/// Both halves of that sentence are a refusal. A stray field is a value that
/// goes nowhere; a field the literal never writes is a zero nobody asked for,
/// and `[corner(), { x = 9 }]` used to compile and print `9 0`.
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

/// The JS backend emitted `return { a: 1, a: a };` for the arrow-block form: a
/// duplicate key, plus a reference to an `a` that was never declared, so `node`
/// answered `ReferenceError: a is not defined`. Assert on the emitted source
/// rather than only on the native run, because the native path failed loudly and
/// this one shipped.
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

    // `several()` binds two names and returns an expression over both.
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

    // And the whole suite runs under node, which is where the bad key showed up.
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
