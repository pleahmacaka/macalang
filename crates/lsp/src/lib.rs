pub mod actions;
pub mod binding;
pub mod workspace;

pub use actions::{Action, Edit, apply_edits, code_actions};
pub use binding::{Binding, Scope};

use maca_parser::ast::*;
use maca_parser::parse;

/// Diagnostics at a glance (parse + type/effect), as `line:col`-free messages.
pub fn diagnostics(src: &str, config: bool) -> Vec<String> {
    let parsed = parse(src);
    if !parsed.errors.is_empty() {
        return parsed.errors.clone();
    }
    let m = if config {
        maca_core::Mode::Config
    } else {
        maca_core::Mode::Program
    };
    maca_core::check(&parsed.module, m)
        .iter()
        .map(|d| format!("{:?}: {}", d.kind, d.msg))
        .collect()
}

/// A diagnostic with a byte span into `src`.
pub struct Located {
    pub start: usize,
    pub end: usize,
    pub message: String,
}

/// Diagnostics with real byte spans.
pub fn diagnostics_located(src: &str, config: bool) -> Vec<Located> {
    let parsed = parse(src);
    if !parsed.errors.is_empty() {
        return parsed
            .errors
            .iter()
            .map(|m| {
                let (start, end) = span_in_message(m).unwrap_or((0, 1));
                Located {
                    start,
                    end,
                    message: m.clone(),
                }
            })
            .collect();
    }
    let mode = if config {
        maca_core::Mode::Config
    } else {
        maca_core::Mode::Program
    };
    maca_core::check(&parsed.module, mode)
        .iter()
        .map(|d| {
            let span = maca_core::resolve_span(src, d);
            Located {
                start: span.start,
                end: span.end,
                message: format!("{:?}: {}", d.kind, d.msg),
            }
        })
        .collect()
}

/// A top-level definition for the document outline / go-to-definition.
pub struct Symbol {
    pub name: String,
    /// LSP `SymbolKind`: 12 = Function, 23 = Struct (a type), 13 = Variable.
    pub kind: u8,
    pub start: usize,
    pub end: usize,
}

/// Top-level definitions in source order (functions, type declarations, and bindings), each with the byte span of its name.
pub fn document_symbols(src: &str) -> Vec<Symbol> {
    let parsed = parse(src);
    let mut out = Vec::new();
    for item in &parsed.module.items {
        let (name, kind) = match item {
            Stmt::Fn(f) => (f.name.clone(), 12u8),
            Stmt::Alias { name, .. } => (name.clone(), 23),
            Stmt::Bind(b) => match &b.target {
                Expr::Ident(n) => {
                    let is_type = n.chars().next().is_some_and(|c| c.is_uppercase())
                        && matches!(
                            &b.value,
                            Expr::Record(_) | Expr::Binary { .. } | Expr::Ctor { .. }
                        );
                    (n.clone(), if is_type { 23 } else { 13 })
                }
                _ => continue,
            },
            _ => continue,
        };
        if let Some((start, end)) = top_level_span(src, &name) {
            out.push(Symbol {
                name,
                kind,
                start,
                end,
            });
        }
    }
    out
}

/// The definition span of the identifier at `byte_offset`, if it names a top-level function, type, or binding.
pub fn definition(src: &str, byte_offset: usize) -> Option<(usize, usize)> {
    let word = word_at(src, byte_offset)?;
    document_symbols(src)
        .into_iter()
        .find(|s| s.name == word)
        .map(|s| (s.start, s.end))
}

/// Byte offset → (0-based line, 0-based UTF-16 character), the inverse of `position_to_offset`, for turning spans into LSP ranges.
pub fn offset_to_position(src: &str, byte: usize) -> (usize, usize) {
    let byte = byte.min(src.len());
    let mut line = 0usize;
    let mut col = 0usize;
    for (i, ch) in src.char_indices() {
        if i >= byte {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16();
        }
    }
    (line, col)
}

