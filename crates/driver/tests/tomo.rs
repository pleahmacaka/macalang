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

    let out = Command::new(&bin)
        .current_dir(&dir)
        .output()
        .expect("run tomo");
    let html = String::from_utf8_lossy(&out.stdout);

    assert!(
        html.contains("id=\"title\"") && html.contains(">Title</h1>"),
        "h1 wrong: {html}"
    );
    assert!(
        html.contains("id=\"is-it-fast\"") && html.contains(">Is it fast?</h2>"),
        "h2 wrong: {html}"
    );
    assert!(
        html.contains("data-tomo=\"toc\"")
            && html.contains("href=\"#title\"")
            && html.contains("href=\"#is-it-fast\""),
        "toc wrong: {html}"
    );
    assert!(
        html.contains("<strong>bold</strong>"),
        "inline formatting wrong: {html}"
    );
    assert!(
        html.contains(">one</li>") && html.contains(">two</li>"),
        "unordered list wrong: {html}"
    );
    assert!(
        html.contains(">first</li>") && html.contains(">second</li>"),
        "ordered list wrong: {html}"
    );
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
    assert!(
        html.contains(">a quoted aside that wraps across two source lines</blockquote>"),
        "multi-line blockquote was split: {html}"
    );
    assert_eq!(html.matches("<blockquote").count(), 1, "blockquote split");
    assert!(
        html.contains("language-maca") && html.contains("x = 1\n</code></pre>"),
        "code fence wrong: {html}"
    );
    assert!(
        html.contains("href=\"guide.html\">docs</a>"),
        "a link after a code span was lost: {html}"
    );
    assert!(
        html.contains("href=\"a.html\">two</a>") && html.contains("href=\"b.html\">links</a>"),
        "two links on one line: {html}"
    );
    assert!(html.contains("int[]</code>"), "bracket in prose: {html}");
    assert!(
        html.contains("<details data-tomo=\"lang\">") && html.contains("English ▾"),
        "i18n dropdown wrong: {html}"
    );
    assert!(
        html.contains("lang=\"ko\">한국어</a>"),
        "no link to the other language: {html}"
    );
}

/// The CLI driver: Tomo reads `book.toml` and the chapter tree and writes a full HTML site, including falling back to the default language for a chapter that hasn't been translated yet.
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
    let out = Command::new(&bin)
        .current_dir(&repo)
        .output()
        .expect("run tomo");
    let log = String::from_utf8_lossy(&out.stdout);
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
    assert!(
        log.contains(&format!("built {} pages", (want + 1) * 2 + 3)),
        "expected {want} chapters + index in 2 languages + landings; log: {log}"
    );
    let en_side = std::fs::read_to_string(site.join("en/08-collections.html")).unwrap();
    let ko_side = std::fs::read_to_string(site.join("ko/08-collections.html")).unwrap();
    assert!(
        en_side.contains(">The Language</li>")
            && en_side.contains("data-tomo=\"section\"")
            && ko_side.contains(">언어</li>"),
        "sections missing, or not translated"
    );

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
    assert!(!root.contains("href=\"../"), "root page links above itself");
    assert!(
        !root.contains("href=\"en/\"")
            && !root.contains("href=\"ko/\"")
            && !root.contains("href=\"play/\""),
        "root page links a bare directory, which won't open from a file:// path"
    );

    let ko_home = std::fs::read_to_string(site.join("ko/home.html")).unwrap();
    assert!(
        ko_home.contains("../ko/index.html"),
        "the Korean landing's links don't resolve from its directory: {ko_home}"
    );

    let ko_intro = std::fs::read_to_string(site.join("ko/00-introduction.html")).unwrap();
    assert!(ko_intro.contains("Maca 핸드북"), "ko chapter not Korean");
    assert!(
        ko_intro.contains("한국어 ▾</summary>")
            && ko_intro.contains("href=\"../en/00-introduction.html\""),
        "ko switcher wrong"
    );

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

    let en_config = std::fs::read_to_string(site.join("en/14-config-mode.html")).unwrap();
    assert!(
        en_config.contains("<strong>config mode</strong>"),
        "soft-wrapped bold was split across paragraphs"
    );

    assert!(
        en_config.contains("prefers-color-scheme") && en_config.contains("<style>"),
        "page is missing its stylesheet"
    );

    assert!(
        en_config.contains("name=\"viewport\"") && en_config.contains("width=device-width"),
        "no viewport meta: a phone would render this at desktop width"
    );
    assert!(
        en_config.contains("grid-template-columns")
            && en_config.contains("@media(max-width:48rem)"),
        "the layout has no breakpoint"
    );

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
    assert!(
        last.contains("grid grid-cols-2") && !last.contains("class=\"chapters\""),
        "chapter nav lost its layout"
    );
}

