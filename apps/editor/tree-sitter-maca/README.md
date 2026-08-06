# tree-sitter-maca

A tree-sitter grammar for Maca, for editor highlighting and structural editing.

The significant-newline layout is handled by an external scanner
(`src/scanner.c`): it emits a `_newline` between items and statements, but
suppresses it inside `(` / `[` groups and at line continuations (a line that
starts with an operator, `.`, a closing bracket, or `,`), so a multi-line
ternary or operator chain parses as one expression.

The grammar covers imports, functions (including body-less FFI/extern
declarations), bindings (with the config-mode `name: Type: Base =` layering),
sum and record type declarations, control flow (`if` / `match` / `for` /
`while`), patterns (literal, constructor, record, or-patterns, and list
`..rest`), the reactive-UI DSL (`class="c"`, juxtaposed arguments, `bind:` /
`on:` directives, assignment lambda bodies), and the expression grammar
(operators with precedence, ternary, calls, field/index, lambdas, ranges,
string interpolation with `{{`/`}}` brace escapes).

All 33 `examples/*.maca` and every `modules/maca/*.maca` parse with zero error
nodes. (The `apps/microkernel` freestanding-asm intrinsics and `apps/mcmod`
JVM-annotation surface remain out of scope.)

## Build & test

```sh
npm install                 # tree-sitter-cli
npx tree-sitter generate    # grammar.js + src/scanner.c → src/parser.c
npx tree-sitter test        # runs test/corpus
npx tree-sitter parse ../../examples/hello.maca
```

The generated parser (`src/parser.c`, `src/grammar.json`, …) is git-ignored;
regenerate it from `grammar.js` and `src/scanner.c`.

## Files

| file | role |
|---|---|
| `grammar.js` | the grammar (rules, precedence, conflicts) |
| `src/scanner.c` | external scanner for significant newlines |
| `queries/highlights.scm` | syntax-highlighting captures |
| `test/corpus/` | parse-tree regression tests |
