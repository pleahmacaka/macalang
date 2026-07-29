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
//!
//! Almost every rule below is here because a plausible one was wrong on a real
//! file. An adversarial review renamed things across this repository and found
//! that 102 of its 1178 definitions — every record, every sum type, every
//! constant — renamed to a single edit: their own declaration. Two golden
//! examples came out of a rename not compiling. The cases are in
//! `crates/lsp/tests/smoke.rs`, one test each, and each of them fails if the
//! rule beside it is put back the way it was.

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
    /// A key that *declares* the field (`x: int` in a record body) rather than
    /// setting or reading it.
    declares: bool,
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
    // does the one ending right there count, so a click just past the end of a
    // name still resolves it.
    let hit = occurrences
        .iter()
        .find(|o| offset >= o.span.0 && offset < o.span.1)
        .or_else(|| occurrences.iter().find(|o| offset == o.span.1))?;
    let name = hit.name.to_string();
    let items = top_level_items(src, &tokens);
    // `int`, `str`, `info` — an editor would happily open a rename box over
    // these, and renaming all three `int`s in a signature is never what anyone
    // meant. They are `Ident`s to the lexer, so refusing them is this
    // function's job rather than the keyword check's.
    //
    // Only when the file hasn't defined the name itself, though: `main`,
    // `label`, `code` and `p` are UI tags *and* ordinary names, and a user
    // definition always shadows a tag. Refusing them outright made `main`
    // unrenameable in every program in this repository.
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

    // A name the enclosing function binds itself is that function's, even when
    // a top-level definition shares the name — the local shadows it.
    //
    // Everything else is top-level, including a name this file never defines:
    // a call to an imported function is exactly that, and treating it as a
    // local silently renamed the call and left the definition and every other
    // caller alone.
    let scope = match enclosing(&items, hit.span.0) {
        // The cursor is on the item's own head — `Point` in `Point = { … }`,
        // `NTASKS` in `NTASKS = 6`. The item *is* the definition, so this is
        // top-level however much the head looks like an assignment. Reading it
        // as a local gave every record, sum type and constant in the repository
        // a one-edit rename: its own declaration, and nothing that used it.
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
    // A top-level name is not visible inside a function that binds the same
    // name itself — that function's `helper` is its own. Without this a rename
    // of the top-level `helper` also rewrote every unrelated local called
    // `helper`, in this file and in every file that imports it.
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
            // the shadowing item's own head is still the definition being
            // renamed when the definition is what the head declares
            _ => !shadowed.iter().any(|(s, e)| o.span.0 > *s && o.span.0 < *e),
        })
        .map(|o| o.span)
        .collect()
}

/// Does this file declare `name` as a record field?
///
/// A field rename is single-file, so one declared elsewhere can only be renamed
/// half-way — see `workspace`.
pub fn declares_field(src: &str, name: &str) -> bool {
    let tokens = lex(src).tokens;
    occurrences(src, &tokens)
        .iter()
        .any(|o| o.name == name && o.role == Role::Key && o.declares)
}

/// A type name the language owns. Unlike the backend intrinsics, no definition
/// can shadow one of these, so a rename over it is never right.
fn is_primitive_type(name: &str) -> bool {
    matches!(name, "int" | "float" | "str" | "bool" | "any" | "unit")
}

