mod common;
use common::*;

use std::process::Command;

/// The licence gate and the record shape, which are what decide whether the corpus may be redistributed.
#[test]
fn the_corpus_rules_hold() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let out = Command::new(maca())
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .args(["test", "apps/corpus/tests/corpus.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// A licence nobody may redistribute must never reach the collected directory, whatever else changes.
#[test]
fn the_allowed_list_holds_no_copyleft() {
    let source = std::fs::read_to_string(repo().join("apps/corpus/licence.maca"))
        .expect("the licence module is committed");
    for refused in ["GPL", "AGPL", "LGPL", "SSPL", "CC-BY-SA"] {
        assert!(
            !source.contains(&format!("\"{refused}")),
            "`{refused}` appears in the allowed list, and a corpus is redistributed"
        );
    }
}
