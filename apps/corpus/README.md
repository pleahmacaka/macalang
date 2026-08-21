# corpus

TypeScript to Maca to JavaScript, with the package's own test suite as the
oracle. A pair is kept only when the tests that were written for the original
pass against the compiled translation.

## Running it

```sh
maca -m corpus.collect lodash.chunk is-odd     # licence-gated fetch
MACA_CORPUS_MODEL=./your-model maca -m corpus  # translate, compile, grade
```

`MACA_CORPUS_MODEL` is a command taking one argument, the path to a prompt
file, and writing Maca to stdout. The prompt is `maca spec --llm` followed by
the TypeScript. Deterministic rules are meant to take this over one construct
at a time; until they do the translator is a model, and it is a subprocess so
that swapping one for the other changes nothing else here.

## The licence gate is at the front

A corpus is redistributed, so the check runs in `collect.maca` on the
registry's metadata, before a tarball is fetched. Bytes that were never
downloaded cannot leak into a record through an oversight further down.

`licence.maca` holds the list. `MIT OR GPL-3.0` passes because the taker
chooses; `MIT AND GPL-3.0` does not, because both bind. An unstated licence is
not a permissive one.

## Output

`corpus.jsonl`, one record a line:

```json
{"pkg": "…", "license": "…", "ts_source": "…", "maca_source": "…", "test_result": "…"}
```

`failures.jsonl` gets everything else, with the step that rejected it
(`licence`, `fetch`, `translate`, `compile`, `test`) and why. That file is the
point as much as the first one: a pipeline that records only its successes
cannot tell you what to fix next.

Neither is committed. They are outputs, and a corpus of other people's source
is not this repository's to carry.

## What CI checks

The licence gate and the record shape, in `tests/corpus.maca`, with no network
and no model. A separate test greps the allowed list for copyleft identifiers,
so widening it by accident fails rather than ships.
