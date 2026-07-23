//! maca-lsp: language-server features as pure functions (the tower-lsp stdio
//! transport is a thin wrapper, future work). Exposes diagnostics, hover, and
//! config-mode NixOS option completion — enough for editor smoke tests.

use maca_parser::ast::*;
use maca_parser::parse;

/// Diagnostics at a glance (parse + type/effect), as `line:col`-free messages.
pub fn diagnostics(src: &str, config: bool) -> Vec<String> {
    let parsed = parse(src);
    if !parsed.errors.is_empty() {
        return parsed.errors.clone();
    }
    let m = if config { maca_core::Mode::Config } else { maca_core::Mode::Program };
    maca_core::check(&parsed.module, m).iter().map(|d| format!("{:?}: {}", d.kind, d.msg)).collect()
}

/// Hover: the signature/type of the identifier at `byte_offset`, if known.
pub fn hover(src: &str, byte_offset: usize) -> Option<String> {
    let word = word_at(src, byte_offset)?;
    let parsed = parse(src);
    for item in &parsed.module.items {
        match item {
            Stmt::Fn(f) if f.name == word => return Some(fn_sig(f)),
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target {
                    if *n == word {
                        return Some(format!("{word}: value"));
                    }
                }
            }
            _ => {}
        }
    }
    match word.as_str() {
        "int" | "float" | "str" | "bool" | "bytes" => Some(format!("builtin type `{word}`")),
        _ => Some(format!("`{word}`")),
    }
}

/// Whether a source should be checked in config (Nix) mode — heuristic: it
/// imports nixpkgs or drives a top-level NixOS/home option namespace.
pub fn is_config_source(src: &str) -> bool {
    src.lines().any(|l| {
        let t = l.trim();
        t.starts_with("import nixpkgs")
            || t.starts_with("system.")
            || t.starts_with("services.")
            || t.starts_with("networking.")
            || t.starts_with("user.")
            || t.starts_with("dev.")
    })
}

/// LSP (0-based line, character) → byte offset into `src`. Character is treated
/// as a UTF-8 column (exact for ASCII source, which Maca overwhelmingly is).
pub fn position_to_offset(src: &str, line: usize, character: usize) -> usize {
    let mut off = 0;
    for (i, l) in src.split_inclusive('\n').enumerate() {
        if i == line {
            // clamp character to the line's content length (excl. newline)
            let content = l.strip_suffix('\n').unwrap_or(l);
            let col = character.min(content.len());
            return off + col;
        }
        off += l.len();
    }
    off.min(src.len())
}

/// The identifier prefix immediately before `offset` (for completion).
pub fn prefix_at(src: &str, offset: usize) -> String {
    let bytes = src.as_bytes();
    let mut start = offset.min(bytes.len());
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'.';
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    src[start..offset.min(src.len())].to_string()
}

/// Program-mode completion: user-defined top-level function names plus the
/// builtin type names, filtered by `prefix`.
pub fn program_completions(src: &str, prefix: &str) -> Vec<String> {
    let parsed = parse(src);
    let mut names: Vec<String> = parsed
        .module
        .items
        .iter()
        .filter_map(|it| match it {
            Stmt::Fn(f) => Some(f.name.clone()),
            _ => None,
        })
        .collect();
    names.extend(["int", "float", "str", "bool", "bytes"].iter().map(|s| s.to_string()));
    names.into_iter().filter(|n| n.starts_with(prefix)).collect()
}

/// Config-mode completion: NixOS option namespaces matching `prefix`.
pub fn config_completions(prefix: &str) -> Vec<String> {
    const ROOTS: &[&str] = &[
        "networking", "system", "services", "users", "user", "environment", "programs", "fonts",
        "boot", "hardware", "security", "nix", "systemd", "i18n", "time", "xdg", "home", "console",
    ];
    ROOTS.iter().filter(|r| r.starts_with(prefix)).map(|r| r.to_string()).collect()
}

fn fn_sig(f: &FnDef) -> String {
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let ty = p.ty.as_ref().map(ty_str).unwrap_or_else(|| "any".into());
            format!("{}{}: {ty}", if p.variadic { "..." } else { "" }, p.name)
        })
        .collect();
    let ret = f.ret.as_ref().map(ty_str).unwrap_or_else(|| "()".into());
    format!("{}({}) -> {ret}", f.name, params.join(", "))
}

fn ty_str(t: &Type) -> String {
    match t {
        Type::Name(segs) => segs.join("."),
        Type::Array(t) => format!("{}[]", ty_str(t)),
        Type::Opt(t) => format!("{}?", ty_str(t)),
        Type::Apply(h, args) => {
            format!("{} {}", ty_str(h), args.iter().map(ty_str).collect::<Vec<_>>().join(" "))
        }
        Type::Paren(t) => format!("({})", ty_str(t)),
    }
}

fn word_at(src: &str, off: usize) -> Option<String> {
    let bytes = src.as_bytes();
    if off > bytes.len() {
        return None;
    }
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut start = off.min(bytes.len());
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = off.min(bytes.len());
    while end < bytes.len() && is_word(bytes[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some(src[start..end].to_string())
}
