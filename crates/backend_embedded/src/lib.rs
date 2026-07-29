//! maca-backend-embedded: lower Maca to **freestanding C** for bare-metal
//! microcontrollers (ARM Cortex-M / RISC-V), plus the startup code and linker
//! script needed to turn it into a firmware image.
//!
//! Embedded firmware is a small language subset — no heap, no OS, no stdlib —
//! driving hardware through **memory-mapped I/O**. Rather than fight the hosted
//! C backend's runtime coupling, this is a focused emitter over that subset:
//! `int` → `uint32_t`, functions, arithmetic, `if`, an infinite `for _ in
//! forever()` loop, and a small set of MMIO intrinsics.
//!
//! Intrinsics (recognised by call name):
//!   * `mmio_write(addr, val)` / `mmio_read(addr)` — 32-bit volatile access
//!   * `set_bits(addr, mask)` / `clear_bits(addr, mask)` / `toggle_bits(addr,mask)`
//!   * `bit(n)` = `1<<n`; `shl`/`shr`/`bit_or`/`bit_and`
//!   * `delay(n)` — calibrated busy-wait; `nop()` — a single `nop`
//!
//! `main()` is the firmware entry, invoked by the reset handler.

use maca_parser::ast::*;

/// A microcontroller target: how to drive clang + the memory map.
#[derive(Clone, Copy)]
pub struct Mcu {
    pub name: &'static str,
    pub triple: &'static str,
    pub cpu: &'static str,
    pub flash_origin: u64,
    pub flash_len_k: u32,
    pub ram_origin: u64,
    pub ram_len_k: u32,
}

impl Mcu {
    /// Resolve a `--mcu` name. Defaults to a generic Cortex-M4 (STM32F4-like).
    pub fn resolve(name: &str) -> Option<Mcu> {
        Some(match name {
            "cortex-m0" | "cortex-m0plus" => Mcu {
                name: "cortex-m0",
                triple: "thumbv6m-none-eabi",
                cpu: "cortex-m0",
                flash_origin: 0x0800_0000,
                flash_len_k: 64,
                ram_origin: 0x2000_0000,
                ram_len_k: 8,
            },
            "cortex-m3" => Mcu {
                name: "cortex-m3",
                triple: "thumbv7m-none-eabi",
                cpu: "cortex-m3",
                flash_origin: 0x0800_0000,
                flash_len_k: 256,
                ram_origin: 0x2000_0000,
                ram_len_k: 64,
            },
            "cortex-m4" | "" | "default" => Mcu {
                name: "cortex-m4",
                triple: "thumbv7em-none-eabi",
                cpu: "cortex-m4",
                flash_origin: 0x0800_0000,
                flash_len_k: 512,
                ram_origin: 0x2000_0000,
                ram_len_k: 128,
            },
            "riscv32" => Mcu {
                name: "riscv32",
                triple: "riscv32-none-elf",
                cpu: "generic-rv32",
                flash_origin: 0x2000_0000,
                flash_len_k: 512,
                ram_origin: 0x8000_0000,
                ram_len_k: 128,
            },
            _ => return None,
        })
    }
}

