use std::fmt::Write as _;

/// Byte range `[start, end)` into the source.
pub type Span = (usize, usize);

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Int(i64),
    Float(f64),
    Ident(String),
    True,
    False,
    StrOpen,
    StrText(String),
    InterpStart,
    /// A format spec closing an interpolation: the `>8` of `"{name:>8}"`.
    FmtSpec(String),
    InterpEnd,
    StrClose,
    Path(String),
    Const,
    As,
    If,
    Else,
    For,
    In,
    While,
    Break,
    Continue,
    Return,
    Match,
    Import,
    With,
    Fail,
    Try,
    Alias,
    Await,
    Spawn,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
    DotDot,
    Ellipsis,
    Eq,
    EqEq,
    NotEq,
    Bang,
    Lt,
    Gt,
    Le,
    Ge,
    Arrow,
    FatArrow,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Shl,
    Shr,
    PlusPlus,
    Bar,
    BarBar,
    PipeGt,
    AmpAmp,
    Question,
    QuestionPost,
    Newline,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LexError {
    pub msg: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}

/// Tokenize `src`.
pub fn lex(src: &str) -> Lexed {
    Lexer::new(src).run()
}

/// Render a token stream as one token per line: the golden-snapshot format.
pub fn dump_tokens(toks: &[Token]) -> String {
    let mut out = String::new();
    for t in toks {
        let _ = writeln!(out, "{}", dump_tok(&t.tok));
    }
    out
}

fn dump_tok(t: &Tok) -> String {
    match t {
        Tok::Int(n) => format!("Int {n}"),
        Tok::Float(f) => format!("Float {f}"),
        Tok::Ident(s) => format!("Ident {s:?}"),
        Tok::StrText(s) => format!("StrText {s:?}"),
        Tok::Path(s) => format!("Path {s:?}"),
        other => format!("{other:?}"),
    }
}

#[derive(PartialEq)]
enum Mode {
    Code,
    Str,
}

enum Brace {
    Code,
    Interp,
}

