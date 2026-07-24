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
        if let Stmt::Bind(b) = item
            && let Expr::Ident(name) = &b.target
            && sum_variants(&b.value).is_none()
            && !is_record_type(&b.value)
        {
            cx.state.push((name.clone(), js_init(&b.value)));
        }
    }
    let state_names: BTreeSet<String> = cx.state.iter().map(|(n, _)| n.clone()).collect();
    cx.state_names = state_names.clone();
    set_state_names(&state_names);

    // Collect Tailwind class candidates from every string literal in the module
    // (not just `class=` attributes), so classes returned from a helper — e.g.
    // `tab(n) => n == active ? "px-2 bg-zinc-800" : "px-2"` — still emit CSS.
    // Non-class tokens simply resolve to nothing and are dropped.
    for item in &m.items {
        collect_class_strings(item, &mut cx.classes);
    }

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

    // foreign blocks embedded in the .maca source: `import js """…"""` carries
    // raw JS (the host/runtime glue a UI app needs), `import css """…"""` raw
    // CSS. This lets a single .maca file carry everything it needs. The JS is
    // *prepended* so any helpers it defines exist before the app first mounts.
    let mut css = cx.css();
    let mut foreign_js = String::new();
    for item in &m.items {
        if let Stmt::Import(Import::Foreign { lang, spec }) = item {
            match lang.as_str() {
                "js" => {
                    foreign_js.push_str("// ---- embedded (import js) ----\n");
                    foreign_js.push_str(spec);
                    foreign_js.push('\n');
                }
                "css" => {
                    css.push_str("\n/* ---- embedded (import css) ---- */\n");
                    css.push_str(spec);
                    css.push('\n');
                }
                _ => {}
            }
        }
    }
    let js = if foreign_js.is_empty() {
        js
    } else {
        format!("{foreign_js}\n{js}")
    };
    let html = HTML.into();
    JsOut {
        js,
        html,
        css,
        exports,
    }
}

