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
    "from", "with", "fail", "try", "alias", "await", "spawn",
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

/// The Monarch grammar (embedded in `playground/playground.maca`) and the Zed
/// tree-sitter queries (`highlights.scm`) also name every keyword.
#[test]
fn monarch_and_zed_grammars_name_every_keyword() {
    let monarch = repo("playground/playground.maca");
    let zed = repo("editor/zed-maca/languages/maca/highlights.scm");
    assert!(!monarch.is_empty(), "playground.maca missing");
    assert!(!zed.is_empty(), "highlights.scm missing");
    for kw in LEXER_KEYWORDS {
        // `import`/`from` are matched by dedicated rules in the Zed grammar, so
        // they may not appear in the ident-keyword alternation — check Monarch
        // for those and both for the rest.
        assert!(monarch.contains(&format!("\"{kw}\"")), "Monarch grammar missing keyword `{kw}`");
        if *kw != "import" && *kw != "from" {
            assert!(zed.contains(kw), "Zed grammar missing keyword `{kw}`");
        }
    }
}

/// Words that are NOT Maca keywords must lex as plain identifiers — a guard so
/// no grammar re-introduces a phantom `let`/`return`/`fn`/`type` keyword that
/// the language does not actually have.
#[test]
fn phantom_keywords_are_not_reserved() {
    for word in ["let", "return", "fn", "type", "def", "var"] {
        let first = &lex(word).tokens[0].tok;
        assert!(
            matches!(first, Tok::Ident(_)),
            "`{word}` is not a Maca keyword but lexed as {first:?}"
        );
    }
    // and no editor grammar should list them as keywords/reserved words
    let tm = repo("editor/maca.tmLanguage.json");
    let zed = repo("editor/zed-maca/languages/maca/highlights.scm");
    for phantom in ["let", "return"] {
        assert!(!tm.contains(&format!("|{phantom})")), "TextMate lists phantom keyword `{phantom}`");
        assert!(!tm.contains(&format!("({phantom}|")), "TextMate lists phantom keyword `{phantom}`");
        assert!(!zed.contains(&format!("^({phantom}|")), "Zed lists phantom keyword `{phantom}`");
        assert!(!zed.contains(&format!("|{phantom}|")), "Zed lists phantom keyword `{phantom}`");
    }
}