thread_local! {
    /// Constructs this target cannot honour, collected while lowering. A
    /// freestanding MCU has no allocator and no libc, so a good part of the
    /// language genuinely does not fit — but it has to be said by name at
    /// compile time, not turned into a plausible-looking `0u`.
    static PROBLEMS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn problem(msg: impl Into<String>) {
    PROBLEMS.with(|p| p.borrow_mut().push(msg.into()));
}

/// Emit freestanding C, or the list of constructs the target cannot honour.
/// The driver uses this so an unsupported construct is a clean error rather
/// than C that compiles and computes something else.
pub fn emit_c_checked(m: &Module) -> Result<String, Vec<String>> {
    PROBLEMS.with(|p| p.borrow_mut().clear());
    let out = emit_c(m);
    let problems = PROBLEMS.with(|p| p.borrow().clone());
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

/// The full firmware C translation unit: intrinsics, startup, user code.
pub fn emit_c(m: &Module) -> String {
    let mut out = String::new();
    out.push_str(PREAMBLE);
    out.push('\n');
    // top-level bindings → const globals (register addresses, masks, …)
    let mut had_const = false;
    for it in &m.items {
        if let Stmt::Bind(b) = it
            && let Expr::Ident(name) = &b.target
        {
            // A sum or record declaration is a binding too, and lowering one to
            // a `uint32_t` const yields C naming variants that do not exist.
            if matches!(
                b.value,
                Expr::Binary {
                    op: BinOp::Union,
                    ..
                }
            ) {
                problem(format!(
                    "`{name}` is a sum type; the embedded target has no tagged \
                     values — use integer constants"
                ));
                continue;
            }
            if matches!(b.value, Expr::Record(_)) {
                problem(format!(
                    "`{name}` is a record type; the embedded target has no \
                     structs — use separate values"
                ));
                continue;
            }
            out.push_str(&format!(
                "static const uint32_t {name} = {};\n",
                cexpr(&b.value)
            ));
            had_const = true;
        }
    }
    if had_const {
        out.push('\n');
    }
    for it in &m.items {
        if let Stmt::Fn(f) = it {
            out.push_str(&emit_fn(f));
            out.push('\n');
        }
    }
    out.push_str(STARTUP);
    out
}

/// The linker script placing the vector table in flash and RAM for `.data`/`.bss`.
pub fn linker_script(mcu: &Mcu) -> String {
    format!(
        "ENTRY(Reset_Handler)\n\
         MEMORY {{\n\
         \x20 FLASH (rx) : ORIGIN = 0x{:08X}, LENGTH = {}K\n\
         \x20 RAM  (rwx) : ORIGIN = 0x{:08X}, LENGTH = {}K\n\
         }}\n\
         _estack = ORIGIN(RAM) + LENGTH(RAM);\n\
         SECTIONS {{\n\
         \x20 .isr_vector : {{ KEEP(*(.isr_vector)) }} > FLASH\n\
         \x20 .text : {{ *(.text*) *(.rodata*) }} > FLASH\n\
         \x20 _sidata = LOADADDR(.data);\n\
         \x20 .data : {{ _sdata = .; *(.data*) . = ALIGN(4); _edata = .; }} > RAM AT> FLASH\n\
         \x20 .bss  : {{ _sbss = .; *(.bss* COMMON) . = ALIGN(4); _ebss = .; }} > RAM\n\
         }}\n",
        mcu.flash_origin, mcu.flash_len_k, mcu.ram_origin, mcu.ram_len_k
    )
}

const PREAMBLE: &str = r#"/* generated by maca --target embedded — freestanding, no libc */
#include <stdint.h>

static inline void maca_delay(uint32_t n) { while (n) { __asm__ volatile("nop"); n--; } }
"#;

const STARTUP: &str = r#"
/* ---- Cortex-M startup ---- */
extern uint32_t _sdata, _edata, _sidata, _sbss, _ebss, _estack;
void main(void);

void Reset_Handler(void) {
    uint32_t *src = &_sidata, *dst = &_sdata;
    while (dst < &_edata) *dst++ = *src++;   /* copy .data from flash */
    for (dst = &_sbss; dst < &_ebss;) *dst++ = 0; /* zero .bss */
    main();
    for (;;) { __asm__ volatile("wfi"); }    /* halt if main returns */
}

__attribute__((section(".isr_vector"), used))
void (* const g_vectors[])(void) = {
    (void (*)(void)) &_estack,   /* initial stack pointer */
    Reset_Handler,               /* reset */
};
"#;

fn emit_fn(f: &FnDef) -> String {
    let ret = if f.ret.is_some() { "uint32_t" } else { "void" };
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| format!("uint32_t {}", p.name))
        .collect();
    let ps = if params.is_empty() {
        "void".into()
    } else {
        params.join(", ")
    };
    let body = match &f.body {
        Some(FnBody::Block(stmts)) => cblock(stmts, ret != "void", 1),
        // `name() => { … }` parses as an expression body holding a block
        Some(FnBody::Expr(e)) if matches!(&**e, Expr::Block(_)) => {
            let Expr::Block(stmts) = &**e else {
                unreachable!()
            };
            cblock(stmts, ret != "void", 1)
        }
        Some(FnBody::Expr(e)) => {
            if ret == "void" {
                format!("    {};\n", cexpr(e))
            } else {
                format!("    return {};\n", cexpr(e))
            }
        }
        None => String::new(),
    };
    format!("{ret} {}({ps}) {{\n{body}}}\n", f.name)
}

fn cblock(stmts: &[Stmt], wants_value: bool, ind: usize) -> String {
    let pad = "    ".repeat(ind);
    let mut out = String::new();
    // names declared in this block: first `x =` declares (with a type), a later
    // `x =` reassigns (C forbids re-declaring in the same scope).
    let mut declared: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (i, s) in stmts.iter().enumerate() {
        let last = i + 1 == stmts.len();
        match s {
            Stmt::Bind(b) => match &b.target {
                Expr::Ident(n) => {
                    let decl = if declared.insert(n.clone()) {
                        "uint32_t "
                    } else {
                        ""
                    };
                    out.push_str(&format!("{pad}{decl}{n} = {};\n", cexpr(&b.value)));
                }
                // `p.f = v` and `xs[i] = v` are ordinary C stores. Emitting only
                // the right-hand side left `v;`, which C accepts in silence.
                Expr::Field { .. } | Expr::Index { .. } => {
                    out.push_str(&format!(
                        "{pad}{} = {};\n",
                        cexpr(&b.target),
                        cexpr(&b.value)
                    ));
                }
                other => {
                    problem(format!(
                        "cannot assign to {} on the embedded target",
                        describe(other)
                    ));
                }
            },
            Stmt::Expr(Expr::For { pat, iter, body }) => out.push_str(&cfor(pat, iter, body, ind)),
            Stmt::Expr(Expr::While { cond, body }) => {
                out.push_str(&format!(
                    "{pad}while ({}) {{\n{}{pad}}}\n",
                    cexpr(cond),
                    cblock(body, false, ind + 1)
                ));
            }
            Stmt::Expr(Expr::Break) => out.push_str(&format!("{pad}break;\n")),
            Stmt::Expr(Expr::Continue) => out.push_str(&format!("{pad}continue;\n")),
            Stmt::Expr(e @ Expr::If { .. }) => out.push_str(&cif(e, ind)),
            Stmt::Expr(e) => {
                if last && wants_value {
                    out.push_str(&format!("{pad}return {};\n", cexpr(e)));
                } else if !is_pure(e) {
                    out.push_str(&format!("{pad}{};\n", cexpr(e)));
                }
            }
            _ => {}
        }
    }
    out
}

fn cfor(pat: &Pattern, iter: &Expr, body: &[Stmt], ind: usize) -> String {
    let pad = "    ".repeat(ind);
    // `for _ in forever()` → an infinite loop
    if let Expr::Call { callee, .. } = iter
        && matches!(&**callee, Expr::Ident(n) if n == "forever")
    {
        return format!(
            "{pad}while (1) {{\n{}{pad}}}\n",
            cblock(body, false, ind + 1)
        );
    }
    // otherwise treat the iterator as a count: `for i in N` → 0..N
    let v = match pat {
        Pattern::Bind(n) => n.clone(),
        _ => "_i".into(),
    };
    format!(
        "{pad}for (uint32_t {v} = 0; {v} < ({}); {v}++) {{\n{}{pad}}}\n",
        cexpr(iter),
        cblock(body, false, ind + 1)
    )
}

fn cif(e: &Expr, ind: usize) -> String {
    let Expr::If { cond, then, els } = e else {
        return String::new();
    };
    let pad = "    ".repeat(ind);
    let mut out = format!(
        "{pad}if ({}) {{\n{}{pad}}}",
        cexpr(cond),
        cblock(then, false, ind + 1)
    );
    if let Some(els) = els {
        out.push_str(&format!(" else {{\n{}{pad}}}", cblock(els, false, ind + 1)));
    }
    out.push('\n');
    out
}

fn cexpr(e: &Expr) -> String {
    match e {
        Expr::Int(n) if *n >= 0 => format!("{n}u"),
        Expr::Int(n) => format!("{n}"),
        Expr::Bool(b) => (if *b { "1u" } else { "0u" }).into(),
        Expr::Ident(n) => n.clone(),
        Expr::Unit => "0u".into(),
        Expr::Unary { op, expr } => {
            let o = if matches!(op, UnOp::Not) { "!" } else { "-" };
            format!("{o}({})", cexpr(expr))
        }
        Expr::Binary { op, lhs, rhs } => {
            let o = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Mod => "%",
                BinOp::Shl => "<<",
                BinOp::Shr => ">>",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Lt => "<",
                BinOp::Gt => ">",
                BinOp::Le => "<=",
                BinOp::Ge => ">=",
                BinOp::And => "&&",
                BinOp::Or => "||",
                BinOp::Union => "|", // surface `|` as bit-or in embedded
                // `++` was reaching the `+` fallback, so a concatenation
                // silently became pointer arithmetic.
                BinOp::Concat => {
                    problem(
                        "`++` needs an allocator; the embedded target is \
                         freestanding, with no heap",
                    );
                    "+"
                }
                BinOp::Pipe => {
                    problem("`|>` is not lowered on the embedded target");
                    "+"
                }
            };
            format!("({} {o} {})", cexpr(lhs), cexpr(rhs))
        }
        Expr::Ternary { cond, then, els } => {
            format!("({} ? {} : {})", cexpr(cond), cexpr(then), cexpr(els))
        }
        Expr::Call { callee, args } => ccall(callee, args),
        Expr::Field { base, name } => format!("{}.{name}", cexpr(base)),
        Expr::Index { base, index } => format!("{}[{}]", cexpr(base), cexpr(index)),
        // `a = b` is an expression in C too.
        Expr::Assign { target, value } => {
            format!("({} = {})", cexpr(target), cexpr(value))
        }
        // Everything else used to become the literal `0u`, which C accepts
        // wherever a `uint32_t` is wanted — a `match` in firmware compiled to
        // the number zero. Name it instead.
        other => {
            problem(format!(
                "{} is not lowered on the embedded target",
                describe(other)
            ));
            "0u".into()
        }
    }
}

