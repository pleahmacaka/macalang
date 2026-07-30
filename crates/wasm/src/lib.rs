//! maca-wasm: a tiny `wasm32-unknown-unknown` surface over the front-end +
//! emitters, for the browser playground.
//!
//! No `wasm-bindgen`: the ABI is three raw exports over linear memory so the
//! `.wasm` builds with a plain `cargo build --target wasm32-unknown-unknown`
//! and loads with `WebAssembly.instantiate` alone (no JS glue, no toolchain).
//!
//!   * `alloc(len) -> ptr` — reserve `len` bytes for the caller to fill
//!   * `dealloc(ptr, len)` — release a buffer returned by `run`
//!   * `run(ptr, len, mode) -> (ptr<<32)|len` — parse + check + emit, returning
//!     a UTF-8 JSON blob the caller reads out of memory and then frees.
//!
//! `mode`: 0 = program (native/JS), 1 = config (Nix).
//!
//! The packing is `wasm32`-only: it folds a 32-bit pointer into the high half of
//! a `u64`, so a 64-bit host cannot call the exports. [`compile_json`] and
//! [`lsp_json`] are the same work as plain Rust functions, which is what a
//! native test asserts the JSON contract against.

use maca_core::{DiagKind, Mode};
use maca_parser::ast::*;

/// The tree-walking interpreter the playground runs programs with. Public so a
/// native test can check it against the answers the C backend gives: it is a
/// third implementation of Maca's semantics, and nothing else compares them.
pub mod interp;

/// Hand ownership of `bytes` (as an exact-length boxed slice) to the caller and
/// return `(ptr << 32) | len`. Exact length matters: [`dealloc`] frees using
/// `len` as the allocation size, so the box must be sized exactly `len`.
fn leak_bytes(bytes: Vec<u8>) -> u64 {
    let boxed: Box<[u8]> = bytes.into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed) as *mut u8;
    ((ptr as u64) << 32) | (len as u64)
}

/// Reserve `len` bytes and hand back the pointer. The caller writes source
/// bytes here before calling [`run`].
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let boxed: Box<[u8]> = vec![0u8; len].into_boxed_slice();
    Box::into_raw(boxed) as *mut u8
}

/// Free a buffer previously produced by [`alloc`] or returned by [`run`].
///
/// # Safety
/// `ptr`/`len` must come from a matching `alloc`/`run` and be freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        let slice = core::ptr::slice_from_raw_parts_mut(ptr, len);
        let _ = unsafe { Box::from_raw(slice) };
    }
}

/// Parse, check, and emit `src`. Returns `(out_ptr << 32) | out_len` locating a
/// leaked UTF-8 JSON string in linear memory; the caller reads it and calls
/// [`dealloc`].
///
/// # Safety
/// `ptr`/`len` must describe a valid UTF-8 buffer from [`alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn run(ptr: *const u8, len: usize, mode: u32) -> u64 {
    let src = unsafe { core::slice::from_raw_parts(ptr, len) };
    let src = core::str::from_utf8(src).unwrap_or("");
    let json = compile_json(src, mode);
    leak_bytes(json.into_bytes())
}

/// Version string, for the playground footer. `(ptr<<32)|len`.
#[unsafe(no_mangle)]
pub extern "C" fn version() -> u64 {
    leak_bytes(env!("CARGO_PKG_VERSION").as_bytes().to_vec())
}

/// LSP hover at `off` (byte offset): the signature/type of the identifier under
/// the caret, or "" if none. Reuses the same analysis as the native `maca-lsp`.
/// `(ptr<<32)|len`.
///
/// # Safety
/// `ptr`/`len` must describe a valid UTF-8 buffer from [`alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hover(ptr: *const u8, len: usize, off: usize) -> u64 {
    let src = unsafe { core::slice::from_raw_parts(ptr, len) };
    let src = core::str::from_utf8(src).unwrap_or("");
    let h = maca_lsp::hover(src, off.min(src.len())).unwrap_or_default();
    leak_bytes(h.into_bytes())
}

