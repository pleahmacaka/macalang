# Maca → bare-metal firmware (embedded)

Write microcontroller firmware in **Maca** and cross-compile it to a flashable
image. No OS, no libc, no heap — just freestanding code driving hardware through
memory-mapped registers.

```sh
maca build --target embedded --mcu cortex-m4 blink.maca -o build
```

This emits `build/firmware.c` (freestanding), `build/link.ld` (memory map), and
cross-compiles them with clang + lld into:

- `build/firmware.elf` — the linked image, with a Cortex-M vector table
  (initial stack pointer + reset handler) at the flash origin.
- `build/firmware.bin` — the raw binary to flash (`probe-rs`, `openocd`,
  `st-flash`, …).

Supported `--mcu`: `cortex-m0`, `cortex-m3`, `cortex-m4` (default), `riscv32`.

## Language

Embedded Maca is a freestanding subset — `int` is a 32-bit word, functions
compile to plain C functions, `main()` is the firmware entry (called by the
reset handler after `.data`/`.bss` init).

Hardware access is through intrinsics:

| Maca | meaning |
|---|---|
| `mmio_write(addr, val)` / `mmio_read(addr)` | 32-bit volatile store / load |
| `set_bits(addr, mask)` / `clear_bits(addr, mask)` / `toggle_bits(addr, mask)` | atomic-ish read-modify-write |
| `bit(n)` | `1 << n` |
| `shl(x,n)` / `shr(x,n)` / `bit_or(a,b)` / `bit_and(a,b)` | bit ops |
| `delay(n)` | calibrated busy-wait; `nop()` — one `nop` |
| `for _ in forever() { … }` | the main super-loop |

Numeric literals take hex/binary/octal with `_` separators: `0x4002_0C00`.

## Verified

`set_bits(odr, bit(12))` compiles to a real Cortex-M read-modify-write
(`ldr` / `orr #0x1000` / `str`), and the emitted vector table boots with the
correct stack top and Thumb reset vector — checked in
`crates/driver/tests/embedded.rs`. (Running on real silicon or QEMU is outside
this repo's CI; the image is a valid Cortex-M binary ready to flash.)
