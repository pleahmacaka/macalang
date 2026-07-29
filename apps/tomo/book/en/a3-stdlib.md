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
| `push(x)` `pop()` | a new list |
| `slice(from, to)` | `to` is **exclusive** |
| `contains(x)` `index_of(x)` | search |
| `sum()` `min()` `max()` | numeric |
| `first()` `last()` `get(i)` | elements |
| `length()` | `int` |
| `join(sep)` | a `str[]` into one `str` |
| `parallel(f)` | like `map`, evaluated concurrently |

## Maps

`Map str V` is a string-keyed hash map, monomorphized on its value type the way
an array is on its element type.

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
info("{counts.get("kiwi", 0)}")      // 0 — a miss gives the default
```

Keys are `str` and only `str`. One key type is one hash and one comparison, and
an integer key is `str(n)` away. `keys()` comes back sorted, so a program that
walks a map twice produces the same output twice — which matters when the output
is a file under version control.

`get` takes a default rather than returning something empty, because the
language has no null to return.

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

A missing file's size is `-1` rather than `0`, so an empty file and an absent one
are distinguishable without a second call.

`copy_bytes` exists because `write_file(dst, read_file(src))` stops at the first
NUL — fine for source, silently truncating for a wasm module or an image.

## Processes

| Function | Does |
|---|---|
| `exec(cmd, args)` | run it, wait, return the exit code |
| `capture(cmd, args)` | run it, return its stdout |
| `env(name)` | an environment variable, `""` when unset |
| `cwd()` | the working directory |
| `chdir(path)` | change it |

There is no shell in between. `args` is a `str[]`, and each element is one
argument however it is spelled:

```maca
exec("cp", ["my notes.txt", dest])   // one file, not two
exec("echo", ["$HOME"])              // prints $HOME, does not expand it
```

That is the difference from a command string, and it is the whole reason these
are builtins rather than a `system()` wrapper. `exec` searches `PATH` the way a
shell would; a program that isn't there exits `127`.

`std/proc` builds the usual conveniences on top: `run` (stop the program if the
step fails), `try_run`, `run_in` (in a directory, and back again), `output`
(captured and trimmed), `which`/`have`, `env_or`.

## Standard input

| Function | Does |
|---|---|
| `read_line()` | one line, newline stripped |
| `at_eof()` | is input exhausted? |
| `read_stdin()` | all of it |

`at_eof` exists because a blank line and end-of-input both read as the empty
string:

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

Everything is UTC. Local time needs a zone database and a policy for what to do
without one; a program that wants it can format the epoch milliseconds itself.

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
| `data-tomo="x"` | an attached `-` is part of the name; a spaced one subtracts |
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

## Assertions

| Function | Does |
|---|---|
| `assert(cond, msg)` | report `msg` if `cond` is false |
| `assert_eq(got, want, msg)` | report both sides if they differ |
| `failures()` | how many assertions have failed |

A failing assertion reports and keeps going. Aborting on the first one means
fixing a suite takes as many runs as it has bugs; counting them means one run
tells you everything. `failures()` is the number a test function returns, which
is the same "0 or non-zero" contract as chapter 12.

## Regular expressions

There are none. `contains`, `starts_with`, `ends_with`, `index_of`, `split` and
the character classes cover what a program in this language actually reaches for
— `selfhost/lexer.maca` scans the whole language with `chars`, `at` and three
predicates — and a regex engine is a language of its own to learn, debug and
implement. Reach for `split` and a loop.
