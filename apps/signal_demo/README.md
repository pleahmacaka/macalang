# signal_demo: the single node each change touches

A counter, a label derived from it, and the patch a browser would apply. The
worked example of [`modules/signal`](../../modules/signal).

```sh
maca run apps/signal_demo/signal_demo.maca
```

It renders the page once, then prints the patch it *would* apply instead of
applying it, which is what makes the wiring testable with no browser. The `js:`
line is the call a browser would run against that same markup; send it down
however the page gets its updates, next to the runtime `runtime_script()` puts
in the page.

Do not reach for `--target js` here: that backend drops an imported module's
definitions without saying so, and the page loads and throws.

## What to watch

The page has four bound-looking nodes: two bound to keys that change, one bound
to `theme`, which does not, and one bound to nothing at all. The patches name
only the first two. The last section writes `count` the value it already had,
and there is nothing to patch.

`settle` walks `derived()` rather than naming `"label"`, so the page can grow a
computed without this function needing an edit.
