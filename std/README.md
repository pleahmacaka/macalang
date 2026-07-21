# std — Maca-source standard library

Written in `.maca`, not Rust. Lands module-by-module as the backends come online.

**Prelude (imported implicitly, unqualified):**
- `minstd` — core types and functions
- `minconsole` — syslog-level logging (`emerg`..`debug`) + `input`

**Explicit `import`:**
`std/os` · `std/net` · `std/json` · `std/path` · `std/dirs` · `std/collections` ·
`std/str` · `std/html` · `std/mqtt`

See `docs/PLAN.md` for the full stdlib table and effect rows.
