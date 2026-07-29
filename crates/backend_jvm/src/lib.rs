//! maca-backend-jvm: lower Maca to Java source (JVM interop).
//!
//! Java is the substrate for JVM ecosystems — notably Minecraft (Fabric/Forge)
//! modding. This backend transpiles Maca to a `.java` file that `javac`
//! compiles, so Maca can call Java APIs and implement Java interfaces directly.
//!
//! Mapping:
//!   * top-level functions → `static` methods
//!   * records → Java `record`s; sums (nullary) → `enum`s
//!   * `Name : Iface = { m() {…} }` → `class Name implements Iface { … }`
//!     (this is how you write a Fabric `ModInitializer`)
//!   * `import java "pkg.Class"` → `import pkg.Class;`
//!   * a capitalized call `Pos(1,2,3)` → `new Pos(1, 2, 3)`; `obj.m(a)` →
//!     `obj.m(a)`; `Blocks.STONE` → `Blocks.STONE`
//!
//! Type/expression coverage is the functional core plus interop pass-through;
//! unknown types map to their name verbatim (a Java type).

use maca_parser::ast::*;
use std::collections::BTreeSet;

/// Emit a Java compilation unit named `class_name` (usually the file stem, or
/// overridden by `package`/entry conventions). `package` is optional.
pub fn emit(m: &Module, class_name: &str, package: Option<&str>) -> String {
    let mut cx = Cx::default();
    cx.collect(m);
    let body = cx.emit_members(m, class_name);

    let mut out = String::new();
    if let Some(p) = package {
        out.push_str(&format!("package {p};\n\n"));
    }
    for imp in &cx.imports {
        out.push_str(&format!("import {imp};\n"));
    }
    if !cx.imports.is_empty() {
        out.push('\n');
    }
    out.push_str(&body);
    out
}

#[derive(Default)]
struct Cx {
    imports: BTreeSet<String>,
    sums: BTreeSet<String>,
    records: BTreeSet<String>,
}

