# tambo_demo: a small site on tambo

A page, two JSON endpoints, a route with a parameter, a wildcard route, a POST,
and a handler that fails. The worked example of
[`modules/tambo`](../../modules/tambo) over
[`modules/http`](../../modules/http).

```sh
maca run apps/tambo_demo/tambo_demo.maca       # print the table and answer a
                                               # few requests, no socket
maca run apps/tambo_demo/tambo_demo.maca 8080  # serve on port 8080
```

With no port it dispatches a handful of hand-written requests through the real
`dispatch` and prints each reply, so the whole site is exercised without opening
a socket.

## The shape is the point

A table of routes at the top, one `handle` where every name in that table is
answered, and nothing in between. Read the two against each other, because that
is the whole program.

A name in the table with no arm in `handle` is not a 404. `serve` refuses to
start and says which name it could not find, and `missing_handlers` is what the
last line of output counts.
