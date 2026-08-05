use crate::ast::*;
use std::fmt::Write;

pub fn print_module(m: &Module) -> String {
    let mut s = String::new();
    for (i, item) in m.items.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        stmt(&mut s, item);
    }
    s.push('\n');
    s
}

fn stmt(s: &mut String, st: &Stmt) {
    match st {
        Stmt::Import(im) => import(s, im),
        Stmt::Alias { name, value } => {
            let _ = write!(s, "alias {name} = ");
            expr(s, value);
        }
        Stmt::Fn(f) => fndef(s, f),
        Stmt::Bind(b) => bind(s, b),
        Stmt::Expr(e) => expr(s, e),
    }
}

fn import(s: &mut String, im: &Import) {
    match im {
        Import::Module(segs) => {
            let _ = write!(s, "import {}", segs.join("/"));
        }
        Import::Names { names, module } => {
            let _ = write!(
                s,
                "import {{ {} }} from {}",
                names.join(", "),
                module.join("/")
            );
        }
        Import::Path(p) => {
            let _ = write!(s, "import {p}");
        }
        Import::Bare(n) => {
            let _ = write!(s, "import {n}");
        }
        Import::Foreign { lang, spec } => match lang.as_str() {
            "css" | "js" => {
                let _ = write!(s, "import {lang} \"\"\"{spec}\"\"\"");
            }
            "stylesheet" | "script" | "wasm" => {
                let _ = write!(s, "import \"{}\"", escape(spec));
            }
            _ => {
                let _ = write!(s, "import {lang} \"{}\"", escape(spec));
            }
        },
        Import::ForeignNames { names, spec } => {
            let _ = write!(
                s,
                "import {{ {} }} from \"{}\"",
                names.join(", "),
                escape(spec)
            );
        }
    }
}

fn bind(s: &mut String, b: &Bind) {
    let capital =
        matches!(&b.target, Expr::Ident(n) if n.chars().next().is_some_and(|c| c.is_uppercase()));
    if b.is_const && !capital {
        s.push_str("const ");
    }
    expr(s, &b.target);
    for t in &b.tys {
        s.push_str(": ");
        s.push_str(&ty(t));
    }
    s.push_str(" = ");
    expr(s, &b.value);
}

fn fndef(s: &mut String, f: &FnDef) {
    let _ = write!(s, "{}(", f.name);
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        if p.variadic {
            s.push_str("...");
        }
        s.push_str(&p.name);
        if let Some(t) = &p.ty {
            let _ = write!(s, ": {}", ty(t));
        }
    }
    s.push(')');
    if let Some(r) = &f.ret {
        let _ = write!(s, " -> {}", ty(r));
    }
    if let Some(effs) = &f.effects {
        let _ = write!(s, " / <{}>", effs.join(", "));
    }
    match &f.body {
        Some(FnBody::Block(stmts)) => {
            s.push(' ');
            block(s, stmts);
        }
        Some(FnBody::Expr(e)) => {
            s.push_str(" => ");
            expr(s, e);
        }
        None => {}
    }
}

fn block(s: &mut String, stmts: &[Stmt]) {
    s.push_str("{\n");
    for st in stmts {
        stmt(s, st);
        s.push('\n');
    }
    s.push('}');
}

fn ty(t: &Type) -> String {
    match t {
        Type::Name(segs) => segs.join("."),
        Type::Apply(h, args) => {
            let mut out = ty(h);
            for a in args {
                out.push(' ');
                out.push_str(&ty(a));
            }
            out
        }
        Type::Array(t) => format!("{}[]", ty(t)),
        Type::Opt(t) => format!("{}?", ty(t)),
        Type::Paren(t) => format!("({})", ty(t)),
        Type::Fn(ps, r) => {
            let ps: Vec<String> = ps.iter().map(ty).collect();
            format!("({}) -> {}", ps.join(", "), ty(r))
        }
    }
}

