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
- `-` list items,
- ```` ``` ```` fenced code blocks (HTML-escaped),
- paragraphs,
- inline `**bold**`, `` `code` ``, and links `[text](url)`,
- heading anchor ids, plus `toc(...)` which builds a table of contents from a
  document's headings,

and `page(...)` wraps rendered chapters in an HTML shell with the i18n language
switcher. The file-walking CLI driver (read `book.toml` + `book/<lang>/*.md`,
write `site/<lang>/*.html`) is the next layer — it needs the file-I/O builtins
that are being added to Maca; until then `tomo.maca`'s `main` is a self-check
that renders a sample and prints it.

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
content Tomo renders, and doubles as the i18n showcase: each chapter exists in
`en/` and `ko/`.

## Roadmap

- [x] Markdown renderer in Maca (headings, lists, code, paragraphs, inline)
- [x] inline links `[text](url)`
- [x] heading anchors + a generated table of contents
- [x] i18n page shell with a language switcher
- [ ] file-I/O CLI driver (`maca run tomo.maca -- build`) once the I/O builtins land
- [ ] `book.toml` parsing and chapter ordering
- [ ] translation-fallback to the default language for missing chapters
- [ ] tables, blockquotes, and nested lists
