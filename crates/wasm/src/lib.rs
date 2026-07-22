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

use maca_core::{DiagKind, Mode};

mod interp;

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
    }
}

/// The playground's whole result, as JSON (built by hand to keep deps zero).
///
/// ```json
/// { "parseErrors": ["…"],
///   "diagnostics": [{"kind":"TypeMismatch","msg":"…"}],
///   "outputs": {"C":"…"} | {"JS":"…","HTML":"…","CSS":"…"} | {"Nix":"…"} }
/// ```
fn compile_json(src: &str, mode: u32) -> String {
    let parsed = maca_parser::parse(src);
    let mut out = String::from("{");

    // parse errors
    out.push_str("\"parseErrors\":[");
    for (i, e) in parsed.errors.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(&mut out, e);
    }
    out.push(']');

    // type/effect diagnostics
    out.push_str(",\"diagnostics\":[");
    if parsed.errors.is_empty() {
        let diags = maca_core::check(&parsed.module, mode_of(mode));
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
    }
    out.push(']');

    // backend outputs (only when the source parses)
    let mut js_exports: Vec<String> = Vec::new();
    out.push_str(",\"outputs\":{");
    if parsed.errors.is_empty() {
        match mode_of(mode) {
            Mode::Config => {
                out.push_str("\"Nix\":");
                push_json_str(&mut out, &maca_backend_nix::emit(&parsed.module));
            }
            Mode::Program => {
                out.push_str("\"C\":");
                push_json_str(&mut out, &maca_backend_c::emit(&parsed.module));
                let js = maca_backend_js::emit(&parsed.module);
                js_exports = js.exports.clone();
                out.push_str(",\"JS\":");
                push_json_str(&mut out, &js.js);
                if !js.html.is_empty() {
                    out.push_str(",\"HTML\":");
                    push_json_str(&mut out, &js.html);
                }
                if !js.css.is_empty() {
                    out.push_str(",\"CSS\":");
                    push_json_str(&mut out, &js.css);
                }
            }
        }
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
        out.push_str(",\"functions\":[");
        for (i, (name, calls)) in r.profile.calls.iter().take(12).enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"name\":");
            push_json_str(&mut out, name);
            out.push_str(",\"calls\":");
            out.push_str(&calls.to_string());
            out.push('}');
        }
        out.push_str("]}}");
    }

    out.push('}');
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
        assert!(json.contains("\"output\":\"hi\\n\""), "output not captured: {json}");
        assert!(json.contains("\"exit\":0"), "exit code missing: {json}");
    }

    #[test]
    fn profile_counts_function_calls() {
        // fib(10): fib is called 177 times
        let json =
            compile_json("fib(n: int) -> int =>\n    n < 2 ? n : fib(n - 1) + fib(n - 2)\nmain() -> int {\n    info(\"{fib(10)}\")\n    0\n}\n", 0);
        assert!(json.contains("\"output\":\"55\\n\""), "fib(10) wrong: {json}");
        assert!(json.contains("\"name\":\"fib\",\"calls\":177"), "fib call count wrong: {json}");
        assert!(json.contains("\"maxDepth\":"), "no depth: {json}");
    }

    #[test]
    fn recursive_sum_runs() {
        let json = compile_json(
            "Tree = Leaf(int) | Node(Tree, Tree)\ntotal(t: Tree) -> int {\n    match t {\n        Leaf(n) => n\n        Node(l, r) => total(l) + total(r)\n    }\n}\nmain() -> int {\n    info(\"{total(Node(Leaf(1), Node(Leaf(2), Leaf(3))))}\")\n    0\n}\n",
            0,
        );
        assert!(json.contains("\"output\":\"6\\n\""), "tree total wrong: {json}");
    }

    #[test]
    fn indexing_and_update_run() {
        let json = compile_json(
            "main() -> int {\n    let xs = 10, 20, 30\n    xs[0] = 99\n    info(\"{xs[0]} {len(xs)}\")\n    0\n}\n",
            0,
        );
        assert!(json.contains("\"output\":\"99 3\\n\""), "indexing wrong: {json}");
    }
}
