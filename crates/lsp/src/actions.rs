use crate::binding;
use maca_core::{DiagKind, Diagnostic, Mode};
use maca_lexer::{Span, Tok, Token, lex};
use maca_parser::ast::*;
use maca_parser::parse;

/// A replacement of `src[start..end]`.
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
    /// An LSP `CodeActionKind`: `quickfix` for an action answering a diagnostic, `refactor.rewrite` for one the author asks for.
    pub kind: &'static str,
    pub edits: Vec<Edit>,
}

/// An action and the diagnostic message it claims to remove, which is what [`survives`] holds it to.
struct Candidate {
    action: Action,
    fixes: Option<String>,
}

/// Apply edits to `src`.
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

    /// `Immutable`: the constant was declared with an explicit `const`, so the edit that makes it mutable is dropping that word.
    fn mutable_fix(&self, d: &Diagnostic, out: &mut Vec<Candidate>) {
        let Some(name) = backticks(&d.msg).first().copied() else {
            return;
        };
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

    /// The edit that turns `const x = e` or `x = e as const` into a mutable binding, whichever of the two the declaration inside `region` is.
    fn drop_const(&self, region: Span, name: &str) -> Option<Edit> {
        let ts = self.tokens;
        for (i, t) in ts.iter().enumerate() {
            if t.span.0 < region.0 || t.span.0 >= region.1 {
                continue;
            }
            if t.tok == Tok::Const && names(ts.get(i + 1), name) {
                return Some(Edit {
                    start: t.span.0,
                    end: past_spaces(self.src, t.span.1),
                    new_text: String::new(),
                });
            }
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

    /// `UndefinedName` on a word Maca does not have.
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

    /// `UndefinedName` on a UFCS method.
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

    /// The nudge `maca lint` gives: a Capitalized local binding is a constant by convention, and the explicit `const` says so.
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

    /// `f() -> int => 1 + 2` becomes a block, when everything after the `=>` fits on the arrow's own line.
    fn arrow_to_block(&self, head: usize) -> Option<(Span, String, &'static str)> {
        let arrow = self.body_opener(head)?;
        if self.tokens[arrow].tok != Tok::FatArrow {
            return None;
        }
        let after = self.tokens[arrow].span.1;
        let end = line_end(self.src, after);
        let rest = self.src[after..end].trim();
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

    /// Each top-level function, paired with the token index of its name.
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

    /// The token that opens a function's body: the `=>` or the `{` after its signature.
    fn body_opener(&self, head: usize) -> Option<usize> {
        self.tokens[head..]
            .iter()
            .position(|t| matches!(t.tok, Tok::FatArrow | Tok::LBrace))
            .map(|i| i + head)
    }

    /// The byte range a function's definition covers, for the searches that must not wander into the next one.
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

    /// The `{`/`}` token indices of the innermost `match` block the selection sits in, if it sits in one.
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
            if self.from < t.span.0 || self.to > self.tokens[close].span.1 {
                continue;
            }
            if best.is_none_or(|(o, _)| self.tokens[open].span.0 > self.tokens[o].span.0) {
                best = Some((open, close));
            }
        }
        best
    }
}

/// Every `` `name` `` in a diagnostic message, in order.
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

/// The variants of the sum type `name`, each with the number of payload fields it carries.
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

/// Past the spaces and tabs following `byte`.
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
