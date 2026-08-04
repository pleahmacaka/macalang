mod common;
use common::*;

use std::process::Command;

/// Elements, escaping, composition, `styles()`, hyphenated and boolean attributes, runtime tags, and tag shadowing.
#[test]
fn ui_renders_to_html_natively() {
    if unsupported_host() {
        return;
    }

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["test", &program("ui").to_string_lossy()])
        .output()
        .expect("spawn maca test");

    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// A DOM handler cannot work in a string, and says so rather than emitting markup that silently does nothing.
#[test]
fn event_handlers_are_rejected_on_the_native_target() {
    if unsupported_host() {
        return;
    }

    let dir = std::env::temp_dir().join("maca-native-ui");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("handler.maca");
    std::fs::write(
        &file,
        r#"go() -> int => 0

main() -> int {
    info(button(on:click=go, "press"))
    0
}
"#,
    )
    .expect("write source");

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &file.to_string_lossy()])
        .output()
        .expect("spawn maca run");

    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "an event handler should not compile natively:\n{text}"
    );
    assert!(text.contains("--target js"), "unhelpful message:\n{text}");
}