/// Pull a `(start, end)` byte span out of a flattened parse/lex error like `parse (12, 15): unexpected token` or `lex (3, 4): …`.
fn span_in_message(msg: &str) -> Option<(usize, usize)> {
    let open = msg.find('(')?;
    let close = open + msg[open..].find(')')?;
    let (a, b) = msg[open + 1..close].split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// Every reference to the binding under the cursor, its definition and every use alike, as byte spans.
pub fn references(src: &str, byte_offset: usize) -> Vec<(usize, usize)> {
    match binding::resolve(src, byte_offset) {
        Some(b) => binding::spans(src, &b),
        None => Vec::new(),
    }
}

/// Byte span of a top-level definition's name.
fn top_level_span(src: &str, name: &str) -> Option<(usize, usize)> {
    let mut off = 0;
    for line in src.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix(name)
            && rest
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '.')
        {
            return Some((off, off + name.len()));
        }
        off += line.len();
    }
    None
}

/// Signature help for the call the cursor sits inside.
pub fn signature_help(src: &str, byte_offset: usize) -> Option<(String, Vec<String>, usize)> {
    let b = src.as_bytes();
    let off = byte_offset.min(b.len());
    let mut depth = 0i32;
    let mut commas = 0usize;
    let mut i = off;
    let open = loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        match b[i] {
            b')' => depth += 1,
            b'(' if depth == 0 => break i,
            b'(' => depth -= 1,
            b',' if depth == 0 => commas += 1,
            b'\n' if depth == 0 => return None,
            _ => {}
        }
    };
    let name = word_at(src, open)?;
    let f = parse(src)
        .module
        .items
        .into_iter()
        .find_map(|it| match it {
            Stmt::Fn(f) if f.name == name => Some(f),
            _ => None,
        })?;
    let labels: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let ty = p.ty.as_ref().map(ty_str).unwrap_or_else(|| "any".into());
            format!("{}{}: {ty}", if p.variadic { "..." } else { "" }, p.name)
        })
        .collect();
    let active = commas.min(labels.len().saturating_sub(1));
    Some((fn_sig(&f), labels, active))
}

/// Hover: the signature/type of the identifier at `byte_offset`, if known.
pub fn hover(src: &str, byte_offset: usize) -> Option<String> {
    let word = word_at(src, byte_offset)?;
    let parsed = parse(src);
    for item in &parsed.module.items {
        match item {
            Stmt::Fn(f) if f.name == word => return Some(fn_sig(f)),
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target
                    && *n == word
                {
                    return Some(format!("{word}: value"));
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

/// Whether a source should be checked in config (Nix) mode.
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

/// LSP (0-based line, character) → byte offset into `src`.
pub fn position_to_offset(src: &str, line: usize, character: usize) -> usize {
    let mut off = 0;
    for (i, l) in src.split_inclusive('\n').enumerate() {
        if i == line {
            let content = l.strip_suffix('\n').unwrap_or(l);
            let mut u16s = 0usize;
            for (b, ch) in content.char_indices() {
                if u16s >= character {
                    return off + b;
                }
                u16s += ch.len_utf16();
            }
            return off + content.len();
        }
        off += l.len();
    }
    off.min(src.len())
}

/// The identifier prefix immediately before `offset` (for completion).
pub fn prefix_at(src: &str, offset: usize) -> String {
    let bytes = src.as_bytes();
    let mut end = offset.min(src.len());
    while end > 0 && !src.is_char_boundary(end) {
        end -= 1;
    }
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'.';
    let mut start = end;
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    src[start..end].to_string()
}

/// Program-mode completion: user-defined top-level function names plus the builtin type names, filtered by `prefix`.
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
    names.extend(
        ["int", "float", "str", "bool", "bytes"]
            .iter()
            .map(|s| s.to_string()),
    );
    names
        .into_iter()
        .filter(|n| n.starts_with(prefix))
        .collect()
}

/// Config-mode completion: NixOS option namespaces matching `prefix`.
pub fn config_completions(prefix: &str) -> Vec<String> {
    maca_core::NIXOS_ROOTS
        .iter()
        .filter(|r| r.starts_with(prefix))
        .map(|r| r.to_string())
        .collect()
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
        Type::Fn(ps, r) => {
            let ps: Vec<String> = ps.iter().map(ty_str).collect();
            format!("({}) -> {}", ps.join(", "), ty_str(r))
        }
        Type::Apply(h, args) => {
            format!(
                "{} {}",
                ty_str(h),
                args.iter().map(ty_str).collect::<Vec<_>>().join(" ")
            )
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
