//! maca-backend-rust: lower Maca to Rust source (`--target rust`).
//!
//! Rust is the substrate for the crates.io ecosystem — the way the JVM backend
//! is for Maven. This backend transpiles Maca to a `.rs` file that `cargo`/
//! `rustc` compiles, so a Maca program can build on real Rust crates.
//!
//! Mapping (the functional core; foreign interop grows on top):
//!   * `main() -> int { … }` → `fn main()` that exits with the returned code
//!   * top-level functions → `fn`s
//!   * records → `#[derive(Clone)] struct`s; nullary sums → `enum`s
//!   * `int`→`i64`, `float`→`f64`, `str`→`String`, `bool`→`bool`, `T[]`→`Vec<T>`
//!   * `info(x)`/`print(x)` → `println!`/`print!`; interpolation → `format!`
//!
//! The generated file opens with `#![allow(warnings)]` — generated code is not
//! read, and Maca's mutability model produces harmless `unused_mut`/dead-code.

use maca_parser::ast::*;
use std::collections::BTreeSet;

/// The `import rust "spec"` specs, in source order — the crates.io / std items a
/// program pulls in. `import rust "gpui::div"` → the driver validates the crate
/// against `[rust-dependencies]` and `emit` turns it into `use gpui::div;`.
pub fn rust_imports(m: &Module) -> Vec<String> {
    m.items
        .iter()
        .filter_map(|it| match it {
            Stmt::Import(Import::Foreign { lang, spec }) if lang == "rust" => Some(spec.clone()),
            _ => None,
        })
        .collect()
}

/// Turn an `import rust` spec into a `use` item. A spec that already looks like
/// a full item (`use …`, an attribute, or a multi-line raw block) is emitted
/// verbatim; a bare path (`gpui::div`, `std::process`) is wrapped in `use …;`.
fn use_line(spec: &str) -> String {
    let s = spec.trim();
    if s.contains('\n') || s.starts_with("use ") || s.starts_with('#') {
        format!("{s}\n")
    } else {
        format!("use {s};\n")
    }
}

/// Emit a Rust compilation unit for `m`.
pub fn emit(m: &Module) -> String {
    let mut cx = Cx::default();
    cx.collect(m);
    let mut out = String::from("#![allow(warnings)]\n\n");
    for spec in rust_imports(m) {
        out.push_str(&use_line(&spec));
    }
    if m.items
        .iter()
        .any(|it| matches!(it, Stmt::Import(Import::Foreign { lang, .. }) if lang == "rust"))
    {
        out.push('\n');
    }
    out.push_str(&cx.prelude());

    for it in &m.items {
        match it {
            Stmt::Bind(b) => {
                if let Expr::Ident(name) = &b.target {
                    if let Some(vars) = sum_variants(&b.value) {
                        out.push_str(&emit_enum(name, &vars));
                    } else if let Some(fields) = record_fields(&b.value) {
                        out.push_str(&cx.emit_struct(name, &fields));
                    } else {
                        // a top-level constant binding
                        let (v, _) = cx.expr(&b.value);
                        out.push_str(&format!("static {}: i64 = {v};\n\n", name.to_uppercase()));
                    }
                }
            }
            Stmt::Fn(f) => out.push_str(&cx.emit_fn(f)),
            _ => {}
        }
    }
    out
}

#[derive(Default)]
struct Cx {
    records: BTreeSet<String>,
    sums: BTreeSet<String>,
    /// variant name → its enum, so a bare `Green` emits `Color::Green`.
    variant_of: std::collections::BTreeMap<String, String>,
    /// names already bound in the current function — a later `x = e` is a
    /// reassignment (`x = e`), not a new `let`.
    declared: BTreeSet<String>,
}

impl Cx {
    fn collect(&mut self, m: &Module) {
        for it in &m.items {
            if let Stmt::Bind(b) = it
                && let Expr::Ident(name) = &b.target
            {
                if let Some(vars) = sum_variants(&b.value) {
                    self.sums.insert(name.clone());
                    for v in vars {
                        self.variant_of.insert(v.name, name.clone());
                    }
                } else if record_fields(&b.value).is_some() {
                    self.records.insert(name.clone());
                }
            }
        }
    }

    /// Small runtime shims so generated calls stay readable.
    fn prelude(&self) -> String {
        String::from(
            "#[allow(dead_code)]\n\
             fn maca_len<T>(xs: &[T]) -> i64 { xs.len() as i64 }\n\n",
        )
    }

