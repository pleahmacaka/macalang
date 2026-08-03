//! `apps/tomo/conf.maca`: the `book.toml` reader the handbook builder and the
//! site build share.
//!
//! The assertions are in Maca (`tests/programs/conf.maca`). What stays here is
//! the process: running them needs a compiler on disk.

mod common;
use common::*;

use std::process::Command;

#[test]
fn the_config_reader_reads_what_it_claims() {
    if !have("cc") {
        eprintln!("skipping: needs a host cc");
        return;
    }
    let out = Command::new(maca())
        .current_dir(repo())
        .args(["test", "crates/driver/tests/programs/conf.maca"])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
