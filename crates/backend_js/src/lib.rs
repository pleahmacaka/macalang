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

    let js = cx.finish(&build_body, &root_var);
    let css = cx.css();
    let html = HTML.into();
    JsOut { js, html, css }
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