    // ---- declarations -----------------------------------------------------

    fn emit_fn(&mut self, f: &FnDef) -> String {
        // fresh binding scope per function; params are `mut` (a fn may reassign
        // a parameter, and `allow(warnings)` silences the unused-mut noise).
        self.declared.clear();
        for p in &f.params {
            self.declared.insert(p.name.clone());
        }
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| {
                format!(
                    "mut {}: {}",
                    ident(&p.name),
                    p.ty.as_ref().map(rust_ty).unwrap_or_else(|| "i64".into())
                )
            })
            .collect();
        let ret = f.ret.as_ref().map(rust_ty).unwrap_or_else(|| "()".into());
        let body = match &f.body {
            Some(FnBody::Expr(e)) => self.expr(e).0,
            Some(FnBody::Block(stmts)) => self.block(stmts),
            None => "unimplemented!()".into(),
        };

        if f.name == "main" {
            // `main() -> int` returns an exit code; wrap it so `fn main` exits.
            return format!(
                "fn __maca_main() -> i64 {{\n    {body}\n}}\n\n\
                 fn main() {{ std::process::exit(__maca_main() as i32); }}\n\n"
            );
        }
        format!(
            "#[allow(dead_code)]\nfn {}({}) -> {ret} {{\n    {body}\n}}\n\n",
            ident(&f.name),
            params.join(", ")
        )
    }

    fn emit_struct(&mut self, name: &str, fields: &[(String, Type)]) -> String {
        // `PartialEq` matches the enum derive so a sum variant can carry a
        // record payload (`Rows(Grid)`) — the derive cascades to field types.
        let mut out =
            format!("#[derive(Clone, Debug, PartialEq)]\n#[allow(dead_code)]\nstruct {name} {{\n");
        for (fname, ty) in fields {
            out.push_str(&format!("    {}: {},\n", ident(fname), rust_ty(ty)));
        }
        out.push_str("}\n\n");
        out
    }

    // ---- statements -------------------------------------------------------

    /// A block: bindings and expression statements, the last expression is the
    /// block's value (Rust's own block semantics).
    fn block(&mut self, stmts: &[Stmt]) -> String {
        let mut lines: Vec<String> = Vec::new();
        let last = stmts.len().saturating_sub(1);
        for (i, s) in stmts.iter().enumerate() {
            match s {
                Stmt::Bind(b) => {
                    let (v, _) = self.expr(&b.value);
                    match &b.target {
                        // first `x = e` declares; a later one reassigns.
                        Expr::Ident(n) if !self.declared.contains(n) => {
                            self.declared.insert(n.clone());
                            lines.push(format!("let mut {} = {v};", ident(n)));
                        }
                        Expr::Ident(n) => lines.push(format!("{} = {v};", ident(n))),
                        // an lvalue assignment written as a bind (`xs[i] = v`)
                        other => {
                            let (t, _) = self.expr(other);
                            lines.push(format!("{t} = {v};"));
                        }
                    }
                }
                Stmt::Expr(e) => {
                    let (c, _) = self.expr(e);
                    if i == last {
                        lines.push(c);
                    } else {
                        lines.push(format!("{c};"));
                    }
                }
                _ => {}
            }
        }
        if lines.is_empty() {
            return "()".into();
        }
        lines.join("\n    ")
    }

    // ---- expressions ------------------------------------------------------

    /// Lower an expression to `(rust_code, is_string)`. `is_string` lets `++`
    /// and interpolation choose string vs numeric handling.
    fn expr(&mut self, e: &Expr) -> (String, bool) {
        match e {
            // Maca `int` is 64-bit; suffix so Rust doesn't default literals to
            // `i32` (which then clashes with `i64` arithmetic / `Vec<i64>`).
            Expr::Int(n) => (format!("{n}i64"), false),
            Expr::Float(f) => (format!("{f}_f64"), false),
            Expr::Bool(b) => (b.to_string(), false),
            Expr::Str(parts) => (self.interp(parts), true),
            Expr::Unit => ("()".into(), false),
            Expr::Ident(n) => match self.variant_of.get(n) {
                Some(enom) => (format!("{enom}::{}", ident(n)), false),
                None => (ident(n), false),
            },
            Expr::Unary { op, expr } => {
                let (c, _) = self.expr(expr);
                let o = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                };
                (format!("({o}{c})"), false)
            }
            Expr::Binary { op, lhs, rhs } => self.binary(*op, lhs, rhs),
            Expr::Ternary { cond, then, els } => {
                let (c, _) = self.expr(cond);
                let (t, s) = self.expr(then);
                let (e2, _) = self.expr(els);
                (format!("(if {c} {{ {t} }} else {{ {e2} }})"), s)
            }
            Expr::If { cond, then, els } => {
                let (c, _) = self.expr(cond);
                let t = self.block(then);
                let e2 = els.as_ref().map(|s| self.block(s));
                match e2 {
                    Some(e2) => (format!("if {c} {{ {t} }} else {{ {e2} }}"), false),
                    None => (format!("if {c} {{ {t}; }}"), false),
                }
            }
            Expr::Block(stmts) => (format!("{{ {} }}", self.block(stmts)), false),
            Expr::While { cond, body } => {
                let (c, _) = self.expr(cond);
                let b = self.block(body);
                (format!("while {c} {{ {b}; }}"), false)
            }
            Expr::For { pat, iter, body } => {
                let (it, _) = self.expr(iter);
                let b = self.block(body);
                // `.clone()` so iterating doesn't move the original collection
                // (works for both `Vec` and a range).
                (
                    format!("for {} in ({it}).clone() {{ {b}; }}", pat_bind(pat)),
                    false,
                )
            }
            Expr::Range { lo, hi } => {
                let (l, _) = self.expr(lo);
                let (h, _) = self.expr(hi);
                (format!("({l}..={h})"), false)
            }
            Expr::List(es) => {
                let items: Vec<String> = es.iter().map(|x| self.expr(x).0).collect();
                (format!("vec![{}]", items.join(", ")), false)
            }
            Expr::Index { base, index } => {
                let (b, _) = self.expr(base);
                let (i, _) = self.expr(index);
                (format!("{b}[({i}) as usize]"), false)
            }
            Expr::Field { base, name } => {
                let (b, _) = self.expr(base);
                (format!("{b}.{}", ident(name)), false)
            }
            Expr::Assign { target, value } => {
                let (t, _) = self.expr(target);
                let (v, _) = self.expr(value);
                (format!("{t} = {v}"), false)
            }
            Expr::Match { scrut, arms } => (self.match_expr(scrut, arms), false),
            Expr::Ctor { name, fields } => (self.ctor(name, fields), false),
            Expr::Record(fields) => (self.ctor("", fields), false),
            Expr::Call { callee, args } => self.call(callee, args),
            _ => ("Default::default()".into(), false),
        }
    }

    fn binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr) -> (String, bool) {
        let (l, ls) = self.expr(lhs);
        let (r, rs) = self.expr(rhs);
        if matches!(op, BinOp::Concat) || (matches!(op, BinOp::Add) && (ls || rs)) {
            return (format!("format!(\"{{}}{{}}\", {l}, {r})"), true);
        }
        let o = match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            _ => "+",
        };
        let is_bool = matches!(
            op,
            BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Gt
                | BinOp::Le
                | BinOp::Ge
                | BinOp::And
                | BinOp::Or
        );
        (format!("({l} {o} {r})"), is_bool && false)
    }

    fn call(&mut self, callee: &Expr, args: &[Arg]) -> (String, bool) {
        let argv: Vec<(String, bool)> = args.iter().map(|a| self.expr(arg_expr(a))).collect();
        let a0 = argv.first().map(|(c, _)| c.clone()).unwrap_or_default();
        // By-value user calls *move* their arguments in Rust, but Maca values are
        // freely reusable — so a local variable passed by value is `.clone()`d
        // (a no-op for `Copy` scalars). Fresh temporaries (literals, calls, ctors)
        // aren't moved from anything, so they're passed as-is.
        let cloned: Vec<String> = args
            .iter()
            .zip(&argv)
            .map(|(a, (c, _))| match arg_expr(a) {
                Expr::Ident(n) if self.declared.contains(n) => format!("{c}.clone()"),
                _ => c.clone(),
            })
            .collect();
        if let Expr::Ident(name) = callee {
            match name.as_str() {
                "info" | "print" | "err" | "warn" | "debug" | "notice" => {
                    let macro_ = if name == "print" {
                        "print!"
                    } else {
                        "println!"
                    };
                    return (format!("{macro_}(\"{{}}\", {a0})"), false);
                }
                "str" => return (format!("format!(\"{{}}\", {a0})"), true),
                "int" => return (format!("(({a0}) as i64)"), false),
                "float" => return (format!("(({a0}) as f64)"), false),
                "len" => return (format!("(maca_len(&{a0}))"), false),
                _ => {}
            }
            let joined = cloned.join(", ");
            // a payload sum-variant constructor `Circle(5)` → `Shape::Circle(5)`.
            if let Some(enom) = self.variant_of.get(name) {
                return if joined.is_empty() {
                    (format!("{enom}::{}", ident(name)), false)
                } else {
                    (format!("{enom}::{}({joined})", ident(name)), false)
                };
            }
            // otherwise a plain function / foreign call (or a record ctor call).
            return (format!("{}({joined})", ident(name)), false);
        }
        // UFCS / method call: `recv.method(args)`
        if let Expr::Field { base, name } = callee {
            let (b, _) = self.expr(base);
            let joined = cloned.join(", ");
            return (format!("{b}.{}({joined})", ident(name)), false);
        }
        ("Default::default()".into(), false)
    }

    fn ctor(&mut self, name: &str, fields: &[Field]) -> String {
        let mut parts = Vec::new();
        for f in fields {
            match f {
                Field::Value { name: n, value } => {
                    let (v, _) = self.expr(value);
                    parts.push(format!("{}: {v}", ident(n)));
                }
                Field::Shorthand(n) => parts.push(format!("{}: {}", ident(n), ident(n))),
                _ => {}
            }
        }
        format!("{name} {{ {} }}", parts.join(", "))
    }

    fn match_expr(&mut self, scrut: &Expr, arms: &[Arm]) -> String {
        let (s, _) = self.expr(scrut);
        let mut out = format!("match {s} {{\n");
        for arm in arms {
            let pat = self.pat_match(&arm.pat);
            let (body, _) = self.expr(&arm.body);
            if let Some(g) = &arm.guard {
                let (gc, _) = self.expr(g);
                out.push_str(&format!("        {pat} if {gc} => {{ {body} }}\n"));
            } else {
                out.push_str(&format!("        {pat} => {{ {body} }}\n"));
            }
        }
        out.push_str("    }");
        out
    }

    /// A `match` pattern, qualifying any variant name to `Enum::Variant`.
    fn pat_match(&self, p: &Pattern) -> String {
        match p {
            Pattern::Wild => "_".into(),
            Pattern::Int(n) => n.to_string(),
            Pattern::Bool(b) => b.to_string(),
            Pattern::Str(s) => format!("{s:?}"),
            // a capitalized "binder" that is really a nullary variant.
            Pattern::Bind(n) => match self.variant_of.get(n) {
                Some(enom) => format!("{enom}::{}", ident(n)),
                None => ident(n),
            },
            Pattern::Ctor { name, args } => {
                let qualified = match self.variant_of.get(name) {
                    Some(enom) => format!("{enom}::{}", ident(name)),
                    None => ident(name),
                };
                if args.is_empty() {
                    qualified
                } else {
                    let a: Vec<String> = args.iter().map(|x| self.pat_match(x)).collect();
                    format!("{qualified}({})", a.join(", "))
                }
            }
            Pattern::Or(ps) => ps
                .iter()
                .map(|x| self.pat_match(x))
                .collect::<Vec<_>>()
                .join(" | "),
            _ => "_".into(),
        }
    }

    fn interp(&self, parts: &[StrPart]) -> String {
        // one plain text run → a string literal
        if parts.len() == 1
            && let StrPart::Text(t) = &parts[0]
        {
            return format!("{:?}.to_string()", t);
        }
        if parts.is_empty() {
            return "String::new()".into();
        }
        let mut fmt = String::new();
        let mut args: Vec<String> = Vec::new();
        for p in parts {
            match p {
                StrPart::Text(t) => fmt.push_str(&t.replace('{', "{{").replace('}', "}}")),
                StrPart::Interp(e) => {
                    fmt.push_str("{}");
                    let mut c = Cx {
                        records: self.records.clone(),
                        sums: self.sums.clone(),
                        variant_of: self.variant_of.clone(),
                        declared: self.declared.clone(),
                    };
                    args.push(c.expr(e).0);
                }
            }
        }
        if args.is_empty() {
            format!("{fmt:?}.to_string()")
        } else {
            format!("format!({fmt:?}, {})", args.join(", "))
        }
    }
}

