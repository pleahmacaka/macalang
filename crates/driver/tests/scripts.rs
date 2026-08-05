mod common;
use common::*;

use std::process::Command;

/// Compile `script` from the repository root, where its `import std/…` and its own relative paths resolve.
fn builds(script: &str) {
    if have_wsl() || !have("cc") {
        eprintln!("skipping {script}: needs a host cc and no wsl");
        return;
    }

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
    builds("apps/build_site/build_site.maca");
}

#[test]
fn the_package_builder_compiles() {
    builds("apps/npm/build.maca");
}

#[test]
fn the_benchmark_harness_compiles() {
    builds("apps/bench/run.maca");
}

/// The corpus pipeline and its collector, whose licence gate decides what may be redistributed.
#[test]
fn the_corpus_pipeline_compiles() {
    builds("apps/corpus/build.maca");
    builds("apps/corpus/collect.maca");
}

/// The eval harness and the porter that generates its problems.
#[test]
fn the_eval_harness_compiles() {
    builds("apps/evals/run.maca");
    builds("apps/evals/port.maca");
}

#[test]
fn the_linter_compiles() {
    builds("apps/lint/lint.maca");
}

#[test]
fn bindgen_compiles() {
    builds("apps/bindgen/bindgen.maca");
}

#[test]
fn the_front_page_compiles() {
    builds("apps/site/home.maca");
}

#[test]
fn macadoc_compiles() {
    builds("apps/macadoc/macadoc.maca");
}
