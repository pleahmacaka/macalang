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

    // `main` also builds a book from `apps/tomo` relative to the working
    // directory, so run from a scratch dir — otherwise the render check would
    // scatter a `site/` into whatever directory the test happened to run in.
    let out = Command::new(&bin)
        .current_dir(&dir)
        .output()
        .expect("run tomo");
    let html = String::from_utf8_lossy(&out.stdout);

    // headings carry anchor ids (slugged) so the TOC can link to them
    assert!(html.contains("<h1 id=\"title\">Title</h1>"), "h1 wrong: {html}");
    // punctuation is dropped from the slug, so `Is it fast?` still anchors
    assert!(
        html.contains("<h2 id=\"is-it-fast\">Is it fast?</h2>"),
        "h2 wrong: {html}"
    );
    // a table of contents generated from the document's headings
    assert!(
        html.contains("<nav class=\"toc\">")
            && html.contains("<li><a href=\"#title\">Title</a></li>")
            && html.contains("<li><a href=\"#is-it-fast\">Is it fast?</a></li>"),
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
    // list items, including a nested one wrapped in its own <ul>
    // a list is wrapped in its own <ul>/<ol> — bare <li>s are invalid HTML and
    // render without indent or markers
    assert!(
        html.contains("<ul>\n<li>one</li>") && html.contains("<li>two</li>\n</ul>"),
        "unordered list wrong: {html}"
    );
    assert!(
        html.contains("<ol>\n<li>first</li>\n<li>second</li>\n</ol>"),
        "ordered list wrong: {html}"
    );
    // tables — the reference appendices are built out of these
    assert!(
        html.contains("<table>\n<thead>\n<tr><th>target</th><th>flag</th></tr>")
            && html.contains("<tr><td>native</td><td><code>--target c</code></td></tr>")
            && !html.contains("<td>--------</td>"),
        "table wrong: {html}"
    );
    assert!(
        html.contains("<ul><li>nested</li></ul>"),
        "nested list item wrong: {html}"
    );
    // blockquotes
    assert!(
        html.contains("<blockquote>a quoted aside</blockquote>"),
        "blockquote wrong: {html}"
    );
    // fenced code block, HTML-escaped content
    assert!(
        html.contains("<pre><code class=\"language-maca\">x = 1\n</code></pre>"),
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
    // 8 chapters + an index, in each of 2 languages
    assert!(log.contains("built 18 pages"), "build log: {log}");

    // a translated chapter renders in its own language
    let ko_intro = std::fs::read_to_string(site.join("ko/00-introduction.html")).unwrap();
    assert!(ko_intro.contains("Maca 핸드북"), "ko chapter not Korean");
    // the switcher marks the current language and links the other
    assert!(
        ko_intro.contains("<a href=\"../en/\">en</a> <strong>ko</strong>"),
        "ko switcher wrong"
    );

    // each language gets an index listing every chapter by its own heading
    let ko_index = std::fs::read_to_string(site.join("ko/index.html")).unwrap();
    assert!(
        ko_index.contains("<a href=\"00-introduction.html\">소개</a>"),
        "ko index doesn't title chapters in Korean"
    );
    let en_index = std::fs::read_to_string(site.join("en/index.html")).unwrap();
    assert!(
        en_index.contains("<a href=\"06-targets-and-tooling.html\">Targets and Tooling</a>"),
        "en index missing a chapter"
    );

    // paragraphs join soft-wrapped lines, so inline formatting survives a wrap
    let en_config = std::fs::read_to_string(site.join("en/05-config-mode.html")).unwrap();
    assert!(
        en_config.contains("<strong>config mode</strong>"),
        "soft-wrapped bold was split across paragraphs"
    );

    // a quote wrapped across source lines becomes ONE blockquote, not one per line
    let en_err = std::fs::read_to_string(site.join("en/07-errors-and-testing.html")).unwrap();
    assert_eq!(
        en_err.matches("<blockquote>").count(),
        1,
        "multi-line blockquote was split"
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
    let last = std::fs::read_to_string(site.join("en/07-errors-and-testing.html")).unwrap();
    assert!(
        last.contains("<a href=\"06-targets-and-tooling.html\">&larr; previous</a>")
            && !last.contains("next &rarr;"),
        "last chapter's nav wrong"
    );
}

/// Every page carries the whole book: a sidebar listing each chapter with the
/// current one marked, and a search box over a generated index. The index is a
/// `<script>` rather than JSON the page fetches, so search works when the book
/// is opened straight off disk — mdBook's needs a server.
#[test]
fn every_page_has_the_sidebar_and_a_working_search_index() {
    if wsl() || !have("cc") {
        eprintln!("skipping tomo sidebar test: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let site = repo.join("apps/tomo/site");
    let dir = std::env::temp_dir().join("maca-tomo-side");
    let _ = std::fs::create_dir_all(&dir);
    let bin = dir.join("tomo");
    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &repo.join("apps/tomo/tomo.maca").to_string_lossy(),
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(build.status.success(), "tomo build failed");
    assert!(Command::new(&bin)
        .current_dir(&repo)
        .output()
        .expect("run tomo")
        .status
        .success());

    // a mid-book chapter lists every chapter, and marks itself
    let mid = std::fs::read_to_string(site.join("en/03-functions-and-control-flow.html")).unwrap();
    assert!(mid.contains("<div class=\"side\">"), "no sidebar");
    assert!(
        mid.contains("href=\"00-introduction.html\">Introduction</a>")
            && mid.contains("href=\"07-errors-and-testing.html\">"),
        "sidebar doesn't list the whole book"
    );
    assert!(
        mid.contains("<a class=\"cur\" href=\"03-functions-and-control-flow.html\">"),
        "sidebar doesn't mark the current chapter"
    );
    // the index page's sidebar marks nothing as current
    let idx = std::fs::read_to_string(site.join("en/index.html")).unwrap();
    assert!(idx.contains("<div class=\"side\">") && !idx.contains("class=\"cur\""));

    // the search index is real JavaScript, one entry per heading, carrying the
    // anchor a hit should jump to
    let js = std::fs::read_to_string(site.join("en/search-index.js")).unwrap();
    assert!(js.starts_with("window.TOMO_INDEX=["), "index isn't a script: {js:.80}");
    assert!(
        js.contains("\"u\":\"02-values-and-types.html#format-specs\""),
        "index is missing a section anchor"
    );
    // section text is lowercased for matching, and HTML-unsafe characters are
    // escaped as JSON — a stray quote would break the whole index
    assert!(js.contains("\"x\":\""), "index has no body text");
    assert!(!js.contains("\n\n"), "index rows should be one line");
    // each language gets its own index, so search stays inside the language
    let ko = std::fs::read_to_string(site.join("ko/search-index.js")).unwrap();
    assert_ne!(js, ko, "both languages got the same search index");
    // and every page loads it
    assert!(mid.contains("<script src=\"search-index.js\">"), "search not wired up");
}

/// Tomo's point of difference from mdBook: a chapter that hasn't been
/// translated yet falls back to the default language instead of 404-ing.
/// Uses a synthetic two-chapter book so the assertion stays valid however
/// complete the real handbook's translations become.
#[test]
fn untranslated_chapters_fall_back_to_the_default_language() {
    if wsl() || !have("cc") {
        eprintln!("skipping tomo fallback test: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join("maca-tomo-fallback");
    let _ = std::fs::remove_dir_all(&dir);
    // `main` builds "apps/tomo" relative to the working directory, so mirror
    // that layout inside the fixture.
    let book = dir.join("apps/tomo");
    std::fs::create_dir_all(book.join("book/en")).unwrap();
    std::fs::create_dir_all(book.join("book/ko")).unwrap();
    std::fs::write(
        book.join("book.toml"),
        "[book]\ntitle = \"Fixture\"\nlanguages = [\"en\", \"ko\"]\nchapters = [\n    \"a\",\n    \"b\",\n]\n",
    )
    .unwrap();
    std::fs::write(book.join("book/en/a.md"), "# Alpha\n\nenglish alpha\n").unwrap();
    std::fs::write(book.join("book/en/b.md"), "# Beta\n\nenglish beta\n").unwrap();
    // only `a` is translated; `b` must fall back
    std::fs::write(book.join("book/ko/a.md"), "# 알파\n\n한국어 알파\n").unwrap();

    let bin = dir.join("tomo");
    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &repo.join("apps/tomo/tomo.maca").to_string_lossy(),
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(build.status.success(), "tomo build failed");
    let out = Command::new(&bin).current_dir(&dir).output().expect("run tomo");
    let log = String::from_utf8_lossy(&out.stdout);
    // 2 chapters + an index, per language
    assert!(log.contains("built 6 pages"), "fixture build log: {log}");

    let site = book.join("site");
    // the translated chapter is Korean
    let ko_a = std::fs::read_to_string(site.join("ko/a.html")).unwrap();
    assert!(ko_a.contains("한국어 알파"), "translated chapter wrong: {ko_a}");
    // the untranslated one falls back to English but stays a Korean page
    let ko_b = std::fs::read_to_string(site.join("ko/b.html")).unwrap();
    assert!(
        ko_b.contains("english beta"),
        "untranslated chapter didn't fall back: {ko_b}"
    );
    assert!(ko_b.contains("<html lang=\"ko\">"), "fallback lost its language");
    // and the index mixes the translated title with the fallen-back one
    let ko_index = std::fs::read_to_string(site.join("ko/index.html")).unwrap();
    assert!(
        ko_index.contains(">알파</a>") && ko_index.contains(">Beta</a>"),
        "index didn't mix translated + fallback titles: {ko_index}"
    );
}