// ---- free helpers ---------------------------------------------------------

/// One variant of a sum type: its name and (possibly empty) payload types.
struct Variant {
    name: String,
    payload: Vec<String>, // Rust types
}

/// The variants of a sum-type declaration — `A | B(x) | C(x, y)` — or `None` if
/// the value isn't a top-level union of variant forms. Each leaf is either a
/// bare name (a nullary variant) or a call on a capitalized name whose argument
/// "expressions" are really the payload types (`Circle(int)` parses as a call
/// because the whole `T = …` right-hand side is parsed in expression position).
fn sum_variants(e: &Expr) -> Option<Vec<Variant>> {
    fn leaf(e: &Expr, out: &mut Vec<Variant>) -> bool {
        match e {
            Expr::Ident(n) => {
                out.push(Variant {
                    name: n.clone(),
                    payload: vec![],
                });
                true
            }
            Expr::Call { callee, args } => match &**callee {
                Expr::Ident(n) => {
                    out.push(Variant {
                        name: n.clone(),
                        payload: args.iter().map(|a| expr_as_rust_ty(arg_expr(a))).collect(),
                    });
                    true
                }
                _ => false,
            },
            Expr::Binary {
                op: BinOp::Union,
                lhs,
                rhs,
            } => leaf(lhs, out) && leaf(rhs, out),
            _ => false,
        }
    }
    match e {
        Expr::Binary {
            op: BinOp::Union, ..
        } => {
            let mut out = Vec::new();
            leaf(e, &mut out).then_some(out)
        }
        _ => None,
    }
}

