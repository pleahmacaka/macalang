//! Guard: the editor grammar must stay in sync with the lexer's keyword set.
//!
//! `editor/maca.tmLanguage.json` lists the words the highlighter paints as
//! keywords. If a keyword is added to / removed from the lexer without updating
//! the grammar (or vice versa), this fails.

use maca_lexer::{lex, Tok};
use std::path::PathBuf;

/// The lexer's reserved words (mirrors the `match` in `lex_ident`).
const LEXER_KEYWORDS: &[&str] = &[
    "const", "as", "if", "else", "for", "in", "while", "break", "continue", "match", "import",
    "from", "with", "fail", "try", "alias",
];

fn repo(rel: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    std::fs::read_to_string(p).unwrap_or_default()
}

/// Every reserved word lexes to a non-`Ident` token (i.e. it really is a keyword).
#[test]
fn lexer_keywords_are_reserved() {
    for kw in LEXER_KEYWORDS {
        let toks = lex(kw).tokens;
        let first = &toks[0].tok;
        assert!(
            !matches!(first, Tok::Ident(_)),
            "`{kw}` should lex as a keyword, got {first:?}"
        );
    }
    // `true`/`false` are their own tokens too.
    assert!(matches!(lex("true").tokens[0].tok, Tok::True));
    assert!(matches!(lex("false").tokens[0].tok, Tok::False));
}

/// The TextMate grammar (`editor/maca.tmLanguage.json`) names every keyword.
#[test]
fn textmate_grammar_names_every_keyword() {
    let tm = repo("editor/maca.tmLanguage.json");
    assert!(!tm.is_empty(), "editor/maca.tmLanguage.json missing");
    for kw in LEXER_KEYWORDS {
        assert!(tm.contains(kw), "TextMate grammar missing keyword `{kw}`");
    }
}
