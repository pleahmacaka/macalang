# The UI Syntax

Every form the element syntax has, and every rule the stylesheet generator
follows. Both are part of the language rather than a library, and both work on
more than one target: the same markup becomes a live DOM when you build for the
browser and a string of HTML when you build a native binary.

The introduction is [Interfaces and Documents](15-ui.md). The page you are
reading was produced by what is described here.

## An element is a call

Any HTML tag name can be called:

```maca
article(class="prose",
    h1("Hello")
    span("Some text"))
```

Named arguments are attributes; positional arguments are children. Children are
separated by commas or by nothing at all. The second is what makes nested
markup readable, and it is why the example above needs no punctuation between
the heading and the paragraph.

There is no closing tag to forget, no template language, and no separate file.
An element is an ordinary expression, so it composes with everything else: you
can build one in a function, put one in a list, return one from a `match`.

```maca
item(name: str) -> str => li(name)

list_of(names: str[]) -> str =>
    ul(names.map(item).join(""))
```

## Two targets, one syntax

```
maca build app.maca --target js -o out
```

builds a reactive page. Elements become `createElement` calls, `onclick=…`
attaches a handler, and `value=…` on a state name binds it two ways. The
playground that ships with this book is one `.maca` file compiled exactly this
way.

```
maca build gen.maca -o gen
```

builds a native binary, and there the same elements render to **text**:

```maca
main() -> int {
    info(article(class="prose", h1("Hi") span("Body")))
    0
}
```

```
<article class="prose"><h1>Hi</h1><span>Body</span></article>
```

That is what a static site generator needs. An event handler has nowhere to
attach in a string, so the older `on:click=` directive on the native target is a
compile error that tells you to build for `js`, rather than markup that silently
does nothing. A vanilla `onclick="…"` is an ordinary HTML attribute there, which
is what HTML makes of it too, and it renders. A *function* given to `onclick=`
on that target does not reach a page either, but the complaint comes from the C
compiler rather than from Maca, and that is the one place the two spellings
differ.

Attribute values are escaped. Children are **not**, because a child is either
another element (already markup) or text the program chose to put there. A
generator that has escaped its own code block cannot have the renderer escape it
a second time.

## Events, and the two-way name

An attribute whose name is `on` followed by lowercase letters is an event
handler, and the letters are the event: `onclick`, `oninput`, `onchange`,
`ondragstart`, `ondragover`, `ondragend`, `ondrop`, and any other the platform
grows. There is no list to be added to, because the rule is the spelling.

```maca
li(draggable=true, ondragstart=grab, ondragover=over, ondrop=drop_here, name)
```

The value is a function: a top-level definition by name, or a lambda that takes
the event.

```maca
button(onclick=(e => count = count + 1), "+")
```

`value=` is the one attribute that reads in both directions. Given a name the
program declared, the property follows the state and typing writes it back:

```maca
who = "world"

main() -> Element => div(input(value=who) span("Hello, {who}"))
```

Given anything else, including a constant, it is an ordinary attribute:
`input(value="literal")` sets the attribute and listens for nothing. When the
value to store is not the text typed, a lambda says what to store:

```maca
input(value=(v => age = int(v)))
```

The older directive spellings, `on:click=` and `bind:value=`, still parse and
mean the same thing. `bind:` is also the only way to two-way bind a property
other than `value`.

## Assignment is the update

A handler does not ask for a repaint. Writing a declared state name *is* the
request:

```maca
count = 0
note = "idle"

go() {
    count = count + 1
    note = "counted"
}

main() -> Element =>
    div(button(onclick=go, "go") span("{count}") span(note))
```

Three rules hold that up, and each is worth knowing when a page surprises you.

**Only what reads the name runs again.** Every bound node records the state
names its expression mentions, so assigning `count` re-runs the node reading
`count` and leaves the one reading `note` alone. A node whose value comes from
a *call* (`span(shown(count))`) records no names, because a function body is out
of reach, so it re-runs on any change at all.

**A handler is one turn.** Everything assigned between the event arriving and
the handler returning is collected, and the view is repainted once, at the end.
That is also the answer for a loop: a hundred assignments inside one handler are
one repaint, after the loop.

**A write that changes nothing is not an update.** Assigning the value a name
already holds marks nothing dirty and repaints nothing.

`update()` remains, and so does `maca.refresh()`, for the case the rule cannot
see: something outside Maca moved, and a node that reads it has to be told. A
view that *assigns* state is a different matter: it would repaint itself
forever, so it stops and says so by name rather than hanging the tab.

## A definition wins over a tag

