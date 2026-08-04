use maca_parser::ast::*;
use std::collections::BTreeSet;

pub struct LlvmOut {
    pub ir: String,
    /// Names of the functions defined here (the C backend declares them extern).
    pub simd_fns: Vec<String>,
}

pub fn emit(m: &Module) -> LlvmOut {
    let mut simd_fns = Vec::new();
    let mut decls = BTreeSet::new();
    let mut defs = String::new();
    for item in &m.items {
        if let Stmt::Fn(f) = item
            && is_simd_fn(f)
            && let Some(def) = emit_fn(f, &mut decls)
        {
            simd_fns.push(f.name.clone());
            defs.push_str(&def);
        }
    }
    let mut ir = String::new();
    for d in &decls {
        ir.push_str(d);
        ir.push('\n');
    }
    if !decls.is_empty() {
        ir.push('\n');
    }
    ir.push_str(&defs);
    LlvmOut { ir, simd_fns }
}

/// A function is on the LLVM path iff any parameter or its return is a vector.
pub fn is_simd_fn(f: &FnDef) -> bool {
    f.params
        .iter()
        .any(|p| p.ty.as_ref().is_some_and(is_vec_type))
        || f.ret.as_ref().is_some_and(is_vec_type)
}

fn is_vec_type(t: &Type) -> bool {
    matches!(t, Type::Name(segs) if segs.len() == 1 && parse_vec(&segs[0]).is_some())
}

/// `f32x8` → (llvm scalar, lanes, mangled scalar, is_float).
fn parse_vec(name: &str) -> Option<(String, usize, String, bool)> {
    let (elem, lanes) = name.split_once('x')?;
    let lanes: usize = lanes.parse().ok()?;
    let (llvm, mang, isf) = match elem {
        "f32" => ("float", "f32", true),
        "f64" => ("double", "f64", true),
        "i8" | "u8" => ("i8", "i8", false),
        "i16" | "u16" => ("i16", "i16", false),
        "i32" | "u32" => ("i32", "i32", false),
        "i64" | "u64" => ("i64", "i64", false),
        _ => return None,
    };
    Some((llvm.into(), lanes, mang.into(), isf))
}

fn llvm_type(t: &Type) -> String {
    if let Type::Name(segs) = t
        && segs.len() == 1
    {
        if let Some((scal, lanes, _, _)) = parse_vec(&segs[0]) {
            return format!("<{lanes} x {scal}>");
        }
        return match segs[0].as_str() {
            "f32" => "float".into(),
            "f64" | "float" => "double".into(),
            "int" | "i64" => "i64".into(),
            "i32" => "i32".into(),
            "bool" => "i1".into(),
            _ => "i64".into(),
        };
    }
    "i64".into()
}

fn emit_fn(f: &FnDef, decls: &mut BTreeSet<String>) -> Option<String> {
    f.body.as_ref()?;
    let ret = f
        .ret
        .as_ref()
        .map(llvm_type)
        .unwrap_or_else(|| "void".into());
    let params: Vec<(String, String)> = f
        .params
        .iter()
        .map(|p| {
            (
                format!("%{}", p.name),
                p.ty.as_ref().map(llvm_type).unwrap_or_else(|| "i64".into()),
            )
        })
        .collect();
    let sig = params
        .iter()
        .map(|(n, t)| format!("{t} {n}"))
        .collect::<Vec<_>>()
        .join(", ");

    let mut cx = Ctx {
        out: String::new(),
        n: 0,
        decls,
        env: Vec::new(),
    };
    for (n, t) in &params {
        cx.env.push((
            n.trim_start_matches('%').to_string(),
            (n.clone(), t.clone()),
        ));
    }
    let (val, _) = match &f.body {
        Some(FnBody::Expr(e)) => cx.expr(e)?,
        Some(FnBody::Block(stmts)) => cx.block(stmts)?,
        None => return None,
    };
    let mut out = format!("define {ret} @{}({sig}) {{\nentry:\n", f.name);
    out.push_str(&cx.out);
    out.push_str(&format!("  ret {ret} {val}\n}}\n\n"));
    Some(out)
}