struct Lexer<'a> {
    #[allow(dead_code)]
    src: &'a str,
    chars: Vec<(usize, char)>,
    i: usize,
    end: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
    group_depth: i32,
    braces: Vec<Brace>,
    mode: Mode,
    leading_ws: bool,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer {
            src,
            chars: src.char_indices().collect(),
            i: 0,
            end: src.len(),
            tokens: Vec::new(),
            errors: Vec::new(),
            group_depth: 0,
            braces: Vec::new(),
            mode: Mode::Code,
            leading_ws: true,
        }
    }

    fn peek(&self) -> char {
        self.chars.get(self.i).map(|c| c.1).unwrap_or('\0')
    }
    fn peek_n(&self, n: usize) -> char {
        self.chars.get(self.i + n).map(|c| c.1).unwrap_or('\0')
    }
    fn byte(&self) -> usize {
        self.chars.get(self.i).map(|c| c.0).unwrap_or(self.end)
    }
    fn at_end(&self) -> bool {
        self.i >= self.chars.len()
    }
    fn bump(&mut self) -> char {
        let c = self.peek();
        self.i += 1;
        c
    }
    fn push(&mut self, tok: Tok, span: Span) {
        self.tokens.push(Token { tok, span });
    }
    fn error(&mut self, msg: impl Into<String>, span: Span) {
        self.errors.push(LexError {
            msg: msg.into(),
            span,
        });
    }

    fn last_sig(&self) -> Option<&Tok> {
        self.tokens
            .iter()
            .rev()
            .find(|t| t.tok != Tok::Newline)
            .map(|t| &t.tok)
    }

    fn run(mut self) -> Lexed {
        loop {
            match self.mode {
                Mode::Str => self.lex_str_chunk(),
                Mode::Code => {
                    let saw_nl = self.skip_trivia();
                    if self.at_end() {
                        break;
                    }
                    if saw_nl
                        && !self.suppressed()
                        && !self.last_is_trailing_cont()
                        && !self.next_is_leading_cont()
                    {
                        let b = self.byte();
                        self.push(Tok::Newline, (b, b));
                    }
                    self.lex_code();
                }
            }
        }
        self.push(Tok::Eof, (self.end, self.end));
        Lexed {
            tokens: self.tokens,
            errors: self.errors,
        }
    }

    fn suppressed(&self) -> bool {
        self.group_depth > 0 || matches!(self.braces.last(), Some(Brace::Interp))
    }

    /// Skip spaces, comments, and newlines.
    #[allow(clippy::nonminimal_bool)]
    fn skip_trivia(&mut self) -> bool {
        self.leading_ws = false;
        let mut saw_nl = false;
        loop {
            let c = self.peek();
            match c {
                ' ' | '\t' | '\r' => {
                    self.bump();
                    self.leading_ws = true;
                }
                '\n' => {
                    self.bump();
                    saw_nl = true;
                    self.leading_ws = true;
                }
                '/' if self.peek_n(1) == '/' => {
                    while !self.at_end() && self.peek() != '\n' {
                        self.bump();
                    }
                    self.leading_ws = true;
                }
                '/' if self.peek_n(1) == '*' => {
                    self.bump();
                    self.bump();
                    while !self.at_end() && !(self.peek() == '*' && self.peek_n(1) == '/') {
                        self.bump();
                    }
                    self.bump();
                    self.bump();
                    self.leading_ws = true;
                }
                _ => break,
            }
        }
        saw_nl
    }

    fn last_is_trailing_cont(&self) -> bool {
        use Tok::*;
        match self.last_sig() {
            None => true,
            Some(t) => matches!(
                t,
                Comma
                    | Eq
                    | EqEq
                    | NotEq
                    | Lt
                    | Gt
                    | Le
                    | Ge
                    | Arrow
                    | FatArrow
                    | Plus
                    | Minus
                    | Star
                    | Slash
                    | PlusPlus
                    | Bar
                    | BarBar
                    | PipeGt
                    | AmpAmp
                    | Colon
                    | Dot
                    | DotDot
                    | Ellipsis
                    | Question
                    | LParen
                    | LBracket
                    | LBrace
                    | Const
                    | If
                    | Else
                    | For
                    | In
                    | Match
                    | Import
                    | With
                    | Fail
                    | Try
                    | Alias
            ),
        }
    }

    fn next_is_leading_cont(&self) -> bool {
        match self.peek() {
            '?' | ':' => true,
            ')' | ']' | '}' => true,
            '.' => !self.peek_n(1).is_ascii_digit(),
            '|' => true,
            '&' => self.peek_n(1) == '&',
            '+' => self.peek_n(1) == '+',
            c if is_ident_start(c) => {
                let w = self.peek_word();
                w == "else" || w == "with"
            }
            _ => false,
        }
    }

    fn peek_word(&self) -> String {
        let mut s = String::new();
        let mut j = self.i;
        while let Some(&(_, c)) = self.chars.get(j) {
            if is_ident_continue(c) {
                s.push(c);
                j += 1;
            } else {
                break;
            }
        }
        s
    }

    /// True when an operand is expected next (start of expression), which is where a leading `/`, `./`, `~` begins a path literal rather than an op.
    fn expect_operand(&self) -> bool {
        use Tok::*;
        match self.last_sig() {
            None => true,
            Some(t) => !matches!(
                t,
                Ident(_)
                    | Int(_)
                    | Float(_)
                    | True
                    | False
                    | Path(_)
                    | StrClose
                    | RParen
                    | RBracket
                    | RBrace
                    | QuestionPost
            ),
        }
    }

    fn lex_code(&mut self) {
        let start = self.byte();
        let c = self.peek();

        if self.expect_operand() && self.starts_path(c) {
            return self.lex_path();
        }

        match c {
            '(' => self.one(Tok::LParen, |l| l.group_depth += 1),
            ')' => self.one(Tok::RParen, |l| l.group_depth = (l.group_depth - 1).max(0)),
            '[' => self.one(Tok::LBracket, |l| l.group_depth += 1),
            ']' => self.one(Tok::RBracket, |l| {
                l.group_depth = (l.group_depth - 1).max(0)
            }),
            '{' => {
                self.bump();
                self.braces.push(Brace::Code);
                self.push(Tok::LBrace, (start, self.byte()));
            }
            '}' => {
                self.bump();
                match self.braces.pop() {
                    Some(Brace::Interp) => {
                        self.push(Tok::InterpEnd, (start, self.byte()));
                        self.mode = Mode::Str;
                    }
                    _ => self.push(Tok::RBrace, (start, self.byte())),
                }
            }
            ',' => self.one(Tok::Comma, |_| {}),
            ':' if self.fmt_spec_here().is_some() => {
                let spec = self.fmt_spec_here().unwrap();
                for _ in 0..=spec.len() {
                    self.bump();
                }
                self.push(Tok::FmtSpec(spec), (start, self.byte()));
            }
            ':' => self.one(Tok::Colon, |_| {}),
            '.' => {
                if self.peek_n(1) == '.' && self.peek_n(2) == '.' {
                    self.take(3, Tok::Ellipsis, start);
                } else if self.peek_n(1) == '.' {
                    self.take(2, Tok::DotDot, start);
                } else {
                    self.take(1, Tok::Dot, start);
                }
            }
            '+' if self.peek_n(1) == '+' => self.take(2, Tok::PlusPlus, start),
            '+' => self.take(1, Tok::Plus, start),
            '-' if self.peek_n(1) == '>' => self.take(2, Tok::Arrow, start),
            '-' => self.take(1, Tok::Minus, start),
            '*' => self.take(1, Tok::Star, start),
            '/' => self.take(1, Tok::Slash, start),
            '=' if self.peek_n(1) == '>' => self.take(2, Tok::FatArrow, start),
            '=' if self.peek_n(1) == '=' => self.take(2, Tok::EqEq, start),
            '=' => self.take(1, Tok::Eq, start),
            '!' if self.peek_n(1) == '=' => self.take(2, Tok::NotEq, start),
            '!' => self.take(1, Tok::Bang, start),
            '<' if self.peek_n(1) == '<' => self.take(2, Tok::Shl, start),
            '<' if self.peek_n(1) == '=' => self.take(2, Tok::Le, start),
            '<' => self.take(1, Tok::Lt, start),
            '>' if self.peek_n(1) == '>' => self.take(2, Tok::Shr, start),
            '>' if self.peek_n(1) == '=' => self.take(2, Tok::Ge, start),
            '>' => self.take(1, Tok::Gt, start),
            '%' => self.take(1, Tok::Percent, start),
            '|' if self.peek_n(1) == '>' => self.take(2, Tok::PipeGt, start),
            '|' if self.peek_n(1) == '|' => self.take(2, Tok::BarBar, start),
            '|' => self.take(1, Tok::Bar, start),
            '&' if self.peek_n(1) == '&' => self.take(2, Tok::AmpAmp, start),
            '?' => {
                self.bump();
                let tok = if self.leading_ws {
                    Tok::Question
                } else {
                    Tok::QuestionPost
                };
                self.push(tok, (start, self.byte()));
            }
            '"' => {
                if self.peek_n(1) == '"' && self.peek_n(2) == '"' {
                    self.bump();
                    self.bump();
                    self.bump();
                    self.push(Tok::StrOpen, (start, self.byte()));
                    let ts = self.byte();
                    let mut s = String::new();
                    loop {
                        if self.at_end() {
                            self.error("unterminated raw string", (start, self.byte()));
                            break;
                        }
                        if self.peek() == '"' && self.peek_n(1) == '"' && self.peek_n(2) == '"' {
                            break;
                        }
                        s.push(self.bump());
                    }
                    if !s.is_empty() {
                        self.push(Tok::StrText(s), (ts, self.byte()));
                    }
                    let q = self.byte();
                    self.bump();
                    self.bump();
                    self.bump();
                    self.push(Tok::StrClose, (q, self.byte()));
                } else {
                    self.bump();
                    self.push(Tok::StrOpen, (start, self.byte()));
                    self.mode = Mode::Str;
                }
            }
            c if is_ident_start(c) => self.lex_ident(start),
            c if c.is_ascii_digit() => self.lex_number(start),
            _ => {
                self.bump();
                self.error(format!("unexpected character {c:?}"), (start, self.byte()));
            }
        }
    }

    fn one(&mut self, tok: Tok, f: impl FnOnce(&mut Self)) {
        let start = self.byte();
        self.bump();
        f(self);
        self.push(tok, (start, self.byte()));
    }

    fn take(&mut self, n: usize, tok: Tok, start: usize) {
        for _ in 0..n {
            self.bump();
        }
        self.push(tok, (start, self.byte()));
    }

    fn lex_ident(&mut self, start: usize) {
        let mut s = String::new();
        while is_ident_continue(self.peek()) {
            s.push(self.bump());
        }
        while self.peek() == '-' && is_ident_start(self.peek_n(1)) {
            s.push(self.bump());
            while is_ident_continue(self.peek()) {
                s.push(self.bump());
            }
        }
        let tok = match s.as_str() {
            "const" => Tok::Const,
            "as" => Tok::As,
            "if" => Tok::If,
            "else" => Tok::Else,
            "for" => Tok::For,
            "in" => Tok::In,
            "while" => Tok::While,
            "break" => Tok::Break,
            "continue" => Tok::Continue,
            "return" => Tok::Return,
            "match" => Tok::Match,
            "import" => Tok::Import,
            "with" => Tok::With,
            "fail" => Tok::Fail,
            "try" => Tok::Try,
            "alias" => Tok::Alias,
            "await" => Tok::Await,
            "spawn" => Tok::Spawn,
            "true" => Tok::True,
            "false" => Tok::False,
            _ => Tok::Ident(s),
        };
        self.push(tok, (start, self.byte()));
    }

    fn lex_number(&mut self, start: usize) {
        if self.peek() == '0' {
            let (radix, valid): (u32, fn(char) -> bool) = match self.peek_n(1) {
                'x' | 'X' => (16, |c| c.is_ascii_hexdigit()),
                'b' | 'B' => (2, |c| c == '0' || c == '1'),
                'o' | 'O' => (8, |c| ('0'..='7').contains(&c)),
                _ => (0, |_| false),
            };
            if radix != 0 && (valid(self.peek_n(2)) || self.peek_n(2) == '_') {
                self.bump();
                self.bump();
                let mut digits = String::new();
                while valid(self.peek()) || self.peek() == '_' {
                    let c = self.bump();
                    if c != '_' {
                        digits.push(c);
                    }
                }
                let span = (start, self.byte());
                match i64::from_str_radix(&digits, radix) {
                    Ok(n) => self.push(Tok::Int(n), span),
                    Err(_) => match u64::from_str_radix(&digits, radix) {
                        Ok(u) => self.push(Tok::Int(u as i64), span),
                        Err(_) => self.error(format!("integer 0x{digits} out of range"), span),
                    },
                }
                return;
            }
        }
        let mut s = String::new();
        while self.peek().is_ascii_digit() || self.peek() == '_' {
            let c = self.bump();
            if c != '_' {
                s.push(c);
            }
        }
        let is_float = self.peek() == '.' && self.peek_n(1).is_ascii_digit();
        if is_float {
            s.push(self.bump());
            while self.peek().is_ascii_digit() {
                s.push(self.bump());
            }
        }
        let span = (start, self.byte());
        if is_float {
            match s.parse::<f64>() {
                Ok(f) => self.push(Tok::Float(f), span),
                Err(_) => self.error(format!("bad float {s:?}"), span),
            }
        } else {
            match s.parse::<i64>() {
                Ok(n) => self.push(Tok::Int(n), span),
                Err(_) => self.error(format!("integer {s:?} out of range"), span),
            }
        }
    }

    fn starts_path(&self, c: char) -> bool {
        match c {
            '~' => true,
            '/' => is_path_char(self.peek_n(1)),
            '.' => self.peek_n(1) == '/' || (self.peek_n(1) == '.' && self.peek_n(2) == '/'),
            _ => false,
        }
    }

    fn lex_path(&mut self) {
        let start = self.byte();
        let mut s = String::new();
        while is_path_char(self.peek()) {
            s.push(self.bump());
        }
        self.push(Tok::Path(s), (start, self.byte()));
    }

    /// The format spec starting at the current `:`, if this colon closes an interpolation rather than separating a ternary's arms.
    fn fmt_spec_here(&self) -> Option<String> {
        if !matches!(self.braces.last(), Some(Brace::Interp)) {
            return None;
        }
        let prev = self.src[..self.byte()].chars().next_back();
        if matches!(prev, None | Some(' ') | Some('\t')) {
            return None;
        }
        let mut spec = String::new();
        let mut i = 1;
        loop {
            match self.peek_n(i) {
                '}' => return (!spec.is_empty()).then_some(spec),
                c @ ('<' | '>' | '^' | '.' | '0'..='9') => spec.push(c),
                _ => return None,
            }
            i += 1;
        }
    }

    fn lex_str_chunk(&mut self) {
        let start = self.byte();
        let mut s = String::new();
        loop {
            let c = self.peek();
            match c {
                '\0' if self.at_end() => {
                    if !s.is_empty() {
                        self.push(Tok::StrText(s), (start, self.byte()));
                    }
                    self.error("unterminated string", (start, self.end));
                    self.push(Tok::StrClose, (self.end, self.end));
                    self.mode = Mode::Code;
                    return;
                }
                '"' => {
                    if !s.is_empty() {
                        self.push(Tok::StrText(s), (start, self.byte()));
                    }
                    let q = self.byte();
                    self.bump();
                    self.push(Tok::StrClose, (q, self.byte()));
                    self.mode = Mode::Code;
                    return;
                }
                '\n' => {
                    if !s.is_empty() {
                        self.push(Tok::StrText(s), (start, self.byte()));
                    }
                    let nl = self.byte();
                    self.error(
                        "string literal spans a line; write `\\n`, or use a raw \
                         \"\"\"…\"\"\" string. (A literal brace is `\\{` or `{{`.)",
                        (start, nl),
                    );
                    self.push(Tok::StrClose, (nl, nl));
                    self.mode = Mode::Code;
                    return;
                }
                '{' if self.peek_n(1) == '{' => {
                    self.bump();
                    self.bump();
                    s.push('{');
                }
                '}' if self.peek_n(1) == '}' => {
                    self.bump();
                    self.bump();
                    s.push('}');
                }
                '{' => {
                    if !s.is_empty() {
                        self.push(Tok::StrText(s), (start, self.byte()));
                    }
                    let b = self.byte();
                    self.bump();
                    self.push(Tok::InterpStart, (b, self.byte()));
                    self.braces.push(Brace::Interp);
                    self.mode = Mode::Code;
                    return;
                }
                '\\' => {
                    self.bump();
                    let e = self.bump();
                    s.push(match e {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '0' => '\0',
                        '\\' => '\\',
                        '"' => '"',
                        '{' => '{',
                        '}' => '}',
                        other => other,
                    });
                }
                _ => {
                    s.push(self.bump());
                }
            }
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}
fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}
fn is_path_char(c: char) -> bool {
    matches!(c, '/' | '.' | '~' | '_' | '-') || c.is_ascii_alphanumeric()
}

/// Is `word` a keyword rather than an identifier?
pub fn is_keyword(word: &str) -> bool {
    matches!(
        lex(word).tokens.first().map(|t| &t.tok),
        Some(
            Tok::Const
                | Tok::As
                | Tok::If
                | Tok::Else
                | Tok::For
                | Tok::In
                | Tok::While
                | Tok::Break
                | Tok::Continue
                | Tok::Return
                | Tok::Match
                | Tok::Import
                | Tok::With
                | Tok::Fail
                | Tok::Try
                | Tok::Alias
                | Tok::Await
                | Tok::Spawn
                | Tok::True
                | Tok::False
        )
    )
}
