# std — the Maca standard library

Most of the library ships as **compiler/runtime builtins** — always available,
no `import` needed — rather than `.maca` source, because that's what a mature
language surface wants (a prelude you can just use). What exists today:

**Prelude (always available, unqualified):**
- **console** — syslog-level logging `emerg`/`alert`/`crit`/`err`/`warn`/
  `notice`/`info`/`debug`, plus `print` and `input`.
- **conversions** — `int(x)`, `float(x)`, `str(x)`, `len(x)`.
- **math** — `abs`, `min`, `max`, `clamp`, `sign`, `gcd`, `sqrt`, `floor`,
  `ceil`, `round`, `pow`, `sin`, `cos`, `tan`, `log`, `exp`.
- **async** — `spawn`/`await` (colorblind), `sleep_ms`.

**String methods (UFCS on `str`):** `split`, `join`, `trim`, `upper`, `lower`,
`contains`, `starts_with`, `ends_with`, `replace`, `substr`, `index_of`,
subscripting `s[i]`.

**List methods (UFCS on `T[]`):** `map`, `filter`, `reduce`/`fold`, `sort`,
`reverse`, `push`, `pop`, `contains`, `index_of`, `sum`, `min`, `max`, `first`,
`last`, `len`, subscripting `xs[i]`, functional update.

**FFI-backed modules:** `import c "sqlite3.h"` (open/exec/prepare/step/column_*/
finalize/close), `import c "mqtt.h"`, `import py "…"`, and `import nixpkgs` for
config mode.

See `docs/PLAN.md` for the language cheatsheet and effect rows, and
`examples/{collections,strings,async}.maca` for the prelude in use.
