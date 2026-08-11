pub mod ast;
pub mod braces;
pub mod imports;
pub mod modules;
mod parser;
mod print;

pub use ast::*;
pub use braces::{Brace, brace_kind};
pub use parser::{ParseError, Parser};
pub use print::print_module;

/// Is `name` an HTML element tag? A hyphen is how the platform spells a custom element, and it is what makes one here too.
pub fn is_ui_element_tag(name: &str) -> bool {
    is_custom_element_tag(name)
        || matches!(
            name,
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

/// Is `name` a custom element? The platform's own rule: a lowercase name with a hyphen inside it, which is why no built-in tag can ever be one.
pub fn is_custom_element_tag(name: &str) -> bool {
    let parts: Vec<&str> = name.split('-').collect();
    let plain = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    };

    parts.len() > 1
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && parts.iter().all(|p| plain(p))
}

/// Is `name` one of the two calls the driver rewrites before a program is compiled?
pub fn is_host_form(name: &str) -> bool {
    matches!(name, "data" | "stored")
}

/// Is `name` a backend intrinsic the checker must not treat as an undefined call?
pub fn is_backend_intrinsic(name: &str) -> bool {
    is_ui_element_tag(name)
        || is_host_form(name)
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
                | "styles"
                | "element"
                | "read_line"
                | "read_file"
                | "write_file"
                | "file_exists"
                | "real_path"
                | "make_dir"
                | "list_dir"
                | "is_dir"
                | "file_size"
                | "modified_ms"
                | "remove_file"
                | "remove_dir"
                | "copy_bytes"
                | "exec"
                | "capture"
                | "env"
                | "cwd"
                | "chdir"
                | "capture_err"
                | "at_eof"
                | "read_stdin"
                | "now_ms"
                | "now_iso"
                | "format_time"
                | "assert"
                | "assert_eq"
                | "failures"
                | "alloc_count"
                | "reuse_count"
                | "map"
        )
}

#[derive(Debug)]
pub struct Parsed {
    pub module: Module,
    pub errors: Vec<String>,
}

/// Lex then parse.
pub fn parse(src: &str) -> Parsed {
    let lexed = maca_lexer::lex(src);
    let mut errors: Vec<String> = lexed
        .errors
        .iter()
        .map(|e| format!("lex {:?}: {}", e.span, e.msg))
        .collect();
    let mut p = Parser::new(lexed.tokens);
    let mut module = p.parse_module();
    for e in &p.errors {
        errors.push(format!("parse {:?}: {}", e.span, e.msg));
    }
    lift_top_level_lambdas(&mut module);
    Parsed { module, errors }
}

/// A top-level `name = (a, b) [-> T] => body` *is* a function definition.
fn lift_top_level_lambdas(m: &mut Module) {
    for item in &mut m.items {
        let Stmt::Bind(b) = item else { continue };
        let Expr::Ident(name) = &b.target else {
            continue;
        };
        if !b.tys.is_empty() {
            continue;
        }
        let Expr::Lambda { params, ret, body } = &b.value else {
            continue;
        };
        *item = Stmt::Fn(FnDef {
            name: name.clone(),
            params: params.clone(),
            ret: ret.clone(),
            effects: None,
            body: Some(FnBody::Expr(body.clone())),
        });
    }
}
