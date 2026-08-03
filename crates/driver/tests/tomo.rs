//! Gate for `apps/tomo`: the macalang handbook builder, written in Maca.
//!
//! Builds `tomo.maca` with the stage-0 native backend and runs its self-check
//! `main`, asserting the Maca-written markdown renderer produces the expected
//! HTML (headings, inline `**bold**`/`` `code` ``, lists, fenced code) and that
//! the i18n page shell emits a language switcher.

mod common;
use common::*;

use std::path::PathBuf;
use std::process::Command;

#[test]
fn tomo_renders_markdown_to_html() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping tomo run: needs a host cc and no wsl");
        return;
    }
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/tomo/tomo.maca");
    let dir = std::env::temp_dir().join("maca-tomo");
    let _ = std::fs::create_dir_all(&dir);
    let bin = dir.join("tomo");

    let build = Command::new(env!("CARGO_BIN_EXE_maca"))
        .args([
            "build",
            &src.to_string_lossy(),
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        build.status.success(),
        "tomo failed to build:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    // `main` also builds a book from `apps/tomo` relative to the working
    // directory, so run from a scratch dir. Otherwise the render check would
    // scatter a `site/` into whatever directory the test happened to run in.
    let out = Command::new(&bin)
        .current_dir(&dir)
        .output()
        .expect("run tomo");
    let html = String::from_utf8_lossy(&out.stdout);

    // headings carry anchor ids (slugged) so the TOC can link to them
    assert!(
        html.contains("id=\"title\"") && html.contains(">Title</h1>"),
        "h1 wrong: {html}"
    );
    // punctuation is dropped from the slug, so `Is it fast?` still anchors
    assert!(
        html.contains("id=\"is-it-fast\"") && html.contains(">Is it fast?</h2>"),
        "h2 wrong: {html}"
    );
    // a table of contents generated from the document's headings
    assert!(
        html.contains("data-tomo=\"toc\"")
            && html.contains("href=\"#title\"")
            && html.contains("href=\"#is-it-fast\""),
        "toc wrong: {html}"
    );
    // inline bold + code + link
    assert!(
        html.contains(
            // a `.md` target is rewritten to the page that was produced
            "<strong>bold</strong>"
        ),
        "inline formatting wrong: {html}"
    );
    // list items, including a nested one wrapped in its own <ul>
    // a list is wrapped in its own <ul>/<ol>: bare <li>s are invalid HTML and
    // render without indent or markers
    assert!(
        html.contains(">one</li>") && html.contains(">two</li>"),
        "unordered list wrong: {html}"
    );
    assert!(
        html.contains(">first</li>") && html.contains(">second</li>"),
        "ordered list wrong: {html}"
    );
    // tables: the reference appendices are built out of these
    assert!(
        html.contains(">target</th>")
            && html.contains(">flag</th>")
            && html.contains(">native</td>")
            && !html.contains(">--------</td>"),
        "table wrong: {html}"
    );
    assert!(
        html.contains("<ul><li>nested</li></ul>"),
        "nested list item wrong: {html}"
    );
    // a quote wrapped across source lines is ONE blockquote, not one per line
    assert!(
        html.contains(">a quoted aside that wraps across two source lines</blockquote>"),
        "multi-line blockquote was split: {html}"
    );
    // `<blockquote` without the closing angle: the tag carries generated
    // utility classes, so matching `<blockquote>` would count zero and pass
    // vacuously for the wrong reason.
    assert_eq!(html.matches("<blockquote").count(), 1, "blockquote split");
    // fenced code block, HTML-escaped content
    assert!(
        html.contains("language-maca") && html.contains("x = 1\n</code></pre>"),
        "code fence wrong: {html}"
    );
    // A `[` that doesn't open a link must not swallow the links after it.
    //
    // Code spans are rendered before links and emit a `text-[0.88em]` class, so
    // *every* line carrying an inline code span reaches the link pass with a
    // stray `[` in front of its real links. Giving up on that line (which is
    // what this used to do) silently broke most links in the book.
    assert!(
        html.contains("href=\"guide.html\">docs</a>"),
        "a link after a code span was lost: {html}"
    );
    assert!(
        html.contains("href=\"a.html\">two</a>") && html.contains("href=\"b.html\">links</a>"),
        "two links on one line: {html}"
    );
    // and a bare `int[]` in prose stays prose
    assert!(html.contains("int[]</code>"), "bracket in prose: {html}");
    // i18n page shell: a language switcher marking the current language and
    // linking the other.
    // the switcher is a `<details>` dropdown holding real links: a `<select>`
    // cannot navigate without script, and this is the control a reader who
    // landed in the wrong language most needs to work
    // asserted on the `data-tomo` hook rather than the classes, so restyling
    // the book doesn't break the test that checks the book works
    assert!(
        html.contains("<details data-tomo=\"lang\">") && html.contains("English ▾"),
        "i18n dropdown wrong: {html}"
    );
    assert!(
        html.contains("lang=\"ko\">한국어</a>"),
        "no link to the other language: {html}"
    );
}

/// The CLI driver: Tomo reads `book.toml` and the chapter tree and writes a
/// full HTML site, including falling back to the default language for a
/// chapter that hasn't been translated yet.
#[test]
fn tomo_builds_the_handbook_site() {
    if have_wsl() || !have("cc") {
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
        .args([
            "build",
            &src.to_string_lossy(),
            "-o",
            &bin.to_string_lossy(),
        ])
        .output()
        .expect("spawn maca build");
    assert!(
        build.status.success(),
        "tomo failed to build:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    // `main` builds the book with paths relative to the repo root
    let out = Command::new(&bin)
        .current_dir(&repo)
        .output()
        .expect("run tomo");
    let log = String::from_utf8_lossy(&out.stdout);
    // every chapter listed in book.toml, plus an index, in each of 2 languages
    // count real chapters, not the `# Section|섹션` headings, which are labels
    // in the same list and produce no page
    let entries: Vec<String> = std::fs::read_to_string(repo.join("apps/tomo/book.toml"))
        .unwrap()
        .lines()
        .map(|l| l.trim().trim_end_matches(',').trim_matches('"').to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let want = entries
        .iter()
        .filter(|e| e.starts_with(char::is_numeric) || e.starts_with('a'))
        .filter(|e| e.contains('-'))
        .count();
    let sections = entries.iter().filter(|e| e.starts_with("# ")).count();
    assert!(want >= 20, "the handbook shrank to {want} chapters");
    assert!(sections >= 3, "the book lost its sections");
    // chapters + a per-language index, in 2 languages, plus the root landing
    // page and one landing per language
    assert!(
        log.contains(&format!("built {} pages", (want + 1) * 2 + 3)),
        "expected {want} chapters + index in 2 languages + landings; log: {log}"
    );
    // sections appear as headings, in each language, and are not links
    let en_side = std::fs::read_to_string(site.join("en/08-collections.html")).unwrap();
    let ko_side = std::fs::read_to_string(site.join("ko/08-collections.html")).unwrap();
    assert!(
        en_side.contains(">The Language</li>")
            && en_side.contains("data-tomo=\"section\"")
            && ko_side.contains(">언어</li>"),
        "sections missing, or not translated"
    );

    // The root page is tomo's, and this book has no `home.md`: the front page
    // is a designed page in the UI syntax (`apps/site/home.maca`), written over
    // these three addresses by `tools/build-site.maca`. What tomo owes is the
    // fallback it documents, a language picker that links every language's
    // handbook, and that is what is asserted here. The front page's own
    // content is asserted in `tests/programs/sitegen.maca`.
    let root = std::fs::read_to_string(site.join("index.html")).unwrap();
    for want in [
        "en/index.html",
        "ko/index.html",
        "en/home.html",
        "ko/home.html",
    ] {
        assert!(
            root.contains(want),
            "the picker doesn't link {want:?}: {root}"
        );
    }
    // it links files, not bare directories: `…/ko/` only resolves to index.html
    // when a web server does it, and this book is meant to open off disk too
    assert!(!root.contains("href=\"../"), "root page links above itself");
    assert!(
        !root.contains("href=\"en/\"")
            && !root.contains("href=\"ko/\"")
            && !root.contains("href=\"play/\""),
        "root page links a bare directory, which won't open from a file:// path"
    );

    // a language's own landing resolves from inside its directory
    let ko_home = std::fs::read_to_string(site.join("ko/home.html")).unwrap();
    assert!(
        ko_home.contains("../ko/index.html"),
        "the Korean landing's links don't resolve from its directory: {ko_home}"
    );

    // a translated chapter renders in its own language
    let ko_intro = std::fs::read_to_string(site.join("ko/00-introduction.html")).unwrap();
    assert!(ko_intro.contains("Maca 핸드북"), "ko chapter not Korean");
    // the switcher marks the current language and links the other
    // the switcher keeps your place: it links the *same chapter* in the other
    // language, not that language's table of contents
    assert!(
        ko_intro.contains("한국어 ▾</summary>")
            && ko_intro.contains("href=\"../en/00-introduction.html\""),
        "ko switcher wrong"
    );

    // each language gets an index listing every chapter by its own heading
    let ko_index = std::fs::read_to_string(site.join("ko/index.html")).unwrap();
    assert!(
        ko_index.contains("href=\"00-introduction.html\"") && ko_index.contains(">소개</a>"),
        "ko index doesn't title chapters in Korean"
    );
    let en_index = std::fs::read_to_string(site.join("en/index.html")).unwrap();
    assert!(
        en_index.contains("href=\"15-targets.html\"")
            && en_index.contains("href=\"a1-keywords.html\""),
        "en index missing a chapter"
    );

    // paragraphs join soft-wrapped lines, so inline formatting survives a wrap
    let en_config = std::fs::read_to_string(site.join("en/14-config-mode.html")).unwrap();
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

    // responsive: a viewport meta, a two-column grid that collapses, and a
    // phone breakpoint. Without the meta a phone renders at desktop width and
    // every other precaution is wasted.
    assert!(
        en_config.contains("name=\"viewport\"") && en_config.contains("width=device-width"),
        "no viewport meta: a phone would render this at desktop width"
    );
    assert!(
        en_config.contains("grid-template-columns")
            && en_config.contains("@media(max-width:48rem)"),
        "the layout has no breakpoint"
    );

    // chapter-to-chapter navigation, with the boundaries handled: the first
    // chapter has no previous link, the last no next.
    let first = std::fs::read_to_string(site.join("en/00-introduction.html")).unwrap();
    assert!(
        first.contains("href=\"01-installing.html\">next") && !first.contains("← previous"),
        "first chapter's nav wrong"
    );
    let last = std::fs::read_to_string(site.join("en/a4-diagnostics.html")).unwrap();
    assert!(
        last.contains("href=\"a3-stdlib.html\">← previous</a>") && !last.contains("next →"),
        "last chapter's nav wrong"
    );
    // The pair is a ruled-off two-column grid, not two bare links: `.chapters`
    // was a class name with no rule behind it, so this nav had no styling at
    // all on every page of the book.
    assert!(
        last.contains("grid grid-cols-2") && !last.contains("class=\"chapters\""),
        "chapter nav lost its layout"
    );
}

/// Every page carries the whole book: a sidebar listing each chapter with the
/// current one marked, and a search box over a generated index. The index is a
/// `<script>` rather than JSON the page fetches, so search works when the book
/// is opened straight off disk. mdBook's needs a server.
#[test]
fn every_page_has_the_sidebar_and_a_working_search_index() {
    if have_wsl() || !have("cc") {
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
    assert!(
        Command::new(&bin)
            .current_dir(&repo)
            .output()
            .expect("run tomo")
            .status
            .success()
    );

    // a mid-book chapter lists every chapter, and marks itself
    let mid = std::fs::read_to_string(site.join("en/08-collections.html")).unwrap();
    assert!(mid.contains("data-tomo=\"side\""), "no sidebar");

    // Everything a reader navigates with lives in the left column: the title,
    // the language switcher (top-right of that column), search, the chapter
    // list, and the page's own headings. `main` carries the text and nothing
    // else: a nav bar above the prose pushes the first paragraph off a phone.
    let side = &mid[mid.find("data-tomo=\"side\"").unwrap()..mid.find("<main>").unwrap()];
    let main = &mid[mid.find("<main>").unwrap()..];
    for (probe, what) in [
        ("data-tomo=\"head\"", "header row"),
        ("data-tomo=\"i18n\"", "language switcher"),
        ("id=\"q\"", "search box"),
        ("data-tomo=\"toc\"", "the page's own headings"),
    ] {
        assert!(side.contains(probe), "sidebar is missing {what}");
        assert!(!main.contains(probe), "{what} leaked into main");
    }
    // in the header row the title comes first and the switcher after it, so the
    // switcher sits at the row's right edge
    let sh = &side[side.find("data-tomo=\"head\"").unwrap()..];
    assert!(
        sh.find("index.html").unwrap() < sh.find("data-tomo=\"i18n\"").unwrap(),
        "the language switcher should follow the title in the header row"
    );
    // the nav collapses on a narrow viewport, and ships open so it still works
    // without script
    assert!(
        side.contains("<details data-tomo=\"nav\" open>") && side.contains("<summary"),
        "the sidebar nav isn't collapsible"
    );
    assert!(
        mid.contains("href=\"00-introduction.html\">Introduction</a>")
            && mid.contains("href=\"a4-diagnostics.html\">"),
        "sidebar doesn't list the whole book"
    );
    assert!(
        mid.contains("data-tomo=\"current\"") && mid.contains("08-collections.html"),
        "sidebar doesn't mark the current chapter"
    );
    // the index page's sidebar marks nothing as current
    let idx = std::fs::read_to_string(site.join("en/index.html")).unwrap();
    assert!(idx.contains("data-tomo=\"side\""), "index has no sidebar");
    assert!(
        !idx.contains("data-tomo=\"current\""),
        "the index marked a chapter as current"
    );

    // the search index is real JavaScript, one entry per heading, carrying the
    // anchor a hit should jump to
    let js = std::fs::read_to_string(site.join("en/search-index.js")).unwrap();
    assert!(
        js.starts_with("window.TOMO_INDEX=["),
        "index isn't a script: {js:.80}"
    );
    assert!(
        js.contains("\"u\":\"03-common-concepts.html#format-specs\""),
        "index is missing a section anchor"
    );
    // section text is lowercased for matching, and HTML-unsafe characters are
    // escaped as JSON: a stray quote would break the whole index
    assert!(js.contains("\"x\":\""), "index has no body text");
    assert!(!js.contains("\n\n"), "index rows should be one line");
    // each language gets its own index, so search stays inside the language
    let ko = std::fs::read_to_string(site.join("ko/search-index.js")).unwrap();
    assert_ne!(js, ko, "both languages got the same search index");
    // and every page loads it
    assert!(
        mid.contains("<script src=\"search-index.js\">"),
        "search not wired up"
    );
}

/// Anchors have to work in every language the book ships in.
///
/// `chars()` is byte-based, so a Korean heading arrives as multi-byte
/// sequences. A slug that kept only `is_alpha() || is_ascii_digit()` deleted
/// every one of those bytes, and "값과 타입" became an empty anchor: every
/// Korean heading collapsing to `#`, in the language the i18n support exists
/// for. The rule now drops a known set of ASCII punctuation and keeps the rest.
#[test]
fn headings_anchor_in_every_language() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping tomo anchor test: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let site = repo.join("apps/tomo/site");
    let dir = std::env::temp_dir().join("maca-tomo-anchor");
    let _ = std::fs::create_dir_all(&dir);
    let bin = dir.join("tomo");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_maca"))
            .args([
                "build",
                &repo.join("apps/tomo/tomo.maca").to_string_lossy(),
                "-o",
                &bin.to_string_lossy(),
            ])
            .output()
            .expect("spawn maca build")
            .status
            .success()
    );
    assert!(
        Command::new(&bin)
            .current_dir(&repo)
            .output()
            .expect("run tomo")
            .status
            .success()
    );

    let ko = std::fs::read_to_string(site.join("ko/06-sum-types.html")).unwrap();
    assert!(
        ko.contains("id=\"선언\"") && ko.contains(">선언</h2>"),
        "a Korean heading lost its anchor"
    );
    // no heading may collapse to an empty or bare-hyphen anchor
    for bad in ["id=\"\"", "id=\"-\"", "href=\"#\""] {
        assert!(
            !ko.contains(bad),
            "degenerate anchor {bad} in the Korean page"
        );
    }
    // and the TOC links match the headings it links to
    assert!(
        ko.contains("href=\"#선언\"") && ko.contains(">선언</a>"),
        "Korean TOC doesn't link its own headings"
    );
    // punctuation is still dropped, and English is unaffected
    let en = std::fs::read_to_string(site.join("en/06-sum-types.html")).unwrap();
    assert!(
        en.contains("id=\"exhaustiveness\""),
        "English anchor changed"
    );
}

/// Tomo's point of difference from mdBook: a chapter that hasn't been
/// translated yet falls back to the default language instead of 404-ing.
/// Uses a synthetic two-chapter book so the assertion stays valid however
/// complete the real handbook's translations become.
#[test]
fn untranslated_chapters_fall_back_to_the_default_language() {
    if have_wsl() || !have("cc") {
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
    let out = Command::new(&bin)
        .current_dir(&dir)
        .output()
        .expect("run tomo");
    let log = String::from_utf8_lossy(&out.stdout);
    // 2 chapters + an index, per language, plus the root page and a landing
    // page per language (this fixture has no `home.md`, so those are the
    // language picker the root used to be)
    assert!(log.contains("built 9 pages"), "fixture build log: {log}");

    let site = book.join("site");
    // the translated chapter is Korean
    let ko_a = std::fs::read_to_string(site.join("ko/a.html")).unwrap();
    assert!(
        ko_a.contains("한국어 알파"),
        "translated chapter wrong: {ko_a}"
    );
    // the untranslated one falls back to English but stays a Korean page
    let ko_b = std::fs::read_to_string(site.join("ko/b.html")).unwrap();
    assert!(
        ko_b.contains("english beta"),
        "untranslated chapter didn't fall back: {ko_b}"
    );
    assert!(
        ko_b.contains("<html lang=\"ko\">"),
        "fallback lost its language"
    );
    // and the index mixes the translated title with the fallen-back one
    let ko_index = std::fs::read_to_string(site.join("ko/index.html")).unwrap();
    assert!(
        ko_index.contains(">알파</a>") && ko_index.contains(">Beta</a>"),
        "index didn't mix translated + fallback titles: {ko_index}"
    );
}

/// Every internal link in the built book must resolve.
///
/// Two classes of breakage got here by hand-checking and both are invisible in
/// the Markdown: a cross-chapter link is written to the *source* file
/// (`[next](01-x.md)`) so it works in an editor, and used to be emitted
/// verbatim, a 404 on every one. And a chapter rename left a link pointing at
/// a file that no longer existed. Neither shows up until someone clicks.
#[test]
fn every_link_in_the_built_book_resolves() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping tomo link check: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let site = repo.join("apps/tomo/site");
    let dir = std::env::temp_dir().join("maca-tomo-links");
    let _ = std::fs::create_dir_all(&dir);
    let bin = dir.join("tomo");
    assert!(
        Command::new(env!("CARGO_BIN_EXE_maca"))
            .args([
                "build",
                &repo.join("apps/tomo/tomo.maca").to_string_lossy(),
                "-o",
                &bin.to_string_lossy(),
            ])
            .output()
            .expect("spawn maca build")
            .status
            .success()
    );
    assert!(
        Command::new(&bin)
            .current_dir(&repo)
            .output()
            .expect("run tomo")
            .status
            .success()
    );

    let mut pages = Vec::new();
    collect_html(&site, &mut pages);
    assert!(pages.len() > 40, "only {} pages built", pages.len());

    let mut broken = Vec::new();
    let mut checked = 0usize;
    for page in &pages {
        let html = std::fs::read_to_string(page).unwrap();
        // the search index is JavaScript; its hrefs are built at run time
        let markup = strip_scripts(&html);
        for href in hrefs(&markup) {
            if href.starts_with("http")
                || href.starts_with('#')
                || href.starts_with("mailto:")
                || href.starts_with("data:")
            {
                continue;
            }
            let target = href.split('#').next().unwrap_or("");
            if target.is_empty() {
                continue;
            }
            checked += 1;
            let mut dest = page.parent().unwrap().join(target);
            if dest.is_dir() {
                dest = dest.join("index.html");
            }
            // `play/` is added by the Pages workflow, not by tomo
            // `play/` is the playground, put beside the book by the Pages
            // workflow rather than by tomo, so it isn't in this tree
            if !dest.exists() && !target.contains("play/") {
                broken.push(format!("{}: {href}", page.display()));
            }
        }
    }
    assert!(
        checked > 500,
        "only {checked} links checked. Did parsing break?"
    );
    assert!(
        broken.is_empty(),
        "broken links:\n  {}",
        broken.join("\n  ")
    );
}

fn collect_html(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_html(&p, out);
        } else if p.extension().is_some_and(|x| x == "html") {
            out.push(p);
        }
    }
}

fn strip_scripts(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(i) = rest.find("<script") {
        out.push_str(&rest[..i]);
        rest = match rest[i..].find("</script>") {
            Some(j) => &rest[i + j + 9..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// The `href` of every `<a>` in the markup.
fn hrefs(markup: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = markup;
    while let Some(i) = rest.find("<a ") {
        rest = &rest[i..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[..end];
        if let Some(h) = tag.find("href=\"") {
            let after = &tag[h + 6..];
            if let Some(q) = after.find('"') {
                out.push(after[..q].to_string());
            }
        }
        rest = &rest[end..];
    }
    out
}
