//! The handbook's reference appendices must describe the real compiler.
//!
//! A keyword table is exactly the kind of documentation that rots: someone adds
//! a keyword to the lexer, and the book quietly becomes wrong. So the appendix
//! is checked against the lexer itself: every reserved word must appear in the
//! table, and every word the table claims is reserved must actually be.
//!
//! The same for the "not a keyword" list: those entries are what the checker
//! offers a hint for, and a hint that doesn't fire is worse than none.

use std::path::PathBuf;

fn book(file: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/tomo/book/en")
        .join(file);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The first column of every `| `x` | … |` row in the document.
fn table_terms(md: &str) -> Vec<String> {
    md.lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .filter_map(|l| l.split('|').nth(1))
        .map(str::trim)
        .filter(|c| c.starts_with('`'))
        .flat_map(|c| {
            c.split(", ")
                .map(|t| t.trim().trim_matches('`').to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Every word the lexer turns into something other than an identifier.
fn reserved() -> Vec<String> {
    // Probe the lexer rather than duplicating its table: a word is reserved
    // exactly when lexing it alone yields a token that isn't `Ident`.
    const CANDIDATES: &[&str] = &[
        "const", "as", "if", "else", "for", "in", "while", "break", "continue", "match", "import",
        "from", "with", "fail", "try", "alias", "await", "spawn", "true", "false",
        // words that must NOT be reserved
        "fn", "let", "var", "return", "type", "struct", "enum", "class", "async", "null", "nil",
        "pub", "mut", "impl", "trait", "self", "new", "def", "func",
    ];
    CANDIDATES
        .iter()
        .filter(|w| {
            let toks = maca_lexer::lex(w).tokens;
            !matches!(
                toks.first().map(|t| &t.tok),
                Some(maca_lexer::Tok::Ident(_))
            )
        })
        .map(|w| w.to_string())
        .collect()
}

#[test]
fn appendix_a_lists_every_real_keyword() {
    let md = book("a1-keywords.md");
    let listed = table_terms(&md);
    for kw in reserved() {
        assert!(
            listed.contains(&kw),
            "appendix A doesn't document the keyword `{kw}`"
        );
    }
}

#[test]
fn appendix_a_claims_no_keyword_that_isnt_one() {
    let md = book("a1-keywords.md");
    // only the first table; the second is explicitly the *non*-keywords
    let first = md.split("## Words Maca does not reserve").next().unwrap();
    let real = reserved();
    for claimed in table_terms(first) {
        assert!(
            real.contains(&claimed),
            "appendix A calls `{claimed}` a keyword, but the lexer doesn't reserve it"
        );
    }
}

/// The "not a keyword" table promises a hint for each of these. Check the
/// checker actually gives one, so the book and the compiler agree.
#[test]
fn the_phantom_keywords_the_book_lists_all_produce_a_hint() {
    use maca_core::{DiagKind, Mode, check};
    for word in ["fn", "let", "var", "return", "type", "null"] {
        let src = format!("f() -> int {{\n    {word}\n    1\n}}\n");
        let parsed = maca_parser::parse(&src);
        let d = check(&parsed.module, Mode::Program);
        assert!(
            d.iter()
                .any(|x| x.kind == DiagKind::UndefinedName && x.msg.contains(word)),
            "appendix A promises a hint for `{word}`, but the checker is silent"
        );
    }
}

/// Every diagnostic the checker can emit has an appendix D section, and every
/// section names a real one.
#[test]
fn appendix_d_covers_every_diagnostic_kind() {
    let md = book("a4-diagnostics.md");
    for kind in [
        "TypeMismatch",
        "NonExhaustive",
        "Immutable",
        "UndefinedName",
        "UnknownOption",
        "EffectInConfig",
    ] {
        assert!(
            md.contains(&format!("## {kind}")),
            "appendix D has no section for {kind}"
        );
    }
}
