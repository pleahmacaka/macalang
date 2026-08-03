//! Hand-written recursive-descent + Pratt parser over the lexer's token stream.
//!
//! Layout is already resolved by the lexer (newlines are real tokens, suppressed
//! where they continue), so the parser treats `Newline` as a statement/field
//! separator and otherwise ignores it. `no_brace` mode is set while parsing a
//! control-flow header (`if`/`for`/`match` scrutinee) so a following `{` is read
//! as the block, not a `Name { .. }` constructor.

use crate::ast::*;
use crate::braces::{self, ArrowBrace};
use maca_lexer::{Span, Tok, Token};

#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    pub msg: String,
    pub span: Span,
}

pub struct Parser {
    toks: Vec<Token>,
    i: usize,
    pub errors: Vec<ParseError>,
    no_brace: bool,
}

impl Parser {
    pub fn new(toks: Vec<Token>) -> Self {
        Parser {
            toks,
            i: 0,
            errors: Vec::new(),
            no_brace: false,
        }
    }

    // ---- cursor -----------------------------------------------------------

    fn peek(&self) -> &Tok {
        &self.toks[self.i.min(self.toks.len() - 1)].tok
    }
    fn peekn(&self, n: usize) -> &Tok {
        &self.toks[(self.i + n).min(self.toks.len() - 1)].tok
    }
    fn span(&self) -> Span {
        self.toks[self.i.min(self.toks.len() - 1)].span
    }
    fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }
    fn bump(&mut self) -> Tok {
        let t = self.peek().clone();
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
        t
    }
    fn at(&self, t: Tok) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(&t)
    }
    fn eat(&mut self, t: Tok) -> bool {
        if self.at(t) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Tok, what: &str) {
        if !self.eat(t) {
            let found = self.peek().clone();
            self.err(format!("expected {what}, found {found:?}"));
        }
    }
    fn err(&mut self, msg: impl Into<String>) {
        self.errors.push(ParseError {
            msg: msg.into(),
            span: self.span(),
        });
    }
    fn ident(&mut self) -> Ident {
        if let Tok::Ident(s) = self.peek().clone() {
            self.bump();
            s
        } else {
            let found = self.peek().clone();
            self.err(format!("expected identifier, found {found:?}"));
            // consume the offending token so callers that loop on `ident()`
            // (record fields/patterns, params) can't stall on it forever
            if !self.at_eof() {
                self.bump();
            }
            "?".into()
        }
    }
    fn skip_newlines(&mut self) {
        while self.at(Tok::Newline) {
            self.bump();
        }
    }
    fn skip_seps(&mut self) {
        while self.at(Tok::Newline) || self.at(Tok::Comma) {
            self.bump();
        }
    }

    // ---- module -----------------------------------------------------------

    pub fn parse_module(&mut self) -> Module {
        let mut items = Vec::new();
        self.skip_newlines();
        while !self.at_eof() {
            let before = self.i;
            items.push(self.parse_stmt());
            if self.i == before {
                self.bump(); // guarantee progress on error
            }
            self.skip_newlines();
        }
        Module { items }
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.peek() {
            Tok::Import => Stmt::Import(self.parse_import()),
            Tok::Alias => self.parse_alias(),
            Tok::Const => {
                self.bump();
                let target = self.parse_expr();
                Stmt::Bind(self.finish_bind(true, target))
            }
            _ if self.def_ahead() => Stmt::Fn(self.parse_fn()),
            _ => {
                let e = self.parse_expr();
                if self.at(Tok::Colon) || self.at(Tok::Eq) {
                    Stmt::Bind(self.finish_bind(false, e))
                } else {
                    Stmt::Expr(e)
                }
            }
        }
    }

    fn finish_bind(&mut self, const_kw: bool, target: Expr) -> Bind {
        let mut tys = Vec::new();
        while self.eat(Tok::Colon) {
            tys.push(self.parse_type());
        }
        self.expect(Tok::Eq, "'=' in binding");
        let value = self.parse_list_expr();
        // A binding is a constant if it used `const`, a trailing `as const`, or a
        // Capitalized name (the convention; the linter nudges toward explicit).
        let as_const = self.at(Tok::As) && matches!(self.peekn(1), Tok::Const);
        if as_const {
            self.bump();
            self.bump();
        }
        let capital =
            matches!(&target, Expr::Ident(n) if n.chars().next().is_some_and(|c| c.is_uppercase()));
        let is_const = const_kw || as_const || capital;
        Bind {
            is_const,
            target,
            tys,
            value,
        }
    }

    /// `ident ( ... )` followed by `->`, `{`, or `=>` is a function definition.
    fn def_ahead(&self) -> bool {
        if !matches!(self.peek(), Tok::Ident(_)) || !matches!(self.peekn(1), Tok::LParen) {
            return false;
        }
        let mut depth = 0i32;
        let mut j = self.i + 1;
        loop {
            match self.toks.get(j).map(|t| &t.tok) {
                Some(Tok::LParen) => depth += 1,
                Some(Tok::RParen) => {
                    depth -= 1;
                    if depth == 0 {
                        j += 1;
                        break;
                    }
                }
                Some(Tok::Eof) | None => return false,
                _ => {}
            }
            j += 1;
        }
        while matches!(self.toks.get(j).map(|t| &t.tok), Some(Tok::Newline)) {
            j += 1;
        }
        matches!(
            self.toks.get(j).map(|t| &t.tok),
            Some(Tok::Arrow | Tok::LBrace | Tok::FatArrow)
        )
    }

    fn parse_import(&mut self) -> Import {
        self.bump(); // import
        match self.peek().clone() {
            Tok::LBrace => {
                self.bump();
                let mut names = Vec::new();
                self.skip_seps();
                while !self.at(Tok::RBrace) && !self.at_eof() {
                    names.push(self.ident());
                    self.skip_seps();
                }
                self.expect(Tok::RBrace, "'}'");
                // `from` is an ordinary identifier everywhere else. It is far
                // too natural a parameter name (`copy(from, to)`) to spend a
                // keyword on, and here the grammar already knows what follows a
                // selective import's brace list.
                match self.peek().clone() {
                    Tok::Ident(n) if n == "from" => {
                        self.bump();
                    }
                    other => self.err(format!("expected 'from', found {other:?}")),
                }
                let module = self.parse_module_path();
                Import::Names { names, module }
            }
            Tok::Path(p) => {
                self.bump();
                Import::Path(p)
            }
            Tok::Ident(first) => {
                self.bump();
                if matches!(self.peek(), Tok::StrOpen) {
                    let spec = self.parse_string_literal();
                    Import::Foreign { lang: first, spec }
                } else if self.at(Tok::Slash) {
                    let mut segs = vec![first];
                    while self.eat(Tok::Slash) {
                        segs.push(self.ident());
                    }
                    Import::Module(segs)
                } else {
                    Import::Bare(first)
                }
            }
            other => {
                self.err(format!("bad import target {other:?}"));
                Import::Bare("?".into())
            }
        }
    }

    fn parse_module_path(&mut self) -> Vec<Ident> {
        let mut segs = vec![self.ident()];
        while self.eat(Tok::Slash) {
            segs.push(self.ident());
        }
        segs
    }

    fn parse_alias(&mut self) -> Stmt {
        self.bump(); // alias
        let name = self.ident();
        self.expect(Tok::Eq, "'=' in alias");
        let value = self.parse_expr();
        Stmt::Alias { name, value }
    }

    fn parse_fn(&mut self) -> FnDef {
        let name = self.ident();
        self.expect(Tok::LParen, "'('");
        let params = self.parse_params();
        self.expect(Tok::RParen, "')'");
        let ret = if self.eat(Tok::Arrow) {
            Some(self.parse_type())
        } else {
            None
        };
        let effects = if self.at(Tok::Slash) && matches!(self.peekn(1), Tok::Lt) {
            self.bump(); // /
            Some(self.parse_effect_row())
        } else {
            None
        };
        let body = if self.at(Tok::LBrace) {
            Some(FnBody::Block(self.parse_block()))
        } else if self.eat(Tok::FatArrow) {
            Some(self.parse_arrow_body(&name))
        } else {
            None
        };
        FnDef {
            name,
            params,
            ret,
            effects,
            body,
        }
    }

    /// The body after a function's `=>`.
    ///
    /// A bracketless comma list is a valid arrow body (`make() -> int[] => 1,
    /// 2, 3`) and a single expression parses unchanged. The one shape that has
    /// to be decided is a leading `{`, which is a record literal and a block at
    /// the same time.
    fn parse_arrow_body(&mut self, name: &str) -> FnBody {
        if self.at(Tok::LBrace) {
            match self.arrow_brace() {
                // `f() -> T => { … }` means `f() -> T { … }`, so it is parsed
                // as one: every back end already lowers a block body, and the
                // printer prints back the spelling without the spare `=>`.
                ArrowBrace::Block => return FnBody::Block(self.parse_block()),
                ArrowBrace::Record => {}
                ArrowBrace::Both => {
                    self.err(format!(
                        "`{name}`: this `=> {{ … }}` reads as a record literal \
                         and as a block. Write `Name {{ … }}` for the record, \
                         or drop the `=>` for the block"
                    ));
                    return FnBody::Block(self.parse_block());
                }
            }
        }
        FnBody::Expr(Box::new(self.parse_list_expr()))
    }

    /// Decide the `{` at the cursor, which follows this function's `=>`.
    ///
    /// The scan lives in `braces` because the language server needs the same
    /// answer from the same tokens, and a second copy of it there was already
    /// wrong about this case.
    fn arrow_brace(&self) -> ArrowBrace {
        braces::arrow_brace(&self.toks, self.i)
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut ps = Vec::new();
        self.skip_seps();
        while !self.at(Tok::RParen) && !self.at_eof() {
            let before = self.i;
            let variadic = self.eat(Tok::Ellipsis);
            let name = self.ident();
            let ty = if self.eat(Tok::Colon) {
                Some(self.parse_type())
            } else {
                None
            };
            ps.push(Param { name, ty, variadic });
            self.skip_seps();
            if self.i == before {
                self.bump(); // guarantee progress on malformed params (no infinite loop)
            }
        }
        ps
    }

    fn parse_effect_row(&mut self) -> Vec<Ident> {
        self.expect(Tok::Lt, "'<'");
        let mut effs = Vec::new();
        while !self.at(Tok::Gt) && !self.at_eof() {
            let before = self.i;
            effs.push(self.ident());
            self.eat(Tok::Comma);
            if self.i == before {
                self.bump(); // guarantee progress on a malformed effect row
            }
        }
        self.expect(Tok::Gt, "'>'");
        effs
    }

    // ---- types ------------------------------------------------------------

    fn parse_type(&mut self) -> Type {
        let head = self.parse_type_postfix();
        let mut args = Vec::new();
        while matches!(self.peek(), Tok::Ident(_) | Tok::LParen) {
            args.push(self.parse_type_postfix());
        }
        if args.is_empty() {
            head
        } else {
            Type::Apply(Box::new(head), args)
        }
    }

    fn parse_type_postfix(&mut self) -> Type {
        let mut t = self.parse_type_atom();
        loop {
            if self.eat(Tok::LBracket) {
                self.expect(Tok::RBracket, "']'");
                t = Type::Array(Box::new(t));
            } else if self.eat(Tok::QuestionPost) || self.eat(Tok::Question) {
                t = Type::Opt(Box::new(t));
            } else {
                break;
            }
        }
        t
    }

    fn parse_type_atom(&mut self) -> Type {
        match self.peek().clone() {
            Tok::Ident(_) => {
                let mut segs = vec![self.ident()];
                while self.at(Tok::Dot) && matches!(self.peekn(1), Tok::Ident(_)) {
                    self.bump();
                    segs.push(self.ident());
                }
                Type::Name(segs)
            }
            // `(T)` is a grouped type; `(T, U) -> R` and `() -> R` are the
            // type of a function value. Which one it is cannot be known before
            // the `)`, so both are parsed the same way and the arrow decides.
            Tok::LParen => {
                self.bump();
                let mut parts = Vec::new();
                while !self.at(Tok::RParen) && !self.at(Tok::Eof) {
                    parts.push(self.parse_type());
                    if !self.eat(Tok::Comma) {
                        break;
                    }
                }
                self.expect(Tok::RParen, "')'");
                if self.eat(Tok::Arrow) {
                    return Type::Fn(parts, Box::new(self.parse_type()));
                }
                match parts.len() {
                    1 => Type::Paren(Box::new(parts.remove(0))),
                    0 => {
                        self.err("expected a type, or `-> R` to make this a function type");
                        Type::Name(vec!["?".into()])
                    }
                    _ => {
                        self.err("a parenthesised list of types needs `-> R` after it");
                        Type::Name(vec!["?".into()])
                    }
                }
            }
            other => {
                self.err(format!("expected type, found {other:?}"));
                Type::Name(vec!["?".into()])
            }
        }
    }

    // ---- expressions ------------------------------------------------------

    fn parse_expr(&mut self) -> Expr {
        if let Some(e) = self.typed_lambda() {
            return e;
        }
        let lhs = self.parse_ternary();
        if let Some(ret) = self.lambda_ret() {
            let params = self.expr_to_params(lhs);
            return Expr::Lambda {
                params,
                ret,
                body: Box::new(self.parse_lambda_body()),
            };
        }
        lhs
    }

    /// `(a: T, b) [-> R] => body`: a lambda whose parameters carry types.
    ///
    /// A parameter list is not an expression, so the usual route (parse an
    /// expression, then reinterpret it as parameters) cannot see the `: T`.
    /// When the tokens ahead are a parenthesized list followed by the lambda
    /// arrow, the list is parsed as what it is.
    fn typed_lambda(&mut self) -> Option<Expr> {
        if !self.at(Tok::LParen) || !self.typed_params_ahead() {
            return None;
        }
        self.bump(); // '('
        let params = self.parse_params();
        self.expect(Tok::RParen, "')'");
        let ret = if self.eat(Tok::Arrow) {
            Some(self.parse_type())
        } else {
            None
        };
        self.expect(Tok::FatArrow, "'=>'");
        Some(Expr::Lambda {
            params,
            ret,
            body: Box::new(self.parse_lambda_body()),
        })
    }

    /// Does a `(` here open a parameter list with at least one `name: Type` in
    /// it, closed and followed by the lambda arrow? Only then is it worth
    /// parsing as parameters rather than as an expression. `(a, b) => …` is
    /// handled fine by the existing path, and `(a + b)` must stay an
    /// expression.
    fn typed_params_ahead(&self) -> bool {
        let mut depth = 0usize;
        let mut typed = false;
        let mut i = 0usize;
        loop {
            match self.peekn(i) {
                Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RParen | Tok::RBracket => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                // a `:` directly inside this list is a parameter's type; deeper
                // in, it belongs to a nested list or a ternary.
                Tok::Colon if depth == 1 => typed = true,
                Tok::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        if !typed {
            return false;
        }
        matches!(self.peekn(i + 1), Tok::FatArrow | Tok::Arrow)
    }

    /// The arrow between a lambda's parameters and its body, with an optional
    /// return type in front of it: `(a, b) => …` or `(a, b) -> T => …`.
    ///
    /// `Some(None)` is a lambda with no annotation, `Some(Some(t))` one with,
    /// and `None` means this was not a lambda at all. A declared type is what a
    /// trait-impl method needs when it has to match a signature the compiler
    /// cannot read.
    fn lambda_ret(&mut self) -> Option<Option<Type>> {
        if self.eat(Tok::FatArrow) {
            return Some(None);
        }
        if self.at(Tok::Arrow) && self.lambda_arrow_ahead() {
            self.bump();
            let t = self.parse_type();
            self.expect(Tok::FatArrow, "'=>'");
            return Some(Some(t));
        }
        None
    }

    /// Is this `-> T` the head of a lambda rather than a function signature?
    /// Only a `=>` after the type makes it one, so scan forward for it before
    /// the next token that could not be part of a type.
    fn lambda_arrow_ahead(&self) -> bool {
        let mut i = 1;
        loop {
            match self.peekn(i) {
                Tok::FatArrow => return true,
                Tok::Ident(_)
                | Tok::Dot
                | Tok::LBracket
                | Tok::RBracket
                | Tok::Question
                | Tok::QuestionPost
                | Tok::LParen
                | Tok::RParen => i += 1,
                _ => return false,
            }
        }
    }

    /// Lambda bodies additionally allow a `{ … }` block, the UI setter form
    /// `x = e` (assign expr), and nested lambdas.
    fn parse_lambda_body(&mut self) -> Expr {
        // `=> { … }` is a block, the way a match arm's `=> { … }` is. An
        // anonymous record still has its own syntax everywhere a value is
        // wanted; what it cannot be is the *whole* body of a lambda, which is
        // the same trade a match arm already makes.
        if self.at(Tok::LBrace) {
            return Expr::Block(self.parse_block());
        }
        if let Some(e) = self.typed_lambda() {
            return e;
        }
        let lhs = self.parse_ternary();
        if let Some(ret) = self.lambda_ret() {
            let params = self.expr_to_params(lhs);
            Expr::Lambda {
                params,
                ret,
                body: Box::new(self.parse_lambda_body()),
            }
        } else if self.at(Tok::Eq) {
            self.bump();
            Expr::Assign {
                target: Box::new(lhs),
                value: Box::new(self.parse_lambda_body()),
            }
        } else {
            lhs
        }
    }

    fn parse_ternary(&mut self) -> Expr {
        let cond = self.parse_range();
        if self.eat(Tok::Question) {
            let then = self.parse_ternary();
            self.expect(Tok::Colon, "':' in ternary");
            let els = self.parse_ternary();
            Expr::Ternary {
                cond: Box::new(cond),
                then: Box::new(then),
                els: Box::new(els),
            }
        } else {
            cond
        }
    }

    /// `lo..hi`, an inclusive integer range (lo … hi), one notch looser than
    /// the binary operators so `1..n - 1` reads as `1..(n - 1)`. Non-associative:
    /// a range isn't itself a range endpoint.
    fn parse_range(&mut self) -> Expr {
        let lo = self.parse_binary(0);
        if self.eat(Tok::DotDot) {
            let hi = self.parse_binary(0);
            Expr::Range {
                lo: Box::new(lo),
                hi: Box::new(hi),
            }
        } else {
            lo
        }
    }

    fn parse_binary(&mut self, min_bp: u8) -> Expr {
        let mut lhs = self.parse_unary();
        while let Some((op, bp)) = self.bin_op() {
            if bp < min_bp {
                break;
            }
            self.bump();
            let rhs = self.parse_binary(bp + 1);
            lhs = if op == BinOp::Pipe {
                piped(lhs, rhs)
            } else {
                Expr::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            };
        }
        lhs
    }

    fn bin_op(&self) -> Option<(BinOp, u8)> {
        Some(match self.peek() {
            Tok::BarBar => (BinOp::Or, 1),
            Tok::Bar => (BinOp::Union, 1),
            Tok::AmpAmp => (BinOp::And, 2),
            Tok::PipeGt => (BinOp::Pipe, 3),
            Tok::EqEq => (BinOp::Eq, 4),
            Tok::NotEq => (BinOp::Ne, 4),
            Tok::Lt => (BinOp::Lt, 4),
            Tok::Gt => (BinOp::Gt, 4),
            Tok::Le => (BinOp::Le, 4),
            Tok::Ge => (BinOp::Ge, 4),
            Tok::PlusPlus => (BinOp::Concat, 5),
            Tok::Shl => (BinOp::Shl, 5),
            Tok::Shr => (BinOp::Shr, 5),
            Tok::Plus => (BinOp::Add, 6),
            Tok::Minus => (BinOp::Sub, 6),
            Tok::Star => (BinOp::Mul, 7),
            Tok::Slash => (BinOp::Div, 7),
            Tok::Percent => (BinOp::Mod, 7),
            _ => return None,
        })
    }

    fn parse_unary(&mut self) -> Expr {
        if self.at(Tok::Minus) {
            self.bump();
            Expr::Unary {
                op: UnOp::Neg,
                expr: Box::new(self.parse_unary()),
            }
        } else if self.at(Tok::Bang) {
            self.bump();
            Expr::Unary {
                op: UnOp::Not,
                expr: Box::new(self.parse_unary()),
            }
        } else if self.at(Tok::Await) {
            // `await e` binds tighter than binary ops: `await a + await b`
            // is `(await a) + (await b)`. No `async` keyword: the async effect
            // is inferred (colorblind async).
            self.bump();
            Expr::Await(Box::new(self.parse_unary()))
        } else if self.at(Tok::Spawn) {
            // `spawn e` runs `e` concurrently and yields a Future.
            self.bump();
            Expr::Spawn(Box::new(self.parse_unary()))
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut e = self.parse_primary();
        loop {
            match self.peek() {
                Tok::Dot => {
                    self.bump();
                    let name = self.ident();
                    e = Expr::Field {
                        base: Box::new(e),
                        name,
                    };
                }
                Tok::LParen => {
                    let args = self.parse_args();
                    e = Expr::Call {
                        callee: Box::new(e),
                        args,
                    };
                }
                Tok::QuestionPost => {
                    self.bump();
                    e = Expr::Try(Box::new(e));
                }
                Tok::LBracket => {
                    // postfix subscript `base[index]` (a `[` in primary position
                    // is a bracketed list literal, handled in parse_primary)
                    self.bump();
                    let index = self.parse_expr();
                    self.expect(Tok::RBracket, "']'");
                    e = Expr::Index {
                        base: Box::new(e),
                        index: Box::new(index),
                    };
                }
                Tok::With => {
                    self.bump();
                    let fields = self.parse_brace_fields();
                    e = Expr::With {
                        base: Box::new(e),
                        fields,
                    };
                }
                _ => break,
            }
        }
        e
    }

    fn parse_primary(&mut self) -> Expr {
        match self.peek().clone() {
            Tok::Int(n) => {
                self.bump();
                Expr::Int(n)
            }
            Tok::Float(f) => {
                self.bump();
                Expr::Float(f)
            }
            Tok::True => {
                self.bump();
                Expr::Bool(true)
            }
            Tok::False => {
                self.bump();
                Expr::Bool(false)
            }
            Tok::StrOpen => self.parse_string(),
            Tok::Path(p) => {
                self.bump();
                Expr::Path(p)
            }
            Tok::Ident(name) => {
                self.bump();
                if !self.no_brace && self.at(Tok::LBrace) {
                    let fields = self.parse_brace_fields();
                    Expr::Ctor { name, fields }
                } else {
                    Expr::Ident(name)
                }
            }
            Tok::LParen => self.parse_group(),
            Tok::LBracket => self.parse_bracket_list(),
            Tok::LBrace => Expr::Record(self.parse_brace_fields()),
            Tok::If => self.parse_if(),
            Tok::Match => self.parse_match(),
            Tok::For => self.parse_for(),
            Tok::While => self.parse_while(),
            Tok::Break => {
                self.bump();
                Expr::Break
            }
            Tok::Continue => {
                self.bump();
                Expr::Continue
            }
            Tok::Fail => {
                self.bump();
                Expr::Fail(Box::new(self.parse_expr()))
            }
            Tok::Try => {
                self.bump();
                Expr::Reify(Box::new(self.parse_expr()))
            }
            other => {
                self.err(format!("unexpected token {other:?}"));
                self.bump();
                Expr::Unit
            }
        }
    }

    fn parse_group(&mut self) -> Expr {
        let save = self.no_brace;
        self.no_brace = false;
        self.expect(Tok::LParen, "'('");
        if self.eat(Tok::RParen) {
            self.no_brace = save;
            return Expr::Unit;
        }
        self.skip_newlines();
        let e = self.parse_list_expr();
        self.skip_newlines();
        self.expect(Tok::RParen, "')'");
        self.no_brace = save;
        e
    }

    fn parse_bracket_list(&mut self) -> Expr {
        let save = self.no_brace;
        self.no_brace = false;
        self.expect(Tok::LBracket, "'['");
        let mut es = Vec::new();
        self.skip_seps();
        while !self.at(Tok::RBracket) && !self.at_eof() {
            es.push(self.parse_expr());
            self.skip_seps();
        }
        self.expect(Tok::RBracket, "']'");
        self.no_brace = save;
        Expr::List(es)
    }

    /// Comma-separated list at expression level (bracketless): `a, b, c`.
    fn parse_list_expr(&mut self) -> Expr {
        let first = self.parse_expr();
        if !self.at(Tok::Comma) {
            return first;
        }
        let mut es = vec![first];
        while self.eat(Tok::Comma) {
            self.skip_newlines();
            if matches!(
                self.peek(),
                Tok::RParen | Tok::RBracket | Tok::RBrace | Tok::Eof
            ) {
                break; // trailing comma
            }
            es.push(self.parse_expr());
        }
        Expr::List(es)
    }

    fn parse_brace_fields(&mut self) -> Vec<Field> {
        let save = self.no_brace;
        self.no_brace = false;
        self.expect(Tok::LBrace, "'{'");
        let mut fs = Vec::new();
        self.skip_seps();
        while !self.at(Tok::RBrace) && !self.at_eof() {
            fs.push(self.parse_field());
            self.skip_seps();
        }
        self.expect(Tok::RBrace, "'}'");
        self.no_brace = save;
        fs
    }

    fn parse_field(&mut self) -> Field {
        if let Tok::Ident(name) = self.peek().clone() {
            match self.peekn(1) {
                Tok::Eq => {
                    self.bump();
                    self.bump();
                    Field::Value {
                        name,
                        value: self.parse_expr(),
                    }
                }
                Tok::Colon => {
                    self.bump();
                    self.bump();
                    Field::Type {
                        name,
                        ty: self.parse_type(),
                    }
                }
                Tok::Comma | Tok::Newline | Tok::RBrace => {
                    self.bump();
                    Field::Shorthand(name)
                }
                _ => Field::Bare(self.parse_expr()),
            }
        } else {
            Field::Bare(self.parse_expr())
        }
    }

    fn parse_args(&mut self) -> Vec<Arg> {
        let save = self.no_brace;
        self.no_brace = false;
        self.expect(Tok::LParen, "'('");
        let mut args = Vec::new();
        self.skip_seps();
        while !self.at(Tok::RParen) && !self.at_eof() {
            args.push(self.parse_arg());
            self.skip_seps();
        }
        self.expect(Tok::RParen, "')'");
        self.no_brace = save;
        args
    }

    fn parse_arg(&mut self) -> Arg {
        if let Tok::Ident(n) = self.peek().clone() {
            let is_dir = (n == "bind" || n == "on")
                && matches!(self.peekn(1), Tok::Colon)
                && matches!(self.peekn(2), Tok::Ident(_))
                && matches!(self.peekn(3), Tok::Eq);
            if is_dir {
                self.bump(); // name
                self.bump(); // :
                let prop = self.ident();
                self.bump(); // =
                let kind = if n == "bind" { Dir::Bind } else { Dir::On };
                return Arg::Directive {
                    kind,
                    prop,
                    value: self.parse_expr(),
                };
            }
            if matches!(self.peekn(1), Tok::Eq) {
                self.bump(); // name
                self.bump(); // =
                return Arg::Named {
                    name: n,
                    value: self.parse_expr(),
                };
            }
        }
        Arg::Pos(self.parse_expr())
    }

    fn parse_string(&mut self) -> Expr {
        self.expect(Tok::StrOpen, "'\"'");
        let mut parts = Vec::new();
        loop {
            match self.peek().clone() {
                Tok::StrText(s) => {
                    self.bump();
                    parts.push(StrPart::Text(s));
                }
                Tok::InterpStart => {
                    self.bump();
                    let mut e = self.parse_expr();
                    if let Tok::FmtSpec(spec) = self.peek().clone() {
                        self.bump();
                        e = self.apply_fmt_spec(e, &spec);
                    }
                    self.expect(Tok::InterpEnd, "'}'");
                    parts.push(StrPart::Interp(e));
                }
                Tok::StrClose => {
                    self.bump();
                    break;
                }
                other => {
                    self.err(format!("unterminated string near {other:?}"));
                    break;
                }
            }
        }
        Expr::Str(parts)
    }

    /// Desugar `"{x:>8}"`'s spec into ordinary calls. A format spec is not a
    /// new evaluation rule. It is spelling for things you could already write
    /// by hand, so it lowers here and every back end gets it for free:
    ///
    /// ```text
    ///   {x:.2}   →  x.fixed(2)
    ///   {x:>8}   →  str(x).pad_start(8, " ")
    ///   {x:<8}   →  str(x).pad_end(8, " ")
    ///   {x:^8}   →  str(x).pad_center(8, " ")
    ///   {x:08}   →  str(x).pad_start(8, "0")
    /// ```
    ///
    /// Grammar: `[<|>|^] [0] width [. precision]`, every part optional.
    fn apply_fmt_spec(&mut self, e: Expr, spec: &str) -> Expr {
        let mut rest = spec;
        let align = match rest.chars().next() {
            Some(c @ ('<' | '>' | '^')) => {
                rest = &rest[1..];
                Some(c)
            }
            _ => None,
        };
        // a leading zero means zero-fill, and implies right alignment
        let zero = rest.starts_with('0') && rest.len() > 1;
        let (width, precision) = match rest.split_once('.') {
            Some((w, p)) => (w, Some(p)),
            None => (rest, None),
        };

        // precision first: it produces the text that padding then aligns
        let mut out = match precision {
            Some(p) => match p.parse::<i64>() {
                Ok(n) => call_method(e, "fixed", vec![Expr::Int(n)]),
                Err(_) => {
                    self.err(format!("format spec `{spec}`: `.{p}` is not a number"));
                    e
                }
            },
            None => Expr::Call {
                callee: Box::new(Expr::Ident("str".into())),
                args: vec![Arg::Pos(e)],
            },
        };

        if !width.is_empty() {
            match width.parse::<i64>() {
                Ok(w) => {
                    let pad = if zero { "0" } else { " " };
                    let how = match align {
                        Some('<') => "pad_end",
                        Some('^') => "pad_center",
                        _ => "pad_start",
                    };
                    out = call_method(
                        out,
                        how,
                        vec![Expr::Int(w), Expr::Str(vec![StrPart::Text(pad.into())])],
                    );
                }
                Err(_) => self.err(format!("format spec `{spec}`: `{width}` is not a width")),
            }
        }
        out
    }

    fn parse_string_literal(&mut self) -> String {
        self.expect(Tok::StrOpen, "'\"'");
        let mut s = String::new();
        loop {
            match self.peek().clone() {
                Tok::StrText(t) => {
                    self.bump();
                    s.push_str(&t);
                }
                Tok::StrClose => {
                    self.bump();
                    break;
                }
                Tok::InterpStart => {
                    self.err("interpolation not allowed here");
                    self.bump();
                    let _ = self.parse_expr();
                    self.eat(Tok::InterpEnd);
                }
                _ => {
                    self.err("unterminated string");
                    break;
                }
            }
        }
        s
    }

    fn parse_if(&mut self) -> Expr {
        self.bump(); // if
        let save = self.no_brace;
        self.no_brace = true;
        let cond = self.parse_expr();
        self.no_brace = save;
        let then = self.parse_block();
        let els = if self.eat(Tok::Else) {
            if self.at(Tok::If) {
                Some(vec![Stmt::Expr(self.parse_if())])
            } else {
                Some(self.parse_block())
            }
        } else {
            None
        };
        Expr::If {
            cond: Box::new(cond),
            then,
            els,
        }
    }

    fn parse_for(&mut self) -> Expr {
        self.bump(); // for
        let save = self.no_brace;
        self.no_brace = true;
        let pat = self.parse_pattern_atom();
        self.expect(Tok::In, "'in'");
        let iter = self.parse_expr();
        self.no_brace = save;
        let body = self.parse_block();
        Expr::For {
            pat,
            iter: Box::new(iter),
            body,
        }
    }

    fn parse_while(&mut self) -> Expr {
        self.bump(); // while
        let save = self.no_brace;
        self.no_brace = true;
        let cond = self.parse_expr();
        self.no_brace = save;
        let body = self.parse_block();
        Expr::While {
            cond: Box::new(cond),
            body,
        }
    }

    fn parse_match(&mut self) -> Expr {
        self.bump(); // match
        let save = self.no_brace;
        self.no_brace = true;
        let scrut = self.parse_expr();
        self.no_brace = save;
        self.expect(Tok::LBrace, "'{'");
        let mut arms = Vec::new();
        self.skip_newlines();
        while !self.at(Tok::RBrace) && !self.at_eof() {
            let before = self.i;
            arms.push(self.parse_arm());
            if self.i == before {
                self.bump();
            }
            self.skip_newlines();
        }
        self.expect(Tok::RBrace, "'}'");
        Expr::Match {
            scrut: Box::new(scrut),
            arms,
        }
    }

    fn parse_arm(&mut self) -> Arm {
        let pat = self.parse_pattern_top();
        // A guard is a boolean expression; parse it at ternary level so the
        // arm's own `=>` isn't mistaken for a lambda arrow (`_ if c => body`).
        let guard = if self.eat(Tok::If) {
            Some(self.parse_ternary())
        } else {
            None
        };
        self.expect(Tok::FatArrow, "'=>'");
        let body = if self.at(Tok::LBrace) {
            Expr::Block(self.parse_block())
        } else {
            self.parse_expr()
        };
        Arm { pat, guard, body }
    }

    fn parse_block(&mut self) -> Vec<Stmt> {
        self.expect(Tok::LBrace, "'{'");
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !self.at(Tok::RBrace) && !self.at_eof() {
            let before = self.i;
            stmts.push(self.parse_stmt());
            if self.i == before {
                self.bump();
            }
            self.skip_newlines();
        }
        self.expect(Tok::RBrace, "'}'");
        stmts
    }

    // ---- patterns ---------------------------------------------------------

    fn parse_pattern_top(&mut self) -> Pattern {
        let first = self.parse_or_pattern();
        if !self.at(Tok::Comma) {
            return first;
        }
        let mut elems = vec![first];
        let mut rest = None;
        while self.eat(Tok::Comma) {
            self.skip_newlines();
            if self.eat(Tok::DotDot) {
                rest = Some(Box::new(self.parse_or_pattern()));
                break;
            }
            if self.at(Tok::FatArrow) || self.at(Tok::If) {
                break;
            }
            elems.push(self.parse_or_pattern());
        }
        Pattern::List { elems, rest }
    }

    fn parse_or_pattern(&mut self) -> Pattern {
        let first = self.parse_pattern_atom();
        if !self.at(Tok::Bar) {
            return first;
        }
        let mut alts = vec![first];
        while self.eat(Tok::Bar) {
            alts.push(self.parse_pattern_atom());
        }
        Pattern::Or(alts)
    }

    fn parse_pattern_atom(&mut self) -> Pattern {
        match self.peek().clone() {
            Tok::Int(n) => {
                self.bump();
                Pattern::Int(n)
            }
            Tok::Float(f) => {
                self.bump();
                Pattern::Float(f)
            }
            Tok::True => {
                self.bump();
                Pattern::Bool(true)
            }
            Tok::False => {
                self.bump();
                Pattern::Bool(false)
            }
            Tok::StrOpen => Pattern::Str(self.parse_string_literal()),
            // A bracketed list pattern: `[]`, `[x]`, `[x, y]`, `[x, ..rest]`.
            // The bracketless spelling (`x, ..rest`) is handled by
            // `parse_pattern_top`; brackets additionally make the empty and
            // single-element cases expressible.
            Tok::LBracket => {
                self.bump();
                let mut elems = Vec::new();
                let mut rest = None;
                self.skip_newlines();
                while !self.at(Tok::RBracket) && !self.at_eof() {
                    if self.eat(Tok::DotDot) {
                        rest = Some(Box::new(self.parse_or_pattern()));
                        self.skip_newlines();
                        break;
                    }
                    elems.push(self.parse_or_pattern());
                    self.skip_newlines();
                    if !self.eat(Tok::Comma) {
                        break;
                    }
                    self.skip_newlines();
                }
                self.expect(Tok::RBracket, "']'");
                Pattern::List { elems, rest }
            }
            Tok::Ident(name) => {
                self.bump();
                if name == "_" {
                    Pattern::Wild
                } else if self.at(Tok::LParen) {
                    self.bump();
                    let mut args = Vec::new();
                    self.skip_seps();
                    while !self.at(Tok::RParen) && !self.at_eof() {
                        args.push(self.parse_or_pattern());
                        self.skip_seps();
                    }
                    self.expect(Tok::RParen, "')'");
                    Pattern::Ctor { name, args }
                } else {
                    Pattern::Bind(name)
                }
            }
            Tok::LBrace => {
                self.bump();
                let mut fs = Vec::new();
                self.skip_seps();
                while !self.at(Tok::RBrace) && !self.at_eof() {
                    let fname = self.ident();
                    let sub = if self.eat(Tok::Colon) {
                        Some(self.parse_or_pattern())
                    } else {
                        None
                    };
                    fs.push((fname, sub));
                    self.skip_seps();
                }
                self.expect(Tok::RBrace, "'}'");
                Pattern::Record(fs)
            }
            Tok::LParen => {
                self.bump();
                let p = self.parse_pattern_top();
                self.expect(Tok::RParen, "')'");
                p
            }
            other => {
                self.err(format!("unexpected pattern {other:?}"));
                self.bump();
                Pattern::Wild
            }
        }
    }

    fn expr_to_params(&mut self, e: Expr) -> Vec<Param> {
        match e {
            Expr::Ident(name) => vec![Param {
                name,
                ty: None,
                variadic: false,
            }],
            Expr::Unit => Vec::new(),
            Expr::List(es) => es
                .into_iter()
                .map(|x| match x {
                    Expr::Ident(name) => Param {
                        name,
                        ty: None,
                        variadic: false,
                    },
                    _ => {
                        self.err("lambda parameters must be identifiers");
                        Param {
                            name: "?".into(),
                            ty: None,
                            variadic: false,
                        }
                    }
                })
                .collect(),
            _ => {
                self.err("invalid lambda parameter list");
                Vec::new()
            }
        }
    }
}

/// `lhs |> rhs`: the piped value becomes `rhs`'s first argument.
///
/// `x |> f` is `f(x)`, and `x |> f(a, b)` is `f(x, a, b)`: what is written to
/// the right of the arrow reads as the call it will become, with the subject
/// left out because the arrow supplies it. A pipeline is then the order the
/// steps happen in rather than the reverse, which is the whole reason to write
/// one.
///
/// Desugaring here rather than lowering per backend is what keeps the answer
/// the same everywhere. It was a `BinOp` for a while, and every backend
/// evaluated it to the left operand and discarded the right, so `3 |> double`
/// was `3`, with no diagnostic anywhere.
fn piped(lhs: Expr, rhs: Expr) -> Expr {
    match rhs {
        Expr::Call { callee, args } => {
            let mut all = vec![Arg::Pos(lhs)];
            all.extend(args);
            Expr::Call { callee, args: all }
        }
        callee => Expr::Call {
            callee: Box::new(callee),
            args: vec![Arg::Pos(lhs)],
        },
    }
}

/// `recv.name(args…)`: a UFCS method call, which is a call whose callee is a
/// field access.
fn call_method(recv: Expr, name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Field {
            base: Box::new(recv),
            name: name.into(),
        }),
        args: args.into_iter().map(Arg::Pos).collect(),
    }
}
