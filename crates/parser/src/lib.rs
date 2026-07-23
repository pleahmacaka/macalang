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
        "div" | "span" | "p" | "pre" | "code" | "a" | "button" | "input" | "textarea" | "select"
            | "option" | "label" | "form" | "header" | "footer" | "main" | "section" | "article"
            | "nav" | "aside" | "ul" | "ol" | "li" | "table" | "thead" | "tbody" | "tr" | "td"
            | "th" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "img" | "svg" | "canvas" | "small"
            | "strong" | "em" | "b" | "i" | "hr" | "br" | "figure" | "figcaption" | "details"
            | "summary" | "dialog" | "progress" | "meter" | "video" | "audio"
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
    let mut errors: Vec<String> =
        lexed.errors.iter().map(|e| format!("lex {:?}: {}", e.span, e.msg)).collect();
    let mut p = Parser::new(lexed.tokens);
    let module = p.parse_module();
    for e in &p.errors {
        errors.push(format!("parse {:?}: {}", e.span, e.msg));
    }
    Parsed { module, errors }
}
