//! `modules/http`: the server, driven by a client in the same program.
//!
//! The assertions are Maca (`modules/http/tests/server.maca`), including the
//! ones that go through a real socket: the suite `spawn`s the accept loop and
//! fetches from it. What stays here is the process: running any of it needs a
//! compiler and a C toolchain on disk.

mod common;
use common::*;

use std::process::Command;

#[test]
fn the_server_answers_what_the_module_claims() {
    if !have("cc") && !have_wsl() {
        eprintln!("skipping: needs a host cc or wsl");
        return;
    }
    let out = Command::new(maca())
        .current_dir(repo())
        .args(["test", "modules/http/tests/server.maca"])
        .output()
        .expect("spawn maca test");
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
