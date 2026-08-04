mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch page inside the workspace, so it can still reach `modules/web`.
fn scratch(name: &str) -> PathBuf {
    let dir = repo()
        .join("target")
        .join(format!("maca-web-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn errors(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr)
}

/// A page whose one piece of state the browser keeps.
const PAGE: &str = "import web/storage\n\n\
     locked = stored(\"homepage.locked\", true)\n\
     note = stored(\"homepage.note\", \"hello\")\n\n\
     toggle() -> int {\n\
     \x20   locked = !locked\n\
     \x20   0\n\
     }\n\n\
     rename(v: str) -> int {\n\
     \x20   note = v\n\
     \x20   0\n\
     }\n\n\
     main() -> Element =>\n\
     \x20   div(button(onclick=toggle() \"toggle\") span(\"{locked}{note}\"))\n";

fn build_page(dir: &Path, source: &str) -> std::process::Output {
    std::fs::write(dir.join("home.maca"), source).unwrap();
    Command::new(maca())
        .args(["build", "--target", "js"])
        .arg(dir.join("home.maca"))
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .expect("spawn maca")
}

fn app_js(dir: &Path, source: &str) -> String {
    let o = build_page(dir, source);
    assert!(o.status.success(), "build failed: {}", errors(&o));
    std::fs::read_to_string(dir.join("out/app.js")).expect("app.js")
}

/// A node harness: a `localStorage` and nothing else, so the page is exercised as the browser would.
const HARNESS: &str = r#"
const path = process.argv[2];
const seed = JSON.parse(process.argv[3]);
const store = new Map(Object.entries(seed));
globalThis.localStorage = {
  getItem: (k) => (store.has(k) ? store.get(k) : null),
  setItem: (k, v) => { store.set(k, String(v)); },
  removeItem: (k) => { store.delete(k); },
};
const app = require(path);
const out = { start: {}, after: {}, saved: {}, reload: {} };
out.start.locked = app.state.locked;
out.start.note = app.state.note;
app.toggle();
app.rename("changed");
out.after.locked = app.state.locked;
out.after.note = app.state.note;
out.saved = Object.fromEntries(store);
delete require.cache[require.resolve(path)];
const again = require(path);
out.reload.locked = again.state.locked;
out.reload.note = again.state.note;
console.log(JSON.stringify(out));
"#;

fn in_node(dir: &Path, seed: &str) -> serde_json::Value {
    let harness = dir.join("harness.js");
    std::fs::write(&harness, HARNESS).unwrap();
    let o = Command::new("node")
        .arg(&harness)
        .arg(dir.join("out/app.js"))
        .arg(seed)
        .output()
        .expect("spawn node");
    assert!(o.status.success(), "node failed: {}", errors(&o));
    let text = String::from_utf8_lossy(&o.stdout);
    serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[test]
fn a_stored_name_starts_at_its_declared_value_when_nothing_is_saved() {
    if !have("node") {
        eprintln!("skipping: no node on PATH");
        return;
    }
    let dir = scratch("fresh");
    app_js(&dir, PAGE);
    let out = in_node(&dir, "{}");
    assert_eq!(out["start"]["locked"], serde_json::json!(true));
    assert_eq!(out["start"]["note"], serde_json::json!("hello"));
}

#[test]
fn a_stored_name_starts_at_what_the_browser_saved() {
    if !have("node") {
        eprintln!("skipping: no node on PATH");
        return;
    }
    let dir = scratch("restore");
    app_js(&dir, PAGE);
    let out = in_node(
        &dir,
        "{\"homepage.locked\":\"false\",\"homepage.note\":\"\\\"kept\\\"\"}",
    );
    assert_eq!(
        out["start"]["locked"],
        serde_json::json!(false),
        "the saved value wins over the declared one"
    );
    assert_eq!(out["start"]["note"], serde_json::json!("kept"));
}

#[test]
fn assigning_a_stored_name_saves_it_and_a_reload_sees_it() {
    if !have("node") {
        eprintln!("skipping: no node on PATH");
        return;
    }
    let dir = scratch("persist");
    app_js(&dir, PAGE);
    let out = in_node(&dir, "{}");
    assert_eq!(out["after"]["locked"], serde_json::json!(false));
    assert_eq!(out["after"]["note"], serde_json::json!("changed"));
    assert_eq!(
        out["saved"]["homepage.locked"],
        serde_json::json!("false"),
        "the write reached the browser, not just the page\n{out}"
    );
    assert_eq!(
        out["saved"]["homepage.note"],
        serde_json::json!("\"changed\"")
    );
    assert_eq!(
        out["reload"]["locked"],
        serde_json::json!(false),
        "and a fresh visit starts from it\n{out}"
    );
    assert_eq!(out["reload"]["note"], serde_json::json!("changed"));
}

#[test]
fn the_program_never_writes_a_read_or_a_write_call_of_its_own() {
    let dir = scratch("sugar");
    let js = app_js(&dir, PAGE);
    let source = std::fs::read_to_string(dir.join("home.maca")).unwrap();
    assert!(
        !source.contains("local_start") && !source.contains("local_store"),
        "assignment is the save, so the program says neither\n{source}"
    );
    assert!(
        js.contains("local_store(`homepage.locked`"),
        "and the emitted page does\n{js}"
    );
}

#[test]
fn a_key_that_is_not_written_down_fails_the_build_saying_so() {
    let dir = scratch("computed-key");
    let o = build_page(
        &dir,
        "import web/storage\n\n\
         locked = stored(\"a\" ++ \"b\", true)\n\n\
         main() -> Element =>\n\
         \x20   div(span(\"{locked}\"))\n",
    );
    assert!(!o.status.success(), "a slot needs a name a build can read");
    assert!(
        errors(&o).contains("stored"),
        "the message should say which call it was about\n{}",
        errors(&o)
    );
}

#[test]
fn storing_without_the_module_that_implements_it_says_which_import_is_missing() {
    let dir = scratch("no-import");
    let o = build_page(
        &dir,
        "locked = stored(\"homepage.locked\", true)\n\n\
         main() -> Element =>\n\
         \x20   div(span(\"{locked}\"))\n",
    );
    assert!(
        !o.status.success(),
        "nothing implements it without the module"
    );
    assert!(
        errors(&o).contains("web/storage"),
        "the message should name the module to import\n{}",
        errors(&o)
    );
}

#[test]
fn a_constant_cannot_be_stored_because_a_stored_name_is_written_back() {
    let dir = scratch("const");
    let o = build_page(
        &dir,
        "import web/storage\n\n\
         const locked = stored(\"homepage.locked\", true)\n\n\
         main() -> Element =>\n\
         \x20   div(span(\"{locked}\"))\n",
    );
    assert!(!o.status.success(), "a constant is never assigned");
    assert!(
        errors(&o).contains("constant"),
        "the message should say why\n{}",
        errors(&o)
    );
}

#[test]
fn a_browser_module_built_for_anywhere_else_refuses_by_name() {
    let dir = scratch("native");
    std::fs::write(
        dir.join("app.maca"),
        "import { local_forget } from web/storage\n\n\
         main() -> int {\n\
         \x20   local_forget(\"x\")\n\
         \x20   0\n\
         }\n",
    )
    .unwrap();
    let _lock = BuildLock::acquire();
    let o = Command::new(maca())
        .arg("run")
        .arg(dir.join("app.maca"))
        .output()
        .expect("spawn maca");
    assert!(
        !o.status.success(),
        "a browser module has nothing to run natively"
    );
    let text = errors(&o);
    assert!(
        text.contains("web/storage"),
        "the message should name the module\n{text}"
    );
    assert!(
        text.contains("--target js"),
        "and say where it does run\n{text}"
    );
}

#[test]
fn the_pure_half_of_the_browser_modules_is_a_suite_that_runs_anywhere() {
    let _lock = BuildLock::acquire();
    let o = Command::new(maca())
        .arg("test")
        .arg(repo().join("modules/web/tests/format.maca"))
        .output()
        .expect("spawn maca test");
    assert!(o.status.success(), "web/format: {}", errors(&o));
}
