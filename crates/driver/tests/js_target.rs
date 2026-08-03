//! The JS target, from the driver's side: what it emits has to parse, and what
//! it computes has to be what native computes.
//!
//! `maca build --target js apps/site/home.maca` once wrote an `app.js` node
//! refused to load: an `if` in statement position lowered to an IIFE, and the
//! `break` inside it could not reach the loop it belonged to. Nothing noticed,
//! because nothing in the repository had ever parsed the JS a build produced.
//! `node --check` per app is that missing step, and it is per app on purpose:
//! a failure has to name the page that broke, since each is built by a
//! different person for a different reason.
//!
//! Adding an app that builds to JS means adding a test here. There is no
//! structural marker to discover them by (`main` is `-> int` for the ones that
//! render to a string and `-> Element` for the ones that mount a DOM), so the
//! list is written down.

mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Build `src` to JS in a scratch directory and hand the emitted `app.js` to
/// `node --check`. `key` keeps concurrent tests out of each other's output.
fn parses(src: &str, key: &str) {
    let out = std::env::temp_dir().join(format!("maca-js-check-{}-{key}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let built = Command::new(maca())
        .current_dir(repo())
        .args([
            "build",
            "--target",
            "js",
            src,
            "-o",
            &out.display().to_string(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        built.status.success(),
        "{src} did not build to JS:\n{}\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr),
    );
    check(&out.join("app.js"), src);
}

/// `node --check` one emitted file, reporting the offending lines when it
/// fails: the error names a line number and nothing else.
fn check(app: &Path, src: &str) {
    let out = Command::new("node")
        .arg("--check")
        .arg(app)
        .output()
        .expect("spawn node");
    if out.status.success() {
        return;
    }
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    panic!("node --check rejected the JS emitted for {src}:\n{stderr}");
}

/// The apps this repository builds to JS. Each is its own test so a failure
/// names one.
macro_rules! js_app {
    ($name:ident, $src:expr) => {
        #[test]
        fn $name() {
            if !have("node") {
                eprintln!("skipping: no node");
                return;
            }
            parses($src, stringify!($name));
        }
    };
}

js_app!(the_front_page_parses, "apps/site/home.maca");
js_app!(the_desktop_front_parses, "apps/desktop/app.maca");
js_app!(the_signal_demo_parses, "apps/signal_demo/signal_demo.maca");
js_app!(the_bench_demo_parses, "apps/bench_demo/bench_demo.maca");
js_app!(
    the_profile_demo_parses,
    "apps/profile_demo/profile_demo.maca"
);
js_app!(the_cli_tool_parses, "apps/cli_tool/cli_tool.maca");
js_app!(the_tambo_demo_parses, "apps/tambo_demo/tambo_demo.maca");

/// The playground is the one page the site actually ships as JS, and the only
/// one that cannot be built where it stands: `import wasm` names a build
/// artifact under `target/` that a `cargo test` job has no reason to have
/// produced. So the source is copied into a tree that has a stand-in where it
/// looks for one, exactly as `tests/programs/sitegen.maca` does.
#[test]
fn the_playground_parses() {
    if !have("node") {
        eprintln!("skipping: no node");
        return;
    }
    let root = std::env::temp_dir().join(format!("maca-js-check-{}-play", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("apps/playground/playground.maca");
    let stub = root.join("target/wasm32-unknown-unknown/release/maca_wasm.wasm");
    std::fs::create_dir_all(src.parent().unwrap()).unwrap();
    std::fs::create_dir_all(stub.parent().unwrap()).unwrap();
    std::fs::copy(repo().join("apps/playground/playground.maca"), &src).unwrap();
    std::fs::write(&stub, "not the compiler").unwrap();

    let out = root.join("out");
    let built = Command::new(maca())
        .args([
            "build",
            "--target",
            "js",
            &src.display().to_string(),
            "-o",
            &out.display().to_string(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        built.status.success(),
        "the playground did not build to JS:\n{}\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr),
    );
    check(&out.join("app.js"), "apps/playground/playground.maca");
}

/// The other half of `crates/backend_js/tests/control_flow_run.rs`: the same
/// file, run natively. The JS suite asserts nothing of its own, it runs the
/// assertions this program carries, so the two suites agree on the answers or
/// one of them is red.
#[test]
fn the_control_flow_fixture_computes_the_same_thing_natively() {
    if unsupported_host() {
        return;
    }
    let _lock = BuildLock::acquire();
    let program =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/programs/js_control_flow.maca");
    let out = Command::new(maca())
        .args(["test", &program.display().to_string()])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
