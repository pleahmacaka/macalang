//! Which binding an identifier belongs to.
//!
//! References and rename used to be a whole-word text search over the file.
//! Renaming the local `x` in
//!
//! ```text
//! P = { x: int }
//! f() -> int { x = 1  x + 1 }
//! g(x: int) -> int => x * 2
//! h() -> int { p = P { x = 9 }  p.x }
//! ```
//!
//! rewrote all seven `x`s: the field declaration, `g`'s parameter, `g`'s body,
//! the literal's key and the field access were all collateral. Two of the seven
//! were right. In an editor that is a rename that silently breaks the file.
//!
//! The AST is span-free on purpose (`maca_parser::ast` — the print/parse
//! roundtrip test compares structure), so scope is recovered from the token
//! stream instead, which does carry spans. That is enough to separate the three
//! things a name can be:
//!
//!   * a **top-level** function, type or constant — every use in the file, and
//!     in any file that imports it;
//!   * a **local** or parameter — uses inside the one function that binds it;
//!   * a **field** — declarations, literal keys, named arguments, and `.field`
//!     accesses.
//!
//! What it does not do is inner-block scope: two `x`s in different `if` arms of
//! one function are one binding here. That is a narrower error than the old
//! behaviour by a whole file, and it needs real scope tracking in the parser to
//! fix properly.

use maca_lexer::{Span, Tok, Token, lex};

/// Where a name is visible, and therefore what a rename may touch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// A top-level function, type, or constant.
    TopLevel,
    /// A parameter or local, confined to the byte range of its function.
    Local(Span),
    /// A record field, a named argument, or a `.field` access.
    Field,
}

/// The identifier under the cursor, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub name: String,
    pub scope: Scope,
    /// The span the cursor is actually on, for `prepareRename`.
    pub at: Span,
}

/// What syntactic position an identifier occurrence sits in.
///
/// The distinction that matters is between a *value* — a name being read or
/// written — and a *key*, which names a field rather than referring to one of
/// these bindings. `p.x`, `P { x = 1 }`, `x: int` inside a record, and
/// `div(class="c")` are all keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Value,
    Key,
}

struct Occurrence<'a> {
    name: &'a str,
    span: Span,
    role: Role,
}

/// A brace either opens a record — a type declaration, a literal, or a `with`
/// update — or a block. Only the first kind makes `name =` a field key.
#[derive(Clone, Copy, PartialEq)]
enum Brace {
    Record,
    Block,
}

/// Resolve the identifier at `offset`, or `None` if the cursor is not on one.
pub fn resolve(src: &str, offset: usize) -> Option<Binding> {
    let tokens = lex(src).tokens;
    let occurrences = occurrences(src, &tokens);
    // A cursor strictly inside a name wins; only if it sits in no name at all
    // does the one ending right there count, so a click at the `.` of `p.x`
    // resolves `x` rather than `p`.
    let hit = occurrences
        .iter()
        .find(|o| offset >= o.span.0 && offset < o.span.1)
        .or_else(|| occurrences.iter().find(|o| offset == o.span.1))?;
    let name = hit.name.to_string();

    if hit.role == Role::Key {
        return Some(Binding {
            name,
            scope: Scope::Field,
            at: hit.span,
        });
    }

    // A name the enclosing function binds itself is that function's, even when
    // a top-level definition shares the name — the local shadows it.
    //
    // Everything else is top-level, including a name this file never defines:
    // a call to an imported function is exactly that, and treating it as a
    // local silently renamed the call and left the definition and every other
    // caller alone.
    let items = top_level_items(src, &tokens);
    let scope = match enclosing(&items, hit.span.0) {
        Some(region) if binds_locally(&tokens, region, &name) => Scope::Local(region),
        _ => Scope::TopLevel,
    };
    Some(Binding {
        name,
        scope,
        at: hit.span,
    })
}

/// Every span the binding covers, in source order.
pub fn spans(src: &str, binding: &Binding) -> Vec<Span> {
    let tokens = lex(src).tokens;
    let want = match binding.scope {
        Scope::Field => Role::Key,
        _ => Role::Value,
    };
    occurrences(src, &tokens)
        .into_iter()
        .filter(|o| o.name == binding.name && o.role == want)
        .filter(|o| match binding.scope {
            Scope::Local((start, end)) => o.span.0 >= start && o.span.0 < end,
            _ => true,
        })
        .map(|o| o.span)
        .collect()
}

/// Every identifier in the file, with the position it sits in.
///
/// Comments and string bodies never reach here — the lexer does not emit them
/// as identifiers — which is why prose mentioning a name is safe from rename
/// without the scanner having to know what a comment looks like. An
/// interpolation *is* code and its identifiers do come through, which is right:
/// `"{count}"` refers to the binding.
fn occurrences<'a>(src: &'a str, tokens: &'a [Token]) -> Vec<Occurrence<'a>> {
    let mut out = Vec::new();
    let mut braces: Vec<Brace> = Vec::new();
    let mut parens = 0usize;

    for (i, t) in tokens.iter().enumerate() {
        match &t.tok {
            Tok::LBrace => braces.push(brace_kind(tokens, i)),
            Tok::RBrace => {
                braces.pop();
            }
            Tok::LParen => parens += 1,
            Tok::RParen => parens = parens.saturating_sub(1),
            Tok::Ident(name) => {
                let role = if is_key(tokens, i, braces.last().copied(), parens) {
                    Role::Key
                } else {
                    Role::Value
                };
                out.push(Occurrence {
                    name: &src[t.span.0..t.span.1],
                    span: t.span,
                    role,
                });
                let _ = name;
            }
            _ => {}
        }
    }
    out
}

