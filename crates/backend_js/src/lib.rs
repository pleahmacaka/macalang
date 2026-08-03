//! maca-backend-js: lower a UI component to vanilla-DOM JS + HTML + CSS.
//!
//! Elements are functions (`div(class=…, …children)`); top-level bindings are
//! reactive state (Svelte-style compile-time reactivity: assigning to state and
//! calling `update()` re-syncs bound nodes). `bind:value` is two-way,
//! `on:event` wires a handler. `class` utility names are collected and a
//! tree-shaken Tailwind subset is emitted (only used classes ship).

use maca_parser::ast::*;
use std::collections::{BTreeMap, BTreeSet};

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
            if b.is_const {
                cx.consts.insert(name.clone());
            }
        }
    }
    let state_names: BTreeSet<String> = cx.state.iter().map(|(n, _)| n.clone()).collect();
    cx.state_names = state_names.clone();
    set_state_names(&state_names);

    let variants = collect_variants(m);
    set_variants(&variants);

    // Collect Tailwind class candidates from every string literal in the module
    // (not just `class=` attributes), so classes returned from a helper, e.g.
    // `tab(n) => n == active ? "px-2 bg-zinc-800" : "px-2"`, still emit CSS.
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
    //
    // A function *declared* without a body is the other half of the boundary:
    // the Maca side says what it takes and returns, the host supplies it. It
    // gets a stub that routes through the bridge, so the implementation arrives
    // by `maca.provide({ f })` rather than by the block assigning `window.f`
    // and hoping a bare call finds it on the global object.
    let mut exports = Vec::new();
    let mut fn_defs = String::new();
    let mut host_stubs = String::new();
    let mut hosts = Vec::new();
    for item in &m.items {
        if let Stmt::Fn(f) = item {
            if f.name == "main" {
                continue;
            }
            if f.body.is_none() {
                host_stubs.push_str(&host_stub(f));
                hosts.push(f.name.clone());
                continue;
            }
            fn_defs.push_str(&emit_fn(f));
            fn_defs.push('\n');
            exports.push(f.name.clone());
        }
    }
    // Variants are exported alongside the functions: a caller in JS cannot build
    // an argument for `area(s: Shape)` without `Circle`.
    exports.extend(variants.keys().cloned());
    if !host_stubs.is_empty() {
        js.push_str("\n// ---- host functions (declared here, provided by the host) ----\n");
        js.push_str(&host_stubs);
    }
    if !fn_defs.is_empty() || !variants.is_empty() {
        if !fn_defs.is_empty() {
            js.push_str("\n// ---- transpiled functions ----\n");
            js.push_str(&fn_defs);
        }
        let names = exports.join(", ");
        js.push_str(&format!(
            "if (typeof module !== \"undefined\") Object.assign(module.exports, {{ {names} }});\n"
        ));
    }

    // foreign blocks embedded in the .maca source: `import js """…"""` carries
    // raw JS (the host/runtime glue a UI app needs), `import css """…"""` raw
    // CSS. This lets a single .maca file carry everything it needs. The JS goes
    // *between the bridge and the app*: after `maca`, so a block can call
    // `maca.provide(…)` and `maca.set(…)` at its top level, and before the app,
    // so whatever it provides is in place by the time `mount()` builds the view.
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
    let bridge = cx.bridge(&hosts);
    let js = if foreign_js.is_empty() {
        format!("{bridge}{js}")
    } else {
        format!("{bridge}\n{foreign_js}\n{js}")
    };
    // Constructors go near the top: `build()` mounts the view as soon as it is
    // defined, and a view may name a variant. They go *after* `"use strict"`
    // rather than before it: a directive prologue only counts as one while
    // it is still the first statement in the file.
    let js = match variant_ctors(&variants) {
        c if c.is_empty() => js,
        c => insert_after_use_strict(&js, &format!("// ---- sum variants ----\n{c}")),
    };
    // Method helpers go above the variants, which may themselves be built from
    // a method call in a view.
    let js = match method_helpers_for(&js) {
        h if h.is_empty() => js,
        h => insert_after_use_strict(&js, &format!("// ---- method helpers ----\n{h}")),
    };
    let html = HTML.into();
    JsOut {
        js,
        html,
        css,
        exports,
    }
}

/// Splice `block` in just below the `"use strict"` directive, or at the very top
/// when there is none.
fn insert_after_use_strict(js: &str, block: &str) -> String {
    match js.split_once('\n') {
        Some((first, rest)) if first.trim_start().starts_with("\"use strict\"") => {
            format!("{first}\n{block}\n{rest}")
        }
        _ => format!("{block}\n{js}"),
    }
}

/// The bridge itself: the fixed half of what `Cx::bridge` emits, above the
/// three generated name lists it reads (`_vars`, `_consts`, `_declared`).
///
/// It is the answer to two undocumented conventions. A foreign block used to
/// reach for `state` and `update()`, which are this generator's own locals and
/// were never promised to anybody, and it had to write `window.f = …` for every
/// function the Maca side declared, because a Maca call lowers to a bare call
/// that resolves on the global object. One project carried twenty two of those
/// assignments, and a mistyped `state.form_titel = …` silently created a field
/// nothing was bound to, so the dialog it filled in simply stayed blank.
const BRIDGE_JS: &str = r#"// The documented boundary between this program and any `import js` block:
//
//   maca.get(name)           read a name the program declared
//   maca.set(name, value)    write it, then refresh the view
//   maca.set({ a, b })       write several, refresh once
//   maca.refresh()           re-sync the bound nodes after something else moved
//   maca.provide({ f })      supply a function the Maca side declared
//
// Nothing else in this file is an interface.
const _hosts = Object.create(null);
function _declaredState() { return _vars.concat(_consts).join(", ") || "(none)"; }
function _readable(name) {
  if (_vars.indexOf(name) < 0 && _consts.indexOf(name) < 0) {
    throw new Error(`maca.get: \`${name}\` is not state in this program; declared: ${_declaredState()}`);
  }
}
function _writable(name) {
  if (_vars.indexOf(name) >= 0) return;
  if (_consts.indexOf(name) >= 0) {
    throw new Error(`maca.set: \`${name}\` is a constant`);
  }
  throw new Error(`maca.set: \`${name}\` is not state in this program; declared: ${_declaredState()}`);
}
const maca = {
  get(name) { _readable(name); return state[name]; },
  // Every name is checked before any is written: a typo in one field of a
  // `set({…})` must not leave the other fields half applied.
  set(name, value) {
    const pairs = typeof name === "string" ? [[name, value]] : Object.entries(name);
    for (const pair of pairs) _writable(pair[0]);
    for (const pair of pairs) state[pair[0]] = pair[1];
    update();
  },
  refresh() { update(); },
  // The other direction: hand back the functions Maca declared without a body.
  // They are ordinary calls over there, so this is what makes them resolve.
  provide(table) {
    for (const key of Object.keys(table)) {
      if (_declared.indexOf(key) < 0) {
        throw new Error(`maca.provide: \`${key}\` is not declared in this program; declared: ${_declared.join(", ") || "(none)"}`);
      }
      if (typeof table[key] !== "function") {
        throw new TypeError(`maca.provide: \`${key}\` must be a function`);
      }
      _hosts[key] = table[key];
    }
  },
};
// One call to a declared-but-not-defined function. A missing implementation
// names itself here, rather than reaching the view as `undefined is not a
// function` from wherever the value ended up being used.
function _host(name, args) {
  const f = _hosts[name];
  if (typeof f !== "function") {
    throw new Error(`maca: \`${name}\` is declared in Maca but nothing implements it; call maca.provide({ ${name}: … }) from the import js block`);
  }
  return f.apply(null, args);
}
"#;

