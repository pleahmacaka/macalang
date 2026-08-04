mod common;
use common::*;

use std::process::Command;

#[test]
fn the_reference_pages_are_what_they_claim() {
    if !have("cc") {
        eprintln!("skipping: needs a host cc");
        return;
    }

    let maca = env!("CARGO_BIN_EXE_maca");
    let out = Command::new(maca)
        .current_dir(repo())
        .env("MACA", maca)
        .args(["test", "crates/driver/tests/programs/macadoc.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