/// Everything the language server can say about the caret in one call: hover,
/// signature help, the definition site, and every reference to the binding.
/// `(ptr<<32)|len` locating the JSON described by [`lsp_json`].
///
/// One export rather than four because the editor asks all four questions about
/// the same caret, and each separate export would re-lex and re-parse the file.
///
/// # Safety
/// `ptr`/`len` must describe a valid UTF-8 buffer from [`alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsp(ptr: *const u8, len: usize, off: usize) -> u64 {
    let src = unsafe { core::slice::from_raw_parts(ptr, len) };
    let src = core::str::from_utf8(src).unwrap_or("");
    leak_bytes(lsp_json(src, off.min(src.len())).into_bytes())
}

/// The language-server answers for one caret position, as JSON:
///
/// ```json
/// { "hover": "fib(n: int) -> int",
///   "signature": { "label": "…", "params": ["n: int"], "active": 0 },
///   "definition": { "line": 3, "col": 1, "endLine": 3, "endCol": 4 },
///   "references": [ { "line": 3, "col": 1, "endLine": 3, "endCol": 4 } ] }
/// ```
///
/// Positions are Monaco's convention (1-based, columns in UTF-16 units) so the
/// editor can use them without a second conversion; `signature` and
/// `definition` are `null` when there is nothing to say.
pub fn lsp_json(src: &str, off: usize) -> String {
    let mut out = String::from("{\"hover\":");
    push_json_str(&mut out, &maca_lsp::hover(src, off).unwrap_or_default());

    out.push_str(",\"signature\":");
    match maca_lsp::signature_help(src, off) {
        Some((label, params, active)) => {
            out.push_str("{\"label\":");
            push_json_str(&mut out, &label);
            out.push_str(",\"params\":[");
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_json_str(&mut out, p);
            }
            out.push_str(&format!("],\"active\":{active}}}"));
        }
        None => out.push_str("null"),
    }

    out.push_str(",\"definition\":");
    match maca_lsp::definition(src, off) {
        Some(span) => out.push_str(&span_json(src, span)),
        None => out.push_str("null"),
    }

    out.push_str(",\"references\":[");
    for (i, span) in maca_lsp::references(src, off).iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&span_json(src, *span));
    }
    out.push_str("]}");
    out
}

/// A byte span as a Monaco range object.
fn span_json(src: &str, (start, end): (usize, usize)) -> String {
    let (line, col) = line_col(src, start);
    let (end_line, end_col) = line_col(src, end);
    format!("{{\"line\":{line},\"col\":{col},\"endLine\":{end_line},\"endCol\":{end_col}}}")
}

fn mode_of(m: u32) -> Mode {
    match m {
        1 => Mode::Config,
        _ => Mode::Program,
    }
}

fn kind_str(k: DiagKind) -> &'static str {
    match k {
        DiagKind::TypeMismatch => "TypeMismatch",
        DiagKind::NonExhaustive => "NonExhaustive",
        DiagKind::EffectInConfig => "EffectInConfig",
        DiagKind::UnknownOption => "UnknownOption",
        DiagKind::Immutable => "Immutable",
        DiagKind::UndefinedName => "UndefinedName",
    }
}

