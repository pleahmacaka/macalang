# Tomo

**Tomo** is the Maca ecosystem's handbook builder — the `mdBook` analogue,
written in Maca (`tomo.maca`). It turns a tree of Markdown chapters into a static
HTML site. *Tomo* is Spanish for a book's *volume*, in the Andean spirit of
`maca` itself.

The point of difference from mdBook: **i18n is built in, not bolted on.** A book
declares its languages up front; every page carries a language switcher, and a
chapter that hasn't been translated yet falls back to the default language
instead of 404-ing.

## Status

The **renderer** is implemented in Maca and gated
(`crates/driver/tests/tomo.rs`): `render : str -> str` turns Markdown into HTML —

- `#`/`##`/`###` headings,
- `-` list items, including nested (`  - `) items,
- `>` blockquotes,
- ```` ``` ```` fenced code blocks (HTML-escaped),
- paragraphs,
- inline `**bold**`, `` `code` ``, and links `[text](url)`,
- heading anchor ids, plus `toc(...)` which builds a table of contents from a
  document's headings,

and `page(...)` wraps rendered chapters in an HTML shell with the i18n language
switcher. The **book builder** is implemented too: `build_book(root, out)` reads
`book.toml`, walks `book/<lang>/*.md`, and writes `site/<lang>/*.html` — each
page carrying its stylesheet, table of contents, language switcher, and
previous/next chapter links. A chapter missing in
a non-default language **falls back** to the default language's text rather than
404-ing. Running `tomo` from the repo root rebuilds this repository's handbook
(18 pages across `en` and `ko` — 8 chapters plus an index each); the output lands in `apps/tomo/site/`, which is
gitignored.

## Layout

```
apps/tomo/
├── tomo.maca        # the renderer (Markdown -> HTML) + i18n page shell, in Maca
├── book.toml        # book config: title, languages, chapter order
└── book/
    ├── en/          # English chapters (the default language)
    └── ko/          # Korean chapters (i18n)
```

## The handbook

`book/` holds **The Maca Handbook** — a book about the language itself, modelled
on the structure of *The Rust Book* but written for Maca (records and sum types,
colorblind async, config mode, the multi-target backends). It is the primary
content Tomo renders, and doubles as the i18n showcase: every chapter exists in
both `en/` and `ko/`.

## Roadmap

- [x] Markdown renderer in Maca (headings, lists, code, paragraphs, inline)
- [x] inline links `[text](url)`
- [x] heading anchors + a generated table of contents
- [x] i18n page shell with a language switcher
- [x] paragraph joining (soft-wrapped lines become one `<p>`)
- [x] file-I/O book builder — reads `book.toml` + `book/<lang>/*.md`, writes `site/<lang>/*.html`
- [x] `book.toml` parsing (including multi-line arrays) and chapter ordering
- [x] translation-fallback to the default language for missing chapters
- [x] blockquotes (multi-line quotes join) and nested list items
- [ ] tables
- [x] a self-contained stylesheet (light/dark) and chapter-to-chapter navigation
- [x] a per-language index page (chapters titled from their own headings)
- [ ] search