/// Is `name` a name a rename may produce? An editor will hand over whatever was
/// typed, and `1x` or `if` turns a working file into one that doesn't parse.
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
///
/// Comments and string bodies never reach here — the lexer does not emit them
/// as identifiers — which is why prose mentioning a name is safe from rename
/// without the scanner having to know what a comment looks like. An
/// interpolation *is* code and its identifiers do come through, which is right:
/// `"{count}"` refers to the binding.
fn occurrences<'a>(src: &'a str, tokens: &'a [Token]) -> Vec<Occurrence<'a>> {
    let mut out = Vec::new();
    // Each open brace remembers the paren depth it interrupted. Without that,
    // `parens` was a flat file counter: a block opened inside a still-open call
    // — `xs.map(v => { y = 1  … })` — kept `parens > 0`, so the block's `y = 1`
    // read as a named argument, and renaming that temporary rewrote every
    // record field called `y` in the file.
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
            // `import std/text` names a module and a directory, not a binding;
            // renaming the path segment used to rewrite every local called
            // `text`. The names inside `import { foo } from a/b` are the
            // exception — those *are* the definitions, and a rename that skips
            // them leaves the importer asking for a name that no longer exists.
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

/// A `{` opens a record when it follows a type name (`P {`), a `with`, or the
/// `=` of a declaration. Everything else is a block.
///
/// The same three characters can be either, and the parser has a `no_brace`
/// mode for exactly this reason — `for x in xs {` is a loop, not a constructor.
/// A control keyword at the head of the phrase opens a block; otherwise the
/// token immediately before the brace decides.
///
/// It has to be the *immediate* one. Asking whether a `->` appears anywhere on
/// the line read `at(n: int) -> Point => Point { x = n }` as a block, so a
/// rename of `x` skipped the literal's key — and skipped `c with { port = p }`
/// in `examples/record_update.maca`, which produced a file that did not
/// compile. A `->` before the name means a return type; a `=>`, a `=`, a `(`
/// or a `,` before it means a constructor.
fn brace_kind(tokens: &[Token], at: usize) -> Brace {
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
    for (i, t) in tokens.iter().enumerate() {
        // A definition's name is followed by what makes it a definition: `(`
        // for a function, `=` for a type or constant, `:` or `.` for config.
        // Column zero alone was enough to let a wrapped call argument that
        // happened to start a line end the enclosing item early, so the rest of
        // the function was outside its own local's scope.
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

/// Does this function bind `name` itself — as a parameter, a `for` variable, a
/// lambda parameter, or an assignment?
fn binds_locally(tokens: &[Token], (start, end): Span, name: &str) -> bool {
    let inside: Vec<&Token> = tokens
        .iter()
        .filter(|t| t.span.0 >= start && t.span.0 < end)
        .collect();
    // The head is the region's first identifier, not its first token: the
    // lexer emits a zero-width `Newline` at the same offset, so counting
    // positions from zero found the newline and left the head looking like an
    // ordinary `Expr =` binding — which hid every use of the type in its own
    // declaration.
    let Some(head) = inside.iter().position(|t| matches!(t.tok, Tok::Ident(_))) else {
        return false;
    };
    let params = param_range(&inside, head);

    inside.iter().enumerate().any(|(i, t)| {
        let Tok::Ident(n) = &t.tok else { return false };
        // The item's own head is its definition, not a binding it introduces.
        // Counting it made `NTASKS = 6` a local of the item called `NTASKS`.
        if n != name || i == head {
            return false;
        }
        let next = inside.get(i + 1).map(|t| &t.tok);
        let prev = i.checked_sub(1).and_then(|j| inside.get(j)).map(|t| &t.tok);
        // `x: T`, `x = e`, `x =>`, `for x in`, and `..rest` in a list pattern.
        //
        // `x =>` is a lambda parameter only when the name isn't a return type:
        // `e_int(n: int) -> Expr => …` reads as one, and calling it a binder
        // hid every use of `Expr` in every constructor that returns one.
        if matches!(next, Some(Tok::Colon) | Some(Tok::Eq))
            || (next == Some(&Tok::FatArrow) && prev != Some(&Tok::Arrow))
            || matches!(prev, Some(Tok::For) | Some(Tok::DotDot))
        {
            return true;
        }
        // A bare name in a comma list binds only where a comma list *can* bind:
        // this item's own parameters, and a pattern — `Circle(r) =>`, `[x, ..r]
        // =>`, `first, ..rest =>`.
        //
        // Anywhere else a bare name in a comma list is an argument being passed.
        // Treating those as binders made every function value — `xs.map(quote)`,
        // `run_end(cs, i, is_alpha)` — a local of its caller: 351 sites in this
        // repository, each renaming to a single edit.
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

/// The token indices of this item's own parameter list — everything between the
/// `(` that follows the item's name and its matching `)`.
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

/// Is the name at `i` inside a `match` arm's pattern — that is, does a `=>`
/// close the phrase it sits in?
///
/// `match s { Circle(r) => … }` binds `r`; `xs.map(f(a, quote))` does not bind
/// `quote`, and the two look identical token by token until the arrow.
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
