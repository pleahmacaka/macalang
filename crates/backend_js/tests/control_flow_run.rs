use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// The shared fixture, which lives with the driver's other Maca test programs.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../driver/tests/programs/js_control_flow.maca")
}

/// `assert`/`assert_eq` are Maca builtins with no JS lowering.
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

    let want = src.lines().filter(|l| l.starts_with("test_")).count();
    assert!(want > 0, "the fixture has no tests");
    assert!(
        text.contains(&format!("ran {want} tests")),
        "expected {want} tests to run:\n{text}"
    );
}
