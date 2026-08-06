# The Standard Library

Most of the standard library is compiler and runtime builtins rather than Maca
source, so it is available on every target. The output names come from syslog
levels; `warn` and below go to stderr.

## Output

| Function | Does |
|---|---|
| `print(s)` | write to stdout, no newline |
| `info(s)` | write a line to stdout |
| `err(s)` | write a line to stderr |

## Conversion

| Function | Does |
|---|---|
| `str(x)` | any value to its text |
| `int(s)` | text to integer |
| `float(s)` | text to float |
| `len(x)` | length of a list or string |

## Strings

| Method | Result |
|---|---|
| `length()` | byte length |
| `split(sep)` | `str[]` |
| `trim()` | both ends stripped |
| `upper()` `lower()` | case |
| `contains(s)` | `bool` |
| `starts_with(s)` `ends_with(s)` | `bool` |
| `replace(from, to)` | every occurrence |
| `substr(start, len)` | a **length**, not an end |
| `slice(from, to)` | `to` is **exclusive**, as on a list |
| `index_of(s)` | index or `-1` |
| `repeat(n)` | `str` |
| `pad_start(w, p)` `pad_end(w, p)` `pad_center(w, p)` | `p` defaults to a space |
| `chars()` | `str[]` of one-character strings |
| `at(i)` | the character at `i` |
| `is_whitespace()` `is_ascii_digit()` `is_alpha()` | character classes |
| `fixed(n)` | a number as text with `n` decimals |

## Lists

| Method | Result |
|---|---|
| `map(f)` `filter(f)` | `T[]` |
| `reduce(init, f)` `fold(init, f)` | a single value |
| `sort()` `reverse()` | `T[]` |
| `sort_by(key)` | `T[]`, ordered by `key(x)`, stable |
| `push(x)` `pop()` | a new list |
| `set(i, x)` `insert(i, x)` `remove(i)` | a new list, edited at `i` |
| `slice(from, to)` | `to` is **exclusive** |
| `contains(x)` `index_of(x)` | search |
| `index_of_by(f)` | the first index where `f(x)`, else `-1` |
| `enumerate()` | `{index, value}[]` |
| `sum()` `min()` `max()` | numeric |
| `first()` `last()` `get(i)` | elements |
| `length()` | `int` |
| `join(sep)` | a `str[]` into one `str` |
| `parallel(f)` | like `map`, evaluated concurrently |

## Maps

`Map str V` is a string-keyed hash map, monomorphized on its value type.

| Method | Result |
|---|---|
| `set(k, v)` | the map, with `k` bound to `v` |
| `get(k, default)` | the value, or `default` |
| `has(k)` | `bool` |
| `remove(k)` | the map, without `k` |
| `keys()` | `str[]`, sorted |
| `length()` | `int` |

```maca
counts: Map str int = map()
counts = counts.set("apple", 3).set("pear", 1)
info("{counts.get("apple", 0)}")     // 3
info("{counts.get("kiwi", 0)}")      // 0, a miss gives the default
```

Keys are `str` only; an integer key is `str(n)` away. `keys()` comes back
sorted. `get` takes a default because the language has no null to return.

## Math

| Function | Does |
|---|---|
| `sqrt(x)` `floor(x)` `ceil(x)` | on `float` |

## Files and directories

| Function | Does |
|---|---|
| `read_file(path)` | contents as `str` |
| `write_file(path, text)` | truncate and write |
| `file_exists(path)` | `bool` |
| `make_dir(path)` | like `mkdir -p` |
| `list_dir(path)` | `str[]` of names, sorted |
| `is_dir(path)` | `bool` |
| `file_size(path)` | bytes, or `-1` |
| `modified_ms(path)` | mtime in ms, or `-1` |
| `remove_file(path)` | delete a file |
| `remove_dir(path)` | delete a directory and its contents |
| `copy_bytes(src, dst)` | byte-for-byte copy |

A missing file's size is `-1` rather than `0`, so an empty file and an absent
one are distinguishable. `copy_bytes` exists because
`write_file(dst, read_file(src))` stops at the first NUL.

## Processes

| Function | Does |
|---|---|
| `exec(cmd, args)` | run it, wait, return the exit code |
| `capture(cmd, args)` | run it, return its stdout |
| `env(name)` | an environment variable, `""` when unset |
| `cwd()` | the working directory |
| `chdir(path)` | change it |