/// The playground's whole result, as JSON (built by hand to keep deps zero).
///
/// ```json
/// { "parseErrors": ["…"],
///   "diagnostics": [{"kind":"TypeMismatch","msg":"…"}],
///   "outputs": {"C":"…"} | {"JS":"…","HTML":"…","CSS":"…"} | {"Nix":"…"},
///   "limits": {"C": ["`on:click` needs a live DOM — …"]} }
/// ```
///
/// A target appears in `outputs` when it lowered the whole module and in
/// `limits` when it refused a construct by name. Never both: code a backend
/// disowned is code that would not compile, so the playground must not show it
/// as if it would.
pub fn compile_json(src: &str, mode: u32) -> String {
    let parsed = maca_parser::parse(src);
    let mut out = String::from("{");

    // syntax-highlight tokens (from the real lexer, so they survive parse
    // errors) and the definition outline (from the parsed module).
    out.push_str("\"tokens\":");
    out.push_str(&tokens_json(src));
    out.push_str(",\"symbols\":");
    out.push_str(&symbols_json(src, &parsed.module));

    out.push_str(",\"parseErrors\":[");
    for (i, e) in parsed.errors.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, e);
    }
    out.push(']');

    // type/effect diagnostics
    let diags = if parsed.errors.is_empty() {
        maca_core::check(&parsed.module, mode_of(mode))
    } else {
        Vec::new()
    };
    out.push_str(",\"diagnostics\":[");
    for (i, d) in diags.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"kind\":");
        push_json_str(&mut out, kind_str(d.kind));
        out.push_str(",\"msg\":");
        push_json_str(&mut out, &d.msg);
        out.push('}');
    }
    out.push(']');

    // editor markers (Monaco-ready 1-based line/col spans) so the LSP can draw
    // squiggles on the offending token. Parse errors carry a real byte span;
    // semantic diagnostics anchor on the first `backticked` name in the message
    // (usually the offending identifier), falling back to the file start.
    out.push_str(",\"markers\":");
    out.push_str(&markers_json(src, &parsed.errors, &diags));

    // backend outputs (only when the source parses), and the constructs a
    // backend refused. Both C and Nix go through `emit_checked`: their
    // unchecked `emit` answers with plausible-looking code for a construct they
    // cannot lower, and a playground that shows it is teaching the wrong thing.
    let mut js_exports: Vec<String> = Vec::new();
    let mut limits: Vec<(&str, Vec<String>)> = Vec::new();
    out.push_str(",\"outputs\":{");
    if parsed.errors.is_empty() {
        match mode_of(mode) {
            Mode::Config => match maca_backend_nix::emit_checked(&parsed.module) {
                Ok(nix) => {
                    out.push_str("\"Nix\":");
                    push_json_str(&mut out, &nix);
                }
                Err(problems) => limits.push(("Nix", problems)),
            },
            Mode::Program => {
                match maca_backend_c::emit_checked(&parsed.module) {
                    Ok(c) => {
                        out.push_str("\"C\":");
                        push_json_str(&mut out, &c);
                        out.push(',');
                    }
                    Err(problems) => limits.push(("C", problems)),
                }
                let js = maca_backend_js::emit(&parsed.module);
                js_exports = js.exports.clone();
                out.push_str("\"JS\":");
                push_json_str(&mut out, &js.js);
                if !js.css.is_empty() {
                    out.push_str(",\"CSS\":");
                    push_json_str(&mut out, &js.css);
                }
            }
        }
    }
    out.push('}');

    out.push_str(",\"limits\":{");
    for (i, (target, problems)) in limits.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, target);
        out.push_str(":[");
        for (j, p) in problems.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            push_json_str(&mut out, p);
        }
        out.push(']');
    }
    out.push('}');

    // callable function names in the emitted JS (for `import` interop)
    out.push_str(",\"jsExports\":[");
    for (i, n) in js_exports.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, n);
    }
    out.push(']');

    // run the program (Program mode only) — captured stdout + an execution
    // profile, so the playground can show real output and where the time went.
    if parsed.errors.is_empty() && matches!(mode_of(mode), Mode::Program) {
        let r = interp::run(&parsed.module);
        out.push_str(",\"run\":{\"output\":");
        push_json_str(&mut out, &r.output);
        out.push_str(",\"error\":");
        match &r.error {
            Some(e) => push_json_str(&mut out, e),
            None => out.push_str("null"),
        }
        out.push_str(",\"exit\":");
        match r.exit {
            Some(n) => out.push_str(&n.to_string()),
            None => out.push_str("null"),
        }
        out.push_str(",\"profile\":{\"totalCalls\":");
        out.push_str(&r.profile.total_calls.to_string());
        out.push_str(",\"maxDepth\":");
        out.push_str(&r.profile.max_depth.to_string());
        out.push_str(",\"steps\":");
        out.push_str(&r.profile.steps.to_string());
        out.push_str(",\"truncated\":");
        out.push_str(if r.profile.truncated { "true" } else { "false" });
        out.push_str(",\"flameSvg\":");
        push_json_str(&mut out, &r.profile.flame_svg);
        out.push_str("}}");
    }

    out.push('}');
    out
}

