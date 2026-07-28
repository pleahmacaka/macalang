# Appendix C: The Standard Library

Most of Maca's "standard library" is compiler and runtime builtins rather than
Maca source. That keeps it available on every target — the same `xs.map(f)`
works in the native binary, the browser playground and the JVM output.

## Output

| Function | Does |
|---|---|
| `print(s)` | write to stdout, no newline |
| `info(s)` | write a line to stdout |
| `err(s)` | write a line to stderr |

The names come from syslog levels; `warn` and below go to stderr.

## Conversion

| Function | Does |
|---|---|
| `str(x)` | any value to its text |
| `int(s)` | text to integer |
| `float(s)` | text to float |
| `len(x)` | length of a list or string |

## Strings

Called as methods through UFCS — `s.trim()` is `trim(s)`.

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
| `push(x)` `pop()` | a new list |
| `slice(from, to)` | `to` is **exclusive** |
| `contains(x)` `index_of(x)` | search |
| `sum()` `min()` `max()` | numeric |
| `first()` `last()` `get(i)` | elements |
| `length()` | `int` |
| `join(sep)` | a `str[]` into one `str` |
| `parallel(f)` | like `map`, evaluated concurrently |

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

There is no `is_dir`. `list_dir` of a plain file returns nothing, which is the
usual way to tell them apart.

## Concurrency

| Form | Does |
|---|---|
| `spawn f(x)` | run concurrently, giving a `Future a` |
| `await fut` | wait for it, giving an `a` |
| `sleep_ms(n)` | suspend |

There is no `async` keyword. See chapter 13.

## Markup

| Form | Does |
|---|---|
| `div(class="x", child)` | an element — named args are attributes, positional ones children |
| `data_tomo="x"` | `_` in an attribute name is a `-` |
| `open=true` | a bool decides whether the attribute exists |
| `element(tag, …)` | the same, with the tag as a value |
| `styles()` | the CSS for the utility classes this module writes |

See chapter 15.

## Errors

| Form | Does |
|---|---|
| `fail "message"` | raise |
| `x?` | propagate a failure to the caller |
| `try e` | catch one |

See chapter 9.

## What is missing

Being straight about it, so you can plan around it:

- **No hash map.** A list of records and a linear scan is the current answer,
  which is what `examples/wordcount.maca` does.
- **No `is_dir`, no file metadata, no delete.**
- **No stdin.** Programs take arguments and read files.
- **No date or time.**
- **No regular expressions.**
- **No assertion library** — a test returns `0` or non-zero (chapter 12).
- **No string `slice`** — `substr` takes a length instead (calling it is a
  clean diagnostic, not a linker error).

Where these land is mostly a question of what gets built in Maca next; a hash
map in particular is a good first contribution, because it needs nothing from
the compiler.
