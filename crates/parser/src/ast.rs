//! Maca AST. Span-free on purpose: the parse→print→parse roundtrip test checks
//! structural equality, so nodes carry no source positions. Parse errors still
//! report token spans, which is where a reader needs them.

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
        /// `(a, b) -> T => …` — an optional declared return type. A lambda
        /// usually infers, but a trait-impl method has to match a signature the
        /// compiler cannot read, so it can be written out.
        ret: Option<Type>,
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

/// Visit `e` and every expression inside it, outermost first.
///
/// Each backend used to carry its own copy of this walk, each covering the
/// variants that backend happened to need — which is how `chars()` went years
/// without registering its array type. One walk, over every variant, means a
/// new `Expr` variant is a compile error here rather than a silent gap there.
pub fn walk_expr(e: &Expr, f: &mut impl FnMut(&Expr)) {
    f(e);
    match e {
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::Bool(_)
        | Expr::Unit
        | Expr::Path(_)
        | Expr::Ident(_)
        | Expr::Break
        | Expr::Continue => {}
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    walk_expr(x, f);
                }
            }
        }
        Expr::List(xs) => {
            for x in xs {
                walk_expr(x, f);
            }
        }
        Expr::Record(fields) | Expr::Ctor { fields, .. } => walk_fields(fields, f),
        Expr::With { base, fields } => {
            walk_expr(base, f);
            walk_fields(fields, f);
        }
        Expr::Call { callee, args } => {
            walk_expr(callee, f);
            for a in args {
                walk_expr(arg_expr(a), f);
            }
        }
        Expr::Field { base, .. } => walk_expr(base, f),
        Expr::Index { base, index } => {
            walk_expr(base, f);
            walk_expr(index, f);
        }
        Expr::Range { lo, hi } => {
            walk_expr(lo, f);
            walk_expr(hi, f);
        }
        Expr::Unary { expr, .. } => walk_expr(expr, f),
        Expr::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, f);
            walk_expr(rhs, f);
        }
        Expr::Ternary { cond, then, els } => {
            walk_expr(cond, f);
            walk_expr(then, f);
            walk_expr(els, f);
        }
        Expr::If { cond, then, els } => {
            walk_expr(cond, f);
            walk_stmts(then, f);
            if let Some(e) = els {
                walk_stmts(e, f);
            }
        }
        Expr::Match { scrut, arms } => {
            walk_expr(scrut, f);
            for a in arms {
                if let Some(g) = &a.guard {
                    walk_expr(g, f);
                }
                walk_expr(&a.body, f);
            }
        }
        Expr::For { iter, body, .. } => {
            walk_expr(iter, f);
            walk_stmts(body, f);
        }
        Expr::While { cond, body } => {
            walk_expr(cond, f);
            walk_stmts(body, f);
        }
        Expr::Lambda { body, .. } => walk_expr(body, f),
        Expr::Try(x) | Expr::Fail(x) | Expr::Reify(x) | Expr::Await(x) | Expr::Spawn(x) => {
            walk_expr(x, f)
        }
        Expr::Assign { target, value } => {
            walk_expr(target, f);
            walk_expr(value, f);
        }
        Expr::Block(ss) => walk_stmts(ss, f),
    }
}

fn walk_stmts(ss: &[Stmt], f: &mut impl FnMut(&Expr)) {
    for s in ss {
        walk_stmt(s, f);
    }
}

fn walk_fields(fields: &[Field], f: &mut impl FnMut(&Expr)) {
    for field in fields {
        match field {
            Field::Value { value, .. } | Field::Bare(value) => walk_expr(value, f),
            Field::Type { .. } | Field::Shorthand(_) => {}
        }
    }
}

/// Visit every expression in a statement.
pub fn walk_stmt(s: &Stmt, f: &mut impl FnMut(&Expr)) {
    match s {
        Stmt::Expr(e) => walk_expr(e, f),
        Stmt::Bind(b) => {
            walk_expr(&b.target, f);
            walk_expr(&b.value, f);
        }
        Stmt::Alias { value, .. } => walk_expr(value, f),
        Stmt::Fn(fd) => match &fd.body {
            Some(FnBody::Expr(e)) => walk_expr(e, f),
            Some(FnBody::Block(ss)) => walk_stmts(ss, f),
            None => {}
        },
        Stmt::Import(_) => {}
    }
}

/// The expression carried by a call argument, whatever form it took.
pub fn arg_expr(a: &Arg) -> &Expr {
    match a {
        Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => e,
    }
}

/// Is this expression a *record type declaration* rather than a record value?
///
/// `P = { x: int }` declares a type — every field is `name: Type`. `P { x = 1 }`
/// builds one. Three backends each carried a byte-identical copy of this, and
/// they name the same source-level distinction, so they share one.
pub fn is_record_type(e: &Expr) -> bool {
    matches!(e, Expr::Record(fs) if !fs.is_empty()
        && fs.iter().all(|f| matches!(f, Field::Type { .. })))
}

/// The literal text of an interpolated string, with the interpolations dropped.
///
/// What a backend needs when a string has to be known at compile time — a Nix
/// attribute path, a Tailwind class list.
pub fn plain_text(parts: &[StrPart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            StrPart::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// Is `n` a type *variable* rather than a concrete type?
///
/// Lowercase and not one of the primitives, and not a sized numeric like `i32`
/// or a SIMD `f32x4`. The C and JVM backends monomorphize against this and must
/// agree with the checker about which names are generic.
pub fn is_type_var_name(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_lowercase()
        && !matches!(n, "int" | "float" | "str" | "bool" | "bytes" | "unit")
        && !(matches!(b[0], b'i' | b'u' | b'f') && b.get(1).is_some_and(u8::is_ascii_digit))
}
