use maca_lexer::{Span, Tok, Token, lex};
use maca_parser::{Brace, brace_kind};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Value,
    Key,
}

struct Occurrence<'a> {
    name: &'a str,
    span: Span,
    role: Role,
    /// A key that *declares* the field (`x: int` in a record body) rather than setting or reading it.
    declares: bool,
}

/// Resolve the identifier at `offset`, or `None` if the cursor is not on one.
pub fn resolve(src: &str, offset: usize) -> Option<Binding> {
    let tokens = lex(src).tokens;
    let occurrences = occurrences(src, &tokens);
    let hit = occurrences
        .iter()
        .find(|o| offset >= o.span.0 && offset < o.span.1)
        .or_else(|| occurrences.iter().find(|o| offset == o.span.1))?;
    let name = hit.name.to_string();
    let items = top_level_items(src, &tokens);
    if is_primitive_type(&name)
        || (maca_parser::is_backend_intrinsic(&name) && !items.iter().any(|(_, n)| n == &name))
    {
        return None;
    }

    if hit.role == Role::Key {
        return Some(Binding {
            name,
            scope: Scope::Field,
            at: hit.span,
        });
    }

    let scope = match enclosing(&items, hit.span.0) {
        Some(region) if hit.span.0 == region.0 => Scope::TopLevel,
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
    let shadowed: Vec<Span> = match binding.scope {
        Scope::TopLevel => top_level_items(src, &tokens)
            .into_iter()
            .map(|(region, _)| region)
            .filter(|r| binds_locally(&tokens, *r, &binding.name))
            .collect(),
        _ => Vec::new(),
    };
    occurrences(src, &tokens)
        .into_iter()
        .filter(|o| o.name == binding.name && o.role == want)
        .filter(|o| match binding.scope {
            Scope::Local((start, end)) => o.span.0 >= start && o.span.0 < end,
            _ => !shadowed.iter().any(|(s, e)| o.span.0 > *s && o.span.0 < *e),
        })
        .map(|o| o.span)
        .collect()
}

/// Does this file declare `name` as a record field?
pub fn declares_field(src: &str, name: &str) -> bool {
    let tokens = lex(src).tokens;
    occurrences(src, &tokens)
        .iter()
        .any(|o| o.name == name && o.role == Role::Key && o.declares)
}

/// A type name the language owns.
fn is_primitive_type(name: &str) -> bool {
    matches!(name, "int" | "float" | "str" | "bool" | "any" | "unit")
}

/// Is `name` a name a rename may produce?
pub fn is_renameable_to(name: &str) -> bool {
    !name.is_empty()
        && !is_primitive_type(name)
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !matches!(
            name,
            "if" | "else"
                | "while"
                | "for"
                | "in"
                | "match"
                | "import"
                | "const"
                | "with"
                | "as"
                | "spawn"
                | "await"
                | "true"
                | "false"
                | "break"
                | "continue"
                | "fail"
        )
}

/// Every identifier in the file, with the position it sits in.
fn occurrences<'a>(src: &'a str, tokens: &'a [Token]) -> Vec<Occurrence<'a>> {
    let mut out = Vec::new();
    let mut braces: Vec<(Brace, usize)> = Vec::new();
    let mut parens = 0usize;
    let mut import_line = false;

    for (i, t) in tokens.iter().enumerate() {
        match &t.tok {
            Tok::LBrace => {
                braces.push((brace_kind(tokens, i), parens));
                parens = 0;
            }
            Tok::RBrace => {
                if let Some((_, outer)) = braces.pop() {
                    parens = outer;
                }
            }
            Tok::LParen => parens += 1,
            Tok::RParen => parens = parens.saturating_sub(1),
            Tok::Newline => import_line = false,
            Tok::Import => import_line = true,
            Tok::Ident(_) if import_line && braces.is_empty() => {}
            Tok::Ident(_) => {
                let brace = braces.last().map(|(b, _)| *b);
                let role = if is_key(tokens, i, brace, parens) {
                    Role::Key
                } else {
                    Role::Value
                };
                out.push(Occurrence {
                    name: &src[t.span.0..t.span.1],
                    span: t.span,
                    role,
                    declares: brace == Some(Brace::Record)
                        && tokens.get(i + 1).map(|t| &t.tok) == Some(&Tok::Colon),
                });
            }
            _ => {}
        }
    }
    out
}

/// Is this identifier naming a field rather than referring to a binding?
fn is_key(tokens: &[Token], at: usize, brace: Option<Brace>, parens: usize) -> bool {
    if at > 0 && tokens[at - 1].tok == Tok::Dot {
        return true;
    }
    let next = tokens.get(at + 1).map(|t| &t.tok);
    if parens > 0 && next == Some(&Tok::Eq) {
        return true;
    }
    brace == Some(Brace::Record) && matches!(next, Some(Tok::Colon) | Some(Tok::Eq))
}

/// The byte range of each top-level item, paired with its name.
fn top_level_items(src: &str, tokens: &[Token]) -> Vec<(Span, String)> {
    let mut starts: Vec<(usize, String)> = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        let opens = matches!(
            tokens.get(i + 1).map(|t| &t.tok),
            Some(Tok::LParen | Tok::Eq | Tok::Colon | Tok::Dot)
        );
        if let Tok::Ident(_) = t.tok
            && at_line_start(src, t.span.0)
            && opens
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

/// Does this function bind `name` itself, as a parameter, a `for` variable, a lambda parameter, or an assignment?
fn binds_locally(tokens: &[Token], (start, end): Span, name: &str) -> bool {
    let inside: Vec<&Token> = tokens
        .iter()
        .filter(|t| t.span.0 >= start && t.span.0 < end)
        .collect();
    let Some(head) = inside.iter().position(|t| matches!(t.tok, Tok::Ident(_))) else {
        return false;
    };
    let params = param_range(&inside, head);

    inside.iter().enumerate().any(|(i, t)| {
        let Tok::Ident(n) = &t.tok else { return false };
        if n != name || i == head {
            return false;
        }
        let next = inside.get(i + 1).map(|t| &t.tok);
        let prev = i.checked_sub(1).and_then(|j| inside.get(j)).map(|t| &t.tok);
        if matches!(next, Some(Tok::Colon) | Some(Tok::Eq))
            || (next == Some(&Tok::FatArrow) && prev != Some(&Tok::Arrow))
            || matches!(prev, Some(Tok::For) | Some(Tok::DotDot))
        {
            return true;
        }
        let bare_in_list = matches!(
            next,
            Some(Tok::Comma) | Some(Tok::RParen) | Some(Tok::RBracket)
        ) && matches!(
            prev,
            Some(Tok::LParen)
                | Some(Tok::Comma)
                | Some(Tok::LBracket)
                | Some(Tok::LBrace)
                | Some(Tok::Newline)
        );
        bare_in_list && (params.contains(&i) || heads_a_pattern(&inside, i))
    })
}

/// The token indices of this item's own parameter list.
fn param_range(inside: &[&Token], head: usize) -> std::ops::Range<usize> {
    if inside.get(head + 1).map(|t| &t.tok) != Some(&Tok::LParen) {
        return 0..0;
    }
    let mut depth = 0usize;
    for (i, t) in inside.iter().enumerate().skip(head + 1) {
        match t.tok {
            Tok::LParen => depth += 1,
            Tok::RParen => {
                depth -= 1;
                if depth == 0 {
                    return head + 2..i;
                }
            }
            _ => {}
        }
    }
    0..0
}

/// Is the name at `i` inside a `match` arm's pattern, that is, does a `=>` close the phrase it sits in?
fn heads_a_pattern(inside: &[&Token], i: usize) -> bool {
    inside[i + 1..]
        .iter()
        .take_while(|t| {
            matches!(
                t.tok,
                Tok::Ident(_)
                    | Tok::Comma
                    | Tok::DotDot
                    | Tok::LParen
                    | Tok::RParen
                    | Tok::LBracket
                    | Tok::RBracket
                    | Tok::FatArrow
            )
        })
        .any(|t| t.tok == Tok::FatArrow)
}
