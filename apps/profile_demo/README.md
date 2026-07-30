# profile_demo: a program that measures itself

Three real pieces of work with a span around each, then the flame chart, the
summary table, and the trace written out as JSON and SVG. The worked example of
[`modules/profile`](../../modules/profile).

```sh
maca run apps/profile_demo/profile_demo.maca
```

The work is building a paragraph, sorting its words, and walking a directory
tree (`modules/` when it is there, `.` otherwise). Output files land in
`/tmp/maca-profile-demo/`: `trace.json` and `flame.svg`.

## The recorder is a value

That is the whole convention: a function that wants to be measured takes the
recorder and gives it back. Where a function also has a result, the pair gets a
name (`Scan` in the source), because Maca has no tuple and a second traversal to
collect the same numbers would itself be measured.

One span per directory means the full summary is mostly names that cost nothing,
so the table shows the eight heaviest and the trace file keeps the rest.
