mod common;
use common::*;

use std::process::Command;

/// A command nobody serves must not be read as "compile argv[1] over argv[2]", which silently overwrote a source file.
#[test]
fn an_unknown_command_writes_nothing() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca1-guard");
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("maca1");
    let build = {
        let _lock = BuildLock::acquire();
        Command::new(maca())
            .current_dir(repo())
            .args([
                "build",
                "apps/maca1/main.maca",
                "-o",
                &bin.to_string_lossy(),
            ])
            .output()
            .expect("spawn maca build")
    };
    assert!(build.status.success());

    let victim = dir.join("precious.maca");
    let before = "main() -> int => 0\n";
    std::fs::write(&victim, before).unwrap();

    let out = Command::new(&bin)
        .args(["nonsense", &victim.to_string_lossy()])
        .output()
        .expect("spawn maca1");

    assert_eq!(
        out.status.code(),
        Some(2),
        "an unknown command is a usage error"
    );
    assert_eq!(
        std::fs::read_to_string(&victim).unwrap(),
        before,
        "and the file named after it is still what it was"
    );
}