/// A syntax-highlight kind for a token. Kept as small integers the JS bridge
/// maps to CSS classes: 0 punct, 1 keyword, 2 number, 3 string, 4 ident,
/// 5 type/constructor (capitalized), 6 operator.
fn tok_kind(t: &maca_lexer::Tok) -> u8 {
    use maca_lexer::Tok::*;
    match t {
        Int(_) | Float(_) => 2,
        StrOpen | StrText(_) | StrClose | Path(_) => 3,
        True | False | Const | As | If | Else | For | In | While | Break | Continue | Match
        | Import | With | Fail | Try | Alias => 1,
        Ident(s) => {
            if s.chars().next().is_some_and(|c| c.is_uppercase()) {
                5
            } else {
                4
            }
        }
        Eq | EqEq | NotEq | Bang | Lt | Gt | Le | Ge | Arrow | FatArrow | Plus | Minus | Star
        | Slash | Percent | Shl | Shr | PlusPlus | Bar | BarBar | PipeGt | AmpAmp | Question
        | QuestionPost | DotDot | Ellipsis => 6,
        _ => 0,
    }
}

/// Tokens as `[[start, len, kind], …]` (byte offsets). Newline/Eof layout
/// tokens are dropped; the JS highlighter fills the gaps (whitespace, comments)
/// itself.
fn tokens_json(src: &str) -> String {
    let lexed = maca_lexer::lex(src);
    let mut out = String::from("[");
    let mut first = true;
    for t in &lexed.tokens {
        if matches!(t.tok, maca_lexer::Tok::Newline | maca_lexer::Tok::Eof) {
            continue;
        }
        let (s, e) = t.span;
        if e <= s {
            continue;
        }
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&format!("[{},{},{}]", s, e - s, tok_kind(&t.tok)));
    }
    out.push(']');
    out
}

/// The definition outline: top-level functions, type declarations, named
/// values, and (config mode) option paths, each with a 1-based source line.
fn symbols_json(src: &str, module: &Module) -> String {
    let mut out = String::from("[");
    let mut first = true;
    for item in &module.items {
        let (name, kind, detail) = match item {
            Stmt::Fn(f) => (f.name.clone(), "fn", fn_signature(f)),
            Stmt::Bind(b) => match &b.target {
                Expr::Ident(n) if n.chars().next().is_some_and(|c| c.is_uppercase()) => {
                    (n.clone(), "type", type_detail(&b.value))
                }
                Expr::Ident(n) => (n.clone(), "value", String::new()),
                Expr::Field { .. } => (dotted_path(&b.target), "option", String::new()),
                _ => continue,
            },
            _ => continue,
        };
        if name.is_empty() {
            continue;
        }
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str("{\"name\":");
        push_json_str(&mut out, &name);
        out.push_str(",\"kind\":");
        push_json_str(&mut out, kind);
        out.push_str(",\"detail\":");
        push_json_str(&mut out, &detail);
        out.push_str(&format!(",\"line\":{}}}", find_line(src, &name)));
    }
    out.push(']');
    out
}

