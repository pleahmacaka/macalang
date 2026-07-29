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
    /// `(T, U) -> R` — the type of a function value.
    ///
    /// A function passed as an argument needs no type: an unannotated parameter
    /// that is *called* in the body is one. A function kept in a record field
    /// does need one, because a field is declared before anything calls it —
    /// which is why a route table, a middleware chain and a list of subscribers
    /// were all impossible to write down.
    ///
    /// The parentheses are required. `str -> str` would have to be told apart
    /// from a return type by lookahead, and a type that means something
    /// different depending on what follows it is not worth the two characters.
    Fn(Vec<Type>, Box<Type>),
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
            // A shorthand names a variable but holds no `Expr` to hand over —
            // see `walk_names`, which is the walk to use when the question is
            // which names an expression mentions.
            Field::Type { .. } | Field::Shorthand(_) => {}
        }
    }
}

/// Every variable name `e` mentions.
///
/// `walk_expr` visits expressions, and `{ s }` mentions `s` without containing
/// an expression for it — the shorthand carries a bare `Ident`. Anything asking
/// *which names does this use* has to see through that, or `{ s }` and
/// `{ s = s }` answer differently: the first told the C back end that nothing
/// was holding `s`, so the block that built it released it while the record it
/// had just been put into still pointed at the bytes.
pub fn walk_names(e: &Expr, f: &mut impl FnMut(&str)) {
    walk_expr(e, &mut |c| match c {
        Expr::Ident(n) => f(n),
        Expr::Record(fields) | Expr::Ctor { fields, .. } | Expr::With { fields, .. } => {
            for field in fields {
                if let Field::Shorthand(n) = field {
                    f(n);
                }
            }
        }
        _ => {}
    });
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

/// Rename every free reference to `from` into `to`, throughout `s`.
///
/// Used when two modules inlined into one program each define a private helper
/// under the same name. Everything lands in one translation unit, so one of the
/// two has to be renamed, and it has to be renamed in its own module's
/// references as well as at its definition.
///
/// Shadowing is not tracked: a local called `helper` inside a module whose
/// `helper` is being renamed is renamed too. That is a rename of something to
/// an unused name — the qualified form is fresh by construction — so it is
/// harmless, and the alternative is scope analysis this pass has no need of.
pub fn rename_ident(s: &mut Stmt, from: &str, to: &str) {
    match s {
        Stmt::Expr(e) => rename_in_expr(e, from, to),
        Stmt::Bind(b) => {
            rename_in_expr(&mut b.target, from, to);
            rename_in_expr(&mut b.value, from, to);
            b.tys.iter_mut().for_each(|t| rename_in_type(t, from, to));
        }
        Stmt::Alias { name, value } => {
            if name == from {
                *name = to.to_string();
            }
            rename_in_expr(value, from, to);
        }
        Stmt::Fn(fd) => {
            if fd.name == from {
                fd.name = to.to_string();
            }
            // A function that binds this name means something else by it, so
            // its body is left alone: a package with a private `at` used to
            // rewrite `at + 1` inside any function with a parameter called
            // `at`, leaving the parameter itself untouched and the arithmetic
            // pointing at a function. Skipping the whole body is coarse — a use
            // *before* the shadow is bound loses its rename too — and it is
            // coarse in the direction where the name still means what the
            // reader thinks it does.
            if binds(fd, from) {
                return;
            }
            // A record declared privately is renamed where it is *written* as
            // well as where it is built: a package's own `Box` becomes
            // `pkg__Box`, and a signature still saying `Box` names a type
            // nothing declares. The C back end then gave the parameter a
            // fallback type and the program failed to compile against itself.
            for p in &mut fd.params {
                if let Some(t) = &mut p.ty {
                    rename_in_type(t, from, to);
                }
            }
            if let Some(t) = &mut fd.ret {
                rename_in_type(t, from, to);
            }
            match &mut fd.body {
                Some(FnBody::Expr(e)) => rename_in_expr(e, from, to),
                Some(FnBody::Block(ss)) => ss.iter_mut().for_each(|x| rename_ident(x, from, to)),
                None => {}
            }
        }
        Stmt::Import(_) => {}
    }
}

/// Does this function give `name` a meaning of its own — a parameter, a local,
/// a loop variable, or a lambda's parameter?
fn binds(fd: &FnDef, name: &str) -> bool {
    if fd.params.iter().any(|p| p.name == name) {
        return true;
    }
    let Some(body) = &fd.body else {
        return false;
    };
    let binder = |e: &Expr| {
        match e {
        Expr::Lambda { params, .. } => params.iter().any(|p| p.name == name),
        Expr::For { pat, .. } => pattern_binds(pat, name),
        Expr::Assign { target, .. } => matches!(&**target, Expr::Ident(n) if n == name),
        Expr::Match { arms, .. } => arms.iter().any(|a| pattern_binds(&a.pat, name)),
        Expr::Block(ss) | Expr::If { then: ss, .. } | Expr::While { body: ss, .. } => ss
            .iter()
            .any(|st| matches!(st, Stmt::Bind(b) if matches!(&b.target, Expr::Ident(n) if n == name))),
        _ => false,
    }
    };
    let mut found = false;
    let mut look = |e: &Expr| found = found || binder(e);
    match body {
        FnBody::Expr(e) => walk_expr(e, &mut look),
        FnBody::Block(ss) => {
            for st in ss {
                if let Stmt::Bind(b) = st
                    && matches!(&b.target, Expr::Ident(n) if n == name)
                {
                    return true;
                }
                walk_stmt(st, &mut look);
            }
        }
    }
    found
}

/// Does this pattern introduce `name`?
fn pattern_binds(p: &Pattern, name: &str) -> bool {
    match p {
        Pattern::Bind(n) => n == name,
        Pattern::Ctor { args, .. } => args.iter().any(|a| pattern_binds(a, name)),
        Pattern::Or(ps) => ps.iter().any(|a| pattern_binds(a, name)),
        Pattern::List { elems, rest } => {
            elems.iter().any(|a| pattern_binds(a, name))
                || rest.as_ref().is_some_and(|r| pattern_binds(r, name))
        }
        Pattern::Record(fs) => fs.iter().any(|(f, sub)| match sub {
            Some(p) => pattern_binds(p, name),
            None => f == name,
        }),
        _ => false,
    }
}

/// Rename a type's name wherever it appears — `T`, `T[]`, `T?`, `Map str T`.
fn rename_in_type(t: &mut Type, from: &str, to: &str) {
    match t {
        Type::Name(segs) => {
            for seg in segs {
                if seg == from {
                    *seg = to.to_string();
                }
            }
        }
        Type::Array(inner) | Type::Opt(inner) | Type::Paren(inner) => {
            rename_in_type(inner, from, to)
        }
        Type::Fn(params, ret) => {
            params.iter_mut().for_each(|p| rename_in_type(p, from, to));
            rename_in_type(ret, from, to);
        }
        Type::Apply(base, args) => {
            rename_in_type(base, from, to);
            args.iter_mut().for_each(|a| rename_in_type(a, from, to));
        }
    }
}

fn rename_in_expr(e: &mut Expr, from: &str, to: &str) {
    match e {
        Expr::Ident(n) => {
            if n == from {
                *n = to.to_string();
            }
        }
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::Bool(_)
        | Expr::Unit
        | Expr::Path(_)
        | Expr::Break
        | Expr::Continue => {}
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    rename_in_expr(x, from, to);
                }
            }
        }
        Expr::List(xs) => xs.iter_mut().for_each(|x| rename_in_expr(x, from, to)),
        Expr::Record(fields) => rename_in_fields(fields, from, to),
        Expr::Ctor { name, fields } => {
            if name == from {
                *name = to.to_string();
            }
            rename_in_fields(fields, from, to);
        }
        Expr::With { base, fields } => {
            rename_in_expr(base, from, to);
            rename_in_fields(fields, from, to);
        }
        Expr::Call { callee, args } => {
            rename_in_expr(callee, from, to);
            for a in args {
                rename_in_expr(arg_expr_mut(a), from, to);
            }
        }
        // The field *name* is a field, not a binding — only the base is renamed.
        Expr::Field { base, .. } => rename_in_expr(base, from, to),
        Expr::Index { base, index } => {
            rename_in_expr(base, from, to);
            rename_in_expr(index, from, to);
        }
        Expr::Range { lo, hi } => {
            rename_in_expr(lo, from, to);
            rename_in_expr(hi, from, to);
        }
        Expr::Unary { expr, .. } => rename_in_expr(expr, from, to),
        Expr::Binary { lhs, rhs, .. } => {
            rename_in_expr(lhs, from, to);
            rename_in_expr(rhs, from, to);
        }
        Expr::Ternary { cond, then, els } => {
            rename_in_expr(cond, from, to);
            rename_in_expr(then, from, to);
            rename_in_expr(els, from, to);
        }
        Expr::If { cond, then, els } => {
            rename_in_expr(cond, from, to);
            then.iter_mut().for_each(|x| rename_ident(x, from, to));
            if let Some(e) = els {
                e.iter_mut().for_each(|x| rename_ident(x, from, to));
            }
        }
        Expr::Match { scrut, arms } => {
            rename_in_expr(scrut, from, to);
            for a in arms {
                if let Some(g) = &mut a.guard {
                    rename_in_expr(g, from, to);
                }
                rename_in_expr(&mut a.body, from, to);
            }
        }
        Expr::For { iter, body, .. } => {
            rename_in_expr(iter, from, to);
            body.iter_mut().for_each(|x| rename_ident(x, from, to));
        }
        Expr::While { cond, body } => {
            rename_in_expr(cond, from, to);
            body.iter_mut().for_each(|x| rename_ident(x, from, to));
        }
        Expr::Lambda { body, .. } => rename_in_expr(body, from, to),
        Expr::Try(x) | Expr::Fail(x) | Expr::Reify(x) | Expr::Await(x) | Expr::Spawn(x) => {
            rename_in_expr(x, from, to)
        }
        Expr::Assign { target, value } => {
            rename_in_expr(target, from, to);
            rename_in_expr(value, from, to);
        }
        Expr::Block(ss) => ss.iter_mut().for_each(|x| rename_ident(x, from, to)),
    }
}

fn rename_in_fields(fields: &mut [Field], from: &str, to: &str) {
    for field in fields {
        match field {
            Field::Value { value, .. } | Field::Bare(value) => rename_in_expr(value, from, to),
            Field::Type { .. } | Field::Shorthand(_) => {}
        }
    }
}

/// The expression carried by a call argument, mutably.
pub fn arg_expr_mut(a: &mut Arg) -> &mut Expr {
    match a {
        Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => e,
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
