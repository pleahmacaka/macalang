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

/// The source `crates/backend_js/tests/view_state.rs` repaints: building it goes to the target that has a DOM, and forcing the other one refuses rather than printing a page whose button is dead.
#[test]
fn a_view_whose_handler_writes_a_local_is_built_for_the_target_that_can_run_it() {
    if unsupported_host() {
        return;
    }

    let dir = std::env::temp_dir().join("maca-native-ui");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("board.maca");
    std::fs::write(
        &file,
        r#"board() -> Element {
    grip = 0

    grab() {
        grip = grip + 1
    }

    div(button(onclick=grab, "grab") span("{grip}"))
}

main() -> Element => div(board())
"#,
    )
    .expect("write source");

    let built = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &file.to_string_lossy(),
            "-o",
            &dir.join("board-out").to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        built.status.success(),
        "a view returning Element picks the JS target:\n{}\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    let app = std::fs::read_to_string(dir.join("board-out/app.js")).expect("emitted app.js");
    assert!(
        app.contains("_cell("),
        "the local became the view's own cell"
    );

    let native = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &file.to_string_lossy()])
        .output()
        .expect("spawn maca run");
    assert!(
        !native.status.success(),
        "a handler has nowhere to attach in an HTML string, so forcing native must refuse:\n{}",
        String::from_utf8_lossy(&native.stdout)
    );
}
