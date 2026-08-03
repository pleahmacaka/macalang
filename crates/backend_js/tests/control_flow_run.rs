//! Statement-position control flow, emitted and then executed under node.
//!
//! `if`, `match` and a block are expressions in Maca and statements in JS, and
//! the back end used to resolve that by lowering every one of them as a value:
//! a ternary whose branches were IIFEs. An IIFE is a function boundary, and two
//! things a branch may legitimately do cannot cross one.
//!
//! `break` and `continue` are a SyntaxError there, so `maca build --target js
//! apps/site/home.maca` wrote an `app.js` node could not parse at all. A
//! reassignment is worse: `tag = "big"` inside a branch lowered to `var tag =
//! "big"` *inside the arrow function*, declaring a fresh local and leaving the
//! enclosing one alone, so the program ran and answered with a variable that
//! had never changed.
//!
//! The program run here is `crates/driver/tests/programs/js_control_flow.maca`,
//! the same file `crates/driver/tests/js_target.rs` runs natively. Sharing it
//! is the point: the expected values are not written down twice, so a back end
//! that disagrees with native turns one of the two suites red rather than
//! quietly agreeing with itself.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// The shared fixture, which lives with the driver's other Maca test programs.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../driver/tests/programs/js_control_flow.maca")
}

/// `assert`/`assert_eq` are Maca builtins with no JS lowering: they arrive as
/// bare calls, which resolve on the global object. `assert_eq` compares what it
/// is given as text, the way the native `maca_assert_eq` does.
const SHIMS: &str = r#"globalThis.assert = function (c, m) {
  if (!c) throw new Error("assert failed: " + m);
};
globalThis.assert_eq = function (got, want, m) {
  if (String(got) !== String(want)) {
    throw new Error(m + "\n  got:  " + String(got) + "\n  want: " + String(want));
  }
};
const m = require("./app.js");
const tests = Object.keys(m).filter((k) => k.startsWith("test_"));
let failed = 0;
for (const name of tests) {
  try {
    m[name]();
  } catch (e) {
    failed += 1;
    console.log("FAILED " + name + ": " + e.message);
  }
}
console.log("ran " + tests.length + " tests");
if (failed > 0) process.exit(1);
"#;

#[test]
fn the_emitted_program_computes_what_the_native_one_computes() {
    let src = std::fs::read_to_string(fixture()).expect("the shared control-flow fixture");
    let p = maca_parser::parse(&src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let js = maca_backend_js::emit(&p.module).js;

    let dir = std::env::temp_dir().join(format!("maca-js-cf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let module = dir.join("app.js");
    std::fs::File::create(&module)
        .unwrap()
        .write_all(js.as_bytes())
        .unwrap();

    // Parseable first, so a syntax error names itself instead of arriving as a
    // module that would not load.
    let checked = Command::new("node")
        .arg("--check")
        .arg(&module)
        .output()
        .expect("node is required for the JS backend tests");
    assert!(
        checked.status.success(),
        "node --check rejected the emitted program\n--- stderr ---\n{}\n--- js ---\n{js}",
        String::from_utf8_lossy(&checked.stderr)
    );

    let driver = dir.join("run.js");
    std::fs::File::create(&driver)
        .unwrap()
        .write_all(SHIMS.as_bytes())
        .unwrap();
    let out = Command::new("node")
        .arg(&driver)
        .current_dir(&dir)
        .output()
        .expect("spawn node");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "{text}{}\n--- js ---\n{js}",
        String::from_utf8_lossy(&out.stderr)
    );

    // A run that found no tests passes vacuously, so count them: the fixture's
    // `test_` functions all have to have reached JS and been called.
    let want = src.lines().filter(|l| l.starts_with("test_")).count();
    assert!(want > 0, "the fixture has no tests");
    assert!(
        text.contains(&format!("ran {want} tests")),
        "expected {want} tests to run:\n{text}"
    );
}
