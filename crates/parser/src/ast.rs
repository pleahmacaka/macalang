//! Maca AST. Span-free on purpose: Phase 2 checks structural equality for the
//! parse→print→parse roundtrip, so nodes carry no source positions (parse
//! errors still report token spans). Spans get added when Phase 3 needs them.

pub type Ident = String;

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub items: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Import(Import),
    Alias { name: Ident, value: Expr },
    Fn(FnDef),
    Bind(Bind),
    Expr(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Import {
    /// `import std/json` → `["std", "json"]`
    Module(Vec<Ident>),
    /// `import { a, b } from m`
    Names {
        names: Vec<Ident>,
        module: Vec<Ident>,
    },
    /// `import ./x.maca`
    Path(String),
    /// `import nixpkgs`
    Bare(Ident),
    /// `import c "sqlite3.h"` / `import nix "./x.nix"` / `import py "numpy"`
    Foreign { lang: Ident, spec: String },
}

/// `[const] target [: T [: Base ...]] = value [as const]`
///
/// A bare lowercase `x = e` binds a *mutable* variable; `const x = e`,
/// `x = e as const`, or a Capitalized name binds a *constant* (`is_const`).
#[derive(Clone, Debug, PartialEq)]
pub struct Bind {
    pub is_const: bool,
    pub target: Expr, // Ident or dotted Field path (config: networking.hostName)
    pub tys: Vec<Type>,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FnDef {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub effects: Option<Vec<Ident>>, // `/ <io, net>`
    pub body: Option<FnBody>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FnBody {
    Block(Vec<Stmt>),
    Expr(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: Option<Type>,
    pub variadic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Name(Vec<Ident>),            // int / Status / nixpkgs.zed
    Apply(Box<Type>, Vec<Type>), // Map k v
    Array(Box<Type>),            // T[]
    Opt(Box<Type>),              // T?
    Paren(Box<Type>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Unit,
    Str(Vec<StrPart>),
    Path(String),
    Ident(Ident),
    List(Vec<Expr>),
    Record(Vec<Field>),
    Ctor {
        name: Ident,
        fields: Vec<Field>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    Field {
        base: Box<Expr>,
        name: Ident,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    }, // `xs[i]`
    Range {
        lo: Box<Expr>,
        hi: Box<Expr>,
    }, // `lo..hi` (inclusive: lo … hi)
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Ternary {
        cond: Box<Expr>,
        then: Box<Expr>,
        els: Box<Expr>,
    },
    If {
        cond: Box<Expr>,
        then: Vec<Stmt>,
        els: Option<Vec<Stmt>>,
    },
    Match {
        scrut: Box<Expr>,
        arms: Vec<Arm>,
    },
    For {
        pat: Pattern,
        iter: Box<Expr>,
        body: Vec<Stmt>,
    },
    While {
        cond: Box<Expr>,
        body: Vec<Stmt>,
    },
    Break,
    Continue,
    Lambda {
        params: Vec<Param>,
        body: Box<Expr>,
    },
    With {
        base: Box<Expr>,
        fields: Vec<Field>,
    },
    Try(Box<Expr>),   // postfix `x?`
    Fail(Box<Expr>),  // `fail e`
    Reify(Box<Expr>), // `try e`
    Await(Box<Expr>), // `await e` — suspend until the future resolves
    Spawn(Box<Expr>), // `spawn e` — run `e` concurrently, yields a Future
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    }, // UI setter `age = int(v)`
    Block(Vec<Stmt>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Text(String),
    Interp(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    Value { name: Ident, value: Expr },
    Type { name: Ident, ty: Type },
    Shorthand(Ident),
    Bare(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Pos(Expr),
    Named { name: Ident, value: Expr },
    Directive { kind: Dir, prop: Ident, value: Expr },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dir {
    Bind,
    On,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arm {
    pub pat: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    Wild,
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Bind(Ident),
    Ctor {
        name: Ident,
        args: Vec<Pattern>,
    },
    Record(Vec<(Ident, Option<Pattern>)>),
    List {
        elems: Vec<Pattern>,
        rest: Option<Box<Pattern>>,
    },
    Or(Vec<Pattern>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,    // %
    Shl,    // <<
    Shr,    // >>
    Concat, // ++
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,   // &&
    Or,    // ||
    Union, // |  (sum types / rows)
    Pipe,  // |>
}