/// `name(p: T, …) -> R` for the outline detail column.
fn fn_signature(f: &FnDef) -> String {
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| {
            let ty = p.ty.as_ref().map(ty_name).unwrap_or_else(|| "any".into());
            format!("{}{}: {ty}", if p.variadic { "..." } else { "" }, p.name)
        })
        .collect();
    let ret = f.ret.as_ref().map(ty_name).unwrap_or_else(|| "()".into());
    format!("({}) -> {ret}", params.join(", "))
}

fn ty_name(t: &Type) -> String {
    match t {
        Type::Name(segs) => segs.join("."),
        Type::Array(t) => format!("{}[]", ty_name(t)),
        Type::Opt(t) => format!("{}?", ty_name(t)),
        Type::Fn(ps, r) => {
            let ps: Vec<String> = ps.iter().map(ty_name).collect();
            format!("({}) -> {}", ps.join(", "), ty_name(r))
        }
        Type::Apply(h, args) => {
            format!(
                "{} {}",
                ty_name(h),
                args.iter().map(ty_name).collect::<Vec<_>>().join(" ")
            )
        }
        Type::Paren(t) => format!("({})", ty_name(t)),
    }
}

/// A one-line summary of a type declaration's right-hand side: variant tags for
/// a sum, field names for a record.
fn type_detail(value: &Expr) -> String {
    fn ctors(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Binary {
                op: BinOp::Union | BinOp::Or,
                lhs,
                rhs,
            } => {
                ctors(lhs, out);
                ctors(rhs, out);
            }
            Expr::Ident(n) => out.push(n.clone()),
            Expr::Call { callee, .. } => {
                if let Expr::Ident(n) = callee.as_ref() {
                    out.push(n.clone());
                }
            }
            _ => {}
        }
    }
    match value {
        Expr::Record(fields) => {
            let names: Vec<String> = fields
                .iter()
                .filter_map(|f| match f {
                    Field::Type { name, .. } | Field::Value { name, .. } => Some(name.clone()),
                    Field::Shorthand(n) => Some(n.clone()),
                    _ => None,
                })
                .collect();
            format!("{{ {} }}", names.join(", "))
        }
        _ => {
            let mut cs = Vec::new();
            ctors(value, &mut cs);
            cs.join(" | ")
        }
    }
}

/// A dotted config target (`networking.hostName`) rendered back to a string.
fn dotted_path(e: &Expr) -> String {
    match e {
        Expr::Ident(n) => n.clone(),
        Expr::Field { base, name } => format!("{}.{name}", dotted_path(base)),
        _ => String::new(),
    }
}

/// First 1-based line whose trimmed start is `name` followed by a non-identifier
/// char (top-level defs begin at column 0), or 0 if not found. The AST is
/// span-free, so the outline recovers positions by matching the source.
fn find_line(src: &str, name: &str) -> usize {
    let matches = |s: &str| -> bool {
        s.strip_prefix(name).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '.')
        })
    };
    // Top-level defs begin at column 0. Prefer an un-indented match so a call
    // above the definition (indented) doesn't shadow it; fall back to a trimmed
    // match only if nothing starts at column 0.
    let mut fallback = 0;
    for (i, line) in src.lines().enumerate() {
        if matches(line) {
            return i + 1;
        }
        if fallback == 0 && matches(line.trim_start()) {
            fallback = i + 1;
        }
    }
    fallback
}

