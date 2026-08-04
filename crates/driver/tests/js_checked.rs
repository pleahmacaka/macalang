//! The JS target runs the type checker, like every other target does.

mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

fn built(program: &str, name: &str) -> (bool, String, PathBuf) {
    let dir = std::env::temp_dir().join(format!("maca-jschecked-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("main.maca");
    std::fs::write(&src, program).unwrap();
    let out = dir.join("site");

    let o = Command::new(maca())
        .args(["build", "--target", "js"])
        .arg(&src)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("spawn maca build");
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);

    (o.status.success(), text, out)
}

/// A build that is refused must not also leave a page behind, or the next step
/// in a pipeline reads a stale one and calls it success.
fn refused(program: &str, name: &str, wanted: &str) {
    let (ok, text, out) = built(program, name);

    assert!(!ok, "the build succeeded:\n{text}");
    assert!(text.contains(wanted), "wanted `{wanted}` in:\n{text}");
    assert!(
        !out.join("index.html").exists(),
        "a refused build still wrote index.html"
    );
}

#[test]
fn a_call_to_a_name_nothing_defines_is_refused() {
    refused(
        "main() -> int {\n  nowhere()\n  0\n}\n",
        "undefined",
        "UndefinedName",
    );
}

/// A view's locals are that view instance's state, so a sibling that reads one
/// has to hear about it here rather than as a `ReferenceError` in the browser.
#[test]
fn reading_another_function_s_local_is_refused_by_name() {
    let (ok, text, _) = built(
        "paneClass() -> str => tab == \"Preview\" ? \"on\" : \"off\"\n\
         \n\
         main() -> Element {\n\
         \x20 tab = \"Console\"\n\
         \n\
         \x20 choose(which: str) {\n\
         \x20   tab = which\n\
         \x20 }\n\
         \n\
         \x20 div(button(onclick=(e => choose(\"Preview\")), \"go\") span(tab))\n\
         }\n",
        "sibling",
    );

    assert!(!ok, "the build succeeded:\n{text}");
    assert!(text.contains("UndefinedName"), "{text}");
    assert!(
        text.contains("`tab`"),
        "the diagnostic must name it:\n{text}"
    );
    assert!(
        text.contains("local of another function"),
        "and say where state two functions share belongs:\n{text}"
    );
}

/// A handler's own scratch local is the same mistake one level down, and gets the same answer.
#[test]
fn reading_a_nested_handler_s_own_local_is_refused_by_name() {
    let (ok, text, _) = built(
        "board() -> Element {\n\
         \x20 grab() {\n\
         \x20   held = 1\n\
         \x20 }\n\
         \n\
         \x20 div(button(onclick=grab, \"grab\") span(\"{held}\"))\n\
         }\n\
         \n\
         main() -> Element => div(board())\n",
        "nestedlocal",
    );

    assert!(!ok, "the build succeeded:\n{text}");
    assert!(
        text.contains("`held` is a local of another function"),
        "a name bound inside a nested function is one of its locals:\n{text}"
    );
}

/// Calling another function's nested handler is one diagnostic, not the call check and the read check both.
#[test]
fn calling_another_function_s_nested_handler_is_named_once() {
    let (ok, text, _) = built(
        "board() -> Element {\n\
         \x20 grab() {\n\
         \x20   held = 1\n\
         \x20 }\n\
         \n\
         \x20 div(button(onclick=grab, \"grab\"))\n\
         }\n\
         \n\
         main() -> Element {\n\
         \x20 grab()\n\
         \x20 div(board())\n\
         }\n",
        "once",
    );

    assert!(!ok, "the build succeeded:\n{text}");
    assert_eq!(
        text.matches("UndefinedName").count(),
        1,
        "one name, one complaint:\n{text}"
    );
}

/// The same shape with the state left where both can see it is the fix, and has to keep building.
#[test]
fn top_level_state_two_functions_share_still_builds() {
    let (ok, text, out) = built(
        "tab = \"Console\"\n\
         \n\
         paneClass() -> str => tab == \"Preview\" ? \"on\" : \"off\"\n\
         \n\
         choose(which: str) {\n\
         \x20 tab = which\n\
         }\n\
         \n\
         main() -> Element =>\n\
         \x20 div(button(onclick=(e => choose(\"Preview\")), \"go\")\n\
         \x20     span(class=paneClass(), tab))\n",
        "shared",
    );

    assert!(ok, "{text}");
    assert!(out.join("index.html").exists(), "no page was written");
}

/// A name a view keeps to itself is still ordinary: nothing outside reads it, and it compiles.
#[test]
fn a_view_local_only_its_own_handler_writes_still_builds() {
    let (ok, text, out) = built(
        "board() -> Element {\n\
         \x20 grip = 0\n\
         \n\
         \x20 grab() {\n\
         \x20   grip = grip + 1\n\
         \x20 }\n\
         \n\
         \x20 div(button(onclick=grab, \"grab\") span(\"{grip}\"))\n\
         }\n\
         \n\
         main() -> Element => div(board())\n",
        "viewlocal",
    );

    assert!(ok, "{text}");
    assert!(out.join("index.html").exists(), "no page was written");
}

#[test]
fn a_type_mismatch_is_refused() {
    refused(
        "take(n: int) -> int => n\n\nmain() -> int => take(\"text\")\n",
        "mismatch",
        "TypeMismatch",
    );
}

#[test]
fn a_match_that_misses_a_variant_is_refused() {
    refused(
        "Colour = Red | Green\n\nname(c: Colour) -> str =>\n  match c {\n    Red => \"red\"\n  }\n\nmain() -> int {\n  info(name(Red))\n  0\n}\n",
        "nonexhaustive",
        "NonExhaustive",
    );
}

#[test]
fn writing_a_constant_is_refused() {
    refused(
        "main() -> int {\n  const n = 1\n  n = 2\n  0\n}\n",
        "immutable",
        "Immutable",
    );
}

/// The checker running is not worth much if it also rejects what the target
/// legitimately builds, so a real page has to keep compiling.
#[test]
fn a_program_the_checker_accepts_still_builds() {
    let (ok, text, out) = built(
        "greet(who: str) -> str => \"hello {who}\"\n\nmain() -> Element =>\n  div(class=\"p-4\", greet(\"world\"))\n",
        "accepted",
    );

    assert!(ok, "{text}");
    assert!(out.join("index.html").exists(), "no page was written");
}

/// `--target tauri` reaches the same emitter through `build_js`, so it must not
/// be a way around the checker.
#[test]
fn the_tauri_target_is_checked_too() {
    let dir = std::env::temp_dir().join(format!("maca-jschecked-tauri-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("main.maca");
    std::fs::write(&src, "main() -> int {\n  nowhere()\n  0\n}\n").unwrap();

    let o = Command::new(maca())
        .args(["build", "--target", "tauri"])
        .arg(&src)
        .arg("-o")
        .arg(dir.join("app"))
        .output()
        .expect("spawn maca build");
    let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);

    assert!(!o.status.success(), "the build succeeded:\n{text}");
    assert!(text.contains("UndefinedName"), "{text}");
}
