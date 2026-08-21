# The UI Syntax

Every form the element syntax has, and every rule the stylesheet generator
follows. The introduction is [Interfaces and Documents](15-ui.md).

## An element is a call

```maca
article(class="prose",
    h1("Hello")
    span("Some text"))
```

Named arguments are attributes; positional arguments are children. Children are
separated by commas or by nothing at all.

An element is an ordinary expression:

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
attaches a handler, and `value=…` on a state name binds it two ways.

```
maca build gen.maca -o gen
```

builds through C, and there the same elements render to **text**:

```maca
main() -> int {
    info(article(class="prose", h1("Hi") span("Body")))
    0
}
```

```
<article class="prose"><h1>Hi</h1><span>Body</span></article>
```

An event handler has nowhere to attach in a string, so `on:click=` on the native
target is a compile error naming `--target js`. A vanilla `onclick="…"` is an
ordinary HTML attribute there and renders.

## Events, and the two-way name

An attribute whose name is `on` followed by lowercase letters is an event
handler, and the letters are the event: `onclick`, `oninput`, `onchange`,
`ondragstart`, `ondrop`, and any other the platform grows.

```maca
li(draggable=true, ondragstart=grab, ondragover=over, ondrop=drop_here, name)
```

The value is a function: a name, or a lambda taking the event.

```maca
button(onclick=(e => count = count + 1), "+")
```

`value=` reads in both directions. Given a name the program declared, the
property follows the state and typing writes it back:

```maca
who = "world"

main() -> Element => div(input(value=who) span("Hello, {who}"))
```

Given anything else it is an ordinary attribute. When the value to store is not
the text typed, a lambda says what to store:

```maca
input(value=(v => age = int(v)))
```

`on:click=` and `bind:value=` still parse and mean the same thing. `bind:` is
the only way to two-way bind a property other than `value`.

## Assignment is the update

Writing a declared state name *is* the repaint request:

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

**Only what reads the name runs again.** Every bound node records the state
names its expression mentions. A node whose value comes from a *call*
(`span(shown(count))`) records no names, so it re-runs on any change.

**A handler is one turn.** Everything assigned before the handler returns is
collected, and the view repaints once, at the end.

**A write that changes nothing is not an update.**

A child that is a call to a view (`toolbar()`, `dialog()`) is a whole subtree.
It is anchored where it was written and **rebuilt** when the state it reads
changes, its old nodes and bindings dropped together. Its locals are made afresh
with it, which is why a rebuild watches the program's state and not the view's
own.

`update()` and `maca.refresh()` remain, for when something outside Maca moved. A
view that *assigns* state would repaint forever, so it stops and says so by
name.

## A definition wins over a tag

`label`, `code`, `main`, `section`, `p`, `a`, `form` and `option` are HTML tags
*and* names people give their own functions. The ordinary scoping rule applies
before the tag is considered, so the definition wins:

```maca
label(pos: bool) -> str => pos ? "right" : "left"

main() -> int {
    info(label(true))     // "right": your function, not <label>
    info(span("tag"))     // "<span>tag</span>": nothing shadows `span`
    0
}
```

## Hyphens, and the rule that makes them work

Write `data-*`, `aria-*` and `http-equiv` with the hyphen they have in HTML:

```maca
nav(data-tomo="toc", aria-label="Contents", body)
```

```
<nav data-tomo="toc" aria-label="Contents">…</nav>
```

An **attached** `-` is part of an identifier; a **spaced** one subtracts. Both
live in one argument list:

```maca
div(data-kind="note", span("{a - b}"))
```

The same hyphen names a **custom element**, which is the platform's own rule:

```maca
iconify-icon(class="text-2xl", icon="lucide:lock")
```

```
<iconify-icon class="text-2xl" icon="lucide:lock"></iconify-icon>
```

The JS backend builds the node and lets the browser upgrade it; the native
backend writes it closed rather than self-closed, because no custom element is
void.

## Two more an identifier alone cannot express

**Booleans.** HTML reads *any* attribute value as true: `hidden="false"` still
hides the element. So a bool controls whether the attribute exists at all:

