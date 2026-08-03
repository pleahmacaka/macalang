//! What a `{` opens.
//!
//! The same three characters start a record literal and a block, and the
//! decision is needed twice: once by the parser, which is walking the file, and
//! once by anything that only has the token stream, such as the language
//! server's scope walker. Both live here so there is one answer rather than two
//! that drift apart.
//!
//! The second reader had its own copy for a while, and the copy was missing the
//! `=> {` case: `mk(n: int) -> Point => { x = n }` read as a block, so the
//! literal's `x` was an ordinary local and a rename of the field skipped it.

use maca_lexer::{Tok, Token};

/// A brace either opens a record (a type declaration, a literal, or a `with`
/// update) or a block. Only the first kind makes `name =` a field key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brace {
    Record,
    Block,
}

/// Which reading a `{` after a function's `=>` has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowBrace {
    /// Some entry cannot be a record field, so this is a block.
    Block,
    /// Every entry is a distinct `name = value`, and a comma sits at the
    /// brace's own depth, which is a thing no block has. A trailing one counts:
    /// `{ x = 1, }` is not a statement either.
    Record,
    /// Every entry is a distinct `name = value` and only newlines separate
    /// them: both readings hold, so neither is taken.
    Both,
}

/// A `{` opens a record when it follows a type name (`P {`), a `with`, the `=`
/// of a declaration, or a `=>` whose body is shaped like a record. Everything
/// else is a block.
///
/// The parser has a `no_brace` mode for the same question in a control header:
/// `for x in xs {` is a loop, not a constructor. A control keyword at the head
/// of the phrase opens a block; otherwise the token immediately before the
/// brace decides.
///
/// It has to be the *immediate* one. Asking whether a `->` appears anywhere on
/// the line read `at(n: int) -> Point => Point { x = n }` as a block, so a
/// rename of `x` skipped the literal's key, and skipped `c with { port = p }`
/// in `examples/record_update.maca`, which produced a file that did not
/// compile. A `->` before the name means a return type; a `=>`, a `=`, a `(`
/// or a `,` before it means a constructor.
///
/// A `=>` sitting directly against the brace is the one case the token before
/// cannot settle, because the literal has no type name in front of it. That is
/// [`arrow_brace`]'s question, and it is asked rather than guessed.
pub fn brace_kind(tokens: &[Token], at: usize) -> Brace {
    let line = line_start(tokens, at);
    let head = &tokens[line..at];

    // Anywhere in the phrase, not just at its head: `x = if c { … }` puts the
    // `if` after the binding it feeds. A record literal inside a control arm is
    // unaffected, because the arm's own `{` ends the phrase.
    if head.iter().any(|t| opens_a_block(&t.tok)) {
        return Brace::Block;
    }
    let mut back = head.iter().rev().filter(|t| t.tok != Tok::Newline);
    match back.next().map(|t| &t.tok) {
        Some(Tok::With | Tok::Eq) => Brace::Record,
        // `f() -> Point {` is a body; `=> Point {` is a literal.
        Some(Tok::Ident(_)) if back.next().map(|t| &t.tok) == Some(&Tok::Arrow) => Brace::Block,
        Some(Tok::Ident(_)) => Brace::Record,
        // `f() => { … }` is whichever the entries make it. `Both` is the
        // ambiguity the parser refuses, and it falls back to a block there, so
        // this answers the same way rather than inventing a third reading.
        Some(Tok::FatArrow) => match arrow_brace(tokens, at) {
            ArrowBrace::Record => Brace::Record,
            ArrowBrace::Block | ArrowBrace::Both => Brace::Block,
        },
        _ => Brace::Block,
    }
}

fn opens_a_block(t: &Tok) -> bool {
    matches!(t, Tok::If | Tok::Else | Tok::While | Tok::For | Tok::Match)
}

/// Index of the first token of the phrase containing `at`.
///
/// A brace is a boundary as much as a newline is: no `Newline` is emitted after
/// `{`, so scanning back for one alone runs out of the block and picks up the
/// enclosing function's signature, which has a `->` in it, and so read every
/// record literal inside a function as a block.
fn line_start(tokens: &[Token], at: usize) -> usize {
    tokens[..at]
        .iter()
        .rposition(|t| matches!(t.tok, Tok::Newline | Tok::LBrace | Tok::RBrace))
        .map_or(0, |i| i + 1)
}

/// The field a `name = value` entry names, if that is what the entry is.
fn field_name(entry: &[Token]) -> Option<&str> {
    match (&entry.first()?.tok, &entry.get(1)?.tok) {
        (Tok::Ident(n), Tok::Eq) if entry.len() > 2 => Some(n),
        _ => None,
    }
}

/// Decide the `{` at `open`, which follows a function's `=>`, from the shape of
/// its body.
///
/// The token before the brace cannot make this call: it answers a `{` in a
/// control header, and here both readings stay live until the whole brace has
/// been read. So this is the same idea one lookahead wider, like the scan that
/// finds a function definition by the `->`, `{`, or `=>` after its `)`.
///
/// The evidence is the entries and their separator. A record literal's fields
/// are `name = value` and a comma separates them; a block's statements are
/// newline-separated and the last one is the value. So a bare expression, a
/// name bound twice (no record has two `x` fields), a punned `{ x, y }`,
/// `xs[i] = v`, `p.f = v` and `const x = e` each rule the record out on their
/// own.
pub fn arrow_brace(toks: &[Token], open: usize) -> ArrowBrace {
    let Some(close) = brace_end(toks, open) else {
        return ArrowBrace::Block; // unterminated; `parse_block` reports it
    };
    // where each entry starts, cutting at the brace's own depth
    let mut cuts = vec![open + 1];
    let mut saw_comma = false;
    let mut depth = 0i32;
    for (j, t) in toks.iter().enumerate().take(close).skip(open + 1) {
        match &t.tok {
            Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
            Tok::RParen | Tok::RBracket | Tok::RBrace => depth -= 1,
            Tok::Comma if depth == 0 => {
                saw_comma = true;
                cuts.push(j + 1);
            }
            Tok::Newline if depth == 0 => cuts.push(j + 1),
            _ => {}
        }
    }
    cuts.push(close + 1);

    let mut names: Vec<&str> = Vec::new();
    for w in cuts.windows(2) {
        let entry = &toks[w[0]..w[1] - 1];
        if entry.is_empty() {
            continue; // two separators in a row
        }
        match field_name(entry) {
            Some(n) if !names.contains(&n) => names.push(n),
            _ => return ArrowBrace::Block,
        }
    }
    if names.is_empty() {
        return ArrowBrace::Block; // `{}` is an empty block, not a record
    }
    if saw_comma {
        ArrowBrace::Record
    } else {
        ArrowBrace::Both
    }
}

/// The index of the `}` that closes the `{` at `open`.
pub fn brace_end(toks: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (j, t) in toks.iter().enumerate().skip(open) {
        match &t.tok {
            // an interpolation's braces are `InterpStart`/`InterpEnd`, so a
            // `{` inside a string never reaches this scan
            Tok::LBrace => depth += 1,
            Tok::RBrace => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            Tok::Eof => return None,
            _ => {}
        }
    }
    None
}