`label`, `code`, `main`, `section`, `p`, `a`, `form` and `option` are HTML tags
*and* names people give their own functions and variables. When a name is
defined, the definition wins:

```maca
label(pos: bool) -> str => pos ? "right" : "left"

main() -> int {
    info(label(true))     // "right": your function, not <label>
    info(span("tag"))     // "<span>tag</span>": nothing shadows `span`
    0
}
```

This is not a special case for a list of names; it is the ordinary scoping rule,
applied before the tag is considered. A local binding shadows a tag the same way
a function does.

## Hyphens, and the rule that makes them work

Documents are full of `data-*`, `aria-*`, `http-equiv`, `accept-charset`. You
write them with the hyphen they have in HTML:

```maca
nav(data-tomo="toc", aria-label="Contents", body)
```

```
<nav data-tomo="toc" aria-label="Contents">…</nav>
```

There is no rewriting here and no workaround, because an **attached** `-` is
part of an identifier while a **spaced** one is the subtraction operator. Both
readings live in the same argument list without ambiguity:

```maca
div(data-kind="note", span("{a - b}"))
```

You have met this rule twice already: `x?` propagates a failure while `c ? x : y`
is a ternary, and `{n:>8}` is a format spec while `{c ? a : b}` is a ternary
inside a string. Attached and spaced mean different things, deliberately, and
whitespace is how you choose.

## Two more an identifier alone cannot express

**Booleans.** HTML reads *any* attribute value as true: `hidden="false"` still
hides the element. So a bool controls whether the attribute exists at all:

```maca
details(open=true, summary("more") "text")   // <details open>…
div(hidden=false, "seen")                    // <div>seen</div>
div(hidden=n > 5, "computed")                // decided at run time
```

**Tags chosen at run time.** A document generator picks its tags from its input:
a heading's depth chooses `h1`…`h6`, a table row chooses `th` or `td`. `element`
takes the tag as a value:

```maca
heading(level: int, text: str) -> str =>
    element("h" ++ level, id=slug(text), text)
```

It also reaches `<main>`, which no call can name, because every program defines
`main` and the definition wins.

## Children that are lists

A positional argument may be an `Element[]` rather than an `Element`, and then
each element of the list is a child in its place. `[]` contributes nothing at
all: no node on `js`, no markup natively.

| Form | Contributes |
|---|---|
| `[a, b]` | two children, in order |
| `xs.map(f)` | one child per element |
| `a ++ b` | the children of both lists |
| `[]` | nothing |
| a call declared `-> Element[]` | whatever that view returned |
| a call declared `-> Element` | that one element |

`Element` is the type of a rendered element: `str` natively, a DOM node on
`js`. The declaration is what the compiler reads, so a view that hands back
nodes says so:

```maca
toolbar(locked: bool) -> Element[] {
    if locked {
        return []
    }

    [div(class="toolbar", button("edit"))]
}
```

A function returning `str` is unaffected: it is still a child rendered as text.
This is what replaces a `class="hidden"` ternary; the node is not built rather
than built and hidden.

## Styles are generated, not linked

Classes are written in `class=` using Tailwind's utility names, and the compiler
generates the stylesheet for the utilities your program actually mentions:

```maca
page() -> str =>
    div(class="max-w-2xl mx-auto font-bold", "text")
```

`styles()` returns that stylesheet as a string:

```maca
head(
    meta(charset="utf-8")
    style(styles()))
```

```css
*,*::before,*::after{box-sizing:border-box}
html,body{margin:0}
.font-bold { font-weight:700; }
.max-w-2xl { max-width:42rem; }
.mx-auto { margin-left:auto;margin-right:auto; }
```

Two lines of reset, then one rule per utility written. A utility the program
never mentions produces no rule. There is no framework to tree-shake, because
nothing was included in the first place. There is also no network fetch, which
is why a book built this way opens correctly straight off a disk.

Classes are collected from anywhere in the module, not only from `class=` at the
point of use, so factoring them into a function works:

```maca
button_class() -> str =>
    "font-bold hover:bg-zinc-100 dark:bg-zinc-800 md:px-4"
```

The one place they are *not* collected is inside a raw `"""…"""` string. Markup
written as a raw block is invisible to the collector, and its classes name rules
that never get generated.

## Variants

A prefix before the utility narrows when it applies. State variants add a
selector suffix:

| variant | selector |
|---|---|
| `hover:` `focus:` `active:` | `:hover` `:focus` `:active` |
| `first:` `last:` | `:first-child` `:last-child` |
| `open:` | `[open]`, an open `<details>` |
| `before:` `after:` `marker:` | the matching pseudo-element |
| `placeholder:` | `::placeholder` |
| `details-marker:` | `::-webkit-details-marker` |

