//! Gate for `apps/tomo` — the macalang handbook builder, written in Maca.
//!
//! Builds `tomo.maca` with the stage-0 native backend and runs its self-check
//! `main`, asserting the Maca-written markdown renderer produces the expected
//! HTML (headings, inline `**bold**`/`` `code` ``, lists, fenced code) and that
//! the i18n page shell emits a language switcher.

use std::path::PathBuf;
use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn wsl() -> bool {
    Command::new("wsl")
        .arg("true")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn tomo_renders_markdown_to_html() {
    if wsl() || !have("cc") {
        eprintln!("skipping tomo run: needs a host cc and no wsl");
        return;
    }
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/tomo/tomo.maca");
    let dir = std::env::temp_dir().join("maca-tomo");
    let _ = std::fs::create_dir_all(&dir);
    let bin = dir.join("tomo");

    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args(["build", &src.to_string_lossy(), "-o", &bin.to_string_lossy()])
        .output()
        .expect("spawn maca build");
    assert!(
        build.status.success(),
        "tomo failed to build:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let out = Command::new(&bin).output().expect("run tomo");
    let html = String::from_utf8_lossy(&out.stdout);

    // headings carry anchor ids (slugged) so the TOC can link to them
    assert!(html.contains("<h1 id=\"title\">Title</h1>"), "h1 wrong: {html}");
    assert!(html.contains("<h2 id=\"section\">Section</h2>"), "h2 wrong: {html}");
    // a table of contents generated from the document's headings
    assert!(
        html.contains("<nav class=\"toc\">")
            && html.contains("<li><a href=\"#title\">Title</a></li>")
            && html.contains("<li><a href=\"#section\">Section</a></li>"),
        "toc wrong: {html}"
    );
    // inline bold + code + link
    assert!(
        html.contains(
            "<p>A <strong>bold</strong> word and <code>code</code>, \
             see <a href=\"guide.md\">docs</a>.</p>"
        ),
        "inline formatting wrong: {html}"
    );
    // list items
    assert!(html.contains("<li>one</li>") && html.contains("<li>two</li>"), "list wrong: {html}");
    // fenced code block, HTML-escaped content
    assert!(
        html.contains("<pre><code>let x = 1\n</code></pre>"),
        "code fence wrong: {html}"
    );
    // i18n page shell: a language switcher marking the current language and
    // linking the other.
    assert!(
        html.contains("<strong>en</strong> <a href=\"../ko/\">ko</a>"),
        "i18n switcher wrong: {html}"
    );
}
