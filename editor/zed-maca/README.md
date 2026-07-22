# Maca — Zed extension

Syntax highlighting, comment toggling, and bracket handling for `.maca` files
in [Zed](https://zed.dev).

## Install as a dev extension

1. Zed → **Extensions** → **Install Dev Extension**.
2. Select this folder (`editor/zed-maca`).

Zed builds the tree-sitter grammar declared in `extension.toml`
(`editor/tree-sitter-maca` in this repo) and applies the queries in
`languages/maca/highlights.scm`.

## Files

| file | role |
|---|---|
| `extension.toml` | extension metadata + grammar source |
| `languages/maca/config.toml` | language config: suffixes, comments, brackets, tab size |
| `languages/maca/highlights.scm` | tree-sitter highlight queries |

## Notes

- Pin `grammars.maca.commit` in `extension.toml` to a real revision before
  publishing; `HEAD` works for local dev builds.
- The grammar (`editor/tree-sitter-maca`) is a scaffold: it classifies the token
  types the highlighter needs. Full significant-newline layout is delegated to
  an external scanner (a future addition) — highlighting works today; structural
  features (folds, indents from the grammar) grow with it.
- The same token model backs the Monarch grammar used by the web playground
  (`playground/maca-lang.js`) and the TextMate grammar (`editor/maca.tmLanguage.json`),
  all kept in sync with the lexer by `crates/lexer/tests/highlight_sync.rs`.