/// A `{` opens a record when it follows a type name (`P {`), a `with`, or the
/// `=` of a declaration. Everything else is a block.
///
/// The same three characters can be either, and the parser has a `no_brace`
/// mode for exactly this reason — `for x in xs {` is a loop, not a constructor.
/// Here the line does the disambiguating: a control keyword opens a block, and
/// so does a `->`, because the name before the brace is then a return type
/// rather than a constructor.
fn brace_kind(tokens: &[Token], at: usize) -> Brace {
    let line = line_start(tokens, at);
    let head = &tokens[line..at];

    if head.first().is_some_and(|t| opens_a_block(&t.tok))
        || head.iter().any(|t| t.tok == Tok::Arrow)
    {
        return Brace::Block;
    }
    match head.iter().rev().find(|t| t.tok != Tok::Newline) {
        Some(t) => match &t.tok {
            Tok::Ident(_) | Tok::With | Tok::Eq => Brace::Record,
            _ => Brace::Block,
        },
        None => Brace::Block,
    }
}

fn opens_a_block(t: &Tok) -> bool {
    matches!(t, Tok::If | Tok::Else | Tok::While | Tok::For | Tok::Match)
}

/// Index of the first token of the phrase containing `at`.
///
/// A brace is a boundary as much as a newline is: no `Newline` is emitted after
/// `{`, so scanning back for one alone runs out of the block and picks up the
/// enclosing function's signature — which has a `->` in it, and so read every
/// record literal inside a function as a block.
fn line_start(tokens: &[Token], at: usize) -> usize {
    tokens[..at]
        .iter()
        .rposition(|t| matches!(t.tok, Tok::Newline | Tok::LBrace | Tok::RBrace))
        .map_or(0, |i| i + 1)
}

/// Is this identifier naming a field rather than referring to a binding?
fn is_key(tokens: &[Token], at: usize, brace: Option<Brace>, parens: usize) -> bool {
    // `p.x` — a field access, whatever encloses it
    if at > 0 && tokens[at - 1].tok == Tok::Dot {
        return true;
    }
    let next = tokens.get(at + 1).map(|t| &t.tok);
    // `div(class="c")` and `f(name = v)` — a named argument, which names a
    // parameter or an attribute rather than referring to a local
    if parens > 0 && next == Some(&Tok::Eq) {
        return true;
    }
    // inside a record: `x: int` declares a field, `x = 1` initialises one
    brace == Some(Brace::Record) && matches!(next, Some(Tok::Colon) | Some(Tok::Eq))
}

/// The byte range of each top-level item, paired with its name.
///
/// An item starts at an identifier in column zero; it runs until the next one.
/// That is the same column-zero rule the outline and go-to-definition already
/// use, and it holds for both `f() { … }` and `f() => …`, which is why the end
/// is found by looking forward rather than by counting braces.
fn top_level_items(src: &str, tokens: &[Token]) -> Vec<(Span, String)> {
    let mut starts: Vec<(usize, String)> = Vec::new();
    for t in tokens {
        if let Tok::Ident(_) = t.tok
            && at_line_start(src, t.span.0)
        {
            starts.push((t.span.0, src[t.span.0..t.span.1].to_string()));
        }
    }
    starts
        .iter()
        .enumerate()
        .map(|(i, (start, name))| {
            let end = starts.get(i + 1).map_or(src.len(), |(next, _)| *next);
            ((*start, end), name.clone())
        })
        .collect()
}

fn at_line_start(src: &str, byte: usize) -> bool {
    byte == 0 || src.as_bytes()[byte - 1] == b'\n'
}

fn enclosing(items: &[(Span, String)], byte: usize) -> Option<Span> {
    items
        .iter()
        .find(|((start, end), _)| byte >= *start && byte < *end)
        .map(|(span, _)| *span)
}

/// Does this function bind `name` itself — as a parameter, a `for` variable, a
/// lambda parameter, or an assignment?
fn binds_locally(tokens: &[Token], (start, end): Span, name: &str) -> bool {
    let inside: Vec<&Token> = tokens
        .iter()
        .filter(|t| t.span.0 >= start && t.span.0 < end)
        .collect();

    inside.iter().enumerate().any(|(i, t)| {
        let Tok::Ident(n) = &t.tok else { return false };
        if n != name {
            return false;
        }
        let next = inside.get(i + 1).map(|t| &t.tok);
        let prev = i.checked_sub(1).and_then(|j| inside.get(j)).map(|t| &t.tok);
        // `x: T` in a parameter list, `x = e`, `x =>`, `for x in`, `..rest` in
        // a list pattern, and a bare `x` in a parameter or constructor-pattern
        // position — `match s { Circle(r) => … }` binds `r`.
        matches!(next, Some(Tok::Colon) | Some(Tok::Eq) | Some(Tok::FatArrow))
            || matches!(prev, Some(Tok::For) | Some(Tok::DotDot))
            || (matches!(
                next,
                Some(Tok::Comma) | Some(Tok::RParen) | Some(Tok::RBracket)
            ) && matches!(
                prev,
                Some(Tok::LParen) | Some(Tok::Comma) | Some(Tok::LBracket)
            ))
    })
}
