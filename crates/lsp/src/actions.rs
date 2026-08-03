//! Quick fixes and refactorings, for `textDocument/codeAction`.
//!
//! Every quick fix here answers a diagnostic the checker already produced. The
//! server does not decide that a name is undefined, that a `match` is short an
//! arm, or that a constant was reassigned; `maca_core::check` decides, and this
//! turns its answer into the edit that performs it. Nothing is offered for a
//! file that does not parse, because then there is no checker answer to act on
//! and no reliable place to put an edit either.
//!
//! The rule every action is held to is that the file still parses and the
//! diagnostic is actually gone. That is not argued from the shape of the edit;
//! it is *executed*. [`survives`] applies the edit, re-parses the result and
//! re-checks it, and an action whose result would not parse, or which fails to
//! remove the diagnostic it claims, is dropped before the editor ever sees it.
//! A quick fix that reports success and leaves a broken file is the one outcome
//! an editor must not have, which is the same reason rename refuses a new name
//! that is not a name.
//!
//! Which action applies is decided from the cursor, not from the diagnostic's
//! range. A checker diagnostic carries no span (`diagnostics_located` anchors
//! it on the first name the message mentions), so the range an editor squiggles
//! is an approximation, while the cursor is exactly where the user asked.

use crate::binding;
use maca_core::{DiagKind, Diagnostic, Mode};
use maca_lexer::{Span, Tok, Token, lex};
use maca_parser::ast::*;
use maca_parser::parse;

/// A replacement of `src[start..end]`. `start == end` is an insertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub new_text: String,
}

/// One offer for the editor's lightbulb.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub title: String,
    /// An LSP `CodeActionKind`: `quickfix` for an action answering a
    /// diagnostic, `refactor.rewrite` for one the author asks for.
    pub kind: &'static str,
    pub edits: Vec<Edit>,
}

/// An action and the diagnostic message it claims to remove, which is what
/// [`survives`] holds it to. A refactoring claims none.
struct Candidate {
    action: Action,
    fixes: Option<String>,
}

/// Apply edits to `src`. Applied last first, so an earlier edit's offsets are
/// still the ones it was computed against.
pub fn apply_edits(src: &str, edits: &[Edit]) -> String {
    let mut out = src.to_string();
    let mut ordered: Vec<&Edit> = edits.iter().collect();
    ordered.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in ordered {
        out.replace_range(e.start..e.end, &e.new_text);
    }
    out
}

/// Every action available for the selection `from..to`.
///
/// `to` may equal `from`: an editor asks for actions at a bare cursor as often
/// as over a selection.
pub fn code_actions(src: &str, from: usize, to: usize, config: bool) -> Vec<Action> {
    let parsed = parse(src);
    if !parsed.errors.is_empty() {
        return Vec::new();
    }
    let mode = if config { Mode::Config } else { Mode::Program };
    let before = maca_core::check(&parsed.module, mode);
    let (from, to) = (from.min(to), from.max(to).min(src.len()));
    let tokens = lex(src).tokens;
    let at = Cursor {
        src,
        tokens: &tokens,
        module: &parsed.module,
        from,
        to,
    };

    let mut found = Vec::new();
    for d in &before {
        match d.kind {
            DiagKind::Immutable => at.mutable_fix(d, &mut found),
            DiagKind::UndefinedName => {
                at.phantom_fix(d, &mut found);
                at.method_fix(d, &mut found);
            }
            DiagKind::NonExhaustive => at.arms_fix(d, &mut found),
            _ => {}
        }
    }
    at.explicit_const(&mut found);
    at.body_style(&mut found);

    found
        .into_iter()
        .filter(|c| survives(src, c, &before, mode))
        .map(|c| c.action)
        .collect()
}