/// Does this expression need parentheses when used as an operand?
fn compound(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Binary { .. }
            | Expr::Unary { .. }
            | Expr::Ternary { .. }
            | Expr::Assign { .. }
            | Expr::Lambda { .. }
            | Expr::With { .. }
            | Expr::Fail(_)
            | Expr::Reify(_)
            | Expr::Await(_)
            | Expr::Spawn(_)
            | Expr::Range { .. }
    )
}

fn operand(s: &mut String, e: &Expr) {
    if compound(e) {
        s.push('(');
        expr(s, e);
        s.push(')');
    } else {
        expr(s, e);
    }
}

fn expr(s: &mut String, e: &Expr) {
    match e {
        Expr::Int(n) => {
            let _ = write!(s, "{n}");
        }
        Expr::Float(f) => {
            if f.fract() == 0.0 && f.is_finite() {
                let _ = write!(s, "{f:.1}");
            } else {
                let _ = write!(s, "{f}");
            }
        }
        Expr::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Expr::Unit => s.push_str("()"),
        Expr::Str(parts) => string(s, parts),
        Expr::Path(p) => s.push_str(p),
        Expr::Ident(n) => s.push_str(n),
        Expr::List(es) => {
            s.push('[');
            for (i, x) in es.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                expr(s, x);
            }
            s.push(']');
        }
        Expr::Record(fields) => record(s, None, fields),
        Expr::Ctor { name, fields } => record(s, Some(name), fields),
        Expr::Call { callee, args } => {
            operand(s, callee);
            s.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                arg(s, a);
            }
            s.push(')');
        }
        Expr::Field { base, name } => {
            operand(s, base);
            let _ = write!(s, ".{name}");
        }
        Expr::Index { base, index } => {
            operand(s, base);
            s.push('[');
            expr(s, index);
            s.push(']');
        }
        Expr::Range { lo, hi } => {
            operand(s, lo);
            s.push_str("..");
            operand(s, hi);
        }
        Expr::Unary { op, expr: e } => {
            s.push_str(match op {
                UnOp::Neg => "-",
                UnOp::Not => "!",
            });
            operand(s, e);
        }
        Expr::Binary { op, lhs, rhs } => {
            operand(s, lhs);
            let _ = write!(s, " {} ", bin(*op));
            operand(s, rhs);
        }
        Expr::Ternary { cond, then, els } => {
            operand(s, cond);
            s.push_str(" ? ");
            operand(s, then);
            s.push_str(" : ");
            operand(s, els);
        }
        Expr::If { cond, then, els } => {
            s.push_str("if ");
            expr(s, cond);
            s.push(' ');
            block(s, then);
            if let Some(e) = els {
                s.push_str(" else ");
                block(s, e);
            }
        }
        Expr::Match { scrut, arms } => {
            s.push_str("match ");
            expr(s, scrut);
            s.push_str(" {\n");
            for a in arms {
                pattern(s, &a.pat);
                if let Some(g) = &a.guard {
                    s.push_str(" if ");
                    expr(s, g);
                }
                s.push_str(" => ");
                expr(s, &a.body);
                s.push('\n');
            }
            s.push('}');
        }
        Expr::For { pat, iter, body } => {
            s.push_str("for ");
            pattern(s, pat);
            s.push_str(" in ");
            expr(s, iter);
            s.push(' ');
            block(s, body);
        }
        Expr::While { cond, body } => {
            s.push_str("while ");
            expr(s, cond);
            s.push(' ');
            block(s, body);
        }
        Expr::Break => s.push_str("break"),
        Expr::Continue => s.push_str("continue"),
        Expr::Return(v) => {
            s.push_str("return");
            if let Some(x) = v {
                s.push(' ');
                expr(s, x);
            }
        }
        Expr::Lambda { params, ret, body } => {
            if params.len() == 1 && !params[0].variadic && params[0].ty.is_none() && ret.is_none() {
                s.push_str(&params[0].name);
            } else {
                s.push('(');
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&p.name);
                    if let Some(t) = &p.ty {
                        let _ = write!(s, ": {}", ty(t));
                    }
                }
                s.push(')');
            }
            if let Some(t) = ret {
                let _ = write!(s, " -> {}", ty(t));
            }
            s.push_str(" => ");
            if matches!(&**body, Expr::Record(_)) {
                s.push('(');
                expr(s, body);
                s.push(')');
            } else {
                expr(s, body);
            }
        }
        Expr::With { base, fields } => {
            operand(s, base);
            s.push_str(" with ");
            record(s, None, fields);
        }
        Expr::Try(e) => {
            operand(s, e);
            s.push('?');
        }
        Expr::Fail(e) => {
            s.push_str("fail ");
            operand(s, e);
        }
        Expr::Reify(e) => {
            s.push_str("try ");
            operand(s, e);
        }
        Expr::Await(e) => {
            s.push_str("await ");
            operand(s, e);
        }
        Expr::Spawn(e) => {
            s.push_str("spawn ");
            operand(s, e);
        }
        Expr::Assign { target, value } => {
            expr(s, target);
            s.push_str(" = ");
            expr(s, value);
        }
        Expr::Block(stmts) => block(s, stmts),
    }
}