/// A signature with no body → a stub that routes the call through the bridge.
///
/// Rest arguments rather than the declared parameter names, so a variadic
/// parameter needs no second spelling; the signature is in the comment above
/// it, which is where a reader of the emitted JS looks for it anyway.
fn host_stub(f: &FnDef) -> String {
    let params = f
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let name = &f.name;
    format!(
        "// declared in Maca as `{name}({params})`; \
         supply it with `maca.provide({{ {name} }})`\n\
         function {name}(...args) {{ return _host(\"{name}\", args); }}\n"
    )
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

/// Where the value of a statement-position expression has to end up.
///
/// This is the C backend's `Sink` by another name, and it exists for the same
/// reason. `if`, `match` and a block are *expressions* in Maca and *statements*
/// in JS, and only one of those two facts can be honoured by the shape of the
/// emitted code. Lowering them as expressions means an IIFE, and an IIFE is a
/// function boundary: `break` and `continue` cannot cross it (a SyntaxError),
/// and a `var` written inside it declares a fresh local instead of assigning
/// the enclosing one (a wrong answer, silently). So the value is routed by
/// deciding, where the statement is emitted and the answer is known, what is to
/// be done with what the branches produce.
#[derive(Clone)]
enum Sink {
    /// The value is not wanted: emit it as a bare statement.
    Discard,
    /// The value is the function's result.
    Return,
    /// The value is written to this JS lvalue.
    Assign(String),
}

/// A block of statements → JS, returning the value of the final expression.
fn jblock(stmts: &[Stmt]) -> String {
    jblock_ret(stmts, true)
}

/// `ret` = whether the final expression is `return`ed (function body) or emitted
/// as a bare statement (loop body).
fn jblock_ret(stmts: &[Stmt], ret: bool) -> String {
    jblock_sink(stmts, &if ret { Sink::Return } else { Sink::Discard })
}

/// A block whose final expression goes to `sink`; everything before it is
/// evaluated for its effect.
fn jblock_sink(stmts: &[Stmt], sink: &Sink) -> String {
    let mut out = String::new();
    for (i, s) in stmts.iter().enumerate() {
        let last = i + 1 == stmts.len();
        let here = if last { sink.clone() } else { Sink::Discard };
        match s {
            Stmt::Bind(b) => out.push_str(&jbind(b)),
            Stmt::Expr(e) => out.push_str(&jstmt(e, &here)),
            Stmt::Fn(f) => out.push_str(&emit_fn(f)),
            _ => {}
        }
    }
    out
}

/// A binding statement. A control-flow value is not built and then assigned:
/// its branches assign the name directly, so a branch that also writes an
/// enclosing local writes *that* local.
fn jbind(b: &Bind) -> String {
    match &b.target {
        Expr::Ident(n) => {
            // reactive state resolves to `state.x`; a local is declared with
            // `var` (which, unlike `let`, tolerates the redeclaration a bare
            // `x =` reassignment would otherwise produce).
            let is_state = STATE.with(|s| s.borrow().contains(n));
            if is_branching(&b.value) {
                let decl = if is_state {
                    String::new()
                } else {
                    format!("  var {n};\n")
                };
                format!("{decl}{}", jstmt(&b.value, &Sink::Assign(jname(n))))
            } else if is_state {
                format!("  {} = {};\n", jname(n), jexpr(&b.value))
            } else {
                format!("  var {n} = {};\n", jexpr(&b.value))
            }
        }
        // lvalue assignment: `xs[i] = v`, `p.field = v`
        t if is_branching(&b.value) => jstmt(&b.value, &Sink::Assign(jexpr(t))),
        t => format!("  {} = {};\n", jexpr(t), jexpr(&b.value)),
    }
}

/// Does this expression choose between blocks, so that lowering it as a value
/// would put those blocks inside an IIFE?
fn is_branching(e: &Expr) -> bool {
    matches!(e, Expr::If { .. } | Expr::Match { .. } | Expr::Block(_))
}

/// Hand one already-lowered JS expression to its sink.
fn deliver(js: &str, sink: &Sink) -> String {
    match sink {
        Sink::Discard => format!("  {js};\n"),
        Sink::Return => format!("  return {js};\n"),
        Sink::Assign(t) => format!("  {t} = {js};\n"),
    }
}

/// One expression in statement position → JS statements.
fn jstmt(e: &Expr, sink: &Sink) -> String {
    match e {
        Expr::If { cond, then, els } => {
            let mut out = format!("  if ({}) {{\n{}  }}", jexpr(cond), jblock_sink(then, sink));
            match els {
                Some(b) => out.push_str(&format!(" else {{\n{}  }}\n", jblock_sink(b, sink))),
                // An `if` with no `else` still owes its sink a value when the
                // condition is false; `null` is what the ternary lowering used
                // to hand back for the missing branch.
                None => out.push_str(&match sink {
                    Sink::Discard => "\n".to_string(),
                    s => format!(" else {{\n{}  }}\n", deliver("null", s)),
                }),
            }
            out
        }
        Expr::Match { scrut, arms } => jmatch_stmt(scrut, arms, sink),
        Expr::Block(stmts) => format!("  {{\n{}  }}\n", jblock_sink(stmts, sink)),
        // A loop is a statement in both languages, and its value is unit, so it
        // is emitted for its effect whatever the sink is.
        Expr::While { cond, body } => format!(
            "  while ({}) {{\n{}  }}\n",
            jexpr(cond),
            jblock_sink(body, &Sink::Discard)
        ),
        Expr::For { pat, iter, body } => jfor(pat, iter, body),
        Expr::Break => "  break;\n".into(),
        Expr::Continue => "  continue;\n".into(),
        _ => deliver(&jexpr(e), sink),
    }
}

/// `for pat in iter { … }` → a `for … of` loop.
///
/// The loop variable is declared `var`, not `const` or `let`: a body that
/// reassigns it emits `var x = …`, and a lexical binding of the same name in
/// the loop head makes that a SyntaxError.
fn jfor(pat: &Pattern, iter: &Expr, body: &[Stmt]) -> String {
    let (name, binds) = match pat {
        Pattern::Bind(n) if !is_variant(n) => (n.clone(), String::new()),
        p => ("_it".to_string(), jpattern(p, "_it").1),
    };
    let mut out = format!("  for (var {name} of {}) {{\n", jexpr(iter));
    if !binds.is_empty() {
        out.push_str(&format!("  {binds}\n"));
    }
    out.push_str(&jblock_sink(body, &Sink::Discard));
    out.push_str("  }\n");
    out
}

/// `match` in statement position → a real `if`/`else if` chain, so an arm may
/// `break`, `continue`, or assign a name the enclosing function owns.
///
/// Wrapped in a block of its own, and the scrutinee held in a block-scoped
/// `const`, so that neither a sibling nor an enclosing `match` can be answered
/// with the wrong value. `$` is what keeps the name to itself: a Maca
/// identifier is `[_A-Za-z][_A-Za-z0-9]*`, so no local the arm bodies declare
/// can hoist a `var` of the same name over it.
fn jmatch_stmt(scrut: &Expr, arms: &[Arm], sink: &Sink) -> String {
    let sv = "_s$";
    let mut out = format!("  {{\n  const {sv} = {};\n", jexpr(scrut));
    let mut chained = false;
    for a in arms {
        let (cond, binds) = jpattern(&a.pat, sv);
        // the guard reads the arm's own bindings, so it belongs inside the
        // branch, after them, not in the condition that selects the branch.
        let (test, pre) = match &a.guard {
            Some(g) => (
                format!("{cond} && (() => {{ {binds}return {}; }})()", jexpr(g)),
                binds.clone(),
            ),
            None => (cond, binds),
        };
        let kw = if chained { "  } else if" } else { "  if" };
        chained = true;
        out.push_str(&format!("{kw} ({test}) {{\n"));
        if !pre.is_empty() {
            out.push_str(&format!("  {pre}\n"));
        }
        out.push_str(&jstmt(&a.body, sink));
    }
    // Falling off the end throws rather than leaving the sink unwritten: a
    // scrutinee no arm covers is a bug in the program, and `undefined` would
    // carry it somewhere else first.
    let no_match = "  throw new Error(\"no match\");\n";
    if chained {
        out.push_str(&format!("  }} else {{\n{no_match}  }}\n"));
    } else {
        out.push_str(no_match);
    }
    out.push_str("  }\n");
    out
}

thread_local! {
    /// Top-level reactive-state names, consulted so a reference to `x` lowers to
    /// `state.x` everywhere (view text, attributes, and transpiled functions).
    static STATE: std::cell::RefCell<BTreeSet<String>> = const { std::cell::RefCell::new(BTreeSet::new()) };
    /// Sum-type variant names and their payload arity. A pattern cannot tell a
    /// nullary variant (`Red`) from a binder by shape alone, since both parse
    /// as `Pattern::Bind`, so the declared set is what disambiguates them, the way
    /// the checker's `is_variant` does for the C backend.
    static VARIANTS: std::cell::RefCell<BTreeMap<String, usize>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}
fn set_state_names(names: &BTreeSet<String>) {
    STATE.with(|s| *s.borrow_mut() = names.clone());
}
fn set_variants(vs: &BTreeMap<String, usize>) {
    VARIANTS.with(|s| *s.borrow_mut() = vs.clone());
}
fn is_variant(n: &str) -> bool {
    VARIANTS.with(|s| s.borrow().contains_key(n))
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
        // `lo..hi` is an inclusive integer range as a JS array (lo … hi).
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
        Expr::Lambda { params, body, .. } => {
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

/// `match` in expression position → an IIFE with an if-chain. Falling off the
/// end throws rather than returning `undefined`: a scrutinee no arm covers is a
/// bug in the program, and `undefined` would carry it somewhere else first.
fn jmatch(scrut: &Expr, arms: &[Arm]) -> String {
    let mut body = format!("(() => {{ const _s = {};", jexpr(scrut));
    for a in arms {
        let (cond, binds) = jpattern(&a.pat, "_s");
        // the guard reads the arm's own bindings, so it belongs inside the
        // branch, after them, not in the condition that selects the branch.
        let (test, pre) = match &a.guard {
            Some(g) => (
                format!("{cond} && (() => {{ {binds}return {}; }})()", jexpr(g)),
                binds.clone(),
            ),
            None => (cond, binds),
        };
        body.push_str(&format!(
            " if ({test}) {{ {pre}return {}; }}",
            jexpr(&a.body)
        ));
    }
    body.push_str(" throw new Error(\"no match\"); })()");
    body
}

/// (condition, binding-statements) for matching `sv` against `pat`. `sv` is the
/// JS expression holding the value, so nested patterns recurse into a field, an
/// element, or a payload slot rather than only ever testing the scrutinee.
fn jpattern(pat: &Pattern, sv: &str) -> (String, String) {
    match pat {
        Pattern::Wild => ("true".into(), String::new()),
        // A bare capitalised name is a nullary variant when one is declared, and
        // a binder otherwise: the same ambiguity the C backend resolves with
        // the checker's `is_variant`.
        Pattern::Bind(n) if is_variant(n) => (format!("{sv}.$ === {n:?}"), String::new()),
        Pattern::Bind(n) => ("true".into(), format!("var {n} = {sv}; ")),
        Pattern::Int(n) => (format!("{sv} === {n}"), String::new()),
        Pattern::Float(f) => (format!("{sv} === {f}"), String::new()),
        Pattern::Bool(b) => (format!("{sv} === {b}"), String::new()),
        Pattern::Str(s) => (format!("{sv} === {s:?}"), String::new()),
        Pattern::Ctor { name, args } => {
            let mut conds = vec![format!("{sv}.$ === {name:?}")];
            let mut binds = String::new();
            for (i, a) in args.iter().enumerate() {
                let (c, b) = jpattern(a, &format!("{sv}._{i}"));
                if c != "true" {
                    conds.push(c);
                }
                binds.push_str(&b);
            }
            (format!("({})", conds.join(" && ")), binds)
        }
        // A record pattern always matches: it exists to name the fields.
        Pattern::Record(fields) => {
            let mut conds = Vec::new();
            let mut binds = String::new();
            for (fname, sub) in fields {
                match sub {
                    None => binds.push_str(&format!("var {fname} = {sv}.{fname}; ")),
                    Some(p) => {
                        let (c, b) = jpattern(p, &format!("{sv}.{fname}"));
                        if c != "true" {
                            conds.push(c);
                        }
                        binds.push_str(&b);
                    }
                }
            }
            let cond = if conds.is_empty() {
                "true".into()
            } else {
                format!("({})", conds.join(" && "))
            };
            (cond, binds)
        }
        // `[a, b]` matches that length exactly; `a, ..rest` matches at least the
        // fixed elements and binds the remainder.
        Pattern::List { elems, rest } => {
            let n = elems.len();
            let op = if rest.is_some() { ">=" } else { "===" };
            let mut conds = vec![format!("Array.isArray({sv}) && {sv}.length {op} {n}")];
            let mut binds = String::new();
            for (i, e) in elems.iter().enumerate() {
                let (c, b) = jpattern(e, &format!("{sv}[{i}]"));
                if c != "true" {
                    conds.push(c);
                }
                binds.push_str(&b);
            }
            // a named rest binds the tail; `..` alone binds nothing
            if let Some(Pattern::Bind(rn)) = rest.as_deref() {
                binds.push_str(&format!("var {rn} = {sv}.slice({n}); "));
            }
            (format!("({})", conds.join(" && ")), binds)
        }
        // Alternatives bind nothing, so each is only a test.
        Pattern::Or(alts) => {
            let conds: Vec<String> = alts.iter().map(|p| jpattern(p, sv).0).collect();
            (format!("({})", conds.join(" || ")), String::new())
        }
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
        Expr::Lambda { params, ret, body } if !params.iter().any(|p| p.name == from) => {
            Expr::Lambda {
                params: params.clone(),
                ret: ret.clone(),
                body: go(body),
            }
        }
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
        // `Pipe` is desugared by the parser and never arrives here.
        Union | Pipe => return l,
    };
    format!("({l} {o} {r})")
}

fn jcall(callee: &Expr, args: &[Arg]) -> String {
    let a: Vec<String> = args.iter().map(|x| jexpr(arg_expr(x))).collect();
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
        // A byte and the string holding it, matching the native pair: `chr(0)`
        // is empty rather than a NUL, and `ord("")` is -1.
        Expr::Ident(f) if f == "chr" => format!(
            "((_b)=>_b>0&&_b<256?String.fromCharCode(_b):\"\")({})",
            a.first().cloned().unwrap_or_default()
        ),
        Expr::Ident(f) if f == "ord" => format!(
            "((_s)=>_s&&_s.length?_s.charCodeAt(0):-1)({})",
            a.first().cloned().unwrap_or_default()
        ),
        Expr::Ident(f) => format!("{f}({})", a.join(", ")),
        Expr::Field { base, name } => match method(name, &jexpr(base), &a) {
            Some(js) => js,
            // Not one of Maca's methods, so it is a field holding a function or
            // a foreign JS call, so pass it through untouched.
            None => format!("{}.{name}({})", jexpr(base), a.join(", ")),
        },
        _ => format!("{}({})", jexpr(callee), a.join(", ")),
    }
}

/// One of Maca's UFCS methods → JS that computes the same value.
///
/// Handing the name straight to JS was wrong twice over. Some do not exist
/// there: `.length` is a property, so `xs.length()` threw. The dangerous ones
/// are those that exist and mean something else: `push` returns the new
/// *length*, `sort` compares as strings so `[10, 9]` stays `[10, 9]`, and both
/// `sort` and `reverse` mutate the receiver. Maca lists and strings are values,
/// so every lowering here is non-mutating even where the answer would have
/// matched. A second holder of that list must not see the change.
///
/// The receiver's type is not known at this point (the JS backend is untyped),
/// so a name shared by both sets lowers to a helper that works for either.
fn method(name: &str, recv: &str, args: &[String]) -> Option<String> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_else(|| "undefined".into());
    let out = match name {
        // shared by `str` and `T[]`
        "length" => format!("({recv}).length"),
        "slice" => format!("({recv}).slice({}, {})", arg(0), arg(1)),
        "index_of" => format!("({recv}).indexOf({})", arg(0)),
        "contains" => format!("_mhas({recv}, {})", arg(0)),

        // `str`
        "upper" => format!("({recv}).toUpperCase()"),
        "lower" => format!("({recv}).toLowerCase()"),
        "trim" => format!("({recv}).trim()"),
        "starts_with" => format!("({recv}).startsWith({})", arg(0)),
        "ends_with" => format!("({recv}).endsWith({})", arg(0)),
        // every occurrence, not the first, which is what JS `replace` does
        "replace" => format!("({recv}).split({}).join({})", arg(0), arg(1)),
        // `substr(start, len)`, so it is not JS `slice(start, end)`
        "substr" => format!("_msubstr({recv}, {}, {})", arg(0), arg(1)),
        "repeat" => format!("({recv}).repeat({})", arg(0)),
        "pad_start" => format!("_mpad({recv}, {}, {}, 0)", arg(0), arg(1)),
        "pad_end" => format!("_mpad({recv}, {}, {}, 1)", arg(0), arg(1)),
        "pad_center" => format!("_mpad({recv}, {}, {}, 2)", arg(0), arg(1)),
        "split" => format!("({recv}).split({})", arg(0)),
        "chars" => format!("Array.from({recv})"),
        "at" => format!("(({recv})[{}] ?? \"\")", arg(0)),
        "is_whitespace" => format!("_mclass({recv}, 0)"),
        "is_ascii_digit" => format!("_mclass({recv}, 1)"),
        "is_alpha" => format!("_mclass({recv}, 2)"),

        // `T[]`
        "map" => format!("({recv}).map({})", arg(0)),
        "filter" => format!("({recv}).filter({})", arg(0)),
        // `reduce(init, f)` and `fold(init, f)` take the seed first
        "reduce" | "fold" => format!("_mfold({recv}, {}, {})", arg(0), arg(1)),
        "sort" => format!("_msort({recv})"),
        "reverse" => format!("[...({recv})].reverse()"),
        "push" => format!("[...({recv}), {}]", arg(0)),
        "pop" => format!("({recv}).slice(0, -1)"),
        "sum" => format!("({recv}).reduce((_a, _b) => _a + _b, 0)"),
        "min" => format!("_mpick({recv}, -1)"),
        "max" => format!("_mpick({recv}, 1)"),
        "first" => format!("({recv})[0]"),
        "last" => format!("_mlast({recv})"),
        "get" => format!("({recv})[{}]", arg(0)),
        "join" => format!("({recv}).join({})", arg(0)),
        // The results are the same either way, so running the closure over each
        // element in turn is the whole of it on a single-threaded host.
        "parallel" => format!("({recv}).map({})", arg(0)),
        _ => return None,
    };
    Some(out)
}

/// Helpers the method lowerings call, emitted only when one is used.
const METHOD_HELPERS: &[(&str, &str)] = &[
    (
        "_mhas",
        "function _mhas(x, v) { return typeof x === \"string\" ? x.includes(v) : x.indexOf(v) >= 0; }",
    ),
    (
        "_msubstr",
        "function _msubstr(s, a, n) { return s.slice(a, a + n); }",
    ),
    // `pad_center` splits the shortfall left-biased, matching the runtime.
    (
        "_mpad",
        "function _mpad(s, w, p, mode) {\n  const n = w - s.length;\n  if (n <= 0) return s;\n  const fill = (c) => p.repeat(Math.ceil(c / p.length)).slice(0, c);\n  if (mode === 0) return fill(n) + s;\n  if (mode === 1) return s + fill(n);\n  const l = Math.floor(n / 2);\n  return fill(l) + s + fill(n - l);\n}",
    ),
    (
        "_mclass",
        "function _mclass(s, kind) {\n  const c = s[0] ?? \"\";\n  if (kind === 0) return /\\s/.test(c);\n  if (kind === 1) return c >= \"0\" && c <= \"9\";\n  return /[A-Za-z]/.test(c);\n}",
    ),
    (
        "_mfold",
        "function _mfold(xs, init, f) { let a = init; for (const x of xs) a = f(a, x); return a; }",
    ),
    // Numbers compare numerically; JS's default sort would order 10 before 9.
    (
        "_mcmp",
        "function _mcmp(a, b) { return typeof a === \"number\" ? a - b : (a < b ? -1 : a > b ? 1 : 0); }",
    ),
    (
        "_msort",
        "function _msort(xs) { return [...xs].sort(_mcmp); }",
    ),
    (
        "_mpick",
        "function _mpick(xs, dir) { return xs.reduce((a, b) => (_mcmp(b, a) * dir > 0 ? b : a)); }",
    ),
    (
        "_mlast",
        "function _mlast(xs) { return xs[xs.length - 1]; }",
    ),
];

/// The helper definitions `js` actually calls, in dependency order.
fn method_helpers_for(js: &str) -> String {
    let mut out = String::new();
    for (name, def) in METHOD_HELPERS {
        // `_mcmp` is reached through `_msort`/`_mpick` rather than by a lowering.
        let needed = js.contains(&format!("{name}("))
            || (*name == "_mcmp" && (js.contains("_msort(") || js.contains("_mpick(")));
        if needed && !out.contains(def) {
            out.push_str(def);
            out.push('\n');
        }
    }
    out
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
    /// The subset of `state` bound with `const` (or a Capitalized name). The
    /// bridge refuses to write one, so `maca.set("Title", …)` is the same
    /// error from JS that reassigning it is from Maca.
    consts: BTreeSet<String>,
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
        // else, such as a literal, an identifier, or a call to a
        // *text-returning* function like `mcTab(tab)`, is a (reactive) text
        // node.
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
                    // `html=expr` sets innerHTML (reactively), for pre-rendered
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
                    // A bool toggles the attribute's presence rather than its
                    // text. `_attr` decides which at run time, because the JS
                    // side has no types to decide it earlier.
                    let expr = jexpr(value);
                    out.push_str(&format!("  _attr({v}, \"{name}\", {expr});\n"));
                    if is_dynamic(value) {
                        out.push_str(&format!(
                            "  _binds.push(() => {{ _attr({v}, \"{name}\", {expr}); }});\n"
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
            Expr::Lambda { params, body, .. } => {
                // `v => age = int(v)`: bound var is the assignment target
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
    /// index, match, etc., not just literals. `subst` replaces a lambda
    /// parameter (the event's `$v`) throughout the expression first.
    fn value(&self, e: &Expr, subst: Option<(&str, &str)>) -> String {
        match subst {
            Some((from, to)) => jexpr(&subst_ident(e, from, to)),
            None => jexpr(e),
        }
    }

    /// The half of the module that has to exist before an `import js` block
    /// runs: the state object, the bind list, and the `maca` object the block
    /// talks to.
    ///
    /// It is emitted first for a reason. A block calling `maca.provide(…)` at
    /// its top level would otherwise hit the temporal dead zone of a `const
    /// maca` declared below it, and fail with a ReferenceError naming nothing
    /// the author wrote.
    fn bridge(&self, hosts: &[String]) -> String {
        let state = self
            .state
            .iter()
            .map(|(n, v)| format!("{n}: {v}"))
            .collect::<Vec<_>>()
            .join(", ");
        let names = |ns: Vec<&String>| {
            ns.iter()
                .map(|n| format!("{n:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let vars = names(
            self.state
                .iter()
                .map(|(n, _)| n)
                .filter(|n| !self.consts.contains(*n))
                .collect(),
        );
        let consts = names(
            self.state
                .iter()
                .map(|(n, _)| n)
                .filter(|n| self.consts.contains(*n))
                .collect(),
        );
        let declared = names(hosts.iter().collect());
        format!(
            "\"use strict\";\n\
             const state = {{ {state} }};\n\
             const _binds = [];\n\
             // ---- the maca bridge ----\n\
             const _vars = [{vars}];\n\
             const _consts = [{consts}];\n\
             const _declared = [{declared}];\n\
             {BRIDGE_JS}"
        )
    }

    fn finish(&self, build_body: &str, root: &str) -> String {
        format!(
            "// ---- the app ----\n\
             function _attr(el, k, v) {{\n\
             \x20 if (v === true) el.setAttribute(k, \"\");\n\
             \x20 else if (v === false || v == null) el.removeAttribute(k);\n\
             \x20 else el.setAttribute(k, v);\n\
             }}\n\
             function build() {{\n{build_body}  return {root};\n}}\n\
             function update() {{ for (const b of _binds) b(); }}\n\
             let _root = null;\n\
             function mount(target) {{ _root = build(); target.appendChild(_root); return _root; }}\n\
             if (typeof document !== \"undefined\" && document.getElementById) {{\n\
             \x20 const app = document.getElementById(\"app\");\n\
             \x20 if (app) mount(app);\n\
             }}\n\
             if (typeof module !== \"undefined\") module.exports = {{ state, mount, build, update, maca }};\n"
        )
    }

    fn css(&self) -> String {
        let mut out = String::from(
            "/* generated Tailwind subset (tree-shaken) */\n\
             *,*::before,*::after{box-sizing:border-box}\nhtml,body{margin:0}\n",
        );
        let mut sorted: Vec<&String> = self.classes.iter().collect();
        sorted.sort_by_key(|c| (order(c), (*c).clone()));
        for c in sorted {
            if let Some(r) = rule(c) {
                out.push_str(&r);
                out.push('\n');
            }
        }
        out
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

/// Where a class's rule belongs in the sheet.
///
/// CSS resolves a tie in specificity by source order, so a variant has to come
/// *after* the plain utility it overrides. Otherwise `max-md:block` loses to
/// `grid` and the narrow layout silently never applies. Plain utilities first,
/// then state variants, then media queries ordered so the more specific
/// breakpoint wins.
pub fn order(class: &str) -> (u8, i32) {
    let (variants, _) = split_variants(class);
    if variants.is_empty() {
        return (0, 0);
    }
    let mut layer = 1u8;
    let mut width = 0i32;
    for v in &variants {
        match *v {
            "sm" => (layer, width) = (2, 40),
            "md" => (layer, width) = (2, 48),
            "lg" => (layer, width) = (2, 64),
            "xl" => (layer, width) = (2, 80),
            // a max-width query is more specific the *smaller* it is, so it
            // sorts the other way round
            "max-lg" => (layer, width) = (3, -64),
            "max-md" => (layer, width) = (3, -48),
            "max-sm" => (layer, width) = (3, -40),
            "dark" => layer = layer.max(2),
            _ => {}
        }
    }
    (layer, width)
}

/// A whole CSS rule for one utility class, variants included.
///
/// A variant is a `name:` prefix that changes *when* the rule applies rather
/// than what it does, and they chain: `dark:hover:bg-zinc-800` is the hover
/// colour in dark mode. Without them a utility system can only express the
/// unconditional case, which is why anything with a theme, a hover state or a
/// breakpoint had to fall back to hand-written CSS.
///
/// Returns the rule text, including any `@media` wrapper, or `None` when the
/// class isn't one we generate.
pub fn rule(class: &str) -> Option<String> {
    let (variants, base) = split_variants(class);
    let body = tailwind(base)?;
    let mut selector = format!(".{}", css_escape(class));
    let mut media: Vec<&str> = Vec::new();
    for v in &variants {
        match *v {
            "hover" => selector.push_str(":hover"),
            "focus" => selector.push_str(":focus"),
            "active" => selector.push_str(":active"),
            "first" => selector.push_str(":first-child"),
            "last" => selector.push_str(":last-child"),
            "open" => selector.push_str("[open]"),
            "before" => selector.push_str("::before"),
            "after" => selector.push_str("::after"),
            "marker" => selector.push_str("::marker"),
            // WebKit draws its own triangle on `<summary>` and ignores
            // `list-style`, so hiding it needs the vendor pseudo-element
            "details-marker" => selector.push_str("::-webkit-details-marker"),
            "placeholder" => selector.push_str("::placeholder"),
            "dark" => media.push("(prefers-color-scheme:dark)"),
            "sm" => media.push("(min-width:40rem)"),
            "md" => media.push("(min-width:48rem)"),
            "lg" => media.push("(min-width:64rem)"),
            "xl" => media.push("(min-width:80rem)"),
            // a max-width breakpoint, for styling *down* from a size
            "max-sm" => media.push("(max-width:40rem)"),
            "max-md" => media.push("(max-width:48rem)"),
            "max-lg" => media.push("(max-width:64rem)"),
            _ => return None,
        }
    }
    let rule = format!("{selector} {{ {body} }}");
    Some(if media.is_empty() {
        rule
    } else {
        format!("@media{} {{ {rule} }}", media.join(" and "))
    })
}

/// Split `dark:hover:bg-x` into its variants and the utility they modify.
///
/// A `:` inside brackets is not a separator, since `[&>summary]:hidden` and
/// colour values keep theirs, so only top-level colons split.
fn split_variants(class: &str) -> (Vec<&str>, &str) {
    let mut variants = Vec::new();
    let mut rest = class;
    while let Some(i) = rest.find(':') {
        // `max-sm:` is two tokens with a hyphen, not a variant boundary problem;
        // a bracket group protects its own colons.
        if rest[..i].contains('[') {
            break;
        }
        variants.push(&rest[..i]);
        rest = &rest[i + 1..];
    }
    (variants, rest)
}

/// Maca's integrated Tailwind: turn a utility class into its CSS body. Covers
/// the common utilities (display/flex/grid, spacing, sizing, text, colors,
/// borders, rounding, overflow, position) generatively: a compact but real
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
        // `Pretendard Variable` first because that is the family name the
        // sheet the pages link (`apps/tomo/fonts/pretendard.css`, the upstream
        // variable dynamic subset) actually declares. Asking only for
        // `Pretendard` matched nothing it defines, so a page could link the
        // font, fetch it, and still render in system-ui. `Pretendard` stays
        // after it for the readers who have the static build installed.
        "font-sans" => {
            "font-family:'Pretendard Variable','Pretendard',ui-sans-serif,system-ui,\
             -apple-system,'Segoe UI',sans-serif"
        }
        // No Pretendard here, though it leads `font-sans`. It is the de-facto
        // Korean UI face and is installed on a large share of Korean machines,
        // and it is a *sans-serif*, so naming it first made every code block
        // and every inline `<code>` render proportionally for exactly the
        // readers the Korean pages are for.
        "font-mono" => "font-family:ui-monospace,'SF Mono','JetBrains Mono',Menlo,monospace",
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
        // Korean text breaks anywhere by default, so a heading splits a word
        // down the middle: `돌아갈` as `돌` / `아갈`. `keep-all` is the
        // convention every Korean site sets, and without this utility a page
        // built here could not express it.
        "break-keep" => "word-break:keep-all",
        "overflow-auto" => "overflow:auto",
        "overflow-hidden" => "overflow:hidden",
        "overflow-x-auto" => "overflow-x:auto",
        "overflow-y-auto" => "overflow-y:auto",
        "relative" => "position:relative",
        "absolute" => "position:absolute",
        "fixed" => "position:fixed",
        "sticky" => "position:sticky",
        "static" => "position:static",
        // list, table and typography bits a document needs and an app doesn't,
        // which is why they were missing until a book was built with this.
        "list-none" => "list-style:none",
        "list-disc" => "list-style:disc",
        "list-decimal" => "list-style:decimal",
        "border-collapse" => "border-collapse:collapse",
        "table" => "display:table",
        "table-auto" => "table-layout:auto",
        "table-fixed" => "table-layout:fixed",
        "text-inherit" => "color:inherit",
        "border-separate" => "border-collapse:separate",
        "align-top" => "vertical-align:top",
        "align-middle" => "vertical-align:middle",
        "align-baseline" => "vertical-align:baseline",
        "font-serif" => "font-family:ui-serif,Georgia,serif",
        "no-underline" => "text-decoration-line:none",
        "shrink-0" => "flex-shrink:0",
        "grow" => "flex-grow:1",
        "content-none" => "content:\"\"",
        "shadow" => "box-shadow:0 1px 3px rgba(0,0,0,.1)",
        "shadow-md" => "box-shadow:0 4px 8px rgba(0,0,0,.12)",
        "shadow-lg" => "box-shadow:0 4px 14px rgba(0,0,0,.14)",
        "shadow-none" => "box-shadow:none",
        "overscroll-contain" => "overscroll-behavior:contain",
        "appearance-none" => "appearance:none",
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
        "border-x" => {
            "border-left-width:1px;border-right-width:1px;\
                       border-left-style:solid;border-right-style:solid"
        }
        "border-y" => {
            "border-top-width:1px;border-bottom-width:1px;\
                       border-top-style:solid;border-bottom-style:solid"
        }
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
    // Arbitrary values: `text-[0.9em]`, `w-[42rem]`, `bg-[#191919]`. Without
    // these a utility system is a fixed menu, and anything outside the scale
    // sends you back to hand-written CSS, which is the whole thing this is
    // meant to replace. Underscores become spaces, as Tailwind does, so a value
    // with spaces can still live in a class attribute.
    if let Some(open) = class.find("[")
        && class.ends_with(']')
    {
        let prefix = class[..open].trim_end_matches('-');
        let raw = &class[open + 1..class.len() - 1].replace('_', " ");
        // A border width has to carry its style, whether the width came from
        // the scale or from brackets: CSS defaults `border-style` to `none`,
        // so a width on its own draws nothing at all.
        if let Some(sides) = border_sides(prefix) {
            return Some(
                sides
                    .iter()
                    .map(|s| format!("border-{s}-width:{raw};border-{s}-style:solid;"))
                    .collect(),
            );
        }
        if let Some(prop) = arbitrary_property(prefix) {
            return Some(format!("{prop}:{raw};"));
        }
    }
    // `underline-offset-2`, `leading-7`, `max-h-none`, `border-l-2`,
    // `decoration-zinc-400`: the last stragglers a text layout needs.
    if let Some(v) = class.strip_prefix("underline-offset-")
        && let Ok(n) = v.parse::<u32>()
    {
        return Some(format!("text-underline-offset:{n}px;"));
    }
    if let Some(v) = class.strip_prefix("leading-") {
        if let Some(n) = line_height(v) {
            return Some(format!("line-height:{n};"));
        }
        return Some(format!("line-height:{};", space(v)?));
    }
    if let Some(v) = class.strip_prefix("decoration-") {
        return Some(format!("text-decoration-color:{};", color(v)?));
    }
    // A width with no style is invisible: CSS defaults `border-style` to
    // `none`, so `border-l-2` drew nothing at all. The unparameterized
    // `border-l` always set both; these have to as well.
    if let Some(at) = class.rfind('-')
        && let Some(sides) = border_sides(&class[..at])
        && let Ok(n) = class[at + 1..].parse::<u32>()
    {
        return Some(
            sides
                .iter()
                .map(|s| format!("border-{s}-width:{n}px;border-{s}-style:solid;"))
                .collect(),
        );
    }
    // A heading that an in-page `#anchor` jumps to needs room above it, or the
    // sticky header lands on top of what you just jumped to.
    for (p, prop) in [
        ("scroll-mt-", "scroll-margin-top"),
        ("scroll-mb-", "scroll-margin-bottom"),
    ] {
        if let Some(v) = class.strip_prefix(p) {
            return Some(format!("{prop}:{};", space(v)?));
        }
    }
    for (p, prop) in [
        ("top-", "top"),
        ("right-", "right"),
        ("bottom-", "bottom"),
        ("left-", "left"),
    ] {
        if let Some(v) = class.strip_prefix(p) {
            return Some(format!("{prop}:{};", space(v)?));
        }
    }
    if let Some(v) = class.strip_prefix("max-w-") {
        if let Some(w) = container_width(v) {
            return Some(format!("max-width:{w};"));
        }
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
        // `mx-auto` is how anything gets centred, so the spacing scale has to
        // accept it even though it isn't a length.
        "auto" => "auto".into(),
        _ => {
            let n: f32 = v.parse().ok()?;
            format!("{}rem", n * 0.25)
        }
    })
}

/// Tailwind's named container widths, for `max-w-*`. These are what a column of
/// text is actually sized with (`max-w-2xl` is the standard reading measure),
/// so a documentation page needs them before it needs most of the numeric scale.
fn container_width(v: &str) -> Option<&'static str> {
    Some(match v {
        "xs" => "20rem",
        "sm" => "24rem",
        "md" => "28rem",
        "lg" => "32rem",
        "xl" => "36rem",
        "2xl" => "42rem",
        "3xl" => "48rem",
        "4xl" => "56rem",
        "5xl" => "64rem",
        "6xl" => "72rem",
        "7xl" => "80rem",
        "prose" => "65ch",
        "none" => "none",
        _ => return None,
    })
}

/// The edges a `border-…` utility draws on, or `None` if it isn't one. Shared
/// by the scale form (`border-y-2`) and the arbitrary one (`border-y-[3px]`)
/// so the two cannot disagree about which edges `y` means.
fn border_sides(prefix: &str) -> Option<&'static [&'static str]> {
    Some(match prefix {
        "border-l" => &["left"],
        "border-r" => &["right"],
        "border-t" => &["top"],
        "border-b" => &["bottom"],
        "border-x" => &["left", "right"],
        "border-y" => &["top", "bottom"],
        _ => return None,
    })
}

/// The CSS property an arbitrary-value class sets: `text-[…]` is a font size,
/// `w-[…]` a width, and so on.
fn arbitrary_property(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "text" => "font-size",
        "w" => "width",
        "h" => "height",
        "min-w" => "min-width",
        "min-h" => "min-height",
        "max-w" => "max-width",
        "max-h" => "max-height",
        "bg" => "background-color",
        "border" => "border-color",
        "p" => "padding",
        "px" => "padding-inline",
        "py" => "padding-block",
        "pt" => "padding-top",
        "pb" => "padding-bottom",
        "pl" => "padding-left",
        "pr" => "padding-right",
        "m" => "margin",
        "mx" => "margin-inline",
        "my" => "margin-block",
        "mt" => "margin-top",
        "mb" => "margin-bottom",
        "ml" => "margin-left",
        "mr" => "margin-right",
        "gap" => "gap",
        "gap-x" => "column-gap",
        "gap-y" => "row-gap",
        "top" => "top",
        "right" => "right",
        "bottom" => "bottom",
        "left" => "left",
        "inset" => "inset",
        // The arbitrary-value branch runs before the numeric parsers, so a
        // prefix missing from this table falls through to one that rejects
        // `[…]` and emits nothing. An arbitrary scroll-margin is the case a
        // sticky header of a non-scale height needs, which is what
        // `scroll-mt-*` was added for.
        "scroll-mt" => "scroll-margin-top",
        "scroll-mb" => "scroll-margin-bottom",
        "leading" => "line-height",
        "font" => "font-family",
        "content" => "content",
        "shadow" => "box-shadow",
        "grid-cols" => "grid-template-columns",
        _ => return None,
    })
}

/// Tailwind's named line heights.
fn line_height(v: &str) -> Option<&'static str> {
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

/// Width/height value: spacing scale, plus `full`/`screen`/`auto`/fractions.
fn size(v: &str) -> Option<String> {
    Some(match v {
        "none" => "none".into(),
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

/// Escape a class name for use in a CSS selector.
///
/// Utility names are full of characters a selector treats as syntax: `.` in
/// `text-[0.9em]`, `:` in every variant, and brackets, parens, commas, `#` and
/// quotes once arbitrary values are allowed. An unescaped one doesn't warn; the
/// browser silently drops the whole rule.
pub fn css_escape(c: &str) -> String {
    let mut out = String::with_capacity(c.len() + 8);
    for ch in c.chars() {
        if matches!(
            ch,
            '/' | '.'
                | ':'
                | '['
                | ']'
                | '('
                | ')'
                | ','
                | '#'
                | '%'
                | '\''
                | '"'
                | '!'
                | '$'
                | '&'
                | '*'
                | '+'
                | ';'
                | '<'
                | '='
                | '>'
                | '?'
                | '@'
                | '^'
                | '`'
                | '{'
                | '|'
                | '}'
                | '~'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Every whitespace-separated token of every string literal reachable from an
/// item, as Tailwind class candidates.
///
/// Not just `class=` attributes: a program that builds its class list in a
/// helper, such as `md_class("pre")`, has the literals in that helper's body,
/// and collecting only at the call site finds nothing. Tokens that aren't
/// utilities simply generate no rule.
pub fn collect_class_strings(s: &Stmt, out: &mut BTreeSet<String>) {
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
                collect_class_expr(arg_expr(a), out);
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

/// Flatten a `A | B(int) | C` union into each variant's name and payload arity.
/// A nullary variant parses as a bare `Ident`, one with a payload as a `Call`.
fn union_arms(e: &Expr, out: &mut BTreeMap<String, usize>) {
    match e {
        Expr::Binary {
            op: BinOp::Union,
            lhs,
            rhs,
        } => {
            union_arms(lhs, out);
            union_arms(rhs, out);
        }
        Expr::Ident(n) => {
            out.insert(n.clone(), 0);
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(n) = callee.as_ref() {
                out.insert(n.clone(), args.len());
            }
        }
        _ => {}
    }
}

/// Every sum variant declared in the module, with its payload arity.
fn collect_variants(m: &Module) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for item in &m.items {
        if let Stmt::Bind(b) = item
            && sum_variants(&b.value).is_some()
        {
            union_arms(&b.value, &mut out);
        }
    }
    out
}

/// A tagged object per variant, so `match` can test the tag and read payloads.
/// A nullary variant is a single shared value; one with a payload is a function,
/// which is what makes the surface `Circle(2.0)` a plain call needing no special
/// case in `jexpr`.
fn variant_ctors(vs: &BTreeMap<String, usize>) -> String {
    let mut out = String::new();
    for (name, arity) in vs {
        if *arity == 0 {
            out.push_str(&format!("const {name} = {{ $: {name:?} }};\n"));
        } else {
            let ps: Vec<String> = (0..*arity).map(|i| format!("_{i}")).collect();
            out.push_str(&format!(
                "function {name}({}) {{ return {{ $: {name:?}, {} }}; }}\n",
                ps.join(", "),
                ps.join(", ")
            ));
        }
    }
    out
}
