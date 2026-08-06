# Tomo: The Book You Are Reading

The HTML you are reading was produced by `apps/tomo/tomo.maca`, a static site
generator written in Maca. *Tomo* is Spanish for a volume of a book.

## What it is

mdBook, roughly (Markdown in, a navigable HTML book out), with one difference:
**i18n is not a plugin.** It is the data model.

## The configuration

```toml
[book]
title = "The Maca Handbook"
languages = ["en", "ko"]
chapters = [
    "## Learning Maca|Maca 배우기",
    "# Getting Started|시작하기",
    "00-introduction",
    "01-installing",
]
```

`languages` is a list, first entry the default. Chapters are named once, and
resolved per language under `book/<lang>/`.

An entry beginning with `#` is a heading rather than a page: `##` opens a
volume, `#` a section inside it, and the label carries one title per language in
the order `languages` gives.

## The fallback, which is the point

In mdBook an untranslated chapter is a missing page. Tomo resolves each chapter
per language, and falls back:

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

A Korean reader who reaches an untranslated chapter gets the English text on a
Korean page, sidebar and navigation intact. The index mixes them too: a
chapter's title comes from its own `# heading`, in whichever language was
resolved.

## The renderer

The core is a pure function:

```maca
render(md: str) -> str
```

Markdown in, HTML out, no IO, so the gate can call `render` on a sample and
assert on the HTML. It is a fold over lines, threading an accumulator:

```maca
render_lines(lines: str[], i: int, acc: str) -> str =>
    i >= lines.length() ? acc : render_line(lines, i, acc)
```

Block elements that span lines (paragraphs, blockquotes, lists, tables, fenced
code) find their own end and consume the whole run, which is why a soft-wrapped
`**config mode**` does not become `<strong>config</p>`:

```maca
render_para(lines: str[], i: int, acc: str) -> str {
    stop = para_end(lines, i)
    text = join_range(lines, i, stop)
    render_lines(lines, stop, acc ++ p(class=md_class("p"), inline(esc(text))) ++ "\n")
}
```

Fenced code is gathered first rather than streamed, so there is no "am I inside
a fence?" flag to carry:

```maca
render_fence(lines: str[], i: int, acc: str, fence: str) -> str {
    stop = fence_end(lines, i + 1)
    lang = fence.substr(3, fence.length() - 3).trim()
    html = pre(class=md_class("pre"),
               code(class=pre_code_class() ++ lang_class(lang),
                    highlight(lang, fence_body(lines, i + 1, stop))))
    render_lines(lines, stop + 1, acc ++ html ++ "\n")
}
```

## Highlighting is a hook, not a special case

`render_fence` calls one function it knows nothing about:

```maca
highlight(lang: str, src: str) -> str
```

The fence's tag goes in, escaped HTML comes out. `apps/tomo/highlight.maca`
answers it: a fence tagged `maca` gets a scanner that follows `crates/lexer`,
config and shell get a shallow generic one, and **a tag nothing highlights falls
through to plain escaped text**. Adding a language is a function and one arm of a
dispatch:

```maca
hl_ini(src: str) -> str => hl_words(src, ";", IniWords)
```

The Maca scanner earns its keep on what a regular expression gets wrong: an
attached `x?` against a spaced `c ? a : b`, `{x:>8}` against a ternary inside an
interpolation, `data-tomo` against `a - b`.

## Markup, not string concatenation

Every element Tomo emits is written with [the element syntax](a11-ui.md), and
every style is a utility class the compiler turns into a rule. There is no
template and two pieces of hand-written CSS: the light/dark palette line, and the
syntax theme's ten colours, which are written out because `text-…` takes a
*named* colour.

The tag has to be a value in three places, because a Markdown renderer chooses
its tags from its input:

```maca
heading(level: str, text: str) -> str =>
    element("h" ++ level, id=slug(text), class=md_class("h" ++ level),
            inline(esc(text))) ++ "\n"

cells(parts: str[], i: int, tag: str) -> str =>   // `th` in the header, `td` below
    element(tag, class=md_class(tag), inline(esc(parts.get(i).trim())))
        ++ cells(parts, i + 1, tag)
```

and once for `<main>`, which no call can name because this program defines
`main`. The class collector cannot see inside a raw `"""…"""` string, so while
the search box was one raw block, every class named in it produced no rule.

## Search without a server

The index is generated per language, one entry per heading, holding that
section's text lowercased:

```javascript
window.TOMO_INDEX=[{"u":"08-collections.html#lists","c":"Collections",
                    "s":"Lists","x":"a list of t is written…"},…]
```

It ships as a `<script>` the page loads, not as JSON the page fetches, because a
book opened as `file://` cannot `fetch`.

## The landing page

`home.md`, one per language, is rendered by the same renderer and becomes
`/index.html` and `<lang>/home.html`. The three buttons live in the layout rather
than in the Markdown, because the root landing and a language's landing are the
same Markdown at different depths. The page splits at its first `##`: what is
above is the pitch, the buttons follow, then the argument.

## Two rules the book holds itself to

**Order lives in one place.** The sidebar carries the order; `sitegen.maca`
counts the chapter links on the page it opens on and fails at anything but one
per volume.

**A chapter opens with a sentence, not a subheading.** Also checked, over both
languages.

## Building it

```
maca run apps/tomo/tomo.maca
```

It renders every chapter in every language, writes a per-language index and
search index, and reports how many pages it wrote. The test suite builds the
real handbook and asserts on the result, including the fallback.

## What it uses from this book

Recursion with an accumulator for every walk over lines. `str` methods for the
parsing. Lists for the chapter and language sets. File IO. The element syntax
and generated styles for every byte of output. Raw `"""…"""` strings for one
thing, the JavaScript that drives search and collapses the sidebar.

No sum types: the renderer dispatches on line prefixes rather than a token type.
It is about a thousand lines.