fn record(s: &mut String, name: Option<&str>, fields: &[Field]) {
    if let Some(n) = name {
        s.push_str(n);
        s.push(' ');
    }
    if fields.is_empty() {
        s.push_str("{}");
        return;
    }
    s.push_str("{ ");
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        match f {
            Field::Value { name, value } => {
                let _ = write!(s, "{name} = ");
                expr(s, value);
            }
            Field::Type { name, ty: t } => {
                let _ = write!(s, "{name}: {}", ty(t));
            }
            Field::Shorthand(n) => s.push_str(n),
            Field::Bare(e) => expr(s, e),
        }
    }
    s.push_str(" }");
}

fn arg(s: &mut String, a: &Arg) {
    match a {
        Arg::Pos(e) => expr(s, e),
        Arg::Named { name, value } => {
            let _ = write!(s, "{name} = ");
            expr(s, value);
        }
        Arg::Directive { kind, prop, value } => {
            let k = match kind {
                Dir::Bind => "bind",
                Dir::On => "on",
            };
            let _ = write!(s, "{k}:{prop} = ");
            expr(s, value);
        }
    }
}

fn string(s: &mut String, parts: &[StrPart]) {
    s.push('"');
    for p in parts {
        match p {
            StrPart::Text(t) => s.push_str(&escape(t)),
            StrPart::Interp(e) => {
                s.push('{');
                expr(s, e);
                s.push('}');
            }
        }
    }
    s.push('"');
}

fn escape(t: &str) -> String {
    let mut out = String::new();
    for c in t.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            _ => out.push(c),
        }
    }
    out
}

fn pattern(s: &mut String, p: &Pattern) {
    match p {
        Pattern::Wild => s.push('_'),
        Pattern::Int(n) => {
            let _ = write!(s, "{n}");
        }
        Pattern::Float(f) => {
            if f.fract() == 0.0 && f.is_finite() {
                let _ = write!(s, "{f:.1}");
            } else {
                let _ = write!(s, "{f}");
            }
        }
        Pattern::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Pattern::Str(t) => {
            let _ = write!(s, "\"{}\"", escape(t));
        }
        Pattern::Bind(n) => s.push_str(n),
        Pattern::Ctor { name, args } => {
            s.push_str(name);
            s.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                pattern(s, a);
            }
            s.push(')');
        }
        Pattern::Record(fields) => {
            s.push_str("{ ");
            for (i, (name, sub)) in fields.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(name);
                if let Some(sp) = sub {
                    s.push_str(": ");
                    pattern(s, sp);
                }
            }
            s.push_str(" }");
        }
        Pattern::List { elems, rest } => {
            s.push('[');
            for (i, e) in elems.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                pattern(s, e);
            }
            if let Some(r) = rest {
                if !elems.is_empty() {
                    s.push_str(", ");
                }
                s.push_str("..");
                pattern(s, r);
            }
            s.push(']');
        }
        Pattern::Or(alts) => {
            for (i, a) in alts.iter().enumerate() {
                if i > 0 {
                    s.push_str(" | ");
                }
                pattern(s, a);
            }
        }
    }
}

fn bin(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::Concat => "++",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::Union => "|",
        BinOp::Pipe => "|>",
    }
}
