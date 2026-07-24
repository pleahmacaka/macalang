# dbbrowser — a SQLite browser, in Maca

A small database browser (`browser.maca`) — the capstone the language grew its
string/list stdlib, closures, and result-set FFI for. It opens a SQLite
database, lists its tables, and prints query results as an aligned table.

```sh
maca run apps/dbbrowser/browser.maca
```

```
tables:
name
--------------
people
(1 rows)

id            name          city
------------------------------------------
1             ada           london
2             alan          manchester
3             grace         new york
(3 rows)
```

## What it exercises

- **C FFI with real result sets** — opaque `int` handles for the db and
  statement, `sqlite_prepare`/`sqlite_step`/`sqlite_column_*` to iterate rows
  and read each column (`import c "sqlite3.h"`, bound in `maca-runtime`).
- **The string stdlib** — `len` + interpolation for column padding and rules.
- **Control flow** — `while` over the result set, nested loops over columns.

## Building

On a plain Linux host with `libsqlite3-dev`, `maca` links the system SQLite
with the host `cc`; on the WSL/Nix path it pulls `nixpkgs#sqlite`. No manual
build steps either way.
