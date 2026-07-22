//! maca-backend-js: lower a UI component to vanilla-DOM JS + HTML + CSS.
//!
//! Elements are functions (`div(class=…, …children)`); top-level bindings are
//! reactive state (Svelte-style compile-time reactivity — assigning to state and
//! calling `update()` re-syncs bound nodes). `bind:value` is two-way,
//! `on:event` wires a handler. `class` utility names are collected and a
//! tree-shaken Tailwind subset is emitted (only used classes ship).

use maca_parser::ast::*;
use std::collections::BTreeSet;

pub struct JsOut {
    pub js: String,
    pub html: String,
    pub css: String,
    /// Names of top-level functions transpiled to callable JS (for `import`).
    pub exports: Vec<String>,
}

pub fn emit(m: &Module) -> JsOut {
    let mut cx = Cx::default();

    // top-level state bindings
    for item in &m.items {
        if let Stmt::Bind(b) = item {
            if let Expr::Ident(name) = &b.target {
                if sum_variants(&b.value).is_none() && !is_record_type(&b.value) {
                    cx.state.push((name.clone(), js_init(&b.value)));
                }
            }
        }
    }
    let state_names: BTreeSet<String> = cx.state.iter().map(|(n, _)| n.clone()).collect();
    cx.state_names = state_names;

    // main() -> Element body
    let root_expr = m.items.iter().find_map(|it| match it {
        Stmt::Fn(f) if f.name == "main" => match &f.body {
            Some(FnBody::Expr(e)) => Some(e.as_ref().clone()),
            Some(FnBody::Block(s)) => s.iter().rev().find_map(|st| match st {
                Stmt::Expr(e) => Some(e.clone()),
                _ => None,
            }),
            None => None,
        },
        _ => None,
    });

    let mut build_body = String::new();
    let root_var = match &root_expr {
        Some(e) => cx.element(e, &mut build_body),
        None => {
            build_body.push_str("  const n0 = document.createElement(\"div\");\n");
            "n0".into()
        }
    };

    let mut js = cx.finish(&build_body, &root_var);

    // Transpile top-level functions to callable JS (skip `main`, which is the
    // UI/entry). These make `import "x.maca"` usable from JS/Bun.
    let mut exports = Vec::new();
    let mut fn_defs = String::new();
    for item in &m.items {
        if let Stmt::Fn(f) = item {
            if f.name == "main" || f.body.is_none() {
                continue;
            }
            fn_defs.push_str(&emit_fn(f));
            fn_defs.push('\n');
            exports.push(f.name.clone());
        }
    }
    if !fn_defs.is_empty() {
        js.push_str("\n// ---- transpiled functions ----\n");
        js.push_str(&fn_defs);
        let names = exports.join(", ");
        js.push_str(&format!(
            "if (typeof module !== \"undefined\") Object.assign(module.exports, {{ {names} }});\n"
        ));
    }

    let css = cx.css();
    let html = HTML.into();
    JsOut { js, html, css, exports }
}

/// Transpile one top-level function to a JS function declaration.
fn emit_fn(f: &FnDef) -> String {
    let params = f.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ");
    let body = match &f.body {
        Some(FnBody::Expr(e)) => format!("  return {};", jexpr(e)),
        Some(FnBody::Block(stmts)) => jblock(stmts),
        None => String::new(),
    };
    format!("function {}({params}) {{\n{body}\n}}", f.name)
}

/// A block of statements → JS, returning the value of the final expression.
fn jblock(stmts: &[Stmt]) -> String {
    jblock_ret(stmts, true)
}