fn emit_enum(name: &str, vars: &[Variant]) -> String {
    let body: Vec<String> = vars
        .iter()
        .map(|v| {
            if v.payload.is_empty() {
                v.name.clone()
            } else {
                format!("{}({})", v.name, v.payload.join(", "))
            }
        })
        .collect();
    format!(
        "#[derive(Clone, Debug, PartialEq)]\n#[allow(dead_code)]\nenum {name} {{ {} }}\n\n",
        body.join(", ")
    )
}

/// Map a payload/type "expression" (from a ctor declaration) to a Rust type.
fn expr_as_rust_ty(e: &Expr) -> String {
    match e {
        Expr::Ident(n) => scalar_ty(n),
        // `T[]` payloads parse as an index expression; recover the element type.
        Expr::Index { base, .. } => format!("Vec<{}>", expr_as_rust_ty(base)),
        _ => "i64".into(),
    }
}

/// A scalar/type name → its Rust spelling; unknown capitalized names (a user
/// record/sum, or a foreign Rust type) pass through verbatim.
fn scalar_ty(n: &str) -> String {
    match n {
        "int" | "i64" => "i64",
        "i32" => "i32",
        "float" | "f64" => "f64",
        "f32" => "f32",
        "str" | "bytes" => "String",
        "bool" => "bool",
        "unit" | "()" => "()",
        other => other,
    }
    .into()
}