/// Byte offset → Monaco (1-based line, 1-based column). Column counts UTF-16
/// code units (Monaco's convention): an astral char (emoji) is two units.
fn line_col(src: &str, byte: usize) -> (usize, usize) {
    let b = byte.min(src.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in src.char_indices() {
        if i >= b {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += ch.len_utf16();
        }
    }
    (line, col)
}

/// Pull `(start, end)` and the trailing message out of a flattened parse-error
/// string like `parse (12, 15): unexpected token`.
fn parse_error_span(s: &str) -> Option<((usize, usize), &str)> {
    let open = s.find('(')?;
    let close = open + s[open..].find(')')?;
    let (a, b) = s[open + 1..close].split_once(',')?;
    let start = a.trim().parse().ok()?;
    let end = b.trim().parse().ok()?;
    let msg = s[close + 1..].trim_start_matches(':').trim();
    Some(((start, end), msg))
}

/// The first `` `name` `` token in a message, if any.
fn first_backtick(msg: &str) -> Option<&str> {
    let a = msg.find('`')? + 1;
    let rest = &msg[a..];
    let b = rest.find('`')?;
    Some(&rest[..b])
}

/// First whole-word occurrence of `name` in *code* — skipping `//` comments and
/// `"…"` string literals — as a byte span. Anchoring a diagnostic marker on a
/// mention inside a comment/string would draw the squiggle in the wrong place.
fn find_word(src: &str, name: &str) -> Option<(usize, usize)> {
    if name.is_empty() {
        return None;
    }
    let bytes = src.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let n = name.len();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            // line comment — skip to end of line
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            // string literal — skip to the closing quote (respecting \" escapes)
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            _ => {
                if bytes[i..].starts_with(name.as_bytes()) {
                    let end = i + n;
                    let before_ok = i == 0 || !is_word(bytes[i - 1]);
                    let after_ok = end >= bytes.len() || !is_word(bytes[end]);
                    if before_ok && after_ok {
                        return Some((i, end));
                    }
                }
                i += 1;
            }
        }
    }
    None
}

/// Build the `markers` array: precise spans for parse errors, best-effort spans
/// for semantic diagnostics. All are `error` severity (the compiler rejects
/// every one of them).
fn markers_json(src: &str, parse_errors: &[String], diags: &[maca_core::Diagnostic]) -> String {
    let mut out = String::from("[");
    let mut first = true;
    let mut emit = |out: &mut String, sb: usize, eb: usize, msg: &str| {
        let (l, c) = line_col(src, sb);
        let (el, ec) = line_col(src, eb.max(sb + 1));
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&format!(
            "{{\"severity\":\"error\",\"line\":{l},\"col\":{c},\"endLine\":{el},\"endCol\":{ec},\"message\":"
        ));
        push_json_str(out, msg);
        out.push('}');
    };

    for e in parse_errors {
        match parse_error_span(e) {
            Some(((s, en), msg)) => emit(&mut out, s, en, msg),
            None => emit(&mut out, 0, 1, e),
        }
    }
    for d in diags {
        let span = first_backtick(&d.msg).and_then(|n| find_word(src, n));
        match span {
            Some((s, e)) => emit(&mut out, s, e, &d.msg),
            None => emit(&mut out, 0, 1, &d.msg),
        }
    }
    out.push(']');
    out
}

