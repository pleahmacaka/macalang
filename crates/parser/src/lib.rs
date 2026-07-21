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
