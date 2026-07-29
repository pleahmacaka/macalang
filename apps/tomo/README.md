# Tomo

**Tomo** is the Maca ecosystem's handbook builder — the `mdBook` analogue,
written in Maca (`tomo.maca`). It turns a tree of Markdown chapters into a static
HTML site. *Tomo* is Spanish for a book's *volume*, in the Andean spirit of
`maca` itself.

Two points of difference from mdBook. **i18n is built in, not bolted on**: a
book declares its languages up front, every page carries a language switcher,
and a chapter that hasn't been translated yet falls back to the default language
instead of 404-ing. And a chapter list can hold **more than one volume**: a `##`
entry opens a book and a `#` entry a section inside it, so a handbook and a
reference share one table of contents, one sidebar and one search index.

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
34 chapters in each of `en` and `ko`, plus an index per language and one above
them. The output lands in `apps/tomo/site/`, which is gitignored.

## Layout

```
apps/tomo/
├── tomo.maca        # the renderer (Markdown -> HTML) + i18n page shell, in Maca
├── book.toml        # book config: title, languages, volumes, chapter order
└── book/
    ├── en/          # English chapters (the default language)
    └── ko/          # Korean chapters (i18n)
```

## The handbook

`book/` holds **The Maca Handbook**, which is two books over one chapter list.

**Learning Maca** (`00-`…`18-`) teaches: it moves in the order a learner needs,
every chapter ends with something runnable, and it is meant to be finished in a
sitting or two. **The Reference** (`a1-`…`a16-`) answers: the exact rule, the
exact syntax, the exact diagnostic. A teaching chapter with a stricter twin
links to it by name, and the reference links back.

The split is entirely in `book.toml` — the `##` volume entries and the order of
the chapter list. Nothing in the renderer knows which book a page belongs to
except the sidebar heading it sits under and the caption on its search-index
entries.

It is also the i18n showcase: every chapter exists in both `en/` and `ko/`, with
the same headings in the same order.

## What it renders

Markdown → HTML, all of it written in Maca:

- headings with anchors, and a table of contents generated from them
- paragraphs that join soft-wrapped source lines, and blockquotes that do the
  same across a run of `>` lines
- lists (ordered, unordered, one level of nesting) and tables — the reference
  appendices are mostly tables
- fenced code, syntax-highlighted by the language its fence names — Maca by a
  scanner that follows the real lexer, the config and shell languages by a
  generic one, and an unknown tag by nothing at all, which is the case that
  keeps a block of program output intact
- inline `**bold**`, `` `code` ``, and `[links](to.md)` rewritten to the page
  that was produced

and, around the text:

- a book builder that reads `book.toml` plus `book/<lang>/*.md` and writes
  `site/<lang>/*.html`, including multi-line arrays and chapter ordering
- an i18n page shell with a language switcher, and per-language index pages
  titled from each chapter's own heading
- translation fallback: a chapter that exists only in the default language is
  served in the reader's language shell rather than 404-ing
- a sidebar with chapter-to-chapter navigation grouped by volume and section,
  and search that works from a `file://` URL because the index ships as a
  `<script>` rather than a fetch — each hit captioned with the volume it came
  from, since two volumes may answer the same question differently
- a stylesheet generated from the utility classes the program writes, so the
  built site needs no network and no CSS file

The markup is built with Maca's element syntax and the styles are generated
(handbook chapter 15). Two pieces of CSS are written by hand: the line that
tells the browser the page has a light and a dark palette, and the syntax
theme's colours in `highlight.maca` — `text-…` accepts a *named* colour only
(`text-[#cf222e]` is read as a font size), and the named palette holds one hue
that clears 4.5:1 on a light code block.

## Layout of the program

```
apps/tomo/
├── tomo.maca        # Markdown -> HTML, the i18n page shell, the book builder
├── highlight.maca   # the syntax highlighter and its theme
├── conf.maca        # the `book.toml` subset, shared with tools/build-site.maca
└── tests/
    └── highlight.maca   # `maca test apps/tomo/tests/highlight.maca`
```
