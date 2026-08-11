mod common;
use common::*;

use std::process::Command;

/// The MCP server is a Maca program, so the protocol it answers is checked the way every other suite is.
#[test]
fn the_mcp_server_rules_hold() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let out = Command::new(maca())
        .current_dir(repo())
        .env("NO_COLOR", "1")
        .args(["test", "apps/mcp/tests/mcp.maca"])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// `input` is an HTML tag, so a program that reads a line needs a name no element already takes.
#[test]
fn a_line_is_read_by_a_name_no_element_takes() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping: needs a host cc and no wsl");
        return;
    }
    let dir = std::env::temp_dir().join("maca-read-line");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("r.maca");
    std::fs::write(
        &file,
        "main() -> int {\n    line = read_line()\n    line == \"hi\" ? 0 : 1\n}\n",
    )
    .unwrap();
    let out = Command::new(maca())
        .current_dir(repo())
        .args(["run", &file.to_string_lossy()])
        .output()
        .expect("spawn maca run");

    assert!(
        out.status.code() != Some(101)
            && !String::from_utf8_lossy(&out.stderr).contains("void element"),
        "`input` lowered to the tag instead of the reader:\n{}",
        String::from_utf8_lossy(&out.stderr),
    );
}
