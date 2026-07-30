//! The front page's benchmark section and its "Get started" button.
//!
//! `apps/site/home.maca` reads `apps/bench/results.json` at build time and
//! renders the comparison out of it, so what is asserted is the emitted HTML
//! against that file. Those assertions are in Maca, in
//! `tests/programs/site_home.maca`. What stays here is the process: the page is
//! a program, so the suite needs a built `maca` on disk to run it with.

mod common;
use common::*;

use std::process::Command;

/// `MACA` names the compiler the suite runs the page with: the binary this
/// `cargo test` just built. Hardcoding `target/release/maca` would skip the
/// whole suite in CI, where the test job has no reason to have built one.
#[test]
fn the_benchmark_section_says_what_the_data_says() {
    if !have("cc") {
        eprintln!("skipping: needs a host cc");
        return;
    }

    let maca = env!("CARGO_BIN_EXE_maca");
    let out = Command::new(maca)
        .current_dir(repo())
        .env("MACA", maca)
        .args(["test", "crates/driver/tests/programs/site_home.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
