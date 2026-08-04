mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch project, keyed by name so concurrent tests do not share one.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maca-page-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// The UI every case below builds.
fn app(dir: &Path, imports: &str) {
    write(
        dir,
        "home.maca",
        &format!("{imports}\nmain() -> Element =>\n    div(class=\"card\" \"hello\")\n"),
    );
}

fn build(dir: &Path, target: &str) -> std::process::Output {
    Command::new(maca())
        .args(["build", "--target", target])
        .arg(dir.join("home.maca"))
        .arg("-o")
        .arg(dir.join("out"))
        .output()
        .expect("spawn maca")
}

fn built_page(dir: &Path) -> String {
    let o = build(dir, "js");
    assert!(
        o.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    std::fs::read_to_string(dir.join("out/index.html")).expect("index.html")
}

fn errors(o: &std::process::Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr)
}

#[test]
fn the_title_comes_from_the_manifest() {
    let dir = project("title");
    write(&dir, "maca.toml", "[page]\ntitle = \"tabpane\"\n");
    app(&dir, "");

    let page = built_page(&dir);
    assert!(
        page.contains("<title>tabpane</title>"),
        "the manifest's title should name the page\n{page}"
    );
    assert!(
        !page.contains("<title>home</title>"),
        "the file's stem should not still name it\n{page}"
    );
}

#[test]
fn without_a_page_section_the_title_is_the_file_stem() {
    let dir = project("stem");
    app(&dir, "");

    let page = built_page(&dir);
    assert!(
        page.contains("<title>home</title>"),
        "the old behaviour is the fallback\n{page}"
    );
    assert!(
        page.contains("<html>"),
        "no [page] lang means no lang attribute\n{page}"
    );
}

#[test]
fn the_language_and_the_description_come_from_the_manifest() {
    let dir = project("meta");
    write(
        &dir,
        "maca.toml",
        "[page]\ntitle = \"탭페인\"\nlang = \"ko\"\ndescription = \"a browser start page\"\n",
    );
    app(&dir, "");

    let page = built_page(&dir);
    assert!(page.contains("<html lang=\"ko\">"), "{page}");
    assert!(
        page.contains("<meta name=\"description\" content=\"a browser start page\">"),
        "{page}"
    );
    assert!(page.contains("<title>탭페인</title>"), "{page}");
}

#[test]
fn a_title_is_escaped_rather_than_pasted() {
    let dir = project("escape");
    write(&dir, "maca.toml", "[page]\ntitle = \"Ben & Jerry <b>\"\n");
    app(&dir, "");

    let page = built_page(&dir);
    assert!(
        page.contains("<title>Ben &amp; Jerry &lt;b&gt;</title>"),
        "{page}"
    );
}

#[test]
fn an_unknown_page_key_fails_the_build_naming_it() {
    let dir = project("unknown-key");
    write(&dir, "maca.toml", "[page]\ntitel = \"tabpane\"\n");
    app(&dir, "");

    let o = build(&dir, "js");
    assert!(!o.status.success(), "an unknown key should fail the build");
    let text = errors(&o);
    assert!(
        text.contains("titel"),
        "the message should name the key\n{text}"
    );
}

/// A vendor stylesheet and a vendor script, with bytes no generated page could contain by accident.
fn vendor(dir: &Path) {
    write(
        dir,
        "vendor/daisyui.css",
        ".vendor-marker { color: rebeccapurple }\n",
    );
    write(
        dir,
        "vendor/iconify-icon.js",
        "globalThis.vendorIconifyMarker = 1;\n",
    );
}

#[test]
fn a_declared_stylesheet_and_script_are_inlined_into_the_page() {
    let dir = project("assets");
    vendor(&dir);
    app(
        &dir,
        "import css \"vendor/daisyui.css\"\nimport js \"vendor/iconify-icon.js\"\n",
    );

    let page = built_page(&dir);
    assert!(
        page.contains(".vendor-marker { color: rebeccapurple }"),
        "the stylesheet's bytes belong in the page\n{page}"
    );
    assert!(
        page.contains("globalThis.vendorIconifyMarker = 1;"),
        "the script's bytes belong in the page\n{page}"
    );
    assert!(
        !page.contains("vendor/daisyui.css") && !page.contains("vendor/iconify-icon.js"),
        "the page should not point at files beside it\n{page}"
    );

    let sheet = page.find(".vendor-marker").unwrap();
    let mount = page.find("<div id=\"app\"></div>").unwrap();
    let script = page.find("globalThis.vendorIconifyMarker").unwrap();
    assert!(
        sheet < mount,
        "a vendor sheet belongs in the head, ahead of the generated one\n{page}"
    );
    assert!(
        script > mount,
        "a vendor script belongs after the element the app mounts into\n{page}"
    );
}

/// A browser runs an ES module only when the page says it is one, so a script's `export` is a SyntaxError that takes the whole file with it.
#[test]
fn an_es_module_asset_is_run_as_a_module() {
    let dir = project("esm");
    write(&dir, "vendor/widget.mjs", "export const widget = 1;\n");
    write(&dir, "vendor/legacy.js", "globalThis.legacyMarker = 1;\n");
    app(
        &dir,
        "import js \"vendor/widget.mjs\"\nimport js \"vendor/legacy.js\"\n",
    );

    let page = built_page(&dir);
    let module = page
        .find("export const widget")
        .expect("the module's bytes");
    let classic = page
        .find("globalThis.legacyMarker")
        .expect("the script's bytes");
    let opened = |at: usize| page[..at].rfind("<script").expect("an opening tag");
    assert!(
        page[opened(module)..module].contains("type=\"module\""),
        "an .mjs asset is a module\n{page}"
    );
    assert!(
        !page[opened(classic)..classic].contains("type=\"module\""),
        "an ordinary script is not\n{page}"
    );
}

