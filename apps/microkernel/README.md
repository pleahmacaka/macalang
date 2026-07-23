# microkernel — a message-passing kernel, in Maca

A small **L4-style microkernel, simulated in Maca** (`kernel.maca`). It runs in
userland — `maca run` boots it — but models the real thing:

- a **round-robin priority scheduler** over a struct-of-arrays task table
- a **bump allocator** over a fixed kernel heap
- synchronous **IPC** through per-task mailboxes (send / recv / block / wake)
- **capability-checked syscall dispatch** (`Compute`/`Send`/`Recv`/`Yield`/`Exit`),
  with a task that faults because its capability lacks `SEND`
- a **tick-driven preemption** loop with per-task time-slice accounting

It's also the project's build/​compiler stress test: 280 lines, ~20 functions,
sum types with payloads, records, inclusive ranges, nested `match`, and deep
loop nesting, all lowered to ~350 lines of C.

```sh
maca run apps/microkernel/kernel.maca
```

prints the boot banner, the scheduler trace (one line per syscall), and a halt
report — context switches, retired tasks, capability faults, IPC volume, heap
high-water mark, and an ASCII bar chart of CPU time per task.

## Incremental build

The native compiler is content-addressed (see `crates/driver/src/build_cache.rs`):
an unchanged `maca build`/`run` copies the cached binary and skips the whole
pipeline. For this kernel that's a **cold build ~0.4s → warm build ~3ms**; a
changed source still reuses the cached C runtime object (~0.2s). Set
`MACA_NO_CACHE=1` to force a full build, or `MACA_VERBOSE=1` to log cache hits.
