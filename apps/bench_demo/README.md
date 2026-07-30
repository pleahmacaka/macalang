# bench_demo: measuring nine cases and printing the table

Nine cases out of the [`modules/bench`](../../modules/bench) corpus, three
samples each, about a second in total.

```sh
maca run apps/bench_demo/bench_demo.maca
```

It prints two tables (primitives, then algorithms), the footnote that says how
the numbers were taken, and one comparison table.

## Why `quick()`

`quick()` is used on purpose and the footer says so: three samples of fifteen
milliseconds is enough to see a tenfold difference and nowhere near enough to
see a five percent one. `defaults()` is what a number you intend to quote is
measured with.

## Why the baseline is faked

`compare` needs two runs and a demo has one, so `against_baseline` halves each
result's iteration count to double its cost per call. Every case has therefore
moved, which is what the table is here to show. A real caller loads the other
run instead:

```maca
before = load("bench/baseline.json")
info(comparison_table(compare(before, results, Tolerance)))
```

Each candidate is passed to `measure` by name, which is the whole calling
convention: a function `(size: int) -> int` goes in as a parameter. There is no
list of cases to loop over here because a function value cannot be stored in a
record field; `bench/cases/catalog` has the version that runs all forty-six by
name, at the cost of a string comparison per call.
