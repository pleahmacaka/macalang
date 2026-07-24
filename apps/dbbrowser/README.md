# dbbrowser — a database browser, in Maca

Database browsers over the C FFI — the capstone the language grew its string/
list stdlib, closures, and result-set FFI for. They open a database, list its
tables, and print query results as an aligned table. The browser logic is all
Maca; only the thin binding is C.

- **`pgbrowser.maca`** — **PostgreSQL** over libpq. A connection is opened with
  an explicit **permission mode**: read-only (the *server* rejects every write)
  or read-write. `pg_connect(dsn, readonly)` sets
  `default_transaction_read_only`, so a read-only browser physically cannot
  mutate the database — the write probe is refused with "cannot execute in a
  read-only transaction". Connection info comes from libpq (`PGHOST`/`PGDATABASE`
  /`PGUSER` env vars or an explicit DSN).

  ```sh
  maca run apps/dbbrowser/pgbrowser.maca   # needs libpq-dev + a reachable server
  ```

- **`browser.maca`** — **SQLite**, self-contained (in-memory demo).

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
