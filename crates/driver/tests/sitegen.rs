//! The two programs that generate the published site's own pages.
//!
//! `apps/site/home.maca` is the front page and `tools/macadoc.maca` is the API
//! reference. Both are Maca programs whose output is HTML, so what they emit is
//! asserted in Maca, in `tests/programs/sitegen.maca`. What stays here is the
//! process: they need a built `maca` on disk to run, because each one shells
//! out to it.

mod common;
use common::*;

use std::process::Command;

/// The suite shells out to a compiler to run the two generators, and `MACA`
/// tells it which one: the binary this `cargo test` just built. It used to
/// hardcode `target/release/maca` and skip when that was absent, which is
/// every CI run — the `test` job has no reason to have made a release build,
/// so the whole suite was green having checked nothing.
#[test]
fn the_generated_pages_are_what_they_claim() {
    if !have("cc") {
        eprintln!("skipping: needs a host cc");
        return;
    }

    let maca = env!("CARGO_BIN_EXE_maca");
    let out = Command::new(maca)
        .current_dir(repo())
        .env("MACA", maca)
        .args(["test", "crates/driver/tests/programs/sitegen.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
