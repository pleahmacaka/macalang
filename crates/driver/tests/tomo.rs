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

/// The CLI driver: Tomo reads `book.toml` and the chapter tree and writes a
/// full HTML site — including falling back to the default language for a
/// chapter that hasn't been translated yet.
#[test]
fn tomo_builds_the_handbook_site() {
    if wsl() || !have("cc") {
        eprintln!("skipping tomo build: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = repo.join("apps/tomo/tomo.maca");
    let site = repo.join("apps/tomo/site");
    let _ = std::fs::remove_dir_all(&site);

    let dir = std::env::temp_dir().join("maca-tomo-build");
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
    // `main` builds the book with paths relative to the repo root
    let out = Command::new(&bin).current_dir(&repo).output().expect("run tomo");
    let log = String::from_utf8_lossy(&out.stdout);
    assert!(log.contains("built 14 pages"), "build log: {log}");

    // a translated chapter renders in its own language
    let ko_intro = std::fs::read_to_string(site.join("ko/00-introduction.html")).unwrap();
    assert!(ko_intro.contains("Maca 핸드북"), "ko chapter not Korean");
    // the switcher marks the current language and links the other
    assert!(
        ko_intro.contains("<a href=\"../en/\">en</a> <strong>ko</strong>"),
        "ko switcher wrong"
    );

    // an *untranslated* chapter falls back to the default language rather than
    // 404-ing — the point of difference from mdBook
    let ko_config = std::fs::read_to_string(site.join("ko/05-config-mode.html")).unwrap();
    assert!(
        ko_config.contains("One Language for Configuration"),
        "missing chapter didn't fall back to the default language"
    );
    assert!(ko_config.contains("<html lang=\"ko\">"), "fallback lost its language");

    // paragraphs join soft-wrapped lines, so inline formatting survives a wrap
    let en_config = std::fs::read_to_string(site.join("en/05-config-mode.html")).unwrap();
    assert!(
        en_config.contains("<strong>config mode</strong>"),
        "soft-wrapped bold was split across paragraphs"
    );

    // every page carries the self-contained stylesheet (no external fetch, so a
    // built book works straight off the filesystem)
    assert!(
        en_config.contains("prefers-color-scheme") && en_config.contains("<style>"),
        "page is missing its stylesheet"
    );

    // chapter-to-chapter navigation, with the boundaries handled: the first
    // chapter has no previous link, the last no next.
    let first = std::fs::read_to_string(site.join("en/00-introduction.html")).unwrap();
    assert!(
        first.contains("<a href=\"01-hello-world.html\">next")
            && !first.contains("previous"),
        "first chapter's nav wrong"
    );
    let last = std::fs::read_to_string(site.join("en/06-targets-and-tooling.html")).unwrap();
    assert!(
        last.contains("<a href=\"05-config-mode.html\">&larr; previous</a>")
            && !last.contains("next"),
        "last chapter's nav wrong"
    );
}