/// The package states which of its files are modules, the same way node reads it.
#[test]
fn a_package_that_calls_its_scripts_modules_is_believed() {
    let dir = project("esm-package");
    write(&dir, "vendor/package.json", "{ \"type\": \"module\" }\n");
    write(&dir, "vendor/widget.js", "export const widget = 1;\n");
    app(&dir, "import js \"vendor/widget.js\"\n");

    let page = built_page(&dir);
    let at = page
        .find("export const widget")
        .expect("the module's bytes");
    let opened = page[..at].rfind("<script").expect("an opening tag");
    assert!(
        page[opened..at].contains("type=\"module\""),
        "`type: module` makes a .js file a module\n{page}"
    );
}

#[test]
fn an_asset_cannot_close_the_element_it_is_inlined_into() {
    let dir = project("closing-tag");
    write(&dir, "vendor/writer.js", "document.write(\"</script>\");\n");
    write(
        &dir,
        "vendor/quote.css",
        ".q::after { content: \"</style>\" }\n",
    );
    app(
        &dir,
        "import css \"vendor/quote.css\"\nimport js \"vendor/writer.js\"\n",
    );

    let page = built_page(&dir);
    assert!(
        page.contains("document.write(\"<\\/script>\")"),
        "the script's closing tag should be escaped\n{page}"
    );
    assert!(
        page.contains("content: \"<\\/style>\""),
        "the stylesheet's closing tag should be escaped\n{page}"
    );
    let vendor_js = page.find("document.write").unwrap();
    let app_js = page.find("\"use strict\"").expect("the app's own script");
    assert!(
        vendor_js < app_js,
        "the app should still be on the page\n{page}"
    );
}

#[test]
fn a_missing_asset_fails_the_build_naming_the_file() {
    let dir = project("missing");
    app(&dir, "import css \"vendor/missing.css\"\n");

    let o = build(&dir, "js");
    assert!(!o.status.success(), "a missing asset should fail the build");
    let text = errors(&o);
    assert!(
        text.contains("vendor/missing.css"),
        "the message should name the file\n{text}"
    );
}

#[test]
fn a_raw_block_is_still_inline_source_and_not_a_path() {
    let dir = project("inline");
    app(
        &dir,
        "import css \"\"\"\n.inline-marker { color: teal }\n\"\"\"\n\
         import js \"\"\"\nglobalThis.inlineMarker = 1;\n\"\"\"\n",
    );

    let page = built_page(&dir);
    assert!(page.contains(".inline-marker { color: teal }"), "{page}");
    assert!(page.contains("globalThis.inlineMarker = 1;"), "{page}");
}

#[test]
fn both_forms_survive_the_formatter_and_the_module_inliner() {
    let dir = project("roundtrip");
    vendor(&dir);
    write(
        &dir,
        "lib/style.maca",
        "import css \"\"\"\n.module-marker { color: olive }\n\"\"\"\n\n\
         label_of(n: str) -> str => n\n",
    );
    app(
        &dir,
        "import { label_of } from lib/style\nimport css \"vendor/daisyui.css\"\n",
    );

    let o = Command::new(maca())
        .args(["fmt", &dir.join("home.maca").to_string_lossy()])
        .output()
        .expect("spawn maca fmt");
    assert!(o.status.success(), "fmt failed: {}", errors(&o));

    let source = std::fs::read_to_string(dir.join("home.maca")).unwrap();
    assert!(
        source.contains("import css \"vendor/daisyui.css\""),
        "the formatter must not rewrite an asset import\n{source}"
    );

    let page = built_page(&dir);
    assert!(page.contains(".module-marker { color: olive }"), "{page}");
    assert!(
        page.contains(".vendor-marker { color: rebeccapurple }"),
        "{page}"
    );
}

#[test]
fn the_desktop_window_takes_the_page_title_too() {
    let dir = project("tauri");
    write(&dir, "maca.toml", "[page]\ntitle = \"tabpane\"\n");
    app(&dir, "");

    let o = build(&dir, "tauri");
    assert!(o.status.success(), "tauri scaffold failed: {}", errors(&o));
    let conf = std::fs::read_to_string(dir.join("out/src-tauri/tauri.conf.json")).unwrap();
    assert!(
        conf.contains("tabpane"),
        "the window title should come from [page]\n{conf}"
    );
}

/// A bridge module declares what its `import js` block provides, and the block provides all of it, so slicing one declaration away leaves the block naming a function the program no longer declares.
#[test]
fn a_selective_import_keeps_every_declaration_its_block_provides() {
    let dir = project("bridge");
    write(
        &dir,
        "lib/clock.maca",
        "/// The hour and minute.\nclock_time() -> str\n\n\
         /// Today, written out.\nclock_date() -> str\n\n\
         import js \"\"\"\nmaca.provide({\n\
         \x20 clock_time: () => \"12:00\",\n\
         \x20 clock_date: () => \"2026-08-04\",\n});\n\"\"\"\n",
    );
    write(
        &dir,
        "home.maca",
        "import { clock_time } from lib/clock\n\n\
         main() -> Element =>\n    div(class=\"card\" span(clock_time()))\n",
    );

    let page = built_page(&dir);
    let app = std::fs::read_to_string(dir.join("out/app.js")).expect("app.js");
    assert!(
        app.contains("const _declared = [\"clock_time\", \"clock_date\"]"),
        "the block provides both, so the program declares both:\n{app}"
    );
    assert!(
        page.contains("clock_date: () =>"),
        "the block is in the page:\n{page}"
    );
}
