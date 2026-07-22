# std — Maca-source standard library

Written in `.maca`, not Rust. Lands module-by-module as the backends come online.

**Prelude (imported implicitly, unqualified):**
- `minstd` — core types and functions
- `minconsole` — syslog-level logging (`emerg`..`debug`) + `input`

**Explicit `import`:**
`std/os` · `std/net` · `std/json` · `std/path` · `std/dirs` · `std/collections` ·
`std/str` · `std/html` · `std/mqtt`

See `docs/PLAN.md` for the full stdlib table and effect rows.

## `std/str` — the contract the self-hosted compiler needs

`selfhost/lexer.maca` (see `docs/BOOTSTRAP.md`) leans on these `str` operations.
They must exist for the stage-0 C backend to lower the stage-1 compiler:

| op (UFCS) | type | meaning |
|---|---|---|
| `s.length()` | `str -> int` | length in characters |
| `s.chars()` | `str -> str[]` | explode into single-character strings |
| `xs.get(i)` | `str[] i:int -> str` | element at `i` |
| `s.slice(a, b)` | `str a:int b:int -> str` | substring `[a, b)` |
| `c.is_whitespace()` | `str -> bool` | space / tab / newline |
| `c.is_ascii_digit()` | `str -> bool` | `0`–`9` |
| `c.is_alpha()` | `str -> bool` | ASCII letter |

Under the stage-0 gradual checker these resolve to `any` (unknown stdlib), so
the sources type-check today; the native implementations land with the C
backend's string/list runtime.
