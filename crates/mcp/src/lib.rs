//! maca-mcp: LLM-native tools for the generate → verify → fix loop.
//!
//! These are the tool implementations behind the MCP server:
//!   * `maca.check(code)`  → diagnostics (parse + type/effect)
//!   * `maca.fmt(code)`    → canonical formatting
//!   * `maca.stdlib(query)`→ prelude/stdlib signatures
//!   * `maca.options(q)`   → known NixOS option namespaces
//!   * `maca.spec(section)`→ spec text
//!
//! The stdio JSON-RPC transport (`src/main.rs`) is a thin wrapper over these.

use maca_core::{check as core_check, DiagKind, Mode};
use maca_parser::{parse, print_module};

/// `maca.check` — return human-readable diagnostics. Empty = clean.
pub fn check(code: &str, config: bool) -> Vec<String> {
    let parsed = parse(code);
    if !parsed.errors.is_empty() {
        return parsed.errors.iter().map(|e| format!("parse: {e}")).collect();
    }
    let mode = if config { Mode::Config } else { Mode::Program };
    core_check(&parsed.module, mode)
        .iter()
        .map(|d| format!("{}: {}", kind_name(d.kind), d.msg))
        .collect()
}

fn kind_name(k: DiagKind) -> &'static str {
    match k {
        DiagKind::TypeMismatch => "type-mismatch",
        DiagKind::NonExhaustive => "non-exhaustive",
        DiagKind::EffectInConfig => "effect-in-config",
        DiagKind::UnknownOption => "unknown-option",
        DiagKind::Undefined => "undefined",
    }
}

/// `maca.fmt` — canonical formatting (parse → print). Errs on a bad parse.
pub fn fmt(code: &str) -> Result<String, Vec<String>> {
    let parsed = parse(code);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }
    Ok(print_module(&parsed.module))
}

/// `maca.stdlib` — prelude/stdlib signatures matching a query substring.
pub fn stdlib(query: &str) -> Vec<String> {
    const SIGS: &[&str] = &[
        "info(s: str) -> () / <io>",
        "warn(s: str) -> () / <io>",
        "err(s: str) -> () / <io>",
        "debug(s: str) -> () / <io>",
        "input() -> str / <io>",
        "int(x: any) -> int",
        "str(x: any) -> str",
        "len(xs: t[]) -> int",
        "json.encode(x: t) -> str",
        "json.decode(s: str) -> t",
        "dirs.data -> Path",
        "Path.read() -> str / <io>",
        "Path.write(s: str) -> () / <io>",
        "Path.exists() -> bool / <io>",
        "str[].join(sep: str) -> str",
        "int[].parallel(f) -> int[] / <async>",
    ];
    SIGS.iter().filter(|s| query.is_empty() || s.contains(query)).map(|s| s.to_string()).collect()
}

/// `maca.options` — known NixOS option namespaces matching a prefix.
pub fn options(prefix: &str) -> Vec<String> {
    NIXOS_ROOTS.iter().filter(|r| r.starts_with(prefix)).map(|r| r.to_string()).collect()
}

pub const NIXOS_ROOTS: &[&str] = &[
    "networking", "system", "services", "users", "user", "environment", "programs", "fonts",
    "boot", "hardware", "security", "nix", "nixpkgs", "virtualisation", "systemd", "i18n", "time",
    "sound", "xdg", "home", "console", "powerManagement", "documentation", "location",
];

/// `maca.spec` — a short reference for a section name.
pub fn spec(section: &str) -> String {
    match section {
        "syntax" | "grammar" => "\
No `fn`/`type`/`Result`/`<>`. Field `:` = type, `=` = value, `Name { = }` = ctor.\n\
Functions: `f(x: T) -> R { body }` or `=> expr`. Variadic `...rest: T`.\n\
Ternary is spaced `c ? x : y`; error-propagate is attached `x?`. `fail e` raises.\n\
Bracketless comma lists; significant newlines; records newline/comma separated.\n\
Generics: lowercase type vars, applied by juxtaposition; postfix `T[]` / `T?`."
            .into(),
        "effects" => "Koka-style, inferred: io · net · os · async · exn. Config mode forces `<>`.".into(),
        "modes" => "General mode → native/BEAM/JS (`main`). Config mode → Nix (root module, pure `<>`).".into(),
        "types" => "Gradual · structural · row-polymorphic. HM + `any` boundary. Sums nominal, exhaustive.".into(),
        _ => "sections: syntax | effects | modes | types".into(),
    }
}
