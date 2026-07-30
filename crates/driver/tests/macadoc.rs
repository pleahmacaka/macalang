//! The API reference generator's own suite.
//!
//! `tools/macadoc.maca` reads `///` blocks out of Maca source and writes the
//! pages published under `/api`. What it produces is asserted in Maca, in
//! `tests/programs/macadoc.maca`: the doc-comment scanner, the signature
//! extractor, the grouping and the anchor naming are called directly, and the
//! built pages are read back.
//!
//! `sitegen.rs` runs the same generator over `modules/std` and checks the
//! reference against what `modules/std/README.md` advertises. This one is about
//! the shape of a page rather than its contents, so it builds three fixture
//! modules of its own: `modules/std` declares no types at all, and neither a
//! link from a signature to a type declared in another module nor the choice
//! between two modules declaring the same name can be exercised without them.
//!
//! What stays here is the process: the suite shells out to a compiler to run the
//! generator, so it needs a built `maca` and a host `cc`.

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
