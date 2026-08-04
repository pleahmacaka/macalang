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
