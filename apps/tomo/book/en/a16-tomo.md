# Tomo: The Book You Are Reading

This handbook is a directory of Markdown files. The HTML you are reading was
produced by `apps/tomo/tomo.maca`, a static site generator written in Maca. It
is here because it uses nearly everything in the book at once, and because a
tool that builds its own documentation is the honest test of a language.

*Tomo* is Spanish for a volume of a book, in the same Andean spirit as *maca*.

## What it is

mdBook, roughly (Markdown in, a navigable HTML book out), with one deliberate
difference: **i18n is not a plugin.** It is the data model.

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

`languages` is a list, and the first entry is the default. Chapters are named
once, and resolved per language under `book/<lang>/`.

An entry beginning with `#` is a heading rather than a page: `##` opens a
volume, `#` a section inside it, and the label carries one title per language in
the same order as `languages`. That is how this book is two books, a handbook
and a reference, under one table of contents, one sidebar and one search index.
Headings are labels, so they live in the config and leave the chapter directory
holding only files a reader can open.

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
can ship, which is the actual reason most translations never happen.

The index mixes them too: a chapter's title comes from its own `# heading`, in
whichever language was resolved.

## The renderer

The core is a pure function:

```maca
render(md: str) -> str
```

Markdown in, HTML out, no IO. Everything else (reading files, walking the
chapter list, writing the site) wraps it. That separation is what makes it
testable: the gate calls `render` on a sample and asserts on the HTML.

It is a fold over lines, threading an accumulator:

```maca
render_lines(lines: str[], i: int, acc: str) -> str =>
    i >= lines.length() ? acc : render_line(lines, i, acc)
```

Block elements that span lines (paragraphs, blockquotes, lists, tables, fenced
code) find their own end and consume the whole run:

```maca
render_para(lines: str[], i: int, acc: str) -> str {
    stop = para_end(lines, i)
    text = join_range(lines, i, stop)
    render_lines(lines, stop, acc ++ p(class=md_class("p"), inline(esc(text))) ++ "\n")
}
```

This matters more than it sounds. The first version emitted one `<p>` per source
line, which turned a soft-wrapped `**config mode**` into `<strong>config</p>`,
markup split across paragraphs. Blockquotes had the same bug, and a three-line
quote became three blockquotes. Both are now single tests in the gate.

Notice there is no `in_code` flag. There used to be: fenced code was streamed as
an opening `<pre><code …>` string, then a line at a time, then a closing string.
That meant every step of the fold had to carry "am I inside a fence?", and
the `<pre>` could only ever be written as raw text. Gathering the run first
costs one more pass over it and lets the markup be markup:

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
the config and shell languages get a shallow generic one, and **a tag nothing
highlights falls through to plain escaped text**. That last case is the one that
decides whether the design is any good: an unlabelled block of program output
must come out exactly as it went in, not mangled by rules written for some other
language.

Adding a language is a function and one arm of a dispatch, and the renderer does
not change:

```maca
hl_ini(src: str) -> str => hl_words(src, ";", IniWords)
```

The Maca scanner earns its keep on the cases a regular expression gets wrong,
which are the ones this book is made of: an attached `x?` against a spaced
`c ? a : b`, a format spec `{x:>8}` against a ternary inside an interpolation,
`data-tomo` against `a - b`. It follows the lexer rather than guessing, and the
expression inside a `{…}` is scanned as code, because that is where a good half
of the code in these pages lives.

## Markup, not string concatenation

Every element Tomo emits is written with [the element syntax](a11-ui.md), and
every style is a utility class the compiler turns into a rule.
There is no template and there are two pieces of hand-written CSS: the line that
tells the browser the page has both a light and a dark palette, and the syntax
theme's ten colours. The second is written out because `text-…` takes a *named*
colour (`text-[#cf222e]` is read as a font size), and the named palette holds
one hue that stays legible on a light code block, which is not a syntax theme.

Getting there needed the tag to be a value in three places, because a Markdown
renderer chooses its tags from its input rather than from its source:

```maca
heading(level: str, text: str) -> str =>
    element("h" ++ level, id=slug(text), class=md_class("h" ++ level),
            inline(esc(text))) ++ "\n"

cells(parts: str[], i: int, tag: str) -> str =>   // `th` in the header, `td` below
    element(tag, class=md_class(tag), inline(esc(parts.get(i).trim())))
        ++ cells(parts, i + 1, tag)
```

and once for `<main>`, which no call can name because this program, like every
program, defines `main`.

This is worth doing for a reason beyond neatness. The compiler decides which CSS
rules to generate by collecting the `class=` strings it can see, and it cannot
see inside a raw `"""…"""` string. While the search box was one raw block, every
class named in it produced no rule at all, and the search field went unstyled
without anything failing.

## Search without a server

Every page carries a search box. The index is generated per language, one entry
per heading, holding that section's text lowercased:

```javascript
window.TOMO_INDEX=[{"u":"08-collections.html#lists","c":"Collections",
                    "s":"Lists","x":"a list of t is written…"},…]
```

It ships as a `<script>` the page loads, not as JSON the page fetches, and that
is not laziness. A book opened straight off disk as `file://` cannot `fetch`;
mdBook's search needs a web server. This one works from a folder on a USB stick.

## The landing page

A book explains a thing. Someone arriving cold needs to know what the thing
*is* before a table of contents helps them, so `home.md`, one per language,
is rendered by the same renderer and becomes `/index.html` and
`<lang>/home.html`.

Two details are worth copying. The three buttons live in the layout rather than
in the Markdown, because the root landing and a language's landing are the same
Markdown at different depths, and a site-relative link written in that Markdown
could only be right in one of them. And the page splits at its first `##`: what
is above is the pitch, the buttons go after it, and the argument follows, so
the reader gets a way out before two screens of prose, not after.

A book with no `home.md` still gets a root page: the language picker this used
to be. A landing page is something you opt into by writing one.

## Two rules the book holds itself to

**Order lives in one place.** `<lang>/index.html` used to render the whole
contents list into the page while the sidebar rendered the same list beside it,
so the reader met two tables of contents and had to work out whether they
differed. The sidebar carries the order; the page it opens on says what each
volume is for and where it starts. `sitegen.maca` counts the chapter links on
that page and fails at anything but one per volume.

**A chapter opens with a sentence, not a subheading.** Landing on `## Functions
are values` tells you what the section is about, but not what the chapter is
for or whether you are in the right one. The first line under the title says
what the page answers. That is also checked, over both languages, because a
convention nothing enforces is one that holds until the next contributor.

## Building it

```
maca run apps/tomo/tomo.maca
```

The program renders every chapter in every language, writes a per-language index
and search index, and reports how many pages it wrote. The test suite builds the
real handbook and asserts on the result, including that an untranslated chapter
falls back and still comes out as a Korean page.

## What it uses from this book

Recursion with an accumulator for every walk over lines. `str` methods for all
the parsing. Lists for the chapter and language sets. File IO for reading and
writing. The element syntax and generated styles for every byte of output. Raw
`"""…"""` strings for exactly one thing, the JavaScript that drives search and
collapses the sidebar, because that is what a raw block is for: a foreign
language, not markup in disguise.

No sum types, as it happens: the renderer dispatches on line prefixes rather
than on a token type. A larger Markdown implementation would want them.

It is about a thousand lines. That is the whole static site generator, in the
language it documents.