/// `ret` = whether the final expression is `return`ed (function body) or emitted
/// as a bare statement (loop body).
fn jblock_ret(stmts: &[Stmt], ret: bool) -> String {
    let mut out = String::new();
    for (i, s) in stmts.iter().enumerate() {
        let last = i + 1 == stmts.len();
        match s {
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target {
                    // `let x =` declares; a bare `x =` reassigns
                    let kw = if b.is_let { "let " } else { "" };
                    out.push_str(&format!("  {kw}{n} = {};\n", jexpr(&b.value)));
                } else {
                    // lvalue assignment: `xs[i] = v`, `p.field = v`
                    out.push_str(&format!("  {} = {};\n", jexpr(&b.target), jexpr(&b.value)));
                }
            }
            Stmt::Expr(Expr::While { cond, body }) => {
                out.push_str(&format!("  while ({}) {{\n{}  }}\n", jexpr(cond), jblock_ret(body, false)));
            }
            Stmt::Expr(Expr::Break) => out.push_str("  break;\n"),
            Stmt::Expr(Expr::Continue) => out.push_str("  continue;\n"),
            Stmt::Expr(e) => {
                if last && ret {
                    out.push_str(&format!("  return {};\n", jexpr(e)));
                } else {
                    out.push_str(&format!("  {};\n", jexpr(e)));
                }
            }
            Stmt::Fn(f) => out.push_str(&emit_fn(f)),
            _ => {}
        }
    }
    out
}

/// Lower a Maca expression to a JS expression (the functional core).
fn jexpr(e: &Expr) -> String {
    match e {
        Expr::Int(n) => n.to_string(),
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
        Expr::Call { callee, args } => jcall(callee, args),
        Expr::Field { base, name } => format!("{}.{name}", jexpr(base)),
        Expr::Index { base, index } => format!("{}[{}]", jexpr(base), jexpr(index)),
        Expr::Record(fields) | Expr::Ctor { fields, .. } => jrecord(fields),
        Expr::List(es) => {
            format!("[{}]", es.iter().map(jexpr).collect::<Vec<_>>().join(", "))
        }
        Expr::If { cond, then, els } => {
            let e = els.as_ref().map(|s| jblock_expr(s)).unwrap_or_else(|| "null".into());
            format!("({} ? {} : {})", jexpr(cond), jblock_expr(then), e)
        }
        Expr::Block(stmts) => jblock_expr(stmts),
        Expr::Try(x) => jexpr(x),
        // `base with { … }` → object spread with the named fields overwritten.
        Expr::With { base, fields } => {
            let upd = jrecord(fields);
            format!("{{ ...{}, ...{} }}", jexpr(base), upd)
        }
        _ => "null".into(),
    }
}

/// A block used in expression position → an IIFE returning its value.
fn jblock_expr(stmts: &[Stmt]) -> String {
    format!("(() => {{\n{}\n}})()", jblock(stmts))
}

fn jbinary(op: BinOp, lhs: &Expr, rhs: &Expr) -> String {
    use BinOp::*;
    let (l, r) = (jexpr(lhs), jexpr(rhs));
    let o = match op {
        Add => "+", Sub => "-", Mul => "*", Div => "/", Mod => "%",
        Shl => "<<", Shr => ">>",
        Eq => "===", Ne => "!==", Lt => "<", Gt => ">", Le => "<=", Ge => ">=",
        And => "&&", Or => "||",
        Concat => return format!("({l}).concat({r})"),
        Union | Pipe => return l,
    };
    format!("({l} {o} {r})")
}

fn jcall(callee: &Expr, args: &[Arg]) -> String {
    let a: Vec<String> = args.iter().map(|x| jexpr(&arg_expr(x))).collect();
    match callee {
        Expr::Ident(f) if f == "int" => format!("Math.trunc(Number({}))", a.first().cloned().unwrap_or_default()),
        Expr::Ident(f) if f == "float" => format!("Number({})", a.first().cloned().unwrap_or_default()),
        Expr::Ident(f) if f == "str" => format!("String({})", a.first().cloned().unwrap_or_default()),
        Expr::Ident(f) if f == "len" => format!("({}).length", a.first().cloned().unwrap_or_default()),
        Expr::Ident(f) => format!("{f}({})", a.join(", ")),
        Expr::Field { base, name } => format!("{}.{name}({})", jexpr(base), a.join(", ")),
        _ => format!("{}({})", jexpr(callee), a.join(", ")),
    }
}

fn jrecord(fields: &[Field]) -> String {
    let mut out = Vec::new();
    for f in fields {
        match f {
            Field::Value { name, value } => out.push(format!("{name}: {}", jexpr(value))),
            Field::Shorthand(n) => out.push(format!("{n}: {n}")),
            _ => {}
        }
    }
    format!("{{ {} }}", out.join(", "))
}