impl Cx {
    fn collect(&mut self, m: &Module) {
        for it in &m.items {
            match it {
                Stmt::Import(Import::Foreign { lang, spec }) if lang == "java" => {
                    self.imports.insert(spec.clone());
                }
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        if sum_variants(&b.value).is_some() {
                            self.sums.insert(name.clone());
                        } else if is_record_type(&b.value) {
                            self.records.insert(name.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn emit_members(&mut self, m: &Module, class_name: &str) -> String {
        let mut types = String::new(); // top-level enums/records
        let mut members = String::new(); // wrapper-class methods + nested classes

        for it in &m.items {
            match it {
                Stmt::Bind(b) => {
                    if let Expr::Ident(name) = &b.target {
                        if let Some(vars) = sum_variants(&b.value) {
                            types.push_str(&format!("enum {name} {{ {} }}\n\n", vars.join(", ")));
                        } else if is_record_type(&b.value) {
                            types.push_str(&self.emit_record(name, &b.value));
                        } else if let (Some(ty), Some(ms)) =
                            (b.tys.first(), lambda_fields(&b.value))
                        {
                            // `Name : Iface = { m = () => … }` → a nested static
                            // class so it can call the wrapper's static helpers.
                            members.push_str(&self.emit_class(name, ty, &ms));
                        }
                    }
                }
                Stmt::Fn(f) if f.name == "main" => members.push_str(&self.emit_main(f)),
                Stmt::Fn(f) => members.push_str(&self.emit_method(f, "static ")),
                _ => {}
            }
        }

        let mut out = types;
        out.push_str(&format!("public final class {class_name} {{\n"));
        out.push_str(&indent(&members, 1));
        out.push_str("}\n");
        out
    }

    fn emit_record(&self, name: &str, value: &Expr) -> String {
        let Expr::Record(fields) = value else {
            return String::new();
        };
        let ps: Vec<String> = fields
            .iter()
            .filter_map(|f| match f {
                Field::Type { name, ty } => Some(format!("{} {name}", jtype(ty))),
                _ => None,
            })
            .collect();
        format!("record {name}({}) {{}}\n\n", ps.join(", "))
    }

    fn emit_class(
        &mut self,
        name: &str,
        iface: &Type,
        methods: &[(String, Vec<Param>, Expr)],
    ) -> String {
        let mut ms = String::new();
        for (mname, params, body) in methods {
            let ps: Vec<String> = params
                .iter()
                .map(|p| {
                    format!(
                        "{} {}",
                        p.ty.as_ref().map(jtype).unwrap_or_else(|| "Object".into()),
                        p.name
                    )
                })
                .collect();
            // interface method return types are unknown here → void; a call/expr
            // body is emitted as a statement.
            ms.push_str(&format!(
                "public void {mname}({}) {{\n    {};\n}}\n\n",
                ps.join(", "),
                jexpr(body)
            ));
        }
        format!(
            "public static class {name} implements {} {{\n{}}}\n\n",
            type_name(iface),
            indent(&ms, 1)
        )
    }

    fn emit_method(&self, f: &FnDef, modifiers: &str) -> String {
        let ret = f.ret.as_ref().map(jtype).unwrap_or_else(|| "void".into());
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| {
                format!(
                    "{} {}",
                    p.ty.as_ref().map(jtype).unwrap_or_else(|| "Object".into()),
                    p.name
                )
            })
            .collect();
        let body = match &f.body {
            Some(FnBody::Expr(e)) => {
                if ret == "void" {
                    format!("    {};\n", jexpr(e))
                } else {
                    format!("    return {};\n", jexpr(e))
                }
            }
            Some(FnBody::Block(stmts)) => jblock(stmts, ret != "void"),
            None => String::new(),
        };
        format!(
            "{modifiers}{ret} {}({}) {{\n{body}}}\n\n",
            f.name,
            params.join(", ")
        )
    }

    fn emit_main(&self, f: &FnDef) -> String {
        let body = match &f.body {
            Some(FnBody::Block(stmts)) => jblock(stmts, false),
            Some(FnBody::Expr(e)) => format!("    {};\n", jexpr(e)),
            None => String::new(),
        };
        format!("public static void main(String[] args) {{\n{body}}}\n\n")
    }
}

// ---- statements -----------------------------------------------------------

/// A block of statements → Java. `wants_value` returns the final expression.
fn jblock(stmts: &[Stmt], wants_value: bool) -> String {
    let mut out = String::new();
    for (i, s) in stmts.iter().enumerate() {
        let last = i + 1 == stmts.len();
        match s {
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target {
                    let ty = b.tys.first().map(jtype).unwrap_or_else(|| "var".into());
                    out.push_str(&format!("    {ty} {n} = {};\n", jexpr(&b.value)));
                } else {
                    out.push_str(&format!("    {};\n", jexpr(&b.value)));
                }
            }
            Stmt::Expr(e) => {
                if let Expr::For { pat, iter, body } = e {
                    out.push_str(&jfor(pat, iter, body));
                } else if let Expr::If { .. } = e {
                    out.push_str(&jif_stmt(e));
                } else if last && wants_value {
                    out.push_str(&format!("    return {};\n", jexpr(e)));
                } else if !is_pure_value(e) {
                    // a bare value in statement position is a no-op (e.g. the
                    // trailing `0` of a Maca `main`); Java rejects it, so skip.
                    out.push_str(&format!("    {};\n", jexpr(e)));
                }
            }
            _ => {}
        }
    }
    out
}

fn jfor(pat: &Pattern, iter: &Expr, body: &[Stmt]) -> String {
    let var = match pat {
        Pattern::Bind(n) => n.clone(),
        _ => "_it".into(),
    };
    format!(
        "    for (var {var} : {}) {{\n{}    }}\n",
        jexpr(iter),
        indent(&jblock(body, false), 1)
    )
}

fn jif_stmt(e: &Expr) -> String {
    let Expr::If { cond, then, els } = e else {
        return String::new();
    };
    let mut out = format!(
        "    if ({}) {{\n{}    }}",
        jexpr(cond),
        indent(&jblock(then, false), 1)
    );
    if let Some(e) = els {
        out.push_str(&format!(" else {{\n{}    }}", indent(&jblock(e, false), 1)));
    }
    out.push('\n');
    out
}

// ---- expressions ----------------------------------------------------------

fn jexpr(e: &Expr) -> String {
    match e {
        Expr::Int(n) => format!("{n}L"),
        Expr::Float(f) => format!("{f}"),
        Expr::Bool(b) => b.to_string(),
        Expr::Unit => "null".into(),
        Expr::Str(parts) => jstr(parts),
        Expr::Path(p) => format!("{p:?}"),
        Expr::Ident(n) => n.clone(),
        Expr::Unary { op, expr } => {
            let o = if matches!(op, UnOp::Not) { "!" } else { "-" };
            format!("{o}({})", jexpr(expr))
        }
        Expr::Binary { op, lhs, rhs } => jbinary(*op, lhs, rhs),
        Expr::Ternary { cond, then, els } => {
            format!("({} ? {} : {})", jexpr(cond), jexpr(then), jexpr(els))
        }
        Expr::If { cond, then, els } => {
            // if-expression → ternary (branch is the last expr of each block)
            let t = block_value(then);
            let e = els
                .as_ref()
                .map(|s| block_value(s))
                .unwrap_or_else(|| "null".into());
            format!("({} ? {} : {})", jexpr(cond), t, e)
        }
        Expr::Call { callee, args } => jcall(callee, args),
        Expr::Field { base, name } => format!("{}.{name}", jexpr(base)),
        // List.get needs an int index; Maca ints are Java longs, so narrow it
        Expr::Index { base, index } => format!("{}.get((int)({}))", jexpr(base), jexpr(index)),
        Expr::Record(fields) | Expr::Ctor { fields, .. } => jctor(e, fields),
        Expr::List(es) => {
            format!(
                "java.util.List.of({})",
                es.iter().map(jexpr).collect::<Vec<_>>().join(", ")
            )
        }
        Expr::Match { scrut, arms } => jmatch(scrut, arms),
        Expr::Block(stmts) => block_value(stmts),
        Expr::Try(x) => jexpr(x),
        _ => "null".into(),
    }
}

/// The value of a block used in expression position (its last expression).
fn block_value(stmts: &[Stmt]) -> String {
    match stmts.last() {
        Some(Stmt::Expr(e)) => jexpr(e),
        _ => "null".into(),
    }
}

fn jbinary(op: BinOp, lhs: &Expr, rhs: &Expr) -> String {
    use BinOp::*;
    let (l, r) = (jexpr(lhs), jexpr(rhs));
    let o = match op {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Mod => "%",
        Shl => "<<",
        Shr => ">>",
        Eq => "==",
        Ne => "!=",
        Lt => "<",
        Gt => ">",
        Le => "<=",
        Ge => ">=",
        And => "&&",
        Or => "||",
        // string / value concat and everything else
        Concat => return format!("({l} + {r})"),
        Union | Pipe => return l,
    };
    // `==`/`!=` on strings → .equals (best effort: only when a side is a literal)
    if matches!(op, Eq | Ne) && (is_str(lhs) || is_str(rhs)) {
        let base = format!("java.util.Objects.equals({l}, {r})");
        return if op == Ne { format!("!{base}") } else { base };
    }
    format!("({l} {o} {r})")
}

fn jcall(callee: &Expr, args: &[Arg]) -> String {
    let a: Vec<String> = args.iter().map(|x| jexpr(arg_expr(x))).collect();
    match callee {
        Expr::Ident(f) if f == "info" || f == "print" => {
            format!(
                "System.out.println({})",
                a.first().cloned().unwrap_or_default()
            )
        }
        Expr::Ident(f) if f == "int" => {
            format!("(long)({})", a.first().cloned().unwrap_or_default())
        }
        Expr::Ident(f) if f == "float" => {
            format!("(double)({})", a.first().cloned().unwrap_or_default())
        }
        Expr::Ident(f) if f == "str" => {
            format!("String.valueOf({})", a.first().cloned().unwrap_or_default())
        }
        Expr::Ident(f) if f == "len" => {
            format!("({}).size()", a.first().cloned().unwrap_or_default())
        }
        // Capitalized target → constructor (`Pos(1,2)` → `new Pos(1, 2)`)
        Expr::Ident(f) if f.chars().next().is_some_and(|c| c.is_ascii_uppercase()) => {
            format!("new {f}({})", a.join(", "))
        }
        Expr::Ident(f) => format!("{f}({})", a.join(", ")),
        Expr::Field { base, name } => format!("{}.{name}({})", jexpr(base), a.join(", ")),
        _ => format!("{}({})", jexpr(callee), a.join(", ")),
    }
}

fn jctor(e: &Expr, fields: &[Field]) -> String {
    let name = match e {
        Expr::Ctor { name, .. } => name.clone(),
        _ => String::new(),
    };
    let vals: Vec<String> = fields
        .iter()
        .filter_map(|f| match f {
            Field::Value { value, .. } => Some(jexpr(value)),
            Field::Shorthand(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    if name.is_empty() {
        // structural record literal — no Java type; fall back to a comment
        "/* record */ null".to_string()
    } else {
        format!("new {name}({})", vals.join(", "))
    }
}

fn jmatch(scrut: &Expr, arms: &[Arm]) -> String {
    // enum/value match → a Java switch expression
    let mut out = format!("switch ({}) {{\n", jexpr(scrut));
    for a in arms {
        let label = match &a.pat {
            Pattern::Wild => "default".to_string(),
            Pattern::Bind(n) => format!("case {n}"),
            Pattern::Ctor { name, .. } => format!("case {name}"),
            Pattern::Int(n) => format!("case {n}"),
            _ => "default".to_string(),
        };
        out.push_str(&format!("        {label} -> {};\n", jexpr(&a.body)));
    }
    out.push_str("    }");
    out
}

/// String with interpolation → Java string concatenation.
fn jstr(parts: &[StrPart]) -> String {
    if parts.is_empty() {
        return "\"\"".into();
    }
    let mut pieces = Vec::new();
    let mut all_text = true;
    for p in parts {
        match p {
            StrPart::Text(t) => pieces.push(java_str_lit(t)),
            StrPart::Interp(e) => {
                all_text = false;
                pieces.push(format!("String.valueOf({})", jexpr(e)));
            }
        }
    }
    if all_text {
        pieces.join(" + ")
    } else {
        // ensure the expression starts as a String
        format!("(\"\" + {})", pieces.join(" + "))
    }
}

// ---- types & helpers ------------------------------------------------------

fn jtype(t: &Type) -> String {
    match t {
        Type::Name(segs) => {
            let n = segs.join(".");
            match n.as_str() {
                "int" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => {
                    "long".into()
                }
                "f32" => "float".into(),
                "float" | "f64" => "double".into(),
                "str" | "bytes" => "String".into(),
                "bool" => "boolean".into(),
                "unit" | "()" => "void".into(),
                // lowercase single-segment = type variable → Object
                _ if segs.len() == 1 && is_type_var_name(&n) => "Object".into(),
                _ => n, // nominal / Java interop type, verbatim
            }
        }
        Type::Array(inner) => format!("java.util.List<{}>", boxed(inner)),
        Type::Opt(inner) => boxed(inner),
        Type::Paren(inner) => jtype(inner),
        Type::Apply(h, args) => {
            let head = type_name(h);
            let a: Vec<String> = args.iter().map(boxed).collect();
            format!("{head}<{}>", a.join(", "))
        }
    }
}

/// A boxed (reference) type, for generic arguments.
fn boxed(t: &Type) -> String {
    match jtype(t).as_str() {
        "long" => "Long".into(),
        "double" => "Double".into(),
        "float" => "Float".into(),
        "boolean" => "Boolean".into(),
        other => other.to_string(),
    }
}

fn type_name(t: &Type) -> String {
    match t {
        Type::Name(segs) => segs.join("."),
        Type::Apply(h, _) => type_name(h),
        Type::Paren(t) => type_name(t),
        _ => jtype(t),
    }
}

fn is_str(e: &Expr) -> bool {
    matches!(e, Expr::Str(_))
}

/// A side-effect-free value expression — useless (and illegal) as a Java
/// statement, so it is dropped when it appears in statement position.
fn is_pure_value(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Ident(_) | Expr::Unit
    )
}

fn java_str_lit(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn indent(s: &str, levels: usize) -> String {
    let pad = "    ".repeat(levels);
    s.lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("{pad}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if s.ends_with('\n') { "\n" } else { "" }
}

/// A record whose every field is `name = (params) => body` → interface methods.
fn lambda_fields(e: &Expr) -> Option<Vec<(String, Vec<Param>, Expr)>> {
    let Expr::Record(fields) = e else { return None };
    let mut out = Vec::new();
    for f in fields {
        if let Field::Value { name, value } = f
            && let Expr::Lambda { params, body, .. } = value
        {
            out.push((name.clone(), params.clone(), (**body).clone()));
            continue;
        }
        return None;
    }
    (!out.is_empty()).then_some(out)
}

fn sum_variants(e: &Expr) -> Option<Vec<String>> {
    fn go(e: &Expr, out: &mut Vec<String>) -> bool {
        match e {
            Expr::Ident(n) => {
                out.push(n.clone());
                true
            }
            Expr::Binary {
                op: BinOp::Union,
                lhs,
                rhs,
            } => go(lhs, out) && go(rhs, out),
            _ => false,
        }
    }
    let mut out = Vec::new();
    match e {
        Expr::Binary {
            op: BinOp::Union, ..
        } if go(e, &mut out) => Some(out),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn java(src: &str) -> String {
        let p = maca_parser::parse(src);
        assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
        emit(&p.module, "Main", None)
    }

    #[test]
    fn functions_and_main() {
        let j = java(
            "fib(n: int) -> int =>\n    n < 2 ? n : fib(n - 1) + fib(n - 2)\n\nmain() -> int {\n    info(\"{fib(10)}\")\n    0\n}\n",
        );
        assert!(j.contains("static long fib(long n)"), "{j}");
        assert!(j.contains("public static void main(String[] args)"), "{j}");
        assert!(j.contains("System.out.println"), "{j}");
    }

    #[test]
    fn record_and_enum() {
        let j = java("Status = Todo | Doing | Done\nVec2 = {\n    x: int\n    y: int\n}\n");
        assert!(j.contains("enum Status { Todo, Doing, Done }"), "{j}");
        assert!(j.contains("record Vec2(long x, long y)"), "{j}");
    }

    #[test]
    fn index_narrows_to_int() {
        // Java List.get takes an int; Maca ints are longs, so the subscript
        // must narrow the index or javac rejects it.
        let j = java("f(xs: int[]) -> int => xs[1]\n");
        assert!(j.contains(".get((int)("), "index not narrowed to int:\n{j}");
    }

    #[test]
    fn interface_impl_and_interop() {
        let j = java(
            "import java \"net.fabricmc.api.ModInitializer\"\n\nExampleMod : ModInitializer = {\n    onInitialize = () => info(\"Maca mod loaded\")\n}\n",
        );
        assert!(j.contains("import net.fabricmc.api.ModInitializer;"), "{j}");
        assert!(
            j.contains("class ExampleMod implements ModInitializer"),
            "{j}"
        );
        assert!(j.contains("public void onInitialize()"), "{j}");
        assert!(j.contains("System.out.println"), "{j}");
    }
}
