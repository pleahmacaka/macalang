# std — the Maca standard library

Two layers.

The **prelude** ships as compiler and runtime builtins: always available, no
`import`, the same on every target. That is what a language surface should give
you for the things every program touches.

The **modules** in this directory are ordinary Maca source you import. They are
the layer above the primitives — the code you would otherwise write again in
every project — and being written in Maca means they are also a working example
of the language and, through `std/tests/`, part of what gates the compiler.

| module | what it is for |
|---|---|
| `std/text` | `lines`, `words`, `split_once`, `strip_prefix`/`strip_suffix`, `index_of_from`, `last_index_of`, `between`, `escape_html`, `count`, `title_case`, `indent`, `dedent`, `wrap` |
| `std/list` | `any_of`, `all_of`, `find_index`, `count_if`, `take`, `drop`, `chunk`, `zip_add`, `flatten`, `unique`, `range`, and for `str[]`: `str_unique`, `str_take`, `str_drop`, `str_find_index`, `str_flatten` |
| `std/path` | `join`, `basename`, `dirname`, `extension`, `stem`, `with_extension`, `is_absolute`, `normalize` |
| `std/json` | `quote`, `array_of_str`/`array_of_int`, `object_of`, `get`, `get_int`, `get_bool`, `items` |
| `std/csv` | `field`, `row`, `document`, `parse`, `parse_row`, `column` — quoted fields, doubled quotes, embedded newlines |
| `std/fs` | `walk`, `walk_dirs`, `find`, `read_lines`, `write_if_changed`, `append_file`, `copy_file`, `copy_tree`, `tree_size` |
| `std/proc` | `run`, `try_run`, `run_in`, `output`, `which`/`have`, `env_or` — running other programs, with no shell in between |

```maca
import std/path
import { lines, dedent } from std/text
```

A selective import pulls in only what you name, plus what it needs.

The table above is the public API, and it is not a promise kept by hand: every
name in it has a `///` doc comment in its module, every `///` in these modules
names something in the table, and `crates/driver/tests/programs/sitegen.maca`
fails if the two ever disagree. `tools/macadoc.maca` renders them —

```
maca run tools/macadoc.maca site/api std/text.maca std/list.maca
```

— and handbook chapter 17 explains the marker.

The prelude, for reference:

**Prelude (always available, unqualified):**
- **console** — syslog-level logging `emerg`/`alert`/`crit`/`err`/`warn`/
  `notice`/`info`/`debug`, plus `print` and `input`.
- **conversions** — `int(x)`, `float(x)`, `str(x)`, `len(x)`.
- **math** — `abs`, `min`, `max`, `clamp`, `sign`, `gcd`, `sqrt`, `floor`,
  `ceil`, `round`, `pow`, `sin`, `cos`, `tan`, `log`, `exp`.
- **async** — `spawn`/`await` (colorblind), `sleep_ms`.
- **file I/O** — `read_file(path) -> str` (empty when unreadable),
  `write_file(path, text) -> bool`, `file_exists(path) -> bool`,
  `make_dir(path) -> bool` (`mkdir -p`), `list_dir(path) -> str[]` (entry names,
  sorted so builds are reproducible), `copy_bytes(src, dst) -> bool` (a binary
  file survives it; `write_file(read_file(…))` stops at the first NUL).
- **processes** — `exec(cmd, args) -> int` (the exit code), `capture(cmd, args)
  -> str` (its stdout), `env(name) -> str`, `cwd() -> str`, `chdir(path) ->
  bool`. `args` is a `str[]` and there is no shell in between, so an argument
  holding a space is one argument.

**String methods (UFCS on `str`):** `split`, `join`, `trim`, `upper`, `lower`,
`contains`, `starts_with`, `ends_with`, `replace`, `substr`, `index_of`,
`repeat`, `pad_start`/`pad_end` (width + optional pad string, default a space),
subscripting `s[i]`. Byte-level scanning: `length` (byte count), `at(i)` (the
1-char string at byte `i`), `chars` (→ `str[]` of single bytes), and the
character classes `is_whitespace` / `is_ascii_digit` / `is_alpha`.

**List methods (UFCS on `T[]`):** `map`, `filter`, `reduce`/`fold`, `sort`,
`reverse`, `push`, `pop`, `contains`, `index_of`, `sum`, `min`, `max`, `first`,
`last`, `len`, `length`, `get(i)`, `slice(from, to)`, subscripting `xs[i]`,
functional update.

**FFI-backed modules:** `import c "sqlite3.h"` (open/exec/prepare/step/column_*/
finalize/close), `import c "mqtt.h"`, `import py "…"`, and `import nixpkgs` for
config mode.

See `docs/SPEC.md` for the language cheatsheet and effect rows, and
`examples/{collections,strings,async}.maca` for the prelude in use.