/// Name a construct the way the author wrote it, for a refusal message.
fn describe(e: &Expr) -> &'static str {
    match e {
        Expr::Match { .. } => "`match`",
        Expr::Float(_) => "a float literal (the target is integer-only)",
        Expr::Str(_) => "a string (the target has no allocator)",
        Expr::List(_) => "a list (the target has no allocator)",
        Expr::Record(_) | Expr::Ctor { .. } => "a record or sum value",
        Expr::Range { .. } => "a range in value position",
        Expr::Lambda { .. } => "a closure",
        Expr::Try(_) | Expr::Fail(_) | Expr::Reify(_) => "the error operators (`?`, `fail`)",
        Expr::Await(_) | Expr::Spawn(_) => "`await`/`spawn` (there is no scheduler)",
        Expr::With { .. } => "a record update",
        Expr::If { .. } => "`if` in value position",
        Expr::Block(_) => "a block in value position",
        _ => "this construct",
    }
}

fn ccall(callee: &Expr, args: &[Arg]) -> String {
    let a: Vec<String> = args.iter().map(|x| cexpr(arg_expr(x))).collect();
    let g = |i: usize| a.get(i).cloned().unwrap_or_else(|| "0u".into());
    let reg = |addr: &str| format!("(*(volatile uint32_t *)(uintptr_t)({addr}))");
    if let Expr::Ident(f) = callee {
        return match f.as_str() {
            "mmio_write" => format!("({} = (uint32_t)({}))", reg(&g(0)), g(1)),
            "mmio_read" => reg(&g(0)),
            "set_bits" => format!("({} |= (uint32_t)({}))", reg(&g(0)), g(1)),
            "clear_bits" => format!("({} &= ~(uint32_t)({}))", reg(&g(0)), g(1)),
            "toggle_bits" => format!("({} ^= (uint32_t)({}))", reg(&g(0)), g(1)),
            "bit" => format!("(1u << ({}))", g(0)),
            "shl" => format!("(({}) << ({}))", g(0), g(1)),
            "shr" => format!("(({}) >> ({}))", g(0), g(1)),
            "bit_or" => format!("(({}) | ({}))", g(0), g(1)),
            "bit_and" => format!("(({}) & ({}))", g(0), g(1)),
            "delay" => format!("maca_delay({})", g(0)),
            "nop" => "__asm__ volatile(\"nop\")".into(),
            _ => format!("{f}({})", a.join(", ")),
        };
    }
    format!("{}({})", cexpr(callee), a.join(", "))
}