/// Map a Maca type to a Rust type. Unknown capitalized names pass through
/// verbatim (a user record/sum, or later a foreign Rust type).
fn rust_ty(t: &Type) -> String {
    match t {
        Type::Name(segs) => scalar_ty(segs.last().map(String::as_str).unwrap_or("")),
        Type::Array(e) => format!("Vec<{}>", rust_ty(e)),
        Type::Opt(e) => format!("Option<{}>", rust_ty(e)),
        Type::Paren(e) => rust_ty(e),
        Type::Apply(base, args) => {
            let a: Vec<String> = args.iter().map(rust_ty).collect();
            format!("{}<{}>", rust_ty(base), a.join(", "))
        }
    }
}

/// A pattern in binding position (`for x in …`, match binder).
fn pat_bind(p: &Pattern) -> String {
    match p {
        Pattern::Bind(n) => ident(n),
        Pattern::Wild => "_".into(),
        _ => "_".into(),
    }
}

fn arg_expr(a: &Arg) -> &Expr {
    match a {
        Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => e,
    }
}

/// Record field helpers (a record literal used as a type declaration value).
fn record_fields(e: &Expr) -> Option<Vec<(String, Type)>> {
    if let Expr::Record(fs) = e {
        let mut out = Vec::new();
        for f in fs {
            if let Field::Type { name, ty } = f {
                out.push((name.clone(), ty.clone()));
            } else {
                return None;
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

/// Escape a Maca identifier that collides with a Rust keyword.
fn ident(n: &str) -> String {
    const KW: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
        "where", "while", "async", "await", "dyn", "box", "final",
    ];
    if KW.contains(&n) {
        format!("r#{n}")
    } else {
        n.replace('-', "_")
    }
}
