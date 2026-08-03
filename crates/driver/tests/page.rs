//! What the built page says it is, and what it carries.
//!
//! `maca build --target js` names the page after its source file and inlines
//! nothing but the program, so a project that wanted a title of its own, a
//! vendor stylesheet or a third-party script had to patch the emitted HTML
//! afterwards with string replaces (`html.replace("<title>home</title>", …)`).
//! Three ways to be silently wrong: the title match depends on the file's name,
//! and either replace failing is a no-op nobody is told about.
//!
//! So the page's identity comes from `[page]` in `maca.toml`, and its assets
//! from `import css "path"` / `import js "path"`, which are read at build time
//! and inlined. A path that does not resolve fails the build, naming the file.
//!
//! The assertions are on the built `index.html` rather than in Maca, because
//! what is under test is the build's product, not a value a program can see.

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

/// The UI every case below builds. `imports` is what the page declares above it.
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

// ---- [page] ---------------------------------------------------------------

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
    // A title is somebody's prose, and prose has ampersands in it.
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
    // A misspelt key that silently kept the old title is the failure this
    // section exists to remove, with a longer detour.
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

// ---- assets ---------------------------------------------------------------

/// A vendor stylesheet and a vendor script, with bytes no generated page could
/// contain by accident.
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
    // Inlined, not referenced: a <link> or a <script src> to a file the build
    // never copied is the failure mode this replaces.
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

#[test]
fn an_asset_cannot_close_the_element_it_is_inlined_into() {
    // An HTML parser ends a <script> at the first `</script`, even inside a
    // JavaScript string, and reads the rest of the page as markup. Inlining is
    // what makes this our problem, so the sequence is escaped on the way in.
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
    // The app still follows the vendor script, which is only true if the vendor
    // script did not end its own element early.
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
    // The quoted form names a file and the `"""…"""` form is the source itself.
    // Told apart wrongly, this program looks for a file called
    // `.inline-marker { … }`.
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
    // `maca fmt` and the module inliner both reprint source through the
    // pretty-printer. It has to write a raw block back as a raw block: printed
    // as a quoted string, an inline stylesheet would come back as the name of a
    // file that does not exist.
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

    // The selectively imported module is reprinted from its tree, raw block and
    // all, so both forms have to reach the page.
    let page = built_page(&dir);
    assert!(page.contains(".module-marker { color: olive }"), "{page}");
    assert!(
        page.contains(".vendor-marker { color: rebeccapurple }"),
        "{page}"
    );
}

#[test]
fn the_desktop_window_takes_the_page_title_too() {
    // One app, one name: `--target tauri` builds the same page and titles the
    // window from the same line.
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
