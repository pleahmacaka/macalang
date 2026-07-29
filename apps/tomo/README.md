# Tomo

**Tomo** is the Maca ecosystem's handbook builder — the `mdBook` analogue,
written in Maca (`tomo.maca`). It turns a tree of Markdown chapters into a static
HTML site. *Tomo* is Spanish for a book's *volume*, in the Andean spirit of
`maca` itself.

The point of difference from mdBook: **i18n is built in, not bolted on.** A book
declares its languages up front; every page carries a language switcher, and a
chapter that hasn't been translated yet falls back to the default language
instead of 404-ing.

## What it does

The **renderer** is Maca, gated by `crates/driver/tests/tomo.rs`:
`render : str -> str` turns Markdown into HTML —

- `#`/`##`/`###` headings,
- `-` list items, including nested (`  - `) items,
- `>` blockquotes,
- ```` ``` ```` fenced code blocks (HTML-escaped),
- paragraphs,
- inline `**bold**`, `` `code` ``, and links `[text](url)`,
- heading anchor ids, plus `toc(...)` which builds a table of contents from a
  document's headings,

and `page(...)` wraps rendered chapters in an HTML shell with the i18n language
switcher.

The **landing page** is `home.md` in each language, rendered by the same
Markdown renderer and wrapped in a header with the language switcher. It is
written to `/index.html` (in the default language) and to `<lang>/home.html`,
so the switcher moves between landings rather than dropping a reader into a
chapter index. A book without a `home.md` gets the language picker instead —
a landing page is something you opt into by writing one. `site` in `book.toml`
names the thing the book is about, which is not the book's own title.

This book is the case that opts out. Its front page is a designed page in the
UI syntax rather than a rendered document (`apps/site/home.maca`), written over
those same three addresses by `tools/build-site.maca`; tomo produces the picker
and the front page replaces it. Markdown is right for a chapter and for most
books' landings, and this is the one where it wasn't.

The **book builder** is `build_book(root, out)`: it reads `book.toml`, walks
`book/<lang>/*.md`, and writes `site/<lang>/*.html` — each page carrying its
stylesheet, table of contents, search index, language switcher, and
previous/next chapter links. A chapter missing in a non-default language
**falls back** to the default language's text rather than 404-ing.

Running `tomo` from the repository root rebuilds this repository's handbook —
27 chapters and appendices in each of `en` and `ko`, plus an index per language
and one above them. The output lands in `apps/tomo/site/`, which is gitignored.

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

## What it renders

Markdown → HTML, all of it written in Maca:

- headings with anchors, and a table of contents generated from them
- paragraphs that join soft-wrapped source lines, and blockquotes that do the
  same across a run of `>` lines
- lists (ordered, unordered, one level of nesting) and tables — the reference
  appendices are mostly tables
- fenced code with its language carried through to a `language-…` class
- inline `**bold**`, `` `code` ``, and `[links](to.md)` rewritten to the page
  that was produced

and, around the text:

- a book builder that reads `book.toml` plus `book/<lang>/*.md` and writes
  `site/<lang>/*.html`, including multi-line arrays and chapter ordering
- an i18n page shell with a language switcher, and per-language index pages
  titled from each chapter's own heading
- translation fallback: a chapter that exists only in the default language is
  served in the reader's language shell rather than 404-ing
- a sidebar with chapter-to-chapter navigation, and search that works from a
  `file://` URL because the index ships as a `<script>` rather than a fetch
- a stylesheet generated from the utility classes the program writes, so the
  built site needs no network and no CSS file

The markup is built with Maca's element syntax and the styles are generated
(handbook chapter 15), so there is exactly one line of hand-written CSS in the
whole program.
