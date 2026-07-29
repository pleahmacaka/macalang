//! The repository's own scripts, compiled.
//!
//! Building the site, building the wasm into the npm package, and running the
//! benchmarks were three bash/Python scripts. They are Maca programs now — the
//! toolchain's own tooling written in the language it compiles, which is the
//! same argument `tools/bindgen.maca` and `tools/lint.maca` already make.
//!
//! A script nothing compiles rots quietly: it is only run at release time, so a
//! rename in `std/` breaks it months before anyone finds out. This builds each
//! one. It does not *run* them — that needs a wasm target, a network, and
//! several minutes — but a script that compiles cannot have lost a function it
//! calls.

use std::process::Command;

fn have_cc() -> bool {
    Command::new("cc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn have_wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn repo() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Compile `script` from the repository root, where its `import std/…` and its
/// own relative paths resolve.
fn builds(script: &str) {
    if have_wsl() || !have_cc() {
        eprintln!("skipping {script}: needs a host cc and no wsl");
        return;
    }

    // Keyed on the path, not its length: two scripts whose names happen to be
    // the same length would otherwise share an output file and race, because
    // these run in parallel.
    let slug = script.replace(['/', '.'], "-");
    let out = std::env::temp_dir().join(format!("maca-script-{slug}"));
    let result = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
        .args(["build", script, "-o", &out.to_string_lossy()])
        .output()
        .expect("spawn maca build");

    assert!(
        result.status.success(),
        "{script} doesn't compile:\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
    );
}

#[test]
fn the_site_builder_compiles() {
    builds("tools/build-site.maca");
}

#[test]
fn the_package_builder_compiles() {
    builds("packages/macalang/build.maca");
}

#[test]
fn the_benchmark_harness_compiles() {
    builds("bench/run.maca");
}

#[test]
fn the_linter_compiles() {
    builds("tools/lint.maca");
}

#[test]
fn bindgen_compiles() {
    builds("tools/bindgen.maca");
}

#[test]
fn the_front_page_compiles() {
    builds("apps/site/home.maca");
}

#[test]
fn macadoc_compiles() {
    builds("tools/macadoc.maca");
}
