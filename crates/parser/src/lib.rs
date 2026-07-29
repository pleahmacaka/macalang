//! maca-parser: token stream → AST.
//!
//! Hand-written recursive-descent + Pratt (a documented deviation from the
//! brief's chumsky suggestion — zero deps and direct control over the
//! significant-newline layout). See `parser` for the grammar, `print` for the
//! canonical pretty-printer used by the roundtrip test.

pub mod ast;
mod parser;
mod print;

pub use ast::*;
pub use parser::{ParseError, Parser};
pub use print::print_module;

/// Is `name` an HTML element tag? In a view, `name(...)` builds a DOM node
/// rather than calling a function, so these open-ended tag names are valid
/// undefined-looking calls (the reactive-UI DSL). The single source of truth,
/// shared by the JS backend (which lowers them to `createElement`) and the type
/// checker (which must not flag them as undefined functions).
pub fn is_ui_element_tag(name: &str) -> bool {
    matches!(
        name,
        // document structure — only reachable on the native target, where an
        // element renders to text and a program emits a whole page. The JS
        // backend mounts into an existing document and never builds these.
        "html"
            | "head"
            | "title"
            | "meta"
            | "link"
            | "style"
            | "script"
            | "body"
            | "div"
            | "span"
            | "p"
            | "pre"
            | "code"
            | "a"
            | "button"
            | "input"
            | "textarea"
            | "select"
            | "option"
            | "label"
            | "form"
            | "header"
            | "footer"
            | "main"
            | "section"
            | "article"
            | "nav"
            | "aside"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "thead"
            | "tbody"
            | "tr"
            | "td"
            | "th"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "img"
            | "svg"
            | "canvas"
            | "small"
            | "strong"
            | "em"
            | "b"
            | "i"
            | "hr"
            | "br"
            | "blockquote"
            | "figure"
            | "figcaption"
            | "details"
            | "summary"
            | "dialog"
            | "progress"
            | "meter"
            | "video"
            | "audio"
    )
}

/// Is `name` a backend intrinsic the checker must not treat as an undefined
/// call? Covers the embedded target's MMIO/bit primitives (lowered by
/// `maca-backend-embedded`, not user-defined) and the UI element tags. Shared
/// so the type checker's undefined-call diagnostic stays in sync with what the
/// backends actually resolve.
pub fn is_backend_intrinsic(name: &str) -> bool {
    is_ui_element_tag(name)
        || matches!(
            name,
            "mmio_write"
                | "mmio_read"
                | "set_bits"
                | "clear_bits"
                | "toggle_bits"
                | "bit"
                | "shl"
                | "shr"
                | "bit_or"
                | "bit_and"
                | "delay"
                | "nop"
                | "forever"
                // file I/O builtins (lowered by the C backend to the runtime)
                // the stylesheet for the Tailwind utilities the module uses,
                // generated at compile time by the back end
                | "styles"
                // an element whose tag is an expression: `element("h" ++ n, …)`
                | "element"
                | "read_file"
                | "write_file"
                | "file_exists"
                | "make_dir"
                | "list_dir"
                | "is_dir"
                | "file_size"
                | "modified_ms"
                | "remove_file"
                | "remove_dir"
                | "copy_bytes"
                // processes
                | "exec"
                | "capture"
                | "env"
                | "cwd"
                | "chdir"
                // stdin
                | "read_line"
                | "at_eof"
                | "read_stdin"
                // time (UTC)
                | "now_ms"
                | "now_iso"
                | "format_time"
                // assertions — report and continue, so one run finds every
                // failure; `failures()` is the count a test returns
                | "assert"
                | "assert_eq"
                | "failures"
                // allocator counters: how many blocks were requested, and how
                // many of those came back off the free-list instead of malloc
                | "alloc_count"
                | "reuse_count"
                // an empty `Map str V`, typed by what it is assigned into
                | "map"
        )
}

#[derive(Debug)]
pub struct Parsed {
    pub module: Module,
    pub errors: Vec<String>,
}

/// Lex then parse. Lexer and parser errors are flattened into one list; an
/// empty list means a clean parse.
pub fn parse(src: &str) -> Parsed {
    let lexed = maca_lexer::lex(src);
    let mut errors: Vec<String> = lexed
        .errors
        .iter()
        .map(|e| format!("lex {:?}: {}", e.span, e.msg))
        .collect();
    let mut p = Parser::new(lexed.tokens);
    let module = p.parse_module();
    for e in &p.errors {
        errors.push(format!("parse {:?}: {}", e.span, e.msg));
    }
    Parsed { module, errors }
}