/// Transpile one top-level function to a JS function declaration.
fn emit_fn(f: &FnDef) -> String {
    let params = f
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
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
                    // reactive state resolves to `state.x`; a local is declared
                    // with `var` (which, unlike `let`, tolerates the redeclaration
                    // a bare `x =` reassignment would otherwise produce).
                    if STATE.with(|s| s.borrow().contains(n)) {
                        out.push_str(&format!("  {} = {};\n", jname(n), jexpr(&b.value)));
                    } else {
                        out.push_str(&format!("  var {n} = {};\n", jexpr(&b.value)));
                    }
                } else {
                    // lvalue assignment: `xs[i] = v`, `p.field = v`
                    out.push_str(&format!("  {} = {};\n", jexpr(&b.target), jexpr(&b.value)));
                }
            }
            Stmt::Expr(Expr::While { cond, body }) => {
                out.push_str(&format!(
                    "  while ({}) {{\n{}  }}\n",
                    jexpr(cond),
                    jblock_ret(body, false)
                ));
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

thread_local! {
    /// Top-level reactive-state names, consulted so a reference to `x` lowers to
    /// `state.x` everywhere (view text, attributes, and transpiled functions).
    static STATE: std::cell::RefCell<BTreeSet<String>> = const { std::cell::RefCell::new(BTreeSet::new()) };
}
fn set_state_names(names: &BTreeSet<String>) {
    STATE.with(|s| *s.borrow_mut() = names.clone());
}
/// A bare identifier → `state.x` when it names reactive state, else itself.
fn jname(n: &str) -> String {
    if STATE.with(|s| s.borrow().contains(n)) {
        format!("state.{n}")
    } else {
        n.to_string()
    }
}
/// Does an expression read reactive state or call a function (so a text/attr
/// node bound to it must be refreshed on `update()`)?
fn is_dynamic(e: &Expr) -> bool {
    match e {
        Expr::Ident(n) => STATE.with(|s| s.borrow().contains(n)),
        Expr::Call { .. } => true,
        Expr::Str(parts) => parts
            .iter()
            .any(|p| matches!(p, StrPart::Interp(x) if is_dynamic(x))),
        Expr::Binary { lhs, rhs, .. } => is_dynamic(lhs) || is_dynamic(rhs),
        Expr::Unary { expr, .. } | Expr::Field { base: expr, .. } => is_dynamic(expr),
        Expr::Index { base, index } => is_dynamic(base) || is_dynamic(index),
        Expr::Ternary { cond, then, els } => {
            is_dynamic(cond) || is_dynamic(then) || is_dynamic(els)
        }
        _ => false,
    }
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
        Expr::Ident(n) => jname(n),
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
        // `lo..hi` — inclusive integer range as a JS array (lo … hi).
        Expr::Range { lo, hi } => format!(
            "Array.from({{length: Math.max(0, ({}) - ({}) + 1)}}, (_, _i) => _i + ({}))",
            jexpr(hi),
            jexpr(lo),
            jexpr(lo)
        ),
        Expr::If { cond, then, els } => {
            let e = els
                .as_ref()
                .map(|s| jblock_expr(s))
                .unwrap_or_else(|| "null".into());
            format!("({} ? {} : {})", jexpr(cond), jblock_expr(then), e)
        }
        Expr::Block(stmts) => jblock_expr(stmts),
        Expr::Try(x) => jexpr(x),
        // `base with { … }` → object spread with the named fields overwritten.
        Expr::With { base, fields } => {
            let upd = jrecord(fields);
            format!("{{ ...{}, ...{} }}", jexpr(base), upd)
        }
        Expr::Assign { target, value } => format!("({} = {})", jexpr(target), jexpr(value)),
        // JS is single-threaded in the UI; colorblind async runs eagerly (the
        // event loop still interleaves), matching the playground interpreter.
        Expr::Await(x) | Expr::Spawn(x) => jexpr(x),
        Expr::Lambda { params, body } => {
            let ps = params
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!("(({ps}) => {})", jexpr(body))
        }
        Expr::Fail(x) => format!("(() => {{ throw new Error(String({})); }})()", jexpr(x)),
        Expr::Reify(x) => format!(
            "(() => {{ try {{ {}; return \"\"; }} catch (_e) {{ return String(_e.message); }} }})()",
            jexpr(x)
        ),
        Expr::Match { scrut, arms } => jmatch(scrut, arms),
        _ => "null".into(),
    }
}

/// `match` in expression position → an IIFE with an if-chain. Handles literal,
/// bind, wildcard, and or-patterns (primitives — what a UI handler needs);
/// constructor patterns fall through to a binding (JS sum values are untagged).
fn jmatch(scrut: &Expr, arms: &[Arm]) -> String {
    let mut body = format!("(() => {{ const _s = {};", jexpr(scrut));
    for a in arms {
        let (cond, binds) = jpattern(&a.pat);
        let guard = a
            .guard
            .as_ref()
            .map(|g| format!(" && ({})", jexpr(g)))
            .unwrap_or_default();
        body.push_str(&format!(
            " if ({cond}{guard}) {{ {binds}return {}; }}",
            jexpr(&a.body)
        ));
    }
    body.push_str(" })()");
    body
}

/// (condition, binding-statements) for matching `_s` against `pat`.
fn jpattern(pat: &Pattern) -> (String, String) {
    match pat {
        Pattern::Wild => ("true".into(), String::new()),
        Pattern::Bind(n) => ("true".into(), format!("const {n} = _s; ")),
        Pattern::Int(n) => (format!("_s === {n}"), String::new()),
        Pattern::Float(f) => (format!("_s === {f}"), String::new()),
        Pattern::Bool(b) => (format!("_s === {b}"), String::new()),
        Pattern::Str(s) => (format!("_s === {s:?}"), String::new()),
        Pattern::Or(alts) => {
            let conds: Vec<String> = alts.iter().map(|p| jpattern(p).0).collect();
            (format!("({})", conds.join(" || ")), String::new())
        }
        // untagged sum values: best effort — bind the scrutinee, always match
        _ => ("true".into(), String::new()),
    }
}

/// A block used in expression position → an IIFE returning its value.
fn jblock_expr(stmts: &[Stmt]) -> String {
    format!("(() => {{\n{}\n}})()", jblock(stmts))
}