struct Ctx<'a> {
    out: String,
    n: u32,
    decls: &'a mut BTreeSet<String>,
    /// flat scope (SIMD kernels are small): name → (ssa value, llvm type).
    env: Vec<(String, (String, String))>,
}

impl<'a> Ctx<'a> {
    fn tmp(&mut self) -> String {
        self.n += 1;
        format!("%v{}", self.n)
    }

    /// A block body: each `x = e` binds an SSA local, and the block's value is its last expression.
    fn block(&mut self, stmts: &[Stmt]) -> Option<(String, String)> {
        let mut last = None;
        for s in stmts {
            match s {
                Stmt::Bind(b) => {
                    let Expr::Ident(n) = &b.target else {
                        return None;
                    };
                    let v = self.expr(&b.value)?;
                    self.env.push((n.clone(), v));
                }
                Stmt::Expr(e) => last = Some(self.expr(e)?),
                _ => return None,
            }
        }
        last
    }

    fn expr(&mut self, e: &Expr) -> Option<(String, String)> {
        match e {
            Expr::Ident(n) => self
                .env
                .iter()
                .rev()
                .find(|(k, _)| k == n)
                .map(|(_, v)| v.clone()),
            Expr::Binary { op, lhs, rhs } => {
                let (lv, lt) = self.expr(lhs)?;
                let (rv, _) = self.expr(rhs)?;
                let is_f = lt.contains("float") || lt.contains("double");
                let inst = match op {
                    BinOp::Mul => {
                        if is_f {
                            "fmul"
                        } else {
                            "mul"
                        }
                    }
                    BinOp::Add => {
                        if is_f {
                            "fadd"
                        } else {
                            "add"
                        }
                    }
                    BinOp::Sub => {
                        if is_f {
                            "fsub"
                        } else {
                            "sub"
                        }
                    }
                    BinOp::Div => {
                        if is_f {
                            "fdiv"
                        } else {
                            "sdiv"
                        }
                    }
                    _ => return None,
                };
                let t = self.tmp();
                self.out
                    .push_str(&format!("  {t} = {inst} {lt} {lv}, {rv}\n"));
                Some((t, lt))
            }
            Expr::Call { callee, args } if args.is_empty() => {
                let Expr::Field { base, name } = callee.as_ref() else {
                    return None;
                };
                let (rv, rt) = self.expr(base)?;
                let (lanes, scal, mang, is_f) = parse_vec_llvm(&rt)?;
                let (op, needs_start) = match name.as_str() {
                    "sum" if is_f => ("fadd", true),
                    "sum" => ("add", false),
                    "max" if is_f => ("fmax", false),
                    "max" => ("smax", false),
                    _ => return None,
                };
                let intr = format!("llvm.vector.reduce.{op}.v{lanes}{mang}");
                let t = self.tmp();
                if needs_start {
                    self.decls
                        .insert(format!("declare {scal} @{intr}({scal}, {rt})"));
                    self.out.push_str(&format!(
                        "  {t} = call {scal} @{intr}({scal} 0.000000e+00, {rt} {rv})\n"
                    ));
                } else {
                    self.decls.insert(format!("declare {scal} @{intr}({rt})"));
                    self.out
                        .push_str(&format!("  {t} = call {scal} @{intr}({rt} {rv})\n"));
                }
                Some((t, scal))
            }
            _ => None,
        }
    }
}

/// Parse an LLVM vector type string `<8 x float>` → (lanes, scalar, mangle, is_float).
fn parse_vec_llvm(t: &str) -> Option<(usize, String, String, bool)> {
    let inner = t.trim().strip_prefix('<')?.strip_suffix('>')?;
    let (lanes, scal) = inner.split_once(" x ")?;
    let lanes: usize = lanes.trim().parse().ok()?;
    let scal = scal.trim();
    let (mang, isf) = match scal {
        "float" => ("f32", true),
        "double" => ("f64", true),
        "i8" => ("i8", false),
        "i16" => ("i16", false),
        "i32" => ("i32", false),
        "i64" => ("i64", false),
        _ => return None,
    };
    Some((lanes, scal.into(), mang.into(), isf))
}
