use maca_parser::ast::*;
use std::collections::{BTreeMap, BTreeSet};

/// The `import rust "spec"` specs, in source order: the crates.io / std items a program pulls in.
fn rust_imports(m: &Module) -> Vec<String> {
    m.items
        .iter()
        .filter_map(|it| match it {
            Stmt::Import(Import::Foreign { lang, spec }) if lang == "rust" => Some(spec.clone()),
            _ => None,
        })
        .collect()
}

/// Turn an `import rust` spec into a `use` item.
fn use_line(spec: &str) -> String {
    let s = spec.trim();
    if is_rust_path(s) {
        format!("use {s};\n")
    } else {
        format!("{s}\n")
    }
}

/// A bare `a::b::c` crate path (only identifier chars, `::`, and raw `#`), as opposed to a raw Rust code block.
fn is_rust_path(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':' || c == '#')
}

/// Emit a Rust compilation unit for `m`.
pub fn emit_checked(m: &Module) -> Result<String, Vec<String>> {
    let mut problems = Vec::new();
    let out = emit_collecting(m, &mut problems);
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

pub fn emit(m: &Module) -> String {
    emit_collecting(m, &mut Vec::new())
}

fn emit_collecting(m: &Module, problems: &mut Vec<String>) -> String {
    let mut cx = Cx {
        named_shapes: named_by_shape(m),
        ..Default::default()
    };
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

    for (name, fields) in anon_shapes(m) {
        let owned = cx
            .named_shapes
            .get(&name)
            .is_some_and(|names| names.len() == 1);
        if !owned {
            out.push_str(&cx.emit_struct(&name, &fields));
        }
    }

    for it in &m.items {
        match it {
            Stmt::Bind(b) => {
                if let Expr::Ident(name) = &b.target {
                    if let Some(vars) = sum_variants(&b.value) {
                        out.push_str(&emit_enum(name, &vars));
                    } else if let (Some(trait_ty), Some(methods)) =
                        (b.tys.first(), lambda_fields(&b.value))
                    {
                        out.push_str(&cx.emit_impl(name, trait_ty, &methods));
                    } else if let Some(fields) = record_fields(&b.value) {
                        out.push_str(&cx.emit_struct(name, &fields));
                    } else {
                        let (v, _) = cx.expr(&b.value);
                        out.push_str(&format!("static {}: i64 = {v};\n\n", name.to_uppercase()));
                    }
                }
            }
            Stmt::Fn(f) => out.push_str(&cx.emit_fn(f)),
            _ => {}
        }
    }
    problems.append(&mut cx.problems);
    out
}

#[derive(Default)]
struct Cx {
    records: BTreeSet<String>,
    /// Field shape -> every named record declared with it (see `named_by_shape`).
    named_shapes: BTreeMap<String, Vec<String>>,
    sums: BTreeSet<String>,
    /// Trait-impl methods, by name → which of their arguments the method takes as a mutable borrow.
    borrowed_args: BTreeMap<String, Vec<bool>>,
    /// The parameters of the method being emitted that are borrows.
    borrows: BTreeSet<String>,
    /// variant name → its enum, so a bare `Green` emits `Color::Green`.
    variant_of: std::collections::BTreeMap<String, String>,
    /// names already bound in the current function, so a later `x = e` is a reassignment (`x = e`), not a new `let`.
    declared: BTreeSet<String>,
    /// Constructs this backend does not lower.
    problems: Vec<String>,
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
        for it in &m.items {
            if let Stmt::Bind(b) = it
                && !b.tys.is_empty()
                && let Some(methods) = lambda_fields(&b.value)
            {
                for m in methods {
                    let borrowed = m
                        .params
                        .iter()
                        .skip_while(|p| p.name == "self")
                        .map(|p| p.ty.as_ref().is_some_and(|t| self.is_foreign_ty(t)))
                        .collect();
                    self.borrowed_args.insert(m.name.clone(), borrowed);
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

    fn emit_fn(&mut self, f: &FnDef) -> String {
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
        let mut out =
            format!("#[derive(Clone, Debug, PartialEq)]\n#[allow(dead_code)]\nstruct {name} {{\n");
        for (fname, ty) in fields {
            out.push_str(&format!("    {}: {},\n", ident(fname), rust_ty(ty)));
        }
        out.push_str("}\n\n");
        out
    }

    /// A foreign trait implementation.
    fn method_param_ty(&self, p: &Param) -> String {
        match &p.ty {
            Some(t) if self.is_foreign_ty(t) => format!("&mut {}", rust_ty(t)),
            Some(t) => format!("mut {}", rust_ty(t)),
            None => "mut i64".into(),
        }
    }

    /// Is this type from outside the module, neither a Maca scalar nor a record/sum declared here?
    fn is_foreign_ty(&self, t: &Type) -> bool {
        let head = match t {
            Type::Name(segs) => segs.last().cloned().unwrap_or_default(),
            Type::Apply(base, _) | Type::Paren(base) => return self.is_foreign_ty(base),
            Type::Fn(ps, r) => {
                return ps.iter().any(|p| self.is_foreign_ty(p)) || self.is_foreign_ty(r);
            }
            Type::Array(_) | Type::Opt(_) => return false,
        };
        !head.is_empty()
            && head.chars().next().is_some_and(char::is_uppercase)
            && !self.records.contains(&head)
            && !self.sums.contains(&head)
    }

    fn emit_impl(&mut self, name: &str, trait_ty: &Type, methods: &[Method]) -> String {
        let mut ms = String::new();
        for m in methods {
            self.declared.clear();
            self.borrows.clear();
            let mut ps = Vec::new();
            for (i, p) in m.params.iter().enumerate() {
                if i == 0 && p.name == "self" {
                    ps.push("&mut self".to_string());
                } else {
                    self.declared.insert(p.name.clone());
                    if p.ty.as_ref().is_some_and(|t| self.is_foreign_ty(t)) {
                        self.borrows.insert(p.name.clone());
                    }
                    ps.push(format!("{}: {}", ident(&p.name), self.method_param_ty(p)));
                }
            }
            let (b, is_str) = self.expr(&m.body);
            let ret = match &m.ret {
                Some(t) => rust_ty(t),
                None => guess_ret(&m.body, is_str),
            };
            let arrow = if ret == "()" {
                String::new()
            } else {
                format!(" -> {ret}")
            };
            ms.push_str(&format!(
                "    fn {}({}){arrow} {{ {b} }}\n",
                ident(&m.name),
                ps.join(", ")
            ));
        }
        format!("impl {} for {name} {{\n{ms}}}\n\n", rust_ty(trait_ty))
    }

    /// A block: bindings and expression statements, the last expression is the block's value (Rust's own block semantics).
    fn block(&mut self, stmts: &[Stmt]) -> String {
        let mut lines: Vec<String> = Vec::new();
        let last = stmts.len().saturating_sub(1);
        for (i, s) in stmts.iter().enumerate() {
            match s {
                Stmt::Bind(b) => {
                    let (v, _) = self.expr(&b.value);
                    match &b.target {
                        Expr::Ident(n) if !self.declared.contains(n) => {
                            self.declared.insert(n.clone());
                            lines.push(format!("let mut {} = {v};", ident(n)));
                        }
                        Expr::Ident(n) => lines.push(format!("{} = {v};", ident(n))),
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

    /// Lower an expression to `(rust_code, is_string)`.
    fn expr(&mut self, e: &Expr) -> (String, bool) {
        match e {
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
            Expr::Spawn(inner) => {
                let (e, _) = self.expr(inner);
                (format!("std::thread::spawn(move || {e})"), false)
            }
            Expr::Await(inner) => {
                let (e, _) = self.expr(inner);
                (format!("({e}).join().unwrap()"), false)
            }
            Expr::Record(fields) => {
                let shape = anon_shape(fields).map(|s| anon_struct_name(&s));
                let n = shape.as_ref().map(|k| match self.named_shapes.get(k) {
                    Some(names) if names.len() == 1 => names[0].clone(),
                    Some(names) => {
                        self.problem(format!(
                            "this `{{ … }}` matches more than one declared \
                             record ({}), and the rust backend needs one; write \
                             the constructor, as in `{} {{ … }}`",
                            names.join(", "),
                            names[0]
                        ));
                        k.clone()
                    }
                    None => k.clone(),
                });
                (self.ctor(n.as_deref().unwrap_or(""), fields), false)
            }
            Expr::Call { callee, args } => self.call(callee, args),
            Expr::Lambda { params, body, .. } => {
                let ps: Vec<String> = params.iter().map(|p| ident(&p.name)).collect();
                let (b, _) = self.expr(body);
                (format!("move |{}| {{ {b} }}", ps.join(", ")), false)
            }
            Expr::Break => ("break".into(), false),
            Expr::Continue => ("continue".into(), false),
            Expr::With { base, fields } => {
                let (b, _) = self.expr(base);
                let mut out = format!("{{ let mut _w = {b}.clone(); ");
                for f in fields {
                    let (n, v) = match f {
                        Field::Value { name, value } => (name.clone(), self.expr(value).0),
                        Field::Shorthand(n) => (n.clone(), ident(n)),
                        _ => continue,
                    };
                    out.push_str(&format!("_w.{} = {v}; ", ident(&n)));
                }
                out.push_str("_w }");
                (out, false)
            }
            other => {
                self.problem(format!(
                    "{} is not lowered by the rust backend",
                    describe(other)
                ));
                ("Default::default()".into(), false)
            }
        }
    }

    fn problem(&mut self, msg: impl Into<String>) {
        self.problems.push(msg.into());
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
        let cloned: Vec<String> = args
            .iter()
            .zip(&argv)
            .map(|(a, (c, _))| match arg_expr(a) {
                Expr::Ident(n) if self.borrows.contains(n) => c.clone(),
                Expr::Ident(n) if self.declared.contains(n) => format!("{c}.clone()"),
                _ => c.clone(),
            })
            .collect();
        let foreign_args: Vec<String> = args
            .iter()
            .zip(&argv)
            .map(|(a, (c, _))| match arg_expr(a) {
                Expr::Int(n) => n.to_string(),
                Expr::Ident(nm) if self.borrows.contains(nm) => c.clone(),
                Expr::Ident(nm) if self.declared.contains(nm) => format!("{c}.clone()"),
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
            if let Some(enom) = self.variant_of.get(name) {
                return if joined.is_empty() {
                    (format!("{enom}::{}", ident(name)), false)
                } else {
                    (format!("{enom}::{}({joined})", ident(name)), false)
                };
            }
            if self.is_foreign_type(name) {
                return (
                    format!("{}::new({})", ident(name), foreign_args.join(", ")),
                    false,
                );
            }
            return (format!("{}({joined})", ident(name)), false);
        }
        if let Expr::Field { base, name } = callee {
            let joined = cloned.join(", ");
            if let Expr::Ident(bn) = &**base
                && self.is_foreign_type(bn)
            {
                return (
                    format!(
                        "{}::{}({})",
                        ident(bn),
                        ident(name),
                        foreign_args.join(", ")
                    ),
                    false,
                );
            }
            let (b, _) = self.expr(base);
            if let Some(borrowed) = self.borrowed_args.get(name) {
                let args: Vec<String> = cloned
                    .iter()
                    .enumerate()
                    .map(|(i, a)| match borrowed.get(i) {
                        Some(true) => format!("&mut {a}"),
                        _ => a.clone(),
                    })
                    .collect();
                return (format!("{b}.{}({})", ident(name), args.join(", ")), false);
            }
            return (format!("{b}.{}({joined})", ident(name)), false);
        }
        ("Default::default()".into(), false)
    }

    /// A capitalized name that isn't a local record, sum, variant, or bound variable, i.e. a type coming from an `import rust` crate.
    fn is_foreign_type(&self, n: &str) -> bool {
        n.chars().next().is_some_and(char::is_uppercase)
            && !self.records.contains(n)
            && !self.sums.contains(n)
            && !self.variant_of.contains_key(n)
            && !self.declared.contains(n)
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
    fn pat_match(&mut self, p: &Pattern) -> String {
        match p {
            Pattern::Wild => "_".into(),
            Pattern::Int(n) => n.to_string(),
            Pattern::Bool(b) => b.to_string(),
            Pattern::Str(s) => format!("{s:?}"),
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
                    let a: Vec<String> = args.iter().map(|x| self.pat_match(x)).collect::<Vec<_>>();
                    format!("{qualified}({})", a.join(", "))
                }
            }
            Pattern::Or(ps) => {
                let a: Vec<String> = ps.iter().map(|x| self.pat_match(x)).collect();
                a.join(" | ")
            }
            Pattern::Float(f) => {
                self.problem(format!(
                    "the float pattern `{f}` has no Rust equivalent; \
                     compare with a guard instead"
                ));
                "_".into()
            }
            Pattern::Record(_) => {
                self.problem("a record pattern is not lowered by the rust backend".to_string());
                "_".into()
            }
            Pattern::List { .. } => {
                self.problem("a list pattern is not lowered by the rust backend".to_string());
                "_".into()
            }
        }
    }

    fn interp(&self, parts: &[StrPart]) -> String {
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
                        problems: Vec::new(),
                        records: self.records.clone(),
                        named_shapes: self.named_shapes.clone(),
                        sums: self.sums.clone(),
                        borrowed_args: self.borrowed_args.clone(),
                        borrows: self.borrows.clone(),
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

/// One variant of a sum type: its name and (possibly empty) payload types.
struct Variant {
    name: String,
    payload: Vec<String>,
}

/// The variants of a sum-type declaration (`A | B(x) | C(x, y)`), or `None` if the value isn't a top-level union of variant forms.
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

/// A record whose every field is `name = (params) => body` → the methods of a trait impl.
fn lambda_fields(e: &Expr) -> Option<Vec<Method>> {
    let Expr::Record(fields) = e else { return None };
    let mut out = Vec::new();
    for f in fields {
        if let Field::Value { name, value } = f
            && let Expr::Lambda { params, ret, body } = value
        {
            out.push(Method {
                name: name.clone(),
                params: params.clone(),
                ret: ret.clone(),
                body: (**body).clone(),
            });
            continue;
        }
        return None;
    }
    (!out.is_empty()).then_some(out)
}

/// One method of a foreign trait impl.
struct Method {
    name: String,
    params: Vec<Param>,
    /// The declared return type, when the method wrote one.
    ret: Option<Type>,
    body: Expr,
}

/// Best-effort return type for a trait-impl method body (Maca lambdas carry no return annotation).
fn guess_ret(body: &Expr, is_str: bool) -> String {
    use BinOp::*;
    if is_str {
        return "String".into();
    }
    match body {
        Expr::Unit | Expr::Assign { .. } | Expr::While { .. } | Expr::For { .. } => "()".into(),
        Expr::Bool(_)
        | Expr::Unary { op: UnOp::Not, .. }
        | Expr::Binary {
            op: Eq | Ne | Lt | Gt | Le | Ge | And | Or,
            ..
        } => "bool".into(),
        Expr::Block(stmts) => match stmts.last() {
            Some(Stmt::Expr(e)) => guess_ret(e, false),
            _ => "()".into(),
        },
        _ => "i64".into(),
    }
}

/// Map a payload/type "expression" (from a ctor declaration) to a Rust type.
fn expr_as_rust_ty(e: &Expr) -> String {
    match e {
        Expr::Ident(n) => scalar_ty(n),
        Expr::Index { base, .. } => format!("Vec<{}>", expr_as_rust_ty(base)),
        _ => "i64".into(),
    }
}

/// A scalar/type name → its Rust spelling.
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

/// Map a Maca type to a Rust type.
fn rust_ty(t: &Type) -> String {
    match t {
        Type::Name(segs) => scalar_ty(segs.last().map(String::as_str).unwrap_or("")),
        Type::Array(e) => format!("Vec<{}>", rust_ty(e)),
        Type::Opt(e) => format!("Option<{}>", rust_ty(e)),
        Type::Paren(e) => rust_ty(e),
        Type::Fn(ps, r) => {
            let a: Vec<String> = ps.iter().map(rust_ty).collect();
            format!("Box<dyn Fn({}) -> {}>", a.join(", "), rust_ty(r))
        }
        Type::Apply(base, args) => {
            let a: Vec<String> = args.iter().map(rust_ty).collect();
            format!("{}<{}>", rust_ty(base), a.join(", "))
        }
    }
}

/// Name a construct the way the author wrote it, for a refusal message.
fn describe(e: &Expr) -> &'static str {
    match e {
        Expr::Try(_) | Expr::Fail(_) | Expr::Reify(_) => "the error operators (`?`, `fail`)",
        Expr::Path(_) => "a path expression",
        Expr::Await(_) | Expr::Spawn(_) => "`await`/`spawn`",
        Expr::Range { .. } => "a range in value position",
        _ => "this construct",
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

/// The shape of an anonymous record *value* (`{ host = "x", port = 80 }`), sorted by field name.
fn anon_shape(fs: &[Field]) -> Option<Vec<(String, Type)>> {
    let mut out = Vec::new();
    for f in fs {
        let Field::Value { name, value } = f else {
            return None;
        };
        out.push((name.clone(), shallow_type(value)?));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    (!out.is_empty()).then_some(out)
}

/// A best-effort type for a record field's value.
fn shallow_type(e: &Expr) -> Option<Type> {
    let name = |n: &str| Type::Name(vec![n.to_string()]);
    Some(match e {
        Expr::Int(_) => name("int"),
        Expr::Float(_) => name("float"),
        Expr::Bool(_) => name("bool"),
        Expr::Str(_) | Expr::Path(_) => name("str"),
        Expr::List(es) => Type::Array(Box::new(shallow_type(es.first()?)?)),
        Expr::Record(fs) => name(&anon_struct_name(&anon_shape(fs)?)),
        _ => return None,
    })
}

/// The struct name for an anonymous record's shape, derived from the shape, so it is the same everywhere the shape is.
fn anon_struct_name(fields: &[(String, Type)]) -> String {
    let mut s = String::from("MacaAnon");
    for (n, t) in fields {
        s.push('_');
        s.push_str(&ident(n));
        s.push('_');
        s.push_str(&type_tag(t));
    }
    s
}

fn type_tag(t: &Type) -> String {
    match t {
        Type::Name(segs) => ident(segs.last().map(String::as_str).unwrap_or("any")),
        Type::Array(e) => format!("{}arr", type_tag(e)),
        Type::Opt(e) => format!("{}opt", type_tag(e)),
        Type::Paren(e) => type_tag(e),
        Type::Apply(b, _) => type_tag(b),
        Type::Fn(_, r) => format!("fn{}", type_tag(r)),
    }
}

/// Every distinct anonymous-record shape in the module, in a stable order.
fn named_by_shape(m: &Module) -> BTreeMap<String, Vec<String>> {
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for it in &m.items {
        if let Stmt::Bind(b) = it
            && let Expr::Ident(name) = &b.target
            && let Some(fields) = record_fields(&b.value)
        {
            let mut shape = fields;
            shape.sort_by(|a, c| a.0.cmp(&c.0));
            seen.entry(anon_struct_name(&shape))
                .or_default()
                .push(name.clone());
        }
    }
    seen
}

fn anon_shapes(m: &Module) -> Vec<(String, Vec<(String, Type)>)> {
    let mut found: BTreeMap<String, Vec<(String, Type)>> = BTreeMap::new();
    for it in &m.items {
        walk_stmt_for_anon(it, &mut found);
    }
    found.into_iter().collect()
}

fn walk_stmt_for_anon(s: &Stmt, out: &mut BTreeMap<String, Vec<(String, Type)>>) {
    match s {
        Stmt::Bind(b) if record_fields(&b.value).is_some() => {}
        Stmt::Bind(b) => walk_expr_for_anon(&b.value, out),
        Stmt::Fn(f) => match &f.body {
            Some(FnBody::Expr(e)) => walk_expr_for_anon(e, out),
            Some(FnBody::Block(ss)) => ss.iter().for_each(|s| walk_stmt_for_anon(s, out)),
            None => {}
        },
        Stmt::Expr(e) => walk_expr_for_anon(e, out),
        _ => {}
    }
}

fn walk_expr_for_anon(e: &Expr, out: &mut BTreeMap<String, Vec<(String, Type)>>) {
    if let Expr::Record(fs) = e
        && let Some(shape) = anon_shape(fs)
    {
        out.insert(anon_struct_name(&shape), shape);
    }
    let go = walk_expr_for_anon;
    let body = |ss: &[Stmt], out: &mut BTreeMap<String, Vec<(String, Type)>>| {
        ss.iter().for_each(|s| walk_stmt_for_anon(s, out))
    };
    match e {
        Expr::Str(parts) => parts.iter().for_each(|p| {
            if let StrPart::Interp(x) = p {
                go(x, out)
            }
        }),
        Expr::List(es) => es.iter().for_each(|x| go(x, out)),
        Expr::Record(fs) | Expr::Ctor { fields: fs, .. } | Expr::With { fields: fs, .. } => {
            fs.iter().for_each(|f| {
                if let Field::Value { value, .. } | Field::Bare(value) = f {
                    go(value, out)
                }
            })
        }
        Expr::Call { callee, args } => {
            go(callee, out);
            args.iter().for_each(|a| go(arg_expr(a), out));
        }
        Expr::Field { base, .. }
        | Expr::Unary { expr: base, .. }
        | Expr::Try(base)
        | Expr::Fail(base)
        | Expr::Reify(base)
        | Expr::Await(base)
        | Expr::Spawn(base)
        | Expr::Lambda { body: base, .. } => go(base, out),
        Expr::Index { base: a, index: b }
        | Expr::Range { lo: a, hi: b }
        | Expr::Binary { lhs: a, rhs: b, .. }
        | Expr::Assign {
            target: a,
            value: b,
        } => {
            go(a, out);
            go(b, out);
        }
        Expr::Ternary { cond, then, els } => {
            go(cond, out);
            go(then, out);
            go(els, out);
        }
        Expr::If { cond, then, els } => {
            go(cond, out);
            body(then, out);
            if let Some(e) = els {
                body(e, out);
            }
        }
        Expr::Match { scrut, arms } => {
            go(scrut, out);
            arms.iter().for_each(|a| {
                go(&a.body, out);
                if let Some(g) = &a.guard {
                    go(g, out)
                }
            });
        }
        Expr::For { iter, body: b, .. } => {
            go(iter, out);
            body(b, out);
        }
        Expr::While { cond, body: b } => {
            go(cond, out);
            body(b, out);
        }
        Expr::Block(ss) => body(ss, out),
        _ => {}
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
        "as", "break", "const", "continue", "else", "enum", "extern", "false", "fn", "for", "if",
        "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
        "static", "struct", "trait", "true", "type", "unsafe", "use", "where", "while", "async",
        "await", "dyn", "box", "final",
    ];
    if KW.contains(&n) {
        format!("r#{n}")
    } else {
        n.replace('-', "_")
    }
}
