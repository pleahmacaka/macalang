//! Hand-written recursive-descent + Pratt parser over the lexer's token stream.
//!
//! Layout is already resolved by the lexer (newlines are real tokens, suppressed
//! where they continue), so the parser treats `Newline` as a statement/field
//! separator and otherwise ignores it. `no_brace` mode is set while parsing a
//! control-flow header (`if`/`for`/`match` scrutinee) so a following `{` is read
//! as the block, not a `Name { .. }` constructor.

use crate::ast::*;
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
        Parser { toks, i: 0, errors: Vec::new(), no_brace: false }
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
        self.errors.push(ParseError { msg: msg.into(), span: self.span() });
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
            Tok::Let => {
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

    fn finish_bind(&mut self, is_let: bool, target: Expr) -> Bind {
        let mut tys = Vec::new();
        while self.eat(Tok::Colon) {
            tys.push(self.parse_type());
        }
        self.expect(Tok::Eq, "'=' in binding");
        let value = self.parse_list_expr();
        Bind { is_let, target, tys, value }
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
                self.expect(Tok::From, "'from'");
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
        let ret = if self.eat(Tok::Arrow) { Some(self.parse_type()) } else { None };
        let effects = if self.at(Tok::Slash) && matches!(self.peekn(1), Tok::Lt) {
            self.bump(); // /
            Some(self.parse_effect_row())
        } else {
            None
        };
        let body = if self.at(Tok::LBrace) {
            Some(FnBody::Block(self.parse_block()))
        } else if self.eat(Tok::FatArrow) {
            // a bracketless comma list is a valid arrow body (`make() -> int[] =>
            // 1, 2, 3`); a single expression parses unchanged.
            Some(FnBody::Expr(Box::new(self.parse_list_expr())))
        } else {
            None
        };
        FnDef { name, params, ret, effects, body }
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut ps = Vec::new();
        self.skip_seps();
        while !self.at(Tok::RParen) && !self.at_eof() {
            let before = self.i;
            let variadic = self.eat(Tok::Ellipsis);
            let name = self.ident();
            let ty = if self.eat(Tok::Colon) { Some(self.parse_type()) } else { None };
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
            Tok::LParen => {
                self.bump();
                let t = self.parse_type();
                self.expect(Tok::RParen, "')'");
                Type::Paren(Box::new(t))
            }
            other => {
                self.err(format!("expected type, found {other:?}"));
                Type::Name(vec!["?".into()])
            }
        }
    }

    // ---- expressions ------------------------------------------------------

    fn parse_expr(&mut self) -> Expr {
        let lhs = self.parse_ternary();
        if self.eat(Tok::FatArrow) {
            let params = self.expr_to_params(lhs);
            Expr::Lambda { params, body: Box::new(self.parse_lambda_body()) }
        } else {
            lhs
        }
    }

    /// Lambda bodies additionally allow the UI setter form `x = e` (assign expr)
    /// and nested lambdas.
    fn parse_lambda_body(&mut self) -> Expr {
        let lhs = self.parse_ternary();
        if self.eat(Tok::FatArrow) {
            let params = self.expr_to_params(lhs);
            Expr::Lambda { params, body: Box::new(self.parse_lambda_body()) }
        } else if self.at(Tok::Eq) {
            self.bump();
            Expr::Assign { target: Box::new(lhs), value: Box::new(self.parse_lambda_body()) }
        } else {
            lhs
        }
    }

    fn parse_ternary(&mut self) -> Expr {
        let cond = self.parse_binary(0);
        if self.eat(Tok::Question) {
            let then = self.parse_ternary();
            self.expect(Tok::Colon, "':' in ternary");
            let els = self.parse_ternary();
            Expr::Ternary { cond: Box::new(cond), then: Box::new(then), els: Box::new(els) }
        } else {
            cond
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
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
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
            Expr::Unary { op: UnOp::Neg, expr: Box::new(self.parse_unary()) }
        } else if self.at(Tok::Bang) {
            self.bump();
            Expr::Unary { op: UnOp::Not, expr: Box::new(self.parse_unary()) }
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
                    e = Expr::Field { base: Box::new(e), name };
                }
                Tok::LParen => {
                    let args = self.parse_args();
                    e = Expr::Call { callee: Box::new(e), args };
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
                    e = Expr::Index { base: Box::new(e), index: Box::new(index) };
                }
                Tok::With => {
                    self.bump();
                    let fields = self.parse_brace_fields();
                    e = Expr::With { base: Box::new(e), fields };
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
            if matches!(self.peek(), Tok::RParen | Tok::RBracket | Tok::RBrace | Tok::Eof) {
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
                    Field::Value { name, value: self.parse_expr() }
                }
                Tok::Colon => {
                    self.bump();
                    self.bump();
                    Field::Type { name, ty: self.parse_type() }
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
                return Arg::Directive { kind, prop, value: self.parse_expr() };
            }
            if matches!(self.peekn(1), Tok::Eq) {
                self.bump(); // name
                self.bump(); // =
                return Arg::Named { name: n, value: self.parse_expr() };
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
                    let e = self.parse_expr();
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
        Expr::If { cond: Box::new(cond), then, els }
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
        Expr::For { pat, iter: Box::new(iter), body }
    }

    fn parse_while(&mut self) -> Expr {
        self.bump(); // while
        let save = self.no_brace;
        self.no_brace = true;
        let cond = self.parse_expr();
        self.no_brace = save;
        let body = self.parse_block();
        Expr::While { cond: Box::new(cond), body }
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
        Expr::Match { scrut: Box::new(scrut), arms }
    }

    fn parse_arm(&mut self) -> Arm {
        let pat = self.parse_pattern_top();
        // A guard is a boolean expression; parse it at ternary level so the
        // arm's own `=>` isn't mistaken for a lambda arrow (`_ if c => body`).
        let guard = if self.eat(Tok::If) { Some(self.parse_ternary()) } else { None };
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
            Expr::Ident(name) => vec![Param { name, ty: None, variadic: false }],
            Expr::Unit => Vec::new(),
            Expr::List(es) => es
                .into_iter()
                .map(|x| match x {
                    Expr::Ident(name) => Param { name, ty: None, variadic: false },
                    _ => {
                        self.err("lambda parameters must be identifiers");
                        Param { name: "?".into(), ty: None, variadic: false }
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
