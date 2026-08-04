use maca_lexer::{Tok, Token};

/// A brace either opens a record (a type declaration, a literal, or a `with` update) or a block.
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
    /// Every entry is a distinct `name = value`, and a comma sits at the brace's own depth, which is a thing no block has.
    Record,
    /// Every entry is a distinct `name = value` and only newlines separate them.
    Both,
}

/// A `{` opens a record when it follows a type name (`P {`), a `with`, the `=` of a declaration, or a `=>` whose body is shaped like a record.
pub fn brace_kind(tokens: &[Token], at: usize) -> Brace {
    let line = line_start(tokens, at);
    let head = &tokens[line..at];

    if head.iter().any(|t| opens_a_block(&t.tok)) {
        return Brace::Block;
    }
    let mut back = head.iter().rev().filter(|t| t.tok != Tok::Newline);
    match back.next().map(|t| &t.tok) {
        Some(Tok::With | Tok::Eq) => Brace::Record,
        Some(Tok::Ident(_)) if back.next().map(|t| &t.tok) == Some(&Tok::Arrow) => Brace::Block,
        Some(Tok::Ident(_)) => Brace::Record,
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

/// Decide the `{` at `open`, which follows a function's `=>`, from the shape of its body.
pub fn arrow_brace(toks: &[Token], open: usize) -> ArrowBrace {
    let Some(close) = brace_end(toks, open) else {
        return ArrowBrace::Block;
    };
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
            continue;
        }
        match field_name(entry) {
            Some(n) if !names.contains(&n) => names.push(n),
            _ => return ArrowBrace::Block,
        }
    }
    if names.is_empty() {
        return ArrowBrace::Block;
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
