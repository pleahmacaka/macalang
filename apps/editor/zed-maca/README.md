# Maca: the Zed extension

Syntax highlighting **and a language server** (diagnostics, hover, completion,
go-to-definition, document symbols, find-references, rename, signature help,
formatting, and code actions: quick fixes for the checker's diagnostics plus a
couple of refactorings)
for `.maca` files in [Zed](https://zed.dev).

## Prerequisite: the language server

The extension launches `maca-lsp`, which must be on your PATH. The Maca
installer puts it next to `maca`:

```sh
curl -fsSL -O https://github.com/pleahmacaka/macalang/releases/latest/download/maca-install-linux-x86_64
chmod +x maca-install-linux-x86_64 && ./maca-install-linux-x86_64
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
| `languages/maca/outline.scm` | symbol picker + breadcrumbs (functions, records, sums) |
| `languages/maca/indents.scm` | auto-indent for blocks, records, lists, params, calls |

## Publishing to the Zed extension registry

Zed installs extensions from the [`zed-industries/extensions`](https://github.com/zed-industries/extensions)
registry. Zed builds the grammar by cloning the grammar repo **at the pinned
commit and compiling the committed `src/parser.c` + `src/scanner.c` directly.
It does not run `tree-sitter generate`.** That is why `editor/tree-sitter-maca/src/`
(the generated `parser.c`, `node-types.json`, `grammar.json`, and the
`tree_sitter/*.h` headers) is committed to this repo rather than `.gitignore`d.

To publish or update the listing:

1. Regenerate + verify the grammar if `grammar.js`/`scanner.c` changed:
   ```sh
   cd editor/tree-sitter-maca
   tree-sitter generate && tree-sitter test
   ```
   Commit the regenerated `src/` and push to `main`.
2. Set `grammars.maca.commit` in `extension.toml` to that commit's SHA
   (a real revision, since `HEAD` only works for local dev builds), and bump
   `version` if the extension itself changed. Push.
3. Fork `zed-industries/extensions`, add an entry to its `extensions.toml`:
   ```toml
   [maca]
   submodule = "extensions/maca"          # a submodule pointing at this repo
   path = "editor/zed-maca"               # where extension.toml lives
   version = "0.6.3"                       # matches extension.toml `version`
   ```
   add the submodule (`git submodule add https://github.com/pleahmacaka/macalang extensions/maca`),
   and open a PR. The Zed team's CI validates the grammar builds and the
   extension compiles to wasm.

Publishing itself is done from outside this repository (the PR lives in
`zed-industries/extensions`); everything this repo can control, meaning the
grammar, the committed parser, `extension.toml`, and the pinned commit, is
release-ready.

## Notes

- `Cargo.toml` pins `zed_extension_api`; if a Zed build reports an API
  mismatch, bump that version to match your Zed (the `Extension` trait +
  `language_server_command` surface used here is stable across recent
  releases). The crate is detached from the main `crates/*` workspace, so it
  never affects `cargo build`/`cargo test` at the repo root.
- The grammar (`editor/tree-sitter-maca`) classifies the token types the
  highlighter needs; significant-newline layout is handled by the external
  scanner (`src/scanner.c`), so highlighting works on real multi-line programs.
- The same token model backs the TextMate grammar (`editor/maca.tmLanguage.json`),
  kept in sync with the lexer by `crates/lexer/tests/highlight_sync.rs`.
