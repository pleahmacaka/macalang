//! The two programs that generate the published site's own pages.
//!
//! `apps/site/home.maca` is the front page and `tools/macadoc.maca` is the API
//! reference. Both are Maca programs whose output is HTML, so what they emit is
//! asserted in Maca, in `tests/programs/sitegen.maca`. What stays here is the
//! process: they need a built `maca` on disk to run, because each one shells
//! out to it.

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

/// The suite runs `target/release/maca`, which a debug-only `cargo test` has no
/// reason to have built. Skipping is the honest answer — the alternative is a
/// test that passes because it silently checked nothing.
#[test]
fn the_generated_pages_are_what_they_claim() {
    if have_wsl() || !have_cc() {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    if !repo().join("target/release/maca").exists() {
        eprintln!("skipping: needs cargo build --release -p maca-driver");
        return;
    }

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .current_dir(repo())
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