/// Does the edit leave a file that parses, and does it remove what it claims?
///
/// Executed rather than reasoned about. The generated `match` arms are the
/// clearest case: whether they are the right arms is exactly the question the
/// checker answers, so the check is run again and the diagnostic has to be
/// gone. The length comparison catches an edit that trades one diagnostic for
/// another, which a fix has no business doing.
fn survives(src: &str, c: &Candidate, before: &[Diagnostic], mode: Mode) -> bool {
    let edited = apply_edits(src, &c.action.edits);
    let parsed = parse(&edited);
    if !parsed.errors.is_empty() {
        return false;
    }
    let after = maca_core::check(&parsed.module, mode);
    if after.len() > before.len() {
        return false;
    }
    match &c.fixes {
        Some(msg) => count(&after, msg) < count(before, msg),
        None => true,
    }
}

fn count(diags: &[Diagnostic], msg: &str) -> usize {
    diags.iter().filter(|d| d.msg == msg).count()
}

/// The file and the selection, which every action needs all of.
struct Cursor<'a> {
    src: &'a str,
    tokens: &'a [Token],
    module: &'a Module,
    from: usize,
    to: usize,
}

impl Cursor<'_> {
    fn touches(&self, span: Span) -> bool {
        span.0 <= self.to && span.1 >= self.from
    }

    // ---- the quick fixes --------------------------------------------------

    /// `Immutable`: the constant was declared with an explicit `const`, so the
    /// edit that makes it mutable is dropping that word.
    ///
    /// A Capitalized name is a constant with no `const` to drop, and the only
    /// edit that would make it mutable is renaming it. That is a rename, with a
    /// rename's collision question, and rename is a request of its own next
    /// door.
    fn mutable_fix(&self, d: &Diagnostic, out: &mut Vec<Candidate>) {
        let Some(name) = backticks(&d.msg).first().copied() else {
            return;
        };
        // The cursor has to be on the name this diagnostic is about, resolved
        // the way rename resolves it, so a local `limit` in one function never
        // offers to edit another function's declaration of the same name.
        let b = binding::resolve(self.src, self.from)
            .or_else(|| binding::resolve(self.src, self.to))
            .filter(|b| b.name == name);
        let Some(b) = b else { return };
        let region = match b.scope {
            binding::Scope::Local(span) => span,
            _ => (0, self.src.len()),
        };
        let Some(edit) = self.drop_const(region, name) else {
            return;
        };
        out.push(Candidate {
            action: Action {
                title: format!("declare `{name}` mutable"),
                kind: "quickfix",
                edits: vec![edit],
            },
            fixes: Some(d.msg.clone()),
        });
    }

    /// The edit that turns `const x = e` or `x = e as const` into a mutable
    /// binding, whichever of the two the declaration inside `region` is.
    fn drop_const(&self, region: Span, name: &str) -> Option<Edit> {
        let ts = self.tokens;
        for (i, t) in ts.iter().enumerate() {
            if t.span.0 < region.0 || t.span.0 >= region.1 {
                continue;
            }
            // `const x = e`: the keyword goes, and the space after it.
            if t.tok == Tok::Const && names(ts.get(i + 1), name) {
                return Some(Edit {
                    start: t.span.0,
                    end: past_spaces(self.src, t.span.1),
                    new_text: String::new(),
                });
            }
            // `x = e as const`: the suffix goes, and the space before it.
            if names(Some(t), name) && tok(ts, i + 1) == Some(&Tok::Eq) {
                for j in i + 2..ts.len() {
                    match ts[j].tok {
                        Tok::Newline => break,
                        Tok::As if tok(ts, j + 1) == Some(&Tok::Const) => {
                            return Some(Edit {
                                start: ts[j - 1].span.1,
                                end: ts[j + 1].span.1,
                                new_text: String::new(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// `UndefinedName` on a word Maca does not have: the diagnostic says what
    /// Maca does instead, and for the two that are a deletion the fix performs
    /// it.
    ///
    /// `null` also reaches here and is not a deletion: what replaces it is a
    /// sum type the author has to design. `fn`, `type` and `def` are absent for
    /// a different reason, which is that they never reach the checker at all:
    /// `fn f() -> int { … }` parses as a call juxtaposed with a block, and
    /// raises nothing to fix.
    fn phantom_fix(&self, d: &Diagnostic, out: &mut Vec<Candidate>) {
        let Some(name) = backticks(&d.msg).first().copied() else {
            return;
        };
        if !matches!(name, "return" | "let" | "var") {
            return;
        }
        let Some(t) = self
            .tokens
            .iter()
            .find(|t| names(Some(t), name) && self.touches(t.span))
        else {
            return;
        };
        out.push(Candidate {
            action: Action {
                title: format!("drop the `{name}`"),
                kind: "quickfix",
                edits: vec![Edit {
                    start: t.span.0,
                    end: past_spaces(self.src, t.span.1),
                    new_text: String::new(),
                }],
            },
            fixes: Some(d.msg.clone()),
        });
    }

    /// `UndefinedName` on a UFCS method: `STR_METHODS` and `LIST_METHODS` are
    /// closed sets, so the checker has already picked the nearest real name.
    /// The fix spells it.
    fn method_fix(&self, d: &Diagnostic, out: &mut Vec<Candidate>) {
        let quoted = backticks(&d.msg);
        if quoted.len() != 3 || !d.msg.contains("did you mean") {
            return;
        }
        let (name, alt) = (quoted[1], quoted[2]);
        let Some(i) = self.tokens.iter().position(|t| {
            names(Some(t), name) && self.touches(t.span) && before_is_dot(self.tokens, t)
        }) else {
            return;
        };
        out.push(Candidate {
            action: Action {
                title: format!("change `{name}` to `{alt}`"),
                kind: "quickfix",
                edits: vec![Edit {
                    start: self.tokens[i].span.0,
                    end: self.tokens[i].span.1,
                    new_text: alt.to_string(),
                }],
            },
            fixes: Some(d.msg.clone()),
        });
    }

    /// `NonExhaustive`: add the arms the checker says are missing.
    ///
    /// The variants come from the diagnostic; only their *arity* is read off
    /// the declaration, because a payload variant needs somewhere to put its
    /// payload and `Rect(a0, a1) =>` is the pattern that has one. The body is
    /// `fail`, whose type is fresh and so fits whatever the other arms return:
    /// an arm that did not type-check would trade one diagnostic for another,
    /// and `survives` would drop the whole action.
    fn arms_fix(&self, d: &Diagnostic, out: &mut Vec<Candidate>) {
        let Some(ty) = backticks(&d.msg).first().copied() else {
            return;
        };
        let Some((_, missing)) = d.msg.split_once("missing: ") else {
            return;
        };
        let missing: Vec<&str> = missing.split(',').map(str::trim).collect();
        let arity = variant_arity(self.module, ty);
        if missing.iter().any(|v| !arity.iter().any(|(n, _)| n == v)) {
            return;
        }
        // The innermost `match` around the cursor is the one being asked about.
        // A file with two short matches on the same type produces two
        // diagnostics with the same text, and the cursor is what tells them
        // apart. Which arms that match already has is not asked here: the
        // checker answered it once, and an arm list read a second time from the
        // tokens would only be a second answer to disagree with the first. A
        // wrong pairing shows up as a diagnostic that did not go away, and
        // `survives` drops the action for it.
        let Some((_, close)) = self.innermost_match() else {
            return;
        };

        let brace = self.tokens[close].span.0;
        let indent = indent_of(self.src, brace);
        let arms: String = missing
            .iter()
            .map(|v| {
                let n = arity.iter().find(|(n, _)| n == v).map_or(0, |(_, a)| *a);
                let binds: Vec<String> = (0..n).map(|i| format!("a{i}")).collect();
                let pat = if n == 0 {
                    (*v).to_string()
                } else {
                    format!("{v}({})", binds.join(", "))
                };
                format!("{indent}    {pat} => fail \"todo: {v}\"\n")
            })
            .collect();
        // A closing brace that starts its own line takes the new arms above it.
        // One that shares a line, `match c { Red => "r" }`, has to be pushed
        // down, or the new arm would run into the old one with no separator.
        // The space it was sitting behind goes with it, because a newline put
        // in front of it would otherwise leave that space at end of line.
        let (start, end, text) = if line_start(self.src, brace) + indent.len() == brace {
            (brace - indent.len(), brace - indent.len(), arms)
        } else {
            (
                before_spaces(self.src, brace),
                brace,
                format!("\n{arms}{indent}"),
            )
        };
        out.push(Candidate {
            action: Action {
                title: format!("fill in the missing `{ty}` arms"),
                kind: "quickfix",
                edits: vec![Edit {
                    start,
                    end,
                    new_text: text,
                }],
            },
            fixes: Some(d.msg.clone()),
        });
    }

    // ---- the refactorings -------------------------------------------------

    /// The nudge `maca lint` gives: a Capitalized local binding is a constant
    /// by convention, and the explicit `const` says so.
    ///
    /// Local, exactly as the linter has it, because a Capitalized name at the
    /// top level is usually a record or a sum, and `const Point = { x: int }`
    /// is not what anybody meant by a type declaration.
    fn explicit_const(&self, out: &mut Vec<Candidate>) {
        for (head, f) in self.fn_heads() {
            let Some(FnBody::Block(stmts)) = &f.body else {
                continue;
            };
            let region = self.body_region(head);
            for b in stmts.iter().filter_map(bind_of) {
                let Expr::Ident(n) = &b.target else { continue };
                if !n.chars().next().is_some_and(char::is_uppercase) {
                    continue;
                }
                // The *first* `Name =` that starts a statement in this function
                // is the declaration; a later one is an assignment to it, and
                // `const` in front of that says something different.
                let Some(i) = (0..self.tokens.len()).find(|i| {
                    let t = &self.tokens[*i];
                    t.span.0 >= region.0
                        && t.span.0 < region.1
                        && names(Some(t), n)
                        && tok(self.tokens, i + 1) == Some(&Tok::Eq)
                        && matches!(
                            i.checked_sub(1).and_then(|j| tok(self.tokens, j)),
                            None | Some(Tok::Newline | Tok::LBrace | Tok::Const)
                        )
                }) else {
                    continue;
                };
                // Already explicit, which is the state this action aims at.
                if i > 0 && self.tokens[i - 1].tok == Tok::Const {
                    continue;
                }
                if !self.touches(self.tokens[i].span) {
                    continue;
                }
                out.push(Candidate {
                    action: Action {
                        title: format!("declare `{n}` with an explicit `const`"),
                        kind: "refactor.rewrite",
                        edits: vec![Edit {
                            start: self.tokens[i].span.0,
                            end: self.tokens[i].span.0,
                            new_text: "const ".to_string(),
                        }],
                    },
                    fixes: None,
                });
            }
        }
    }

    /// Switch a function body between `=> e` and `{ e }`.
    ///
    /// Both spell the same function, and which reads better depends on what the
    /// body has grown into. The edit is textual and minimal: the body's own
    /// text is moved, never reprinted, so a comment on it survives. That is
    /// also why only the single-expression shapes are offered. A `=> e` spread
    /// over several lines, a bracketless comma list (`f() -> int[] => 1, 2`,
    /// which is not one expression), a block of several statements, and a body
    /// that is itself a brace all have no line-for-line reading, and an action
    /// that guessed at one would be rewriting the code rather than restating
    /// it.
    fn body_style(&self, out: &mut Vec<Candidate>) {
        for (head, f) in self.fn_heads() {
            let Some(body) = &f.body else { continue };
            let candidate = match body {
                FnBody::Expr(_) => self.arrow_to_block(head),
                FnBody::Block(stmts) => self.block_to_arrow(head, stmts),
            };
            if let Some((span, text, title)) = candidate
                && self.touches(span)
            {
                out.push(Candidate {
                    action: Action {
                        title: title.to_string(),
                        kind: "refactor.rewrite",
                        edits: vec![Edit {
                            start: span.0,
                            end: span.1,
                            new_text: text,
                        }],
                    },
                    fixes: None,
                });
            }
        }
    }

    /// `f() -> int => 1 + 2` becomes a block, when everything after the `=>`
    /// fits on the arrow's own line.
    fn arrow_to_block(&self, head: usize) -> Option<(Span, String, &'static str)> {
        let arrow = self.body_opener(head)?;
        if self.tokens[arrow].tok != Tok::FatArrow {
            return None;
        }
        let after = self.tokens[arrow].span.1;
        let end = line_end(self.src, after);
        let rest = self.src[after..end].trim();
        // A `{` here opens a record literal or a block, and wrapping either in
        // one more brace is not the same function.
        if rest.is_empty() || rest.starts_with('{') {
            return None;
        }
        let indent = indent_of(self.src, self.tokens[arrow].span.0);
        Some((
            (self.tokens[arrow].span.0, end),
            format!("{{\n{indent}    {rest}\n{indent}}}"),
            "use a block body",
        ))
    }

    /// The other direction, for a block whose one statement is an expression.
    fn block_to_arrow(&self, head: usize, stmts: &[Stmt]) -> Option<(Span, String, &'static str)> {
        if stmts.len() != 1 || !matches!(stmts[0], Stmt::Expr(_)) {
            return None;
        }
        let open = self.body_opener(head)?;
        if self.tokens[open].tok != Tok::LBrace {
            return None;
        }
        let close = matching_brace(self.tokens, open)?;
        let inner = self
            .src
            .get(self.tokens[open].span.1..self.tokens[close].span.0)?;
        // One line of content, so nothing above or below it is dropped. A
        // comment on its own line would be, which is the case this refuses.
        let mut lines = inner.lines().filter(|l| !l.trim().is_empty());
        let body = lines.next()?.trim().to_string();
        if lines.next().is_some() || body.starts_with("//") {
            return None;
        }
        Some((
            (self.tokens[open].span.0, self.tokens[close].span.1),
            format!("=> {body}"),
            "use a `=>` body",
        ))
    }

    // ---- reading the file -------------------------------------------------

    /// Each top-level function, paired with the token index of its name.
    ///
    /// The same column-zero rule the outline and `binding` use: a name at the
    /// start of a line, followed by the `(` that makes it a signature.
    fn fn_heads(&self) -> Vec<(usize, &FnDef)> {
        let mut out = Vec::new();
        for (i, t) in self.tokens.iter().enumerate() {
            let Tok::Ident(n) = &t.tok else { continue };
            if !at_line_start(self.src, t.span.0) || tok(self.tokens, i + 1) != Some(&Tok::LParen) {
                continue;
            }
            if let Some(f) = self
                .module
                .items
                .iter()
                .filter_map(fn_of)
                .find(|f| &f.name == n)
            {
                out.push((i, f));
            }
        }
        out
    }

    /// The token that opens a function's body: the `=>` or the `{` after its
    /// signature.
    fn body_opener(&self, head: usize) -> Option<usize> {
        self.tokens[head..]
            .iter()
            .position(|t| matches!(t.tok, Tok::FatArrow | Tok::LBrace))
            .map(|i| i + head)
    }

    /// The byte range a function's definition covers, for the searches that
    /// must not wander into the next one.
    fn body_region(&self, head: usize) -> Span {
        let start = self.tokens[head].span.0;
        let end = match self.body_opener(head) {
            Some(open) if self.tokens[open].tok == Tok::LBrace => matching_brace(self.tokens, open)
                .map(|c| self.tokens[c].span.1)
                .unwrap_or(self.src.len()),
            Some(open) => line_end(self.src, self.tokens[open].span.1),
            None => self.src.len(),
        };
        (start, end)
    }

    /// The `{`/`}` token indices of the innermost `match` block the selection
    /// sits in, if it sits in one.
    fn innermost_match(&self) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize)> = None;
        for (i, t) in self.tokens.iter().enumerate() {
            if t.tok != Tok::Match {
                continue;
            }
            let Some(open) = self.tokens[i..].iter().position(|u| u.tok == Tok::LBrace) else {
                continue;
            };
            let open = open + i;
            let Some(close) = matching_brace(self.tokens, open) else {
                continue;
            };
            // From the keyword, so a cursor on `match` itself counts.
            if self.from < t.span.0 || self.to > self.tokens[close].span.1 {
                continue;
            }
            // Innermost wins: a `match` inside an arm of another one is the one
            // the cursor is in.
            if best.is_none_or(|(o, _)| self.tokens[open].span.0 > self.tokens[o].span.0) {
                best = Some((open, close));
            }
        }
        best
    }
}

/// Every `` `name` `` in a diagnostic message, in order. The checker's messages
/// quote what they are about, which is how the name being complained of reaches
/// the edit without a second analysis deciding it.
fn backticks(msg: &str) -> Vec<&str> {
    msg.split('`')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s)
        .collect()
}

fn tok(tokens: &[Token], i: usize) -> Option<&Tok> {
    tokens.get(i).map(|t| &t.tok)
}

fn names(t: Option<&Token>, name: &str) -> bool {
    matches!(t.map(|t| &t.tok), Some(Tok::Ident(n)) if n == name)
}

fn before_is_dot(tokens: &[Token], t: &Token) -> bool {
    tokens
        .iter()
        .position(|u| u.span == t.span)
        .is_some_and(|i| i > 0 && tokens[i - 1].tok == Tok::Dot)
}

/// The variants of the sum type `name`, each with the number of payload fields
/// it carries: `Shape = Circle(int) | Rect(int, int)` gives
/// `[(Circle, 1), (Rect, 2)]`.
fn variant_arity(module: &Module, name: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for b in module.items.iter().filter_map(bind_of) {
        if matches!(&b.target, Expr::Ident(n) if n == name) {
            collect_variants(&b.value, &mut out);
        }
    }
    out
}

fn collect_variants(e: &Expr, out: &mut Vec<(String, usize)>) {
    match e {
        Expr::Binary {
            op: BinOp::Union,
            lhs,
            rhs,
        } => {
            collect_variants(lhs, out);
            collect_variants(rhs, out);
        }
        Expr::Ident(n) => out.push((n.clone(), 0)),
        Expr::Call { callee, args } => {
            if let Expr::Ident(n) = &**callee {
                out.push((n.clone(), args.len()));
            }
        }
        _ => {}
    }
}

fn matching_brace(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, t) in tokens.iter().enumerate().skip(open) {
        match t.tok {
            Tok::LBrace => depth += 1,
            Tok::RBrace => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn fn_of(s: &Stmt) -> Option<&FnDef> {
    match s {
        Stmt::Fn(f) => Some(f),
        _ => None,
    }
}

fn bind_of(s: &Stmt) -> Option<&Bind> {
    match s {
        Stmt::Bind(b) => Some(b),
        _ => None,
    }
}

fn at_line_start(src: &str, byte: usize) -> bool {
    byte == 0 || src.as_bytes()[byte - 1] == b'\n'
}

fn line_start(src: &str, byte: usize) -> usize {
    src[..byte].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(src: &str, byte: usize) -> usize {
    src[byte..].find('\n').map_or(src.len(), |i| byte + i)
}

/// The leading whitespace of the line `byte` sits on.
fn indent_of(src: &str, byte: usize) -> String {
    src[line_start(src, byte)..byte]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

/// Past the spaces and tabs following `byte`, so dropping a word takes the gap
/// it left with it rather than leaving `return  x` as a doubly indented `x`.
fn past_spaces(src: &str, byte: usize) -> usize {
    let b = src.as_bytes();
    let mut end = byte;
    while end < b.len() && (b[end] == b' ' || b[end] == b'\t') {
        end += 1;
    }
    end
}

/// The mirror: back over the spaces and tabs preceding `byte`.
fn before_spaces(src: &str, byte: usize) -> usize {
    let b = src.as_bytes();
    let mut start = byte;
    while start > 0 && (b[start - 1] == b' ' || b[start - 1] == b'\t') {
        start -= 1;
    }
    start
}
