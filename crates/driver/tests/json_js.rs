mod common;
use common::*;

use std::process::Command;

/// `assert`/`assert_eq` are Maca builtins with no JS lowering, and `failures()` is what the native runner counts with.
const SHIMS: &str = r#"globalThis.assert = function (c, m) {
  if (!c) throw new Error("assert failed: " + m);
};
globalThis.assert_eq = function (got, want, m) {
  if (String(got) !== String(want)) {
    throw new Error(m + "\n  got:  " + String(got) + "\n  want: " + String(want));
  }
};
globalThis.failures = () => 0;
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
if (tests.length === 0) process.exit(2);
if (failed > 0) process.exit(1);
"#;

/// The typed-JSON suite is written once and holds for both back ends: `encode` and `decode` are the compiler's, not a host's.
#[test]
fn the_typed_json_suite_holds_on_the_js_target() {
    if !have("node") {
        eprintln!("skipping: no node");
        return;
    }
    let out = std::env::temp_dir().join(format!("maca-json-js-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let built = Command::new(maca())
        .current_dir(repo())
        .args([
            "build",
            "--target",
            "js",
            &program("json_typed").display().to_string(),
            "-o",
            &out.display().to_string(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        built.status.success(),
        "the typed-JSON suite did not build to JS:\n{}\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr),
    );

    std::fs::write(out.join("run.js"), SHIMS).unwrap();
    let ran = Command::new("node")
        .arg(out.join("run.js"))
        .output()
        .expect("spawn node");
    assert!(
        ran.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr),
    );
}

/// `decode` reads into a type, and the only thing that says which one is the binding.
#[test]
fn decode_with_nothing_to_read_into_says_so() {
    let dir = std::env::temp_dir().join(format!("maca-json-bare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("bare.maca");
    std::fs::write(
        &src,
        "import { decode } from std/json\n\n\
         main() -> Element => div(str(decode(\"1\")))\n",
    )
    .unwrap();

    let built = Command::new(maca())
        .current_dir(repo())
        .args([
            "build",
            "--target",
            "js",
            &src.display().to_string(),
            "-o",
            &dir.join("out").display().to_string(),
        ])
        .output()
        .expect("spawn maca build");
    let said = String::from_utf8_lossy(&built.stderr).to_string()
        + &String::from_utf8_lossy(&built.stdout);
    assert!(!built.status.success(), "the build should have failed: {said}");
    assert!(
        said.contains("say what it reads into"),
        "the diagnostic should name the fix: {said}"
    );
    assert!(
        !dir.join("out/app.js").exists(),
        "a refused build must leave no page behind"
    );
}