```maca
details(open=true, summary("more") "text")   // <details open>…
div(hidden=false, "seen")                    // <div>seen</div>
div(hidden=n > 5, "computed")                // decided at run time
```

**Tags chosen at run time.** `element` takes the tag as a value, which is also
how you reach `<main>`:

```maca
heading(level: int, text: str) -> str =>
    element("h" ++ level, id=slug(text), text)
```

## Children that are lists

A positional argument may be an `Element[]`, and then each element of the list is
a child in its place. `[]` contributes nothing: no node on `js`, no markup
natively, which replaces a `class="hidden"` ternary.

| Form | Contributes |
|---|---|
| `[a, b]` | two children, in order |
| `xs.map(f)` | one child per element |
| `a ++ b` | the children of both lists |
| `[]` | nothing |
| a call declared `-> Element[]` | whatever that view returned |
| a call declared `-> Element` | that one element |

`Element` is `str` natively and a DOM node on `js`. The declaration is what the
compiler reads, so a view that hands back nodes says so:

```maca
toolbar(locked: bool) -> Element[] {
    if locked {
        return []
    }

    [div(class="toolbar", button("edit"))]
}
```

## Styles are generated, not linked

Classes use Tailwind's utility names, and the compiler generates the stylesheet
for the utilities your program mentions:

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

Two lines of reset, then one rule per utility written, and no network fetch.
Classes are collected from anywhere in the module, so factoring them into a
function works:

```maca
button_class() -> str =>
    "font-bold hover:bg-zinc-100 dark:bg-zinc-800 md:px-4"
```

The one place they are *not* collected is inside a raw `"""…"""` string.

## Variants

A prefix narrows when the utility applies. State variants add a selector
suffix:

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

The generated selector is escaped: `.max-w-[42rem]` is not a valid CSS selector,
and a browser that meets one **drops the rule silently**.

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

[Tomo](a16-tomo.md), the generator that built this book, is the same idea with a
Markdown parser in front of it.

## What each target does with an element

| Target | An element becomes |
|---|---|
| native (C) | a `maca_concat` chain producing an HTML string; `maca_attr` escapes attribute values, children are not re-escaped, void elements self-close |
| `js` | `createElement` calls and a reactive DOM; `onclick=` attaches a handler, `value=` on a state name binds two ways, and an assignment to that name repaints what reads it |
| `element(tag, …)` | the same on both, with voidness decided at run time in `maca_element` |
| `open=true` | `maca_flag`: the attribute is present or absent, never `="false"` |

`on:click=` on the native target is a compile error naming `--target js`, the
only place the two targets diverge. See [Targets](a10-targets.md).

## A page's assets, and naming a package rather than a path

A raw `"""…"""` block *is* the source and says which language it is. A quoted
`"…"` *names a file*, read while the page is built and inlined, and takes no
language word because its extension says what it is:

```maca
import "vendor/reset.css"
import "vendor/iconify-icon.js"
```

A path resolves against the file that wrote it, and one that resolves to no file
is a build error naming it. `npm:` means in an asset import what it means in
`maca.toml`:

```maca
import "npm:daisyui"
import "npm:iconify-icon"
```

**The package names its own entry point.** Its `package.json` is read, and the
first of `style`, `browser`, `module`, `main` naming a file of the right kind
lands: `.css` for a stylesheet, `.js`/`.mjs`/`.cjs` for a script, `.wasm` for
WebAssembly.

Three things are errors rather than a quiet nothing:

| What | The build says |
|---|---|
| the package is not installed | `` `daisyui` is not installed; run `maca add npm:daisyui` `` |
| it states no entry of that kind | `` `iconify-icon` states no stylesheet entry point `` |
| the entry names a file that is not there | `` `daisyui` states style = "dist/full.css", which is not there `` |

When the entry point is not the file you want, name the file:

```maca
import "npm:daisyui/dist/themes.css"
```

A scoped package is reached under the bare name `maca add` installed it as, so
`npm:@wooorm/starry-night` and `npm:starry-night` are the same directory. The
walk that finds `maca_modules` is the one an ordinary `import` takes. See
[Modules and Layout](a9-modules.md).
