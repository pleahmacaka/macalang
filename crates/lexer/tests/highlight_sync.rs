//! Guard: the editor grammars must stay in sync with the lexer's keyword set.
//!
//! `editor/maca.tmLanguage.json` and `playground/maca-lang.js` list the words
//! the highlighter paints as keywords. If a keyword is added to / removed from
//! the lexer without updating the grammars (or vice versa), this fails.

use maca_lexer::{lex, Tok};
use std::path::PathBuf;

/// The lexer's reserved words (mirrors the `match` in `lex_ident`).
const LEXER_KEYWORDS: &[&str] = &[
    "let", "if", "else", "for", "in", "match", "import", "from", "with", "fail", "try", "alias",
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

/// The Monarch grammar (`playground/maca-lang.js`) lists exactly the lexer's
/// keywords in its `MACA_KEYWORDS` array.
#[test]
fn monarch_keywords_match_lexer() {
    let js = repo("playground/maca-lang.js");
    assert!(!js.is_empty(), "playground/maca-lang.js missing");
    let arr = extract_array(&js, "MACA_KEYWORDS");
    for kw in LEXER_KEYWORDS {
        assert!(arr.contains(&kw.to_string()), "Monarch grammar missing keyword `{kw}`");
    }
    for got in &arr {
        assert!(
            LEXER_KEYWORDS.contains(&got.as_str()),
            "Monarch grammar has stray keyword `{got}` not in the lexer"
        );
    }
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

/// Pull single-quoted string items out of `const NAME = [ 'a', 'b', … ]`.
fn extract_array(src: &str, name: &str) -> Vec<String> {
    let start = src.find(name).and_then(|i| src[i..].find('[').map(|j| i + j + 1)).unwrap();
    let end = start + src[start..].find(']').unwrap();
    src[start..end]
        .split(',')
        .filter_map(|s| {
            let s = s.trim().trim_matches(|c| c == '\'' || c == '"');
            (!s.is_empty()).then(|| s.to_string())
        })
        .collect()
}
