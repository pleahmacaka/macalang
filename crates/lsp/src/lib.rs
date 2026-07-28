//! maca-lsp: language-server features as pure functions over `.maca` source —
//! located diagnostics, hover, completion, document symbols, and go-to-
//! definition. The stdio JSON-RPC transport lives in `main.rs`.

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

/// A diagnostic with a byte span into `src`, so the editor can squiggle the
/// offending code instead of anchoring everything at the top of the file.
pub struct Located {
    pub start: usize,
    pub end: usize,
    pub message: String,
}

/// Diagnostics with real byte spans. Parse/lex errors carry their span in the
/// message (`parse (a, b): …`); type/effect diagnostics are anchored on the
/// first back-quoted name found in the source code (skipping comments/strings),
/// falling back to the file start.
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
            let (start, end) = first_backtick(&d.msg)
                .and_then(|name| code_word_span(src, name))
                .unwrap_or((0, 1));
            Located {
                start,
                end,
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

/// Top-level definitions in source order (functions, type declarations, and
/// bindings), each with the byte span of its name.
pub fn document_symbols(src: &str) -> Vec<Symbol> {
    let parsed = parse(src);
    let mut out = Vec::new();
    for item in &parsed.module.items {
        let (name, kind) = match item {
            Stmt::Fn(f) => (f.name.clone(), 12u8),
            Stmt::Alias { name, .. } => (name.clone(), 23),
            Stmt::Bind(b) => match &b.target {
                Expr::Ident(n) => {
                    // a Capitalized binding to a sum/record is a type declaration
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

/// The definition span of the identifier at `byte_offset`, if it names a
/// top-level function, type, or binding.
pub fn definition(src: &str, byte_offset: usize) -> Option<(usize, usize)> {
    let word = word_at(src, byte_offset)?;
    document_symbols(src)
        .into_iter()
        .find(|s| s.name == word)
        .map(|s| (s.start, s.end))
}

/// Byte offset → (0-based line, 0-based UTF-16 character) — the inverse of
/// `position_to_offset`, for turning spans into LSP ranges.
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

/// Pull a `(start, end)` byte span out of a flattened parse/lex error like
/// `parse (12, 15): unexpected token` or `lex (3, 4): …`.
fn span_in_message(msg: &str) -> Option<(usize, usize)> {
    let open = msg.find('(')?;
    let close = open + msg[open..].find(')')?;
    let (a, b) = msg[open + 1..close].split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// The first `` `name` `` token in a message.
fn first_backtick(msg: &str) -> Option<&str> {
    let a = msg.find('`')? + 1;
    let rest = &msg[a..];
    Some(&rest[..rest.find('`')?])
}

/// First whole-word occurrence of `name` in *code* (skipping `//` comments and
/// `"…"` strings), as a byte span — so a marker anchors on the real identifier.
fn code_word_span(src: &str, name: &str) -> Option<(usize, usize)> {
    code_word_spans(src, name).into_iter().next()
}

/// Every whole-word occurrence of `name` in *code* — comments and string
/// literals are skipped, so a rename never touches prose. Byte spans, in source
/// order.
fn code_word_spans(src: &str, name: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if name.is_empty() {
        return out;
    }
    let b = src.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let n = name.len();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            _ => {
                if b[i..].starts_with(name.as_bytes())
                    && (i == 0 || !is_word(b[i - 1]))
                    && (i + n >= b.len() || !is_word(b[i + n]))
                {
                    out.push((i, i + n));
                    i += n;
                } else {
                    i += 1;
                }
            }
        }
    }
    out
}

/// Every reference to the symbol under the cursor — its definition and every
/// use — as byte spans. Powers `textDocument/references` and, with a new name,
/// `textDocument/rename`.
pub fn references(src: &str, byte_offset: usize) -> Vec<(usize, usize)> {
    match word_at(src, byte_offset) {
        Some(word) => code_word_spans(src, &word),
        None => Vec::new(),
    }
}

/// Byte span of a top-level definition's name — a line starting (at column 0)
/// with `name` followed by a non-identifier char.
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

/// LSP (0-based line, character) → byte offset into `src`. `character` is a
/// UTF-16 code-unit index (the LSP/Monaco convention), mapped to a byte offset
/// that always lands on a char boundary — even for multibyte (CJK/emoji) source.
pub fn position_to_offset(src: &str, line: usize, character: usize) -> usize {
    let mut off = 0;
    for (i, l) in src.split_inclusive('\n').enumerate() {
        if i == line {
            let content = l.strip_suffix('\n').unwrap_or(l);
            // consume `character` UTF-16 code units, returning the byte offset
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

/// The identifier prefix immediately before `offset` (for completion). Snaps
/// `offset` down to a char boundary first, so it never panics on multibyte
/// source (the running LSP feeds arbitrary offsets here on every keystroke).
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
    const ROOTS: &[&str] = &[
        "networking",
        "system",
        "services",
        "users",
        "user",
        "environment",
        "programs",
        "fonts",
        "boot",
        "hardware",
        "security",
        "nix",
        "systemd",
        "i18n",
        "time",
        "xdg",
        "home",
        "console",
    ];
    ROOTS
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
