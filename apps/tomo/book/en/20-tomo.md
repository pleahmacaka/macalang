# Tomo: The Book You Are Reading

This handbook is a directory of Markdown files. The HTML you are reading was
produced by `apps/tomo/tomo.maca` — a static site generator written in Maca. It
is the last chapter because it uses nearly everything in the book, and because a
tool that builds its own documentation is a good final exercise.

*Tomo* is Spanish for a volume of a book, in the same Andean spirit as *maca*.

## What it is

mdBook, roughly — Markdown in, a navigable HTML book out — with one deliberate
difference: **i18n is not a plugin.** It is the data model.

## The configuration

```toml
[book]
title = "The Maca Handbook"
languages = ["en", "ko"]
chapters = [
    "00-introduction",
    "01-getting-started",
]
```

`languages` is a list, and the first entry is the default. Chapters are named
once, and resolved per language under `book/<lang>/`.

## The fallback, which is the point

In mdBook, translating a book means maintaining a parallel book. A chapter that
has not been translated yet is a missing page.

Tomo resolves each chapter per language, and falls back:

```maca
build_chapter(root: str, out: str, title: str, langs: str, lang: str,
              fallback: str, chs: str[], titles: str[], ci: int) -> int {
    ch = chs.get(ci)
    own = root ++ "/book/" ++ lang ++ "/" ++ ch ++ ".md"
    src = file_exists(own)
        ? own
        : root ++ "/book/" ++ fallback ++ "/" ++ ch ++ ".md"
    file_exists(src)
        ? write_chapter(out, title, langs, lang, chs, titles, ci,
                        read_file(src))
        : 0
}
```

A Korean reader who reaches an untranslated chapter gets the English text, on a
Korean page, with the sidebar and navigation intact. The book is never broken;
it is only partly translated. That means a translation can start with one
chapter and be useful immediately, instead of needing to be complete before it
can ship — which is the actual reason most translations never happen.

The index mixes them too: a chapter's title comes from its own `# heading`, in
whichever language was resolved.

## The renderer

The core is a pure function:

```maca
render(md: str) -> str
```

Markdown in, HTML out, no IO. Everything else — reading files, walking the
chapter list, writing the site — wraps it. That separation is what makes it
testable: the gate calls `render` on a sample and asserts on the HTML.

It is a fold over lines, threading two pieces of state:

```maca
render_lines(lines: str[], i: int, in_code: bool, acc: str) -> str =>
    i >= lines.length()
        ? (in_code ? acc ++ "</code></pre>\n" : acc)
        : render_line(lines, i, in_code, acc)
```

Block elements that span lines — paragraphs, blockquotes, lists, tables — find
their own end and consume the whole run:

```maca
render_para(lines: str[], i: int, acc: str) -> str {
    stop = para_end(lines, i)
    text = join_range(lines, i, stop)
    render_lines(lines, stop, false, acc ++ "<p>" ++ inline(esc(text)) ++ "</p>\n")
}
```

This matters more than it sounds. The first version emitted one `<p>` per source
line, which turned a soft-wrapped `**config mode**` into `<strong>config</p>` —
markup split across paragraphs. Blockquotes had the same bug, and a three-line
quote became three blockquotes. Both are now single tests in the gate.

## Search without a server

Every page carries a search box. The index is generated per language, one entry
per heading, holding that section's text lowercased:

```javascript
window.TOMO_INDEX=[{"u":"08-collections.html#lists","c":"Collections",
                    "s":"Lists","x":"a list of t is written…"},…]
```

It ships as a `<script>` the page loads, not as JSON the page fetches — and that
is not laziness. A book opened straight off disk as `file://` cannot `fetch`;
mdBook's search needs a web server. This one works from a folder on a USB stick.

## Building it

```
maca run apps/tomo/tomo.maca
```

The program renders every chapter in every language, writes a per-language index
and search index, and reports how many pages it wrote. The test suite builds the
real handbook and asserts on the result — including that an untranslated chapter
falls back and still comes out as a Korean page.

## What it uses from this book

Records for the configuration. Recursion with an accumulator for every walk over
lines. `str` methods for all the parsing. Lists for the chapter and language
sets. File IO for reading and writing. Raw `"""…"""` strings for the stylesheet
and the search JavaScript, because CSS is full of braces that would otherwise
read as interpolation.

No sum types, as it happens — the renderer dispatches on line prefixes rather
than on a token type. A larger Markdown implementation would want them.

It is about 500 lines. That is the whole static site generator, in the language
it documents.
