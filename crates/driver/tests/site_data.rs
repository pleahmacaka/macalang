mod common;
use common::*;

use std::process::Command;

/// The figures the site prints are read off committed files, so the readers are held to those files.
#[test]
fn every_figure_the_site_states_comes_off_a_committed_file() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a native cc and no wsl");
        return;
    }
    let out = Command::new(maca())
        .args(["test", "crates/driver/tests/programs/site_data.maca"])
        .current_dir(repo())
        .env("MACA", maca())
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}
