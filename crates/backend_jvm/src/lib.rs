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
use std::collections::{BTreeMap, BTreeSet};

/// Emit a Java compilation unit named `class_name` (usually the file stem, or
/// overridden by `package`/entry conventions). `package` is optional.
/// Emit Java, or the list of constructs this backend does not lower. The driver
/// uses this so an unsupported construct is a clean error naming what the author
/// wrote, rather than a `null` javac accepts anywhere a reference is wanted.
pub fn emit_checked(
    m: &Module,
    class_name: &str,
    package: Option<&str>,
) -> Result<String, Vec<String>> {
    PROBLEMS.with(|p| p.borrow_mut().clear());
    let out = emit(m, class_name, package);
    let problems = PROBLEMS.with(|p| p.borrow().clone());
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

pub fn emit(m: &Module, class_name: &str, package: Option<&str>) -> String {
    VARIANT_OF.with(|v| v.borrow_mut().clear());
    RECORD_FIELDS.with(|m| m.borrow_mut().clear());
    RECORD_FIELD_NAMES.with(|m| m.borrow_mut().clear());
    let fnps = fn_params(&m.items);
    FN_PARAMS.with(|f| *f.borrow_mut() = fnps.clone());
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
    // One declaration per distinct shape actually used, above the class that
    // names it.
    let mut ifaces: Vec<String> = fnps
        .values()
        .filter(|s| s.arity <= 2)
        .map(|s| s.decl())
        .collect();
    ifaces.sort();
    ifaces.dedup();
    for d in ifaces {
        out.push_str(&d);
    }
    if !fnps.is_empty() {
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
                        if let Some(vars) = sum_variants(&b.value) {
                            note_variants(name, &vars);
                            self.sums.insert(name.clone());
                        } else if is_record_type(&b.value) {
                            self.records.insert(name.clone());
                            if let Expr::Record(fs) = &b.value {
                                let order: Vec<String> = fs
                                    .iter()
                                    .filter_map(|f| match f {
                                        Field::Type { name, .. } => Some(name.clone()),
                                        _ => None,
                                    })
                                    .collect();
                                note_record(name, &order);
                            }
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
        start_scope(&f.params);
        start_fn_scope(&f.name, &f.params);
        let ret = f.ret.as_ref().map(jtype).unwrap_or_else(|| "void".into());
        let params: Vec<String> = f
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| format!("{} {}", param_type(&f.name, i, p), p.name))
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
        start_scope(&f.params);
        start_fn_scope(&f.name, &f.params);
        let body = match &f.body {
            Some(FnBody::Block(stmts)) => jblock(stmts, false),
            // The trailing `0` of `main() -> int => 0` is the exit code, not a
            // statement, and Java rejects a bare value as one.
            Some(FnBody::Expr(e)) if is_pure_value(e) => String::new(),
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
                    let fresh = DECLARED.with(|d| d.borrow_mut().insert(n.clone()));
                    let ty = if fresh {
                        b.tys.first().map(jtype).unwrap_or_else(|| "var".into()) + " "
                    } else {
                        String::new()
                    };
                    out.push_str(&format!("    {ty}{n} = {};\n", jexpr(&b.value)));
                } else {
                    out.push_str(&format!(
                        "    {} = {};\n",
                        jexpr(&b.target),
                        jexpr(&b.value)
                    ));
                }
            }
            Stmt::Expr(e) => {
                if let Expr::For { pat, iter, body } = e {
                    out.push_str(&jfor(pat, iter, body));
                } else if let Expr::While { cond, body } = e {
                    out.push_str(&format!(
                        "    while ({}) {{\n{}    }}\n",
                        jexpr(cond),
                        indent(&jblock(body, false), 1)
                    ));
                } else if let Expr::If { .. } = e {
                    out.push_str(&jif_stmt(e));
                } else if last && wants_value {
                    out.push_str(&format!("    return {};\n", jexpr(e)));
                } else if matches!(e, Expr::Break | Expr::Continue) {
                    out.push_str(&format!("    {};\n", jexpr(e)));
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
        Expr::Ident(n) => qualify(n),
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
        // A Java record keeps its components private and exposes an accessor of
        // the same name, so reading one is `p.x()`. A field of a foreign Java
        // object is a real field and stays `o.x`.
        Expr::Field { base, name } if RECORD_FIELD_NAMES.with(|m| m.borrow().contains(name)) => {
            format!("{}.{name}()", jexpr(base))
        }
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
        // A Java lambda. This is the target's headline use case — a Fabric mod
        // registers callbacks — and it used to become `null`, which is
        // assignable to any functional interface, so it compiled and the
        // callback did nothing.
        Expr::Lambda { params, body, .. } => {
            let ps: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
            format!("({}) -> {}", ps.join(", "), jexpr(body))
        }
        Expr::Assign { target, value } => format!("({} = {})", jexpr(target), jexpr(value)),
        Expr::Break => "break".into(),
        Expr::Continue => "continue".into(),
        // `null` is assignable to every Java reference type, so an unlowered
        // construct type-checked and the program ran with a hole in it.
        other => {
            problem(format!(
                "{} is not lowered by the jvm backend",
                describe(other)
            ));
            "null".into()
        }
    }
}

thread_local! {
    /// Constructs this backend does not lower, collected while emitting.
    static PROBLEMS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Names already bound in the function being emitted. A later `x = e` is a
    /// reassignment; re-emitting the type made it a redeclaration, which javac
    /// rejects.
    static DECLARED: std::cell::RefCell<std::collections::BTreeSet<String>> =
        const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
    /// Declared record name -> its fields, in declaration order. A Java record's
    /// constructor is positional, so a literal written in another order has to
    /// be reordered or the values land in the wrong components.
    static RECORD_FIELDS: std::cell::RefCell<BTreeMap<String, Vec<String>>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
    /// Every field name any declared record has. A Java record keeps its
    /// components private and exposes an accessor of the same name, so reading
    /// one is `p.x()` rather than `p.x`; without types this is what tells a
    /// record field from a field of a foreign Java object.
    static RECORD_FIELD_NAMES: std::cell::RefCell<BTreeSet<String>> =
        const { std::cell::RefCell::new(BTreeSet::new()) };
    /// Function-typed parameters of every top-level function in the module.
    static FN_PARAMS: std::cell::RefCell<BTreeMap<(String, usize), Fnp>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
    /// Those belonging to the function being emitted, by parameter name, so a
    /// call site knows to invoke the interface method rather than a static.
    static IN_SCOPE: std::cell::RefCell<BTreeMap<String, Fnp>> =
        const { std::cell::RefCell::new(BTreeMap::new()) };
    /// variant name → its enum. Java needs `Status.Done` everywhere except a
    /// `case` label, where the bare name is the only spelling allowed.
    static VARIANT_OF: std::cell::RefCell<std::collections::BTreeMap<String, String>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Begin a function: its parameters are already bound, so a `x = e` naming one
/// is a reassignment rather than a declaration that would shadow it.
fn start_scope(params: &[Param]) {
    DECLARED.with(|d| {
        let mut d = d.borrow_mut();
        d.clear();
        for p in params {
            d.insert(p.name.clone());
        }
    });
}

/// Begin a function: note which of its parameters are functions, so a call to
/// one lowers to the interface's method instead of a static of the same name.
fn start_fn_scope(name: &str, params: &[Param]) {
    let all = FN_PARAMS.with(|m| m.borrow().clone());
    IN_SCOPE.with(|s| {
        let mut s = s.borrow_mut();
        s.clear();
        for (i, p) in params.iter().enumerate() {
            if let Some(sig) = all.get(&(name.to_string(), i)) {
                s.insert(p.name.clone(), *sig);
            }
        }
    });
}

/// The declared Java type of parameter `i` of function `name`.
fn param_type(name: &str, i: usize, p: &Param) -> String {
    if let Some(ty) = &p.ty {
        return jtype(ty);
    }
    match FN_PARAMS.with(|m| m.borrow().get(&(name.to_string(), i)).copied()) {
        Some(sig) if sig.arity > 2 => {
            problem(format!(
                "`{}` takes {} arguments; Java has no functional interface of \
                 that shape",
                p.name, sig.arity
            ));
            "Object".into()
        }
        Some(sig) => sig.iface(),
        None => "Object".into(),
    }
}

fn note_record(name: &str, fields: &[String]) {
    RECORD_FIELDS.with(|m| m.borrow_mut().insert(name.to_string(), fields.to_vec()));
    RECORD_FIELD_NAMES.with(|m| {
        let mut m = m.borrow_mut();
        for f in fields {
            m.insert(f.clone());
        }
    });
}

/// The one declared record whose fields are exactly `written`, if there is one.
/// Two records sharing a shape have no single answer, so those are refused.
fn record_of_shape(written: &[String]) -> Result<String, Vec<String>> {
    let mut want: Vec<String> = written.to_vec();
    want.sort();
    let mut hits: Vec<String> = Vec::new();
    RECORD_FIELDS.with(|m| {
        for (name, fields) in m.borrow().iter() {
            let mut have = fields.clone();
            have.sort();
            if have == want {
                hits.push(name.clone());
            }
        }
    });
    match hits.len() {
        1 => Ok(hits.remove(0)),
        _ => Err(hits),
    }
}

fn note_variants(enom: &str, vars: &[String]) {
    VARIANT_OF.with(|v| {
        let mut v = v.borrow_mut();
        for name in vars {
            v.insert(name.clone(), enom.to_string());
        }
    });
}

/// `Done` → `Status.Done` when it names a variant, else itself.
fn qualify(n: &str) -> String {
    VARIANT_OF.with(|v| match v.borrow().get(n) {
        Some(enom) => format!("{enom}.{n}"),
        None => n.to_string(),
    })
}

fn problem(msg: impl Into<String>) {
    PROBLEMS.with(|p| p.borrow_mut().push(msg.into()));
}

/// A parameter with no declared type that is *called* in the body is a
/// function. Maca writes no function type at a parameter, so its use in the
/// body is the only evidence there is; this mirrors the decision
/// `maca_backend_c`'s `closure_params` makes for the native target.
///
/// What Java needs beyond that decision is a *type*, and there is none to read.
/// The generated interfaces are `long`-typed because Maca's `int` is a Java
/// `long` and that is what an unannotated parameter is treated as everywhere
/// else in this emitter. A callback over strings or records would need its type
/// written down, which this target does not accept yet, so it is refused by name
/// rather than emitted as something that will not compile.
#[derive(Clone, Copy, PartialEq)]
struct Fnp {
    arity: usize,
    /// Whether the call's value is used. Java splits these: a `Function` returns
    /// and a `Consumer` does not, and one cannot stand in for the other.
    returns: bool,
}

impl Fnp {
    /// The generated interface this parameter is declared as.
    fn iface(&self) -> String {
        match (self.arity, self.returns) {
            (0, true) => "_Fn0".into(),
            (0, false) => "_Act0".into(),
            (1, true) => "_Fn1".into(),
            (1, false) => "_Act1".into(),
            (_, true) => "_Fn2".into(),
            (_, false) => "_Act2".into(),
        }
    }

    /// The interface's single method.
    fn call(&self) -> &'static str {
        if self.returns { "apply" } else { "accept" }
    }

    fn decl(&self) -> String {
        let ps: Vec<String> = (0..self.arity).map(|i| format!("long _a{i}")).collect();
        let ret = if self.returns { "long" } else { "void" };
        format!(
            "interface {} {{ {ret} {}({}); }}\n",
            self.iface(),
            self.call(),
            ps.join(", ")
        )
    }
}

/// Every function parameter of `items` that is used as a function, keyed by the
/// declaring function's name and the parameter's position.
fn fn_params(items: &[Stmt]) -> BTreeMap<(String, usize), Fnp> {
    let mut out = BTreeMap::new();
    for it in items {
        let Stmt::Fn(f) = it else { continue };
        let Some(body) = &f.body else { continue };
        let mut uses: BTreeMap<String, Fnp> = BTreeMap::new();
        // A function with no declared return type discards its tail value, so a
        // call in that position is a Consumer rather than a Function.
        collect_calls_body(body, &mut uses, f.ret.is_some());
        for (i, p) in f.params.iter().enumerate() {
            if p.ty.is_none()
                && let Some(sig) = uses.get(&p.name)
            {
                out.insert((f.name.clone(), i), *sig);
            }
        }
    }
    out
}

/// Record each `name(args)` in `body`. `valued` says whether the expression's
/// value is used where it appears, which is what decides `Function` vs
/// `Consumer`.
fn collect_calls_body(body: &FnBody, out: &mut BTreeMap<String, Fnp>, valued: bool) {
    match body {
        FnBody::Expr(e) => collect_calls(e, out, valued),
        FnBody::Block(stmts) => collect_calls_stmts(stmts, out, valued),
    }
}

fn collect_calls_stmts(stmts: &[Stmt], out: &mut BTreeMap<String, Fnp>, tail_valued: bool) {
    for (i, s) in stmts.iter().enumerate() {
        let last = i + 1 == stmts.len();
        match s {
            Stmt::Bind(b) => collect_calls(&b.value, out, true),
            // a bare call in the middle of a block discards its value
            Stmt::Expr(e) => collect_calls(e, out, last && tail_valued),
            _ => {}
        }
    }
}

fn collect_calls(e: &Expr, out: &mut BTreeMap<String, Fnp>, valued: bool) {
    match e {
        Expr::Call { callee, args } => {
            if let Expr::Ident(n) = callee.as_ref() {
                let sig = Fnp {
                    arity: args.len(),
                    returns: valued,
                };
                // A name called for its value anywhere is a Function: a
                // Consumer could not stand in, while the reverse is harmless.
                let keep = match out.get(n) {
                    Some(prev) if prev.returns => *prev,
                    _ => sig,
                };
                out.insert(n.clone(), keep);
            }
            for a in args {
                collect_calls(arg_expr(a), out, true);
            }
            collect_calls(callee, out, true);
        }
        Expr::Unary { expr, .. } => collect_calls(expr, out, true),
        Expr::Binary { lhs, rhs, .. } => {
            collect_calls(lhs, out, true);
            collect_calls(rhs, out, true);
        }
        Expr::Ternary { cond, then, els } => {
            collect_calls(cond, out, true);
            collect_calls(then, out, true);
            collect_calls(els, out, true);
        }
        Expr::If { cond, then, els } => {
            collect_calls(cond, out, true);
            collect_calls_stmts(then, out, valued);
            if let Some(e) = els {
                collect_calls_stmts(e, out, valued);
            }
        }
        Expr::Block(stmts) => collect_calls_stmts(stmts, out, valued),
        Expr::While { cond, body } => {
            collect_calls(cond, out, true);
            collect_calls_stmts(body, out, false);
        }
        Expr::For { iter, body, .. } => {
            collect_calls(iter, out, true);
            collect_calls_stmts(body, out, false);
        }
        Expr::Field { base, .. } => collect_calls(base, out, true),
        Expr::Index { base, index } => {
            collect_calls(base, out, true);
            collect_calls(index, out, true);
        }
        Expr::Lambda { body, .. } => collect_calls(body, out, true),
        Expr::Match { scrut, arms } => {
            collect_calls(scrut, out, true);
            for a in arms {
                collect_calls(&a.body, out, valued);
            }
        }
        Expr::Assign { value, .. } => collect_calls(value, out, true),
        Expr::Try(x) | Expr::Fail(x) | Expr::Reify(x) => collect_calls(x, out, true),
        _ => {}
    }
}

/// Name a construct the way the author wrote it, for a refusal message.
fn describe(e: &Expr) -> &'static str {
    match e {
        Expr::While { .. } => "`while` in value position",
        Expr::With { .. } => "a record update (`with`)",
        Expr::Fail(_) => "`fail`",
        Expr::Reify(_) => "`reify`",
        Expr::Await(_) => "`await`",
        Expr::Spawn(_) => "`spawn`",
        Expr::Range { .. } => "a range in value position",
        Expr::Path(_) => "a path expression",
        Expr::Unit => "the unit value",
        _ => "this construct",
    }
}

/// The value of a block used in expression position (its last expression).
fn block_value(stmts: &[Stmt]) -> String {
    match stmts.last() {
        Some(Stmt::Expr(e)) => jexpr(e),
        _ => {
            problem(
                "a block whose last statement is not an expression has no value                  on the jvm backend"
                    .to_string(),
            );
            "null".into()
        }
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
        // A function-typed parameter is not a static of the same name; Java
        // reaches it through the interface's single method.
        Expr::Ident(f) if IN_SCOPE.with(|s| s.borrow().contains_key(f)) => {
            let sig = IN_SCOPE.with(|s| s.borrow()[f]);
            format!("{f}.{}({})", sig.call(), a.join(", "))
        }
        Expr::Ident(f) => format!("{f}({})", a.join(", ")),
        Expr::Field { base, name } => format!("{}.{name}({})", jexpr(base), a.join(", ")),
        _ => format!("{}({})", jexpr(callee), a.join(", ")),
    }
}

fn jctor(e: &Expr, fields: &[Field]) -> String {
    // (field name, value) in the order written, which is not necessarily the
    // order the record declares.
    let written: Vec<(String, String)> = fields
        .iter()
        .filter_map(|f| match f {
            Field::Value { name, value } => Some((name.clone(), jexpr(value))),
            Field::Shorthand(n) => Some((n.clone(), n.clone())),
            _ => None,
        })
        .collect();
    let names: Vec<String> = written.iter().map(|(n, _)| n.clone()).collect();

    let name = match e {
        Expr::Ctor { name, .. } => name.clone(),
        // An anonymous literal takes the name of the one declared record with
        // its shape. Telling the author to "declare a record type" was advice
        // they could not follow once the checker accepted the literal straight
        // into a declared one.
        _ => match record_of_shape(&names) {
            Ok(found) => found,
            Err(hits) if hits.is_empty() => {
                problem(format!(
                    "this `{{ … }}` matches no declared record (its fields are \
                     {}), and a Java `new` needs a type",
                    names.join(", ")
                ));
                return "null".into();
            }
            Err(hits) => {
                problem(format!(
                    "this `{{ … }}` matches more than one declared record ({}), \
                     and the jvm backend needs one; write the constructor, as \
                     in `{} {{ … }}`",
                    hits.join(", "),
                    hits[0]
                ));
                return "null".into();
            }
        },
    };

    // A Java record's constructor is positional, so the values follow the
    // declaration rather than the literal. Writing them in the order they
    // appear put `{ y = 2, x = 1 }` into `Point(2, 1)`, silently.
    let order = RECORD_FIELDS.with(|m| m.borrow().get(&name).cloned());
    let vals: Vec<String> = match &order {
        Some(decl) => {
            let mut out = Vec::with_capacity(decl.len());
            for f in decl {
                match written.iter().find(|(n, _)| n == f) {
                    Some((_, v)) => out.push(v.clone()),
                    None => {
                        problem(format!(
                            "this `{name} {{ … }}` never writes field `{f}`, and \
                             a Java `new` has no value to pass for it"
                        ));
                        return "null".into();
                    }
                }
            }
            out
        }
        // A capitalized call that is not a declared record is Java interop, so
        // the arguments are the author's own order.
        None => written.into_iter().map(|(_, v)| v).collect(),
    };
    format!("new {name}({})", vals.join(", "))
}

fn jmatch(scrut: &Expr, arms: &[Arm]) -> String {
    // enum/value match → a Java switch expression
    let mut out = format!("switch ({}) {{\n", jexpr(scrut));
    for a in arms {
        let label = match &a.pat {
            Pattern::Wild => "default".to_string(),
            Pattern::Bind(n) => format!("case {n}"),
            // A payload pattern's arguments were dropped, so the arm body named
            // variables javac never saw. Sums lower to a plain Java enum, which
            // carries no payload to bind in the first place.
            Pattern::Ctor { name, args } if !args.is_empty() => {
                problem(format!(
                    "`{name}(…)` binds a payload; a sum lowers to a Java enum                      on this target, which carries none"
                ));
                format!("case {name}")
            }
            Pattern::Ctor { name, .. } => format!("case {name}"),
            Pattern::Int(n) => format!("case {n}"),
            // Java switches on strings, so this is a real label — it used to
            // fall to `default`, and two such arms were a duplicate-default
            // error from javac.
            Pattern::Str(v) => format!("case {}", java_str_lit(v)),
            other => {
                problem(format!(
                    "{} cannot be a `case` label on the jvm backend",
                    describe_pattern(other)
                ));
                "default".to_string()
            }
        };
        out.push_str(&format!("        {label} -> {};\n", jexpr(&a.body)));
    }
    out.push_str("    }");
    out
}

/// Name a pattern the way the author wrote it, for a refusal message.
fn describe_pattern(p: &Pattern) -> &'static str {
    match p {
        Pattern::Bool(_) => "a bool pattern (Java cannot switch on a boolean)",
        Pattern::Float(_) => "a float pattern",
        Pattern::List { .. } => "a list pattern",
        Pattern::Record(_) => "a record pattern",
        Pattern::Or(_) => "an or-pattern",
        _ => "this pattern",
    }
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
        // Java has a functional interface per shape; one parameter covers what
        // this back end can carry across the boundary today.
        Type::Fn(ps, r) => match ps.len() {
            0 => format!("java.util.function.Supplier<{}>", boxed(r)),
            _ => format!(
                "java.util.function.Function<{}, {}>",
                boxed(&ps[0]),
                boxed(r)
            ),
        },
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