Condition variants wrap the rule in a media query:

| variant | query |
|---|---|
| `dark:` | `prefers-color-scheme: dark` |
| `sm:` `md:` `lg:` `xl:` | min-width 40 / 48 / 64 / 80rem |
| `max-sm:` `max-md:` `max-lg:` | max-width 40 / 48 / 64rem |

They combine, in any order and any number:

```maca
a(class="text-zinc-500 hover:text-black dark:hover:text-white max-md:hidden",
  href="x.html", "link")
```

Generated rules are ordered so that a variant beats the plain utility it
modifies. CSS breaks ties by source order, so `max-md:block` losing to `grid`
would be a real bug rather than a cosmetic one, and the ordering is part of the
generator rather than something you arrange by hand.

## Arbitrary values

When the scale does not have what you need, brackets take a literal value:

```maca
div(class="max-w-[42rem] text-[0.88em] mt-[3px]", body)
```

Underscores inside the brackets become spaces, since a class attribute cannot
contain one:

```maca
div(class="grid-cols-[1fr_18rem]", body)
```

The generated selector is escaped, which matters more than it looks: `.max-w-[42rem]`
is not a valid CSS selector, and a browser that meets one **drops the rule
silently**, with no console warning and no visible failure except that the style
is missing.

## Putting it together

A page, complete, with no other files:

```maca
main() -> int {
    write_file("index.html",
        "<!doctype html>\n"
        ++ html(lang="en",
            head(
                meta(charset="utf-8")
                meta(name="viewport", content="width=device-width,initial-scale=1")
                title("Notes")
                style(styles()))
            body(class="font-serif bg-white dark:bg-zinc-900",
                element("main",
                    h1(class="text-[2rem] font-bold", "Notes")
                    span(class="my-4", "Written in Maca.")))))
    0
}
```

`maca run` it and you have a styled, self-contained, dark-mode-aware page.
[Tomo](a16-tomo.md), the generator that built this book, is the same idea with a
Markdown parser in front of it.

## What each target does with an element

| Target | An element becomes |
|---|---|
| native (C) | a `maca_concat` chain producing an HTML string; `maca_attr` escapes attribute values, children are not re-escaped, void elements self-close |
| `js` | `createElement` calls and a reactive DOM; `onclick=` attaches a handler, `value=` on a state name binds two ways, and an assignment to that name repaints what reads it |
| `element(tag, …)` | the same on both, with voidness decided at run time in `maca_element` |
| `open=true` | `maca_flag`: the attribute is present or absent, never `="false"` |

The `on:click=` directive on the native target is a compile error naming
`--target js`, and that is the only place the two targets diverge in what they
accept. See [Targets](a10-targets.md).

## A page's assets, and naming a package rather than a path

After `import <lang>`, a raw `"""…"""` block *is* the source and a quoted
`"…"` *names a file*, read while the page is built and inlined into it. So a
page carries its vendor stylesheet and its vendor script without linking to
anything:

```maca
import css "vendor/reset.css"
import js "vendor/iconify-icon.js"
```

A path there is resolved against the file that wrote it, and a path that
resolves to no file is a build error naming it.

A project that ran `maca add` should not have to reach into the directory the
installer chose. `npm:` is the prefix `maca.toml` already writes for a
dependency, and it means the same thing in an asset import:

```maca
import css "npm:daisyui"
import js "npm:iconify-icon"
```

**The package names its own entry point.** Its `package.json` is read, and the
first of `style`, `browser`, `module`, `main` that names a file of the right
kind is the one that lands: `.css` for a stylesheet, `.js`, `.mjs` or `.cjs`
for a script, `.wasm` for WebAssembly. A package that states several of them is
not ambiguous, because the list is ordered and a page is a browser, so
`browser` outranks `module` and `module` outranks `main`.

Three things are errors rather than a quiet nothing:

| What | The build says |
|---|---|
| the package is not installed | `` `daisyui` is not installed; run `maca add npm:daisyui` `` |
| it states no entry of that kind | `` `iconify-icon` states no stylesheet entry point `` |
| the entry names a file that is not there | `` `daisyui` states style = "dist/full.css", which is not there `` |

When the entry point is not the file you want, name the file:

```maca
import css "npm:daisyui/dist/themes.css"
```

A scoped package is reached under the bare name `maca add` installed it as, so
`npm:@wooorm/starry-night` and `npm:starry-night` are the same directory. The
walk that finds `maca_modules` is the one an ordinary `import` takes, from the
importing file up to the workspace root, so a page in a subdirectory finds the
package installed at the project root. See [Modules and Layout](a9-modules.md).