/// Append `s` as a JSON string literal (quotes + escapes) to `out`.
fn push_json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_has_no_diags_and_emits_c() {
        let json = compile_json("main() -> int {\n    info(\"hi\")\n    0\n}\n", 0);
        assert!(json.contains("\"parseErrors\":[]"), "{json}");
        assert!(json.contains("\"diagnostics\":[]"), "{json}");
        assert!(json.contains("\"C\":"), "{json}");
        assert!(json.contains("int main("), "{json}");
    }

    #[test]
    fn bad_return_reports_type_mismatch() {
        let json = compile_json("bad() -> int => \"nope\"\n", 0);
        assert!(json.contains("TypeMismatch"), "{json}");
    }

    #[test]
    fn config_mode_emits_nix() {
        let json = compile_json("networking.hostName = \"rigel\"\n", 1);
        assert!(json.contains("\"Nix\":"), "{json}");
    }

    #[test]
    fn program_runs_and_captures_output() {
        let json = compile_json("main() -> int {\n    info(\"hi\")\n    0\n}\n", 0);
        assert!(json.contains("\"run\":"), "no run block: {json}");
        assert!(
            json.contains("\"output\":\"hi\\n\""),
            "output not captured: {json}"
        );
        assert!(json.contains("\"exit\":0"), "exit code missing: {json}");
    }

    #[test]
    /// `chr` and `ord` must mean the same thing here as they do natively, or
    /// the playground quietly runs a different language. The domain is the
    /// agreed one: `chr` answers "" outside 1..255, and the pair round-trips.
    fn chr_and_ord_match_the_native_domain() {
        let json = compile_json(
            "main() -> int {\n    \
             a = ord(chr(200))\n    \
             b = ord(chr(65))\n    \
             c = len(chr(256))\n    \
             d = len(chr(0))\n    \
             e = ord(\"\")\n    \
             info(\"{a} {b} {c} {d} {e}\")\n    0\n}\n",
            0,
        );
        assert!(
            json.contains("\"output\":\"200 65 0 0 -1\\n\""),
            "chr/ord disagree with the native domain: {json}"
        );
    }

    #[test]
    fn profile_renders_flame_graph() {
        let json = compile_json(
            "fib(n: int) -> int =>\n    n < 2 ? n : fib(n - 1) + fib(n - 2)\nmain() -> int {\n    info(\"{fib(10)}\")\n    0\n}\n",
            0,
        );
        assert!(
            json.contains("\"output\":\"55\\n\""),
            "fib(10) wrong: {json}"
        );
        // the shared renderer produces an HTML flame graph rooted at main, with fib
        assert!(
            json.contains("\"flameSvg\":\"<div"),
            "no flame html: {json}"
        );
        assert!(
            json.contains("flame graph"),
            "not the shared renderer: {json}"
        );
        assert!(json.contains("fib"), "fib frame missing: {json}");
        assert!(json.contains("\"maxDepth\":"), "no depth: {json}");
    }

    #[test]
    fn range_for_loop_runs() {
        // inclusive: `1..100` sums to 5050; `1..5` is a 5-element int[].
        let json = compile_json(
            "main() -> int {\n    sum = 0\n    for i in 1..100 {\n        sum = sum + i\n    }\n    xs = 1..5\n    info(\"{sum} {len(xs)} {xs[0]}\")\n    0\n}\n",
            0,
        );
        assert!(
            json.contains("\"output\":\"5050 5 1\\n\""),
            "range wrong: {json}"
        );
    }

    #[test]
    fn recursive_sum_runs() {
        let json = compile_json(
            "Tree = Leaf(int) | Node(Tree, Tree)\ntotal(t: Tree) -> int {\n    match t {\n        Leaf(n) => n\n        Node(l, r) => total(l) + total(r)\n    }\n}\nmain() -> int {\n    info(\"{total(Node(Leaf(1), Node(Leaf(2), Leaf(3))))}\")\n    0\n}\n",
            0,
        );
        assert!(
            json.contains("\"output\":\"6\\n\""),
            "tree total wrong: {json}"
        );
    }

    #[test]
    fn indexing_and_update_run() {
        let json = compile_json(
            "main() -> int {\n    xs = 10, 20, 30\n    xs[0] = 99\n    info(\"{xs[0]} {len(xs)}\")\n    0\n}\n",
            0,
        );
        assert!(
            json.contains("\"output\":\"99 3\\n\""),
            "indexing wrong: {json}"
        );
    }

    #[test]
    fn math_prelude_runs_in_interp() {
        let src = "main() -> int {\n    info(\"{sqrt(144.0)} {abs(0 - 7)} {min(4, 9)} {max(4, 9)} {gcd(54, 24)} {clamp(42, 0, 10)}\")\n    0\n}\n";
        let json = compile_json(src, 0);
        assert!(
            json.contains("\"output\":\"12.0 7 4 9 6 10\\n\""),
            "math interp wrong: {json}"
        );
    }

    #[test]
    fn closures_and_list_methods_run_in_interp() {
        // capturing lambda + map/filter/reduce/sort/sum + first-class closure,
        // all in the playground interpreter (must match the native C backend).
        let src = "main() -> int {\n\
            \x20   xs = 5, 3, 1, 4, 2\n\
            \x20   k = 10\n\
            \x20   shifted = xs.map(v => v + k)\n\
            \x20   evens = xs.filter(v => v % 2 == 0)\n\
            \x20   total = xs.reduce(0, (a, v) => a + v)\n\
            \x20   sorted = xs.sort()\n\
            \x20   inc = n => n + 1\n\
            \x20   g = inc(41)\n\
            \x20   info(\"{shifted[0]} {len(evens)} {total} {sorted[0]} {xs.sum()} {xs.max()} {g}\")\n\
            \x20   0\n\
            }\n";
        let json = compile_json(src, 0);
        assert!(
            json.contains("\"output\":\"15 2 15 1 15 5 42\\n\""),
            "closures interp wrong: {json}"
        );
    }

    #[test]
    fn async_spawn_await_runs_in_interp() {
        // The playground interpreter runs colorblind async eagerly; results match
        // the concurrent native runtime (20 + 40 = 60).
        let src = "work(n: int) -> int {\n    sleep_ms(1)\n    n * 2\n}\nmain() -> int {\n    a = spawn work(10)\n    b = spawn work(20)\n    info(\"{await a + await b}\")\n    0\n}\n";
        let json = compile_json(src, 0);
        assert!(
            json.contains("\"output\":\"60\\n\""),
            "async interp wrong: {json}"
        );
    }

    #[test]
    fn find_line_prefers_definition_over_earlier_call() {
        // `helper` is called (indented) before it is defined at column 0.
        let src = "main() -> int {\n    helper()\n    0\n}\nhelper() -> int => 1\n";
        assert_eq!(
            find_line(src, "helper"),
            5,
            "should point at the def, not the call"
        );
    }

    #[test]
    fn line_col_counts_utf16_units() {
        // 😀 is one scalar but two UTF-16 units; the `x` after it is column 3.
        let src = "😀x";
        let bx = src.char_indices().nth(1).unwrap().0;
        assert_eq!(line_col(src, bx), (1, 3));
    }

    #[test]
    fn string_stdlib_runs_in_interp() {
        // split/trim/lower/upper/contains/replace/substr/index_of all run in the
        // playground interpreter and agree with the native C backend.
        let src = "main() -> int {\n\
            \x20   row = \"a, B ,c\"\n\
            \x20   parts = row.split(\",\")\n\
            \x20   t = \"Hello World\"\n\
            \x20   mid = parts[1].trim().lower()\n\
            \x20   up = t.upper()\n\
            \x20   has = t.contains(\"World\")\n\
            \x20   rep = t.replace(\"World\", \"Maca\")\n\
            \x20   sub = t.substr(0, 5)\n\
            \x20   idx = t.index_of(\"World\")\n\
            \x20   info(\"{len(parts)} {mid} {up} {has} {rep} {sub} {idx}\")\n\
            \x20   0\n\
            }\n";
        let json = compile_json(src, 0);
        assert!(
            json.contains("\"output\":\"3 b HELLO WORLD true Hello Maca Hello 6\\n\""),
            "string stdlib output wrong: {json}"
        );
    }

    #[test]
    fn find_word_skips_comments_and_strings() {
        // `foo` appears in a comment and a string before the real definition.
        let src = "// foo is broken\nmsg = \"foo\"\nfoo() -> int => 1\n";
        let (a, b) = find_word(src, "foo").expect("should find the code occurrence");
        assert_eq!(&src[a..b], "foo");
        // the match must be the def on line 3, not the comment/string mentions
        assert!(
            a > src.find("\"foo\"").unwrap(),
            "anchored on comment/string, not code"
        );
    }
}
