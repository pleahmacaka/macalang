# Testing

Tests live beside the code they test, in the same file, and the toolchain finds
them by name.

## Writing a test

Any function whose name begins with `test_` is a test, and `assert` /
`assert_eq` are what it says:

```maca
Counter = {
    n: int
}

bump(c: Counter) -> Counter =>
    c with { n = c.n + 1 }

test_bump_increments_by_one() {
    c = bump(Counter { n = 1 })
    assert_eq(str(c.n), "2", "one more than it was")
}

test_bump_is_not_in_place() {
    c = Counter { n = 1 }
    bump(c)
    assert_eq(str(c.n), "1", "the original is untouched")
}
```

A test declares no return type and returns nothing. It passes when none of its
assertions failed.

Name a test after what it establishes, not after the function it calls.
`test_bump_is_not_in_place` tells you what broke when it fails;
`test_bump` tells you where to start looking.

## The two assertions

| Call | Passes when |
|---|---|
| `assert(cond, message)` | `cond` is true |
| `assert_eq(got, want, message)` | `got == want` (both `str`) |

The third argument is not the expression restated — the runner already prints
the values. It is what the expression was *supposed to establish*, so a failure
reads as a sentence:

```
assertion failed: the two disagree
  got:  got
  want: want
```

`assert_eq` compares strings, so a number is `str(n)` on the way in. That keeps
one assertion instead of one per type, and the failure output shows the values
as you would write them.

## Running

```
maca test counter.maca
```

```
running 2 tests
  test_bump_increments_by_one
    ok
  test_bump_is_not_in_place
    ok
2 tests passed
```

The driver collects every `test_`-prefixed function in the file, drops the
file's own `main` if it has one, and generates a runner that announces each test
before calling it — so a test that crashes tells you which one it was.

The exit code is the number of failed assertions, so `maca test` composes with
anything that reads exit codes.

## A failed assertion does not stop the run

Every assertion runs, and the failures are counted:

```
assertion failed: the two disagree
  got:  got
  want: want
assertion failed: one is not greater than two
running 3 tests
  test_a_failure_shows_both_sides
    FAILED
  test_a_bare_assertion_shows_its_message
    FAILED
  test_a_passing_one_still_runs
    ok
2 assertion(s) failed
```

That is deliberate. Aborting on the first failure means fixing a suite takes as
many runs as it has bugs; counting them means one run tells you everything.

`failures()` returns the running count, which is what the generated runner uses
to decide whether a test passed — and what a test can use itself if it wants to
skip work that a failed precondition has made meaningless.

## Testing across files

Tests are found in the file you point at. A test file can import the module it
exercises:

```maca
import geometry

test_origin_is_the_zero_point() {
    p = origin()
    assert_eq("{p.x},{p.y}", "0,0", "both coordinates start at zero")
}
```

That is how `std/` is tested: `std/tests/path.maca` imports `std/path` and
nothing else, so the suite is exactly the module's public surface exercised
through the front door.

## The larger point: run your documentation

The Maca repository holds a file called `examples/handbook.maca`. It contains
every runnable claim this book makes — the record update from
[Records](05-records.md), the format specs from
[Common Concepts](03-common-concepts.md), the list patterns from
[Sum Types](06-sum-types.md) — in one program,
and the test suite runs it and checks each line of its output.

That file exists because writing this handbook broke things. Five compiler bugs
and one nonexistent command were found by taking prose that had been written
confidently and actually executing it:

- a function with no declared return type discarded its body
- an undeclared return type left callers unable to convert the result
- list methods rejected a named function, accepting only lambdas
- there was no pattern for an empty list
- `maca test` was documented but did not exist
- a literal `{` in a string silently swallowed the rest of the file

Every one of those was in text that read perfectly well. Documentation that
isn't run is a claim, not a fact — and the cheapest way to make it a fact is to
put it in a file the test suite executes.

The same argument applies one level down, to the tests themselves. A test that
prints its results and is checked by something else grepping that output is
testing the printing. Assert in the language, and let the exit code be the
verdict.
