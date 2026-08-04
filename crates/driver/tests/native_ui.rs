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
    for (case, attr) in [
        ("directive", "on:click=go"),
        ("vanilla", "onclick=go"),
        ("lambda", "onclick=(e => 0)"),
        ("two-way", "value=(v => 0)"),
    ] {
        refuses(case, attr);
    }
}

/// Build a one-attribute page natively and report what the compiler said about it.
fn refuses(case: &str, attr: &str) {
    if unsupported_host() {
        return;
    }

    let dir = std::env::temp_dir().join("maca-native-ui");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join(format!("handler-{case}.maca"));
    std::fs::write(
        &file,
        format!(
            "go() -> int => 0\n\n\
             main() -> int {{\n    info(button({attr}, \"press\"))\n    0\n}}\n"
        ),
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
        "`{attr}` should not compile natively:\n{text}"
    );
    assert!(
        text.contains("--target js"),
        "`{attr}` gave an unhelpful message:\n{text}"
    );
}

/// A quoted `onclick` is what HTML has always had, and it is text like any other attribute.
#[test]
fn a_quoted_handler_is_still_an_ordinary_attribute() {
    if unsupported_host() {
        return;
    }

    let dir = std::env::temp_dir().join("maca-native-ui");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("inline.maca");
    std::fs::write(
        &file,
        "main() -> int {\n    info(button(onclick=\"alert(1)\", \"press\"))\n    0\n}\n",
    )
    .expect("write source");

    let out = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["run", &file.to_string_lossy()])
        .output()
        .expect("spawn maca run");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success() && said.contains("onclick=\"alert(1)\""),
        "a string belongs in the markup:\n{said}{}",
        String::from_utf8_lossy(&out.stderr)
    );
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
