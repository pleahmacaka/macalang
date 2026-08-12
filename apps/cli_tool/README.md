# cli_tool: a command line, end to end

`report` counts what is in a directory. It is the worked example of
[`modules/cli`](../../modules/cli): the command is one value, the `--help` page
is rendered from that value, and a misspelt option is answered with the nearest
name out of the same list.

```sh
maca run apps/cli_tool/cli_tool.maca --help
maca run apps/cli_tool/cli_tool.maca modules/cli --sort lines
maca run apps/cli_tool/cli_tool.maca modules/cli --srot lines
```

No `--` before the program's own arguments. `maca run` forwards it and
`cli/parse` reads it as the end-of-options marker, so `-- --help` asks for a
directory called `--help`.

## The three outcomes

A command line has three, and only one of them is the happy path:

| invocation | what happens |
|---|---|
| `--help` | the page printed from `Report`, exit 0, nothing done |
| `--srot lines` | `unknown option --srot, did you mean --sort?`, exit 1 |
| `modules/cli --top 3 --totals` | the table, a totals row, exit 0 |

`modules/maca/tests/examples.maca` builds this program and asserts all three,
because a help page that drifts from the options it documents is the failure
this design exists to prevent.