/// String with interpolation → a JS template literal.
fn jstr(parts: &[StrPart]) -> String {
    let mut out = String::from("`");
    for p in parts {
        match p {
            StrPart::Text(t) => out.push_str(&t.replace('\\', "\\\\").replace('`', "\\`").replace('$', "\\$")),
            StrPart::Interp(e) => out.push_str(&format!("${{{}}}", jexpr(e))),
        }
    }
    out.push('`');
    out
}

#[derive(Default)]
struct Cx {
    state: Vec<(String, String)>,
    state_names: BTreeSet<String>,
    classes: BTreeSet<String>,
    n: usize,
}

impl Cx {
    fn fresh(&mut self) -> String {
        let v = format!("n{}", self.n);
        self.n += 1;
        v
    }

    /// Emit code that builds `expr` (an element call), returning its var name.
    fn element(&mut self, e: &Expr, out: &mut String) -> String {
        let Expr::Call { callee, args } = e else {
            // a bare string/expr child → text node
            let v = self.fresh();
            out.push_str(&format!(
                "  const {v} = document.createTextNode({});\n",
                self.value(e, None)
            ));
            return v;
        };
        let tag = match callee.as_ref() {
            Expr::Ident(t) => t.clone(),
            _ => "div".into(),
        };
        let v = self.fresh();
        out.push_str(&format!("  const {v} = document.createElement(\"{tag}\");\n"));

        for a in args {
            match a {
                Arg::Named { name, value } if name == "class" => {
                    if let Expr::Str(parts) = value {
                        for c in plain_text(parts).split_whitespace() {
                            self.classes.insert(c.to_string());
                        }
                    }
                    out.push_str(&format!("  {v}.className = {};\n", self.value(value, None)));
                }
                Arg::Named { name, value } => {
                    out.push_str(&format!(
                        "  {v}.setAttribute(\"{name}\", {});\n",
                        self.value(value, None)
                    ));
                }
                Arg::Directive { kind: Dir::Bind, prop, value } => {
                    let (getter, setter) = self.bind(value);
                    out.push_str(&format!("  {v}.{prop} = {getter};\n"));
                    out.push_str(&format!(
                        "  {v}.addEventListener(\"input\", (e) => {{ {} ; update(); }});\n",
                        setter.replace("$v", "e.target.value")
                    ));
                    out.push_str(&format!("  _binds.push(() => {{ {v}.{prop} = {getter}; }});\n"));
                }
                Arg::Directive { kind: Dir::On, prop, value } => {
                    out.push_str(&format!(
                        "  {v}.addEventListener(\"{prop}\", {});\n",
                        self.value(value, None)
                    ));
                }
                Arg::Pos(child) => {
                    let cv = self.element(child, out);
                    out.push_str(&format!("  {v}.appendChild({cv});\n"));
                }
            }
        }
        v
    }

    /// Two-way bind → (JS getter expr, JS setter stmt using `$v` for the value).
    fn bind(&self, target: &Expr) -> (String, String) {
        match target {
            Expr::Ident(n) => (format!("state.{n}"), format!("state.{n} = $v")),
            Expr::Lambda { params, body } => {
                // `v => age = int(v)` — bound var is the assignment target
                let pv = params.first().map(|p| p.name.as_str()).unwrap_or("v");
                if let Expr::Assign { target: t, value } = body.as_ref() {
                    if let Expr::Ident(x) = t.as_ref() {
                        let set = format!("state.{x} = {}", self.value(value, Some((pv, "$v"))));
                        return (format!("state.{x}"), set);
                    }
                }
                ("\"\"".into(), String::new())
            }
            _ => ("\"\"".into(), String::new()),
        }
    }

    /// JS for a value expression. `subst` replaces a lambda parameter.
    fn value(&self, e: &Expr, subst: Option<(&str, &str)>) -> String {
        match e {
            Expr::Str(parts) => format!("{:?}", plain_text(parts)),
            Expr::Int(n) => n.to_string(),
            Expr::Float(f) => format!("{f}"),
            Expr::Bool(b) => b.to_string(),
            Expr::Ident(n) => {
                if let Some((p, v)) = subst {
                    if n == p {
                        return v.to_string();
                    }
                }
                if self.state_names.contains(n) {
                    format!("state.{n}")
                } else {
                    n.clone()
                }
            }
            Expr::Call { callee, args } => match callee.as_ref() {
                Expr::Ident(f) if f == "int" => {
                    format!("(parseInt({}) || 0)", self.value(&arg_expr(&args[0]), subst))
                }
                Expr::Ident(f) if f == "str" => {
                    format!("String({})", self.value(&arg_expr(&args[0]), subst))
                }
                _ => "null".into(),
            },
            Expr::Assign { target, value } => {
                let t = match target.as_ref() {
                    Expr::Ident(n) => format!("state.{n}"),
                    _ => "_".into(),
                };
                format!("{t} = {}", self.value(value, subst))
            }
            _ => "null".into(),
        }
    }