/// Clone `e`, replacing every free reference to `from` with `Ident(to)`. Used to
/// substitute a UI-handler lambda's parameter with the event value sentinel.
fn subst_ident(e: &Expr, from: &str, to: &str) -> Expr {
    let go = |x: &Expr| Box::new(subst_ident(x, from, to));
    let field = |f: &Field| match f {
        Field::Value { name, value } => Field::Value {
            name: name.clone(),
            value: subst_ident(value, from, to),
        },
        Field::Bare(v) => Field::Bare(subst_ident(v, from, to)),
        other => other.clone(),
    };
    match e {
        Expr::Ident(n) if n == from => Expr::Ident(to.to_string()),
        Expr::Str(parts) => Expr::Str(
            parts
                .iter()
                .map(|p| match p {
                    StrPart::Interp(x) => StrPart::Interp(subst_ident(x, from, to)),
                    t => t.clone(),
                })
                .collect(),
        ),
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: go(expr),
        },
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op: *op,
            lhs: go(lhs),
            rhs: go(rhs),
        },
        Expr::Ternary { cond, then, els } => Expr::Ternary {
            cond: go(cond),
            then: go(then),
            els: go(els),
        },
        Expr::Call { callee, args } => Expr::Call {
            callee: go(callee),
            args: args
                .iter()
                .map(|a| match a {
                    Arg::Pos(x) => Arg::Pos(subst_ident(x, from, to)),
                    Arg::Named { name, value } => Arg::Named {
                        name: name.clone(),
                        value: subst_ident(value, from, to),
                    },
                    Arg::Directive { kind, prop, value } => Arg::Directive {
                        kind: *kind,
                        prop: prop.clone(),
                        value: subst_ident(value, from, to),
                    },
                })
                .collect(),
        },
        Expr::Field { base, name } => Expr::Field {
            base: go(base),
            name: name.clone(),
        },
        Expr::Index { base, index } => Expr::Index {
            base: go(base),
            index: go(index),
        },
        Expr::Assign { target, value } => Expr::Assign {
            target: go(target),
            value: go(value),
        },
        Expr::List(es) => Expr::List(es.iter().map(|x| subst_ident(x, from, to)).collect()),
        Expr::Record(fs) => Expr::Record(fs.iter().map(field).collect()),
        Expr::Ctor { name, fields } => Expr::Ctor {
            name: name.clone(),
            fields: fields.iter().map(field).collect(),
        },
        Expr::With { base, fields } => Expr::With {
            base: go(base),
            fields: fields.iter().map(field).collect(),
        },
        Expr::Range { lo, hi } => Expr::Range {
            lo: go(lo),
            hi: go(hi),
        },
        Expr::Try(x) => Expr::Try(go(x)),
        Expr::Fail(x) => Expr::Fail(go(x)),
        Expr::Reify(x) => Expr::Reify(go(x)),
        Expr::Await(x) => Expr::Await(go(x)),
        Expr::Spawn(x) => Expr::Spawn(go(x)),
        // a nested lambda that rebinds `from` shadows it; otherwise descend
        Expr::Lambda { params, body } if !params.iter().any(|p| p.name == from) => Expr::Lambda {
            params: params.clone(),
            body: go(body),
        },
        other => other.clone(),
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
        Eq => "===",
        Ne => "!==",
        Lt => "<",
        Gt => ">",
        Le => "<=",
        Ge => ">=",
        And => "&&",
        Or => "||",
        Concat => return format!("({l}).concat({r})"),
        Union | Pipe => return l,
    };
    format!("({l} {o} {r})")
}