There is no shell in between: each element of `args` is one argument, however it
is spelled:

```maca
exec("cp", ["my notes.txt", dest])   // one file, not two
exec("echo", ["$HOME"])              // prints $HOME, does not expand it
```

`exec` searches `PATH`; a program that isn't there exits `127`. `std/proc` adds
`run` (stop the program if the step fails), `try_run`, `run_in`, `output`
(captured and trimmed), `which`/`have`, `env_or`.

## Standard input

| Function | Does |
|---|---|
| `read_line()` | one line, newline stripped |
| `at_eof()` | is input exhausted? |
| `read_stdin()` | all of it |

```maca
while !at_eof() {
    line = read_line()
    info(line.upper())
}
```

## Time

| Function | Does |
|---|---|
| `now_ms()` | milliseconds since the Unix epoch |
| `now_iso()` | `"YYYY-MM-DDTHH:MM:SSZ"` |
| `format_time(ms, fmt)` | `strftime` over the instant |

Everything is UTC.

## Concurrency

| Form | Does |
|---|---|
| `spawn f(x)` | run concurrently, giving a `Future a` |
| `await fut` | wait for it, giving an `a` |
| `sleep_ms(n)` | suspend |

There is no `async` keyword. See [Effects and Async](a7-effects.md).

## Markup

| Form | Does |
|---|---|
| `div(class="x", child)` | an element: named args are attributes, positional ones children |
| `data-tomo="x"` | an attached `-` is part of the name; a spaced one subtracts |
| `open=true` | a bool decides whether the attribute exists |
| `element(tag, …)` | the same, with the tag as a value |
| `styles()` | the CSS for the utility classes this module writes |

See [The UI Syntax](a11-ui.md).

## JSON

`import std/json` brings in two halves. `encode` and `decode` are the typed pair,
written by the compiler from the record and sum types the program declares; the
rest reads and writes JSON as text.

```maca
import std/json

Layout = List | Grid
Link   = { title: str, url: str }
Config = { columns: int, layout: Layout, links: Link[] }

save(c: Config) -> unit => write_file("conf.json", encode(c))

load(text: str) -> Config {
    c: Config = decode(text)
    c
}
```

`encode(value)` writes the JSON for the value's static type: a record becomes an
object with one member per field, **in the order the record declares them**; a
list becomes an array. `decode(text)` reads into whatever the binding says, so
the type has to be written down. A bare `decode(text)` is a build error.

### How a sum maps

**A variant is its own name in lower case.** `Layout = List | Grid` is stored as
`"list"` and `"grid"`. A variant carrying a payload has no JSON form beyond its
name, so a type that round-trips should be an enumeration.

### What decode says when the text does not match

It fails, and the message names the field. `try` catches it
([Errors](09-errors.md)):

```maca
why = try load(text)
if why != "" {
    warn("bad config: {why}")
}
```

| The text | The message |
|---|---|
| `{"columns": "three", …}` | ``field `columns`: expected a number, got a string`` |
| `{"layout": "grid", …}` with no `columns` | ``field `columns`: expected a number, and the object has no such field`` |
| `{"layout": "table", …}` | ``field `layout`: "table" is not one of list, grid`` |
| `[1, 2, 3]` | ``` `Config`: expected an object, got a list ``` |

A field inside a nested record or list element reports under its own name.

### The text half

| Function | Does |
|---|---|
| `quote(s)` | `s` as a JSON string literal |
| `array_of_str(xs)` `array_of_int(xs)` | an array from a list |
| `object_of(keys, values)` | an object from parallel lists |
| `get(src, key)` | the raw text of a member, `""` when absent |
| `get_int(src, key, dflt)` `get_bool(src, key)` | read one member |
| `items(src)` | the elements of an array, each as raw text |

## Errors

| Form | Does |
|---|---|
| `fail "message"` | raise |
| `x?` | propagate a failure to the caller |
| `try e` | catch one |

See [Errors](09-errors.md).

## Assertions

| Function | Does |
|---|---|
| `assert(cond, msg)` | report `msg` if `cond` is false |
| `assert_eq(got, want, msg)` | report both sides if they differ |
| `failures()` | how many assertions have failed |

A failing assertion reports and keeps going. `failures()` is the number a test
function returns; see [Testing](12-testing.md).

## Regular expressions

There are none. `apps/selfhost/lexer.maca` scans the whole language with
`chars`, `at` and three predicates. Reach for `split` and a loop.