/// Every page carries the whole book.
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

    let mid = std::fs::read_to_string(site.join("en/08-collections.html")).unwrap();
    assert!(mid.contains("data-tomo=\"side\""), "no sidebar");

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
    let sh = &side[side.find("data-tomo=\"head\"").unwrap()..];
    assert!(
        sh.find("index.html").unwrap() < sh.find("data-tomo=\"i18n\"").unwrap(),
        "the language switcher should follow the title in the header row"
    );
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
    let idx = std::fs::read_to_string(site.join("en/index.html")).unwrap();
    assert!(idx.contains("data-tomo=\"side\""), "index has no sidebar");
    assert!(
        !idx.contains("data-tomo=\"current\""),
        "the index marked a chapter as current"
    );

    let js = std::fs::read_to_string(site.join("en/search-index.js")).unwrap();
    assert!(
        js.starts_with("window.TOMO_INDEX=["),
        "index isn't a script: {js:.80}"
    );
    assert!(
        js.contains("\"u\":\"03-common-concepts.html#format-specs\""),
        "index is missing a section anchor"
    );
    assert!(js.contains("\"x\":\""), "index has no body text");
    assert!(!js.contains("\n\n"), "index rows should be one line");
    let ko = std::fs::read_to_string(site.join("ko/search-index.js")).unwrap();
    assert_ne!(js, ko, "both languages got the same search index");
    assert!(
        mid.contains("<script src=\"search-index.js\">"),
        "search not wired up"
    );
}

/// Anchors have to work in every language the book ships in.
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
    for bad in ["id=\"\"", "id=\"-\"", "href=\"#\""] {
        assert!(
            !ko.contains(bad),
            "degenerate anchor {bad} in the Korean page"
        );
    }
    assert!(
        ko.contains("href=\"#선언\"") && ko.contains(">선언</a>"),
        "Korean TOC doesn't link its own headings"
    );
    let en = std::fs::read_to_string(site.join("en/06-sum-types.html")).unwrap();
    assert!(
        en.contains("id=\"exhaustiveness\""),
        "English anchor changed"
    );
}

/// Tomo's point of difference from mdBook.
#[test]
fn untranslated_chapters_fall_back_to_the_default_language() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping tomo fallback test: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join("maca-tomo-fallback");
    let _ = std::fs::remove_dir_all(&dir);
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
    assert!(log.contains("built 9 pages"), "fixture build log: {log}");

    let site = book.join("site");
    let ko_a = std::fs::read_to_string(site.join("ko/a.html")).unwrap();
    assert!(
        ko_a.contains("한국어 알파"),
        "translated chapter wrong: {ko_a}"
    );
    let ko_b = std::fs::read_to_string(site.join("ko/b.html")).unwrap();
    assert!(
        ko_b.contains("english beta"),
        "untranslated chapter didn't fall back: {ko_b}"
    );
    assert!(
        ko_b.contains("<html lang=\"ko\">"),
        "fallback lost its language"
    );
    let ko_index = std::fs::read_to_string(site.join("ko/index.html")).unwrap();
    assert!(
        ko_index.contains(">알파</a>") && ko_index.contains(">Beta</a>"),
        "index didn't mix translated + fallback titles: {ko_index}"
    );
}

/// A cross-page link that names a section still points at the built page.
///
/// `href` rewrote a URL only when it *ended* with `.md`, so `a11-ui.md#anchor`
/// went out unchanged and pointed at a file the site does not contain. Nothing
/// noticed because until the FFI chapter linked one, no page in the book had
/// written a cross-page anchor at all.
#[test]
fn a_cross_page_link_keeps_its_anchor_and_gains_its_extension() {
    if have_wsl() || !have("cc") {
        eprintln!("skipping tomo anchor check: needs a host cc and no wsl");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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

    let page = std::fs::read_to_string(repo.join("apps/tomo/site/en/a13-ffi.html")).unwrap();

    assert!(
        page.contains("a11-ui.html#assignment-is-the-update"),
        "the anchored cross-link was not rewritten"
    );
    assert!(
        !page.contains("a11-ui.md#"),
        "a `.md` href survived into the built page"
    );
}

/// Every internal link in the built book must resolve.
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