    fn finish(&self, build_body: &str, root: &str) -> String {
        let state = self
            .state
            .iter()
            .map(|(n, v)| format!("{n}: {v}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "\"use strict\";\n\
             const state = {{ {state} }};\n\
             const _binds = [];\n\
             function build() {{\n{build_body}  return {root};\n}}\n\
             function update() {{ for (const b of _binds) b(); }}\n\
             let _root = null;\n\
             function mount(target) {{ _root = build(); target.appendChild(_root); return _root; }}\n\
             if (typeof document !== \"undefined\" && document.getElementById) {{\n\
             \x20 const app = document.getElementById(\"app\");\n\
             \x20 if (app) mount(app);\n\
             }}\n\
             if (typeof module !== \"undefined\") module.exports = {{ state, mount, build, update }};\n"
        )
    }

    fn css(&self) -> String {
        let mut out = String::from("/* generated Tailwind subset (tree-shaken) */\n");
        for c in &self.classes {
            if let Some(rule) = tailwind(c) {
                out.push_str(&format!(".{} {{ {rule} }}\n", css_escape(c)));
            }
        }
        out
    }
}

fn arg_expr(a: &Arg) -> Expr {
    match a {
        Arg::Pos(e) | Arg::Named { value: e, .. } | Arg::Directive { value: e, .. } => e.clone(),
    }
}

const HTML: &str = "<!doctype html>\n\
<html>\n<head>\n<meta charset=\"utf-8\">\n<title>maca app</title>\n\
<link rel=\"stylesheet\" href=\"app.css\">\n</head>\n\
<body>\n<div id=\"app\"></div>\n<script src=\"app.js\"></script>\n</body>\n</html>\n";

fn js_init(e: &Expr) -> String {
    match e {
        Expr::Str(parts) => format!("{:?}", plain_text(parts)),
        Expr::Int(n) => n.to_string(),
        Expr::Float(f) => format!("{f}"),
        Expr::Bool(b) => b.to_string(),
        _ => "null".into(),
    }
}

/// A tiny, extensible Tailwind subset. Returns the CSS body for a utility class.
fn tailwind(class: &str) -> Option<&'static str> {
    Some(match class {
        "flex" => "display: flex;",
        "flex-col" => "flex-direction: column;",
        "flex-row" => "flex-direction: row;",
        "grid" => "display: grid;",
        "block" => "display: block;",
        "inline" => "display: inline;",
        "hidden" => "display: none;",
        "text-center" => "text-align: center;",
        "text-left" => "text-align: left;",
        "text-right" => "text-align: right;",
        "items-center" => "align-items: center;",
        "justify-center" => "justify-content: center;",
        "justify-between" => "justify-content: space-between;",
        "w-full" => "width: 100%;",
        "h-full" => "height: 100%;",
        "font-bold" => "font-weight: 700;",
        "rounded" => "border-radius: 0.25rem;",
        "gap-2" => "gap: 0.5rem;",
        "gap-4" => "gap: 1rem;",
        "p-2" => "padding: 0.5rem;",
        "p-4" => "padding: 1rem;",
        "m-2" => "margin: 0.5rem;",
        "m-4" => "margin: 1rem;",
        _ => return None,
    })
}

fn css_escape(c: &str) -> String {
    c.to_string()
}

fn plain_text(parts: &[StrPart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            StrPart::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn is_record_type(e: &Expr) -> bool {
    matches!(e, Expr::Record(fs) if !fs.is_empty() && fs.iter().all(|f| matches!(f, Field::Type { .. })))
}

fn sum_variants(e: &Expr) -> Option<()> {
    matches!(e, Expr::Binary { op: BinOp::Union, .. }).then_some(())
}
