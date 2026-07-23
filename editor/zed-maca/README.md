# Maca — Zed extension

Syntax highlighting **and a language server** (diagnostics, hover, completion)
for `.maca` files in [Zed](https://zed.dev).

## Prerequisite: the language server

The extension launches `maca-lsp`, which must be on your PATH. The Maca
installer puts it next to `maca`:

```sh
curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
```

(or `cargo build --release -p maca-lsp` and copy it onto PATH).

## Install as a dev extension

1. Zed → **Extensions** → **Install Dev Extension**.
2. Select this folder (`editor/zed-maca`).

Zed compiles the Rust extension (`src/lib.rs`, via `zed_extension_api`) to
wasm, builds the tree-sitter grammar from `extension.toml`, applies the
`languages/maca/highlights.scm` queries, and starts `maca-lsp` for open
`.maca` buffers.

## Files

| file | role |
|---|---|
| `extension.toml` | extension metadata + grammar source + `[language_servers.maca-lsp]` |
| `Cargo.toml` / `src/lib.rs` | the wasm extension: resolves & launches `maca-lsp` |
| `languages/maca/config.toml` | language config: suffixes, comments, brackets, tab size |
| `languages/maca/highlights.scm` | tree-sitter highlight queries |

## Notes

- Pin `grammars.maca.commit` in `extension.toml` to a real revision before
  publishing; `HEAD` works for local dev builds.
- `Cargo.toml` pins `zed_extension_api`; if a Zed build reports an API
  mismatch, bump that version to match your Zed (the `Extension` trait +
  `language_server_command` surface used here is stable across recent
  releases). The crate is detached from the main `crates/*` workspace, so it
  never affects `cargo build`/`cargo test` at the repo root.
- The grammar (`editor/tree-sitter-maca`) is a scaffold: it classifies the token
  types the highlighter needs. Full significant-newline layout is delegated to
  an external scanner (a future addition) — highlighting works today; structural
  features (folds, indents from the grammar) grow with it.
- The same token model backs the TextMate grammar (`editor/maca.tmLanguage.json`),
  kept in sync with the lexer by `crates/lexer/tests/highlight_sync.rs`.