fn is_pure(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Int(_) | Expr::Bool(_) | Expr::Ident(_) | Expr::Unit
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(src: &str) -> String {
        let p = maca_parser::parse(src);
        assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
        emit_c(&p.module)
    }

    #[test]
    fn mmio_and_forever() {
        let j = c(
            "blink() {\n    for _ in forever() {\n        set_bits(0x48000014, bit(5))\n        delay(1000)\n    }\n}\n\nmain() {\n    blink()\n}\n",
        );
        assert!(j.contains("while (1)"), "{j}");
        assert!(j.contains("volatile uint32_t *"), "{j}");
        assert!(j.contains("|= (uint32_t)((1u << (5u)))"), "{j}");
        assert!(j.contains("maca_delay(1000u)"), "{j}");
        assert!(j.contains("Reset_Handler"), "{j}");
        assert!(j.contains(".isr_vector"), "{j}");
    }

    #[test]
    fn mmio_write_and_read() {
        let j = c(
            "cfg(a: int, v: int) => mmio_write(a, v)\nget(a: int) -> int => mmio_read(a)\nmain() => cfg(0x40020000, 1)\n",
        );
        assert!(j.contains("void cfg(uint32_t a, uint32_t v)"), "{j}");
        assert!(j.contains("uint32_t get(uint32_t a)"), "{j}");
        assert!(j.contains("= (uint32_t)(v))"), "{j}");
    }

    #[test]
    fn mcu_resolves() {
        assert_eq!(
            Mcu::resolve("cortex-m4").unwrap().triple,
            "thumbv7em-none-eabi"
        );
        assert_eq!(Mcu::resolve("").unwrap().cpu, "cortex-m4");
        assert!(Mcu::resolve("cortex-m0").unwrap().triple.contains("v6m"));
        assert!(Mcu::resolve("nonsense").is_none());
        assert!(linker_script(&Mcu::resolve("cortex-m4").unwrap()).contains("0x08000000"));
    }
}