fn jcall(callee: &Expr, args: &[Arg]) -> String {
    let a: Vec<String> = args.iter().map(|x| jexpr(&arg_expr(x))).collect();
    match callee {
        Expr::Ident(f) if f == "int" => format!(
            "Math.trunc(Number({}))",
            a.first().cloned().unwrap_or_default()
        ),
        Expr::Ident(f) if f == "float" => {
            format!("Number({})", a.first().cloned().unwrap_or_default())
        }
        Expr::Ident(f) if f == "str" => {
            format!("String({})", a.first().cloned().unwrap_or_default())
        }
        Expr::Ident(f) if f == "len" => {
            format!("({}).length", a.first().cloned().unwrap_or_default())
        }
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
            StrPart::Text(t) => out.push_str(
                &t.replace('\\', "\\\\")
                    .replace('`', "\\`")
                    .replace('$', "\\$"),
            ),
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
        // A call to a known HTML tag (`div(...)`) builds an element; anything
        // else — a literal, an identifier, or a call to a *text-returning*
        // function like `mcTab(tab)` — is a (reactive) text node.
        let is_element = matches!(
            e,
            Expr::Call { callee, .. } if matches!(callee.as_ref(), Expr::Ident(t) if is_html_tag(t))
        );
        if !is_element {
            let v = self.fresh();
            let expr = jexpr(e);
            out.push_str(&format!("  const {v} = document.createTextNode({expr});\n"));
            if is_dynamic(e) {
                out.push_str(&format!(
                    "  _binds.push(() => {{ {v}.textContent = {expr}; }});\n"
                ));
            }
            return v;
        }
        let Expr::Call { callee, args } = e else {
            unreachable!()
        };
        let tag = match callee.as_ref() {
            Expr::Ident(t) => t.clone(),
            _ => "div".into(),
        };
        let v = self.fresh();
        out.push_str(&format!(
            "  const {v} = document.createElement(\"{tag}\");\n"
        ));

        for a in args {
            match a {
                Arg::Named { name, value } if name == "html" => {
                    // `html=expr` sets innerHTML (reactively) — for pre-rendered
                    // markup like the flame-graph SVG or highlighted source.
                    let expr = jexpr(value);
                    out.push_str(&format!("  {v}.innerHTML = {expr};\n"));
                    if is_dynamic(value) {
                        out.push_str(&format!(
                            "  _binds.push(() => {{ {v}.innerHTML = {expr}; }});\n"
                        ));
                    }
                }
                Arg::Named { name, value } if name == "class" => {
                    if let Expr::Str(parts) = value {
                        for c in plain_text(parts).split_whitespace() {
                            self.classes.insert(c.to_string());
                        }
                    }
                    let expr = jexpr(value);
                    out.push_str(&format!("  {v}.className = {expr};\n"));
                    if is_dynamic(value) {
                        out.push_str(&format!(
                            "  _binds.push(() => {{ {v}.className = {expr}; }});\n"
                        ));
                    }
                }
                Arg::Named { name, value } => {
                    let expr = jexpr(value);
                    out.push_str(&format!("  {v}.setAttribute(\"{name}\", {expr});\n"));
                    if is_dynamic(value) {
                        out.push_str(&format!(
                            "  _binds.push(() => {{ {v}.setAttribute(\"{name}\", {expr}); }});\n"
                        ));
                    }
                }
                Arg::Directive {
                    kind: Dir::Bind,
                    prop,
                    value,
                } => {
                    let (getter, setter) = self.bind(value);
                    out.push_str(&format!("  {v}.{prop} = {getter};\n"));
                    out.push_str(&format!(
                        "  {v}.addEventListener(\"input\", (e) => {{ {} ; update(); }});\n",
                        setter.replace("$v", "e.target.value")
                    ));
                    // don't clobber an input the user is actively editing (avoids
                    // the caret jumping to the end on every re-render).
                    out.push_str(&format!(
                        "  _binds.push(() => {{ if (document.activeElement !== {v}) {v}.{prop} = {getter}; }});\n"
                    ));
                }
                Arg::Directive {
                    kind: Dir::On,
                    prop,
                    value,
                } => {
                    out.push_str(&format!(
                        "  {v}.addEventListener(\"{prop}\", {});\n",
                        jexpr(value)
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
                if let Expr::Assign { target: t, value } = body.as_ref()
                    && let Expr::Ident(x) = t.as_ref()
                {
                    let set = format!("state.{x} = {}", self.value(value, Some((pv, "$v"))));
                    return (format!("state.{x}"), set);
                }
                ("\"\"".into(), String::new())
            }
            _ => ("\"\"".into(), String::new()),
        }
    }

    /// JS for a value expression (event handlers, bindings, state init). Reuses
    /// the full `jexpr` lowering, so handlers can call functions, do arithmetic,
    /// index, match, etc. — not just literals. `subst` replaces a lambda
    /// parameter (the event's `$v`) throughout the expression first.
    fn value(&self, e: &Expr, subst: Option<(&str, &str)>) -> String {
        match subst {
            Some((from, to)) => jexpr(&subst_ident(e, from, to)),
            None => jexpr(e),
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
        let mut out = String::from(
            "/* generated Tailwind subset (tree-shaken) */\n\
             *,*::before,*::after{box-sizing:border-box}\nhtml,body{margin:0}\n",
        );
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

/// Is `name` an HTML element tag (so `name(...)` in a view builds a DOM node),
/// as opposed to a text-returning function call used as a child? Delegates to
/// the canonical list in `maca-parser`, shared with the type checker.
fn is_html_tag(name: &str) -> bool {
    maca_parser::is_ui_element_tag(name)
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

/// Maca's integrated Tailwind: turn a utility class into its CSS body. Covers
/// the common utilities (display/flex/grid, spacing, sizing, text, colors,
/// borders, rounding, overflow, position) generatively — a compact but real
/// engine, not a fixed table. Unknown classes return `None` and are dropped.
fn tailwind(class: &str) -> Option<String> {
    // fixed keywords first
    let fixed = match class {
        "flex" => "display:flex",
        "inline-flex" => "display:inline-flex",
        "grid" => "display:grid",
        "block" => "display:block",
        "inline" => "display:inline",
        "inline-block" => "display:inline-block",
        "hidden" => "display:none",
        "flex-col" => "flex-direction:column",
        "flex-row" => "flex-direction:row",
        "flex-wrap" => "flex-wrap:wrap",
        "flex-1" => "flex:1 1 0%",
        "flex-auto" => "flex:1 1 auto",
        "flex-none" => "flex:none",
        "items-center" => "align-items:center",
        "items-start" => "align-items:flex-start",
        "items-end" => "align-items:flex-end",
        "items-baseline" => "align-items:baseline",
        "items-stretch" => "align-items:stretch",
        "justify-center" => "justify-content:center",
        "justify-between" => "justify-content:space-between",
        "justify-start" => "justify-content:flex-start",
        "justify-end" => "justify-content:flex-end",
        "justify-around" => "justify-content:space-around",
        "self-start" => "align-self:flex-start",
        "self-end" => "align-self:flex-end",
        "self-center" => "align-self:center",
        "self-stretch" => "align-self:stretch",
        "text-center" => "text-align:center",
        "text-left" => "text-align:left",
        "text-right" => "text-align:right",
        "font-bold" => "font-weight:700",
        "font-semibold" => "font-weight:600",
        "font-medium" => "font-weight:500",
        "font-normal" => "font-weight:400",
        "font-sans" => {
            "font-family:'Pretendard',ui-sans-serif,system-ui,-apple-system,'Segoe UI',sans-serif"
        }
        "font-mono" => {
            "font-family:'Pretendard',ui-monospace,'SF Mono','JetBrains Mono',Menlo,monospace"
        }
        "uppercase" => "text-transform:uppercase",
        "lowercase" => "text-transform:lowercase",
        "italic" => "font-style:italic",
        "underline" => "text-decoration-line:underline",
        "whitespace-pre" => "white-space:pre",
        "whitespace-pre-wrap" => "white-space:pre-wrap",
        "whitespace-nowrap" => "white-space:nowrap",
        "whitespace-normal" => "white-space:normal",
        "truncate" => "overflow:hidden;text-overflow:ellipsis;white-space:nowrap",
        "break-words" => "overflow-wrap:break-word",
        "break-all" => "word-break:break-all",
        "overflow-auto" => "overflow:auto",
        "overflow-hidden" => "overflow:hidden",
        "overflow-x-auto" => "overflow-x:auto",
        "overflow-y-auto" => "overflow-y:auto",
        "relative" => "position:relative",
        "absolute" => "position:absolute",
        "fixed" => "position:fixed",
        "sticky" => "position:sticky",
        "inset-0" => "top:0;right:0;bottom:0;left:0",
        "cursor-pointer" => "cursor:pointer",
        "cursor-default" => "cursor:default",
        "resize-none" => "resize:none",
        "outline-none" => "outline:none",
        "pointer-events-none" => "pointer-events:none",
        "pointer-events-auto" => "pointer-events:auto",
        "select-none" => "user-select:none",
        "box-border" => "box-sizing:border-box",
        "box-content" => "box-sizing:content-box",
        "tabular-nums" => "font-variant-numeric:tabular-nums",
        "w-full" => "width:100%",
        "h-full" => "height:100%",
        "w-screen" => "width:100vw",
        "h-screen" => "height:100vh",
        "w-auto" => "width:auto",
        "h-auto" => "height:auto",
        "min-h-0" => "min-height:0",
        "min-w-0" => "min-width:0",
        "max-w-full" => "max-width:100%",
        "rounded-none" => "border-radius:0",
        "rounded" => "border-radius:0.25rem",
        "rounded-md" => "border-radius:0.375rem",
        "rounded-lg" => "border-radius:0.5rem",
        "rounded-xl" => "border-radius:0.75rem",
        "rounded-full" => "border-radius:9999px",
        "border" => "border-width:1px;border-style:solid",
        "border-0" => "border-width:0",
        "border-t" => "border-top-width:1px;border-top-style:solid",
        "border-b" => "border-bottom-width:1px;border-bottom-style:solid",
        "border-l" => "border-left-width:1px;border-left-style:solid",
        "border-r" => "border-right-width:1px;border-right-style:solid",
        _ => "",
    };
    if !fixed.is_empty() {
        return Some(format!("{fixed};"));
    }

    // compound-prefix utilities (the prefix itself contains a hyphen)
    if let Some(v) = class.strip_prefix("min-h-") {
        return Some(format!("min-height:{};", size(v)?));
    }
    if let Some(v) = class.strip_prefix("min-w-") {
        return Some(format!("min-width:{};", size(v)?));
    }
    if let Some(v) = class.strip_prefix("max-w-") {
        return Some(format!("max-width:{};", size(v)?));
    }
    if let Some(v) = class.strip_prefix("max-h-") {
        return Some(format!("max-height:{};", size(v)?));
    }
    if let Some(v) = class.strip_prefix("gap-x-") {
        return Some(format!("column-gap:{};", space(v)?));
    }
    if let Some(v) = class.strip_prefix("gap-y-") {
        return Some(format!("row-gap:{};", space(v)?));
    }
    if let Some(v) = class.strip_prefix("grid-cols-") {
        return Some(format!(
            "grid-template-columns:repeat({},minmax(0,1fr));",
            v.parse::<u32>().ok()?
        ));
    }

    // patterned utilities: <prefix>-<value> (split on the first hyphen so color
    // shades like `zinc-900` stay intact as the value)
    let (prefix, val) = class.split_once('-')?;
    let css = match prefix {
        // spacing
        "p" => format!("padding:{}", space(val)?),
        "px" => format!("padding-left:{v};padding-right:{v}", v = space(val)?),
        "py" => format!("padding-top:{v};padding-bottom:{v}", v = space(val)?),
        "pt" => format!("padding-top:{}", space(val)?),
        "pr" => format!("padding-right:{}", space(val)?),
        "pb" => format!("padding-bottom:{}", space(val)?),
        "pl" => format!("padding-left:{}", space(val)?),
        "m" => format!("margin:{}", space(val)?),
        "mx" => format!("margin-left:{v};margin-right:{v}", v = space(val)?),
        "my" => format!("margin-top:{v};margin-bottom:{v}", v = space(val)?),
        "mt" => format!("margin-top:{}", space(val)?),
        "mr" => format!("margin-right:{}", space(val)?),
        "mb" => format!("margin-bottom:{}", space(val)?),
        "ml" => format!("margin-left:{}", space(val)?),
        "gap" => format!("gap:{}", space(val)?),
        // sizing
        "w" => format!("width:{}", size(val)?),
        "h" => format!("height:{}", size(val)?),
        "basis" => format!("flex-basis:{}", size(val)?),
        // text size / leading / tracking / weight / opacity / z / rounding side
        "text" => {
            if let Some(sz) = text_size(val) {
                format!("font-size:{sz}")
            } else {
                format!("color:{}", color(val)?)
            }
        }
        "leading" => format!("line-height:{}", leading(val)?),
        "tracking" => format!("letter-spacing:{}", tracking(val)?),
        "opacity" => format!("opacity:{}", val.parse::<f32>().ok()? / 100.0),
        "z" => format!("z-index:{}", val.parse::<i32>().ok()?),
        // colors
        "bg" => format!("background-color:{}", color(val)?),
        "border" => format!("border-color:{}", color(val)?),
        "caret" => format!("caret-color:{}", color(val)?),
        "rounded" if val == "sm" => "border-radius:0.125rem".into(),
        _ => return None,
    };
    Some(format!("{css};"))
}

/// Tailwind spacing scale → a rem/px length (`4` → `1rem`, `0.5` → `0.125rem`).
fn space(v: &str) -> Option<String> {
    Some(match v {
        "0" => "0".into(),
        "px" => "1px".into(),
        _ => {
            let n: f32 = v.parse().ok()?;
            format!("{}rem", n * 0.25)
        }
    })
}

/// Width/height value: spacing scale, plus `full`/`screen`/`auto`/fractions.
fn size(v: &str) -> Option<String> {
    Some(match v {
        "full" => "100%".into(),
        "screen" => "100vh".into(),
        "auto" => "auto".into(),
        "min" => "min-content".into(),
        "max" => "max-content".into(),
        "fit" => "fit-content".into(),
        _ if v.contains('/') => {
            let (a, b) = v.split_once('/')?;
            let (a, b): (f32, f32) = (a.parse().ok()?, b.parse().ok()?);
            format!("{:.4}%", a / b * 100.0)
        }
        _ => space(v)?,
    })
}

fn text_size(v: &str) -> Option<&'static str> {
    Some(match v {
        "xs" => "0.75rem",
        "sm" => "0.875rem",
        "base" => "1rem",
        "lg" => "1.125rem",
        "xl" => "1.25rem",
        "2xl" => "1.5rem",
        "3xl" => "1.875rem",
        _ => return None,
    })
}

fn leading(v: &str) -> Option<&'static str> {
    Some(match v {
        "none" => "1",
        "tight" => "1.25",
        "snug" => "1.375",
        "normal" => "1.5",
        "relaxed" => "1.625",
        "loose" => "2",
        _ => return None,
    })
}

fn tracking(v: &str) -> Option<&'static str> {
    Some(match v {
        "tight" => "-0.025em",
        "normal" => "0",
        "wide" => "0.025em",
        "wider" => "0.05em",
        "widest" => "0.1em",
        _ => return None,
    })
}

/// A compact color palette (name → hex). Covers `white`/`black`/`transparent`
/// and the shades used across the UI (zinc neutrals, a violet accent, and the
/// semantic hues). `text-`/`bg-`/`border-` all resolve through here.
fn color(v: &str) -> Option<&'static str> {
    Some(match v {
        "white" => "#ffffff",
        "black" => "#000000",
        "transparent" => "transparent",
        "current" => "currentColor",
        "zinc-50" => "#fafafa",
        "zinc-100" => "#f4f4f5",
        "zinc-200" => "#e4e4e7",
        "zinc-300" => "#d4d4d8",
        "zinc-400" => "#a1a1aa",
        "zinc-500" => "#71717a",
        "zinc-600" => "#52525b",
        "zinc-700" => "#3f3f46",
        "zinc-800" => "#27272a",
        "zinc-850" => "#1f1f23",
        "zinc-900" => "#18181b",
        "zinc-950" => "#0d0d0f",
        "violet-300" => "#c4b5fd",
        "violet-400" => "#a78bfa",
        "violet-500" => "#8b5cf6",
        "violet-600" => "#7c3aed",
        "cyan-400" => "#22d3ee",
        "emerald-400" => "#34d399",
        "emerald-500" => "#10b981",
        "rose-400" => "#fb7185",
        "rose-500" => "#f43f5e",
        "amber-400" => "#fbbf24",
        "amber-500" => "#f59e0b",
        "sky-400" => "#38bdf8",
        _ => return None,
    })
}

/// Escape a class name for a CSS selector (`/` in `w-1/2`, `.` in `p-1.5`).
fn css_escape(c: &str) -> String {
    c.replace('/', "\\/")
        .replace('.', "\\.")
        .replace(':', "\\:")
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

/// Walk a statement collecting whitespace-separated tokens of every string
/// literal as Tailwind class candidates.
fn collect_class_strings(s: &Stmt, out: &mut BTreeSet<String>) {
    match s {
        Stmt::Fn(f) => match &f.body {
            Some(FnBody::Expr(e)) => collect_class_expr(e, out),
            Some(FnBody::Block(stmts)) => stmts.iter().for_each(|s| collect_class_strings(s, out)),
            None => {}
        },
        Stmt::Bind(b) => collect_class_expr(&b.value, out),
        Stmt::Expr(e) => collect_class_expr(e, out),
        _ => {}
    }
}
fn collect_class_expr(e: &Expr, out: &mut BTreeSet<String>) {
    match e {
        Expr::Str(parts) => {
            for c in plain_text(parts).split_whitespace() {
                out.insert(c.to_string());
            }
            for p in parts {
                if let StrPart::Interp(x) = p {
                    collect_class_expr(x, out);
                }
            }
        }
        Expr::Call { callee, args } => {
            collect_class_expr(callee, out);
            for a in args {
                collect_class_expr(&arg_expr(a), out);
            }
        }
        Expr::Ternary { cond, then, els } => {
            collect_class_expr(cond, out);
            collect_class_expr(then, out);
            collect_class_expr(els, out);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_class_expr(lhs, out);
            collect_class_expr(rhs, out);
        }
        Expr::Unary { expr, .. }
        | Expr::Field { base: expr, .. }
        | Expr::Try(expr)
        | Expr::Fail(expr)
        | Expr::Reify(expr) => collect_class_expr(expr, out),
        Expr::Index { base, index } => {
            collect_class_expr(base, out);
            collect_class_expr(index, out);
        }
        Expr::If { cond, then, els } => {
            collect_class_expr(cond, out);
            then.iter().for_each(|s| collect_class_strings(s, out));
            if let Some(e) = els {
                e.iter().for_each(|s| collect_class_strings(s, out));
            }
        }
        Expr::Match { scrut, arms } => {
            collect_class_expr(scrut, out);
            for a in arms {
                collect_class_expr(&a.body, out);
            }
        }
        Expr::Block(stmts) | Expr::For { body: stmts, .. } | Expr::While { body: stmts, .. } => {
            stmts.iter().for_each(|s| collect_class_strings(s, out))
        }
        Expr::Lambda { body, .. } => collect_class_expr(body, out),
        Expr::List(es) => es.iter().for_each(|x| collect_class_expr(x, out)),
        Expr::With { base, fields } => {
            collect_class_expr(base, out);
            for f in fields {
                if let Field::Value { value, .. } = f {
                    collect_class_expr(value, out);
                }
            }
        }
        Expr::Record(fields) | Expr::Ctor { fields, .. } => {
            for f in fields {
                if let Field::Value { value, .. } = f {
                    collect_class_expr(value, out);
                }
            }
        }
        Expr::Assign { target, value } => {
            collect_class_expr(target, out);
            collect_class_expr(value, out);
        }
        _ => {}
    }
}

fn is_record_type(e: &Expr) -> bool {
    matches!(e, Expr::Record(fs) if !fs.is_empty() && fs.iter().all(|f| matches!(f, Field::Type { .. })))
}

fn sum_variants(e: &Expr) -> Option<()> {
    matches!(
        e,
        Expr::Binary {
            op: BinOp::Union,
            ..
        }
    )
    .then_some(())
}
